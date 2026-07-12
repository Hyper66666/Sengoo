use miette::{IntoDiagnostic, Result};
use sengoo_compiler::hir::HIRItem;
use sengoo_compiler::mir::MirFunction;
use sengoo_compiler::{
    collect_ffi_codegen_config, lower_ast, lower_ast_with_coverage, lower_hir_with_options,
    AssertCallsiteContext, Codegen, CoverageContext, DebugInfoConfig, FfiCodegenConfig,
    IntegerOverflowMode, MirLowerOptions, MirOptLevel, Parser, TargetPointerWidth, TypeChecker,
};
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fs;
use std::io::BufWriter;
use std::path::Path;
use std::sync::atomic::{AtomicI8, Ordering};
use std::time::Instant;

use crate::{resolve_frontend_memory_mode, FrontendMemoryMode};

mod ast_pruning;
mod hir_pruning;
mod mir_pruning;
mod source_pruning;

use ast_pruning::{
    prune_unreachable_ast_functions, should_filter_typecheck_function_bodies_in_default_mode,
    should_prune_unreachable_ast_in_default_mode,
};
use hir_pruning::prune_unreachable_hir_functions;
use mir_pruning::prune_unreachable_mir_functions;
use source_pruning::prune_unreachable_plain_source_functions;

const DEFAULT_HIR_PRUNE_MIN_FUNCTIONS: usize = 20_000;
const DEFAULT_TYPECK_FILTER_MIN_FUNCTIONS: usize = DEFAULT_HIR_PRUNE_MIN_FUNCTIONS;
const LARGE_PROJECT_MODE_AUTO: i8 = 0;
const LARGE_PROJECT_MODE_ENABLED: i8 = 1;
const LARGE_PROJECT_MODE_DISABLED: i8 = -1;
const CONTRACT_CHECKS_AUTO: i8 = 0;
const CONTRACT_CHECKS_ENABLED: i8 = 1;
const CONTRACT_CHECKS_DISABLED: i8 = -1;

static LARGE_PROJECT_MODE_OVERRIDE: AtomicI8 = AtomicI8::new(LARGE_PROJECT_MODE_AUTO);
static CONTRACT_CHECKS_OVERRIDE: AtomicI8 = AtomicI8::new(CONTRACT_CHECKS_AUTO);

fn encode_large_project_mode_override(value: Option<bool>) -> i8 {
    match value {
        Some(true) => LARGE_PROJECT_MODE_ENABLED,
        Some(false) => LARGE_PROJECT_MODE_DISABLED,
        None => LARGE_PROJECT_MODE_AUTO,
    }
}
fn decode_large_project_mode_override(value: i8) -> Option<bool> {
    match value {
        LARGE_PROJECT_MODE_ENABLED => Some(true),
        LARGE_PROJECT_MODE_DISABLED => Some(false),
        _ => None,
    }
}

pub(crate) fn set_large_project_mode_override(value: Option<bool>) -> Option<bool> {
    let previous = LARGE_PROJECT_MODE_OVERRIDE
        .swap(encode_large_project_mode_override(value), Ordering::Relaxed);
    decode_large_project_mode_override(previous)
}

fn encode_contract_checks_override(value: Option<bool>) -> i8 {
    match value {
        Some(true) => CONTRACT_CHECKS_ENABLED,
        Some(false) => CONTRACT_CHECKS_DISABLED,
        None => CONTRACT_CHECKS_AUTO,
    }
}

fn decode_contract_checks_override(value: i8) -> Option<bool> {
    match value {
        CONTRACT_CHECKS_ENABLED => Some(true),
        CONTRACT_CHECKS_DISABLED => Some(false),
        _ => None,
    }
}

pub(crate) fn set_contract_runtime_checks_override(value: Option<bool>) -> Option<bool> {
    let previous =
        CONTRACT_CHECKS_OVERRIDE.swap(encode_contract_checks_override(value), Ordering::Relaxed);
    decode_contract_checks_override(previous)
}

fn parse_large_project_mode_env(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" | "enable" | "enabled" => Some(true),
        "0" | "false" | "off" | "no" | "disable" | "disabled" => Some(false),
        _ => None,
    }
}

fn parse_contract_checks_env(raw: &str) -> Option<Option<bool>> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" | "enable" | "enabled" => Some(Some(true)),
        "0" | "false" | "off" | "no" | "disable" | "disabled" => Some(Some(false)),
        "auto" => Some(None),
        _ => None,
    }
}

fn contract_runtime_checks_enabled(opt_level: u8) -> bool {
    if let Some(override_mode) =
        decode_contract_checks_override(CONTRACT_CHECKS_OVERRIDE.load(Ordering::Relaxed))
    {
        return override_mode;
    }

    if let Some(parsed) = std::env::var("SENGOO_CONTRACT_CHECKS")
        .ok()
        .and_then(|raw| parse_contract_checks_env(&raw))
    {
        return parsed.unwrap_or(opt_level <= 1);
    }

    false
}

fn integer_overflow_mode(opt_level: u8) -> IntegerOverflowMode {
    if opt_level <= 1 {
        IntegerOverflowMode::DebugChecked
    } else {
        IntegerOverflowMode::ReleaseWrapping
    }
}

fn large_project_optimization_enabled() -> bool {
    if let Some(override_mode) =
        decode_large_project_mode_override(LARGE_PROJECT_MODE_OVERRIDE.load(Ordering::Relaxed))
    {
        return override_mode;
    }

    std::env::var("SENGOO_LARGE_PROJECT_MODE")
        .ok()
        .and_then(|raw| parse_large_project_mode_env(&raw))
        .unwrap_or(true)
}

fn hir_prune_min_functions() -> usize {
    match std::env::var("SENGOO_HIR_PRUNE_MIN_FUNCTIONS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
    {
        Some(0) => usize::MAX,
        Some(value) => value,
        None => DEFAULT_HIR_PRUNE_MIN_FUNCTIONS,
    }
}

fn typeck_filter_min_functions() -> usize {
    match std::env::var("SENGOO_TYPECK_FILTER_MIN_FUNCTIONS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
    {
        Some(0) => usize::MAX,
        Some(value) => value,
        None => DEFAULT_TYPECK_FILTER_MIN_FUNCTIONS,
    }
}

fn hir_function_count(items: &[HIRItem]) -> usize {
    items
        .iter()
        .filter(|item| matches!(item, HIRItem::Function(_)))
        .count()
}

fn should_prune_unreachable_hir_in_default_mode(items: &[HIRItem]) -> bool {
    if !large_project_optimization_enabled() {
        return false;
    }
    hir_function_count(items) >= hir_prune_min_functions()
}

#[cfg(test)]
pub(crate) fn compile_source(source: &str, opt_level: u8) -> std::result::Result<String, String> {
    compile_source_with_phase_timings(source, opt_level)
        .map(|(llvm_ir, _)| llvm_ir)
        .map_err(|e| e.to_string())
}

#[cfg(test)]
pub(crate) fn compile_source_with_phase_timings(
    source: &str,
    opt_level: u8,
) -> Result<(String, BTreeMap<String, f64>)> {
    let resolved_memory_mode = resolve_frontend_memory_mode(source.len());
    let (mir_fns, ffi_codegen, mut phases) = compile_frontend_to_mir_with_phase_timings(
        source,
        opt_level,
        matches!(resolved_memory_mode, FrontendMemoryMode::LowMemory),
        None,
        None,
        TargetPointerWidth::host().bits(),
    )?;

    let codegen_start = Instant::now();
    let mut codegen = Codegen::with_ffi_target_debug_and_overflow(
        ffi_codegen,
        None,
        DebugInfoConfig::disabled(),
        integer_overflow_mode(opt_level),
    );
    let llvm_ir = match resolved_memory_mode {
        FrontendMemoryMode::Stream | FrontendMemoryMode::LowMemory => {
            let mut out = Vec::new();
            codegen
                .codegen_to_writer(&mir_fns, &mut out)
                .map_err(|e| miette::miette!("codegen failed: {}", e))?;
            String::from_utf8(out).map_err(|e| miette::miette!("invalid utf-8 LLVM IR: {}", e))?
        }
        FrontendMemoryMode::Legacy | FrontendMemoryMode::Auto => codegen
            .codegen(&mir_fns)
            .map_err(|e| miette::miette!("codegen failed: {}", e))?,
    };
    phases.insert(
        "codegen".to_string(),
        codegen_start.elapsed().as_secs_f64() * 1000.0,
    );
    phases.insert("link".to_string(), 0.0);

    Ok((llvm_ir, phases))
}

pub(crate) fn user_source_base_offset(expanded_source: &str, root_source: &str) -> u32 {
    expanded_source.rfind(root_source).unwrap_or(0) as u32
}

fn compile_frontend_to_mir_with_phase_timings<S: AsRef<str>>(
    source: S,
    opt_level: u8,
    low_memory_mode: bool,
    assert_callsite: Option<AssertCallsiteContext>,
    coverage: Option<CoverageContext>,
    target_pointer_width: u8,
) -> Result<(Vec<MirFunction>, FfiCodegenConfig, BTreeMap<String, f64>)> {
    let mut phases = BTreeMap::new();
    let coverage_enabled = coverage.is_some();

    let (
        mut mir_fns,
        ffi_codegen,
        parse_ms,
        typeck_ms,
        mir_ms,
        ast_prune_ms,
        ast_pruned_count,
        ast_prune_applied,
        source_prune_ms,
        source_pruned_count,
        source_prune_applied,
        hir_prune_ms,
        hir_pruned_count,
        hir_prune_applied,
    ) = {
        let source_ref = source.as_ref();
        let mut source_prune_ms = 0.0;
        let mut source_pruned_count = 0usize;
        let mut source_prune_applied = false;
        let mut pruned_source = None;
        if !coverage_enabled && large_project_optimization_enabled() {
            let source_prune_start = Instant::now();
            if let Some(result) =
                prune_unreachable_plain_source_functions(source_ref, typeck_filter_min_functions())
            {
                source_prune_ms = source_prune_start.elapsed().as_secs_f64() * 1000.0;
                source_pruned_count = result.removed_functions;
                source_prune_applied = true;
                pruned_source = Some(result.source);
            } else {
                source_prune_ms = source_prune_start.elapsed().as_secs_f64() * 1000.0;
            }
        }
        let parse_input = pruned_source
            .as_deref()
            .map(Cow::Borrowed)
            .unwrap_or_else(|| Cow::Borrowed(source_ref));
        let parse_start = Instant::now();
        let mut program = Some(
            Parser::parse_with_pointer_width(parse_input.as_ref(), target_pointer_width)
                .map_err(|e| miette::miette!("parse failed: {}", e))?,
        );
        let parse_ms = parse_start.elapsed().as_secs_f64() * 1000.0;
        drop(parse_input);
        drop(source);

        let mut ast_prune_ms = 0.0;
        let mut ast_pruned_count = 0usize;
        let mut ast_prune_applied = false;
        if low_memory_mode && !coverage_enabled {
            let ast_prune_start = Instant::now();
            ast_pruned_count =
                prune_unreachable_ast_functions(program.as_mut().expect("program present"));
            ast_prune_ms = ast_prune_start.elapsed().as_secs_f64() * 1000.0;
            ast_prune_applied = true;
        }

        if !coverage_enabled
            && !low_memory_mode
            && should_filter_typecheck_function_bodies_in_default_mode(
                program.as_ref().expect("program present before typecheck"),
            )
        {
            let ast_prune_start = Instant::now();
            ast_pruned_count =
                prune_unreachable_ast_functions(program.as_mut().expect("program present"));
            ast_prune_ms += ast_prune_start.elapsed().as_secs_f64() * 1000.0;
            ast_prune_applied = ast_prune_applied || ast_pruned_count > 0;
        }

        let typeck_start = Instant::now();
        let mut checker = TypeChecker::new();
        checker
            .check_program(program.as_ref().expect("program present during typeck"))
            .map_err(|e| miette::miette!("typecheck failed: {}", e))?;
        let async_functions = checker.async_function_names().clone();
        let mut type_env = Some(checker.into_env());
        let typeck_ms = typeck_start.elapsed().as_secs_f64() * 1000.0;
        if !coverage_enabled
            && !ast_prune_applied
            && !low_memory_mode
            && should_prune_unreachable_ast_in_default_mode(
                program.as_ref().expect("program present after typeck"),
            )
        {
            let ast_prune_start = Instant::now();
            ast_pruned_count =
                prune_unreachable_ast_functions(program.as_mut().expect("program present"));
            ast_prune_ms = ast_prune_start.elapsed().as_secs_f64() * 1000.0;
            ast_prune_applied = true;
        }

        let mir_start = Instant::now();
        let runtime_contract_checks = contract_runtime_checks_enabled(opt_level);
        let hir_lower_start = Instant::now();
        let program_ref = program.as_ref().expect("program present during lowering");
        let type_env_ref = type_env.as_ref().expect("type env present during lowering");
        let mut hir_module = if coverage_enabled {
            lower_ast_with_coverage(program_ref, type_env_ref)
        } else {
            lower_ast(program_ref, type_env_ref)
        };
        let hir_lower_ms = hir_lower_start.elapsed().as_secs_f64() * 1000.0;
        let mut hir_prune_ms = 0.0;
        let mut hir_pruned_count = 0usize;
        let mut hir_prune_applied = false;
        if low_memory_mode && !coverage_enabled {
            let hir_prune_start = Instant::now();
            hir_pruned_count = prune_unreachable_hir_functions(&mut hir_module.items);
            hir_prune_ms = hir_prune_start.elapsed().as_secs_f64() * 1000.0;
            hir_prune_applied = true;
        } else if !coverage_enabled
            && should_prune_unreachable_hir_in_default_mode(&hir_module.items)
        {
            // Keep full typechecking, but skip lowering/codegen of cold unreachable functions.
            let hir_prune_start = Instant::now();
            hir_pruned_count = prune_unreachable_hir_functions(&mut hir_module.items);
            hir_prune_ms = hir_prune_start.elapsed().as_secs_f64() * 1000.0;
            hir_prune_applied = true;
        }
        if low_memory_mode {
            drop(type_env.take());
            drop(program.take());
        }
        let ffi_codegen = collect_ffi_codegen_config(&hir_module);
        let mir_lower_start = Instant::now();
        let mut mir_options =
            MirLowerOptions::new(runtime_contract_checks, true, async_functions.clone())
                .with_target_pointer_width(target_pointer_width);
        if let Some(context) = assert_callsite {
            mir_options = mir_options.with_assert_callsite_context(context);
        }
        if let Some(context) = coverage {
            mir_options = mir_options.with_coverage_context(context);
        }
        let mut mir_fns = lower_hir_with_options(&hir_module.items, mir_options)
            .map_err(|e| miette::miette!("{}", e))?;

        // Expand async functions into frame-backed __start/__poll/__result helpers
        if !async_functions.is_empty() {
            let async_helpers =
                sengoo_compiler::mir::async_lowering::expand_async_functions(&mut mir_fns)
                    .map_err(|e| miette::miette!("{}", e))?;
            mir_fns.extend(async_helpers);
        }
        let mir_lower_ms = mir_lower_start.elapsed().as_secs_f64() * 1000.0;
        drop(hir_module);
        if !low_memory_mode {
            drop(type_env.take());
            drop(program.take());
        }
        // Prune unreachable functions before MIR optimization to avoid spending
        // optimization work on dead code in large single-file workloads.
        prune_unreachable_mir_functions(&mut mir_fns);
        let effective_opt_level = if low_memory_mode {
            opt_level.min(1)
        } else {
            opt_level
        };
        let mir_opt_level = MirOptLevel::from_u8(effective_opt_level)
            .ok_or_else(|| miette::miette!("invalid optimization level: {}", opt_level))?;
        let pipeline = sengoo_compiler::mir::opt::pipeline_for_level(mir_opt_level);
        let mir_opt_start = Instant::now();
        pipeline.run(&mut mir_fns);
        let mir_opt_ms = mir_opt_start.elapsed().as_secs_f64() * 1000.0;
        let mir_ms = mir_start.elapsed().as_secs_f64() * 1000.0;
        phases.insert(
            "contract_runtime_checks".to_string(),
            if runtime_contract_checks { 1.0 } else { 0.0 },
        );
        // Sub-phase split of the `mir` bucket, for frontend hotspot profiling.
        phases.insert("hir_lower".to_string(), hir_lower_ms);
        phases.insert("mir_lower".to_string(), mir_lower_ms);
        phases.insert("mir_opt".to_string(), mir_opt_ms);

        (
            mir_fns,
            ffi_codegen,
            parse_ms,
            typeck_ms,
            mir_ms,
            ast_prune_ms,
            ast_pruned_count,
            ast_prune_applied,
            source_prune_ms,
            source_pruned_count,
            source_prune_applied,
            hir_prune_ms,
            hir_pruned_count,
            hir_prune_applied,
        )
    };

    phases.insert("parse".to_string(), parse_ms);
    if source_prune_applied {
        phases.insert("source_prune".to_string(), source_prune_ms);
        phases.insert(
            "source_prune_removed".to_string(),
            source_pruned_count as f64,
        );
    }
    phases.insert("typeck".to_string(), typeck_ms);
    if ast_prune_applied {
        phases.insert("ast_prune".to_string(), ast_prune_ms);
        phases.insert("ast_prune_removed".to_string(), ast_pruned_count as f64);
    }
    if low_memory_mode || hir_prune_applied {
        phases.insert("hir_prune".to_string(), hir_prune_ms);
        phases.insert("hir_prune_removed".to_string(), hir_pruned_count as f64);
    }
    phases.insert("mir".to_string(), mir_ms);

    let prune_start = Instant::now();
    prune_unreachable_mir_functions(&mut mir_fns);
    phases.insert(
        "mir_prune".to_string(),
        prune_start.elapsed().as_secs_f64() * 1000.0,
    );

    Ok((mir_fns, ffi_codegen, phases))
}

pub(crate) fn compile_source_to_mir_bundle_for_fast_jit(
    source: &str,
    opt_level: u8,
) -> Result<(Vec<MirFunction>, FfiCodegenConfig)> {
    let (mir_fns, ffi_codegen, _phases) = compile_frontend_to_mir_with_phase_timings(
        source,
        opt_level,
        false,
        None,
        None,
        TargetPointerWidth::host().bits(),
    )?;
    Ok((mir_fns, ffi_codegen))
}

pub(crate) fn compile_source_to_llvm_file_with_phase_timings(
    source: &str,
    opt_level: u8,
    llvm_path: &Path,
) -> Result<(BTreeMap<String, f64>, FrontendMemoryMode)> {
    compile_source_to_llvm_file_with_phase_timings_with_mode(
        source, opt_level, llvm_path, None, None, None, None,
    )
}

pub(crate) fn compile_source_to_llvm_file_with_phase_timings_with_mode<S: AsRef<str>>(
    source: S,
    opt_level: u8,
    llvm_path: &Path,
    forced_memory_mode: Option<FrontendMemoryMode>,
    assert_callsite: Option<AssertCallsiteContext>,
    target_triple: Option<&str>,
    debug_info: Option<DebugInfoConfig>,
) -> Result<(BTreeMap<String, f64>, FrontendMemoryMode)> {
    compile_source_to_llvm_file_with_phase_timings_with_mode_and_coverage(
        source,
        opt_level,
        llvm_path,
        forced_memory_mode,
        assert_callsite,
        target_triple,
        debug_info,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn compile_source_to_llvm_file_with_phase_timings_with_mode_and_coverage<
    S: AsRef<str>,
>(
    source: S,
    opt_level: u8,
    llvm_path: &Path,
    forced_memory_mode: Option<FrontendMemoryMode>,
    assert_callsite: Option<AssertCallsiteContext>,
    target_triple: Option<&str>,
    debug_info: Option<DebugInfoConfig>,
    coverage: Option<CoverageContext>,
) -> Result<(BTreeMap<String, f64>, FrontendMemoryMode)> {
    let resolved_memory_mode =
        forced_memory_mode.unwrap_or_else(|| resolve_frontend_memory_mode(source.as_ref().len()));
    let (mir_fns, ffi_codegen, mut phases) = compile_frontend_to_mir_with_phase_timings(
        source,
        opt_level,
        matches!(resolved_memory_mode, FrontendMemoryMode::LowMemory),
        assert_callsite,
        coverage,
        target_triple
            .and_then(TargetPointerWidth::from_target_triple)
            .unwrap_or_else(TargetPointerWidth::host)
            .bits(),
    )?;

    let codegen_target = target_triple.map(str::to_string);
    let codegen_start = Instant::now();
    let mut effective_mode = resolved_memory_mode;
    let stream_result = if matches!(
        resolved_memory_mode,
        FrontendMemoryMode::Stream | FrontendMemoryMode::LowMemory
    ) {
        let file = fs::File::create(llvm_path).into_diagnostic().map_err(|e| {
            miette::miette!(
                "failed to create LLVM IR output {}: {}",
                llvm_path.display(),
                e
            )
        })?;
        let mut writer = BufWriter::new(file);
        let mut codegen = Codegen::with_ffi_target_debug_and_overflow(
            ffi_codegen.clone(),
            codegen_target.clone(),
            debug_info.clone().unwrap_or_else(DebugInfoConfig::disabled),
            integer_overflow_mode(opt_level),
        );
        codegen
            .codegen_to_writer(&mir_fns, &mut writer)
            .map_err(|e| miette::miette!("codegen failed: {}", e))
    } else {
        Ok(())
    };

    if let Err(_err) = stream_result {
        effective_mode = FrontendMemoryMode::Legacy;
        let mut codegen = Codegen::with_ffi_target_debug_and_overflow(
            ffi_codegen.clone(),
            codegen_target.clone(),
            debug_info.clone().unwrap_or_else(DebugInfoConfig::disabled),
            integer_overflow_mode(opt_level),
        );
        let llvm_ir = codegen
            .codegen(&mir_fns)
            .map_err(|e| miette::miette!("codegen failed: {}", e))?;
        fs::write(llvm_path, llvm_ir)
            .into_diagnostic()
            .map_err(|e| miette::miette!("failed to write LLVM IR: {}", e))?;
    } else if matches!(resolved_memory_mode, FrontendMemoryMode::Legacy) {
        let mut codegen = Codegen::with_ffi_target_debug_and_overflow(
            ffi_codegen,
            codegen_target,
            debug_info.unwrap_or_else(DebugInfoConfig::disabled),
            integer_overflow_mode(opt_level),
        );
        let llvm_ir = codegen
            .codegen(&mir_fns)
            .map_err(|e| miette::miette!("codegen failed: {}", e))?;
        fs::write(llvm_path, llvm_ir)
            .into_diagnostic()
            .map_err(|e| miette::miette!("failed to write LLVM IR: {}", e))?;
    }

    phases.insert(
        "codegen".to_string(),
        codegen_start.elapsed().as_secs_f64() * 1000.0,
    );
    phases.insert("link".to_string(), 0.0);
    Ok((phases, effective_mode))
}

/// Whether the env-gated per-phase compile timing breakdown is enabled.
///
/// Follows the same opt-in spelling as the other `SENGOO_*` pipeline toggles
/// (`SENGOO_LARGE_PROJECT_MODE`, `SENGOO_CONTRACT_CHECKS`).
fn phase_timings_enabled() -> bool {
    std::env::var("SENGOO_PHASE_TIMINGS")
        .ok()
        .map(|raw| {
            matches!(
                raw.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "on" | "yes" | "enable" | "enabled"
            )
        })
        .unwrap_or(false)
}

/// Print the per-phase frontend/codegen timing breakdown to stderr when
/// `SENGOO_PHASE_TIMINGS` is set.
///
/// The pipeline already records `parse` / `typeck` / `mir` (and prune) phase
/// timings but the command layer previously discarded them. Surfacing the
/// split is a measurement aid for frontend optimization work; it never changes
/// compilation behavior and writes to stderr so stdout build/run output is
/// untouched. Note: for `sgc build`, link is measured separately downstream, so
/// the `link` entry here is `0` and the reported percentage is over frontend +
/// codegen only.
pub(crate) fn maybe_print_phase_timings(phases: &BTreeMap<String, f64>) {
    if !phase_timings_enabled() {
        return;
    }

    // Time-valued phases, in pipeline execution order.
    const TIME_PHASES: [&str; 9] = [
        "source_prune",
        "parse",
        "typeck",
        "ast_prune",
        "hir_prune",
        "mir",
        "mir_prune",
        "codegen",
        "link",
    ];
    // Frontend = everything before object codegen.
    const FRONTEND_PHASES: [&str; 7] = [
        "source_prune",
        "parse",
        "typeck",
        "ast_prune",
        "hir_prune",
        "mir",
        "mir_prune",
    ];

    let get = |key: &str| phases.get(key).copied().unwrap_or(0.0);
    let frontend: f64 = FRONTEND_PHASES.iter().map(|key| get(key)).sum();
    let measured: f64 = TIME_PHASES.iter().map(|key| get(key)).sum();

    let parts: Vec<String> = TIME_PHASES
        .iter()
        .filter_map(|key| {
            phases
                .get(*key)
                .map(|value| format!("{}={:.3}ms", key, value))
        })
        .collect();

    let frontend_pct = if measured > 0.0 {
        frontend / measured * 100.0
    } else {
        0.0
    };

    eprintln!(
        "[sgc phase-timings] {} | frontend={:.3}ms measured={:.3}ms (frontend {:.1}% of measured)",
        parts.join(" "),
        frontend,
        measured,
        frontend_pct
    );

    // Sub-split of the `mir` bucket (hir lowering vs mir lowering vs mir opt),
    // shown separately so it is not double-counted in the totals above.
    const MIR_SUBPHASES: [&str; 3] = ["hir_lower", "mir_lower", "mir_opt"];
    let sub_parts: Vec<String> = MIR_SUBPHASES
        .iter()
        .filter_map(|key| {
            phases
                .get(*key)
                .map(|value| format!("{}={:.3}ms", key, value))
        })
        .collect();
    if !sub_parts.is_empty() {
        eprintln!("[sgc phase-timings]   mir split: {}", sub_parts.join(" "));
    }
}

/// Best-effort peak resident set size (high-water mark) of the current process,
/// in bytes; `None` when the platform cannot report it without extra crates.
///
/// This reads the OS-maintained high-water mark directly — Windows
/// `PeakWorkingSetSize` and Linux `VmHWM` — so it is exact and needs no
/// sampling loop, unlike the external Python harness that polls current RSS.
/// It only observes process memory and never changes any compilation result;
/// the compile benchmark uses it to record compiler peak memory (the
/// `frontend-compile-perf` Phase 3 peak-RSS target) next to per-phase timings,
/// natively and dependency-free. Semantics match the harness's
/// `PeakWorkingSetSize` (Windows) / RSS-from-`/proc` (Linux) accounting.
#[cfg(target_os = "linux")]
pub(crate) fn process_peak_rss_bytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmHWM:") {
            // Format: `VmHWM:\t   12345 kB`.
            let kib: u64 = rest.split_whitespace().next()?.parse().ok()?;
            return Some(kib.saturating_mul(1024));
        }
    }
    None
}

/// See the platform-neutral doc on the Linux variant above.
#[cfg(windows)]
pub(crate) fn process_peak_rss_bytes() -> Option<u64> {
    // `PROCESS_MEMORY_COUNTERS` layout per the Win32 API; only
    // `PeakWorkingSetSize` is read. `GetCurrentProcess` and
    // `K32GetProcessMemoryInfo` are exported by kernel32 (already linked by
    // std), so this stays dependency-free.
    #[repr(C)]
    struct ProcessMemoryCounters {
        cb: u32,
        page_fault_count: u32,
        peak_working_set_size: usize,
        working_set_size: usize,
        quota_peak_paged_pool_usage: usize,
        quota_paged_pool_usage: usize,
        quota_peak_non_paged_pool_usage: usize,
        quota_non_paged_pool_usage: usize,
        pagefile_usage: usize,
        peak_pagefile_usage: usize,
    }

    extern "system" {
        fn GetCurrentProcess() -> *mut core::ffi::c_void;
        fn K32GetProcessMemoryInfo(
            process: *mut core::ffi::c_void,
            counters: *mut ProcessMemoryCounters,
            cb: u32,
        ) -> i32;
    }

    // SAFETY: `counters` is a correctly sized, zero-initialized POD buffer whose
    // `cb` is set to its own size, exactly as the API requires; the current
    // process pseudo-handle is always valid. We only read scalar fields back.
    unsafe {
        let mut counters: ProcessMemoryCounters = std::mem::zeroed();
        counters.cb = std::mem::size_of::<ProcessMemoryCounters>() as u32;
        if K32GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) != 0 {
            Some(counters.peak_working_set_size as u64)
        } else {
            None
        }
    }
}

/// See the platform-neutral doc on the Linux variant above.
#[cfg(not(any(target_os = "linux", windows)))]
pub(crate) fn process_peak_rss_bytes() -> Option<u64> {
    None
}

#[cfg(test)]
mod tests {
    use super::{prune_unreachable_ast_functions, prune_unreachable_hir_functions};
    use sengoo_compiler::ast::DeclKind as AstDeclKind;
    use sengoo_compiler::ast::Program as AstProgram;
    use sengoo_compiler::hir::HIRItem;
    use sengoo_compiler::{lower_ast, Parser, TypeChecker};
    use std::collections::HashSet;

    fn function_names(items: &[HIRItem]) -> HashSet<String> {
        items
            .iter()
            .filter_map(|item| match item {
                HIRItem::Function(fn_item) => Some(fn_item.name.clone()),
                _ => None,
            })
            .collect()
    }

    fn lower_source_to_hir_items(source: &str) -> Vec<HIRItem> {
        let program = Parser::parse(source).expect("parse should succeed");
        let mut checker = TypeChecker::new();
        checker
            .check_program(&program)
            .expect("typecheck should succeed");
        let env = checker.into_env();
        let module = lower_ast(&program, &env);
        module.items
    }

    fn lower_source_to_ast(source: &str) -> AstProgram {
        Parser::parse(source).expect("parse should succeed")
    }

    #[test]
    fn default_typeck_filter_threshold_covers_100k_scale_bucket() {
        // `advanced_pipeline_bench.py::make_scale_source_sengoo(100000)` emits
        // max(50, loc / 4) functions. Keep the default filter threshold low
        // enough that the 100k production gate bucket avoids full cold-body
        // typechecking for unreachable functions.
        assert_eq!(
            super::DEFAULT_TYPECK_FILTER_MIN_FUNCTIONS,
            super::DEFAULT_HIR_PRUNE_MIN_FUNCTIONS
        );
        const {
            assert!(super::DEFAULT_TYPECK_FILTER_MIN_FUNCTIONS <= 25_000);
        }
    }

    fn ast_function_names(program: &AstProgram) -> HashSet<String> {
        program
            .decls
            .iter()
            .filter_map(|decl| match &decl.kind {
                AstDeclKind::Function(fn_decl) => Some(fn_decl.name.name.clone()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn prune_unreachable_hir_functions_keeps_only_main_reachable_graph() {
        let source = r#"
def keep(x: i64) -> i64 {
    x + 1
}

def dead(x: i64) -> i64 {
    x + 2
}

def main() -> i64 {
    keep(1)
}
"#;
        let mut items = lower_source_to_hir_items(source);

        let removed = prune_unreachable_hir_functions(&mut items);
        let names = function_names(&items);

        assert_eq!(removed, 1);
        assert!(names.contains("main"));
        assert!(names.contains("keep"));
        assert!(!names.contains("dead"));
    }

    #[test]
    fn prune_unreachable_hir_functions_keeps_contract_reachable_helpers() {
        let source = r#"
def helper() -> bool {
    true
}

def dead() -> bool {
    true
}

def main() -> i64
requires helper()
{
    1
}
"#;
        let mut items = lower_source_to_hir_items(source);

        let removed = prune_unreachable_hir_functions(&mut items);
        let names = function_names(&items);

        assert_eq!(removed, 1);
        assert!(names.contains("main"));
        assert!(names.contains("helper"));
        assert!(!names.contains("dead"));
    }

    #[test]
    fn prune_unreachable_hir_functions_skips_when_main_missing() {
        let source = r#"
def a(x: i64) -> i64 {
    x + 1
}

def b(x: i64) -> i64 {
    a(x)
}
"#;
        let mut items = lower_source_to_hir_items(source);
        let before = function_names(&items);

        let removed = prune_unreachable_hir_functions(&mut items);
        let after = function_names(&items);

        assert_eq!(removed, 0);
        assert_eq!(after, before);
    }

    #[test]
    fn prune_unreachable_ast_functions_keeps_only_main_reachable_graph() {
        let source = r#"
def keep(x: i64) -> i64 {
    x + 1
}

def dead(x: i64) -> i64 {
    x + 2
}

def main() -> i64 {
    keep(1)
}
"#;
        let mut program = lower_source_to_ast(source);

        let removed = prune_unreachable_ast_functions(&mut program);
        let names = ast_function_names(&program);

        assert_eq!(removed, 1);
        assert!(names.contains("main"));
        assert!(names.contains("keep"));
        assert!(!names.contains("dead"));
    }

    #[test]
    fn prune_unreachable_ast_functions_keeps_contract_reachable_helpers() {
        let source = r#"
def helper() -> bool {
    true
}

def dead() -> bool {
    true
}

def main() -> i64
requires helper()
{
    1
}
"#;
        let mut program = lower_source_to_ast(source);

        let removed = prune_unreachable_ast_functions(&mut program);
        let names = ast_function_names(&program);

        assert_eq!(removed, 1);
        assert!(names.contains("main"));
        assert!(names.contains("helper"));
        assert!(!names.contains("dead"));
    }

    #[test]
    fn prune_unreachable_ast_functions_skips_when_main_missing() {
        let source = r#"
def a(x: i64) -> i64 {
    x + 1
}

def b(x: i64) -> i64 {
    a(x)
}
"#;
        let mut program = lower_source_to_ast(source);
        let before = ast_function_names(&program);

        let removed = prune_unreachable_ast_functions(&mut program);
        let after = ast_function_names(&program);

        assert_eq!(removed, 0);
        assert_eq!(after, before);
    }

    #[cfg(any(target_os = "linux", windows))]
    #[test]
    fn process_peak_rss_is_reported_on_supported_platforms() {
        // On the platforms this project builds/tests on (Windows, Linux), the
        // OS high-water mark is always queryable and must be a positive value.
        let rss =
            super::process_peak_rss_bytes().expect("peak RSS should be reported on Windows/Linux");
        assert!(
            rss > 0,
            "peak RSS should be a positive byte count, got {rss}"
        );
        // Sanity bound: a live test process uses at least a few hundred KiB and
        // far less than 1 TiB; this guards against a unit/field-offset mistake.
        assert!(
            rss > 64 * 1024 && rss < (1u64 << 40),
            "peak RSS {rss} bytes is implausible (unit or struct-layout bug?)"
        );
    }
}
