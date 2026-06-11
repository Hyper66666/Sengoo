use crate::hir::{HIRBody, HIRExpr, HIRItem, HIRStmt};
use std::collections::HashSet;

pub(crate) fn collect_direct_calls_in_expr(expr: &HIRExpr, out: &mut HashSet<String>) {
    match expr {
        HIRExpr::Lit(_) | HIRExpr::Var { .. } | HIRExpr::Continue => {}
        HIRExpr::Unary(_, inner)
        | HIRExpr::Deref(inner)
        | HIRExpr::Ref(_, inner)
        | HIRExpr::Cast(inner, _)
        | HIRExpr::Ascribe(inner, _) => {
            collect_direct_calls_in_expr(inner, out);
        }
        HIRExpr::Binary(_, lhs, rhs)
        | HIRExpr::And(lhs, rhs)
        | HIRExpr::Or(lhs, rhs)
        | HIRExpr::Assign {
            target: lhs,
            value: rhs,
        }
        | HIRExpr::AssignOp {
            target: lhs,
            value: rhs,
            ..
        } => {
            collect_direct_calls_in_expr(lhs, out);
            collect_direct_calls_in_expr(rhs, out);
        }
        HIRExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_direct_calls_in_expr(cond, out);
            collect_direct_calls_in_body(then_branch, out);
            if let Some(else_branch) = else_branch.as_deref() {
                collect_direct_calls_in_body(else_branch, out);
            }
        }
        HIRExpr::Match { scrutinee, arms } => {
            collect_direct_calls_in_expr(scrutinee, out);
            for arm in arms {
                if let Some(guard) = arm.guard.as_deref() {
                    collect_direct_calls_in_expr(guard, out);
                }
                collect_direct_calls_in_expr(&arm.body, out);
            }
        }
        HIRExpr::Loop(body) | HIRExpr::Block(body) => {
            collect_direct_calls_in_body(body, out);
        }
        HIRExpr::While { cond, body } => {
            collect_direct_calls_in_expr(cond, out);
            collect_direct_calls_in_body(body, out);
        }
        HIRExpr::For { iter, body, .. } => {
            collect_direct_calls_in_expr(iter, out);
            collect_direct_calls_in_body(body, out);
        }
        HIRExpr::Call { func, args, .. } => {
            if let HIRExpr::Var { name, .. } = func.as_ref() {
                out.insert(name.clone());
            }
            collect_direct_calls_in_expr(func, out);
            for arg in args {
                collect_direct_calls_in_expr(arg, out);
            }
        }
        HIRExpr::EnumConstruct { args, .. } => {
            for arg in args {
                collect_direct_calls_in_expr(arg, out);
            }
        }
        HIRExpr::MethodCall { receiver, args, .. } => {
            collect_direct_calls_in_expr(receiver, out);
            for arg in args {
                collect_direct_calls_in_expr(arg, out);
            }
        }
        HIRExpr::Struct { fields, .. } => {
            for (_, value) in fields {
                collect_direct_calls_in_expr(value, out);
            }
        }
        HIRExpr::Array(items) | HIRExpr::Tuple(items) => {
            for item in items {
                collect_direct_calls_in_expr(item, out);
            }
        }
        HIRExpr::Index { base, index } => {
            collect_direct_calls_in_expr(base, out);
            collect_direct_calls_in_expr(index, out);
        }
        HIRExpr::Field { base, .. } => {
            collect_direct_calls_in_expr(base, out);
        }
        HIRExpr::Return(value) | HIRExpr::Break(value) => {
            if let Some(value) = value.as_deref() {
                collect_direct_calls_in_expr(value, out);
            }
        }
        HIRExpr::Range { start, end, .. } => {
            if let Some(start) = start.as_deref() {
                collect_direct_calls_in_expr(start, out);
            }
            if let Some(end) = end.as_deref() {
                collect_direct_calls_in_expr(end, out);
            }
        }
        HIRExpr::Lambda { body, .. } => {
            collect_direct_calls_in_expr(body, out);
        }
        HIRExpr::Await(inner) => {
            collect_direct_calls_in_expr(inner, out);
        }
        HIRExpr::AsyncBlock(body) | HIRExpr::TryBlock(body) => {
            collect_direct_calls_in_body(body, out);
        }
        HIRExpr::Try(inner) => {
            collect_direct_calls_in_expr(inner, out);
        }
    }
}

pub(crate) fn collect_direct_calls_in_stmt(stmt: &HIRStmt, out: &mut HashSet<String>) {
    match stmt {
        HIRStmt::Let { value, .. } => {
            if let Some(value) = value {
                collect_direct_calls_in_expr(value, out);
            }
        }
        HIRStmt::Expr(expr) => collect_direct_calls_in_expr(expr, out),
        HIRStmt::Item => {}
    }
}

pub(crate) fn collect_direct_calls_in_body(body: &HIRBody, out: &mut HashSet<String>) {
    for stmt in &body.stmts {
        collect_direct_calls_in_stmt(stmt, out);
    }
    if let Some(expr) = body.expr.as_deref() {
        collect_direct_calls_in_expr(expr, out);
    }
}

pub(crate) fn collect_direct_call_names(items: &[HIRItem]) -> HashSet<String> {
    let mut out = HashSet::new();
    for item in items {
        match item {
            HIRItem::Function(function) => collect_direct_calls_in_body(&function.body, &mut out),
            HIRItem::Impl(impl_item) => {
                for method in &impl_item.items {
                    collect_direct_calls_in_body(&method.body, &mut out);
                }
            }
            _ => {}
        }
    }
    out
}
