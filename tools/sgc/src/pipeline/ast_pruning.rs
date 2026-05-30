use sengoo_compiler::ast::{
    Block as AstBlock, Decl as AstDecl, DeclKind as AstDeclKind, Expr as AstExpr,
    ExprKind as AstExprKind, Program as AstProgram, Stmt as AstStmt, StmtKind as AstStmtKind,
};
use std::collections::{HashMap, HashSet};

use super::{
    hir_prune_min_functions, large_project_optimization_enabled, typeck_filter_min_functions,
};

fn ast_function_count(program: &AstProgram) -> usize {
    program
        .decls
        .iter()
        .filter(|decl| matches!(decl.kind, AstDeclKind::Function(_)))
        .count()
}

pub(super) fn should_prune_unreachable_ast_in_default_mode(program: &AstProgram) -> bool {
    if !large_project_optimization_enabled() {
        return false;
    }
    ast_function_count(program) >= hir_prune_min_functions()
}

pub(super) fn should_filter_typecheck_function_bodies_in_default_mode(
    program: &AstProgram,
) -> bool {
    if !large_project_optimization_enabled() {
        return false;
    }
    ast_function_count(program) >= typeck_filter_min_functions()
}

pub(super) fn reachable_ast_function_names(program: &AstProgram) -> Option<HashSet<String>> {
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
    for root_async_helper in [
        "main__async_body",
        "main__start",
        "main__poll",
        "main__result",
    ] {
        if let Some(&idx) = index_by_name.get(root_async_helper) {
            stack.push(idx);
        }
    }
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

pub(super) fn prune_ast_functions_by_name_set(
    program: &mut AstProgram,
    keep_names: &HashSet<String>,
) -> usize {
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

pub(super) fn prune_unreachable_ast_functions(program: &mut AstProgram) -> usize {
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
    for root_async_helper in [
        "main__async_body",
        "main__start",
        "main__poll",
        "main__result",
    ] {
        if let Some(&idx) = index_by_name.get(root_async_helper) {
            stack.push(idx);
        }
    }
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
