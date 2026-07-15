use crate::formatting::full_document_range;
use sengoo_compiler::error::{ParseError, TypeError};
use sengoo_compiler::{collect_compile_warnings, compile_to_ir, CompileError, CompileWarning};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_lsp::lsp_types::*;

use super::semantic::{byte_to_char_index, line_char_len};
use super::text_editing::span_to_range;

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

fn embedded_compiler_diagnostics(content: &str) -> Vec<Diagnostic> {
    match compile_to_ir(content) {
        Ok(_) => collect_compile_warnings(content)
            .unwrap_or_default()
            .into_iter()
            .map(|warning| diagnostic_from_compile_warning(content, warning))
            .collect(),
        Err(error) => {
            let range = diagnostic_range_from_compile_error(content, &error)
                .unwrap_or_else(|| full_document_range(content));

            vec![Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::ERROR),
                code: Some(NumberOrString::String(
                    diagnostic_code_from_compile_error(&error)
                        .unwrap_or_else(|| compile_error_stage(&error).to_string()),
                )),
                source: Some("sengoo-compiler".to_string()),
                message: error.to_string(),
                ..Default::default()
            }]
        }
    }
}

fn warning_subject_range(content: &str, message: &str) -> Option<Range> {
    let name = message.split('`').nth(1)?;
    let lo = content.find(name)? as u32;
    Some(range_from_byte_span(content, lo, lo + name.len() as u32))
}

fn diagnostic_from_compile_warning(content: &str, warning: CompileWarning) -> Diagnostic {
    let message = warning.to_string();
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
        ..Default::default()
    }
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
            Diagnostic {
                range,
                severity: Some(DiagnosticSeverity::WARNING),
                code: payload.code.map(NumberOrString::String),
                source: Some("sgc".to_string()),
                message,
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

struct WildcardInsertion {
    range: Range,
    text: String,
}

fn wildcard_arm_insertion(content: &str, diagnostic_range: &Range) -> Option<WildcardInsertion> {
    let lines: Vec<&str> = content.lines().collect();
    let start = diagnostic_range.start.line as usize;
    for (line_idx, line) in lines.iter().enumerate().skip(start) {
        if let Some(open_idx) = line.find('{') {
            let indent = line[..open_idx]
                .chars()
                .take_while(|ch| ch.is_whitespace())
                .collect::<String>();
            let arm_indent = format!("{indent}    ");
            return Some(WildcardInsertion {
                range: Range {
                    start: Position {
                        line: line_idx as u32,
                        character: line.len() as u32,
                    },
                    end: Position {
                        line: line_idx as u32,
                        character: line.len() as u32,
                    },
                },
                text: format!("\n{arm_indent}_ => todo(),"),
            });
        }
        if line.contains("=>") {
            break;
        }
    }
    None
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

pub(crate) fn quick_fix_actions(
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
            "non-exhaustive-match" => {
                if let Some(insert) = wildcard_arm_insertion(content, &diagnostic.range) {
                    actions.push(quick_fix_action(
                        uri.clone(),
                        TextEdit {
                            range: insert.range,
                            new_text: insert.text,
                        },
                        diagnostic,
                        "Add wildcard match arm",
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
    use sengoo_compiler::error::ParseError;
    use tower_lsp::lsp_types::{CodeActionOrCommand, NumberOrString, Url};

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
            r#"{{"ok":true,"kind":"compile_warning","severity":"warning","code":"attributes::deprecated_use","message":"use of deprecated fn `old_main`: use new_main instead","location":{{"span":{{"lo":{lo},"hi":{hi}}}}}}}"#
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
    fn non_exhaustive_match_quick_fix_adds_wildcard_arm() {
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
        let actions = quick_fix_actions(uri, text, vec![diagnostic]);
        assert_eq!(actions.len(), 1);
        let title = match &actions[0] {
            CodeActionOrCommand::CodeAction(action) => action.title.as_str(),
            CodeActionOrCommand::Command(_) => panic!("expected code action"),
        };
        assert_eq!(title, "Add wildcard match arm");
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
