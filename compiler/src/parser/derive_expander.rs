use std::env;
use std::process::Command;

use crate::error::{CompileError, ParseError};
use crate::Result;

use super::Parser;

#[derive(Debug, Clone, Copy)]
enum DeriveTargetKind {
    Struct,
    Enum,
    Class,
}

impl DeriveTargetKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Struct => "struct",
            Self::Enum => "enum",
            Self::Class => "class",
        }
    }
}

#[derive(Debug, Clone)]
struct DeriveTarget {
    name: String,
    kind: DeriveTargetKind,
    insert_after: usize,
}

#[derive(Debug, Clone)]
struct DeriveInvocation {
    derives: Vec<String>,
    target: DeriveTarget,
}

pub(super) fn expand_derive_macros(source: &str) -> Result<String> {
    let (without_attributes, invocations) = collect_derive_invocations(source)?;
    if invocations.is_empty() {
        return Ok(source.to_string());
    }

    let mut expanded = without_attributes;
    let mut insertions: Vec<(usize, String)> = Vec::new();

    for invocation in &invocations {
        for derive in &invocation.derives {
            let generated = execute_derive_macro(derive, &invocation.target)?;
            if generated.trim().is_empty() {
                return Err(parse_error(format!(
                    "derive macro `{derive}` generated empty output for {} `{}`",
                    invocation.target.kind.as_str(),
                    invocation.target.name
                )));
            }
            insertions.push((invocation.target.insert_after, format!("\n{}\n", generated)));
        }
    }

    insertions.sort_by_key(|(pos, _)| *pos);
    let mut offset = 0usize;
    for (pos, snippet) in insertions {
        let insert_at = pos.saturating_add(offset);
        if insert_at > expanded.len() {
            return Err(parse_error(
                "internal derive expansion error: insertion index out of bounds",
            ));
        }
        expanded.insert_str(insert_at, &snippet);
        offset += snippet.len();
    }

    validate_expanded_source(&expanded)?;
    Ok(expanded)
}

fn collect_derive_invocations(source: &str) -> Result<(String, Vec<DeriveInvocation>)> {
    let bytes = source.as_bytes();
    let mut invocations = Vec::new();
    let mut output = Vec::with_capacity(bytes.len());

    let mut i = 0;
    let mut last_copy = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' {
            if let Some((derives, attr_end)) = parse_derive_attribute(source, bytes, i)? {
                output.extend_from_slice(&bytes[last_copy..i]);
                output.extend_from_slice(&mask_removed_attribute(&bytes[i..attr_end]));
                let target = find_derive_target(source, bytes, attr_end)?;
                invocations.push(DeriveInvocation { derives, target });
                i = attr_end;
                last_copy = attr_end;
                continue;
            }
        }
        i += 1;
    }

    output.extend_from_slice(&bytes[last_copy..]);
    let stripped = String::from_utf8(output)
        .map_err(|_| parse_error("derive expansion produced invalid utf-8"))?;
    Ok((stripped, invocations))
}

fn parse_derive_attribute(
    source: &str,
    bytes: &[u8],
    start: usize,
) -> Result<Option<(Vec<String>, usize)>> {
    if bytes.get(start) != Some(&b'#') {
        return Ok(None);
    }

    let mut cursor = start + 1;
    cursor = skip_ws(bytes, cursor);
    if bytes.get(cursor) != Some(&b'[') {
        return Ok(None);
    }
    cursor += 1;
    cursor = skip_ws(bytes, cursor);

    let Some((name_start, name_end)) = parse_ident_range(bytes, cursor) else {
        return Ok(None);
    };
    if &source[name_start..name_end] != "derive" {
        return Ok(None);
    }

    cursor = skip_ws(bytes, name_end);
    if bytes.get(cursor) != Some(&b'(') {
        return Err(parse_error("`#[derive(...)]` requires parentheses"));
    }
    let (list_start, list_end, after_list) = parse_balanced(bytes, cursor, b'(', b')')?;
    let derives = parse_derive_list(&source[list_start..list_end])?;

    cursor = skip_ws(bytes, after_list);
    if bytes.get(cursor) != Some(&b']') {
        return Err(parse_error("`#[derive(...)]` is missing closing `]`"));
    }

    Ok(Some((derives, cursor + 1)))
}

fn parse_derive_list(value: &str) -> Result<Vec<String>> {
    let mut derives = Vec::new();
    for item in value.split(',') {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !is_path(trimmed) {
            return Err(parse_error(format!(
                "invalid derive macro path `{trimmed}`; expected identifiers separated by `::`"
            )));
        }
        derives.push(trimmed.to_string());
    }

    if derives.is_empty() {
        return Err(parse_error("derive attribute requires at least one macro"));
    }
    Ok(derives)
}

fn find_derive_target(source: &str, bytes: &[u8], from: usize) -> Result<DeriveTarget> {
    let mut cursor = skip_ws_and_comments(bytes, from);

    loop {
        if bytes.get(cursor) != Some(&b'#') {
            break;
        }
        let attr = skip_ws(bytes, cursor + 1);
        if bytes.get(attr) != Some(&b'[') {
            break;
        }
        let (_, _, after_attr) = parse_balanced(bytes, attr, b'[', b']')?;
        cursor = skip_ws_and_comments(bytes, after_attr);
    }

    if let Some((kw_start, kw_end)) = parse_ident_range(bytes, cursor) {
        if &source[kw_start..kw_end] == "pub" {
            cursor = skip_ws_and_comments(bytes, kw_end);
            if bytes.get(cursor) == Some(&b'(') {
                let (_, _, after_vis) = parse_balanced(bytes, cursor, b'(', b')')?;
                cursor = skip_ws_and_comments(bytes, after_vis);
            }
        }
    }

    let (kind_start, kind_end) = parse_ident_range(bytes, cursor)
        .ok_or_else(|| parse_error("derive attribute must be followed by a declaration"))?;
    let kind = match &source[kind_start..kind_end] {
        "struct" => DeriveTargetKind::Struct,
        "enum" => DeriveTargetKind::Enum,
        "class" => DeriveTargetKind::Class,
        found => {
            return Err(parse_error(format!(
                "derive attribute is only supported on struct/enum/class declarations, found `{found}`"
            )));
        }
    };

    cursor = skip_ws_and_comments(bytes, kind_end);
    let (name_start, name_end) = parse_ident_range(bytes, cursor)
        .ok_or_else(|| parse_error("failed to resolve derive target type name"))?;
    let insert_after = find_declaration_end(bytes, kind_start)?;

    Ok(DeriveTarget {
        name: source[name_start..name_end].to_string(),
        kind,
        insert_after,
    })
}

fn execute_derive_macro(derive: &str, target: &DeriveTarget) -> Result<String> {
    let env_key = format!("SENGOO_DERIVE_{}", to_env_macro_key(derive));
    if let Ok(command) = env::var(&env_key) {
        if !command.trim().is_empty() {
            return run_external_derive(&command, derive, target);
        }
    }

    Ok(generate_builtin_derive(derive, target))
}

fn run_external_derive(command: &str, derive: &str, target: &DeriveTarget) -> Result<String> {
    let output = Command::new(command)
        .env("SENGOO_DERIVE_NAME", derive)
        .env("SENGOO_DERIVE_TARGET", &target.name)
        .env("SENGOO_DERIVE_TARGET_KIND", target.kind.as_str())
        .output()
        .map_err(|e| {
            parse_error(format!(
                "failed to execute derive macro command `{command}` for `{derive}`: {e}"
            ))
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(parse_error(format!(
            "derive macro command `{command}` failed for `{derive}`: {stderr}"
        )));
    }

    let generated = String::from_utf8(output.stdout)
        .map_err(|_| parse_error(format!("derive macro `{derive}` output is not valid utf-8")))?;
    if generated.trim().is_empty() {
        return Err(parse_error(format!(
            "derive macro `{derive}` command `{command}` returned empty output"
        )));
    }
    Ok(generated)
}

fn generate_builtin_derive(derive: &str, target: &DeriveTarget) -> String {
    let method = format!("__derive_{}", sanitize_ident(derive));
    format!(
        "impl {} {{\n    def {}(self) -> i64 {{\n        1\n    }}\n}}",
        target.name, method
    )
}

fn validate_expanded_source(source: &str) -> Result<()> {
    let mut parser = Parser::new(source);
    parser.parse_program()?;
    if parser.has_errors() {
        return Err(parse_error(
            "post-expansion validation failed for derive output",
        ));
    }
    Ok(())
}

fn sanitize_ident(value: &str) -> String {
    let mut result = String::new();
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch.to_ascii_lowercase());
        } else {
            result.push('_');
        }
    }

    let mut collapsed = String::new();
    let mut prev_underscore = false;
    for ch in result.chars() {
        if ch == '_' {
            if !prev_underscore {
                collapsed.push(ch);
            }
            prev_underscore = true;
        } else {
            collapsed.push(ch);
            prev_underscore = false;
        }
    }

    let mut sanitized = collapsed.trim_matches('_').to_string();
    if sanitized.is_empty() {
        sanitized = "derived".to_string();
    }
    if !is_ident_start(sanitized.as_bytes()[0]) {
        sanitized.insert(0, 'd');
    }
    sanitized
}

fn to_env_macro_key(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_uppercase()
            } else {
                '_'
            }
        })
        .collect()
}

fn is_path(value: &str) -> bool {
    value.split("::").all(is_ident)
}

fn is_ident(value: &str) -> bool {
    let bytes = value.as_bytes();
    parse_ident_range(bytes, 0)
        .map(|(_, end)| end == bytes.len())
        .unwrap_or(false)
}

fn skip_ws_and_comments(bytes: &[u8], mut i: usize) -> usize {
    loop {
        i = skip_ws(bytes, i);
        if let Some(next) = skip_comment(bytes, i) {
            i = next;
            continue;
        }
        break;
    }
    i
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
}

fn mask_removed_attribute(bytes: &[u8]) -> Vec<u8> {
    bytes
        .iter()
        .map(|b| if *b == b'\n' || *b == b'\r' { *b } else { b' ' })
        .collect()
}

fn find_declaration_end(bytes: &[u8], start: usize) -> Result<usize> {
    let mut i = start;
    while i < bytes.len() {
        if let Some(next) = skip_quoted_literal(bytes, i) {
            i = next;
            continue;
        }
        if let Some(next) = skip_comment(bytes, i) {
            i = next;
            continue;
        }

        match bytes[i] {
            b'{' => {
                let (_, _, after) = parse_balanced(bytes, i, b'{', b'}')?;
                return Ok(after);
            }
            b'(' => {
                let (_, _, after_paren) = parse_balanced(bytes, i, b'(', b')')?;
                let after = skip_ws_and_comments(bytes, after_paren);
                if bytes.get(after) == Some(&b';') {
                    return Ok(after + 1);
                }
                return Ok(after_paren);
            }
            b';' => return Ok(i + 1),
            _ => i += 1,
        }
    }

    Err(parse_error(
        "failed to locate end of declaration for derive target",
    ))
}

fn skip_comment(bytes: &[u8], start: usize) -> Option<usize> {
    if bytes.get(start) != Some(&b'/') {
        return None;
    }
    if bytes.get(start + 1) == Some(&b'/') {
        let mut i = start + 2;
        while i < bytes.len() && bytes[i] != b'\n' {
            i += 1;
        }
        return Some(i);
    }
    if bytes.get(start + 1) == Some(&b'*') {
        let mut i = start + 2;
        while i + 1 < bytes.len() {
            if bytes[i] == b'*' && bytes[i + 1] == b'/' {
                return Some(i + 2);
            }
            i += 1;
        }
        return Some(bytes.len());
    }
    None
}

fn parse_balanced(
    bytes: &[u8],
    start: usize,
    open: u8,
    close: u8,
) -> Result<(usize, usize, usize)> {
    if bytes.get(start) != Some(&open) {
        return Err(parse_error("expected opening delimiter"));
    }

    let mut depth = 1usize;
    let mut i = start + 1;
    while i < bytes.len() {
        if let Some(next) = skip_quoted_literal(bytes, i) {
            i = next;
            continue;
        }
        if let Some(next) = skip_comment(bytes, i) {
            i = next;
            continue;
        }

        if bytes[i] == open {
            depth += 1;
            i += 1;
            continue;
        }
        if bytes[i] == close {
            depth -= 1;
            if depth == 0 {
                return Ok((start + 1, i, i + 1));
            }
            i += 1;
            continue;
        }
        i += 1;
    }

    Err(parse_error("unbalanced delimiters in derive attribute"))
}

fn skip_quoted_literal(bytes: &[u8], start: usize) -> Option<usize> {
    let quote = *bytes.get(start)?;
    if quote != b'"' && quote != b'\'' {
        return None;
    }
    let mut i = start + 1;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i = i.saturating_add(2),
            b if b == quote => return Some(i + 1),
            _ => i += 1,
        }
    }
    Some(bytes.len())
}

fn parse_ident_range(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    if start >= bytes.len() || !is_ident_start(bytes[start]) {
        return None;
    }
    let mut end = start + 1;
    while end < bytes.len() && is_ident_continue(bytes[end]) {
        end += 1;
    }
    Some((start, end))
}

fn is_ident_start(value: u8) -> bool {
    value.is_ascii_alphabetic() || value == b'_'
}

fn is_ident_continue(value: u8) -> bool {
    value.is_ascii_alphanumeric() || value == b'_'
}

fn parse_error(message: impl Into<String>) -> CompileError {
    CompileError::ParseError(ParseError::InvalidPattern(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_derive_attribute_into_impl_block() {
        let source = r#"
#[derive(Auto)]
struct User {
    id: i64,
}
"#;

        let expanded = expand_derive_macros(source).expect("derive expansion should succeed");
        assert!(!expanded.contains("#[derive"));
        assert!(expanded.contains("impl User"));
        assert!(expanded.contains("__derive_auto"));
    }

    #[test]
    fn derive_attribute_on_invalid_target_is_rejected() {
        let source = r#"
#[derive(Auto)]
const FLAG: i64 = 1;
"#;

        let err = expand_derive_macros(source).expect_err("derive on const should fail");
        assert!(err
            .to_string()
            .contains("derive attribute is only supported"));
    }
}
