//! Pre-parse attribute matrix: `cfg(target_os)`, `deprecated`, and validation.
//!
//! `#[derive(...)]` is left for `derive_expander`. FFI export attributes are
//! handled by the parser on `extern` declarations.

use std::borrow::Cow;
use std::cell::RefCell;

use crate::error::{CompileError, ParseError};
use crate::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceDeclKind {
    Struct,
    Enum,
    Class,
    Trait,
    Impl,
    Function,
    Const,
    Other,
}

#[derive(Debug, Clone)]
pub struct DeprecatedDecl {
    pub name: String,
    pub kind: String,
    pub message: Option<String>,
}

#[derive(Debug, Default)]
struct AttributeState {
    deprecated: Vec<DeprecatedDecl>,
}

thread_local! {
    static ATTRIBUTE_STATE: RefCell<Option<AttributeState>> = const { RefCell::new(None) };
    static CFG_FEATURES: RefCell<Vec<String>> = const { RefCell::new(Vec::new()) };
}

struct CfgFeatureGuard {
    previous: Option<Vec<String>>,
}

impl Drop for CfgFeatureGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            CFG_FEATURES.with(|state| {
                *state.borrow_mut() = previous;
            });
        }
    }
}

/// Evaluate `#[cfg(feature = "...")]` against a selected package feature set.
///
/// Standalone parses use the default empty set, so feature predicates evaluate
/// false until a caller with manifest context supplies selected package
/// features here.
pub fn with_cfg_features<I, S, R>(features: I, f: impl FnOnce() -> R) -> R
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let features = features
        .into_iter()
        .map(|feature| feature.as_ref().to_string())
        .collect::<Vec<_>>();
    let previous = CFG_FEATURES.with(|state| std::mem::replace(&mut *state.borrow_mut(), features));
    let _guard = CfgFeatureGuard {
        previous: Some(previous),
    };
    f()
}

fn reset_state() {
    ATTRIBUTE_STATE.with(|state| {
        *state.borrow_mut() = Some(AttributeState::default());
    });
}

pub fn take_deprecated_decls() -> Vec<DeprecatedDecl> {
    ATTRIBUTE_STATE.with(|state| {
        state
            .borrow_mut()
            .as_mut()
            .map(|entry| std::mem::take(&mut entry.deprecated))
            .unwrap_or_default()
    })
}

pub(super) fn process_surface_attributes(source: &str) -> Result<Cow<'_, str>> {
    if !source.contains('#') {
        return Ok(Cow::Borrowed(source));
    }

    reset_state();
    let bytes = source.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut cursor = 0usize;
    let mut emitted_to = 0usize;

    while cursor < bytes.len() {
        let ws_start = cursor;
        cursor = skip_ws_and_comments(bytes, cursor);
        if cursor >= bytes.len() {
            output.extend_from_slice(&bytes[emitted_to..]);
            break;
        }

        output.extend_from_slice(&bytes[emitted_to..ws_start]);
        output.extend_from_slice(&bytes[ws_start..cursor]);

        let decl_start = cursor;
        let mut decl_cursor = skip_attribute_regions(bytes, cursor);
        decl_cursor = skip_visibility(bytes, decl_cursor);
        decl_cursor = skip_keyword(bytes, decl_cursor, b"unsafe");
        decl_cursor = skip_keyword(bytes, decl_cursor, b"async");

        if starts_with_keyword(bytes, decl_cursor, b"extern") {
            let decl_end = find_declaration_end(bytes, decl_cursor)?;
            output.extend_from_slice(&bytes[decl_start..decl_end]);
            cursor = decl_end;
            emitted_to = decl_end;
            continue;
        }

        let (attrs, after_attrs) = collect_leading_attributes(source, bytes, cursor)?;
        cursor = after_attrs;

        cursor = skip_visibility(bytes, cursor);
        cursor = skip_keyword(bytes, cursor, b"unsafe");
        cursor = skip_keyword(bytes, cursor, b"async");

        let Some((decl_kind, name, decl_end)) = locate_top_level_decl(source, bytes, cursor)?
        else {
            output.push(bytes[cursor]);
            cursor += 1;
            emitted_to = cursor;
            continue;
        };

        if decl_kind == SurfaceDeclKind::Other {
            output.extend_from_slice(&bytes[decl_start..decl_end]);
            cursor = decl_end;
            emitted_to = decl_end;
            continue;
        }

        let action = evaluate_attributes(source, attrs, decl_kind, &name, decl_start)?;
        match action {
            AttributeAction::Keep { mask_ranges } => {
                append_decl_with_masked_attrs(
                    &mut output,
                    &bytes[decl_start..decl_end],
                    &mask_ranges,
                );
                cursor = decl_end;
                emitted_to = decl_end;
            }
            AttributeAction::Remove => {
                cursor = decl_end;
                emitted_to = decl_end;
            }
        }
    }

    if emitted_to < bytes.len() {
        output.extend_from_slice(&bytes[emitted_to..]);
    }

    Ok(Cow::Owned(String::from_utf8(output).map_err(|_| {
        parse_error("attribute expansion produced invalid utf-8")
    })?))
}

#[derive(Debug)]
enum ParsedAttribute {
    Derive,
    Cfg(CfgPredicate),
    Deprecated(Option<String>),
    Unsupported { name: String, span_start: usize },
}

#[derive(Debug)]
enum CfgPredicate {
    TargetOs(String),
    TargetFamily(String),
    Feature(String),
    All(Vec<CfgPredicate>),
    Any(Vec<CfgPredicate>),
    Not(Box<CfgPredicate>),
}

type ParsedAttributeSpan = (usize, usize, ParsedAttribute);

#[derive(Debug)]
enum AttributeAction {
    Keep { mask_ranges: Vec<(usize, usize)> },
    Remove,
}

fn append_decl_with_masked_attrs(
    output: &mut Vec<u8>,
    decl_slice: &[u8],
    mask_ranges: &[(usize, usize)],
) {
    if mask_ranges.is_empty() {
        output.extend_from_slice(decl_slice);
        return;
    }

    let mut pos = 0usize;
    for &(start, end) in mask_ranges {
        if start > pos {
            output.extend_from_slice(&decl_slice[pos..start]);
        }
        for byte in &decl_slice[start..end] {
            output.push(if *byte == b'\n' || *byte == b'\r' {
                *byte
            } else {
                b' '
            });
        }
        pos = end;
    }
    if pos < decl_slice.len() {
        output.extend_from_slice(&decl_slice[pos..]);
    }
}

fn evaluate_attributes(
    _source: &str,
    attrs: Vec<ParsedAttributeSpan>,
    decl_kind: SurfaceDeclKind,
    decl_name: &str,
    decl_start: usize,
) -> Result<AttributeAction> {
    if attrs.is_empty() {
        return Ok(AttributeAction::Keep {
            mask_ranges: Vec::new(),
        });
    }

    let mut cfg_enabled = true;
    let mut mask_ranges = Vec::new();

    for (start, end, attr) in attrs {
        let rel_start = start.saturating_sub(decl_start);
        let rel_end = end.saturating_sub(decl_start);
        match attr {
            ParsedAttribute::Derive => {
                if !allows_derive(decl_kind) {
                    return Err(attribute_error(
                        start,
                        "derive attribute is only supported on struct/enum/class declarations",
                    ));
                }
            }
            ParsedAttribute::Cfg(predicate) => {
                if !allows_cfg(decl_kind) {
                    return Err(attribute_error(
                        start,
                        format!(
                            "cfg attribute is not supported on `{}` declarations",
                            decl_kind_label(decl_kind)
                        ),
                    ));
                }
                if !cfg_predicate_matches(&predicate) {
                    cfg_enabled = false;
                }
                mask_ranges.push((rel_start, rel_end));
            }
            ParsedAttribute::Deprecated(message) => {
                if !allows_deprecated(decl_kind) {
                    return Err(attribute_error(
                        start,
                        format!(
                            "deprecated attribute is not supported on `{}` declarations",
                            decl_kind_label(decl_kind)
                        ),
                    ));
                }
                record_deprecated(decl_kind, decl_name, message);
                mask_ranges.push((rel_start, rel_end));
            }
            ParsedAttribute::Unsupported { name, span_start } => {
                return Err(attribute_error(
                    span_start,
                    format!("unsupported attribute `{name}`"),
                ));
            }
        }
    }

    if !cfg_enabled {
        return Ok(AttributeAction::Remove);
    }

    Ok(AttributeAction::Keep { mask_ranges })
}

fn record_deprecated(kind: SurfaceDeclKind, name: &str, message: Option<String>) {
    ATTRIBUTE_STATE.with(|state| {
        if let Some(entry) = state.borrow_mut().as_mut() {
            entry.deprecated.push(DeprecatedDecl {
                name: name.to_string(),
                kind: decl_kind_label(kind).to_string(),
                message,
            });
        }
    });
}

fn allows_derive(kind: SurfaceDeclKind) -> bool {
    matches!(
        kind,
        SurfaceDeclKind::Struct | SurfaceDeclKind::Enum | SurfaceDeclKind::Class
    )
}

fn allows_cfg(kind: SurfaceDeclKind) -> bool {
    !matches!(kind, SurfaceDeclKind::Other)
}

fn allows_deprecated(kind: SurfaceDeclKind) -> bool {
    !matches!(kind, SurfaceDeclKind::Impl | SurfaceDeclKind::Other)
}

fn decl_kind_label(kind: SurfaceDeclKind) -> &'static str {
    match kind {
        SurfaceDeclKind::Struct => "struct",
        SurfaceDeclKind::Enum => "enum",
        SurfaceDeclKind::Class => "class",
        SurfaceDeclKind::Trait => "trait",
        SurfaceDeclKind::Impl => "impl",
        SurfaceDeclKind::Function => "fn",
        SurfaceDeclKind::Const => "const",
        SurfaceDeclKind::Other => "declaration",
    }
}

fn cfg_target_os_matches(value: &str) -> bool {
    current_target_os() == value
}

fn supported_target_os(value: &str) -> bool {
    matches!(value, "windows" | "linux" | "macos")
}

fn cfg_target_family_matches(value: &str) -> bool {
    current_target_family() == value
}

fn supported_target_family(value: &str) -> bool {
    matches!(value, "windows" | "unix")
}

fn cfg_feature_enabled(value: &str) -> bool {
    CFG_FEATURES.with(|features| features.borrow().iter().any(|feature| feature == value))
}

fn cfg_predicate_matches(predicate: &CfgPredicate) -> bool {
    match predicate {
        CfgPredicate::TargetOs(value) => cfg_target_os_matches(value),
        CfgPredicate::TargetFamily(value) => cfg_target_family_matches(value),
        CfgPredicate::Feature(value) => cfg_feature_enabled(value),
        CfgPredicate::All(predicates) => predicates.iter().all(cfg_predicate_matches),
        CfgPredicate::Any(predicates) => predicates.iter().any(cfg_predicate_matches),
        CfgPredicate::Not(predicate) => !cfg_predicate_matches(predicate),
    }
}

fn current_target_os() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        "unknown"
    }
}

fn current_target_family() -> &'static str {
    if cfg!(target_os = "windows") {
        "windows"
    } else if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
        "unix"
    } else {
        "unknown"
    }
}

fn skip_attribute_regions(bytes: &[u8], mut start: usize) -> usize {
    loop {
        start = skip_ws_and_comments(bytes, start);
        if bytes.get(start) != Some(&b'#') {
            break;
        }
        start += 1;
        start = skip_ws(bytes, start);
        if bytes.get(start) == Some(&b'[') {
            if let Ok((_, _, after)) = parse_balanced(bytes, start, b'[', b']') {
                start = after;
                continue;
            }
        }
        break;
    }
    start
}

fn collect_leading_attributes(
    source: &str,
    bytes: &[u8],
    start: usize,
) -> Result<(Vec<ParsedAttributeSpan>, usize)> {
    let mut attrs = Vec::new();
    let mut cursor = start;

    loop {
        cursor = skip_ws_and_comments(bytes, cursor);
        if bytes.get(cursor) != Some(&b'#') {
            break;
        }
        let attr_start = cursor;
        let (attr, attr_end) = parse_surface_attribute(source, bytes, cursor)?;
        attrs.push((attr_start, attr_end, attr));
        cursor = attr_end;
    }

    Ok((attrs, cursor))
}

fn parse_surface_attribute(
    source: &str,
    bytes: &[u8],
    start: usize,
) -> Result<(ParsedAttribute, usize)> {
    if bytes.get(start) != Some(&b'#') {
        return Err(parse_error("expected `#` at attribute start"));
    }
    let mut cursor = start + 1;
    cursor = skip_ws(bytes, cursor);
    if bytes.get(cursor) != Some(&b'[') {
        return Err(parse_error("attribute requires `[` after `#`"));
    }
    cursor += 1;
    cursor = skip_ws(bytes, cursor);

    let (name_start, name_end) = parse_ident_range(bytes, cursor)
        .ok_or_else(|| parse_error("attribute name expected after `#`"))?;
    let name = &source[name_start..name_end];
    cursor = name_end;

    let attr = match name {
        "derive" => {
            cursor = skip_ws(bytes, cursor);
            if bytes.get(cursor) != Some(&b'(') {
                return Err(parse_error("`#[derive(...)]` requires parentheses"));
            }
            let (_, _, after) = parse_balanced(bytes, cursor, b'(', b')')?;
            cursor = after;
            ParsedAttribute::Derive
        }
        "cfg" => {
            cursor = skip_ws(bytes, cursor);
            if bytes.get(cursor) != Some(&b'(') {
                return Err(parse_error("`#[cfg(...)]` requires parentheses"));
            }
            let (inner_start, inner_end, after) = parse_balanced(bytes, cursor, b'(', b')')?;
            cursor = after;
            let predicate = parse_cfg_predicate(&source[inner_start..inner_end])
                .map_err(|err| attribute_parse_error(name_start, err))?;
            ParsedAttribute::Cfg(predicate)
        }
        "deprecated" => {
            let message = if bytes.get(cursor) == Some(&b'(') {
                let (inner_start, inner_end, after) = parse_balanced(bytes, cursor, b'(', b')')?;
                cursor = after;
                Some(parse_deprecated_message(&source[inner_start..inner_end])?)
            } else {
                None
            };
            ParsedAttribute::Deprecated(message)
        }
        other => ParsedAttribute::Unsupported {
            name: other.to_string(),
            span_start: name_start,
        },
    };

    cursor = skip_ws(bytes, cursor);
    if bytes.get(cursor) != Some(&b']') {
        return Err(parse_error("attribute is missing closing `]`"));
    }
    Ok((attr, cursor + 1))
}

fn parse_cfg_predicate(inner: &str) -> Result<CfgPredicate> {
    let mut parser = CfgPredicateParser::new(inner);
    let predicate = parser.parse_predicate()?;
    parser.skip_ws();
    if !parser.is_eof() {
        return Err(parse_error("malformed cfg predicate"));
    }
    Ok(predicate)
}

struct CfgPredicateParser<'a> {
    source: &'a str,
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> CfgPredicateParser<'a> {
    fn new(source: &'a str) -> Self {
        Self {
            source,
            bytes: source.as_bytes(),
            cursor: 0,
        }
    }

    fn is_eof(&self) -> bool {
        self.cursor >= self.bytes.len()
    }

    fn skip_ws(&mut self) {
        self.cursor = skip_ws(self.bytes, self.cursor);
    }

    fn parse_predicate(&mut self) -> Result<CfgPredicate> {
        self.skip_ws();
        let (ident_start, ident_end) = parse_ident_range(self.bytes, self.cursor)
            .ok_or_else(|| parse_error("expected cfg predicate"))?;
        let ident = &self.source[ident_start..ident_end];
        self.cursor = ident_end;
        self.skip_ws();

        match ident {
            "target_os" => {
                let value = self.parse_key_value_string("target_os")?;
                if !supported_target_os(&value) {
                    return Err(parse_error(format!(
                        "unsupported cfg target_os `{value}`; supported: windows, linux, macos"
                    )));
                }
                Ok(CfgPredicate::TargetOs(value))
            }
            "target_family" => {
                let value = self.parse_key_value_string("target_family")?;
                if !supported_target_family(&value) {
                    return Err(parse_error(format!(
                        "unsupported cfg target_family `{value}`; supported: windows, unix"
                    )));
                }
                Ok(CfgPredicate::TargetFamily(value))
            }
            "feature" => {
                let value = self.parse_key_value_string("feature")?;
                if value.is_empty() {
                    return Err(parse_error("cfg feature name must not be empty"));
                }
                Ok(CfgPredicate::Feature(value))
            }
            "all" => {
                let predicates = self.parse_predicate_list("all")?;
                Ok(CfgPredicate::All(predicates))
            }
            "any" => {
                let predicates = self.parse_predicate_list("any")?;
                Ok(CfgPredicate::Any(predicates))
            }
            "not" => {
                let predicates = self.parse_predicate_list("not")?;
                if predicates.len() != 1 {
                    return Err(parse_error(
                        "cfg not(...) requires exactly one predicate",
                    ));
                }
                Ok(CfgPredicate::Not(Box::new(
                    predicates.into_iter().next().unwrap(),
                )))
            }
            other => Err(parse_error(format!(
                "unsupported cfg predicate `{other}`; supported: target_os, target_family, feature, all, any, not"
            ))),
        }
    }

    fn parse_key_value_string(&mut self, key: &str) -> Result<String> {
        self.skip_ws();
        if self.bytes.get(self.cursor) != Some(&b'=') {
            return Err(parse_error(format!(
                "cfg({key}) requires `{key} = \"...\"` syntax"
            )));
        }
        self.cursor += 1;
        self.skip_ws();
        self.parse_string_literal()
    }

    fn parse_predicate_list(&mut self, name: &str) -> Result<Vec<CfgPredicate>> {
        self.skip_ws();
        if self.bytes.get(self.cursor) != Some(&b'(') {
            return Err(parse_error(format!("cfg {name}(...) requires parentheses")));
        }
        self.cursor += 1;

        let mut predicates = Vec::new();
        loop {
            self.skip_ws();
            if self.bytes.get(self.cursor) == Some(&b')') {
                self.cursor += 1;
                break;
            }
            predicates.push(self.parse_predicate()?);
            self.skip_ws();
            match self.bytes.get(self.cursor) {
                Some(b',') => {
                    self.cursor += 1;
                }
                Some(b')') => {
                    self.cursor += 1;
                    break;
                }
                _ => {
                    return Err(parse_error(format!(
                        "cfg {name}(...) expects predicates separated by commas"
                    )));
                }
            }
        }

        if predicates.is_empty() {
            return Err(parse_error(format!(
                "cfg {name}(...) requires at least one predicate"
            )));
        }
        Ok(predicates)
    }

    fn parse_string_literal(&mut self) -> Result<String> {
        if self.bytes.get(self.cursor) != Some(&b'"') {
            return Err(parse_error("expected string literal"));
        }
        let start = self.cursor;
        self.cursor += 1;
        while self.cursor < self.bytes.len() {
            match self.bytes[self.cursor] {
                b'\\' => {
                    self.cursor = self.cursor.saturating_add(2);
                }
                b'"' => {
                    let end = self.cursor;
                    self.cursor += 1;
                    return Ok(self.source[start + 1..end].to_string());
                }
                _ => self.cursor += 1,
            }
        }
        Err(parse_error("unclosed string literal in cfg predicate"))
    }
}

fn parse_deprecated_message(inner: &str) -> Result<String> {
    parse_string_literal_token(inner.trim())
}

fn parse_string_literal_token(value: &str) -> Result<String> {
    let bytes = value.as_bytes();
    if bytes.first() != Some(&b'"') || bytes.last() != Some(&b'"') || bytes.len() < 2 {
        return Err(parse_error("expected string literal"));
    }
    Ok(value[1..value.len() - 1].to_string())
}

fn locate_top_level_decl(
    source: &str,
    bytes: &[u8],
    start: usize,
) -> Result<Option<(SurfaceDeclKind, String, usize)>> {
    let mut cursor = start;

    if starts_with_keyword(bytes, cursor, b"extern") {
        let decl_end = find_declaration_end(bytes, cursor)?;
        return Ok(Some((
            SurfaceDeclKind::Other,
            "extern".to_string(),
            decl_end,
        )));
    }

    if starts_with_keyword(bytes, cursor, b"def") {
        cursor = skip_ident(bytes, cursor);
        cursor = skip_ws_and_comments(bytes, cursor);
        let (name_start, name_end) = parse_ident_range(bytes, cursor)
            .ok_or_else(|| parse_error("expected function name after `def`"))?;
        let decl_end = find_declaration_end(bytes, cursor)?;
        return Ok(Some((
            SurfaceDeclKind::Function,
            source[name_start..name_end].to_string(),
            decl_end,
        )));
    }

    let Some((kw_start, kw_end)) = parse_ident_range(bytes, cursor) else {
        return Ok(None);
    };
    let keyword = &source[kw_start..kw_end];
    let kind = match keyword {
        "struct" => SurfaceDeclKind::Struct,
        "enum" => SurfaceDeclKind::Enum,
        "class" => SurfaceDeclKind::Class,
        "trait" => SurfaceDeclKind::Trait,
        "impl" => SurfaceDeclKind::Impl,
        "const" => SurfaceDeclKind::Const,
        "static" | "type" | "import" => SurfaceDeclKind::Other,
        _ => return Ok(None),
    };

    cursor = kw_end;
    cursor = skip_ws_and_comments(bytes, cursor);

    let name = if matches!(kind, SurfaceDeclKind::Impl) {
        "impl".to_string()
    } else if matches!(kind, SurfaceDeclKind::Const) {
        let (name_start, name_end) = parse_ident_range(bytes, cursor)
            .ok_or_else(|| parse_error(format!("expected name after `{keyword}`")))?;
        source[name_start..name_end].to_string()
    } else {
        let (name_start, name_end) = parse_ident_range(bytes, cursor)
            .ok_or_else(|| parse_error(format!("expected name after `{keyword}`")))?;
        source[name_start..name_end].to_string()
    };

    let decl_end = find_declaration_end(bytes, kw_start)?;
    Ok(Some((kind, name, decl_end)))
}

fn skip_visibility(bytes: &[u8], mut cursor: usize) -> usize {
    loop {
        cursor = skip_ws_and_comments(bytes, cursor);
        let Some((start, end)) = parse_ident_range(bytes, cursor) else {
            break;
        };
        let keyword = std::str::from_utf8(&bytes[start..end]).unwrap_or("");
        if keyword == "pub" || keyword == "priv" {
            cursor = end;
            if bytes.get(cursor) == Some(&b'(') {
                if let Ok((_, _, after)) = parse_balanced(bytes, cursor, b'(', b')') {
                    cursor = after;
                }
            }
            continue;
        }
        break;
    }
    cursor
}

fn skip_keyword(bytes: &[u8], cursor: usize, keyword: &[u8]) -> usize {
    let pos = skip_ws_and_comments(bytes, cursor);
    if starts_with_keyword(bytes, pos, keyword) {
        pos + keyword.len()
    } else {
        cursor
    }
}

fn starts_with_keyword(bytes: &[u8], cursor: usize, keyword: &[u8]) -> bool {
    let Some(end) = cursor.checked_add(keyword.len()) else {
        return false;
    };
    bytes.get(cursor..end) == Some(keyword)
        && !bytes
            .get(end)
            .map(|next| is_ident_continue(*next))
            .unwrap_or(false)
}

fn skip_ident(bytes: &[u8], cursor: usize) -> usize {
    parse_ident_range(bytes, cursor)
        .map(|(_, end)| end)
        .unwrap_or(cursor)
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
                i = after_paren;
            }
            b';' => return Ok(i + 1),
            _ => i += 1,
        }
    }

    Err(parse_error(
        "failed to locate end of declaration for attributes",
    ))
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

    Err(parse_error("unbalanced delimiters in attribute"))
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

fn attribute_error(start: usize, message: impl Into<String>) -> CompileError {
    CompileError::ParseError(ParseError::UnsupportedAttribute {
        message: message.into(),
        span: miette::SourceSpan::new(start.into(), 1),
    })
}

fn attribute_parse_error(start: usize, err: CompileError) -> CompileError {
    match err {
        CompileError::ParseError(ParseError::InvalidPattern(message)) => {
            attribute_error(start, message)
        }
        other => other,
    }
}

fn parse_error(message: impl Into<String>) -> CompileError {
    CompileError::ParseError(ParseError::InvalidPattern(message.into()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cfg_filter_leaves_parseable_program() {
        let other_os = if cfg!(target_os = "windows") {
            "linux"
        } else {
            "windows"
        };
        let source = format!(
            r#"
#[cfg(target_os = "{other_os}")]
struct Hidden {{}}

struct Visible {{ x: i64 }}

def main() -> i64 {{
    let v = Visible {{ x: 1 }};
    v.x
}}
"#
        );
        let processed = process_surface_attributes(&source).expect("cfg filter should succeed");
        assert!(!processed.contains("Hidden"));
        assert!(
            processed.contains("struct Visible"),
            "processed source lost Visible decl: {processed}"
        );
        let mut parser = crate::parser::Parser::new(processed.as_ref());
        if let Err(err) = parser.parse_program() {
            panic!("cfg-filtered source should parse: {err}\nprocessed:\n{processed}");
        }
    }

    #[test]
    fn cfg_false_removes_declaration() {
        let source = r#"
#[cfg(target_os = "linux")]
struct LinuxOnly {}

struct Always {}
"#;
        let processed = process_surface_attributes(source).expect("cfg filter should succeed");
        assert!(!processed.contains("LinuxOnly"));
        assert!(processed.contains("Always"));
    }

    #[test]
    fn unsupported_attribute_reports_name() {
        let source = r#"
#[inline]
struct Bad {}
"#;
        let err = process_surface_attributes(source).expect_err("inline should fail");
        assert!(err.to_string().contains("unsupported attribute `inline`"));
    }

    #[test]
    fn non_ascii_before_hash_does_not_panic_on_byte_cursor() {
        let result = process_surface_attributes("¡#");
        assert!(result.is_err());
    }

    #[test]
    fn deprecated_on_function_records_metadata() {
        let source = r#"
#[deprecated("legacy")]
def old_main() -> i64 { 1 }
"#;
        process_surface_attributes(source).expect("function deprecated should strip");
        let deprecated = take_deprecated_decls();
        assert_eq!(deprecated.len(), 1);
        assert_eq!(deprecated[0].name, "old_main");
    }

    #[test]
    fn deprecated_records_metadata() {
        let source = r#"
#[deprecated("use NewType instead")]
struct OldType {}
"#;
        process_surface_attributes(source).expect("deprecated should strip");
        let deprecated = take_deprecated_decls();
        assert_eq!(deprecated.len(), 1);
        assert_eq!(deprecated[0].name, "OldType");
        assert_eq!(
            deprecated[0].message.as_deref(),
            Some("use NewType instead")
        );
    }
}
