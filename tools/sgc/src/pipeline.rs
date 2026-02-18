use miette::{IntoDiagnostic, Result};
use sengoo_compiler::mir::{
    Instruction as MirInstruction, MirFunction, Terminator as MirTerminator,
};
use sengoo_compiler::{lower_ast, lower_hir, Codegen, MirOptLevel, Parser, TypeChecker};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::BufWriter;
use std::path::Path;
use std::time::Instant;

use crate::{resolve_frontend_memory_mode, FrontendMemoryMode};

pub(crate) fn compile_source(source: &str, opt_level: u8) -> std::result::Result<String, String> {
    compile_source_with_phase_timings(source, opt_level)
        .map(|(llvm_ir, _)| llvm_ir)
        .map_err(|e| e.to_string())
}

pub(crate) fn compile_source_with_phase_timings(
    source: &str,
    opt_level: u8,
) -> Result<(String, BTreeMap<String, f64>)> {
    let resolved_memory_mode = resolve_frontend_memory_mode(source.len());
    let (mir_fns, mut phases) = compile_frontend_to_mir_with_phase_timings(source, opt_level)?;

    let codegen_start = Instant::now();
    let mut codegen = Codegen::new();
    let llvm_ir = match resolved_memory_mode {
        FrontendMemoryMode::Stream => {
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
) -> Result<(Vec<MirFunction>, BTreeMap<String, f64>)> {
    let mut phases = BTreeMap::new();

    let (mut mir_fns, parse_ms, typeck_ms, mir_ms) = {
        let parse_start = Instant::now();
        let program = Parser::parse(source).map_err(|e| miette::miette!("parse failed: {}", e))?;
        let parse_ms = parse_start.elapsed().as_secs_f64() * 1000.0;

        let typeck_start = Instant::now();
        let mut checker = TypeChecker::new();
        checker
            .check_program(&program)
            .map_err(|e| miette::miette!("typecheck failed: {}", e))?;
        let type_env = checker.into_env();
        let typeck_ms = typeck_start.elapsed().as_secs_f64() * 1000.0;

        let mir_start = Instant::now();
        let hir_module = lower_ast(&program, &type_env);
        let mut mir_fns = lower_hir(&hir_module.items).map_err(|e| miette::miette!("{}", e))?;
        drop(hir_module);
        drop(type_env);
        drop(program);
        // Prune unreachable functions before MIR optimization to avoid spending
        // optimization work on dead code in large single-file workloads.
        prune_unreachable_mir_functions(&mut mir_fns);
        let mir_opt_level = MirOptLevel::from_u8(opt_level)
            .ok_or_else(|| miette::miette!("invalid optimization level: {}", opt_level))?;
        let pipeline = sengoo_compiler::mir::opt::pipeline_for_level(mir_opt_level);
        pipeline.run(&mut mir_fns);
        let mir_ms = mir_start.elapsed().as_secs_f64() * 1000.0;

        (mir_fns, parse_ms, typeck_ms, mir_ms)
    };

    phases.insert("parse".to_string(), parse_ms);
    phases.insert("typeck".to_string(), typeck_ms);
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
    let resolved_memory_mode = resolve_frontend_memory_mode(source.len());
    let (mir_fns, mut phases) = compile_frontend_to_mir_with_phase_timings(source, opt_level)?;

    let codegen_start = Instant::now();
    let mut effective_mode = resolved_memory_mode;
    let stream_result = if matches!(resolved_memory_mode, FrontendMemoryMode::Stream) {
        let file = fs::File::create(llvm_path)
            .into_diagnostic()
            .map_err(|e| {
                miette::miette!("failed to create LLVM IR output {}: {}", llvm_path.display(), e)
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

fn prune_unreachable_mir_functions(mir_fns: &mut Vec<MirFunction>) -> usize {
    if mir_fns.len() <= 1 {
        return 0;
    }

    let mut index_by_name = HashMap::new();
    for (idx, mir_fn) in mir_fns.iter().enumerate() {
        index_by_name.insert(mir_fn.name.clone(), idx);
    }

    let Some(&main_index) = index_by_name.get("main") else {
        return 0;
    };

    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); mir_fns.len()];
    for (idx, mir_fn) in mir_fns.iter().enumerate() {
        edges[idx] = collect_mir_call_targets(mir_fn, &index_by_name);
    }

    let mut reachable = vec![false; mir_fns.len()];
    let mut stack = vec![main_index];
    while let Some(idx) = stack.pop() {
        if reachable[idx] {
            continue;
        }
        reachable[idx] = true;
        for &target in &edges[idx] {
            if !reachable[target] {
                stack.push(target);
            }
        }
    }

    let before = mir_fns.len();
    let mut old_fns = std::mem::take(mir_fns);
    old_fns.reverse();
    let mut kept = Vec::with_capacity(before);
    while let Some(mir_fn) = old_fns.pop() {
        if let Some(&idx) = index_by_name.get(&mir_fn.name) {
            if reachable[idx] {
                kept.push(mir_fn);
            }
        }
    }
    let removed = before.saturating_sub(kept.len());
    *mir_fns = kept;
    removed
}

fn collect_mir_call_targets(
    mir_fn: &MirFunction,
    index_by_name: &HashMap<String, usize>,
) -> Vec<usize> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();
    for block in &mir_fn.basic_blocks {
        for inst_id in &block.instructions {
            let inst = mir_fn.instruction(*inst_id);
            if let MirInstruction::Call { func, .. } = inst {
                if let Some(&idx) = index_by_name.get(func) {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                }
            }
        }
        if let Some(MirTerminator::Call { func, .. }) = &block.terminator {
            if let Some(&idx) = index_by_name.get(func) {
                if seen.insert(idx) {
                    targets.push(idx);
                }
            }
        }
    }
    targets
}
