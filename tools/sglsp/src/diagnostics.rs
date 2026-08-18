use crate::formatting::full_document_range;
use sengoo_compiler::error::{ParseError, TypeError};
use sengoo_compiler::typeck::TypeckError;
use sengoo_compiler::{collect_compile_warnings, compile_to_ir, CompileError, CompileWarning};
use sengoo_compiler::{Lexer, Token, TokenKind};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_lsp::lsp_types::*;

use super::completion::safe_import_edit;
use super::semantic::{byte_to_char_index, line_char_len};
use super::symbols::find_symbol_occurrences;
use super::text_editing::span_to_range;
use super::workspace_index::{
    collect_import_facts, ImportFact, ImportFactKind, IndexedEnumVariant, WorkspaceIndex,
};

const DIAGNOSTIC_DATA_SCHEMA_VERSION: u32 = 1;
const UNRESOLVED_SYMBOL_CODE: &str = "sglsp-unresolved-symbol";
const UNUSED_IMPORT_CODE: &str = "sglsp-unused-import";

#[derive(Debug, Clone, PartialEq, Eq)]
struct WireHash(String);

impl WireHash {
    fn from_u64(value: u64) -> Self {
        Self(format!("{value:016x}"))
    }

    fn of(value: &str) -> Self {
        Self::from_u64(stable_hash(value))
    }
}

impl Serialize for WireHash {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for WireHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        if value.len() == 16
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            Ok(Self(value))
        } else {
            Err(serde::de::Error::custom(
                "wire hash must be 16 lowercase hexadecimal characters",
            ))
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum UnresolvedSymbolKind {
    Type,
    Function,
    Variable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum SafeCodeActionDataV1 {
    UnresolvedSymbol {
        #[serde(rename = "schemaVersion")]
        schema_version: u32,
        #[serde(rename = "documentUri")]
        document_uri: Url,
        #[serde(rename = "documentRevision")]
        document_revision: i32,
        #[serde(rename = "contentHash")]
        content_hash: WireHash,
        name: String,
        #[serde(rename = "compilerKind")]
        compiler_kind: UnresolvedSymbolKind,
        #[serde(rename = "symbolKind")]
        symbol_kind: UnresolvedSymbolKind,
        range: Range,
    },
    UnusedImport {
        #[serde(rename = "schemaVersion")]
        schema_version: u32,
        #[serde(rename = "documentUri")]
        document_uri: Url,
        #[serde(rename = "documentRevision")]
        document_revision: i32,
        #[serde(rename = "contentHash")]
        content_hash: WireHash,
        #[serde(rename = "factId")]
        fact_id: String,
        #[serde(rename = "canonicalIdentity")]
        canonical_identity: String,
        range: Range,
        #[serde(rename = "sourceHash")]
        source_hash: WireHash,
    },
    MissingEnumArms {
        #[serde(rename = "schemaVersion")]
        schema_version: u32,
        #[serde(rename = "documentUri")]
        document_uri: Url,
        #[serde(rename = "documentRevision")]
        document_revision: i32,
        #[serde(rename = "contentHash")]
        content_hash: WireHash,
        #[serde(rename = "enumUri")]
        enum_uri: Url,
        #[serde(rename = "enumName")]
        enum_name: String,
        missing: Vec<String>,
        range: Range,
    },
}

fn stable_hash(value: &str) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn effective_unresolved_kind(
    content: &str,
    name: &str,
    compiler_kind: UnresolvedSymbolKind,
) -> UnresolvedSymbolKind {
    if compiler_kind != UnresolvedSymbolKind::Variable {
        return compiler_kind;
    }
    let tokens = Lexer::tokenize(content);
    let matches = tokens
        .iter()
        .enumerate()
        .filter(|(_, token)| {
            token.kind == TokenKind::Ident
                && content
                    .get(token.span.lo as usize..token.span.hi as usize)
                    .is_some_and(|source| source == name)
        })
        .map(|(index, _)| index)
        .collect::<Vec<_>>();
    let [index] = matches.as_slice() else {
        return compiler_kind;
    };
    if tokens
        .get(index + 1)
        .is_some_and(|token| token.kind == TokenKind::LParen)
    {
        return UnresolvedSymbolKind::Function;
    }
    if index
        .checked_sub(1)
        .and_then(|index| tokens.get(index))
        .is_some_and(|token| {
            matches!(
                token.kind,
                TokenKind::Colon | TokenKind::Arrow | TokenKind::AsKw
            )
        })
    {
        return UnresolvedSymbolKind::Type;
    }
    compiler_kind
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct UnresolvedEvidence {
    name: String,
    compiler_kind: UnresolvedSymbolKind,
    symbol_kind: UnresolvedSymbolKind,
    range: Range,
}

fn unresolved_evidence_from_error(
    content: &str,
    error: &CompileError,
) -> Option<UnresolvedEvidence> {
    let CompileError::TypeckError(error) = error else {
        return None;
    };
    let (name, compiler_kind) = match error {
        TypeckError::UndefinedType { name } => (name, UnresolvedSymbolKind::Type),
        TypeckError::UndefinedFunction { name } => (name, UnresolvedSymbolKind::Function),
        TypeckError::UndefinedVariable { name } => (name, UnresolvedSymbolKind::Variable),
        _ => return None,
    };
    let occurrences = find_symbol_occurrences(content, name);
    let [range] = occurrences.as_slice() else {
        return None;
    };
    Some(UnresolvedEvidence {
        name: name.clone(),
        compiler_kind,
        symbol_kind: effective_unresolved_kind(content, name, compiler_kind),
        range: *range,
    })
}

fn current_unresolved_evidence(content: &str) -> Option<UnresolvedEvidence> {
    let error = compile_to_ir(content).err()?;
    unresolved_evidence_from_error(content, &error)
}

fn enum_pattern_variants(
    content: &str,
    byte_start: usize,
    byte_end: usize,
    enum_name: &str,
) -> HashSet<String> {
    let Some(source) = content.get(byte_start..byte_end) else {
        return HashSet::new();
    };
    let tokens = Lexer::tokenize(source);
    let Some(match_index) = tokens
        .iter()
        .position(|token| token.kind == TokenKind::MatchKw)
    else {
        return HashSet::new();
    };
    let Some(body_index) = tokens
        .iter()
        .enumerate()
        .skip(match_index + 1)
        .find(|(_, token)| token.kind == TokenKind::LBrace)
        .map(|(index, _)| index)
    else {
        return HashSet::new();
    };

    let token_text = |token: &Token| {
        source
            .get(token.span.lo as usize..token.span.hi as usize)
            .unwrap_or_default()
    };
    let mut variants = HashSet::new();
    let mut brace_depth = 1u32;
    let mut paren_depth = 0u32;
    let mut bracket_depth = 0u32;
    let mut in_pattern = true;
    let mut index = body_index + 1;
    while index < tokens.len() && brace_depth > 0 {
        let token = &tokens[index];
        match token.kind {
            TokenKind::LBrace => brace_depth += 1,
            TokenKind::RBrace => brace_depth = brace_depth.saturating_sub(1),
            TokenKind::LParen => paren_depth += 1,
            TokenKind::RParen => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::LBracket => bracket_depth += 1,
            TokenKind::RBracket => bracket_depth = bracket_depth.saturating_sub(1),
            TokenKind::FatArrow if brace_depth == 1 && paren_depth == 0 && bracket_depth == 0 => {
                in_pattern = false;
            }
            TokenKind::Comma
                if brace_depth == 1 && paren_depth == 0 && bracket_depth == 0 && !in_pattern =>
            {
                in_pattern = true;
            }
            TokenKind::Ident if in_pattern && index + 2 < tokens.len() => {
                if token_text(token) == enum_name
                    && tokens[index + 1].kind == TokenKind::ColonColon
                    && tokens[index + 2].kind == TokenKind::Ident
                {
                    variants.insert(token_text(&tokens[index + 2]).to_string());
                }
            }
            _ => {}
        }
        index += 1;
    }
    variants
}

#[derive(Debug, Deserialize)]
struct SgcErrorSpanPayload {
    lo: u32,
    hi: u32,
}

#[derive(Debug, Deserialize)]
struct SgcErrorLocationPayload {
    #[serde(default)]
    line: Option<u32>,
    #[serde(default, alias = "col")]
    column: Option<u32>,
    #[serde(default)]
    span: Option<SgcErrorSpanPayload>,
}

#[derive(Debug, Deserialize)]
struct SgcErrorPayload {
    #[allow(dead_code)]
    ok: Option<bool>,
    #[allow(dead_code)]
    kind: Option<String>,
    stage: Option<String>,
    code: Option<String>,
    message: Option<String>,
    #[serde(default)]
    details: Vec<String>,
    #[serde(default)]
    location: Option<SgcErrorLocationPayload>,
}

#[derive(Debug, Deserialize)]
struct SgcWarningPayload {
    kind: Option<String>,
    severity: Option<String>,
    code: Option<String>,
    message: Option<String>,
    replacement: Option<String>,
    removal: Option<String>,
    #[serde(default)]
    location: Option<SgcErrorLocationPayload>,
}

pub(crate) fn build_diagnostics(content: &str, max_problems: usize) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();

    for (line_idx, line) in content.lines().enumerate() {
        if diagnostics.len() >= max_problems {
            break;
        }

        if line.contains("TODO") || line.contains("FIXME") {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: 0,
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: line_char_len(line),
                    },
                },
                severity: Some(DiagnosticSeverity::HINT),
                code: Some(NumberOrString::String("todo-item".to_string())),
                source: Some("sglsp".to_string()),
                message: "TODO/FIXME marker found".to_string(),
                ..Default::default()
            });
        }

        if diagnostics.len() >= max_problems {
            break;
        }

        if let Some(tab_pos) = line.find('\t') {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: byte_to_char_index(line, tab_pos),
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: byte_to_char_index(line, tab_pos + 1),
                    },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String("tab-indentation".to_string())),
                source: Some("sglsp".to_string()),
                message: "Tab indentation detected (prefer spaces)".to_string(),
                ..Default::default()
            });
        }

        if diagnostics.len() >= max_problems {
            break;
        }

        let trimmed = line.trim_end_matches([' ', '\t']);
        if trimmed.len() < line.len() {
            diagnostics.push(Diagnostic {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: line_char_len(trimmed),
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: line_char_len(line),
                    },
                },
                severity: Some(DiagnosticSeverity::WARNING),
                code: Some(NumberOrString::String("trailing-whitespace".to_string())),
                source: Some("sglsp".to_string()),
                message: "Trailing whitespace".to_string(),
                ..Default::default()
            });
        }
    }

    diagnostics
}

fn parse_sgc_payload(raw: &str) -> Option<SgcErrorPayload> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(payload) = serde_json::from_str::<SgcErrorPayload>(trimmed) {
        return Some(payload);
    }

    let start = trimmed.find('{')?;
    let end = trimmed.rfind('}')?;
    if start >= end {
        return None;
    }
    serde_json::from_str::<SgcErrorPayload>(&trimmed[start..=end]).ok()
}

fn extract_u32_numbers(text: &str) -> Vec<u32> {
    let mut numbers = Vec::new();
    let mut current = String::new();

    for ch in text.chars() {
        if ch.is_ascii_digit() {
            current.push(ch);
            continue;
        }

        if !current.is_empty() {
            if let Ok(value) = current.parse::<u32>() {
                numbers.push(value);
            }
            current.clear();
        }
    }

    if !current.is_empty() {
        if let Ok(value) = current.parse::<u32>() {
            numbers.push(value);
        }
    }

    numbers
}

fn parse_line_col_hint(text: &str) -> Option<(u32, u32)> {
    let lower = text.to_ascii_lowercase();
    if !lower.contains("line") {
        return None;
    }
    if !(lower.contains("col") || lower.contains("column")) {
        return None;
    }

    let numbers = extract_u32_numbers(&lower);
    if numbers.len() < 2 {
        return None;
    }

    Some((numbers[0], numbers[1]))
}

fn line_col_to_position(content: &str, line_1_based: u32, col_1_based: u32) -> Position {
    let lines = content.split('\n').collect::<Vec<_>>();
    if lines.is_empty() {
        return Position {
            line: 0,
            character: 0,
        };
    }

    let max_line = lines.len().saturating_sub(1) as u32;
    let line = line_1_based.saturating_sub(1).min(max_line);
    let line_text = lines[line as usize];
    let max_col = line_text.encode_utf16().count() as u32;
    let character = col_1_based.saturating_sub(1).min(max_col);

    Position { line, character }
}

fn range_from_line_col(content: &str, line_1_based: u32, col_1_based: u32) -> Range {
    let start = line_col_to_position(content, line_1_based, col_1_based);
    let line_text = content
        .split('\n')
        .nth(start.line as usize)
        .unwrap_or_default();
    let max_col = line_text.encode_utf16().count() as u32;
    let end_character = (start.character + 1).min(max_col);

    Range {
        start,
        end: Position {
            line: start.line,
            character: end_character,
        },
    }
}

fn range_from_byte_span(content: &str, lo: u32, hi: u32) -> Range {
    let max = u32::try_from(content.len()).unwrap_or(u32::MAX);
    let lo = lo.min(max);
    let mut hi = hi.min(max);
    if hi <= lo && lo < max {
        hi = lo + 1;
    }
    span_to_range(content, lo, hi)
}

fn diagnostic_range_from_location(
    content: &str,
    location: &SgcErrorLocationPayload,
) -> Option<Range> {
    if let Some(span) = &location.span {
        return Some(range_from_byte_span(content, span.lo, span.hi));
    }

    if let (Some(line), Some(column)) = (location.line, location.column) {
        return Some(range_from_line_col(content, line, column));
    }

    None
}

fn diagnostic_range_from_payload(content: &str, payload: &SgcErrorPayload) -> Option<Range> {
    if let Some(location) = payload.location.as_ref() {
        if let Some(range) = diagnostic_range_from_location(content, location) {
            return Some(range);
        }
    }

    for hint in payload
        .details
        .iter()
        .chain(payload.message.as_ref().into_iter())
    {
        if let Some((line, col)) = parse_line_col_hint(hint) {
            return Some(range_from_line_col(content, line, col));
        }
    }
    None
}

fn source_span_to_range(content: &str, span: &miette::SourceSpan) -> Option<Range> {
    let lo: usize = span.offset();
    if lo > content.len() {
        return None;
    }

    let mut hi = lo.saturating_add(span.len()).min(content.len());
    if hi == lo && lo < content.len() {
        hi = lo + 1;
    }

    let lo = u32::try_from(lo).ok()?;
    let hi = u32::try_from(hi).ok()?;
    Some(span_to_range(content, lo, hi))
}

fn diagnostic_range_from_parse_error(content: &str, error: &ParseError) -> Option<Range> {
    match error {
        ParseError::UnexpectedToken { span, .. }
        | ParseError::UnclosedBlock(span)
        | ParseError::UnclosedParen(span)
        | ParseError::InvalidStructField { span, .. }
        | ParseError::InvalidStructFieldShorthand { span }
        | ParseError::InvalidPatternAt { span, .. }
        | ParseError::UnsupportedAttribute { span, .. } => source_span_to_range(content, span),
        ParseError::InvalidPattern(_)
        | ParseError::DuplicateParam(_)
        | ParseError::UnexpectedEof => None,
    }
}

fn diagnostic_range_from_type_error(content: &str, error: &TypeError) -> Option<Range> {
    match error {
        TypeError::Mismatch { span, .. } => source_span_to_range(content, span),
        TypeError::UndefinedVar { _span, .. } => source_span_to_range(content, _span),
        TypeError::UndefinedType(_)
        | TypeError::UndefinedMethod(_)
        | TypeError::ArgCountMismatch { .. }
        | TypeError::TraitNotImplemented { .. } => None,
    }
}

fn diagnostic_range_from_compile_error(content: &str, error: &CompileError) -> Option<Range> {
    match error {
        CompileError::ParseError(error) => diagnostic_range_from_parse_error(content, error),
        CompileError::TypeError(error) => diagnostic_range_from_type_error(content, error),
        CompileError::TypeckError(error) => error
            .span()
            .map(|(lo, hi)| range_from_byte_span(content, lo, hi)),
        _ => None,
    }
}

fn bracketed_diagnostic_code(message: &str) -> Option<String> {
    let start = message.find('[')? + 1;
    let rest = &message[start..];
    let end = rest.find(']')?;
    let code = &rest[..end];
    (!code.is_empty()).then(|| code.to_string())
}

fn async_user_future_diagnostic_code(message: &str) -> Option<String> {
    if message.contains("Poll<T> must contain `is_ready: bool` followed by `value: T`")
        || message.contains("Future<T>::poll must return Poll<T>")
        || message.contains("Future<T>::poll must use `&mut self` receiver")
    {
        Some("async::user_future_contract".to_string())
    } else {
        None
    }
}

fn diagnostic_code_from_compile_error(error: &CompileError) -> Option<String> {
    match error {
        CompileError::ParseError(ParseError::UnsupportedAttribute { .. }) => {
            Some("attributes::unsupported_attribute".to_string())
        }
        CompileError::TypeckError(error) => error
            .stable_code()
            .map(str::to_string)
            .or_else(|| async_user_future_diagnostic_code(&error.to_string()))
            .or_else(|| bracketed_diagnostic_code(&error.to_string())),
        CompileError::MirLower(message)
        | CompileError::HirLower(message)
        | CompileError::Codegen(message) => async_user_future_diagnostic_code(message)
            .or_else(|| bracketed_diagnostic_code(message)),
        CompileError::AsyncUnsupportedType { .. } => {
            Some("async::unsupported_frame_type".to_string())
        }
        _ => bracketed_diagnostic_code(&error.to_string()),
    }
}

fn fallback_diagnostic_range_from_compiler(content: &str) -> Option<Range> {
    compile_to_ir(content)
        .err()
        .and_then(|error| diagnostic_range_from_compile_error(content, &error))
}

fn compile_error_stage(error: &CompileError) -> &'static str {
    match error {
        CompileError::LexError(_) => "lex",
        CompileError::ParseError(_) => "parse",
        CompileError::TypeError(_) | CompileError::TypeckError(_) => "typecheck",
        CompileError::IoError(_) => "io",
        CompileError::HirLower(_) => "hir_lower",
        CompileError::MirLower(_) => "mir_lower",
        CompileError::Codegen(_) => "codegen",
        CompileError::AsyncUnsupportedType { .. } => "mir_lower",
    }
}

fn structured_error_data(
    index: &WorkspaceIndex,
    uri: &Url,
    revision: i32,
    content: &str,
    error: &CompileError,
) -> Option<(String, Range, serde_json::Value)> {
    let CompileError::TypeckError(error) = error else {
        return None;
    };
    match error {
        TypeckError::UndefinedType { name }
        | TypeckError::UndefinedVariable { name }
        | TypeckError::UndefinedFunction { name } => {
            let compiler_kind = match error {
                TypeckError::UndefinedType { .. } => UnresolvedSymbolKind::Type,
                TypeckError::UndefinedFunction { .. } => UnresolvedSymbolKind::Function,
                TypeckError::UndefinedVariable { .. } => UnresolvedSymbolKind::Variable,
                _ => unreachable!(),
            };
            let symbol_kind = effective_unresolved_kind(content, name, compiler_kind);
            let occurrences = find_symbol_occurrences(content, name);
            let [range] = occurrences.as_slice() else {
                return None;
            };
            let data = SafeCodeActionDataV1::UnresolvedSymbol {
                schema_version: DIAGNOSTIC_DATA_SCHEMA_VERSION,
                document_uri: uri.clone(),
                document_revision: revision,
                content_hash: WireHash::of(content),
                name: name.clone(),
                compiler_kind,
                symbol_kind,
                range: *range,
            };
            Some((
                UNRESOLVED_SYMBOL_CODE.to_string(),
                *range,
                serde_json::to_value(data).ok()?,
            ))
        }
        TypeckError::NonExhaustiveMatch {
            missing,
            span_lo,
            span_hi,
        } => {
            let lo = *span_lo as usize;
            let hi = (*span_hi as usize).min(content.len());
            let missing_set = missing.iter().cloned().collect::<HashSet<_>>();
            let candidates = index
                .enum_candidates()
                .into_iter()
                .filter(|(_, item)| {
                    let present = enum_pattern_variants(content, lo, hi, &item.name);
                    let expected = item
                        .variants
                        .iter()
                        .filter(|variant| !present.contains(variant.name()))
                        .map(|variant| variant.name().to_string())
                        .collect::<HashSet<_>>();
                    expected == missing_set
                })
                .collect::<Vec<_>>();
            let [(enum_uri, item)] = candidates.as_slice() else {
                return None;
            };
            let range = range_from_byte_span(content, *span_lo, *span_hi);
            let data = SafeCodeActionDataV1::MissingEnumArms {
                schema_version: DIAGNOSTIC_DATA_SCHEMA_VERSION,
                document_uri: uri.clone(),
                document_revision: revision,
                content_hash: WireHash::of(content),
                enum_uri: enum_uri.clone(),
                enum_name: item.name.clone(),
                missing: missing.clone(),
                range,
            };
            Some((
                "non-exhaustive-match".to_string(),
                range,
                serde_json::to_value(data).ok()?,
            ))
        }
        _ => None,
    }
}

fn embedded_compiler_diagnostics_with_context(
    content: &str,
    context: Option<(&WorkspaceIndex, &Url, i32)>,
) -> Vec<Diagnostic> {
    match compile_to_ir(content) {
        Ok(_) => collect_compile_warnings(content)
            .unwrap_or_default()
            .into_iter()
            .map(|warning| diagnostic_from_compile_warning(content, warning))
            .collect(),
        Err(error) => {
            let structured = context.and_then(|(index, uri, revision)| {
                structured_error_data(index, uri, revision, content, &error)
            });
            let range = structured
                .as_ref()
                .map(|(_, range, _)| *range)
                .or_else(|| diagnostic_range_from_compile_error(content, &error))
                .unwrap_or_else(|| full_document_range(content));

            vec![Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String(
                    structured
                        .as_ref()
                        .map(|(code, _, _)| code.clone())
                        .or_else(|| diagnostic_code_from_compile_error(&error))
                        .unwrap_or_else(|| compile_error_stage(&error).to_string()),
                )),
                source: Some("sengoo-compiler".to_string()),
                message: error.to_string(),
                data: structured.map(|(_, _, data)| data),
                ..Default::default()
            }]
        }
    }
}

fn embedded_compiler_diagnostics(content: &str) -> Vec<Diagnostic> {
    embedded_compiler_diagnostics_with_context(content, None)
}

fn warning_subject_range(content: &str, message: &str) -> Option<Range> {
    let name = message.split('`').nth(1)?;
    let lo = content.find(name)? as u32;
    Some(range_from_byte_span(content, lo, lo + name.len() as u32))
}

fn diagnostic_from_compile_warning(content: &str, warning: CompileWarning) -> Diagnostic {
    let message = warning.to_string();
    let data = deprecation_data(warning.replacement(), warning.removal());
    let range = warning
        .span()
        .filter(|(lo, hi)| hi > lo)
        .map(|(lo, hi)| range_from_byte_span(content, lo, hi))
        .or_else(|| warning_subject_range(content, &message))
        .unwrap_or_else(|| full_document_range(content));
    Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::WARNING),
        code: Some(NumberOrString::String(warning.code().to_string())),
        source: Some("sengoo-compiler".to_string()),
        message,
        data,
        ..Default::default()
    }
}

fn deprecation_data(replacement: Option<&str>, removal: Option<&str>) -> Option<serde_json::Value> {
    if replacement.is_none() && removal.is_none() {
        return None;
    }
    Some(serde_json::json!({
        "replacement": replacement,
        "removal": removal,
    }))
}

fn temporary_source_path(uri: &Url) -> PathBuf {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    if let Ok(file_path) = uri.to_file_path() {
        if let Some(parent) = file_path.parent() {
            return parent.join(format!(".sglsp_tmp_{}.sg", now));
        }
    }

    std::env::temp_dir().join(format!("sglsp_tmp_{}.sg", now))
}

pub(crate) fn compiler_diagnostics_from_sgc_json(uri: &Url, content: &str) -> Vec<Diagnostic> {
    compiler_diagnostics_from_sgc_tool("sgc", uri, content)
}

fn unused_import_diagnostics(index: &WorkspaceIndex, uri: &Url) -> Vec<Diagnostic> {
    let Some(document) = index.document(uri) else {
        return Vec::new();
    };
    let Some(revision) = document.revision else {
        return Vec::new();
    };
    if !document.parse_valid {
        return Vec::new();
    }
    let content = document.content.as_ref();
    let tokens = Lexer::tokenize(content);
    let identifier_is_used = |name: &str| {
        tokens.iter().any(|token| {
            token.kind == TokenKind::Ident
                && content
                    .get(token.span.lo as usize..token.span.hi as usize)
                    .is_some_and(|source| source == name)
                && !document.imports.iter().any(|import| {
                    token.span.lo >= import.byte_start && token.span.hi <= import.byte_end
                })
        })
    };
    document
        .imports
        .iter()
        .filter_map(|import| {
            let label = match &import.kind {
                ImportFactKind::Alias { alias } if !identifier_is_used(alias) => alias.clone(),
                ImportFactKind::Selective { names }
                    if !names.is_empty() && names.iter().all(|name| !identifier_is_used(name)) =>
                {
                    names.join(", ")
                }
                ImportFactKind::Simple
                | ImportFactKind::Wildcard
                | ImportFactKind::Alias { .. }
                | ImportFactKind::Selective { .. } => return None,
            };
            let data = SafeCodeActionDataV1::UnusedImport {
                schema_version: DIAGNOSTIC_DATA_SCHEMA_VERSION,
                document_uri: uri.clone(),
                document_revision: revision,
                content_hash: WireHash::of(content),
                fact_id: import.fact_id.clone(),
                canonical_identity: import.canonical_identity.clone(),
                range: import.range,
                source_hash: WireHash::from_u64(import.source_hash),
            };
            Some(Diagnostic {
                range: import.range,
                severity: Some(DiagnosticSeverity::HINT),
                code: Some(NumberOrString::String(UNUSED_IMPORT_CODE.to_string())),
                source: Some("sglsp".to_string()),
                message: format!("unused import `{label}`"),
                data: serde_json::to_value(data).ok(),
                ..Default::default()
            })
        })
        .collect()
}

pub(crate) fn semantic_diagnostics_for_document(
    index: &WorkspaceIndex,
    uri: &Url,
    content: &str,
) -> Vec<Diagnostic> {
    let revision = index.document(uri).and_then(|document| document.revision);
    let embedded = revision
        .map(|revision| {
            embedded_compiler_diagnostics_with_context(content, Some((index, uri, revision)))
        })
        .unwrap_or_else(|| embedded_compiler_diagnostics(content));
    let mut diagnostics = compiler_diagnostics_from_sgc_json(uri, content);
    if embedded.iter().any(|diagnostic| diagnostic.data.is_some()) {
        diagnostics = embedded;
    }
    diagnostics.extend(unused_import_diagnostics(index, uri));
    diagnostics
}

fn diagnostics_from_failed_sgc_output(content: &str, stderr: &str) -> Vec<Diagnostic> {
    let Some(payload) = parse_sgc_payload(stderr) else {
        let embedded = embedded_compiler_diagnostics(content);
        if !embedded.is_empty() {
            return embedded;
        }

        let summary = stderr
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .unwrap_or_else(|| "compilation failed".to_string());
        return vec![Diagnostic {
            range: full_document_range(content),
            severity: Some(DiagnosticSeverity::ERROR),
            code: None,
            source: Some("sgc".to_string()),
            message: summary,
            ..Default::default()
        }];
    };

    let range = diagnostic_range_from_payload(content, &payload)
        .or_else(|| fallback_diagnostic_range_from_compiler(content))
        .unwrap_or_else(|| full_document_range(content));
    let code = payload
        .code
        .clone()
        .or_else(|| {
            payload
                .message
                .as_deref()
                .and_then(async_user_future_diagnostic_code)
        })
        .or_else(|| payload.stage.clone());
    let mut message = payload
        .message
        .unwrap_or_else(|| "compilation failed".to_string());
    if !payload.details.is_empty() {
        message.push('\n');
        message.push_str(&payload.details.join("\n"));
    }

    vec![Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: code.map(NumberOrString::String),
        source: Some("sgc".to_string()),
        message,
        ..Default::default()
    }]
}

fn diagnostics_from_successful_sgc_output(content: &str, stderr: &str) -> Vec<Diagnostic> {
    stderr
        .lines()
        .filter_map(|line| serde_json::from_str::<SgcWarningPayload>(line.trim()).ok())
        .filter(|payload| {
            payload.kind.as_deref() == Some("compile_warning")
                && payload.severity.as_deref() == Some("warning")
        })
        .map(|payload| {
            let message = payload
                .message
                .unwrap_or_else(|| "compiler warning".to_string());
            let range = payload
                .location
                .as_ref()
                .and_then(|location| diagnostic_range_from_location(content, location))
                .or_else(|| warning_subject_range(content, &message))
                .unwrap_or_else(|| full_document_range(content));
            let data = deprecation_data(payload.replacement.as_deref(), payload.removal.as_deref());
            Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::WARNING),
                code: payload.code.map(NumberOrString::String),
                source: Some("sgc".to_string()),
                message,
                data,
                ..Default::default()
            }
        })
        .collect()
}

fn compiler_diagnostics_from_sgc_tool(tool: &str, uri: &Url, content: &str) -> Vec<Diagnostic> {
    let scratch = temporary_source_path(uri);
    if fs::write(&scratch, content).is_err() {
        return embedded_compiler_diagnostics(content);
    }

    let output = Command::new(tool)
        .arg("--error-format")
        .arg("json")
        .arg("check")
        .arg(&scratch)
        .output();

    let _ = fs::remove_file(&scratch);

    let Ok(output) = output else {
        return embedded_compiler_diagnostics(content);
    };
    if output.status.success() {
        return diagnostics_from_successful_sgc_output(
            content,
            &String::from_utf8_lossy(&output.stderr),
        );
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    diagnostics_from_failed_sgc_output(content, &stderr)
}

fn quick_fix_action(
    uri: Url,
    edit: TextEdit,
    diagnostic: Diagnostic,
    title: &str,
) -> CodeActionOrCommand {
    CodeActionOrCommand::CodeAction(CodeAction {
        title: title.to_string(),
        kind: Some(CodeActionKind::QUICKFIX),
        diagnostics: Some(vec![diagnostic]),
        edit: Some(WorkspaceEdit {
            changes: Some(HashMap::from([(uri, vec![edit])])),
            ..Default::default()
        }),
        is_preferred: Some(true),
        ..Default::default()
    })
}

fn validated_document(
    index: &WorkspaceIndex,
    uri: &Url,
    content: &str,
    data_uri: &Url,
    revision: i32,
    content_hash: &WireHash,
) -> Option<std::sync::Arc<crate::workspace_index::IndexedDocument>> {
    if data_uri != uri || &WireHash::of(content) != content_hash {
        return None;
    }
    let document = index.document(uri)?;
    (document.revision == Some(revision)
        && document.parse_valid
        && document.content.as_ref() == content)
        .then_some(document)
}

fn unresolved_symbol_edit(
    index: &WorkspaceIndex,
    uri: &Url,
    content: &str,
    diagnostic: &Diagnostic,
) -> Option<(TextEdit, String)> {
    let data = serde_json::from_value::<SafeCodeActionDataV1>(diagnostic.data.clone()?).ok()?;
    let SafeCodeActionDataV1::UnresolvedSymbol {
        schema_version,
        document_uri,
        document_revision,
        content_hash,
        name,
        compiler_kind,
        symbol_kind,
        range,
    } = data
    else {
        return None;
    };
    if schema_version != DIAGNOSTIC_DATA_SCHEMA_VERSION || diagnostic.range != range {
        return None;
    }
    let document = validated_document(
        index,
        uri,
        content,
        &document_uri,
        document_revision,
        &content_hash,
    )?;
    if diagnostic.code != Some(NumberOrString::String(UNRESOLVED_SYMBOL_CODE.to_string()))
        || diagnostic.source.as_deref() != Some("sengoo-compiler")
    {
        return None;
    }
    let evidence = current_unresolved_evidence(content)?;
    if evidence.name != name
        || evidence.compiler_kind != compiler_kind
        || evidence.symbol_kind != symbol_kind
        || evidence.range != range
    {
        return None;
    }
    if find_symbol_occurrences(content, &name) != vec![range] {
        return None;
    }
    if document.symbols.iter().any(|symbol| symbol.name == name) {
        return None;
    }
    let candidates = index
        .completion_candidates(uri)
        .into_iter()
        .filter(|candidate| {
            candidate.symbol.name == name
                && candidate.definition_uri != *uri
                && candidate.symbol.container.as_deref() == Some("<document>")
                && match symbol_kind {
                    UnresolvedSymbolKind::Type => matches!(
                        candidate.symbol.kind,
                        CompletionItemKind::STRUCT
                            | CompletionItemKind::ENUM
                            | CompletionItemKind::CLASS
                            | CompletionItemKind::INTERFACE
                            | CompletionItemKind::TYPE_PARAMETER
                    ),
                    UnresolvedSymbolKind::Function => {
                        candidate.symbol.kind == CompletionItemKind::FUNCTION
                    }
                    UnresolvedSymbolKind::Variable => matches!(
                        candidate.symbol.kind,
                        CompletionItemKind::CONSTANT | CompletionItemKind::VARIABLE
                    ),
                }
        })
        .collect::<Vec<_>>();
    let [candidate] = candidates.as_slice() else {
        return None;
    };
    let module = index
        .module_identity(&candidate.definition_uri)?
        .import_path;
    let edit = safe_import_edit(&document, &module, &name)?;
    Some((edit, name))
}

fn unused_import_edit(
    index: &WorkspaceIndex,
    uri: &Url,
    content: &str,
    diagnostic: &Diagnostic,
) -> Option<(TextEdit, String)> {
    let data = serde_json::from_value::<SafeCodeActionDataV1>(diagnostic.data.clone()?).ok()?;
    let SafeCodeActionDataV1::UnusedImport {
        schema_version,
        document_uri,
        document_revision,
        content_hash,
        fact_id,
        canonical_identity,
        range,
        source_hash,
    } = data
    else {
        return None;
    };
    if schema_version != DIAGNOSTIC_DATA_SCHEMA_VERSION || diagnostic.range != range {
        return None;
    }
    let document = validated_document(
        index,
        uri,
        content,
        &document_uri,
        document_revision,
        &content_hash,
    )?;
    let indexed = document
        .imports
        .iter()
        .find(|import| import.fact_id == fact_id)?;
    let reparsed = collect_import_facts(content);
    let parsed = reparsed.iter().find(|import| import.fact_id == fact_id)?;
    if indexed != parsed
        || indexed.range != range
        || indexed.canonical_identity != canonical_identity
        || WireHash::from_u64(indexed.source_hash) != source_hash
    {
        return None;
    }
    let label = match &indexed.kind {
        ImportFactKind::Alias { alias } => alias.clone(),
        ImportFactKind::Selective { names } => names.join(", "),
        ImportFactKind::Simple | ImportFactKind::Wildcard => return None,
    };
    let edit = import_deletion_edit(content, indexed)?;
    Some((edit, label))
}

fn import_deletion_edit(content: &str, import: &ImportFact) -> Option<TextEdit> {
    let mut start = import.byte_start as usize;
    let mut end = import.byte_end as usize;
    let line_start = content[..start].rfind('\n').map_or(0, |offset| offset + 1);
    let line_end = content[end..]
        .find('\n')
        .map_or(content.len(), |offset| end + offset);
    let prefix = &content[line_start..start];
    let suffix = &content[end..line_end];

    if prefix.trim().is_empty() && suffix.trim().is_empty() {
        start = line_start;
        end = if line_end < content.len() {
            line_end + 1
        } else {
            line_end
        };
    } else if prefix.chars().last().is_some_and(char::is_whitespace) && !prefix.trim().is_empty() {
        while start > line_start
            && content[..start]
                .chars()
                .next_back()
                .is_some_and(|ch| matches!(ch, ' ' | '\t'))
        {
            start -= 1;
        }
    } else if suffix.chars().next().is_some_and(char::is_whitespace) && !suffix.trim().is_empty() {
        while end < line_end
            && content[end..]
                .chars()
                .next()
                .is_some_and(|ch| matches!(ch, ' ' | '\t'))
        {
            end += 1;
        }
    }

    Some(TextEdit {
        range: span_to_range(content, start as u32, end as u32),
        new_text: String::new(),
    })
}

fn position_to_byte(content: &str, position: Position) -> Option<usize> {
    let mut base = 0usize;
    for (line_index, segment) in content.split_inclusive('\n').enumerate() {
        if line_index == position.line as usize {
            let line = segment.strip_suffix('\n').unwrap_or(segment);
            let line = line.strip_suffix('\r').unwrap_or(line);
            let mut utf16 = 0u32;
            for (byte, ch) in line.char_indices() {
                if utf16 == position.character {
                    return Some(base + byte);
                }
                utf16 += ch.len_utf16() as u32;
                if utf16 > position.character {
                    return None;
                }
            }
            return (utf16 == position.character).then_some(base + line.len());
        }
        base += segment.len();
    }
    None
}

fn enum_arm_pattern(enum_name: &str, variant: &IndexedEnumVariant) -> String {
    match variant {
        IndexedEnumVariant::Unit { name } => format!("{enum_name}::{name}"),
        IndexedEnumVariant::Tuple { name, arity } => format!(
            "{enum_name}::{name}({})",
            std::iter::repeat_n("_", *arity)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        IndexedEnumVariant::Struct { name, fields } => format!(
            "{enum_name}::{name} {{ {} }}",
            fields
                .iter()
                .map(|field| format!("{field}: _"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    }
}

fn missing_enum_arms_edit(
    index: &WorkspaceIndex,
    uri: &Url,
    content: &str,
    diagnostic: &Diagnostic,
) -> Option<TextEdit> {
    let data = serde_json::from_value::<SafeCodeActionDataV1>(diagnostic.data.clone()?).ok()?;
    let SafeCodeActionDataV1::MissingEnumArms {
        schema_version,
        document_uri,
        document_revision,
        content_hash,
        enum_uri,
        enum_name,
        missing,
        range,
    } = data
    else {
        return None;
    };
    if schema_version != DIAGNOSTIC_DATA_SCHEMA_VERSION || diagnostic.range != range {
        return None;
    }
    validated_document(
        index,
        uri,
        content,
        &document_uri,
        document_revision,
        &content_hash,
    )?;
    let item = index
        .enum_candidates()
        .into_iter()
        .find(|(candidate_uri, item)| *candidate_uri == enum_uri && item.name == enum_name)?
        .1;
    let start = position_to_byte(content, range.start)?;
    let end = position_to_byte(content, range.end)?;
    let present = enum_pattern_variants(content, start, end, &enum_name);
    let expected = item
        .variants
        .iter()
        .filter(|variant| !present.contains(variant.name()))
        .map(|variant| variant.name().to_string())
        .collect::<HashSet<_>>();
    if expected != missing.iter().cloned().collect::<HashSet<_>>() {
        return None;
    }
    let close = content.get(start..end)?.rfind('}')? + start;
    let insert_range = span_to_range(content, close as u32, close as u32);
    let base_indent = content
        .lines()
        .nth(range.start.line as usize)?
        .chars()
        .take_while(|ch| ch.is_whitespace())
        .collect::<String>();
    let arm_indent = format!("{base_indent}    ");
    let newline = if content.contains("\r\n") {
        "\r\n"
    } else {
        "\n"
    };
    let mut variants = item
        .variants
        .iter()
        .filter(|variant| expected.contains(variant.name()))
        .collect::<Vec<_>>();
    variants.sort_by_key(|variant| {
        missing
            .iter()
            .position(|name| name == variant.name())
            .unwrap_or(usize::MAX)
    });
    let text = variants
        .into_iter()
        .map(|variant| {
            format!(
                "{arm_indent}{} => todo(),",
                enum_arm_pattern(&enum_name, variant)
            )
        })
        .collect::<Vec<_>>()
        .join(newline);
    Some(TextEdit {
        range: insert_range,
        new_text: format!("{text}{newline}{base_indent}"),
    })
}

pub(crate) fn quick_fix_actions(
    index: &WorkspaceIndex,
    uri: Url,
    content: &str,
    diagnostics: Vec<Diagnostic>,
) -> Vec<CodeActionOrCommand> {
    let mut actions = Vec::new();

    for diagnostic in diagnostics {
        let Some(code) = diagnostic.code.as_ref() else {
            continue;
        };

        let Some(code) = (match code {
            NumberOrString::String(s) => Some(s.as_str()),
            NumberOrString::Number(_) => None,
        }) else {
            continue;
        };

        match code {
            "todo-item" => {
                let line_idx = diagnostic.range.start.line as usize;
                if let Some(line) = content.lines().nth(line_idx) {
                    let fixed = line.replace("TODO", "").replace("FIXME", "");
                    if fixed != line {
                        actions.push(quick_fix_action(
                            uri.clone(),
                            TextEdit {
                                range: Range {
                                    start: Position {
                                        line: diagnostic.range.start.line,
                                        character: 0,
                                    },
                                    end: Position {
                                        line: diagnostic.range.start.line,
                                        character: line_char_len(line),
                                    },
                                },
                                new_text: fixed,
                            },
                            diagnostic,
                            "Remove TODO/FIXME marker",
                        ));
                    }
                }
            }
            "tab-indentation" => {
                let line_idx = diagnostic.range.start.line as usize;
                if let Some(line) = content.lines().nth(line_idx) {
                    let fixed = line.replace('\t', "    ");
                    if fixed != line {
                        actions.push(quick_fix_action(
                            uri.clone(),
                            TextEdit {
                                range: Range {
                                    start: Position {
                                        line: diagnostic.range.start.line,
                                        character: 0,
                                    },
                                    end: Position {
                                        line: diagnostic.range.start.line,
                                        character: line_char_len(line),
                                    },
                                },
                                new_text: fixed,
                            },
                            diagnostic,
                            "Convert tabs to spaces",
                        ));
                    }
                }
            }
            "trailing-whitespace" => {
                actions.push(quick_fix_action(
                    uri.clone(),
                    TextEdit {
                        range: diagnostic.range,
                        new_text: String::new(),
                    },
                    diagnostic,
                    "Remove trailing whitespace",
                ));
            }
            UNRESOLVED_SYMBOL_CODE => {
                if let Some((edit, name)) =
                    unresolved_symbol_edit(index, &uri, content, &diagnostic)
                {
                    actions.push(quick_fix_action(
                        uri.clone(),
                        edit,
                        diagnostic,
                        &format!("Import `{name}`"),
                    ));
                }
            }
            UNUSED_IMPORT_CODE => {
                if let Some((edit, alias)) = unused_import_edit(index, &uri, content, &diagnostic) {
                    actions.push(quick_fix_action(
                        uri.clone(),
                        edit,
                        diagnostic,
                        &format!("Remove unused import `{alias}`"),
                    ));
                }
            }
            "non-exhaustive-match" => {
                if let Some(edit) = missing_enum_arms_edit(index, &uri, content, &diagnostic) {
                    actions.push(quick_fix_action(
                        uri.clone(),
                        edit,
                        diagnostic,
                        "Add missing enum match arms",
                    ));
                }
            }
            _ => {}
        }
    }

    actions
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace_index::IndexCancellation;
    use sengoo_compiler::error::ParseError;
    use std::time::{SystemTime, UNIX_EPOCH};
    use tower_lsp::lsp_types::{CodeActionOrCommand, NumberOrString, Url};

    fn code_action_workspace(
        main_source: &str,
        other_sources: &[(&str, &str)],
    ) -> (WorkspaceIndex, Url) {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("sglsp-code-actions-{id}"));
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("Sengoo.toml"), "[package]\nname = \"demo\"\n").unwrap();
        fs::write(root.join("src/main.sg"), main_source).unwrap();
        for (name, source) in other_sources {
            fs::write(root.join("src").join(name), source).unwrap();
        }
        let uri = Url::from_file_path(root.join("src/main.sg")).unwrap();
        let index = WorkspaceIndex::build(&[root], IndexCancellation::default()).unwrap();
        assert!(index.open(
            uri.clone(),
            7,
            main_source.into(),
            &IndexCancellation::default()
        ));
        (index, uri)
    }

    #[test]
    fn unresolved_symbol_diagnostic_offers_one_revision_safe_import() {
        let source = "def main() -> i64 { helper() }\n";
        let (index, uri) =
            code_action_workspace(source, &[("helpers.sg", "def helper() -> i64 { 1 }\n")]);
        let diagnostics = semantic_diagnostics_for_document(&index, &uri, source);
        let diagnostic = diagnostics
            .into_iter()
            .find(|diagnostic| {
                diagnostic.code == Some(NumberOrString::String(UNRESOLVED_SYMBOL_CODE.into()))
            })
            .expect("structured unresolved diagnostic");
        let data = serde_json::from_value::<SafeCodeActionDataV1>(
            diagnostic.data.clone().expect("structured data"),
        )
        .unwrap();
        assert!(matches!(
            data,
            SafeCodeActionDataV1::UnresolvedSymbol {
                compiler_kind: UnresolvedSymbolKind::Variable,
                symbol_kind: UnresolvedSymbolKind::Function,
                ..
            }
        ));

        let actions = quick_fix_actions(&index, uri, source, vec![diagnostic]);
        assert_eq!(actions.len(), 1);
        let action = match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => action,
            _ => panic!("expected code action"),
        };
        let edits = action
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .values()
            .next()
            .unwrap();
        assert_eq!(edits[0].new_text, "import demo::helpers;\n");
    }

    #[test]
    fn unresolved_symbol_action_is_suppressed_after_revision_changes() {
        let source = "def main() -> i64 { helper() }\n";
        let (index, uri) =
            code_action_workspace(source, &[("helpers.sg", "def helper() -> i64 { 1 }\n")]);
        let diagnostic = semantic_diagnostics_for_document(&index, &uri, source)
            .into_iter()
            .find(|diagnostic| diagnostic.data.is_some())
            .unwrap();
        assert!(index.open(uri.clone(), 8, source.into(), &IndexCancellation::default()));

        assert!(quick_fix_actions(&index, uri, source, vec![diagnostic]).is_empty());
    }

    #[test]
    fn unresolved_symbol_action_is_suppressed_for_ambiguous_origins() {
        let source = "def main() -> i64 { helper() }\n";
        let (index, uri) = code_action_workspace(
            source,
            &[
                ("a.sg", "def helper() -> i64 { 1 }\n"),
                ("b.sg", "def helper() -> i64 { 2 }\n"),
            ],
        );
        let diagnostic = semantic_diagnostics_for_document(&index, &uri, source)
            .into_iter()
            .find(|diagnostic| diagnostic.data.is_some())
            .unwrap();

        assert!(quick_fix_actions(&index, uri, source, vec![diagnostic]).is_empty());
    }

    #[test]
    fn unresolved_symbol_action_rejects_duplicate_exports_from_one_module() {
        let source = "def main() -> i64 { helper() }\n";
        let (index, uri) = code_action_workspace(
            source,
            &[(
                "helpers.sg",
                "def helper() -> i64 { 1 }\ndef helper() -> i64 { 2 }\n",
            )],
        );
        let diagnostic = semantic_diagnostics_for_document(&index, &uri, source)
            .into_iter()
            .find(|diagnostic| diagnostic.data.is_some())
            .unwrap();

        assert!(quick_fix_actions(&index, uri, source, vec![diagnostic]).is_empty());
    }

    #[test]
    fn unresolved_auto_import_rejects_method_and_field_candidates() {
        let call = "def main() -> i64 { helper() }\n";
        let (method_index, method_uri) = code_action_workspace(
            call,
            &[(
                "worker.sg",
                "struct Worker {}\nimpl Worker { def helper(self) -> i64 { 1 } }\n",
            )],
        );
        let method_diagnostic = semantic_diagnostics_for_document(&method_index, &method_uri, call)
            .into_iter()
            .find(|diagnostic| diagnostic.data.is_some())
            .unwrap();
        assert!(
            quick_fix_actions(&method_index, method_uri, call, vec![method_diagnostic]).is_empty()
        );

        let variable = "def main() -> i64 { field }\n";
        let (field_index, field_uri) =
            code_action_workspace(variable, &[("record.sg", "struct Record { field: i64 }\n")]);
        let field_diagnostic =
            semantic_diagnostics_for_document(&field_index, &field_uri, variable)
                .into_iter()
                .find(|diagnostic| diagnostic.data.is_some())
                .unwrap();
        assert!(
            quick_fix_actions(&field_index, field_uri, variable, vec![field_diagnostic]).is_empty()
        );
    }

    #[test]
    fn unresolved_action_recomputes_evidence_and_rejects_tampered_data() {
        let source = "def main() -> i64 { helper() }\n";
        let (index, uri) = code_action_workspace(
            source,
            &[(
                "helpers.sg",
                "struct helper {}\ndef helper() -> i64 { 1 }\n",
            )],
        );
        let diagnostic = semantic_diagnostics_for_document(&index, &uri, source)
            .into_iter()
            .find(|diagnostic| diagnostic.data.is_some())
            .unwrap();

        for (field, value) in [
            ("compilerKind", serde_json::json!("type")),
            ("symbolKind", serde_json::json!("type")),
            ("name", serde_json::json!("other")),
            (
                "range",
                serde_json::to_value(Range::new(Position::new(0, 0), Position::new(0, 1))).unwrap(),
            ),
        ] {
            let mut tampered = diagnostic.clone();
            tampered.data.as_mut().unwrap()[field] = value;
            if field == "range" {
                tampered.range = Range::new(Position::new(0, 0), Position::new(0, 1));
            }
            assert!(
                quick_fix_actions(&index, uri.clone(), source, vec![tampered]).is_empty(),
                "tampered {field} must be rejected"
            );
        }
    }

    #[test]
    fn diagnostic_hashes_are_hex_strings_preserved_by_node_json_roundtrip() {
        let source = "def main() -> i64 { helper() }\n";
        let (index, uri) =
            code_action_workspace(source, &[("helpers.sg", "def helper() -> i64 { 1 }\n")]);
        let unresolved = semantic_diagnostics_for_document(&index, &uri, source)
            .into_iter()
            .find(|diagnostic| diagnostic.data.is_some())
            .unwrap()
            .data
            .unwrap();
        let unused_source = "import demo::helpers as helpers;\ndef main() -> i64 { 0 }\n";
        let (unused_index, unused_uri) = code_action_workspace(unused_source, &[]);
        let unused = unused_import_diagnostics(&unused_index, &unused_uri)
            .pop()
            .unwrap()
            .data
            .unwrap();

        for (value, fields) in [
            (unresolved, &["contentHash"][..]),
            (unused, &["contentHash", "sourceHash"][..]),
        ] {
            for field in fields {
                let hash = value[field].as_str().expect("wire hash must be a string");
                assert_eq!(hash.len(), 16);
                assert!(hash
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
            }
            let encoded = serde_json::to_string(&value).unwrap();
            let output = Command::new("node")
                .arg("-e")
                .arg("const v=JSON.parse(process.argv[1]);process.stdout.write(JSON.stringify(v))")
                .arg(&encoded)
                .output()
                .expect("Node must be available for protocol compatibility test");
            assert!(output.status.success());
            let roundtrip: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
            assert_eq!(roundtrip, value);
        }
    }

    #[test]
    fn legacy_numeric_wire_hash_is_never_trusted_for_code_actions() {
        let source = "def main() -> i64 { helper() }\n";
        let (index, uri) =
            code_action_workspace(source, &[("helpers.sg", "def helper() -> i64 { 1 }\n")]);
        let mut diagnostic = semantic_diagnostics_for_document(&index, &uri, source)
            .into_iter()
            .find(|diagnostic| diagnostic.data.is_some())
            .unwrap();
        diagnostic.data.as_mut().unwrap()["contentHash"] =
            serde_json::json!(9_007_199_254_740_993_u64);

        assert!(quick_fix_actions(&index, uri, source, vec![diagnostic]).is_empty());
    }

    #[test]
    fn unresolved_action_rejects_client_forgery_when_current_source_compiles() {
        let source = "def main() -> i64 { 0 }\n";
        let (index, uri) =
            code_action_workspace(source, &[("helpers.sg", "def helper() -> i64 { 1 }\n")]);
        let range = Range::new(Position::new(0, 4), Position::new(0, 8));
        let data = SafeCodeActionDataV1::UnresolvedSymbol {
            schema_version: DIAGNOSTIC_DATA_SCHEMA_VERSION,
            document_uri: uri.clone(),
            document_revision: 7,
            content_hash: WireHash::of(source),
            name: "helper".into(),
            compiler_kind: UnresolvedSymbolKind::Variable,
            symbol_kind: UnresolvedSymbolKind::Function,
            range,
        };
        let forged = Diagnostic {
            range,
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String(UNRESOLVED_SYMBOL_CODE.into())),
            source: Some("sengoo-compiler".into()),
            message: "forged".into(),
            data: Some(serde_json::to_value(data).unwrap()),
            ..Default::default()
        };

        assert!(quick_fix_actions(&index, uri, source, vec![forged]).is_empty());
    }

    #[test]
    fn unused_alias_import_action_removes_only_its_line() {
        let source = "// keep this comment\r\nimport demo::helpers as helpers;\r\ndef main() -> i64 { 0 }\r\n";
        let (index, uri) =
            code_action_workspace(source, &[("helpers.sg", "def helper() -> i64 { 1 }\n")]);
        let diagnostic = unused_import_diagnostics(&index, &uri)
            .pop()
            .expect("unused alias diagnostic");
        let actions = quick_fix_actions(&index, uri, source, vec![diagnostic]);
        let action = match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => action,
            _ => panic!("expected code action"),
        };
        let edit = &action
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .values()
            .next()
            .unwrap()[0];
        assert_eq!(
            edit.range,
            Range::new(Position::new(1, 0), Position::new(2, 0))
        );
        assert!(edit.new_text.is_empty());
    }

    #[test]
    fn unused_alias_import_at_eof_uses_an_in_bounds_range() {
        let source = "import demo::helpers as helpers;";
        let (index, uri) = code_action_workspace(source, &[]);
        let diagnostic = unused_import_diagnostics(&index, &uri)
            .pop()
            .expect("unused alias diagnostic");

        assert_eq!(
            diagnostic.range.end,
            Position::new(0, line_char_len(source))
        );
    }

    fn apply_edit(source: &str, edit: &TextEdit) -> String {
        let start = position_to_byte(source, edit.range.start).unwrap();
        let end = position_to_byte(source, edit.range.end).unwrap();
        format!("{}{}{}", &source[..start], edit.new_text, &source[end..])
    }

    #[test]
    fn unused_import_analysis_uses_one_indexed_fact_on_a_shared_line() {
        let source =
            "import demo::a as a; import demo::b as b;\ndef main() -> i64 { a::value() }\n";
        let (index, uri) = code_action_workspace(
            source,
            &[
                ("a.sg", "def value() -> i64 { 1 }\n"),
                ("b.sg", "def value() -> i64 { 2 }\n"),
            ],
        );
        let diagnostics = unused_import_diagnostics(&index, &uri);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].range,
            index.document(&uri).unwrap().imports[1].range
        );

        let actions = quick_fix_actions(&index, uri, source, diagnostics);
        let action = match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => action,
            _ => panic!("expected code action"),
        };
        let edit = &action
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .values()
            .next()
            .unwrap()[0];
        assert_eq!(
            apply_edit(source, edit),
            "import demo::a as a;\ndef main() -> i64 { a::value() }\n"
        );
    }

    #[test]
    fn selective_import_is_removed_only_when_every_selected_name_is_unused() {
        let unused = "// leading 😀\r\nimport demo::helpers { alpha, beta }; // trailing\r\ndef main() -> i64 { 0 }\r\n";
        let (unused_index, unused_uri) = code_action_workspace(unused, &[]);
        let diagnostics = unused_import_diagnostics(&unused_index, &unused_uri);
        assert_eq!(diagnostics.len(), 1);
        let actions = quick_fix_actions(&unused_index, unused_uri, unused, diagnostics);
        let action = match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => action,
            _ => panic!("expected code action"),
        };
        let edit = &action
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .values()
            .next()
            .unwrap()[0];
        let result = apply_edit(unused, edit);
        assert!(result.contains("// leading 😀"));
        assert!(result.contains("// trailing"));
        assert!(!result.contains("import demo::helpers"));

        let used = "import demo::helpers { alpha, beta };\ndef main() -> i64 { alpha() }\n";
        let (used_index, used_uri) = code_action_workspace(used, &[]);
        assert!(unused_import_diagnostics(&used_index, &used_uri).is_empty());
    }

    #[test]
    fn unused_import_action_rejects_stale_or_mismatched_fact_identity() {
        let source = "import demo::helpers as helpers;\ndef main() -> i64 { 0 }\n";
        let (index, uri) = code_action_workspace(source, &[]);
        let diagnostic = unused_import_diagnostics(&index, &uri).pop().unwrap();
        let data = serde_json::from_value::<SafeCodeActionDataV1>(diagnostic.data.clone().unwrap())
            .unwrap();
        let SafeCodeActionDataV1::UnusedImport {
            fact_id,
            canonical_identity,
            source_hash,
            ..
        } = data
        else {
            panic!("expected unused import data");
        };
        let fact = &index.document(&uri).unwrap().imports[0];
        assert_eq!(fact_id, fact.fact_id);
        assert_eq!(canonical_identity, fact.canonical_identity);
        assert_eq!(source_hash, WireHash::from_u64(fact.source_hash));

        assert!(index.open(uri.clone(), 8, source.into(), &IndexCancellation::default()));
        assert!(quick_fix_actions(&index, uri.clone(), source, vec![diagnostic]).is_empty());

        let (fresh, fresh_uri) = code_action_workspace(source, &[]);
        let mut mismatched = unused_import_diagnostics(&fresh, &fresh_uri).pop().unwrap();
        let mut value = mismatched.data.take().unwrap();
        value["sourceHash"] = serde_json::Value::from(0_u64);
        mismatched.data = Some(value);
        assert!(quick_fix_actions(&fresh, fresh_uri, source, vec![mismatched]).is_empty());
    }

    #[test]
    fn non_exhaustive_diagnostic_adds_exact_concrete_enum_arms() {
        let source = r#"enum Message { Quit, Move(i64, i64), Write { text: i64 } }

def handle(message: Message) -> i64 {
    match message {
        Message::Quit => 0,
    }
}
"#;
        let (index, uri) = code_action_workspace(source, &[]);
        let diagnostics = semantic_diagnostics_for_document(&index, &uri, source);
        let diagnostic = diagnostics
            .iter()
            .find(|diagnostic| {
                diagnostic.code == Some(NumberOrString::String("non-exhaustive-match".into()))
            })
            .cloned()
            .unwrap_or_else(|| panic!("structured non-exhaustive diagnostic: {diagnostics:#?}"));
        assert!(diagnostic.data.is_some());

        let actions = quick_fix_actions(&index, uri, source, vec![diagnostic]);
        assert_eq!(actions.len(), 1);
        let action = match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => action,
            _ => panic!("expected code action"),
        };
        let text = &action
            .edit
            .as_ref()
            .unwrap()
            .changes
            .as_ref()
            .unwrap()
            .values()
            .next()
            .unwrap()[0]
            .new_text;
        assert!(text.contains("Message::Move(_, _) => todo(),"));
        assert!(text.contains("Message::Write { text: _ } => todo(),"));
        assert!(!text.contains("_ =>"));
    }

    #[test]
    fn enum_arm_detection_uses_exact_patterns_not_prefix_comments_or_strings() {
        let prefix =
            "enum Color { A, AB }\ndef main(c: Color) -> i64 { match c { Color::AB => 1, } }\n";
        let (prefix_index, prefix_uri) = code_action_workspace(prefix, &[]);
        let prefix_diagnostic =
            semantic_diagnostics_for_document(&prefix_index, &prefix_uri, prefix)
                .into_iter()
                .find(|diagnostic| {
                    diagnostic.code == Some(NumberOrString::String("non-exhaustive-match".into()))
                })
                .expect("A must remain missing when only AB is present");
        assert!(prefix_diagnostic.data.is_some());

        let trivia = r#"enum Color { A, B }
def main(c: Color) -> i64 {
    match c {
        // Color::B is not a pattern
        Color::A => { let note = "Color::B"; 1 },
    }
}
"#;
        let (trivia_index, trivia_uri) = code_action_workspace(trivia, &[]);
        let trivia_diagnostic =
            semantic_diagnostics_for_document(&trivia_index, &trivia_uri, trivia)
                .into_iter()
                .find(|diagnostic| {
                    diagnostic.code == Some(NumberOrString::String("non-exhaustive-match".into()))
                })
                .expect("comment/string text must not satisfy B");
        assert!(trivia_diagnostic.data.is_some());
    }

    #[test]
    fn safe_code_actions_reject_incomplete_or_ambiguous_evidence() {
        let duplicate = "def main() -> i64 { helper() + helper() }\n";
        let (duplicate_index, duplicate_uri) =
            code_action_workspace(duplicate, &[("helpers.sg", "def helper() -> i64 { 1 }\n")]);
        let unresolved =
            semantic_diagnostics_for_document(&duplicate_index, &duplicate_uri, duplicate);
        assert!(unresolved
            .iter()
            .all(|diagnostic| diagnostic.data.is_none()));

        let wildcard = "import demo::helpers * from;\ndef main() -> i64 { 0 }\n";
        let (wildcard_index, wildcard_uri) = code_action_workspace(wildcard, &[]);
        assert!(unused_import_diagnostics(&wildcard_index, &wildcard_uri).is_empty());

        let commented = "import demo::helpers as helpers; // keep\ndef main() -> i64 { 0 }\n";
        let (commented_index, commented_uri) = code_action_workspace(commented, &[]);
        assert_eq!(
            unused_import_diagnostics(&commented_index, &commented_uri).len(),
            1
        );

        let recovered = "import demo::helpers as helpers;\ndef main( -> i64 { 0 }\n";
        let (recovered_index, recovered_uri) = code_action_workspace(recovered, &[]);
        assert!(unused_import_diagnostics(&recovered_index, &recovered_uri).is_empty());
    }

    #[test]
    fn unresolved_action_rejects_hash_mismatch_and_uses_utf16_range() {
        let source = "def main() -> i64 { let marker = \"😀\"; helper() }\n";
        let (index, uri) =
            code_action_workspace(source, &[("helpers.sg", "def helper() -> i64 { 1 }\n")]);
        let diagnostic = semantic_diagnostics_for_document(&index, &uri, source)
            .into_iter()
            .find(|diagnostic| {
                diagnostic.code == Some(NumberOrString::String(UNRESOLVED_SYMBOL_CODE.into()))
            })
            .expect("structured unresolved diagnostic");
        let expected = find_symbol_occurrences(source, "helper");
        assert_eq!(diagnostic.range, expected[0]);
        let prefix = source
            .lines()
            .next()
            .unwrap()
            .split("helper")
            .next()
            .unwrap();
        assert_eq!(
            diagnostic.range.start.character,
            prefix.encode_utf16().count() as u32
        );

        let changed = source.replace("helper()", " helper()");
        assert!(quick_fix_actions(&index, uri, &changed, vec![diagnostic]).is_empty());
    }

    #[test]
    fn missing_enum_action_rejects_unknown_or_incomplete_variant_set() {
        let source = "enum Color { Red, Green }\ndef main(c: Color) -> i64 { match c { Color::Red => 1, } }\n";
        let (index, uri) = code_action_workspace(source, &[]);
        let mut diagnostic = semantic_diagnostics_for_document(&index, &uri, source)
            .into_iter()
            .find(|diagnostic| {
                diagnostic.code == Some(NumberOrString::String("non-exhaustive-match".into()))
            })
            .unwrap();
        let mut data =
            serde_json::from_value::<SafeCodeActionDataV1>(diagnostic.data.clone().unwrap())
                .unwrap();
        if let SafeCodeActionDataV1::MissingEnumArms { missing, .. } = &mut data {
            missing.push("Unknown".into());
        }
        diagnostic.data = Some(serde_json::to_value(data).unwrap());

        assert!(quick_fix_actions(&index, uri, source, vec![diagnostic]).is_empty());
    }

    #[test]
    fn diagnostics_cover_three_quick_fix_kinds() {
        let text = "\tlet x = 1; // TODO   ";
        let diagnostics = build_diagnostics(text, 16);
        let mut codes = diagnostics
            .into_iter()
            .filter_map(|d| match d.code {
                Some(NumberOrString::String(code)) => Some(code),
                _ => None,
            })
            .collect::<Vec<_>>();
        codes.sort();
        assert_eq!(
            codes,
            vec![
                "tab-indentation".to_string(),
                "todo-item".to_string(),
                "trailing-whitespace".to_string()
            ]
        );
    }

    #[test]
    fn quick_fix_actions_cover_style_diagnostics() {
        let text = "\tlet x = 1; // TODO   ";
        let diagnostics = build_diagnostics(text, 16);
        let actions = quick_fix_actions(
            &WorkspaceIndex::default(),
            Url::parse("file:///workspace/main.sg").unwrap(),
            text,
            diagnostics,
        );

        let titles = actions
            .iter()
            .map(|action| match action {
                CodeActionOrCommand::CodeAction(action) => action.title.as_str(),
                CodeActionOrCommand::Command(_) => panic!("expected code action"),
            })
            .collect::<Vec<_>>();

        assert_eq!(
            titles,
            vec![
                "Remove TODO/FIXME marker",
                "Convert tabs to spaces",
                "Remove trailing whitespace"
            ]
        );
    }

    #[test]
    fn parse_line_col_hint_understands_compiler_format() {
        assert_eq!(parse_line_col_hint("line 12, col 34"), Some((12, 34)));
        assert_eq!(parse_line_col_hint("Line 2, Column 9"), Some((2, 9)));
        assert_eq!(parse_line_col_hint("note: expected `}`"), None);
    }

    #[test]
    fn diagnostic_range_from_payload_uses_line_col_details() {
        let payload = SgcErrorPayload {
            ok: Some(false),
            kind: Some("compile_error".to_string()),
            stage: Some("parse".to_string()),
            code: None,
            message: Some("unexpected token".to_string()),
            details: vec!["line 2, col 5".to_string()],
            location: None,
        };

        let src = "def main() -> i64 {\n    123\n}\n";
        let range = diagnostic_range_from_payload(src, &payload).expect("range should be parsed");
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 4);
    }

    #[test]
    fn diagnostic_range_from_payload_prefers_structured_location_span() {
        let src = "def main() -> i64 {\n    let x = 1\n}\n";
        let lo = src.find('x').expect("x should exist") as u32;
        let payload = SgcErrorPayload {
            ok: Some(false),
            kind: Some("compile_error".to_string()),
            stage: Some("parse".to_string()),
            code: None,
            message: Some("unexpected token".to_string()),
            details: vec!["line 1, col 1".to_string()],
            location: Some(SgcErrorLocationPayload {
                line: None,
                column: None,
                span: Some(SgcErrorSpanPayload { lo, hi: lo + 1 }),
            }),
        };

        let range = diagnostic_range_from_payload(src, &payload).expect("range should be parsed");
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 8);
    }

    #[test]
    fn realworld_missing_import_json_diagnostic_maps_to_lsp_range() {
        let src = "import definitely_missing_realworld_module;\n\ndef main() -> i64 {\n    0\n}\n";
        let lo = src
            .find("definitely_missing_realworld_module")
            .expect("fixture import should exist") as u32;
        let hi = lo + "definitely_missing_realworld_module".len() as u32;
        let stderr = format!(
            r#"{{
  "ok": false,
  "kind": "compile_error",
  "stage": "import",
  "message": "unresolved source import 'definitely_missing_realworld_module' from main.sg",
  "input": "main.sg",
  "hint": "check the import path, package module map, or supported stdlib module",
  "details": [],
  "location": {{
    "line": 1,
    "column": 8,
    "span": {{ "lo": {lo}, "hi": {hi} }}
  }}
}}"#
        );

        let diagnostics = diagnostics_from_failed_sgc_output(src, &stderr);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].source.as_deref(), Some("sgc"));
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String("import".to_string()))
        );
        assert_eq!(diagnostics[0].range.start.line, 0);
        assert_eq!(diagnostics[0].range.start.character, 7);
        assert!(diagnostics[0]
            .message
            .contains("definitely_missing_realworld_module"));
    }

    #[test]
    fn sgc_json_diagnostic_code_is_preserved_for_lsp() {
        let src = "async def main() -> i64 { 0 }\n";
        let stderr = r#"{
  "ok": false,
  "kind": "compile_error",
  "stage": "mir_lower",
  "code": "async::user_future_contract",
  "message": "Poll<T> must contain `is_ready: bool` followed by `value: T`",
  "details": []
}"#;

        let diagnostics = diagnostics_from_failed_sgc_output(src, stderr);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(
                "async::user_future_contract".to_string()
            ))
        );
        assert_eq!(diagnostics[0].source.as_deref(), Some("sgc"));
    }

    fn assert_embedded_user_future_contract_code(src: &str, expected_message: &str) {
        let diagnostics = embedded_compiler_diagnostics(src);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].source.as_deref(), Some("sengoo-compiler"));
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(
                "async::user_future_contract".to_string()
            ))
        );
        assert!(
            diagnostics[0].message.contains(expected_message),
            "diagnostic should contain `{expected_message}`, got {}",
            diagnostics[0].message
        );
    }

    #[test]
    fn embedded_compiler_reports_user_future_contract_codes() {
        assert_embedded_user_future_contract_code(
            r#"
struct Poll<T> {
    value: T,
}

struct AsyncContext {
    handle: i64,
}

trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> Poll<T> {
        Poll { value: 0 }
    }
}

struct BadFuture {
    value: i64,
}

impl Future<i64> for BadFuture {
    def poll(&mut self, ctx: AsyncContext) -> Poll<i64> {
        Poll { value: self.value }
    }
}

async def main() -> i64 {
    await BadFuture { value: 1 }
}
"#,
            "Poll<T> must contain",
        );

        assert_embedded_user_future_contract_code(
            r#"
struct Poll<T> {
    is_ready: bool,
    value: T,
}

struct AsyncContext {
    handle: i64,
}

trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> i64 {
        0
    }
}

struct BadFuture {
    value: i64,
}

impl Future<i64> for BadFuture {
    def poll(&mut self, ctx: AsyncContext) -> i64 {
        self.value
    }
}

async def main() -> i64 {
    await BadFuture { value: 1 }
}
"#,
            "Future<T>::poll must return Poll<T>",
        );

        assert_embedded_user_future_contract_code(
            r#"
struct Poll<T> {
    is_ready: bool,
    value: T,
}

struct AsyncContext {
    handle: i64,
}

trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> Poll<T> {
        Poll { is_ready: false, value: 0 }
    }
}

struct BadFuture {
    value: i64,
}

impl Future<i64> for BadFuture {
    def poll(self, ctx: AsyncContext) -> Poll<i64> {
        Poll { is_ready: true, value: self.value }
    }
}

async def main() -> i64 {
    await BadFuture { value: 1 }
}
"#,
            "Future<T>::poll must use `&mut self` receiver",
        );
    }

    #[test]
    fn embedded_compiler_reports_user_future_missing_wakeup_code() {
        let src = r#"
struct Poll<T> { is_ready: bool, value: T }
struct AsyncContext { handle: i64 }
trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> Poll<T> {
        Poll { is_ready: false, value: 0 }
    }
}
struct NeverWakes {}
impl Future<i64> for NeverWakes {
    def poll(&mut self, ctx: AsyncContext) -> Poll<i64> {
        Poll { is_ready: false, value: 0 }
    }
}
async def main() -> i64 { await NeverWakes {} }
"#;

        let diagnostics = embedded_compiler_diagnostics(src);
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].source.as_deref(), Some("sengoo-compiler"));
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(
                "async::user_future_missing_wakeup".to_string()
            ))
        );
        assert!(diagnostics[0].message.contains("Pending"));
    }

    #[test]
    fn source_span_to_range_maps_offsets() {
        let src = "def main() -> i64 {\n    let x = 1;\n}\n";
        let span: miette::SourceSpan = (src.find('x').expect("x should exist"), 1).into();
        let range = source_span_to_range(src, &span).expect("range should be computed");

        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 8);
        assert_eq!(range.end.line, 1);
    }

    #[test]
    fn diagnostic_range_from_compile_error_handles_parse_span() {
        let src = "def main() -> i64 {\n    let x = 1;\n}\n";
        let span: miette::SourceSpan = (src.find('x').expect("x should exist"), 1).into();
        let err = CompileError::ParseError(ParseError::UnexpectedToken {
            expected: "identifier".to_string(),
            found: ";".to_string(),
            span,
        });

        let range =
            diagnostic_range_from_compile_error(src, &err).expect("range should be extracted");
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 8);
    }

    #[test]
    fn diagnostic_range_from_compile_error_handles_invalid_pattern_with_span() {
        let src = "def main() -> i64 {\n    let = 1;\n}\n";
        let span: miette::SourceSpan = (src.find('=').expect("equal should exist"), 1).into();
        let err = CompileError::ParseError(ParseError::InvalidPatternAt {
            message: "expected identifier".to_string(),
            span,
        });

        let range =
            diagnostic_range_from_compile_error(src, &err).expect("range should be extracted");
        assert_eq!(range.start.line, 1);
        assert_eq!(range.start.character, 8);
    }

    #[test]
    fn immutable_assignment_uses_typeck_code_and_exact_target_range() {
        let src = r#"
def main() -> i64 {
    let value = 1;
    value = value + 1;
    value
}
"#;
        let diagnostics = embedded_compiler_diagnostics(src);
        let target_lo = src.find("value = value").expect("assignment target") as u32;
        let expected = range_from_byte_span(src, target_lo, target_lo + "value".len() as u32);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String("immutable-assignment".to_string()))
        );
        assert_eq!(diagnostics[0].range, expected);
        assert!(diagnostics[0].message.contains("let mut"));
    }

    #[test]
    fn use_after_move_uses_typeck_code_and_exact_target_range() {
        let src = r#"
struct String { handle: i64 }

def main() -> i64 {
    let a: String = String { handle: 1 };
    let b = a;
    a.handle
}
"#;
        let diagnostics = embedded_compiler_diagnostics(src);
        let target_lo = src.rfind("a.handle").expect("moved value use") as u32;
        let expected = range_from_byte_span(src, target_lo, target_lo + "a.".len() as u32);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String("use-after-move".to_string()))
        );
        assert_eq!(diagnostics[0].range, expected);
    }

    #[test]
    fn unsatisfied_trait_bound_uses_typeck_code_and_exact_target_range() {
        let src = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

def consume<T: Showable>(x: T) -> i64 {
    0
}

def main() -> i64 {
    consume(42)
}
"#;
        let diagnostics = embedded_compiler_diagnostics(src);
        let target_lo = src.rfind("consume(").expect("generic call target") as u32;
        let expected = range_from_byte_span(src, target_lo, target_lo + "consume(".len() as u32);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::ERROR));
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(
                "unsatisfied-trait-bound".to_string()
            ))
        );
        assert_eq!(diagnostics[0].range, expected);
        assert!(diagnostics[0].message.contains("Showable"));
    }

    #[test]
    fn enum_value_errors_keep_stable_codes_and_precise_ranges() {
        let cases = [
            (
                "enum Color { Red }\ndef main() -> Color { Color::Blue }\n",
                "unknown-enum-variant",
                "Color::Blue",
            ),
            (
                "enum Maybe { Value(i64) }\ndef main() -> Maybe { Maybe::Value() }\n",
                "enum-variant-arity",
                "Maybe::Value",
            ),
            (
                "enum Maybe { Value(i64) }\ndef main() -> Maybe { Maybe::Value(true) }\n",
                "enum-variant-type",
                "true",
            ),
        ];

        for (src, code, target) in cases {
            let diagnostics = embedded_compiler_diagnostics(src);
            let lo = src.find(target).expect("diagnostic target") as u32;
            let expected = range_from_byte_span(src, lo, lo + target.len() as u32);

            assert_eq!(diagnostics.len(), 1, "{code}");
            assert_eq!(
                diagnostics[0].code,
                Some(NumberOrString::String(code.to_string()))
            );
            assert_eq!(diagnostics[0].range, expected, "{code}");
        }
    }

    #[test]
    fn array_and_closure_errors_keep_stable_codes_and_precise_ranges() {
        let cases = [
            (
                "def main() -> i64 {\n    let values = [1, 2, 3];\n    values[3]\n}\n",
                "array-index-out-of-bounds",
                "3",
            ),
            (
                "def main() -> i64 {\n    let values = [1, 2, 3];\n    values[true]\n}\n",
                "invalid-array-index",
                "true",
            ),
            (
                "def main() -> i64 {\n    let invalid = |value, value| value;\n    invalid(1, 2)\n}\n",
                "duplicate-closure-parameter",
                "value|",
            ),
        ];

        for (src, code, target) in cases {
            let diagnostics = embedded_compiler_diagnostics(src);
            let target_lo = src.rfind(target).expect("diagnostic target") as u32;
            let target_len = target.trim_end_matches('|').len() as u32;
            let expected = range_from_byte_span(src, target_lo, target_lo + target_len);

            assert_eq!(diagnostics.len(), 1, "{code}");
            assert_eq!(
                diagnostics[0].code,
                Some(NumberOrString::String(code.to_string()))
            );
            assert_eq!(diagnostics[0].range, expected, "{code}");
        }
    }

    #[test]
    fn dyn_errors_keep_stable_codes_for_lsp() {
        let cases = [
            (
                "trait Read {}\ntrait Write {}\ndef stream(x: dyn Read + Write) -> i64 { 0 }\n",
                "dyn-multi-trait-unsupported",
            ),
            (
                "trait Show {}\nstruct Box<T> { value: T }\ndef takes(x: Box<dyn Show>) -> i64 { 0 }\n",
                "dyn-box-unsupported",
            ),
        ];

        for (src, code) in cases {
            let diagnostics = embedded_compiler_diagnostics(src);
            assert_eq!(diagnostics.len(), 1, "{code}");
            assert_eq!(
                diagnostics[0].code,
                Some(NumberOrString::String(code.to_string()))
            );
        }
    }

    #[test]
    fn compiler_diagnostics_fall_back_to_embedded_compiler_when_sgc_is_missing() {
        let src = "def main() -> i64 {\n    let = 1;\n}\n";
        let diagnostics = compiler_diagnostics_from_sgc_tool(
            "sglsp-definitely-missing-sgc-for-test",
            &Url::parse("file:///workspace/main.sg").unwrap(),
            src,
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].source.as_deref(), Some("sengoo-compiler"));
        assert_eq!(diagnostics[0].range.start.line, 1);
        assert_eq!(diagnostics[0].range.start.character, 8);
        assert!(
            diagnostics[0].message.contains("expected identifier"),
            "message should include embedded compiler error: {}",
            diagnostics[0].message
        );
    }

    #[test]
    fn deprecated_warning_from_sgc_json_maps_to_lsp_warning() {
        let src = r#"
#[deprecated("use new_main instead")]
def old_main() -> i64 { 1 }

def main() -> i64 {
    old_main()
}
"#;
        let lo = src.rfind("old_main").expect("call site should exist") as u32;
        let hi = lo + "old_main".len() as u32;
        let stderr = format!(
            r#"{{"ok":true,"kind":"compile_warning","severity":"warning","code":"attributes::deprecated_use","message":"use of deprecated fn `old_main`: use new_main instead","replacement":"new_main","removal":"v0.3.0","location":{{"span":{{"lo":{lo},"hi":{hi}}}}}}}"#
        );

        let diagnostics = diagnostics_from_successful_sgc_output(src, &stderr);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].severity, Some(DiagnosticSeverity::WARNING));
        assert_eq!(
            diagnostics[0].code,
            Some(NumberOrString::String(
                "attributes::deprecated_use".to_string()
            ))
        );
        assert!(diagnostics[0].message.contains("old_main"));
        assert_eq!(
            diagnostics[0].data,
            Some(serde_json::json!({
                "replacement": "new_main",
                "removal": "v0.3.0"
            }))
        );
        assert_eq!(diagnostics[0].range.start.line, 5);
        assert_eq!(diagnostics[0].range.start.character, 4);
    }

    #[test]
    fn embedded_deprecated_warning_prefers_compiler_span_over_text_search() {
        let src = r#"
#[deprecated("use new_main instead")]
def old_main() -> i64 { 1 }

def main() -> i64 {
    old_main()
}
"#;
        let lo = src.rfind("old_main").expect("call site should exist") as u32;
        let warning = CompileWarning::deprecated_use(
            "fn",
            "old_main",
            Some("use new_main instead".to_string()),
            Some((lo, lo + "old_main".len() as u32)),
        );

        let diagnostic = diagnostic_from_compile_warning(src, warning);

        assert_eq!(diagnostic.source.as_deref(), Some("sengoo-compiler"));
        assert_eq!(diagnostic.range.start.line, 5);
        assert_eq!(diagnostic.range.start.character, 4);
    }

    #[test]
    fn cfg_false_declaration_produces_no_embedded_diagnostic() {
        let other_os = if cfg!(target_os = "windows") {
            "linux"
        } else {
            "windows"
        };
        let src = format!(
            r#"
#[cfg(target_os = "{other_os}")]
def hidden() -> i64 {{ missing_name }}

def main() -> i64 {{ 0 }}
"#
        );

        assert!(embedded_compiler_diagnostics(&src).is_empty());
    }

    #[test]
    fn unsupported_attribute_maps_to_its_source_range() {
        let src = "#[must_use]\nstruct Bad {}\n";
        let diagnostics = embedded_compiler_diagnostics(src);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].range.start.line, 0);
        assert_eq!(diagnostics[0].range.start.character, 2);
        assert!(diagnostics[0].message.contains("unsupported attribute"));
    }

    #[test]
    fn non_exhaustive_match_without_structured_data_has_no_quick_fix() {
        let text =
            "def paint(c: Color) -> i64 {\n    match c {\n        Color::Red => 1,\n    }\n}\n";
        let uri = Url::parse("file:///workspace/match.sg").unwrap();
        let diagnostic = Diagnostic {
            range: Range {
                start: Position {
                    line: 1,
                    character: 4,
                },
                end: Position {
                    line: 1,
                    character: 12,
                },
            },
            severity: Some(DiagnosticSeverity::ERROR),
            code: Some(NumberOrString::String("non-exhaustive-match".to_string())),
            source: Some("sgc".to_string()),
            message: "[non-exhaustive-match] match is not exhaustive".to_string(),
            ..Default::default()
        };
        let actions = quick_fix_actions(&WorkspaceIndex::default(), uri, text, vec![diagnostic]);
        assert!(actions.is_empty());
    }

    #[test]
    fn json_error_payload_preserves_stable_code_and_matches_embedded_compiler() {
        let src = r#"
enum Color { Red, Blue }

def paint(c: Color) -> i64 {
    match c {
        Color::Red => 1,
    }
}
"#;
        let embedded = embedded_compiler_diagnostics(src);
        assert_eq!(embedded.len(), 1);
        assert_eq!(
            embedded[0].code,
            Some(NumberOrString::String("non-exhaustive-match".to_string()))
        );
        assert!(
            !embedded[0]
                .message
                .chars()
                .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)),
            "embedded diagnostics must be English, got: {}",
            embedded[0].message
        );

        let payload = r#"{
            "schema_version": 1,
            "ok": false,
            "kind": "compile_error",
            "stage": "typecheck",
            "code": "non-exhaustive-match",
            "message": "[non-exhaustive-match] match is not exhaustive: missing Blue",
            "input": "match.sg"
        }"#;
        let from_json = diagnostics_from_failed_sgc_output(src, payload);
        assert_eq!(from_json.len(), 1);
        assert_eq!(from_json[0].code, embedded[0].code);
        assert_eq!(from_json[0].source.as_deref(), Some("sgc"));
        assert!(from_json[0].message.contains("non-exhaustive-match"));
        assert_eq!(from_json[0].severity, embedded[0].severity);
    }

    #[test]
    fn compiler_diagnostics_fall_back_to_embedded_compiler_when_sgc_output_is_not_json() {
        let src = "def main() -> i64 {\n    let = 1;\n}\n";
        let diagnostics = diagnostics_from_failed_sgc_output(
            src,
            "error: unrecognized option '--error-format'\ntry 'sgc --help'\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].source.as_deref(), Some("sengoo-compiler"));
        assert_eq!(diagnostics[0].range.start.line, 1);
        assert_eq!(diagnostics[0].range.start.character, 8);
        assert!(
            diagnostics[0].message.contains("expected identifier"),
            "message should include embedded compiler error: {}",
            diagnostics[0].message
        );
    }
}
