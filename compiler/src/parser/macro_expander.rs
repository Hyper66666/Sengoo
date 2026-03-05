use std::collections::HashMap;

use crate::error::{CompileError, ParseError};
use crate::Result;

const MAX_EXPANSION_PASSES: usize = 16;
const MACRO_RULES_PREFIX: &[u8] = b"macro_rules!";

#[derive(Debug, Clone)]
struct PatternParam {
    name: String,
    fragment: String,
}

#[derive(Debug, Clone)]
struct MacroArm {
    params: Vec<PatternParam>,
    template: String,
}

#[derive(Debug, Clone, Default)]
struct DeclarativeMacro {
    arms: Vec<MacroArm>,
}

pub(super) fn expand_declarative_macros(source: &str) -> Result<String> {
    let (without_definitions, macros) = extract_macro_definitions(source)?;
    if macros.is_empty() {
        return Ok(source.to_string());
    }
    expand_macro_invocations(&without_definitions, &macros)
}

fn extract_macro_definitions(source: &str) -> Result<(String, HashMap<String, DeclarativeMacro>)> {
    let bytes = source.as_bytes();
    let mut macros = HashMap::new();
    let mut output = Vec::with_capacity(bytes.len());

    let mut i = 0;
    let mut last_copy = 0;
    while i < bytes.len() {
        if is_macro_rules_at(bytes, i) {
            output.extend_from_slice(&bytes[last_copy..i]);
            let (name, definition, next) = parse_macro_definition(bytes, i)?;
            if macros.insert(name.clone(), definition).is_some() {
                return Err(parse_error(format!("duplicate macro definition `{name}`")));
            }
            i = next;
            last_copy = next;
            continue;
        }
        i += 1;
    }

    output.extend_from_slice(&bytes[last_copy..]);
    let source_without_definitions = String::from_utf8(output)
        .map_err(|_| parse_error("expanded source contains invalid utf-8"))?;
    Ok((source_without_definitions, macros))
}

fn expand_macro_invocations(source: &str, macros: &HashMap<String, DeclarativeMacro>) -> Result<String> {
    let mut expanded = source.to_string();

    for pass in 0..MAX_EXPANSION_PASSES {
        let bytes = expanded.as_bytes();
        let mut output = Vec::with_capacity(bytes.len());
        let mut changed = false;
        let mut i = 0;

        while i < bytes.len() {
            if let Some(next) = skip_quoted_literal(bytes, i) {
                output.extend_from_slice(&bytes[i..next]);
                i = next;
                continue;
            }

            if let Some(next) = skip_comment(bytes, i) {
                output.extend_from_slice(&bytes[i..next]);
                i = next;
                continue;
            }

            if !is_ident_start(bytes[i]) {
                output.push(bytes[i]);
                i += 1;
                continue;
            }

            let (name_start, name_end) = parse_ident_range(bytes, i)
                .ok_or_else(|| parse_error("internal macro parser error while reading identifier"))?;
            let mut cursor = skip_ws(bytes, name_end);
            if cursor >= bytes.len() || bytes[cursor] != b'!' {
                output.extend_from_slice(&bytes[i..name_end]);
                i = name_end;
                continue;
            }

            cursor = skip_ws(bytes, cursor + 1);
            if cursor >= bytes.len() {
                output.extend_from_slice(&bytes[i..name_end]);
                i = name_end;
                continue;
            }

            let Some(close) = matching_delim(bytes[cursor]) else {
                output.extend_from_slice(&bytes[i..name_end]);
                i = name_end;
                continue;
            };
            let (arg_start, arg_end, after_invocation) = parse_balanced(bytes, cursor, bytes[cursor], close)?;
            let macro_name = &expanded[name_start..name_end];
            let args = &expanded[arg_start..arg_end];

            let Some(definition) = macros.get(macro_name) else {
                return Err(parse_error(format!("unknown macro `{macro_name}`")));
            };

            let expansion = expand_single_invocation(macro_name, definition, args)?;
            output.extend_from_slice(expansion.as_bytes());
            i = after_invocation;
            changed = true;
        }

        let next = String::from_utf8(output)
            .map_err(|_| parse_error("expanded source contains invalid utf-8"))?;
        if !changed {
            return Ok(next);
        }
        expanded = next;

        if pass + 1 == MAX_EXPANSION_PASSES {
            return Err(parse_error("macro expansion exceeded recursion/pass limit"));
        }
    }

    Ok(expanded)
}

fn expand_single_invocation(name: &str, definition: &DeclarativeMacro, args: &str) -> Result<String> {
    let args = split_top_level(args)?
        .into_iter()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .collect::<Vec<_>>();

    for arm in &definition.arms {
        if arm.params.len() != args.len() {
            continue;
        }

        if !arm
            .params
            .iter()
            .zip(args.iter())
            .all(|(param, arg)| fragment_accepts(&param.fragment, arg))
        {
            continue;
        }

        let captures = arm
            .params
            .iter()
            .zip(args.iter())
            .map(|(param, value)| (param.name.clone(), value.clone()))
            .collect::<HashMap<_, _>>();

        return Ok(apply_template(&arm.template, &captures));
    }

    Err(parse_error(format!(
        "no matching macro arm for `{name}` with {} argument(s)",
        args.len()
    )))
}

fn parse_macro_definition(bytes: &[u8], start: usize) -> Result<(String, DeclarativeMacro, usize)> {
    let mut cursor = start + MACRO_RULES_PREFIX.len();
    cursor = skip_ws(bytes, cursor);

    let (name_start, name_end) = parse_ident_range(bytes, cursor)
        .ok_or_else(|| parse_error("expected macro name after `macro_rules!`"))?;
    let name = String::from_utf8(bytes[name_start..name_end].to_vec())
        .map_err(|_| parse_error("macro name contains invalid utf-8"))?;
    cursor = skip_ws(bytes, name_end);

    if bytes.get(cursor) != Some(&b'{') {
        return Err(parse_error(format!(
            "expected `{{` after macro name `{name}` in `macro_rules!`"
        )));
    }

    let (body_start, body_end, next) = parse_balanced(bytes, cursor, b'{', b'}')?;
    let body = &bytes[body_start..body_end];
    let arms = parse_macro_arms(body, &name)?;
    Ok((name, DeclarativeMacro { arms }, next))
}

fn parse_macro_arms(body: &[u8], macro_name: &str) -> Result<Vec<MacroArm>> {
    let mut arms = Vec::new();
    let mut i = 0;

    while i < body.len() {
        i = skip_ws(body, i);
        if i >= body.len() {
            break;
        }

        let Some(pattern_close) = matching_delim(body[i]) else {
            return Err(parse_error(format!(
                "invalid macro arm in `{macro_name}`: expected pattern delimiter"
            )));
        };

        let (pattern_start, pattern_end, after_pattern) =
            parse_balanced(body, i, body[i], pattern_close)?;
        let pattern = String::from_utf8(body[pattern_start..pattern_end].to_vec())
            .map_err(|_| parse_error("macro pattern contains invalid utf-8"))?;
        i = skip_ws(body, after_pattern);

        if body.get(i) != Some(&b'=') || body.get(i + 1) != Some(&b'>') {
            return Err(parse_error(format!(
                "invalid macro arm in `{macro_name}`: expected `=>` after pattern"
            )));
        }
        i = skip_ws(body, i + 2);

        let Some(template_close) = matching_delim(body.get(i).copied().unwrap_or_default()) else {
            return Err(parse_error(format!(
                "invalid macro arm in `{macro_name}`: expected expansion delimiters"
            )));
        };
        let (template_start, template_end, after_template) =
            parse_balanced(body, i, body[i], template_close)?;
        let template = String::from_utf8(body[template_start..template_end].to_vec())
            .map_err(|_| parse_error("macro template contains invalid utf-8"))?;
        let params = parse_pattern_params(&pattern)?;
        arms.push(MacroArm { params, template });

        i = skip_ws(body, after_template);
        if body.get(i) == Some(&b';') {
            i += 1;
        }
    }

    if arms.is_empty() {
        return Err(parse_error(format!(
            "macro `{macro_name}` must define at least one arm"
        )));
    }
    Ok(arms)
}

fn parse_pattern_params(pattern: &str) -> Result<Vec<PatternParam>> {
    let mut params = Vec::new();
    for item in split_top_level(pattern)? {
        let trimmed = item.trim();
        if trimmed.is_empty() {
            continue;
        }
        if !trimmed.starts_with('$') {
            return Err(parse_error(
                "macro pattern currently supports only `$name:fragment` placeholders",
            ));
        }

        let Some((name, fragment)) = trimmed[1..].split_once(':') else {
            return Err(parse_error("invalid macro placeholder, expected `$name:fragment`"));
        };
        let name = name.trim();
        let fragment = fragment.trim();

        if !is_ident(name) {
            return Err(parse_error(format!(
                "invalid macro capture name `{name}` in pattern"
            )));
        }
        if !is_ident(fragment) {
            return Err(parse_error(format!(
                "invalid macro fragment kind `{fragment}` in pattern"
            )));
        }

        params.push(PatternParam {
            name: name.to_string(),
            fragment: fragment.to_string(),
        });
    }
    Ok(params)
}

fn apply_template(template: &str, captures: &HashMap<String, String>) -> String {
    let bytes = template.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == b'$' {
            if let Some((name_start, name_end)) = parse_ident_range(bytes, i + 1) {
                let key = &template[name_start..name_end];
                if let Some(value) = captures.get(key) {
                    output.extend_from_slice(value.as_bytes());
                    i = name_end;
                    continue;
                }
            }
        }
        output.push(bytes[i]);
        i += 1;
    }

    String::from_utf8(output).unwrap_or_default()
}

fn split_top_level(input: &str) -> Result<Vec<&str>> {
    let bytes = input.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0;
    let mut i = 0;
    let mut paren_depth = 0i32;
    let mut brace_depth = 0i32;
    let mut bracket_depth = 0i32;
    let mut angle_depth = 0i32;

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
            b'(' => paren_depth += 1,
            b')' => paren_depth -= 1,
            b'{' => brace_depth += 1,
            b'}' => brace_depth -= 1,
            b'[' => bracket_depth += 1,
            b']' => bracket_depth -= 1,
            b'<' => angle_depth += 1,
            b'>' => angle_depth -= 1,
            b',' if paren_depth == 0 && brace_depth == 0 && bracket_depth == 0 && angle_depth == 0 => {
                parts.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
        i += 1;
    }

    if paren_depth != 0 || brace_depth != 0 || bracket_depth != 0 || angle_depth != 0 {
        return Err(parse_error(
            "unbalanced delimiters while splitting macro arguments/pattern",
        ));
    }

    parts.push(&input[start..]);
    Ok(parts)
}

fn fragment_accepts(fragment: &str, arg: &str) -> bool {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return false;
    }

    match fragment {
        "ident" => is_ident(trimmed),
        "literal" => {
            trimmed.starts_with('"')
                || trimmed.starts_with('\'')
                || trimmed
                    .chars()
                    .next()
                    .map(|c| c.is_ascii_digit())
                    .unwrap_or(false)
        }
        "block" => trimmed.starts_with('{') && trimmed.ends_with('}'),
        "expr" | "ty" | "path" | "stmt" | "tt" | "pat" => true,
        _ => false,
    }
}

fn is_macro_rules_at(bytes: &[u8], index: usize) -> bool {
    if index + MACRO_RULES_PREFIX.len() > bytes.len() {
        return false;
    }
    if &bytes[index..index + MACRO_RULES_PREFIX.len()] != MACRO_RULES_PREFIX {
        return false;
    }
    index == 0 || !is_ident_continue(bytes[index - 1])
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

    let mut i = start + 1;
    let mut depth = 1usize;
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

    Err(parse_error("unbalanced delimiters in macro definition/invocation"))
}

fn skip_ws(bytes: &[u8], mut i: usize) -> usize {
    while i < bytes.len() && bytes[i].is_ascii_whitespace() {
        i += 1;
    }
    i
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

fn is_ident(value: &str) -> bool {
    let bytes = value.as_bytes();
    parse_ident_range(bytes, 0)
        .map(|(_, end)| end == bytes.len())
        .unwrap_or(false)
}

fn is_ident_start(byte: u8) -> bool {
    byte.is_ascii_alphabetic() || byte == b'_'
}

fn is_ident_continue(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn matching_delim(open: u8) -> Option<u8> {
    match open {
        b'(' => Some(b')'),
        b'{' => Some(b'}'),
        b'[' => Some(b']'),
        b'<' => Some(b'>'),
        _ => None,
    }
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

fn parse_error(message: impl Into<String>) -> CompileError {
    CompileError::ParseError(ParseError::InvalidPattern(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expands_simple_macro_invocation() {
        let source = r#"
macro_rules! add_one {
    ($value:expr) => { $value + 1 };
}

def main() -> i64 {
    add_one!(41)
}
"#;

        let expanded = expand_declarative_macros(source).expect("macro expansion should succeed");
        assert!(!expanded.contains("macro_rules!"));
        assert!(expanded.contains("41 + 1"));
    }
}
