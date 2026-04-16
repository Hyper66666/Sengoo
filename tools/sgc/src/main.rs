//! Sengoo CLI compiler (`sgc`).

use clap::ValueEnum;
use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::sync::atomic::{AtomicI8, AtomicU8, Ordering};
use std::sync::Arc;
use tokio::time::Duration;
use tracing_subscriber::{fmt, EnvFilter};
use sengoo_compiler::error::{ParseError, TypeError};
use sengoo_compiler::{compile_to_ir, CompileError};

mod bench;
mod cache;
mod cli;
#[cfg_attr(not(test), allow(dead_code))]
mod cranelift_fast_jit;
mod commands;
mod daemon;
mod fingerprint;
mod frontend_helpers;
mod frontend_snapshot;
mod generic_cache;
mod graph_builder;
mod impact;
mod interface;
mod module_graph;
mod native_toolchain;
mod pipeline;
mod reflection;
mod reflection_native;
mod reflection_sidecar;
mod symbol_intern;
mod toolchain_discovery;
mod workset;
#[cfg(test)]
pub(crate) use commands::{
    can_reuse_artifacts_for_unreachable_impl_only_changes, can_skip_codegen_via_generic_cache,
};
pub(crate) use commands::{cmd_build, cmd_run};

#[cfg(test)]
pub(crate) use bench::bench_root_dir;
pub(crate) use bench::{
    cmd_bench_compile, cmd_bench_incremental, cmd_bench_reflection, cmd_bench_run,
};
#[cfg(test)]
pub(crate) use bench::{collect_bench_cases, resolve_bench_suite_path};
pub(crate) use cache::{
    frontend_session_store_path, load_build_cache, load_frontend_session_store, load_run_cache,
    save_build_cache, save_frontend_session_store, save_run_cache,
};
pub(crate) use daemon::{
    cmd_daemon, dispatch_build_via_daemon, dispatch_run_via_daemon, resolve_daemon_addr,
    DaemonDispatchOutcome,
};
#[cfg(test)]
pub(crate) use daemon::{daemon_request_build, handle_daemon_client, send_daemon_request};
pub(crate) use fingerprint::{
    file_fingerprint, implementation_fingerprint, implementation_fingerprint_from_normalized,
    interface_fingerprint, interface_fingerprint_fast, interface_fingerprint_fast_from_normalized,
    normalize_source_for_hash, resolve_root_hashes_for_request, source_fingerprint,
};
pub(crate) use frontend_helpers::{
    dependency_graph_digest, frontend_cache_entry_for_module, frontend_probe_module_body_only,
    frontend_probe_module_full, hir_fragment_fingerprint, merge_frontend_phase_stats,
    resolve_frontend_job_count, run_frontend_tasks_deterministic,
};
pub(crate) use frontend_snapshot::{collect_module_graph_snapshot, module_fingerprints_for_source};
pub(crate) use generic_cache::{
    derive_generic_instance_plan, generic_instance_cache_path, generic_instance_hit_ratio,
    load_generic_instance_cache, save_generic_instance_cache,
};
#[cfg(test)]
pub(crate) use graph_builder::build_graph_v2_for_source;
pub(crate) use graph_builder::build_graph_v2_with_function_fingerprints_for_source;
#[cfg(test)]
pub(crate) use impact::collect_impl_only_impacted_symbols;
pub(crate) use impact::{
    classify_edit_impact, collect_impl_only_impacted_symbols_with_fallback, edit_class_label,
    format_edit_impact_lines, incremental_link_mode_from_env, module_invalidation_stats,
};
pub(crate) use interface::{
    ast_interface_signature, function_fingerprints_for_module, function_fingerprints_for_program,
    function_signatures_for_module, generic_fingerprints_for_module,
    generic_fingerprints_for_program, interface_fingerprint_from_program,
};
pub(crate) use module_graph::{collect_module_sources_with_edges, module_dependency_levels};
pub(crate) use native_toolchain::{
    append_native_runtime_inputs, artifact_exists, build_artifact_exists, compile_ir_to_object, compile_native_binary,
    default_build_output_path_for_case, ensure_runtime_object, link_native_binary_from_objects,
    linker_mode_from_env, recover_native_output_from_cached_artifacts, run_native_binary,
    run_with_lli,
};
#[cfg(test)]
pub(crate) use native_toolchain::{derive_cached_native_recovery_plan, parse_linker_mode};
#[cfg(test)]
pub(crate) use pipeline::{compile_source, compile_source_with_phase_timings};
pub(crate) use pipeline::{
    compile_source_to_llvm_file_with_phase_timings,
    compile_source_to_llvm_file_with_phase_timings_with_mode, set_contract_runtime_checks_override,
    set_large_project_mode_override,
};
#[cfg(test)]
pub(crate) use reflection::source_requests_reflection;
pub(crate) use reflection::{
    decl_requests_reflection, reflection_mode_note, reflection_options_from_cli,
    resolve_reflection_options_for_snapshot,
};
pub(crate) use reflection_native::{
    maybe_prepare_reflection_native_library, measure_reflection_used_ms,
};
#[cfg(test)]
pub(crate) use reflection_native::{
    select_reflection_i64_zero_arity_symbol, signature_is_zero_arity_i64,
};
#[cfg(test)]
pub(crate) use reflection_sidecar::build_reflection_metadata;
pub(crate) use reflection_sidecar::{
    maybe_emit_reflection_sidecar, reflection_sidecar_path_for_artifact,
    validate_reflection_metadata,
};
pub(crate) use toolchain_discovery::{find_clang, find_lli, find_runtime_c};
pub(crate) use workset::{
    build_cache_key, build_cache_mismatch_reasons, build_metadata_matches, cache_key,
    cache_mismatch_reasons, can_use_incremental_link_with_metadata,
    can_use_incremental_link_with_run_metadata, codegen_workset_manifest_path,
    derive_build_workset_plan, derive_codegen_workset_manifest, derive_run_workset_plan,
    metadata_matches, resolve_engine, save_codegen_workset_manifest,
};

include!("model_types.rs");

const ERROR_FORMAT_TEXT_WIRE: u8 = 0;
const ERROR_FORMAT_JSON_WIRE: u8 = 1;
static ERROR_FORMAT_MODE: AtomicU8 = AtomicU8::new(ERROR_FORMAT_TEXT_WIRE);

fn default_build_graph_schema_version() -> u32 {
    BUILD_GRAPH_SCHEMA_VERSION
}

fn default_build_cache_schema_version() -> u32 {
    1
}

fn error_format_to_wire(format: ErrorFormat) -> u8 {
    match format {
        ErrorFormat::Text => ERROR_FORMAT_TEXT_WIRE,
        ErrorFormat::Json => ERROR_FORMAT_JSON_WIRE,
    }
}

pub(crate) fn set_error_format(format: ErrorFormat) {
    ERROR_FORMAT_MODE.store(error_format_to_wire(format), Ordering::Relaxed);
}

pub(crate) fn current_error_format() -> ErrorFormat {
    match ERROR_FORMAT_MODE.load(Ordering::Relaxed) {
        ERROR_FORMAT_JSON_WIRE => ErrorFormat::Json,
        _ => ErrorFormat::Text,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompilerErrorSpanJson {
    lo: u32,
    hi: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompilerErrorLocationJson {
    #[serde(skip_serializing_if = "Option::is_none")]
    line: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    column: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    span: Option<CompilerErrorSpanJson>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct CompilerErrorJson {
    ok: bool,
    kind: &'static str,
    stage: &'static str,
    message: String,
    input: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hint: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    details: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<CompilerErrorLocationJson>,
}

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
fn split_compiler_error_stage(raw: &str) -> (&'static str, String) {
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
        ParseError::InvalidPattern(_) | ParseError::DuplicateParam(_) | ParseError::UnexpectedEof => {
            None
        }
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

fn location_from_compile_error(source: &str, error: &CompileError) -> Option<CompilerErrorLocationJson> {
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
#[tokio::main]
async fn main() -> Result<()> {
    let filter = EnvFilter::from_default_env().add_directive("sgc=info".parse().unwrap());
    fmt().with_env_filter(filter).with_target(false).init();

    cli::run().await
}

fn parse_frontend_jobs_arg(raw: &str) -> std::result::Result<FrontendJobs, String> {
    let trimmed = raw.trim();
    if trimmed.eq_ignore_ascii_case("auto") {
        return Ok(FrontendJobs::Auto);
    }

    let parsed = trimmed
        .parse::<usize>()
        .map_err(|_| "frontend jobs must be 'auto' or an integer >= 1".to_string())?;
    if parsed == 0 {
        return Err("frontend jobs must be >= 1".to_string());
    }
    Ok(FrontendJobs::Fixed(parsed))
}

fn frontend_jobs_label(frontend_jobs: FrontendJobs) -> String {
    match frontend_jobs {
        FrontendJobs::Auto => "auto".to_string(),
        FrontendJobs::Fixed(value) => value.to_string(),
    }
}

fn parse_frontend_memory_mode_wire(raw: &str) -> FrontendMemoryMode {
    let normalized = raw.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "low-memory" | "low_memory" | "low" => FrontendMemoryMode::LowMemory,
        "stream" => FrontendMemoryMode::Stream,
        "legacy" => FrontendMemoryMode::Legacy,
        _ => FrontendMemoryMode::Auto,
    }
}

fn frontend_memory_mode_label(mode: FrontendMemoryMode) -> &'static str {
    match mode {
        FrontendMemoryMode::Auto => "auto",
        FrontendMemoryMode::LowMemory => "low-memory",
        FrontendMemoryMode::Stream => "stream",
        FrontendMemoryMode::Legacy => "legacy",
    }
}

fn frontend_memory_mode_from_env() -> Option<FrontendMemoryMode> {
    std::env::var("SENGOO_FRONTEND_MEMORY_MODE")
        .ok()
        .map(|raw| parse_frontend_memory_mode_wire(&raw))
}

fn resolve_frontend_memory_mode(source_len_bytes: usize) -> FrontendMemoryMode {
    let _ = source_len_bytes;
    match frontend_memory_mode_from_env().unwrap_or(FrontendMemoryMode::Auto) {
        FrontendMemoryMode::Auto => FrontendMemoryMode::Legacy,
        other => other,
    }
}

fn low_memory_hint_should_recommend(available_bytes: u64, source_len_bytes: usize) -> bool {
    source_len_bytes >= FRONTEND_MEMORY_STREAM_THRESHOLD_BYTES
        && available_bytes <= LOW_MEMORY_HINT_AVAILABLE_BYTES
}

#[cfg(target_os = "windows")]
fn system_available_memory_bytes() -> Option<u64> {
    #[repr(C)]
    struct MemoryStatusEx {
        dw_length: u32,
        dw_memory_load: u32,
        ull_total_phys: u64,
        ull_avail_phys: u64,
        ull_total_page_file: u64,
        ull_avail_page_file: u64,
        ull_total_virtual: u64,
        ull_avail_virtual: u64,
        ull_avail_extended_virtual: u64,
    }

    #[link(name = "kernel32")]
    extern "system" {
        fn GlobalMemoryStatusEx(lp_buffer: *mut MemoryStatusEx) -> i32;
    }

    let mut status = MemoryStatusEx {
        dw_length: std::mem::size_of::<MemoryStatusEx>() as u32,
        dw_memory_load: 0,
        ull_total_phys: 0,
        ull_avail_phys: 0,
        ull_total_page_file: 0,
        ull_avail_page_file: 0,
        ull_total_virtual: 0,
        ull_avail_virtual: 0,
        ull_avail_extended_virtual: 0,
    };

    let ok = unsafe { GlobalMemoryStatusEx(&mut status as *mut MemoryStatusEx) };
    if ok == 0 {
        None
    } else {
        Some(status.ull_avail_phys)
    }
}

#[cfg(not(target_os = "windows"))]
fn system_available_memory_bytes() -> Option<u64> {
    let text = fs::read_to_string("/proc/meminfo").ok()?;
    for line in text.lines() {
        let line = line.trim();
        if !line.starts_with("MemAvailable:") {
            continue;
        }
        let mut parts = line.split_whitespace();
        let _label = parts.next()?;
        let kb = parts.next()?.parse::<u64>().ok()?;
        return Some(kb * 1024);
    }
    None
}

fn maybe_low_memory_mode_hint(source_len_bytes: usize, low_memory_enabled: bool) -> Option<String> {
    if low_memory_enabled {
        return None;
    }
    let available_bytes = system_available_memory_bytes()?;
    if !low_memory_hint_should_recommend(available_bytes, source_len_bytes) {
        return None;
    }
    let available_mib = available_bytes as f64 / (1024.0 * 1024.0);
    let source_mib = source_len_bytes as f64 / (1024.0 * 1024.0);
    Some(format!(
        "hint: low-memory environment detected ({:.0} MiB available). Consider `--low-memory` to reduce peak RSS for this build/run (trade-offs: weaker incremental reuse, single-thread frontend, lower MIR opt). source size: {:.2} MiB",
        available_mib, source_mib
    ))
}

fn symbol_fingerprint_collection_limit_bytes() -> usize {
    std::env::var("SENGOO_SYMBOL_FINGERPRINT_MAX_SOURCE_BYTES")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .unwrap_or(DEFAULT_SYMBOL_FINGERPRINT_MAX_SOURCE_BYTES)
}

fn should_collect_symbol_fingerprints(
    source_len_bytes: usize,
    _force_rebuild: bool,
    low_memory_enabled: bool,
) -> bool {
    if low_memory_enabled {
        return false;
    }
    let limit = symbol_fingerprint_collection_limit_bytes();
    if limit == 0 {
        return true;
    }
    source_len_bytes <= limit
}

fn large_project_prompt_source_threshold_bytes() -> usize {
    std::env::var("SENGOO_LARGE_PROJECT_PROMPT_MIN_SOURCE_BYTES")
        .ok()
        .and_then(|raw| raw.trim().parse::<usize>().ok())
        .unwrap_or(4 * 1024 * 1024)
}

fn parse_large_project_mode_toggle(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "y" | "yes" | "on" | "true" | "enable" | "enabled" => Some(true),
        "2" | "n" | "no" | "off" | "false" | "disable" | "disabled" => Some(false),
        _ => None,
    }
}

fn ci_environment_enabled() -> bool {
    std::env::var("CI").ok().is_some_and(|raw| {
        !matches!(
            raw.trim().to_ascii_lowercase().as_str(),
            "" | "0" | "false" | "off"
        )
    })
}

fn maybe_choose_large_project_optimization_mode(
    source_len_bytes: usize,
    low_memory_enabled: bool,
) -> Option<bool> {
    if low_memory_enabled {
        return None;
    }
    if source_len_bytes < large_project_prompt_source_threshold_bytes() {
        return None;
    }

    if let Ok(raw) = std::env::var("SENGOO_LARGE_PROJECT_MODE") {
        if let Some(enabled) = parse_large_project_mode_toggle(&raw) {
            println!(
                "large-project optimization mode: {} (from SENGOO_LARGE_PROJECT_MODE)",
                if enabled { "enabled" } else { "disabled" }
            );
            return Some(enabled);
        }
        println!(
            "large-project optimization mode: invalid SENGOO_LARGE_PROJECT_MODE='{}', fallback to interactive/default",
            raw
        );
    }

    if ci_environment_enabled() || !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        println!("large-project optimization mode: enabled (default in non-interactive/CI)");
        return Some(true);
    }

    let source_mib = source_len_bytes as f64 / (1024.0 * 1024.0);
    println!(
        "detected large project input ({:.2} MiB). choose optimization mode:",
        source_mib
    );
    println!("  1) enable (recommended)");
    println!("  2) disable");
    for _ in 0..3 {
        print!("select [1/2, default 1]: ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            println!("input read failed, defaulting to enable");
            return Some(true);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            println!("large-project optimization mode: enabled");
            return Some(true);
        }
        if let Some(enabled) = parse_large_project_mode_toggle(trimmed) {
            println!(
                "large-project optimization mode: {}",
                if enabled { "enabled" } else { "disabled" }
            );
            return Some(enabled);
        }
        println!("invalid selection, enter 1 (enable) or 2 (disable)");
    }

    println!("too many invalid inputs, defaulting to enable");
    Some(true)
}

fn frontend_trace_enabled(explicit_flag: bool) -> bool {
    if explicit_flag {
        return true;
    }

    let Ok(raw) = std::env::var("SENGOO_FRONTEND_TRACE") else {
        return false;
    };
    let normalized = raw.trim().to_ascii_lowercase();
    matches!(
        normalized.as_str(),
        "1" | "true" | "on" | "yes" | "trace" | "debug"
    )
}

fn object_file_extension() -> &'static str {
    if cfg!(windows) {
        "obj"
    } else {
        "o"
    }
}

fn canonical_or_lossy(path: &Path) -> String {
    fs::canonicalize(path)
        .unwrap_or_else(|_| path.to_path_buf())
        .to_string_lossy()
        .to_string()
}

async fn cmd_check(input: &str) -> Result<()> {
    println!("Checking: {}", input);

    let source = fs::read_to_string(input)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to read source {}: {}", input, e))?;

    match compile_to_ir(&source) {
        Ok(_) => {
            println!("Type check passed");
            Ok(())
        }
        Err(error) => {
            let raw = error.to_string();
            let location = location_from_compile_error(&source, &error);
            emit_compile_error_with_location(Some(input), &raw, location);
            Err(miette::miette!("compile failed"))
        }
    }
}

async fn cmd_repl() -> Result<()> {
    println!("Sengoo REPL v{}", env!("CARGO_PKG_VERSION"));
    println!("REPL is not implemented yet");
    println!("type 'exit' to quit");
    Ok(())
}
async fn cmd_dump_ast(input: &str) -> Result<()> {
    println!("Dump AST: {}", input);
    println!("Parser dump_ast is not implemented yet");
    Ok(())
}

#[allow(dead_code)]
fn escape_html(raw: &str) -> String {
    raw.chars()
        .map(|ch| match ch {
            '&' => "&amp;".to_string(),
            '<' => "&lt;".to_string(),
            '>' => "&gt;".to_string(),
            '"' => "&quot;".to_string(),
            '\'' => "&#39;".to_string(),
            _ => ch.to_string(),
        })
        .collect::<String>()
}

#[allow(dead_code)]
fn sanitize_doc_name(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push_str("module");
    }
    out
}

#[allow(dead_code)]
fn render_doc_index(module_id: &str, module_page_name: &str, fn_count: usize) -> String {
    let module_id = escape_html(module_id);
    let module_page_name = escape_html(module_page_name);
    format!(
        "<!doctype html>
<html lang=\"en\">
<head>
  <meta charset=\"utf-8\">
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
  <title>Sengoo API Docs</title>
  <style>
    body {{ margin: 0; font-family: \"Segoe UI\", sans-serif; background: #f4f7fb; color: #162335; }}
    main {{ max-width: 980px; margin: 0 auto; padding: 24px; }}
    h1 {{ margin: 0 0 12px; font-size: 28px; }}
    p {{ line-height: 1.6; color: #354960; }}
    .panel {{ background: #fff; border: 1px solid #d9e1ec; border-radius: 12px; padding: 16px; }}
    a {{ color: #0f4c81; text-decoration: none; font-weight: 600; }}
    a:hover {{ text-decoration: underline; }}
    code {{ background: #edf3fb; padding: 2px 6px; border-radius: 6px; }}
  </style>
</head>
<body>
  <main>
    <h1>Sengoo API Docs</h1>
    <p>Generated by <code>sgc doc</code>. Layout is rustdoc-like: index page + module page.</p>
    <div class=\"panel\">
      <p><strong>Module:</strong> <code>{}</code></p>
      <p><strong>Functions indexed:</strong> {}</p>
      <p><a href=\"{}\">Open module page</a></p>
      <p><a href=\"search-index.json\">Open search-index.json</a></p>
    </div>
  </main>
</body>
</html>
",
        module_id, fn_count, module_page_name
    )
}

#[allow(dead_code)]
fn render_doc_module(module_id: &str, signatures: &[FunctionSignatureInfo]) -> String {
    let mut items = String::new();
    if signatures.is_empty() {
        items.push_str(
            "<p>No function signatures discovered. Check source syntax or include function declarations.</p>",
        );
    } else {
        for sig in signatures {
            items.push_str("<article class=\"item\">");
            items.push_str(&format!(
                "<h3>{}</h3><pre>{}</pre>",
                escape_html(&sig.symbol),
                escape_html(&sig.signature)
            ));
            items.push_str("</article>");
        }
    }
    format!(
        "<!doctype html>
<html lang=\"en\">
<head>
  <meta charset=\"utf-8\">
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">
  <title>Module API - {}</title>
  <style>
    body {{ margin: 0; font-family: \"Segoe UI\", sans-serif; background: #f4f7fb; color: #162335; }}
    main {{ max-width: 980px; margin: 0 auto; padding: 24px; }}
    h1 {{ margin: 0 0 10px; font-size: 28px; }}
    p {{ line-height: 1.6; color: #354960; }}
    .item {{ background: #fff; border: 1px solid #d9e1ec; border-radius: 12px; padding: 14px; margin-bottom: 12px; }}
    .item h3 {{ margin: 0 0 8px; font-size: 18px; color: #113e68; }}
    pre {{ margin: 0; background: #edf3fb; border-radius: 8px; padding: 10px; overflow-x: auto; }}
    a {{ color: #0f4c81; text-decoration: none; font-weight: 600; }}
    a:hover {{ text-decoration: underline; }}
    code {{ background: #edf3fb; padding: 2px 6px; border-radius: 6px; }}
  </style>
</head>
<body>
  <main>
    <h1>Module API</h1>
    <p><strong>Module:</strong> <code>{}</code></p>
    <p><a href=\"index.html\">Back to index</a></p>
    {}
  </main>
</body>
</html>
",
        escape_html(module_id),
        escape_html(module_id),
        items
    )
}

#[allow(dead_code)]
async fn cmd_doc(input: &str, out_dir: &str) -> Result<()> {
    let input_path = Path::new(input);
    let module_id = canonical_or_lossy(input_path);
    let source = fs::read_to_string(input)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to read source {}: {}", input, e))?;
    let signatures = function_signatures_for_module(&module_id, &source);

    let out_dir = Path::new(out_dir);
    fs::create_dir_all(out_dir)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to create doc output directory {}: {}", out_dir.display(), e))?;

    let module_stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .map(sanitize_doc_name)
        .unwrap_or_else(|| "module".to_string());
    let module_page_name = format!("{}.html", module_stem);
    let module_page_path = out_dir.join(&module_page_name);
    let index_path = out_dir.join("index.html");
    let search_index_path = out_dir.join("search-index.json");

    let module_html = render_doc_module(&module_id, &signatures);
    fs::write(&module_page_path, module_html)
        .into_diagnostic()
        .map_err(|e| {
            miette::miette!(
                "failed to write module doc page {}: {}",
                module_page_path.display(),
                e
            )
        })?;

    let index_html = render_doc_index(&module_id, &module_page_name, signatures.len());
    fs::write(&index_path, index_html)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write doc index {}: {}", index_path.display(), e))?;

    let search_payload = serde_json::json!({
        "schema_version": 1,
        "module": module_id,
        "items": signatures
            .iter()
            .map(|sig| serde_json::json!({
                "symbol": sig.symbol,
                "signature": sig.signature
            }))
            .collect::<Vec<_>>()
    });
    let search_payload_bytes = serde_json::to_vec_pretty(&search_payload)
        .map_err(|e| miette::miette!("failed to encode search index json: {}", e))?;
    fs::write(&search_index_path, search_payload_bytes)
        .into_diagnostic()
        .map_err(|e| {
            miette::miette!(
                "failed to write search index {}: {}",
                search_index_path.display(),
                e
            )
        })?;

    println!("API docs index: {}", index_path.to_string_lossy());
    println!("API docs module page: {}", module_page_path.to_string_lossy());
    Ok(())
}

#[cfg(test)]
mod tests;


