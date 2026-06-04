use crate::hir::{HIRBody, HIRExpr, HIRStmt};
use crate::mir::Local;
use crate::symbol::SymbolId;
use std::collections::{HashMap, HashSet};

pub(crate) fn collect_named_symbols(expr: &HIRExpr, target_name: &str, out: &mut Vec<SymbolId>) {
    match expr {
        HIRExpr::Var { name, symbol } => {
            if name == target_name {
                out.push(*symbol);
            }
        }
        HIRExpr::Unary(_, operand) => collect_named_symbols(operand, target_name, out),
        HIRExpr::Binary(_, left, right) | HIRExpr::And(left, right) | HIRExpr::Or(left, right) => {
            collect_named_symbols(left, target_name, out);
            collect_named_symbols(right, target_name, out);
        }
        HIRExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_named_symbols(cond, target_name, out);
            collect_named_symbols_in_body(then_branch, target_name, out);
            if let Some(else_body) = else_branch {
                collect_named_symbols_in_body(else_body, target_name, out);
            }
        }
        HIRExpr::Match { scrutinee, arms } => {
            collect_named_symbols(scrutinee, target_name, out);
            for arm in arms {
                collect_named_symbols(&arm.body, target_name, out);
            }
        }
        HIRExpr::Loop(body) | HIRExpr::Block(body) => {
            collect_named_symbols_in_body(body, target_name, out);
        }
        HIRExpr::While { cond, body } => {
            collect_named_symbols(cond, target_name, out);
            collect_named_symbols_in_body(body, target_name, out);
        }
        HIRExpr::For { iter, body, .. } => {
            collect_named_symbols(iter, target_name, out);
            collect_named_symbols_in_body(body, target_name, out);
        }
        HIRExpr::Call { func, args } => {
            collect_named_symbols(func, target_name, out);
            for arg in args {
                collect_named_symbols(arg, target_name, out);
            }
        }
        HIRExpr::MethodCall { receiver, args, .. } => {
            collect_named_symbols(receiver, target_name, out);
            for arg in args {
                collect_named_symbols(arg, target_name, out);
            }
        }
        HIRExpr::Struct { fields, .. } => {
            for (_, expr) in fields {
                collect_named_symbols(expr, target_name, out);
            }
        }
        HIRExpr::Array(items) | HIRExpr::Tuple(items) => {
            for item in items {
                collect_named_symbols(item, target_name, out);
            }
        }
        HIRExpr::Index { base, index } => {
            collect_named_symbols(base, target_name, out);
            collect_named_symbols(index, target_name, out);
        }
        HIRExpr::Field { base, .. }
        | HIRExpr::Return(Some(base))
        | HIRExpr::Break(Some(base))
        | HIRExpr::Cast(base, _)
        | HIRExpr::Ascribe(base, _)
        | HIRExpr::Ref(_, base)
        | HIRExpr::Deref(base) => collect_named_symbols(base, target_name, out),
        HIRExpr::Assign { target, value } | HIRExpr::AssignOp { target, value, .. } => {
            collect_named_symbols(target, target_name, out);
            collect_named_symbols(value, target_name, out);
        }
        HIRExpr::Range { start, end, .. } => {
            if let Some(start) = start {
                collect_named_symbols(start, target_name, out);
            }
            if let Some(end) = end {
                collect_named_symbols(end, target_name, out);
            }
        }
        HIRExpr::Lambda { body, .. } => {
            collect_named_symbols(body, target_name, out);
        }
        HIRExpr::Await(inner) => collect_named_symbols(inner, target_name, out),
        HIRExpr::AsyncBlock(body) | HIRExpr::TryBlock(body) => {
            collect_named_symbols_in_body(body, target_name, out);
        }
        HIRExpr::Try(inner) => collect_named_symbols(inner, target_name, out),
        HIRExpr::Lit(_) | HIRExpr::Return(None) | HIRExpr::Break(None) | HIRExpr::Continue => {}
    }
}

pub(crate) fn collect_named_symbols_in_body(
    body: &HIRBody,
    target_name: &str,
    out: &mut Vec<SymbolId>,
) {
    for stmt in &body.stmts {
        match stmt {
            HIRStmt::Expr(expr) => {
                collect_named_symbols(expr, target_name, out);
            }
            HIRStmt::Let { value, .. } => {
                if let Some(value) = value {
                    collect_named_symbols(value, target_name, out);
                }
            }
            HIRStmt::Item => {}
        }
    }
    if let Some(expr) = &body.expr {
        collect_named_symbols(expr, target_name, out);
    }
}

pub(crate) fn collect_free_vars(
    expr: &HIRExpr,
    params: &[String],
    local_names: &HashMap<String, Local>,
) -> Vec<(String, Local)> {
    let param_names: HashSet<String> = params.iter().cloned().collect();
    let mut free_vars = Vec::new();
    collect_vars_from_expr(expr, &param_names, local_names, &mut free_vars);
    free_vars
}

pub(crate) fn collect_free_vars_in_body(
    body: &HIRBody,
    local_names: &HashMap<String, Local>,
) -> Vec<(String, Local)> {
    let param_names = HashSet::new();
    let mut free_vars = Vec::new();
    collect_vars_from_body(body, &param_names, local_names, &mut free_vars);
    free_vars
}

fn collect_vars_from_expr(
    expr: &HIRExpr,
    param_names: &HashSet<String>,
    local_names: &HashMap<String, Local>,
    free_vars: &mut Vec<(String, Local)>,
) {
    match expr {
        HIRExpr::Var { name, .. } => {
            if !param_names.contains(name) {
                if let Some(&local) = local_names.get(name) {
                    if !free_vars.iter().any(|(n, _)| n == name) {
                        free_vars.push((name.clone(), local));
                    }
                }
            }
        }
        HIRExpr::Lit(_) => {}
        HIRExpr::Unary(_, operand) => {
            collect_vars_from_expr(operand, param_names, local_names, free_vars);
        }
        HIRExpr::Binary(_, left, right) | HIRExpr::And(left, right) | HIRExpr::Or(left, right) => {
            collect_vars_from_expr(left, param_names, local_names, free_vars);
            collect_vars_from_expr(right, param_names, local_names, free_vars);
        }
        HIRExpr::Call { func, args } => {
            collect_vars_from_expr(func, param_names, local_names, free_vars);
            for arg in args {
                collect_vars_from_expr(arg, param_names, local_names, free_vars);
            }
        }
        HIRExpr::Lambda {
            params: inner_params,
            body: inner_body,
        } => {
            let inner_param_names: HashSet<String> = inner_params.iter().cloned().collect();
            collect_vars_from_expr(inner_body, &inner_param_names, local_names, free_vars);
        }
        HIRExpr::Block(body) => {
            collect_vars_from_body(body, param_names, local_names, free_vars);
        }
        HIRExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_vars_from_expr(cond, param_names, local_names, free_vars);
            collect_vars_from_body(then_branch, param_names, local_names, free_vars);
            if let Some(else_b) = else_branch {
                collect_vars_from_body(else_b, param_names, local_names, free_vars);
            }
        }
        HIRExpr::Loop(body) => {
            collect_vars_from_body(body, param_names, local_names, free_vars);
        }
        HIRExpr::While { cond, body } => {
            collect_vars_from_expr(cond, param_names, local_names, free_vars);
            collect_vars_from_body(body, param_names, local_names, free_vars);
        }
        HIRExpr::Break(_) | HIRExpr::Continue => {}
        HIRExpr::Array(elems) | HIRExpr::Tuple(elems) => {
            for elem in elems {
                collect_vars_from_expr(elem, param_names, local_names, free_vars);
            }
        }
        HIRExpr::Index { base, index } => {
            collect_vars_from_expr(base, param_names, local_names, free_vars);
            collect_vars_from_expr(index, param_names, local_names, free_vars);
        }
        HIRExpr::Struct { fields, .. } => {
            for (_, field_val) in fields {
                collect_vars_from_expr(field_val, param_names, local_names, free_vars);
            }
        }
        HIRExpr::Field { base, .. } => {
            collect_vars_from_expr(base, param_names, local_names, free_vars);
        }
        HIRExpr::For {
            var_name,
            iter,
            body,
            ..
        } => {
            collect_vars_from_expr(iter, param_names, local_names, free_vars);
            let mut extended_params = param_names.clone();
            extended_params.insert(var_name.clone());
            collect_vars_from_body(body, &extended_params, local_names, free_vars);
        }
        HIRExpr::Assign { target, value } | HIRExpr::AssignOp { target, value, .. } => {
            collect_vars_from_expr(target, param_names, local_names, free_vars);
            collect_vars_from_expr(value, param_names, local_names, free_vars);
        }
        HIRExpr::MethodCall { receiver, args, .. } => {
            collect_vars_from_expr(receiver, param_names, local_names, free_vars);
            for arg in args {
                collect_vars_from_expr(arg, param_names, local_names, free_vars);
            }
        }
        HIRExpr::Await(inner) | HIRExpr::Try(inner) => {
            collect_vars_from_expr(inner, param_names, local_names, free_vars);
        }
        HIRExpr::AsyncBlock(body) | HIRExpr::TryBlock(body) => {
            collect_vars_from_body(body, param_names, local_names, free_vars);
        }
        HIRExpr::Match { scrutinee, arms } => {
            collect_vars_from_expr(scrutinee, param_names, local_names, free_vars);
            for arm in arms {
                collect_vars_from_expr(&arm.body, param_names, local_names, free_vars);
            }
        }
        HIRExpr::Return(Some(base))
        | HIRExpr::Cast(base, _)
        | HIRExpr::Ascribe(base, _)
        | HIRExpr::Ref(_, base)
        | HIRExpr::Deref(base) => {
            collect_vars_from_expr(base, param_names, local_names, free_vars);
        }
        HIRExpr::Range { start, end, .. } => {
            if let Some(start) = start {
                collect_vars_from_expr(start, param_names, local_names, free_vars);
            }
            if let Some(end) = end {
                collect_vars_from_expr(end, param_names, local_names, free_vars);
            }
        }
        _ => {}
    }
}

fn collect_vars_from_body(
    body: &HIRBody,
    param_names: &HashSet<String>,
    local_names: &HashMap<String, Local>,
    free_vars: &mut Vec<(String, Local)>,
) {
    let mut scoped_names = param_names.clone();
    for stmt in &body.stmts {
        collect_vars_from_stmt(stmt, &scoped_names, local_names, free_vars);
        if let HIRStmt::Let { name, .. } = stmt {
            scoped_names.insert(name.clone());
        }
    }
    if let Some(expr) = &body.expr {
        collect_vars_from_expr(expr, &scoped_names, local_names, free_vars);
    }
}

fn collect_vars_from_stmt(
    stmt: &HIRStmt,
    param_names: &HashSet<String>,
    local_names: &HashMap<String, Local>,
    free_vars: &mut Vec<(String, Local)>,
) {
    match stmt {
        HIRStmt::Let { value, .. } => {
            if let Some(v) = value {
                collect_vars_from_expr(v, param_names, local_names, free_vars);
            }
        }
        HIRStmt::Expr(expr) => {
            collect_vars_from_expr(expr, param_names, local_names, free_vars);
        }
        HIRStmt::Item => {}
    }
}
