use sengoo_compiler::hir::{HIRBody, HIRExpr, HIRItem, HIRPattern, HIRStmt};
use std::collections::{HashMap, HashSet};

pub(super) fn prune_unreachable_hir_functions(items: &mut Vec<HIRItem>) -> usize {
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
        | HIRExpr::Deref(inner)
        | HIRExpr::Await(inner) => {
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
        HIRExpr::AsyncBlock(body) => {
            for stmt in &body.stmts {
                collect_hir_call_targets_from_stmt(stmt, index_by_name, targets, seen);
            }
            if let Some(expr) = &body.expr {
                collect_hir_call_targets_from_expr(expr, index_by_name, targets, seen);
            }
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
