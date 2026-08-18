use crate::GenericInstanceFingerprint;
use sengoo_compiler::{Expr, ExprKind, Stmt, StmtKind};
use std::collections::{HashMap, HashSet};

use super::super::function_fingerprints::call_target_signature;
use super::super::generic_items::GenericCallableMeta;
use super::super::signature::type_signature;
use super::{
    infer_expr_type_signature_with_methods, push_instance_if_generic_call,
    push_instance_if_generic_method_call,
};

#[allow(clippy::too_many_arguments)]
fn collect_generic_instances_in_block(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    stmts: &[Stmt],
    local_types: &HashMap<String, String>,
    simple_to_symbol: &HashMap<String, Option<String>>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) {
    let mut scoped_locals = local_types.clone();
    for stmt in stmts {
        collect_generic_instances_in_stmt(
            out,
            seen,
            module_path,
            stmt,
            &mut scoped_locals,
            simple_to_symbol,
            method_to_symbols,
            callable_meta,
        );
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_generic_instances_in_expr(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    expr: &Expr,
    local_types: &HashMap<String, String>,
    simple_to_symbol: &HashMap<String, Option<String>>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) {
    match &expr.kind {
        ExprKind::Literal(_) | ExprKind::Ident(_) | ExprKind::Path(_) | ExprKind::Continue => {}
        ExprKind::Unary { operand, .. }
        | ExprKind::Await(operand)
        | ExprKind::Try(operand)
        | ExprKind::Paren(operand) => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                operand,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Assign {
            target: left,
            value: right,
        }
        | ExprKind::AssignOp {
            target: left,
            value: right,
            ..
        }
        | ExprKind::Index {
            base: left,
            index: right,
        } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                left,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                right,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::Call { func, args } => {
            if let Some(target) = call_target_signature(func) {
                push_instance_if_generic_call(
                    out,
                    seen,
                    module_path,
                    &target,
                    args,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                func,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            for arg in args {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    arg,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            push_instance_if_generic_method_call(
                out,
                seen,
                module_path,
                receiver,
                &method.name,
                args,
                local_types,
                method_to_symbols,
                callable_meta,
            );
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                receiver,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            for arg in args {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    arg,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Block(block)
        | ExprKind::Loop(block)
        | ExprKind::AsyncBlock(block)
        | ExprKind::ParallelBlock(block)
        | ExprKind::TryBlock(block) => {
            collect_generic_instances_in_block(
                out,
                seen,
                module_path,
                &block.stmts,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                cond,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            collect_generic_instances_in_block(
                out,
                seen,
                module_path,
                &then_branch.stmts,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            if let Some(else_expr) = else_branch.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    else_expr,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::IfLet {
            expr,
            then_branch,
            else_branch,
            ..
        } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                expr,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            collect_generic_instances_in_block(
                out,
                seen,
                module_path,
                &then_branch.stmts,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            if let Some(else_expr) = else_branch.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    else_expr,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::While { cond, body } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                cond,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            collect_generic_instances_in_block(
                out,
                seen,
                module_path,
                &body.stmts,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::For { iter, body, .. } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                iter,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            collect_generic_instances_in_block(
                out,
                seen,
                module_path,
                &body.stmts,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::Match { scrutinee, arms } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                scrutinee,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            for arm in arms {
                if let Some(guard) = arm.guard.as_deref() {
                    collect_generic_instances_in_expr(
                        out,
                        seen,
                        module_path,
                        guard,
                        local_types,
                        simple_to_symbol,
                        method_to_symbols,
                        callable_meta,
                    );
                }
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    &arm.body,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Return(value) | ExprKind::Break(value) | ExprKind::Yield(value) => {
            if let Some(value) = value.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    value,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Field { base, .. } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                base,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::Array(elements) | ExprKind::Tuple(elements) => {
            for elem in elements {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    elem,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::VecBang { elements, count } => {
            for elem in elements {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    elem,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
            if let Some(count) = count.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    count,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Struct { fields, base, .. } => {
            for field in fields {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    &field.value,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
            if let Some(base) = base.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    base,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Range { start, end, .. } => {
            if let Some(start) = start.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    start,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
            if let Some(end) = end.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    end,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
        }
        ExprKind::Lambda { body, .. } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                body,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
        ExprKind::Cast { expr, .. } | ExprKind::Is { expr, .. } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                expr,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn collect_generic_instances_in_stmt(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    stmt: &Stmt,
    local_types: &mut HashMap<String, String>,
    simple_to_symbol: &HashMap<String, Option<String>>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) {
    match &stmt.kind {
        StmtKind::Let {
            name, ty, value, ..
        } => {
            if let Some(value) = value.as_deref() {
                collect_generic_instances_in_expr(
                    out,
                    seen,
                    module_path,
                    value,
                    local_types,
                    simple_to_symbol,
                    method_to_symbols,
                    callable_meta,
                );
            }
            let inferred = ty.as_ref().map(type_signature).or_else(|| {
                value.as_deref().map(|expr| {
                    infer_expr_type_signature_with_methods(
                        expr,
                        local_types,
                        method_to_symbols,
                        callable_meta,
                    )
                })
            });
            if let Some(inferred) = inferred.filter(|ty| ty != "_") {
                local_types.insert(name.name.clone(), inferred);
            }
        }
        StmtKind::Const { name, ty, value } => {
            collect_generic_instances_in_expr(
                out,
                seen,
                module_path,
                value,
                local_types,
                simple_to_symbol,
                method_to_symbols,
                callable_meta,
            );
            local_types.insert(name.name.clone(), type_signature(ty));
        }
        StmtKind::Expr(expr) => collect_generic_instances_in_expr(
            out,
            seen,
            module_path,
            expr,
            local_types,
            simple_to_symbol,
            method_to_symbols,
            callable_meta,
        ),
        StmtKind::Item(_) => {}
    }
}
