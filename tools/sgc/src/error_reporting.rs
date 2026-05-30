use sengoo_compiler::error::{ParseError, TypeError};
use sengoo_compiler::CompileError;

use super::{
    current_error_format, CompilerErrorJson, CompilerErrorLocationJson, CompilerErrorSpanJson,
    ErrorFormat,
};

fn compile_error_details(raw: &str) -> Vec<String> {
    raw.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .skip(1)
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>()
}

fn compile_error_payload(
    input: Option<&str>,
    raw: &str,
    location: Option<CompilerErrorLocationJson>,
) -> CompilerErrorJson {
    let (stage, message) = split_compiler_error_stage(raw);
    CompilerErrorJson {
        ok: false,
        kind: "compile_error",
        stage,
        message,
        input: input.map(str::to_owned),
        hint: Some("use --error-format text for human-friendly diagnostics".to_string()),
        details: compile_error_details(raw),
        location,
    }
}

pub(crate) fn render_compile_error_json_with_location(
    input: Option<&str>,
    raw: &str,
    location: Option<CompilerErrorLocationJson>,
) -> String {
    let payload = compile_error_payload(input, raw, location);
    if let Ok(encoded) = serde_json::to_string_pretty(&payload) {
        return encoded;
    }

    format!(
        r#"{{"ok":false,"kind":"compile_error","stage":"{}","message":"{}"}}"#,
        payload.stage,
        raw.replace('"', "\\\"")
    )
}

#[cfg(test)]
pub(crate) fn render_compile_error_json(input: Option<&str>, raw: &str) -> String {
    render_compile_error_json_with_location(input, raw, None)
}
pub(super) fn split_compiler_error_stage(raw: &str) -> (&'static str, String) {
    let text = raw.trim();
    let mapping: [(&str, &str); 12] = [
        ("parse failed:", "parse"),
        ("typecheck failed:", "typecheck"),
        ("codegen failed:", "codegen"),
        ("invalid optimization level:", "config"),
        ("failed to create LLVM IR output", "io"),
        ("failed to write LLVM IR", "io"),
        ("MIR lowering failed:", "mir_lower"),
        ("compile failed", "compile"),
        ("parse error:", "parse"),
        ("type check error:", "typecheck"),
        ("type error:", "typecheck"),
        ("io error:", "io"),
    ];
    for (prefix, stage) in mapping {
        if let Some(rest) = text.strip_prefix(prefix) {
            let summary = rest.lines().next().unwrap_or(rest).trim().to_string();
            return (stage, summary);
        }
    }
    let summary = text.lines().next().unwrap_or(text).trim().to_string();
    ("compile", summary)
}

fn source_span_from_parse_error(error: &ParseError) -> Option<&miette::SourceSpan> {
    match error {
        ParseError::UnexpectedToken { span, .. }
        | ParseError::UnclosedBlock(span)
        | ParseError::UnclosedParen(span)
        | ParseError::InvalidStructField { span, .. }
        | ParseError::InvalidStructFieldShorthand { span }
        | ParseError::InvalidPatternAt { span, .. } => Some(span),
        ParseError::InvalidPattern(_)
        | ParseError::DuplicateParam(_)
        | ParseError::UnexpectedEof => None,
    }
}

fn source_span_from_type_error(error: &TypeError) -> Option<&miette::SourceSpan> {
    match error {
        TypeError::Mismatch { span, .. } => Some(span),
        TypeError::UndefinedVar { _span, .. } => Some(_span),
        TypeError::UndefinedType(_)
        | TypeError::UndefinedMethod(_)
        | TypeError::ArgCountMismatch { .. }
        | TypeError::TraitNotImplemented { .. } => None,
    }
}

fn source_span_from_compile_error(error: &CompileError) -> Option<&miette::SourceSpan> {
    match error {
        CompileError::ParseError(error) => source_span_from_parse_error(error),
        CompileError::TypeError(error) => source_span_from_type_error(error),
        _ => None,
    }
}

fn line_column_for_offset(source: &str, offset: usize) -> (u32, u32) {
    let clamped = offset.min(source.len());
    let mut line = 1u32;
    let mut line_start = 0usize;

    for (idx, ch) in source.char_indices() {
        if idx >= clamped {
            break;
        }
        if ch == '\n' {
            line += 1;
            line_start = idx + 1;
        }
    }

    let column = source[line_start..clamped].encode_utf16().count() as u32 + 1;
    (line, column)
}

fn location_from_source_span(source: &str, span: &miette::SourceSpan) -> CompilerErrorLocationJson {
    let lo_usize: usize = span.offset();
    let lo_clamped = lo_usize.min(source.len());
    let mut hi_clamped = lo_clamped.saturating_add(span.len()).min(source.len());
    if hi_clamped == lo_clamped && lo_clamped < source.len() {
        hi_clamped = lo_clamped + 1;
    }

    let (line, column) = line_column_for_offset(source, lo_clamped);
    let lo = u32::try_from(lo_clamped).unwrap_or(u32::MAX);
    let hi = u32::try_from(hi_clamped).unwrap_or(u32::MAX);

    CompilerErrorLocationJson {
        line: Some(line),
        column: Some(column),
        span: Some(CompilerErrorSpanJson { lo, hi }),
    }
}

pub(super) fn location_from_compile_error(
    source: &str,
    error: &CompileError,
) -> Option<CompilerErrorLocationJson> {
    source_span_from_compile_error(error).map(|span| location_from_source_span(source, span))
}

pub(crate) fn emit_compile_error_with_location(
    input: Option<&str>,
    raw: &str,
    location: Option<CompilerErrorLocationJson>,
) {
    match current_error_format() {
        ErrorFormat::Text => {
            eprintln!("Compilation error:");
            eprintln!("{}", raw);
        }
        ErrorFormat::Json => {
            eprintln!(
                "{}",
                render_compile_error_json_with_location(input, raw, location)
            );
        }
    }
}

pub(crate) fn emit_compile_error(input: Option<&str>, raw: &str) {
    emit_compile_error_with_location(input, raw, None)
}
