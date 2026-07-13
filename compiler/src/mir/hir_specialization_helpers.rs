use crate::hir::{self, HIRBody, HIRExpr, HIRMatchArm, HIRStmt, HIRType, HIRTypeKind};
use std::collections::{HashMap, HashSet};

pub(crate) fn hir_type_is_placeholder_name(
    ty: &HIRType,
    known_named_types: &HashSet<String>,
) -> Option<String> {
    match &ty.kind {
        HIRTypeKind::Named { name, args }
            if args.is_empty() && !known_named_types.contains(name) =>
        {
            Some(name.clone())
        }
        _ => None,
    }
}

pub(crate) fn hir_type_is_concrete(ty: &HIRType, known_named_types: &HashSet<String>) -> bool {
    match &ty.kind {
        HIRTypeKind::Unit
        | HIRTypeKind::Never
        | HIRTypeKind::Bool
        | HIRTypeKind::Char
        | HIRTypeKind::Str
        | HIRTypeKind::Byte
        | HIRTypeKind::Bytes
        | HIRTypeKind::Int(_)
        | HIRTypeKind::Float(_) => true,
        HIRTypeKind::Ref(_, inner) | HIRTypeKind::Ptr(inner) | HIRTypeKind::Slice(inner) => {
            hir_type_is_concrete(inner, known_named_types)
        }
        HIRTypeKind::Array(elem, _) => hir_type_is_concrete(elem, known_named_types),
        HIRTypeKind::Tuple(items) => items
            .iter()
            .all(|item| hir_type_is_concrete(item, known_named_types)),
        HIRTypeKind::Fn { params, ret } => {
            params
                .iter()
                .all(|param| hir_type_is_concrete(param, known_named_types))
                && hir_type_is_concrete(ret, known_named_types)
        }
        HIRTypeKind::Named { name, args } => {
            known_named_types.contains(name)
                && args
                    .iter()
                    .all(|arg| hir_type_is_concrete(arg, known_named_types))
        }
        _ => false,
    }
}

pub(crate) fn substitute_hir_type(ty: &HIRType, subst: &HashMap<String, HIRType>) -> HIRType {
    if let HIRTypeKind::Named { name, args } = &ty.kind {
        if args.is_empty() {
            if let Some(replacement) = subst.get(name) {
                return replacement.clone();
            }
        }
    }

    match &ty.kind {
        HIRTypeKind::Ref(is_mut, inner) => {
            HIRType::reference(*is_mut, substitute_hir_type(inner, subst))
        }
        HIRTypeKind::Ptr(inner) => HIRType::pointer(substitute_hir_type(inner, subst)),
        HIRTypeKind::Array(elem, len) => HIRType::array(substitute_hir_type(elem, subst), *len),
        HIRTypeKind::Slice(elem) => HIRType::slice(substitute_hir_type(elem, subst)),
        HIRTypeKind::Tuple(items) => HIRType::tuple(
            items
                .iter()
                .map(|item| substitute_hir_type(item, subst))
                .collect(),
        ),
        HIRTypeKind::Fn { params, ret } => HIRType::function(
            params
                .iter()
                .map(|param| substitute_hir_type(param, subst))
                .collect(),
            Box::new(substitute_hir_type(ret, subst)),
        ),
        HIRTypeKind::Named { name, args } => HIRType::named(
            name.clone(),
            args.iter()
                .map(|arg| substitute_hir_type(arg, subst))
                .collect(),
        ),
        HIRTypeKind::AssocProjection {
            base,
            trait_name,
            name,
        } => {
            if let HIRTypeKind::Named {
                name: base_name,
                args,
            } = &base.kind
            {
                if args.is_empty() {
                    if let Some(replacement) =
                        subst.get(&format!("<{base_name} as {trait_name}>::{name}"))
                    {
                        return replacement.clone();
                    }
                }
            }
            HIRType::new(HIRTypeKind::AssocProjection {
                base: Box::new(substitute_hir_type(base, subst)),
                trait_name: trait_name.clone(),
                name: name.clone(),
            })
        }
        _ => ty.clone(),
    }
}

pub(crate) fn substitute_hir_expr(expr: &HIRExpr, subst: &HashMap<String, HIRType>) -> HIRExpr {
    match expr {
        HIRExpr::Lit(_) | HIRExpr::Var { .. } | HIRExpr::Continue => expr.clone(),
        HIRExpr::Unary(op, inner) => {
            HIRExpr::Unary(*op, Box::new(substitute_hir_expr(inner, subst)))
        }
        HIRExpr::Binary(op, lhs, rhs) => HIRExpr::Binary(
            *op,
            Box::new(substitute_hir_expr(lhs, subst)),
            Box::new(substitute_hir_expr(rhs, subst)),
        ),
        HIRExpr::And(lhs, rhs) => HIRExpr::And(
            Box::new(substitute_hir_expr(lhs, subst)),
            Box::new(substitute_hir_expr(rhs, subst)),
        ),
        HIRExpr::Or(lhs, rhs) => HIRExpr::Or(
            Box::new(substitute_hir_expr(lhs, subst)),
            Box::new(substitute_hir_expr(rhs, subst)),
        ),
        HIRExpr::If {
            cond,
            then_branch,
            else_branch,
        } => HIRExpr::If {
            cond: Box::new(substitute_hir_expr(cond, subst)),
            then_branch: Box::new(substitute_hir_body(then_branch, subst)),
            else_branch: else_branch
                .as_ref()
                .map(|body| Box::new(substitute_hir_body(body, subst))),
        },
        HIRExpr::Match { scrutinee, arms } => HIRExpr::Match {
            scrutinee: Box::new(substitute_hir_expr(scrutinee, subst)),
            arms: arms
                .iter()
                .map(|arm| HIRMatchArm {
                    pat: arm.pat.clone(),
                    guard: arm
                        .guard
                        .as_ref()
                        .map(|guard| Box::new(substitute_hir_expr(guard, subst))),
                    body: Box::new(substitute_hir_expr(&arm.body, subst)),
                })
                .collect(),
        },
        HIRExpr::Loop(body) => HIRExpr::Loop(Box::new(substitute_hir_body(body, subst))),
        HIRExpr::While { cond, body } => HIRExpr::While {
            cond: Box::new(substitute_hir_expr(cond, subst)),
            body: Box::new(substitute_hir_body(body, subst)),
        },
        HIRExpr::For {
            var_name,
            var_symbol,
            iter,
            body,
        } => HIRExpr::For {
            var_name: var_name.clone(),
            var_symbol: *var_symbol,
            iter: Box::new(substitute_hir_expr(iter, subst)),
            body: Box::new(substitute_hir_body(body, subst)),
        },
        HIRExpr::Call {
            func,
            args,
            site_lo,
            expected_return_type,
        } => HIRExpr::Call {
            func: Box::new(substitute_hir_expr(func, subst)),
            args: args
                .iter()
                .map(|arg| substitute_hir_expr(arg, subst))
                .collect(),
            site_lo: *site_lo,
            expected_return_type: expected_return_type
                .as_ref()
                .map(|ty| substitute_hir_type(ty, subst)),
        },
        HIRExpr::EnumConstruct {
            enum_name,
            variant_name,
            discriminant,
            args,
        } => HIRExpr::EnumConstruct {
            enum_name: enum_name.clone(),
            variant_name: variant_name.clone(),
            discriminant: *discriminant,
            args: args
                .iter()
                .map(|arg| substitute_hir_expr(arg, subst))
                .collect(),
        },
        HIRExpr::MethodCall {
            receiver,
            method,
            args,
            expected_return_type,
        } => HIRExpr::MethodCall {
            receiver: Box::new(substitute_hir_expr(receiver, subst)),
            method: method.clone(),
            args: args
                .iter()
                .map(|arg| substitute_hir_expr(arg, subst))
                .collect(),
            expected_return_type: expected_return_type
                .as_ref()
                .map(|ty| substitute_hir_type(ty, subst)),
        },
        HIRExpr::Struct {
            name,
            fields,
            concrete_type,
        } => HIRExpr::Struct {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(field, value)| (field.clone(), substitute_hir_expr(value, subst)))
                .collect(),
            concrete_type: concrete_type
                .as_ref()
                .map(|ty| substitute_hir_type(ty, subst)),
        },
        HIRExpr::Array(items) => HIRExpr::Array(
            items
                .iter()
                .map(|item| substitute_hir_expr(item, subst))
                .collect(),
        ),
        HIRExpr::Index { base, index } => HIRExpr::Index {
            base: Box::new(substitute_hir_expr(base, subst)),
            index: Box::new(substitute_hir_expr(index, subst)),
        },
        HIRExpr::Field { base, field } => HIRExpr::Field {
            base: Box::new(substitute_hir_expr(base, subst)),
            field: field.clone(),
        },
        HIRExpr::Assign { target, value } => HIRExpr::Assign {
            target: Box::new(substitute_hir_expr(target, subst)),
            value: Box::new(substitute_hir_expr(value, subst)),
        },
        HIRExpr::AssignOp { target, op, value } => HIRExpr::AssignOp {
            target: Box::new(substitute_hir_expr(target, subst)),
            op: *op,
            value: Box::new(substitute_hir_expr(value, subst)),
        },
        HIRExpr::Return(value) => HIRExpr::Return(
            value
                .as_ref()
                .map(|value| Box::new(substitute_hir_expr(value, subst))),
        ),
        HIRExpr::Break(value) => HIRExpr::Break(
            value
                .as_ref()
                .map(|value| Box::new(substitute_hir_expr(value, subst))),
        ),
        HIRExpr::Block(body) => HIRExpr::Block(Box::new(substitute_hir_body(body, subst))),
        HIRExpr::Cast(inner, ty) => HIRExpr::Cast(
            Box::new(substitute_hir_expr(inner, subst)),
            substitute_hir_type(ty, subst),
        ),
        HIRExpr::Ascribe(inner, ty) => HIRExpr::Ascribe(
            Box::new(substitute_hir_expr(inner, subst)),
            substitute_hir_type(ty, subst),
        ),
        HIRExpr::Ref(is_mut, inner) => {
            HIRExpr::Ref(*is_mut, Box::new(substitute_hir_expr(inner, subst)))
        }
        HIRExpr::Deref(inner) => HIRExpr::Deref(Box::new(substitute_hir_expr(inner, subst))),
        HIRExpr::Range {
            start,
            end,
            inclusive,
        } => HIRExpr::Range {
            start: start
                .as_ref()
                .map(|value| Box::new(substitute_hir_expr(value, subst))),
            end: end
                .as_ref()
                .map(|value| Box::new(substitute_hir_expr(value, subst))),
            inclusive: *inclusive,
        },
        HIRExpr::Tuple(items) => HIRExpr::Tuple(
            items
                .iter()
                .map(|item| substitute_hir_expr(item, subst))
                .collect(),
        ),
        HIRExpr::Lambda { params, body } => HIRExpr::Lambda {
            params: params.clone(),
            body: Box::new(substitute_hir_expr(body, subst)),
        },
        HIRExpr::Await(inner) => HIRExpr::Await(Box::new(substitute_hir_expr(inner, subst))),
        HIRExpr::AsyncBlock(body) => {
            HIRExpr::AsyncBlock(Box::new(substitute_hir_body(body, subst)))
        }
        HIRExpr::Try(inner) => HIRExpr::Try(Box::new(substitute_hir_expr(inner, subst))),
        HIRExpr::TryBlock(body) => HIRExpr::TryBlock(Box::new(substitute_hir_body(body, subst))),
    }
}

pub(crate) fn substitute_hir_stmt(stmt: &HIRStmt, subst: &HashMap<String, HIRType>) -> HIRStmt {
    match stmt {
        HIRStmt::Source { site_lo } => HIRStmt::Source { site_lo: *site_lo },
        HIRStmt::Coverage { site_lo } => HIRStmt::Coverage { site_lo: *site_lo },
        HIRStmt::Let {
            name,
            symbol,
            ty,
            value,
            is_mut,
        } => HIRStmt::Let {
            name: name.clone(),
            symbol: *symbol,
            ty: substitute_hir_type(ty, subst),
            value: value
                .as_ref()
                .map(|value| substitute_hir_expr(value, subst)),
            is_mut: *is_mut,
        },
        HIRStmt::Expr(expr) => HIRStmt::Expr(substitute_hir_expr(expr, subst)),
        HIRStmt::Item => HIRStmt::Item,
    }
}

pub(crate) fn substitute_hir_body(body: &HIRBody, subst: &HashMap<String, HIRType>) -> HIRBody {
    HIRBody {
        stmts: body
            .stmts
            .iter()
            .map(|stmt| substitute_hir_stmt(stmt, subst))
            .collect(),
        expr: body
            .expr
            .as_ref()
            .map(|expr| Box::new(substitute_hir_expr(expr, subst))),
    }
}

pub(crate) fn substitute_hir_function(
    function: &hir::HIRFunction,
    subst: &HashMap<String, HIRType>,
) -> hir::HIRFunction {
    hir::HIRFunction {
        name: function.name.clone(),
        type_params: function.type_params.clone(),
        params: function
            .params
            .iter()
            .map(|param| param.with_type(substitute_hir_type(&param.ty, subst)))
            .collect(),
        return_type: substitute_hir_type(&function.return_type, subst),
        precondition: function
            .precondition
            .as_ref()
            .map(|expr| substitute_hir_expr(expr, subst)),
        postcondition: function
            .postcondition
            .as_ref()
            .map(|expr| substitute_hir_expr(expr, subst)),
        body: substitute_hir_body(&function.body, subst),
        is_async: function.is_async,
        abi: function.abi.clone(),
        is_unsafe: function.is_unsafe,
        no_mangle: function.no_mangle,
        export_name: function.export_name.clone(),
        is_pub: function.is_pub,
    }
}
