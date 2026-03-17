use crate::hir::{HIRBody, HIRExpr, HIRItem, HIRStmt, HIRType, HIRTypeKind};
use crate::mir::hir_specialization_helpers::hir_type_is_concrete;
use crate::mir::impl_specialization_helpers::expand_impl_variants;
use crate::type_naming::hir_type_instance_name;
use std::collections::{HashMap, HashSet};

pub(crate) fn collect_concrete_named_types_from_type(
    ty: &HIRType,
    known_named_types: &HashSet<String>,
    out: &mut HashMap<String, HIRType>,
) {
    if let HIRTypeKind::Named { args, .. } = &ty.kind {
        if !args.is_empty() && hir_type_is_concrete(ty, known_named_types) {
            out.entry(hir_type_instance_name(ty))
                .or_insert_with(|| ty.clone());
        }
    }

    match &ty.kind {
        HIRTypeKind::Ref(_, inner)
        | HIRTypeKind::Ptr(inner)
        | HIRTypeKind::Slice(inner) => collect_concrete_named_types_from_type(inner, known_named_types, out),
        HIRTypeKind::Array(elem, _) => {
            collect_concrete_named_types_from_type(elem, known_named_types, out)
        }
        HIRTypeKind::Tuple(items) => {
            for item in items {
                collect_concrete_named_types_from_type(item, known_named_types, out);
            }
        }
        HIRTypeKind::Fn { params, ret } => {
            for param in params {
                collect_concrete_named_types_from_type(param, known_named_types, out);
            }
            collect_concrete_named_types_from_type(ret, known_named_types, out);
        }
        HIRTypeKind::Named { args, .. } => {
            for arg in args {
                collect_concrete_named_types_from_type(arg, known_named_types, out);
            }
        }
        _ => {}
    }
}

pub(crate) fn collect_concrete_named_types_from_expr(
    expr: &HIRExpr,
    known_named_types: &HashSet<String>,
    out: &mut HashMap<String, HIRType>,
) {
    match expr {
        HIRExpr::Lit(_) | HIRExpr::Var { .. } | HIRExpr::Continue => {}
        HIRExpr::Unary(_, inner)
        | HIRExpr::Deref(inner)
        | HIRExpr::Ref(_, inner) => {
            collect_concrete_named_types_from_expr(inner, known_named_types, out);
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
            collect_concrete_named_types_from_expr(lhs, known_named_types, out);
            collect_concrete_named_types_from_expr(rhs, known_named_types, out);
        }
        HIRExpr::If {
            cond,
            then_branch,
            else_branch,
        } => {
            collect_concrete_named_types_from_expr(cond, known_named_types, out);
            collect_concrete_named_types_from_body(then_branch, known_named_types, out);
            if let Some(else_branch) = else_branch.as_deref() {
                collect_concrete_named_types_from_body(else_branch, known_named_types, out);
            }
        }
        HIRExpr::Match { scrutinee, arms } => {
            collect_concrete_named_types_from_expr(scrutinee, known_named_types, out);
            for arm in arms {
                if let Some(guard) = arm.guard.as_ref() {
                    collect_concrete_named_types_from_expr(guard, known_named_types, out);
                }
                collect_concrete_named_types_from_expr(&arm.body, known_named_types, out);
            }
        }
        HIRExpr::Loop(body) | HIRExpr::Block(body) => {
            collect_concrete_named_types_from_body(body, known_named_types, out);
        }
        HIRExpr::While { cond, body } => {
            collect_concrete_named_types_from_expr(cond, known_named_types, out);
            collect_concrete_named_types_from_body(body, known_named_types, out);
        }
        HIRExpr::For { iter, body, .. } => {
            collect_concrete_named_types_from_expr(iter, known_named_types, out);
            collect_concrete_named_types_from_body(body, known_named_types, out);
        }
        HIRExpr::Call { func, args } => {
            collect_concrete_named_types_from_expr(func, known_named_types, out);
            for arg in args {
                collect_concrete_named_types_from_expr(arg, known_named_types, out);
            }
        }
        HIRExpr::MethodCall { receiver, args, .. } => {
            collect_concrete_named_types_from_expr(receiver, known_named_types, out);
            for arg in args {
                collect_concrete_named_types_from_expr(arg, known_named_types, out);
            }
        }
        HIRExpr::Struct { fields, .. } => {
            for (_, value) in fields {
                collect_concrete_named_types_from_expr(value, known_named_types, out);
            }
        }
        HIRExpr::Array(items) | HIRExpr::Tuple(items) => {
            for item in items {
                collect_concrete_named_types_from_expr(item, known_named_types, out);
            }
        }
        HIRExpr::Index { base, index } => {
            collect_concrete_named_types_from_expr(base, known_named_types, out);
            collect_concrete_named_types_from_expr(index, known_named_types, out);
        }
        HIRExpr::Field { base, .. } => {
            collect_concrete_named_types_from_expr(base, known_named_types, out);
        }
        HIRExpr::Return(value) | HIRExpr::Break(value) => {
            if let Some(value) = value.as_deref() {
                collect_concrete_named_types_from_expr(value, known_named_types, out);
            }
        }
        HIRExpr::Cast(inner, ty) | HIRExpr::Ascribe(inner, ty) => {
            collect_concrete_named_types_from_expr(inner, known_named_types, out);
            collect_concrete_named_types_from_type(ty, known_named_types, out);
        }
        HIRExpr::Range { start, end, .. } => {
            if let Some(start) = start.as_deref() {
                collect_concrete_named_types_from_expr(start, known_named_types, out);
            }
            if let Some(end) = end.as_deref() {
                collect_concrete_named_types_from_expr(end, known_named_types, out);
            }
        }
        HIRExpr::Lambda { body, .. } => {
            collect_concrete_named_types_from_expr(body, known_named_types, out);
        }
        HIRExpr::Await(inner) => {
            collect_concrete_named_types_from_expr(inner, known_named_types, out);
        }
        HIRExpr::AsyncBlock(body) => {
            collect_concrete_named_types_from_body(body, known_named_types, out);
        }
    }
}

pub(crate) fn collect_concrete_named_types_from_body(
    body: &HIRBody,
    known_named_types: &HashSet<String>,
    out: &mut HashMap<String, HIRType>,
) {
    for stmt in &body.stmts {
        match stmt {
            HIRStmt::Let { ty, value, .. } => {
                collect_concrete_named_types_from_type(ty, known_named_types, out);
                if let Some(value) = value {
                    collect_concrete_named_types_from_expr(value, known_named_types, out);
                }
            }
            HIRStmt::Expr(expr) => {
                collect_concrete_named_types_from_expr(expr, known_named_types, out)
            }
            HIRStmt::Item => {}
        }
    }
    if let Some(expr) = body.expr.as_deref() {
        collect_concrete_named_types_from_expr(expr, known_named_types, out);
    }
}

pub(crate) fn collect_concrete_named_types_from_items(
    items: &[HIRItem],
    known_named_types: &HashSet<String>,
) -> HashMap<String, HIRType> {
    let mut out = HashMap::new();
    for item in items {
        match item {
            HIRItem::Function(function) => {
                for param in &function.params {
                    collect_concrete_named_types_from_type(&param.ty, known_named_types, &mut out);
                }
                collect_concrete_named_types_from_type(
                    &function.return_type,
                    known_named_types,
                    &mut out,
                );
                if let Some(pre) = function.precondition.as_ref() {
                    collect_concrete_named_types_from_expr(pre, known_named_types, &mut out);
                }
                if let Some(post) = function.postcondition.as_ref() {
                    collect_concrete_named_types_from_expr(post, known_named_types, &mut out);
                }
                collect_concrete_named_types_from_body(&function.body, known_named_types, &mut out);
            }
            HIRItem::Impl(impl_item) => {
                collect_concrete_named_types_from_type(
                    &impl_item.target_type,
                    known_named_types,
                    &mut out,
                );
                for method in &impl_item.items {
                    for param in &method.params {
                        collect_concrete_named_types_from_type(&param.ty, known_named_types, &mut out);
                    }
                    collect_concrete_named_types_from_type(
                        &method.return_type,
                        known_named_types,
                        &mut out,
                    );
                    collect_concrete_named_types_from_body(&method.body, known_named_types, &mut out);
                }
            }
            HIRItem::Struct(struct_item) => {
                for field in &struct_item.fields {
                    collect_concrete_named_types_from_type(&field.ty, known_named_types, &mut out);
                }
            }
            _ => {}
        }
    }
    out
}

pub(crate) fn collect_concrete_named_types_with_impl_variants(
    items: &[HIRItem],
    known_named_types: &HashSet<String>,
) -> HashMap<String, HIRType> {
    let mut out = collect_concrete_named_types_from_items(items, known_named_types);

    loop {
        let before_len = out.len();
        for item in items {
            if let HIRItem::Impl(impl_item) = item {
                for expanded_impl in expand_impl_variants(impl_item, &out, known_named_types) {
                    collect_concrete_named_types_from_impl(
                        &expanded_impl,
                        known_named_types,
                        &mut out,
                    );
                }
            }
        }

        if out.len() == before_len {
            break;
        }
    }

    out
}

pub(crate) fn collect_concrete_named_types_from_impl(
    impl_item: &crate::hir::HIRImpl,
    known_named_types: &HashSet<String>,
    out: &mut HashMap<String, HIRType>,
) {
    collect_concrete_named_types_from_type(&impl_item.target_type, known_named_types, out);
    for method in &impl_item.items {
        for param in &method.params {
            collect_concrete_named_types_from_type(&param.ty, known_named_types, out);
        }
        collect_concrete_named_types_from_type(&method.return_type, known_named_types, out);
        collect_concrete_named_types_from_body(&method.body, known_named_types, out);
    }
}
