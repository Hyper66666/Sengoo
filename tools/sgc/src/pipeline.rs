use miette::{IntoDiagnostic, Result};
use sengoo_compiler::hir::HIRItem;
use sengoo_compiler::mir::MirFunction;
use sengoo_compiler::{
    lower_ast, lower_hir_with_options, Codegen, MirLowerOptions, MirOptLevel, Parser, TypeChecker,
};
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

use ast_pruning::{
    prune_ast_functions_by_name_set, prune_unreachable_ast_functions, reachable_ast_function_names,
    should_filter_typecheck_function_bodies_in_default_mode,
    should_prune_unreachable_ast_in_default_mode,
};
use hir_pruning::prune_unreachable_hir_functions;
use mir_pruning::prune_unreachable_mir_functions;

const DEFAULT_HIR_PRUNE_MIN_FUNCTIONS: usize = 20_000;
const DEFAULT_TYPECK_FILTER_MIN_FUNCTIONS: usize = 120_000;
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
    let (mir_fns, mut phases) = compile_frontend_to_mir_with_phase_timings(
        source,
        opt_level,
        matches!(resolved_memory_mode, FrontendMemoryMode::LowMemory),
    )?;

    let codegen_start = Instant::now();
    let mut codegen = Codegen::new();
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

fn compile_frontend_to_mir_with_phase_timings(
    source: &str,
    opt_level: u8,
    low_memory_mode: bool,
) -> Result<(Vec<MirFunction>, BTreeMap<String, f64>)> {
    let mut phases = BTreeMap::new();

    let (
        mut mir_fns,
        parse_ms,
        typeck_ms,
        mir_ms,
        ast_prune_ms,
        ast_pruned_count,
        ast_prune_applied,
        hir_prune_ms,
        hir_pruned_count,
        hir_prune_applied,
    ) = {
        let parse_start = Instant::now();
        let mut program =
            Some(Parser::parse(source).map_err(|e| miette::miette!("parse failed: {}", e))?);
        let parse_ms = parse_start.elapsed().as_secs_f64() * 1000.0;

        let mut ast_prune_ms = 0.0;
        let mut ast_pruned_count = 0usize;
        let mut ast_prune_applied = false;
        if low_memory_mode {
            let ast_prune_start = Instant::now();
            ast_pruned_count =
                prune_unreachable_ast_functions(program.as_mut().expect("program present"));
            ast_prune_ms = ast_prune_start.elapsed().as_secs_f64() * 1000.0;
            ast_prune_applied = true;
        }

        let reachable_typecheck_bodies = if !low_memory_mode
            && should_filter_typecheck_function_bodies_in_default_mode(
                program.as_ref().expect("program present before typecheck"),
            ) {
            reachable_ast_function_names(
                program.as_ref().expect("program present before typecheck"),
            )
        } else {
            None
        };

        if let Some(reachable) = reachable_typecheck_bodies.as_ref() {
            let ast_prune_start = Instant::now();
            let removed = prune_ast_functions_by_name_set(
                program.as_mut().expect("program present"),
                reachable,
            );
            ast_prune_ms += ast_prune_start.elapsed().as_secs_f64() * 1000.0;
            ast_pruned_count += removed;
            ast_prune_applied = ast_prune_applied || removed > 0;
        }

        let typeck_start = Instant::now();
        let mut checker = TypeChecker::new();
        checker
            .check_program(program.as_ref().expect("program present during typeck"))
            .map_err(|e| miette::miette!("typecheck failed: {}", e))?;
        let async_functions = checker.async_function_names().clone();
        let mut type_env = Some(checker.into_env());
        let typeck_ms = typeck_start.elapsed().as_secs_f64() * 1000.0;
        if !low_memory_mode
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
        let mut hir_module = lower_ast(
            program.as_ref().expect("program present during lowering"),
            type_env.as_ref().expect("type env present during lowering"),
        );
        let mut hir_prune_ms = 0.0;
        let mut hir_pruned_count = 0usize;
        let mut hir_prune_applied = false;
        if low_memory_mode {
            let hir_prune_start = Instant::now();
            hir_pruned_count = prune_unreachable_hir_functions(&mut hir_module.items);
            hir_prune_ms = hir_prune_start.elapsed().as_secs_f64() * 1000.0;
            hir_prune_applied = true;
        } else if should_prune_unreachable_hir_in_default_mode(&hir_module.items) {
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
        let mut mir_fns = lower_hir_with_options(
            &hir_module.items,
            MirLowerOptions::new(runtime_contract_checks, true, async_functions.clone()),
        )
        .map_err(|e| miette::miette!("{}", e))?;

        // Expand async functions into frame-backed __start/__poll/__result helpers
        if !async_functions.is_empty() {
            let async_helpers =
                sengoo_compiler::mir::async_lowering::expand_async_functions(&mut mir_fns)
                    .map_err(|e| miette::miette!("{}", e))?;
            mir_fns.extend(async_helpers);
        }
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
        pipeline.run(&mut mir_fns);
        let mir_ms = mir_start.elapsed().as_secs_f64() * 1000.0;
        phases.insert(
            "contract_runtime_checks".to_string(),
            if runtime_contract_checks { 1.0 } else { 0.0 },
        );

        (
            mir_fns,
            parse_ms,
            typeck_ms,
            mir_ms,
            ast_prune_ms,
            ast_pruned_count,
            ast_prune_applied,
            hir_prune_ms,
            hir_pruned_count,
            hir_prune_applied,
        )
    };

    phases.insert("parse".to_string(), parse_ms);
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

    Ok((mir_fns, phases))
}

pub(crate) fn compile_source_to_llvm_file_with_phase_timings(
    source: &str,
    opt_level: u8,
    llvm_path: &Path,
) -> Result<(BTreeMap<String, f64>, FrontendMemoryMode)> {
    compile_source_to_llvm_file_with_phase_timings_with_mode(source, opt_level, llvm_path, None)
}

pub(crate) fn compile_source_to_llvm_file_with_phase_timings_with_mode(
    source: &str,
    opt_level: u8,
    llvm_path: &Path,
    forced_memory_mode: Option<FrontendMemoryMode>,
) -> Result<(BTreeMap<String, f64>, FrontendMemoryMode)> {
    let resolved_memory_mode =
        forced_memory_mode.unwrap_or_else(|| resolve_frontend_memory_mode(source.len()));
    let (mir_fns, mut phases) = compile_frontend_to_mir_with_phase_timings(
        source,
        opt_level,
        matches!(resolved_memory_mode, FrontendMemoryMode::LowMemory),
    )?;

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
        let mut codegen = Codegen::new();
        codegen
            .codegen_to_writer(&mir_fns, &mut writer)
            .map_err(|e| miette::miette!("codegen failed: {}", e))
    } else {
        Ok(())
    };

    if let Err(_err) = stream_result {
        effective_mode = FrontendMemoryMode::Legacy;
        let mut codegen = Codegen::new();
        let llvm_ir = codegen
            .codegen(&mir_fns)
            .map_err(|e| miette::miette!("codegen failed: {}", e))?;
        fs::write(llvm_path, llvm_ir)
            .into_diagnostic()
            .map_err(|e| miette::miette!("failed to write LLVM IR: {}", e))?;
    } else if matches!(resolved_memory_mode, FrontendMemoryMode::Legacy) {
        let mut codegen = Codegen::new();
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
}
