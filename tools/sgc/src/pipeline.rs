use miette::{IntoDiagnostic, Result};
use sengoo_compiler::ast::{
    Block as AstBlock, Decl as AstDecl, DeclKind as AstDeclKind, Expr as AstExpr,
    ExprKind as AstExprKind, Program as AstProgram, Stmt as AstStmt, StmtKind as AstStmtKind,
};
use sengoo_compiler::hir::{HIRBody, HIRExpr, HIRItem, HIRPattern, HIRStmt};
use sengoo_compiler::mir::{
    Instruction as MirInstruction, MirFunction, Terminator as MirTerminator,
};
use sengoo_compiler::{lower_ast, lower_hir, Codegen, MirOptLevel, Parser, TypeChecker};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::io::BufWriter;
use std::path::Path;
use std::sync::atomic::{AtomicI8, Ordering};
use std::time::Instant;

use crate::{resolve_frontend_memory_mode, FrontendMemoryMode};

const DEFAULT_HIR_PRUNE_MIN_FUNCTIONS: usize = 20_000;
const DEFAULT_TYPECK_FILTER_MIN_FUNCTIONS: usize = 120_000;
const LARGE_PROJECT_MODE_AUTO: i8 = 0;
const LARGE_PROJECT_MODE_ENABLED: i8 = 1;
const LARGE_PROJECT_MODE_DISABLED: i8 = -1;

static LARGE_PROJECT_MODE_OVERRIDE: AtomicI8 = AtomicI8::new(LARGE_PROJECT_MODE_AUTO);

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
    let previous = LARGE_PROJECT_MODE_OVERRIDE.swap(
        encode_large_project_mode_override(value),
        Ordering::Relaxed,
    );
    decode_large_project_mode_override(previous)
}

fn parse_large_project_mode_env(raw: &str) -> Option<bool> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "on" | "yes" | "enable" | "enabled" => Some(true),
        "0" | "false" | "off" | "no" | "disable" | "disabled" => Some(false),
        _ => None,
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

fn ast_function_count(program: &AstProgram) -> usize {
    program
        .decls
        .iter()
        .filter(|decl| matches!(decl.kind, AstDeclKind::Function(_)))
        .count()
}

fn should_prune_unreachable_ast_in_default_mode(program: &AstProgram) -> bool {
    if !large_project_optimization_enabled() {
        return false;
    }
    ast_function_count(program) >= hir_prune_min_functions()
}

fn should_filter_typecheck_function_bodies_in_default_mode(program: &AstProgram) -> bool {
    if !large_project_optimization_enabled() {
        return false;
    }
    ast_function_count(program) >= typeck_filter_min_functions()
}

fn reachable_ast_function_names(program: &AstProgram) -> Option<HashSet<String>> {
    let functions: Vec<_> = program
        .decls
        .iter()
        .filter_map(|decl| match &decl.kind {
            AstDeclKind::Function(fn_decl) => Some(fn_decl),
            _ => None,
        })
        .collect();
    if functions.is_empty() {
        return Some(HashSet::new());
    }

    let mut index_by_name = HashMap::new();
    for (idx, fn_decl) in functions.iter().enumerate() {
        index_by_name.insert(fn_decl.name.name.clone(), idx);
    }
    let &main_index = index_by_name.get("main")?;

    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); functions.len()];
    for (idx, fn_decl) in functions.iter().enumerate() {
        edges[idx] = collect_ast_call_targets_from_block(&fn_decl.body, &index_by_name);
    }

    let mut reachable = vec![false; functions.len()];
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

    let mut names = HashSet::new();
    for (idx, fn_decl) in functions.iter().enumerate() {
        if reachable[idx] {
            names.insert(fn_decl.name.name.clone());
        }
    }
    Some(names)
}

fn prune_ast_functions_by_name_set(program: &mut AstProgram, keep_names: &HashSet<String>) -> usize {
    let mut removed = 0usize;
    let mut kept = Vec::with_capacity(program.decls.len());
    for decl in std::mem::take(&mut program.decls) {
        match decl.kind {
            AstDeclKind::Function(fn_decl) => {
                if keep_names.contains(&fn_decl.name.name) {
                    kept.push(AstDecl {
                        kind: AstDeclKind::Function(fn_decl),
                        span: decl.span,
                    });
                } else {
                    removed += 1;
                }
            }
            _ => kept.push(decl),
        }
    }
    program.decls = kept;
    removed
}

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
            reachable_ast_function_names(program.as_ref().expect("program present before typecheck"))
        } else {
            None
        };

        if let Some(reachable) = reachable_typecheck_bodies.as_ref() {
            let ast_prune_start = Instant::now();
            let removed =
                prune_ast_functions_by_name_set(program.as_mut().expect("program present"), reachable);
            ast_prune_ms += ast_prune_start.elapsed().as_secs_f64() * 1000.0;
            ast_pruned_count += removed;
            ast_prune_applied = ast_prune_applied || removed > 0;
        }

        let typeck_start = Instant::now();
        let mut checker = TypeChecker::new();
        checker
            .check_program(program.as_ref().expect("program present during typeck"))
            .map_err(|e| miette::miette!("typecheck failed: {}", e))?;
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
        let mut mir_fns = lower_hir(&hir_module.items).map_err(|e| miette::miette!("{}", e))?;
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

fn prune_unreachable_ast_functions(program: &mut AstProgram) -> usize {
    let functions: Vec<_> = program
        .decls
        .iter()
        .filter_map(|decl| match &decl.kind {
            AstDeclKind::Function(fn_decl) => Some(fn_decl),
            _ => None,
        })
        .collect();
    if functions.len() <= 1 {
        return 0;
    }

    let mut index_by_name = HashMap::new();
    for (idx, fn_decl) in functions.iter().enumerate() {
        index_by_name.insert(fn_decl.name.name.clone(), idx);
    }

    let Some(&main_index) = index_by_name.get("main") else {
        return 0;
    };

    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); functions.len()];
    for (idx, fn_decl) in functions.iter().enumerate() {
        edges[idx] = collect_ast_call_targets_from_block(&fn_decl.body, &index_by_name);
    }

    let mut reachable = vec![false; functions.len()];
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

    let mut removed = 0usize;
    let mut kept = Vec::with_capacity(program.decls.len());
    for decl in std::mem::take(&mut program.decls) {
        match decl.kind {
            AstDeclKind::Function(fn_decl) => {
                let is_reachable = index_by_name
                    .get(&fn_decl.name.name)
                    .map(|&idx| reachable[idx])
                    .unwrap_or(true);
                if is_reachable {
                    kept.push(AstDecl {
                        kind: AstDeclKind::Function(fn_decl),
                        span: decl.span,
                    });
                } else {
                    removed += 1;
                }
            }
            _ => kept.push(decl),
        }
    }
    program.decls = kept;
    removed
}

fn collect_ast_call_targets_from_block(
    block: &AstBlock,
    index_by_name: &HashMap<String, usize>,
) -> Vec<usize> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();
    for stmt in &block.stmts {
        collect_ast_call_targets_from_stmt(stmt, index_by_name, &mut targets, &mut seen);
    }
    targets
}

fn collect_ast_call_targets_from_stmt(
    stmt: &AstStmt,
    index_by_name: &HashMap<String, usize>,
    targets: &mut Vec<usize>,
    seen: &mut HashSet<usize>,
) {
    match &stmt.kind {
        AstStmtKind::Let { value, .. } => {
            if let Some(value) = value {
                collect_ast_call_targets_from_expr(value, index_by_name, targets, seen);
            }
        }
        AstStmtKind::Const { value, .. } | AstStmtKind::Expr(value) => {
            collect_ast_call_targets_from_expr(value, index_by_name, targets, seen);
        }
        AstStmtKind::Item(decl) => {
            collect_ast_call_targets_from_decl(decl, index_by_name, targets, seen);
        }
    }
}

fn collect_ast_call_targets_from_decl(
    decl: &AstDecl,
    index_by_name: &HashMap<String, usize>,
    targets: &mut Vec<usize>,
    seen: &mut HashSet<usize>,
) {
    match &decl.kind {
        AstDeclKind::Function(fn_decl) => {
            collect_ast_call_targets_from_block(&fn_decl.body, index_by_name)
                .into_iter()
                .for_each(|idx| {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                });
        }
        AstDeclKind::Const(const_decl) => {
            collect_ast_call_targets_from_expr(&const_decl.value, index_by_name, targets, seen);
        }
        AstDeclKind::Static(static_decl) => {
            collect_ast_call_targets_from_expr(&static_decl.value, index_by_name, targets, seen);
        }
        AstDeclKind::Module(module_decl) => {
            for nested in &module_decl.items {
                collect_ast_call_targets_from_decl(nested, index_by_name, targets, seen);
            }
        }
        _ => {}
    }
}

fn collect_ast_call_targets_from_expr(
    expr: &AstExpr,
    index_by_name: &HashMap<String, usize>,
    targets: &mut Vec<usize>,
    seen: &mut HashSet<usize>,
) {
    match &expr.kind {
        AstExprKind::Literal(_) | AstExprKind::Continue => {}
        AstExprKind::Ident(ident) => {
            if let Some(&idx) = index_by_name.get(&ident.name) {
                if seen.insert(idx) {
                    targets.push(idx);
                }
            }
        }
        AstExprKind::Path(path) => {
            if let Some(simple) = path.as_simple() {
                if let Some(&idx) = index_by_name.get(&simple.name) {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                }
            }
        }
        AstExprKind::Unary { operand, .. }
        | AstExprKind::Await(operand)
        | AstExprKind::Try(operand)
        | AstExprKind::Paren(operand) => {
            collect_ast_call_targets_from_expr(operand, index_by_name, targets, seen);
        }
        AstExprKind::Binary { left, right, .. }
        | AstExprKind::Index {
            base: left,
            index: right,
        }
        | AstExprKind::Assign {
            target: left,
            value: right,
        }
        | AstExprKind::AssignOp {
            target: left,
            value: right,
            ..
        } => {
            collect_ast_call_targets_from_expr(left, index_by_name, targets, seen);
            collect_ast_call_targets_from_expr(right, index_by_name, targets, seen);
        }
        AstExprKind::Call { func, args } => {
            collect_ast_call_targets_from_expr(func, index_by_name, targets, seen);
            for arg in args {
                collect_ast_call_targets_from_expr(arg, index_by_name, targets, seen);
            }
            match &func.kind {
                AstExprKind::Ident(ident) => {
                    if let Some(&idx) = index_by_name.get(&ident.name) {
                        if seen.insert(idx) {
                            targets.push(idx);
                        }
                    }
                }
                AstExprKind::Path(path) => {
                    if let Some(simple) = path.as_simple() {
                        if let Some(&idx) = index_by_name.get(&simple.name) {
                            if seen.insert(idx) {
                                targets.push(idx);
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        AstExprKind::MethodCall { receiver, args, .. } => {
            collect_ast_call_targets_from_expr(receiver, index_by_name, targets, seen);
            for arg in args {
                collect_ast_call_targets_from_expr(arg, index_by_name, targets, seen);
            }
        }
        AstExprKind::Block(block)
        | AstExprKind::Loop(block)
        | AstExprKind::AsyncBlock(block)
        | AstExprKind::ParallelBlock(block) => {
            collect_ast_call_targets_from_block(block, index_by_name)
                .into_iter()
                .for_each(|idx| {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                });
        }
        AstExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_ast_call_targets_from_expr(cond, index_by_name, targets, seen);
            collect_ast_call_targets_from_block(then_branch, index_by_name)
                .into_iter()
                .for_each(|idx| {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                });
            if let Some(else_branch) = else_branch {
                collect_ast_call_targets_from_expr(else_branch, index_by_name, targets, seen);
            }
        }
        AstExprKind::While { cond, body } => {
            collect_ast_call_targets_from_expr(cond, index_by_name, targets, seen);
            collect_ast_call_targets_from_block(body, index_by_name)
                .into_iter()
                .for_each(|idx| {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                });
        }
        AstExprKind::For { iter, body, .. } => {
            collect_ast_call_targets_from_expr(iter, index_by_name, targets, seen);
            collect_ast_call_targets_from_block(body, index_by_name)
                .into_iter()
                .for_each(|idx| {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                });
        }
        AstExprKind::Match { scrutinee, arms } => {
            collect_ast_call_targets_from_expr(scrutinee, index_by_name, targets, seen);
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    collect_ast_call_targets_from_expr(guard, index_by_name, targets, seen);
                }
                collect_ast_call_targets_from_expr(&arm.body, index_by_name, targets, seen);
            }
        }
        AstExprKind::Return(value) | AstExprKind::Break(value) | AstExprKind::Yield(value) => {
            if let Some(value) = value {
                collect_ast_call_targets_from_expr(value, index_by_name, targets, seen);
            }
        }
        AstExprKind::Field { base, .. }
        | AstExprKind::Cast { expr: base, .. }
        | AstExprKind::Is { expr: base, .. } => {
            collect_ast_call_targets_from_expr(base, index_by_name, targets, seen);
        }
        AstExprKind::Array(elements) | AstExprKind::Tuple(elements) => {
            for element in elements {
                collect_ast_call_targets_from_expr(element, index_by_name, targets, seen);
            }
        }
        AstExprKind::Struct { fields, base, .. } => {
            for field in fields {
                collect_ast_call_targets_from_expr(&field.value, index_by_name, targets, seen);
            }
            if let Some(base) = base {
                collect_ast_call_targets_from_expr(base, index_by_name, targets, seen);
            }
        }
        AstExprKind::Range { start, end, .. } => {
            if let Some(start) = start {
                collect_ast_call_targets_from_expr(start, index_by_name, targets, seen);
            }
            if let Some(end) = end {
                collect_ast_call_targets_from_expr(end, index_by_name, targets, seen);
            }
        }
        AstExprKind::Lambda { body, .. } => {
            collect_ast_call_targets_from_expr(body, index_by_name, targets, seen);
        }
    }
}

fn prune_unreachable_hir_functions(items: &mut Vec<HIRItem>) -> usize {
    let functions: Vec<_> = items
        .iter()
        .filter_map(|item| match item {
            HIRItem::Function(fn_item) => Some(fn_item),
            _ => None,
        })
        .collect();
    if functions.len() <= 1 {
        return 0;
    }

    let mut index_by_name = HashMap::new();
    for (idx, fn_item) in functions.iter().enumerate() {
        index_by_name.insert(fn_item.name.clone(), idx);
    }

    let Some(&main_index) = index_by_name.get("main") else {
        return 0;
    };

    let mut edges: Vec<Vec<usize>> = vec![Vec::new(); functions.len()];
    for (idx, fn_item) in functions.iter().enumerate() {
        edges[idx] = collect_hir_call_targets_from_body(&fn_item.body, &index_by_name);
    }

    let mut reachable = vec![false; functions.len()];
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

    let mut removed = 0usize;
    let mut kept = Vec::with_capacity(items.len());
    for item in std::mem::take(items) {
        match item {
            HIRItem::Function(fn_item) => {
                let is_reachable = index_by_name
                    .get(&fn_item.name)
                    .map(|&idx| reachable[idx])
                    .unwrap_or(true);
                if is_reachable {
                    kept.push(HIRItem::Function(fn_item));
                } else {
                    removed += 1;
                }
            }
            other => kept.push(other),
        }
    }
    *items = kept;
    removed
}

fn collect_hir_call_targets_from_body(
    body: &HIRBody,
    index_by_name: &HashMap<String, usize>,
) -> Vec<usize> {
    let mut targets = Vec::new();
    let mut seen = HashSet::new();

    for stmt in &body.stmts {
        collect_hir_call_targets_from_stmt(stmt, index_by_name, &mut targets, &mut seen);
    }
    if let Some(expr) = &body.expr {
        collect_hir_call_targets_from_expr(expr, index_by_name, &mut targets, &mut seen);
    }

    targets
}

fn collect_hir_call_targets_from_stmt(
    stmt: &HIRStmt,
    index_by_name: &HashMap<String, usize>,
    targets: &mut Vec<usize>,
    seen: &mut HashSet<usize>,
) {
    match stmt {
        HIRStmt::Let { value, .. } => {
            if let Some(expr) = value {
                collect_hir_call_targets_from_expr(expr, index_by_name, targets, seen);
            }
        }
        HIRStmt::Expr(expr) => {
            collect_hir_call_targets_from_expr(expr, index_by_name, targets, seen);
        }
        HIRStmt::Item => {}
    }
}

fn collect_hir_call_targets_from_expr(
    expr: &HIRExpr,
    index_by_name: &HashMap<String, usize>,
    targets: &mut Vec<usize>,
    seen: &mut HashSet<usize>,
) {
    match expr {
        HIRExpr::Lit(_) | HIRExpr::Continue => {}
        HIRExpr::Var { name, .. } => {
            if let Some(&idx) = index_by_name.get(name) {
                if seen.insert(idx) {
                    targets.push(idx);
                }
            }
        }
        HIRExpr::Unary(_, inner)
        | HIRExpr::Cast(inner, _)
        | HIRExpr::Ascribe(inner, _)
        | HIRExpr::Ref(_, inner)
        | HIRExpr::Deref(inner) => {
            collect_hir_call_targets_from_expr(inner, index_by_name, targets, seen);
        }
        HIRExpr::Binary(_, lhs, rhs)
        | HIRExpr::And(lhs, rhs)
        | HIRExpr::Or(lhs, rhs)
        | HIRExpr::Index {
            base: lhs,
            index: rhs,
        }
        | HIRExpr::Assign {
            target: lhs,
            value: rhs,
        }
        | HIRExpr::AssignOp {
            target: lhs,
            value: rhs,
            ..
        } => {
            collect_hir_call_targets_from_expr(lhs, index_by_name, targets, seen);
            collect_hir_call_targets_from_expr(rhs, index_by_name, targets, seen);
        }
        HIRExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_hir_call_targets_from_expr(cond, index_by_name, targets, seen);
            collect_hir_call_targets_from_body(then_branch, index_by_name)
                .into_iter()
                .for_each(|idx| {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                });
            if let Some(else_branch) = else_branch {
                collect_hir_call_targets_from_body(else_branch, index_by_name)
                    .into_iter()
                    .for_each(|idx| {
                        if seen.insert(idx) {
                            targets.push(idx);
                        }
                    });
            }
        }
        HIRExpr::Match { scrutinee, arms } => {
            collect_hir_call_targets_from_expr(scrutinee, index_by_name, targets, seen);
            for arm in arms {
                collect_hir_call_targets_from_pattern(&arm.pat, index_by_name, targets, seen);
                if let Some(guard) = &arm.guard {
                    collect_hir_call_targets_from_expr(guard, index_by_name, targets, seen);
                }
                collect_hir_call_targets_from_expr(&arm.body, index_by_name, targets, seen);
            }
        }
        HIRExpr::Loop(body) | HIRExpr::Block(body) => {
            collect_hir_call_targets_from_body(body, index_by_name)
                .into_iter()
                .for_each(|idx| {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                });
        }
        HIRExpr::While { cond, body } => {
            collect_hir_call_targets_from_expr(cond, index_by_name, targets, seen);
            collect_hir_call_targets_from_body(body, index_by_name)
                .into_iter()
                .for_each(|idx| {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                });
        }
        HIRExpr::For { iter, body, .. } => {
            collect_hir_call_targets_from_expr(iter, index_by_name, targets, seen);
            collect_hir_call_targets_from_body(body, index_by_name)
                .into_iter()
                .for_each(|idx| {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                });
        }
        HIRExpr::Call { func, args } => {
            collect_hir_call_targets_from_expr(func, index_by_name, targets, seen);
            for arg in args {
                collect_hir_call_targets_from_expr(arg, index_by_name, targets, seen);
            }
            if let HIRExpr::Var { name, .. } = func.as_ref() {
                if let Some(&idx) = index_by_name.get(name) {
                    if seen.insert(idx) {
                        targets.push(idx);
                    }
                }
            }
        }
        HIRExpr::MethodCall { receiver, args, .. } => {
            collect_hir_call_targets_from_expr(receiver, index_by_name, targets, seen);
            for arg in args {
                collect_hir_call_targets_from_expr(arg, index_by_name, targets, seen);
            }
        }
        HIRExpr::Struct { fields, .. } => {
            for (_, value) in fields {
                collect_hir_call_targets_from_expr(value, index_by_name, targets, seen);
            }
        }
        HIRExpr::Array(items) | HIRExpr::Tuple(items) => {
            for item in items {
                collect_hir_call_targets_from_expr(item, index_by_name, targets, seen);
            }
        }
        HIRExpr::Field { base, .. } => {
            collect_hir_call_targets_from_expr(base, index_by_name, targets, seen);
        }
        HIRExpr::Return(value) | HIRExpr::Break(value) => {
            if let Some(value) = value {
                collect_hir_call_targets_from_expr(value, index_by_name, targets, seen);
            }
        }
        HIRExpr::Range { start, end, .. } => {
            if let Some(start) = start {
                collect_hir_call_targets_from_expr(start, index_by_name, targets, seen);
            }
            if let Some(end) = end {
                collect_hir_call_targets_from_expr(end, index_by_name, targets, seen);
            }
        }
        HIRExpr::Lambda { body, .. } => {
            collect_hir_call_targets_from_expr(body, index_by_name, targets, seen);
        }
    }
}

fn collect_hir_call_targets_from_pattern(
    pattern: &HIRPattern,
    index_by_name: &HashMap<String, usize>,
    targets: &mut Vec<usize>,
    seen: &mut HashSet<usize>,
) {
    match pattern {
        HIRPattern::Wild | HIRPattern::Lit(_) | HIRPattern::Var { .. } => {}
        HIRPattern::Struct { fields, .. } => {
            for (_, sub_pattern) in fields {
                if let Some(sub_pattern) = sub_pattern {
                    collect_hir_call_targets_from_pattern(
                        sub_pattern,
                        index_by_name,
                        targets,
                        seen,
                    );
                }
            }
        }
        HIRPattern::Tuple(items) => {
            for item in items {
                collect_hir_call_targets_from_pattern(item, index_by_name, targets, seen);
            }
        }
        HIRPattern::Or(lhs, rhs) => {
            collect_hir_call_targets_from_pattern(lhs, index_by_name, targets, seen);
            collect_hir_call_targets_from_pattern(rhs, index_by_name, targets, seen);
        }
        HIRPattern::Slice {
            before,
            rest,
            after,
        } => {
            for item in before {
                collect_hir_call_targets_from_pattern(item, index_by_name, targets, seen);
            }
            if let Some(rest) = rest {
                collect_hir_call_targets_from_pattern(rest, index_by_name, targets, seen);
            }
            for item in after {
                collect_hir_call_targets_from_pattern(item, index_by_name, targets, seen);
            }
        }
        HIRPattern::Range { start, end } => {
            if let Some(start) = start {
                collect_hir_call_targets_from_expr(start, index_by_name, targets, seen);
            }
            if let Some(end) = end {
                collect_hir_call_targets_from_expr(end, index_by_name, targets, seen);
            }
        }
        HIRPattern::Ref(inner) | HIRPattern::RefMut(inner) => {
            collect_hir_call_targets_from_pattern(inner, index_by_name, targets, seen);
        }
    }
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
