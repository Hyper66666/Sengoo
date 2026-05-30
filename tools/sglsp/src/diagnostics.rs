use crate::formatting::full_document_range;
use sengoo_compiler::error::{ParseError, TypeError};
use sengoo_compiler::{compile_to_ir, CompileError};
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
    message: Option<String>,
    #[serde(default)]
    details: Vec<String>,
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
        | ParseError::InvalidPatternAt { span, .. } => source_span_to_range(content, span),
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
        _ => None,
    }
}

fn fallback_diagnostic_range_from_compiler(content: &str) -> Option<Range> {
    compile_to_ir(content)
        .err()
        .and_then(|error| diagnostic_range_from_compile_error(content, &error))
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
    let scratch = temporary_source_path(uri);
    if fs::write(&scratch, content).is_err() {
        return Vec::new();
    }

    let output = Command::new("sgc")
        .arg("--error-format")
        .arg("json")
        .arg("check")
        .arg(&scratch)
        .output();

    let _ = fs::remove_file(&scratch);

    let Ok(output) = output else {
        return Vec::new();
    };
    if output.status.success() {
        return Vec::new();
    }

    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let payload = parse_sgc_payload(&stderr);
    let fallback_range = fallback_diagnostic_range_from_compiler(content);

    let (message, code, range) = if let Some(payload) = payload {
        let range = diagnostic_range_from_payload(content, &payload)
            .or(fallback_range)
            .unwrap_or_else(|| full_document_range(content));
        let mut message = payload
            .message
            .unwrap_or_else(|| "compilation failed".to_string());
        if !payload.details.is_empty() {
            message.push('\n');
            message.push_str(&payload.details.join("\n"));
        }
        (message, payload.stage, range)
    } else {
        let summary = stderr
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .unwrap_or_else(|| "compilation failed".to_string());
        let range = fallback_range.unwrap_or_else(|| full_document_range(content));
        (summary, None, range)
    };

    vec![Diagnostic {
        range,
        severity: Some(DiagnosticSeverity::ERROR),
        code: code.map(NumberOrString::String),
        source: Some("sgc".to_string()),
        message,
        ..Default::default()
    }]
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
}
