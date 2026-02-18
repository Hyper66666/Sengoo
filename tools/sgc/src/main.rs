//! Sengoo CLI compiler (`sgc`).

use clap::ValueEnum;
use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::sync::atomic::AtomicI8;
use std::sync::Arc;
use tokio::time::Duration;
use tracing_subscriber::{fmt, EnvFilter};

mod bench;
mod cache;
mod cli;
mod commands;
mod daemon;
mod fingerprint;
mod frontend_helpers;
mod frontend_snapshot;
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
    function_signatures_for_module, interface_fingerprint_from_program,
};
pub(crate) use module_graph::{collect_module_sources_with_edges, module_dependency_levels};
pub(crate) use native_toolchain::{
    artifact_exists, build_artifact_exists, compile_ir_to_object, compile_native_binary,
    default_build_output_path_for_case, ensure_runtime_object, link_native_binary_from_objects,
    linker_mode_from_env, recover_native_output_from_cached_artifacts, run_native_binary,
    run_with_lli,
};
#[cfg(test)]
pub(crate) use native_toolchain::{derive_cached_native_recovery_plan, parse_linker_mode};
#[cfg(test)]
pub(crate) use pipeline::compile_source_with_phase_timings;
pub(crate) use pipeline::{
    compile_source, compile_source_to_llvm_file_with_phase_timings,
    compile_source_to_llvm_file_with_phase_timings_with_mode,
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

fn default_build_graph_schema_version() -> u32 {
    BUILD_GRAPH_SCHEMA_VERSION
}

fn default_build_cache_schema_version() -> u32 {
    1
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

    match compile_source(&source, 0) {
        Ok(_) => {
            println!("Type check passed");
            Ok(())
        }
        Err(e) => {
            eprintln!("Compilation error:");
            eprintln!("{}", e);
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

#[cfg(test)]
mod tests;
