//! HIR 闁?MIR 闁汇劌瀚ù鍡涙晸?

use crate::hir::{
    self, HIRBody, HIRExpr, HIRItem, HIRLiteral, HIRParam, HIRStmt, HIRType, HIRTypeKind,
};
use crate::hir::{HIRTrait, HIRTraitItem};
use crate::method_resolution::{
    ambiguous_method_error, explicit_hir_method_param_count, explicit_hir_method_params,
    select_method_candidate, MethodCandidate, MethodCandidateMatch,
};
use crate::mir::{
    Instruction, Local, LocalKind, MIRType, MirBinOp, MirConstant, MirFunction, MirUnOp,
    Terminator, MIR_BOOL, MIR_I64, MIR_UNIT,
};
use super::generic_methods::{
    collect_inherent_method_templates, collect_trait_method_templates_for_impl,
    ConcreteTypeRegistry, InherentMethodTemplate, TraitMethodTemplate,
};
use crate::symbol::SymbolId;
use std::collections::{HashMap, HashSet};

/// 闁?HIRType 閺夌儐鍓氬畷鍙夌▔閾忕顫﹂柛銊ヮ儏婢х姷绱撻埀顒傗偓娑欘殘椤戜焦绋夌拠褏绀勯柣顫妺缁剟寮憴鍕€婇柛姘С閹便劍顨滅敮顔剧
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MirLowerOptions {
    pub runtime_contract_checks: bool,
    pub lazy_generic_mono: bool,
    pub async_functions: HashSet<String>,
}

impl Default for MirLowerOptions {
    fn default() -> Self {
        Self {
            runtime_contract_checks: false,
            lazy_generic_mono: true,
            async_functions: HashSet::new(),
        }
    }
}

fn mir_local_name(local: Local) -> String {
    match local.kind {
        LocalKind::Param => format!("%l_{}", local.id),
        LocalKind::Temp => format!("%t_{}", local.id),
        LocalKind::User => format!("%u_{}", local.id),
        LocalKind::Return => format!("%ret_{}", local.id),
    }
}

fn hir_type_to_prefix(ty: &HIRType) -> String {
    match &ty.kind {
        HIRTypeKind::Int(ik) => format!("i{}", ik.bits()),
        HIRTypeKind::Float(fk) => format!("f{}", fk.bits()),
        HIRTypeKind::Bool => "bool".to_string(),
        HIRTypeKind::Unit => "unit".to_string(),
        HIRTypeKind::Named { name, .. } => name.clone(),
        _ => "unknown".to_string(),
    }
}

fn hir_type_to_instance_name(ty: &HIRType) -> String {
    match &ty.kind {
        HIRTypeKind::Int(ik) => format!("i{}", ik.bits()),
        HIRTypeKind::Float(fk) => format!("f{}", fk.bits()),
        HIRTypeKind::Bool => "bool".to_string(),
        HIRTypeKind::Unit => "unit".to_string(),
        HIRTypeKind::Str => "str".to_string(),
        HIRTypeKind::Ref(_, inner) => format!("ref_{}", hir_type_to_instance_name(inner)),
        HIRTypeKind::Ptr(inner) => format!("ptr_{}", hir_type_to_instance_name(inner)),
        HIRTypeKind::Array(elem, len) => format!("array_{}_{}", len, hir_type_to_instance_name(elem)),
        HIRTypeKind::Tuple(items) => {
            let parts: Vec<String> = items.iter().map(hir_type_to_instance_name).collect();
            format!("tuple_{}", parts.join("_"))
        }
        HIRTypeKind::Named { name, args } => {
            if args.is_empty() {
                name.clone()
            } else {
                let parts: Vec<String> = args.iter().map(hir_type_to_instance_name).collect();
                format!("{}_{}", name, parts.join("_"))
            }
        }
        _ => hir_type_to_prefix(ty),
    }
}

fn mir_type_to_instance_name(ty: &MIRType) -> String {
    match ty {
        MIRType::Int(bits) => format!("i{}", bits),
        MIRType::Float(bits) => format!("f{}", bits),
        MIRType::Bool => "bool".to_string(),
        MIRType::Unit => "unit".to_string(),
        MIRType::Ref(inner) => format!("ref_{}", mir_type_to_instance_name(inner)),
        MIRType::Ptr(inner) => format!("ptr_{}", mir_type_to_instance_name(inner)),
        MIRType::Array(elem, len) => format!("array_{}_{}", len, mir_type_to_instance_name(elem)),
        MIRType::Tuple(items) => {
            let parts: Vec<String> = items.iter().map(mir_type_to_instance_name).collect();
            format!("tuple_{}", parts.join("_"))
        }
        MIRType::Struct { name, .. } => name.clone(),
        _ => "unknown".to_string(),
    }
}

fn hir_type_is_placeholder_name(
    ty: &HIRType,
    known_named_types: &HashSet<String>,
) -> Option<String> {
    match &ty.kind {
        HIRTypeKind::Named { name, args } if args.is_empty() && !known_named_types.contains(name) => {
            Some(name.clone())
        }
        _ => None,
    }
}

fn hir_type_is_concrete(ty: &HIRType, known_named_types: &HashSet<String>) -> bool {
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
        HIRTypeKind::Ref(_, inner)
        | HIRTypeKind::Ptr(inner)
        | HIRTypeKind::Slice(inner) => hir_type_is_concrete(inner, known_named_types),
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

fn substitute_hir_type(ty: &HIRType, subst: &HashMap<String, HIRType>) -> HIRType {
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
            args.iter().map(|arg| substitute_hir_type(arg, subst)).collect(),
        ),
        _ => ty.clone(),
    }
}

fn substitute_hir_expr(expr: &HIRExpr, subst: &HashMap<String, HIRType>) -> HIRExpr {
    match expr {
        HIRExpr::Lit(_) | HIRExpr::Var { .. } | HIRExpr::Continue => expr.clone(),
        HIRExpr::Unary(op, inner) => HIRExpr::Unary(*op, Box::new(substitute_hir_expr(inner, subst))),
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
                .map(|arm| hir::HIRMatchArm {
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
        HIRExpr::Call { func, args } => HIRExpr::Call {
            func: Box::new(substitute_hir_expr(func, subst)),
            args: args.iter().map(|arg| substitute_hir_expr(arg, subst)).collect(),
        },
        HIRExpr::MethodCall {
            receiver,
            method,
            args,
        } => HIRExpr::MethodCall {
            receiver: Box::new(substitute_hir_expr(receiver, subst)),
            method: method.clone(),
            args: args.iter().map(|arg| substitute_hir_expr(arg, subst)).collect(),
        },
        HIRExpr::Struct { name, fields } => HIRExpr::Struct {
            name: name.clone(),
            fields: fields
                .iter()
                .map(|(field, value)| (field.clone(), substitute_hir_expr(value, subst)))
                .collect(),
        },
        HIRExpr::Array(items) => HIRExpr::Array(
            items.iter().map(|item| substitute_hir_expr(item, subst)).collect(),
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
            value.as_ref().map(|value| Box::new(substitute_hir_expr(value, subst))),
        ),
        HIRExpr::Break(value) => HIRExpr::Break(
            value.as_ref().map(|value| Box::new(substitute_hir_expr(value, subst))),
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
        HIRExpr::Ref(is_mut, inner) => HIRExpr::Ref(*is_mut, Box::new(substitute_hir_expr(inner, subst))),
        HIRExpr::Deref(inner) => HIRExpr::Deref(Box::new(substitute_hir_expr(inner, subst))),
        HIRExpr::Range {
            start,
            end,
            inclusive,
        } => HIRExpr::Range {
            start: start.as_ref().map(|value| Box::new(substitute_hir_expr(value, subst))),
            end: end.as_ref().map(|value| Box::new(substitute_hir_expr(value, subst))),
            inclusive: *inclusive,
        },
        HIRExpr::Tuple(items) => HIRExpr::Tuple(
            items.iter().map(|item| substitute_hir_expr(item, subst)).collect(),
        ),
        HIRExpr::Lambda { params, body } => HIRExpr::Lambda {
            params: params.clone(),
            body: Box::new(substitute_hir_expr(body, subst)),
        },
        HIRExpr::Await(inner) => HIRExpr::Await(Box::new(substitute_hir_expr(inner, subst))),
        HIRExpr::AsyncBlock(body) => {
            HIRExpr::AsyncBlock(Box::new(substitute_hir_body(body, subst)))
        }
    }
}

fn substitute_hir_stmt(stmt: &HIRStmt, subst: &HashMap<String, HIRType>) -> HIRStmt {
    match stmt {
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
            value: value.as_ref().map(|value| substitute_hir_expr(value, subst)),
            is_mut: *is_mut,
        },
        HIRStmt::Expr(expr) => HIRStmt::Expr(substitute_hir_expr(expr, subst)),
        HIRStmt::Item => HIRStmt::Item,
    }
}

fn substitute_hir_body(body: &HIRBody, subst: &HashMap<String, HIRType>) -> HIRBody {
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

fn substitute_hir_function(
    function: &hir::HIRFunction,
    subst: &HashMap<String, HIRType>,
) -> hir::HIRFunction {
    hir::HIRFunction {
        name: function.name.clone(),
        type_params: function.type_params.clone(),
        params: function
            .params
            .iter()
            .map(|param| HIRParam::new(
                param.name.clone(),
                param.symbol,
                substitute_hir_type(&param.ty, subst),
            ))
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

fn collect_concrete_named_types_from_type(
    ty: &HIRType,
    known_named_types: &HashSet<String>,
    out: &mut HashMap<String, HIRType>,
) {
    if let HIRTypeKind::Named { args, .. } = &ty.kind {
        if !args.is_empty() && hir_type_is_concrete(ty, known_named_types) {
            out.entry(hir_type_to_instance_name(ty))
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

fn collect_concrete_named_types_from_expr(
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

fn collect_concrete_named_types_from_body(
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

fn collect_concrete_named_types_from_items(
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

fn collect_concrete_named_types_from_impl(
    impl_item: &hir::HIRImpl,
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

fn collect_concrete_named_types_closure(
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

fn match_generic_impl_target(
    template: &HIRType,
    concrete: &HIRType,
    known_named_types: &HashSet<String>,
    subst: &mut HashMap<String, HIRType>,
) -> bool {
    if let Some(name) = hir_type_is_placeholder_name(template, known_named_types) {
        match subst.get(&name) {
            Some(existing) => existing == concrete,
            None => {
                subst.insert(name, concrete.clone());
                true
            }
        }
    } else {
        match (&template.kind, &concrete.kind) {
            (HIRTypeKind::Unit, HIRTypeKind::Unit)
            | (HIRTypeKind::Never, HIRTypeKind::Never)
            | (HIRTypeKind::Bool, HIRTypeKind::Bool)
            | (HIRTypeKind::Char, HIRTypeKind::Char)
            | (HIRTypeKind::Str, HIRTypeKind::Str)
            | (HIRTypeKind::Byte, HIRTypeKind::Byte)
            | (HIRTypeKind::Bytes, HIRTypeKind::Bytes) => true,
            (HIRTypeKind::Int(lhs), HIRTypeKind::Int(rhs)) => lhs == rhs,
            (HIRTypeKind::Float(lhs), HIRTypeKind::Float(rhs)) => lhs == rhs,
            (HIRTypeKind::Ref(lhs_mut, lhs), HIRTypeKind::Ref(rhs_mut, rhs)) => {
                lhs_mut == rhs_mut && match_generic_impl_target(lhs, rhs, known_named_types, subst)
            }
            (HIRTypeKind::Ptr(lhs), HIRTypeKind::Ptr(rhs))
            | (HIRTypeKind::Slice(lhs), HIRTypeKind::Slice(rhs)) => {
                match_generic_impl_target(lhs, rhs, known_named_types, subst)
            }
            (HIRTypeKind::Array(lhs, lhs_len), HIRTypeKind::Array(rhs, rhs_len)) => {
                lhs_len == rhs_len && match_generic_impl_target(lhs, rhs, known_named_types, subst)
            }
            (HIRTypeKind::Tuple(lhs), HIRTypeKind::Tuple(rhs)) => {
                lhs.len() == rhs.len()
                    && lhs.iter().zip(rhs.iter()).all(|(lhs, rhs)| {
                        match_generic_impl_target(lhs, rhs, known_named_types, subst)
                    })
            }
            (
                HIRTypeKind::Fn { params: lhs_params, ret: lhs_ret },
                HIRTypeKind::Fn { params: rhs_params, ret: rhs_ret },
            ) => {
                lhs_params.len() == rhs_params.len()
                    && lhs_params.iter().zip(rhs_params.iter()).all(|(lhs, rhs)| {
                        match_generic_impl_target(lhs, rhs, known_named_types, subst)
                    })
                    && match_generic_impl_target(lhs_ret, rhs_ret, known_named_types, subst)
            }
            (
                HIRTypeKind::Named {
                    name: lhs_name,
                    args: lhs_args,
                },
                HIRTypeKind::Named {
                    name: rhs_name,
                    args: rhs_args,
                },
            ) => {
                lhs_name == rhs_name
                    && lhs_args.len() == rhs_args.len()
                    && lhs_args.iter().zip(rhs_args.iter()).all(|(lhs, rhs)| {
                        match_generic_impl_target(lhs, rhs, known_named_types, subst)
                    })
            }
            _ => false,
        }
    }
}

fn impl_type_prefix(ty: &HIRType) -> String {
    match &ty.kind {
        HIRTypeKind::Named { args, .. } if !args.is_empty() => hir_type_to_instance_name(ty),
        _ => hir_type_to_prefix(ty),
    }
}

fn instantiate_impl_method(
    method: &hir::HIRFunction,
    legacy_prefix: &str,
    concrete_prefix: &str,
    subst: &HashMap<String, HIRType>,
) -> hir::HIRFunction {
    let mut method = substitute_hir_function(method, subst);
    let suffix = method
        .name
        .strip_prefix(&format!("{}_", legacy_prefix))
        .unwrap_or(&method.name)
        .to_string();
    method.name = format!("{}_{}", concrete_prefix, suffix);
    method
}

fn expand_impl_variants(
    impl_item: &hir::HIRImpl,
    concrete_named_types: &HashMap<String, HIRType>,
    known_named_types: &HashSet<String>,
) -> Vec<hir::HIRImpl> {
    let legacy_prefix = hir_type_to_prefix(&impl_item.target_type);
    if hir_type_is_concrete(&impl_item.target_type, known_named_types) {
        let concrete_prefix = impl_type_prefix(&impl_item.target_type);
        return vec![hir::HIRImpl {
            target_type: impl_item.target_type.clone(),
            trait_name: impl_item.trait_name.clone(),
            items: impl_item
                .items
                .iter()
                .map(|method| {
                    instantiate_impl_method(method, &legacy_prefix, &concrete_prefix, &HashMap::new())
                })
                .collect(),
        }];
    }

    let mut variants = Vec::new();
    let mut seen = HashSet::new();
    for concrete in concrete_named_types.values() {
        let mut subst = HashMap::new();
        if match_generic_impl_target(
            &impl_item.target_type,
            concrete,
            known_named_types,
            &mut subst,
        ) {
            let concrete_prefix = impl_type_prefix(concrete);
            if seen.insert(concrete_prefix.clone()) {
                variants.push(hir::HIRImpl {
                    target_type: concrete.clone(),
                    trait_name: impl_item.trait_name.clone(),
                    items: impl_item
                        .items
                        .iter()
                        .map(|method| {
                            instantiate_impl_method(method, &legacy_prefix, &concrete_prefix, &subst)
                        })
                        .collect(),
                });
            }
        }
    }
    variants
}

fn hir_type_to_mir_with_structs_and_subst(
    ty: &HIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    subst: &HashMap<String, MIRType>,
) -> MIRType {
    match &ty.kind {
        HIRTypeKind::Named { name, args } => {
            if args.is_empty() {
                if let Some(replacement) = subst.get(name) {
                    return replacement.clone();
                }
            }

            if let Some(def) = struct_defs.get(name) {
                let mut nested_subst = subst.clone();
                for (type_param, arg) in def.type_params.iter().zip(args.iter()) {
                    nested_subst.insert(
                        type_param.name.clone(),
                        hir_type_to_mir_with_structs_and_subst(arg, struct_defs, subst),
                    );
                }
                let instance_name = if args.is_empty() {
                    name.clone()
                } else {
                    let parts: Vec<String> = args
                        .iter()
                        .map(|arg| {
                            mir_type_to_instance_name(&hir_type_to_mir_with_structs_and_subst(
                                arg,
                                struct_defs,
                                subst,
                            ))
                        })
                        .collect();
                    format!("{}_{}", name, parts.join("_"))
                };
                MIRType::Struct {
                    name: instance_name,
                    fields: def
                        .fields
                        .iter()
                        .map(|field| {
                            (
                                field.name.clone(),
                                hir_type_to_mir_with_structs_and_subst(
                                    &field.ty,
                                    struct_defs,
                                    &nested_subst,
                                ),
                            )
                        })
                        .collect(),
                }
            } else {
                ty.clone().into()
            }
        }
        HIRTypeKind::Str => MIRType::Ptr(Box::new(MIRType::Int(8))),
        HIRTypeKind::Ref(_, inner) if matches!(inner.kind, HIRTypeKind::Str) => {
            MIRType::Ptr(Box::new(MIRType::Int(8)))
        }
        HIRTypeKind::Ref(_, inner) => MIRType::Ref(Box::new(
            hir_type_to_mir_with_structs_and_subst(inner, struct_defs, subst),
        )),
        HIRTypeKind::Ptr(inner) => MIRType::Ptr(Box::new(
            hir_type_to_mir_with_structs_and_subst(inner, struct_defs, subst),
        )),
        HIRTypeKind::Array(elem, len) => MIRType::Array(
            Box::new(hir_type_to_mir_with_structs_and_subst(elem, struct_defs, subst)),
            *len as u64,
        ),
        HIRTypeKind::Tuple(types) => MIRType::Tuple(
            types
                .iter()
                .map(|item| hir_type_to_mir_with_structs_and_subst(item, struct_defs, subst))
                .collect(),
        ),
        HIRTypeKind::Fn { params, ret } => MIRType::Fn {
            params: params
                .iter()
                .map(|item| hir_type_to_mir_with_structs_and_subst(item, struct_defs, subst))
                .collect(),
            ret: Box::new(hir_type_to_mir_with_structs_and_subst(ret, struct_defs, subst)),
        },
        _ => ty.clone().into(),
    }
}

fn hir_type_to_mir_with_structs(
    ty: &HIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
) -> MIRType {
    hir_type_to_mir_with_structs_and_subst(ty, struct_defs, &HashMap::new())
}

fn bind_mir_subst_from_hir_type(
    template: &HIRType,
    actual: &MIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    subst: &mut HashMap<String, MIRType>,
) {
    match &template.kind {
        HIRTypeKind::Named { name, args } if args.is_empty() && !struct_defs.contains_key(name) => {
            match subst.get(name) {
                Some(existing) if existing != actual => {}
                Some(_) => {}
                None => {
                    subst.insert(name.clone(), actual.clone());
                }
            }
        }
        HIRTypeKind::Named { name, args } => {
            if let (Some(def), MIRType::Struct { fields, .. }) = (struct_defs.get(name), actual) {
                let mut field_subst = HashMap::new();
                for (type_param, arg) in def.type_params.iter().zip(args.iter()) {
                    field_subst.insert(type_param.name.clone(), arg.clone());
                }
                for field in &def.fields {
                    if let Some((_, actual_field_ty)) =
                        fields.iter().find(|(field_name, _)| field_name == &field.name)
                    {
                        let template_field_ty = substitute_hir_type(&field.ty, &field_subst);
                        bind_mir_subst_from_hir_type(
                            &template_field_ty,
                            actual_field_ty,
                            struct_defs,
                            subst,
                        );
                    }
                }
            }
        }
        HIRTypeKind::Ref(_, inner) => {
            if let MIRType::Ref(actual_inner) = actual {
                bind_mir_subst_from_hir_type(inner, actual_inner, struct_defs, subst);
            }
        }
        HIRTypeKind::Ptr(inner) => {
            if let MIRType::Ptr(actual_inner) = actual {
                bind_mir_subst_from_hir_type(inner, actual_inner, struct_defs, subst);
            }
        }
        HIRTypeKind::Array(inner, _) => {
            if let MIRType::Array(actual_inner, _) = actual {
                bind_mir_subst_from_hir_type(inner, actual_inner, struct_defs, subst);
            }
        }
        HIRTypeKind::Tuple(items) => {
            if let MIRType::Tuple(actual_items) = actual {
                for (template_item, actual_item) in items.iter().zip(actual_items.iter()) {
                    bind_mir_subst_from_hir_type(template_item, actual_item, struct_defs, subst);
                }
            }
        }
        _ => {}
    }
}

pub fn lower_hir(items: &[HIRItem]) -> Result<Vec<MirFunction>, String> {
    lower_hir_with_options(items, MirLowerOptions::default())
}

fn collect_direct_calls_in_expr(expr: &HIRExpr, out: &mut HashSet<String>) {
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
        HIRExpr::Call { func, args } => {
            if let HIRExpr::Var { name, .. } = func.as_ref() {
                out.insert(name.clone());
            }
            collect_direct_calls_in_expr(func, out);
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
        HIRExpr::AsyncBlock(body) => {
            collect_direct_calls_in_body(body, out);
        }
    }
}

fn collect_direct_calls_in_stmt(stmt: &HIRStmt, out: &mut HashSet<String>) {
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

fn collect_direct_calls_in_body(body: &HIRBody, out: &mut HashSet<String>) {
    for stmt in &body.stmts {
        collect_direct_calls_in_stmt(stmt, out);
    }
    if let Some(expr) = body.expr.as_deref() {
        collect_direct_calls_in_expr(expr, out);
    }
}

fn collect_direct_call_names(items: &[HIRItem]) -> HashSet<String> {
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

pub fn lower_hir_with_options(
    items: &[HIRItem],
    options: MirLowerOptions,
) -> Result<Vec<MirFunction>, String> {
    let mut results = Vec::new();
    let mut errors = Vec::new();
    let mut lambda_counter = 0;
    let direct_calls = if options.lazy_generic_mono {
        collect_direct_call_names(items)
    } else {
        HashSet::new()
    };

    let mut trait_defs: HashMap<String, &HIRTrait> = HashMap::new();
    let mut struct_defs: HashMap<String, &hir::HIRStruct> = HashMap::new();
    let mut known_named_types: HashSet<String> = HashSet::new();
    for item in items {
        match item {
            HIRItem::Trait(trait_item) => {
                trait_defs.insert(trait_item.name.clone(), trait_item);
            }
            HIRItem::Struct(struct_item) => {
                known_named_types.insert(struct_item.name.clone());
                struct_defs.insert(struct_item.name.clone(), struct_item);
            }
            _ => {}
        }
    }
    let concrete_named_types = collect_concrete_named_types_closure(items, &known_named_types);
    let concrete_type_registry = ConcreteTypeRegistry::new(&struct_defs, &concrete_named_types);
    let inherent_method_templates = collect_inherent_method_templates(items);
    let mut trait_method_templates: Vec<TraitMethodTemplate> = Vec::new();

    let mut known_functions: HashSet<String> = HashSet::new();
    let mut known_function_sigs: HashMap<String, FunctionSig> = HashMap::new();
    for item in items {
        match item {
            HIRItem::Function(fn_item) => {
                known_functions.insert(fn_item.name.clone());
                known_function_sigs.insert(
                    fn_item.name.clone(),
                    FunctionSig {
                        ret_type: hir_type_to_mir_with_structs(&fn_item.return_type, &struct_defs),
                        param_count: fn_item.params.len(),
                        env: vec![],
                    },
                );
            }
            HIRItem::Impl(impl_item) => {
                for impl_item in expand_impl_variants(
                    impl_item,
                    &concrete_named_types,
                    &known_named_types,
                ) {
                    let type_prefix = impl_type_prefix(&impl_item.target_type);
                    if let Some(trait_name) = &impl_item.trait_name {
                        let collected = collect_trait_method_templates_for_impl(
                            &impl_item,
                            trait_defs.get(trait_name.as_str()).copied(),
                            &type_prefix,
                        );
                        trait_method_templates.extend(collected.templates);
                        let impl_method_names = collected.implemented_method_names;
                        for method in &impl_item.items {
                            let original_method_name = method
                                .name
                                .strip_prefix(&format!("{}_", type_prefix))
                                .unwrap_or(&method.name);
                            if !method.type_params.is_empty() {
                                continue;
                            }
                            let three_part_name =
                                format!("{}_{}_{}", type_prefix, trait_name, original_method_name);
                            known_function_sigs.insert(
                                three_part_name.clone(),
                                FunctionSig {
                                    ret_type: hir_type_to_mir_with_structs(
                                        &method.return_type,
                                        &struct_defs,
                                    ),
                                    param_count: explicit_hir_method_param_count(method),
                                    env: vec![],
                                },
                            );
                            known_functions.insert(three_part_name);
                        }

                        if let Some(trait_def) = trait_defs.get(trait_name.as_str()) {
                            for trait_item in &trait_def.items {
                                if let HIRTraitItem::Function(trait_fn) = trait_item {
                                    if !impl_method_names.contains(&trait_fn.name) {
                                        if !trait_fn.type_params.is_empty() {
                                            continue;
                                        }
                                        let three_part_name =
                                            format!("{}_{}_{}", type_prefix, trait_name, trait_fn.name);
                                        known_function_sigs.insert(
                                            three_part_name.clone(),
                                            FunctionSig {
                                                ret_type: hir_type_to_mir_with_structs(
                                                    &trait_fn.return_type,
                                                    &struct_defs,
                                                ),
                                                param_count: explicit_hir_method_params(&trait_fn.params).len(),
                                                env: vec![],
                                            },
                                        );
                                        known_functions.insert(three_part_name);
                                    }
                                }
                            }
                        }
                    } else {
                        for method in &impl_item.items {
                            if !method.type_params.is_empty() {
                                continue;
                            }
                            known_functions.insert(method.name.clone());
                            known_function_sigs.insert(
                                method.name.clone(),
                                FunctionSig {
                                    ret_type: hir_type_to_mir_with_structs(
                                        &method.return_type,
                                        &struct_defs,
                                    ),
                                    param_count: explicit_hir_method_param_count(method),
                                    env: vec![],
                                },
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }

    for item in items {
        match item {
            HIRItem::Function(fn_item) => {
                if options.lazy_generic_mono
                    && !fn_item.type_params.is_empty()
                    && !direct_calls.contains(&fn_item.name)
                {
                    continue;
                }
                match lower_function(
                    fn_item,
                    &mut lambda_counter,
                    &known_functions,
                    &known_function_sigs,
                    &struct_defs,
                    concrete_type_registry.clone(),
                    &options,
                    &inherent_method_templates,
                    &trait_method_templates,
                ) {
                    Ok((mir_fn, lambdas)) => {
                        results.push(mir_fn);
                        results.extend(lambdas);
                    }
                    Err(e) => errors.push(e),
                }
            }
            HIRItem::Impl(impl_item) => {
                for impl_item in expand_impl_variants(
                    impl_item,
                    &concrete_named_types,
                    &known_named_types,
                ) {
                    let type_prefix = impl_type_prefix(&impl_item.target_type);
                    let mut impl_method_names: HashSet<String> = HashSet::new();
                    for method in &impl_item.items {
                        if let Some(trait_name) = &impl_item.trait_name {
                            let original_method_name = method
                                .name
                                .strip_prefix(&format!("{}_", type_prefix))
                                .unwrap_or(&method.name);
                            impl_method_names.insert(original_method_name.to_string());
                            if !method.type_params.is_empty() {
                                continue;
                            }
                            let three_part_name =
                                format!("{}_{}_{}", type_prefix, trait_name, original_method_name);
                            let mut renamed_method = method.clone();
                            renamed_method.name = three_part_name;
                            match lower_function(
                                &renamed_method,
                                &mut lambda_counter,
                                &known_functions,
                                &known_function_sigs,
                                &struct_defs,
                                concrete_type_registry.clone(),
                                &options,
                                &inherent_method_templates,
                                &trait_method_templates,
                            ) {
                                Ok((mir_fn, lambdas)) => {
                                    results.push(mir_fn);
                                    results.extend(lambdas);
                                }
                                Err(e) => errors.push(e),
                            }
                        } else {
                            if !method.type_params.is_empty() {
                                continue;
                            }
                            match lower_function(
                                method,
                                &mut lambda_counter,
                                &known_functions,
                                &known_function_sigs,
                                &struct_defs,
                                concrete_type_registry.clone(),
                                &options,
                                &inherent_method_templates,
                                &trait_method_templates,
                            ) {
                                Ok((mir_fn, lambdas)) => {
                                    results.push(mir_fn);
                                    results.extend(lambdas);
                                }
                                Err(e) => errors.push(e),
                            }
                        }
                    }

                    if let Some(trait_name) = &impl_item.trait_name {
                        if let Some(trait_def) = trait_defs.get(trait_name.as_str()) {
                            for trait_item in &trait_def.items {
                                if let HIRTraitItem::Function(trait_fn) = trait_item {
                                    if !impl_method_names.contains(&trait_fn.name) {
                                        if !trait_fn.type_params.is_empty() {
                                            continue;
                                        }
                                        let three_part_name =
                                            format!("{}_{}_{}", type_prefix, trait_name, trait_fn.name);

                                        let mut params = Vec::new();
                                        let has_self = trait_fn.params.iter().any(|p| p.name == "self");
                                        if !has_self {
                                            params.push(HIRParam::new(
                                                "self".to_string(),
                                                SymbolId::INVALID,
                                                impl_item.target_type.clone(),
                                            ));
                                        }
                                        params.extend(trait_fn.params.iter().cloned());

                                        let default_fn = hir::HIRFunction {
                                            name: three_part_name,
                                            type_params: trait_fn.type_params.clone(),
                                            params,
                                            return_type: trait_fn.return_type.clone(),
                                            precondition: trait_fn.precondition.clone(),
                                            postcondition: trait_fn.postcondition.clone(),
                                            body: trait_fn.body.clone(),
                                            is_async: trait_fn.is_async,
                                            abi: trait_fn.abi.clone(),
                                            is_unsafe: trait_fn.is_unsafe,
                                            no_mangle: trait_fn.no_mangle,
                                            export_name: trait_fn.export_name.clone(),
                                            is_pub: trait_fn.is_pub,
                                        };

                                        match lower_function(
                                            &default_fn,
                                            &mut lambda_counter,
                                            &known_functions,
                                            &known_function_sigs,
                                            &struct_defs,
                                            concrete_type_registry.clone(),
                                            &options,
                                            &inherent_method_templates,
                                            &trait_method_templates,
                                        ) {
                                            Ok((mir_fn, lambdas)) => {
                                                results.push(mir_fn);
                                                results.extend(lambdas);
                                            }
                                            Err(e) => errors.push(e),
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
    }

    if !errors.is_empty() {
        return Err(format!("MIR lowering failed:\n{}", errors.join("\n")));
    }

    Ok(results)
}

fn lower_function(
    fn_item: &hir::HIRFunction,
    lambda_counter: &mut usize,
    known_functions: &HashSet<String>,
    known_function_sigs: &HashMap<String, FunctionSig>,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    concrete_type_registry: ConcreteTypeRegistry,
    options: &MirLowerOptions,
    inherent_method_templates: &[InherentMethodTemplate],
    trait_method_templates: &[TraitMethodTemplate],
) -> Result<(MirFunction, Vec<MirFunction>), String> {
    let params: Vec<MIRType> = fn_item
        .params
        .iter()
        .map(|p| hir_type_to_mir_with_structs(&p.ty, struct_defs))
        .collect();
    let return_type: MIRType = hir_type_to_mir_with_structs(&fn_item.return_type, struct_defs);

    let mut mir_fn = MirFunction::new(fn_item.name.clone(), params, return_type);
    mir_fn.is_async = fn_item.is_async;
    let start_block = mir_fn.start_block;
    let mut ctx = LoweringContext::new(
        &mut mir_fn,
        lambda_counter,
        known_functions,
        known_function_sigs,
        struct_defs,
        concrete_type_registry,
        options.clone(),
        inherent_method_templates,
        trait_method_templates,
    );

    // 闁告瑥鍊归弳鐔奉啅閼碱剛鐥呴悶姘煎亝閸у﹪宕濋悩鎻掔厒 locals 濞戞搩鍙忕槐婵嬫閳ь剛鎲版担绛嬪敹鐟滅増娲栭悾鐘崇椤掑倹鐣遍柛姘Ф琚?
    for (i, param) in fn_item.params.iter().enumerate() {
        let local = Local::new(i + 1, LocalKind::Param);
        ctx.local_names.insert(param.name.clone(), local);
        ctx.bind_local_symbol(param.symbol, local);
        if let Some((_, MIRType::Struct { name, .. })) = ctx.mir_fn.locals.get(i + 1) {
            ctx.type_names.insert(local, name.clone());
        }
        ctx.contract_param_bindings
            .push((param.name.clone(), param.symbol, local));
    }

    // 闂傚嫬绉崇紞鍡涘礄閼恒儲娈跺ù锝嗘尭閸╁苯顔忛崣澶嬬畳闁汇劌瀚崣鍡涘矗閿濆懏鍋?
    let body_entry = if options.runtime_contract_checks {
        if let Some(precondition) = fn_item.precondition.as_ref() {
            ctx.inject_precondition_check(precondition, start_block)
        } else {
            start_block
        }
    } else {
        start_block
    };
    ctx.lower_body_to_block(&fn_item.body, body_entry);
    if options.runtime_contract_checks {
        if let Some(postcondition) = fn_item.postcondition.as_ref() {
            ctx.inject_postcondition_checks(postcondition);
        }
    }

    // 婵☆偀鍋撻柡灞诲劜濡叉悂宕ラ敂鑺ョ畳闂佹寧鐟ㄩ銈夊矗閹寸姵鏅?
    if !ctx.errors.is_empty() {
        return Err(format!(
            "MIR lowering errors in function '{}':\n  {}",
            fn_item.name,
            ctx.errors.join("\n  ")
        ));
    }

    // 闁圭粯鍔曡ぐ?lambda_functions闁挎稑鐭傞崳鎾绩閹屽殸 mir_fn 闁汇劌瀚埀顒傚枔閺?
    let lambda_functions = ctx.lambda_functions;
    Ok((mir_fn, lambda_functions))
}

/// 鐎甸偊浜為獮鍡樼▔婵犱胶鐟撻柡鍌氭祫缁辨繈鎮介妸銈囪壘 break/continue
#[derive(Debug, Clone, Copy)]
struct LoopContext {
    /// break 閻犲搫鐤囧ù鍡涘礆閹殿喗鐣遍柣鈺婂枟閻栵綁鏁?
    break_block: usize,
    /// continue 閻犲搫鐤囧ù鍡涘礆閹殿喗鐣遍柣鈺婂枟閻栵綁鏁?
    continue_block: usize,
}

/// 闁告垼濮ら弳鐔虹驳閹勫€冲ǎ鍥ｅ墲娴煎懘鏁嶉崼锝囩闁搞儳鍋熺悮顐﹀垂鐎ｅ墎绀?
#[derive(Clone)]
struct FunctionSig {
    ret_type: MIRType,
    param_count: usize,
    /// 闁硅娲濋獮蹇涙儍閸曨喖娈伴柣銏犲船瑜板鏌岃箛銉х闁告艾绉惰ⅷ, 缂侇偉顕ч悗鐑芥晸?
    #[allow(dead_code)]
    env: Vec<(String, MIRType)>,
}

/// Lambda 闁绘粠鍨伴。銊︾┍閳╁啩绱?
struct LambdaEnv {
    /// 闁绘粠鍨伴。銊╁矗濮椻偓閸ｆ椽宕ュ鍥嗙偤宕仦绛嬪殸閹煎瓨姊诲▓?Local
    vars: Vec<(String, Local)>,
    /// 闁绘粠鍨伴。銊х磼閹惧鈧垱鎷呴幘鎯邦潶闁搞劌顑戠槐娆撴偨閵娿倗鑹惧ù鐙呯悼閻栨粓鎮介悢绋跨亣闁?
    #[allow(dead_code)]
    env_type: MIRType,
    /// 闁绘粠鍨伴。銊╁箰閸ヮ剚瀚涢柨?Local闁挎稑鐗嗗﹢顏嗘嫬閸愵亝鏆忛柡鍐╂构婵炲洭鎮介…鎺旂
    env_ptr_local: Option<Local>,
}

/// 閺夌儐鍓氬畷鍙夌▔婵犱胶鐟撻柨?
struct LoweringContext<'a> {
    mir_fn: &'a mut MirFunction,
    /// 闁告艾绉惰ⅷ闁告帗婢橀惇顒勬焾閵娿儱缍侀梺鎻掔箳濞堟垿寮伴悩鑼
    local_names: HashMap<String, Local>,
    local_symbols: HashMap<SymbolId, Local>,
    contract_param_bindings: Vec<(String, SymbolId, Local)>,
    /// 鐟滅増鎸告晶鐘诲春閻戞ɑ鎷遍柨?
    current_block: Option<usize>,
    /// 闁衡偓閸洘鑲犻柣銊ュ閺佸﹦鎷犻娆庣箚闁?
    errors: Vec<String>,
    /// 鐎甸偊浜為獮鍡涘冀閸剛绀夐柣顫妺缁剚寰勯崟顓熷€?break/continue
    loop_stack: Vec<LoopContext>,
    /// Lambda 閻犱讲鍓濋弳鐔煎闯椤帞绀勯柣顫妺缁剟鎮介悢绋跨亣闁哥儐鍨粩鎾触瀹ュ泦鐐烘晸?
    lambda_counter: &'a mut usize,
    /// 闁汇垻鍠愰崹姘舵晸?Lambda 閺夊牆鎳庢慨顏堝礄閼恒儲娈?
    lambda_functions: Vec<MirFunction>,
    /// Local 闁?Lambda 闁告垼濮ら弳鐔煎触瀹ュ洦鐣遍柡鍕Т閻?
    lambda_names: HashMap<Local, String>,
    /// 闁告垼濮ら弳鐔煎触瀹ュ懎鐓傜紒娑欏劤閹洟鎯冮崟顒佇侀柨?
    function_sigs: HashMap<String, FunctionSig>,
    /// Lambda 闁告垼濮ら弳鐔煎触瀹ュ懎鐓傞柣婊庡灠椤ｃ劍绌遍埄鍐х礀闁汇劌瀚Σ褔鏁?
    lambda_environments: HashMap<String, LambdaEnv>,
    /// 闁哄嫮濮撮惃?Local 闁?闁告鍠庨～鎰尵鐠囪尙鈧兘宕ュ鍥嗙偤鏁嶉崼銏℃殢濞存粌娴风划銊╁几閸曨亞绉奸柡鍌濐潐绾墎鎷崘顏呮殢閻熸瑱绲鹃悗浠嬫晸?
    type_names: HashMap<Local, String>,
    /// 鐎规瓕灏欓悡锟犳儍閸曨偄姣愰柡浣规緲閹洟姊块崱妤佸€ら柨娑樼墢閺併倖绂嶆惔銏＄厵婵炲娲濋惃鐔兼偨閵娾晝宕ｉ悹鍥︾筏缁?
    known_functions: HashSet<String>,
    struct_defs: &'a HashMap<String, &'a hir::HIRStruct>,
    concrete_type_registry: ConcreteTypeRegistry,
    options: MirLowerOptions,
    inherent_method_templates: &'a [InherentMethodTemplate],
    trait_method_templates: &'a [TraitMethodTemplate],
    /// Maps a Local → async function base name when that local holds a future
    /// handle produced by a `foo__start(...)` call. Propagated through let
    /// bindings so that `let f = async_fn(); await f` resolves correctly.
    future_origins: HashMap<Local, String>,
}

impl<'a> LoweringContext<'a> {
    fn new(
        mir_fn: &'a mut MirFunction,
        lambda_counter: &'a mut usize,
        known_functions: &'a HashSet<String>,
        known_function_sigs: &HashMap<String, FunctionSig>,
        struct_defs: &'a HashMap<String, &'a hir::HIRStruct>,
        concrete_type_registry: ConcreteTypeRegistry,
        options: MirLowerOptions,
        inherent_method_templates: &'a [InherentMethodTemplate],
        trait_method_templates: &'a [TraitMethodTemplate],
    ) -> Self {
        Self {
            mir_fn,
            local_names: HashMap::new(),
            local_symbols: HashMap::new(),
            contract_param_bindings: Vec::new(),
            current_block: None,
            errors: Vec::new(),
            loop_stack: Vec::new(),
            lambda_counter,
            lambda_functions: Vec::new(),
            lambda_names: HashMap::new(),
            function_sigs: known_function_sigs.clone(),
            lambda_environments: HashMap::new(),
            type_names: HashMap::new(),
            known_functions: known_functions.clone(),
            struct_defs,
            concrete_type_registry,
            options,
            inherent_method_templates,
            trait_method_templates,
            future_origins: HashMap::new(),
        }
    }

    fn receiver_type_prefix(&self, receiver_ty: &MIRType) -> String {
        match receiver_ty {
            MIRType::Int(bits) => format!("i{}", bits),
            MIRType::Float(bits) => format!("f{}", bits),
            MIRType::Bool => "bool".to_string(),
            MIRType::Array(_, _) => "array".to_string(),
            MIRType::Tuple(_) => "tuple".to_string(),
            MIRType::Ptr(inner) | MIRType::Ref(inner) => match inner.as_ref() {
                MIRType::Int(bits) => format!("i{}_ptr", bits),
                MIRType::Float(bits) => format!("f{}_ptr", bits),
                MIRType::Bool => "bool_ptr".to_string(),
                _ => "ptr".to_string(),
            },
            MIRType::Struct { name, .. } => name.clone(),
            MIRType::Enum { .. } => "enum".to_string(),
            _ => "i64".to_string(),
        }
    }

    fn method_dispatch_name(
        &self,
        receiver_local: Local,
        receiver_ty: &MIRType,
        method: &str,
    ) -> String {
        if let Some(type_name) = self.type_names.get(&receiver_local) {
            format!("{}_{}", type_name, method)
        } else {
            match receiver_ty {
                MIRType::Int(bits) => format!("i{}_{}", bits, method),
                MIRType::Float(bits) => format!("f{}_{}", bits, method),
                MIRType::Bool => format!("bool_{}", method),
                MIRType::Array(_, _) => format!("array_{}", method),
                MIRType::Tuple(_) => format!("tuple_{}", method),
                MIRType::Ptr(inner) | MIRType::Ref(inner) => match inner.as_ref() {
                    MIRType::Int(bits) => format!("i{}_ptr_{}", bits, method),
                    MIRType::Float(bits) => format!("f{}_ptr_{}", bits, method),
                    MIRType::Bool => format!("bool_ptr_{}", method),
                    _ => format!("ptr_{}", method),
                },
                MIRType::Struct { name, .. } => format!("{}_{}", name, method),
                MIRType::Enum { .. } => format!("enum_{}", method),
                _ => format!("i64_{}", method),
            }
        }
    }

    fn receiver_type_display(&self, receiver_local: Local, receiver_ty: &MIRType) -> String {
        if let Some(type_name) = self.type_names.get(&receiver_local) {
            type_name.clone()
        } else {
            match receiver_ty {
                MIRType::Int(bits) => format!("i{}", bits),
                MIRType::Float(bits) => format!("f{}", bits),
                MIRType::Bool => "bool".to_string(),
                MIRType::Array(_, _) => "array".to_string(),
                MIRType::Tuple(_) => "tuple".to_string(),
                MIRType::Ptr(_) | MIRType::Ref(_) => "ptr".to_string(),
                MIRType::Struct { name, .. } => name.clone(),
                MIRType::Enum { .. } => "enum".to_string(),
                _ => format!("{:?}", receiver_ty),
            }
        }
    }

    fn resolve_method_call_target(
        &mut self,
        receiver_local: Local,
        receiver_ty: &MIRType,
        method: &str,
        arg_locals: &[Local],
    ) -> Result<String, String> {
        let method_func_name = self.method_dispatch_name(receiver_local, receiver_ty, method);
        let type_display = self.receiver_type_display(receiver_local, receiver_ty);

        if self.known_functions.contains(&method_func_name) {
            return Ok(method_func_name);
        }
        if let Some(generated_name) =
            self.try_materialize_inherent_method(receiver_ty, method, arg_locals)
        {
            return Ok(generated_name);
        }
        if let Some(generated_name) =
            self.try_materialize_trait_method(receiver_ty, method, arg_locals, &type_display)?
        {
            return Ok(generated_name);
        }

        let type_prefix = if let Some(type_name) = self.type_names.get(&receiver_local) {
            type_name.clone()
        } else {
            self.receiver_type_prefix(receiver_ty)
        };

        match self.select_known_trait_method_candidate(
            &type_prefix,
            method,
            &method_func_name,
            arg_locals.len(),
        ) {
            MethodCandidateMatch::None | MethodCandidateMatch::WrongArity { .. } => Err(format!(
                "method '{}' not found for type '{}'",
                method, type_display
            )),
            MethodCandidateMatch::One(name) => Ok(name),
            MethodCandidateMatch::Ambiguous { labels } => {
                Err(ambiguous_method_error(method, &type_display, &labels))
            }
        }
    }

    fn bind_method_specialization_subst(
        &self,
        target_type: &HIRType,
        method: &hir::HIRFunction,
        receiver_ty: &MIRType,
        arg_locals: &[Local],
    ) -> Option<HashMap<String, MIRType>> {
        let mut mir_subst = HashMap::new();
        bind_mir_subst_from_hir_type(target_type, receiver_ty, self.struct_defs, &mut mir_subst);

        let actual_arg_types: Vec<MIRType> = arg_locals
            .iter()
            .map(|local| self.get_local_type(*local).clone())
            .collect();
        let explicit_params = explicit_hir_method_params(&method.params);
        if explicit_params.len() != actual_arg_types.len() {
            return None;
        }
        for (param, actual_ty) in explicit_params.iter().zip(actual_arg_types.iter()) {
            bind_mir_subst_from_hir_type(&param.ty, actual_ty, self.struct_defs, &mut mir_subst);
        }

        Some(mir_subst)
    }

    fn realize_method_specialization(
        &mut self,
        target_type: &HIRType,
        method: &hir::HIRFunction,
        receiver_ty: &MIRType,
        mir_subst: HashMap<String, MIRType>,
    ) -> Option<(HashMap<String, HIRType>, String)> {
        let receiver_prefix = self.receiver_type_prefix(receiver_ty);
        let mut hir_subst = HashMap::new();
        for (name, mir_ty) in &mir_subst {
            hir_subst.insert(name.clone(), self.concrete_type_registry.hir_type_for_mir(mir_ty)?);
        }
        if !method
            .type_params
            .iter()
            .all(|param| hir_subst.contains_key(&param.name))
        {
            return None;
        }

        let concrete_target = substitute_hir_type(target_type, &hir_subst);
        let concrete_prefix = impl_type_prefix(&concrete_target);
        self.concrete_type_registry
            .register_instance(concrete_prefix.clone(), concrete_target.clone());
        for ty in hir_subst.values() {
            if matches!(ty.kind, HIRTypeKind::Named { .. }) {
                self.concrete_type_registry
                    .register_instance(hir_type_to_instance_name(ty), ty.clone());
            }
        }
        if concrete_prefix != receiver_prefix {
            return None;
        }

        Some((hir_subst, concrete_prefix))
    }

    fn prepare_method_specialization(
        &mut self,
        target_type: &HIRType,
        method: &hir::HIRFunction,
        receiver_ty: &MIRType,
        arg_locals: &[Local],
    ) -> Option<(HashMap<String, HIRType>, String)> {
        let mir_subst =
            self.bind_method_specialization_subst(target_type, method, receiver_ty, arg_locals)?;
        self.realize_method_specialization(target_type, method, receiver_ty, mir_subst)
    }

    fn lower_materialized_method(&mut self, specialized: hir::HIRFunction) -> Option<String> {
        if self.known_functions.contains(&specialized.name) {
            return Some(specialized.name);
        }

        self.function_sigs.insert(
            specialized.name.clone(),
            FunctionSig {
                ret_type: hir_type_to_mir_with_structs(&specialized.return_type, self.struct_defs),
                param_count: explicit_hir_method_param_count(&specialized),
                env: vec![],
            },
        );
        self.known_functions.insert(specialized.name.clone());

        match lower_function(
            &specialized,
            self.lambda_counter,
            &self.known_functions,
            &self.function_sigs,
            self.struct_defs,
            self.concrete_type_registry.clone(),
            &self.options,
            self.inherent_method_templates,
            self.trait_method_templates,
        ) {
            Ok((mir_fn, nested)) => {
                self.lambda_functions.push(mir_fn);
                self.lambda_functions.extend(nested);
                Some(specialized.name)
            }
            Err(error) => {
                self.errors.push(error);
                None
            }
        }
    }

    fn select_known_trait_method_candidate(
        &self,
        type_prefix: &str,
        method: &str,
        excluded_name: &str,
        expected_param_count: usize,
    ) -> MethodCandidateMatch<String> {
        let suffix = format!("_{}", method);
        let prefix = format!("{}_", type_prefix);
        let matches = self
            .known_functions
            .iter()
            .filter(|name| {
                name.starts_with(&prefix)
                    && name.ends_with(&suffix)
                    && *name != excluded_name
                    && {
                        let middle = &name[prefix.len()..name.len() - suffix.len()];
                        !middle.is_empty()
                    }
            })
            .map(|name| MethodCandidate {
                label: name.clone(),
                param_count: self
                    .function_sigs
                    .get(name)
                    .map(|sig| sig.param_count)
                    .unwrap_or(0),
                value: name.clone(),
            })
            .collect();
        select_method_candidate(matches, expected_param_count)
    }

    fn try_materialize_inherent_method(
        &mut self,
        receiver_ty: &MIRType,
        method: &str,
        arg_locals: &[Local],
    ) -> Option<String> {
        for template in self.inherent_method_templates {
            let legacy_prefix = hir_type_to_prefix(&template.target_type);
            let original_method_name = template
                .method
                .name
                .strip_prefix(&format!("{}_", legacy_prefix))
                .unwrap_or(&template.method.name);
            if original_method_name != method {
                continue;
            }

            let (hir_subst, concrete_prefix) = self.prepare_method_specialization(
                &template.target_type,
                &template.method,
                receiver_ty,
                arg_locals,
            )?;

            let mut specialized = instantiate_impl_method(
                &template.method,
                &legacy_prefix,
                &concrete_prefix,
                &hir_subst,
            );
            specialized.type_params.clear();
            if !template.method.type_params.is_empty() {
                let suffixes: Vec<String> = template
                    .method
                    .type_params
                    .iter()
                    .filter_map(|param| hir_subst.get(&param.name))
                    .map(hir_type_to_instance_name)
                    .collect();
                specialized.name = format!("{}_{}", specialized.name, suffixes.join("_"));
            }

            return self.lower_materialized_method(specialized);
        }
        None
    }

    fn specialize_trait_method_candidate(
        &mut self,
        template: &TraitMethodTemplate,
        receiver_ty: &MIRType,
        arg_locals: &[Local],
    ) -> Option<MethodCandidate<hir::HIRFunction>> {
        let (hir_subst, concrete_prefix) = self.prepare_method_specialization(
            &template.target_type,
            &template.method,
            receiver_ty,
            arg_locals,
        )?;

        let mut specialized = substitute_hir_function(&template.method, &hir_subst);
        specialized.type_params.clear();
        if !template.method.type_params.is_empty() {
            let suffixes: Vec<String> = template
                .method
                .type_params
                .iter()
                .filter_map(|param| hir_subst.get(&param.name))
                .map(hir_type_to_instance_name)
                .collect();
            specialized.name = format!(
                "{}_{}_{}_{}",
                concrete_prefix,
                template.trait_name,
                template.method.name,
                suffixes.join("_")
            );
        } else {
            specialized.name = format!(
                "{}_{}_{}",
                concrete_prefix,
                template.trait_name,
                template.method.name,
            );
        }

        Some(MethodCandidate {
            label: format!("{} ({})", specialized.name, template.trait_name),
            param_count: explicit_hir_method_param_count(&specialized),
            value: specialized,
        })
    }

    fn try_materialize_trait_method(
        &mut self,
        receiver_ty: &MIRType,
        method: &str,
        arg_locals: &[Local],
        type_display: &str,
    ) -> Result<Option<String>, String> {
        let mut candidates = Vec::new();
        for template in self.trait_method_templates {
            if template.method.name != method {
                continue;
            }

            if let Some(candidate) =
                self.specialize_trait_method_candidate(template, receiver_ty, arg_locals)
            {
                candidates.push(candidate);
            }
        }

        match select_method_candidate(candidates, arg_locals.len()) {
            MethodCandidateMatch::None | MethodCandidateMatch::WrongArity { .. } => Ok(None),
            MethodCandidateMatch::One(specialized) => Ok(self.lower_materialized_method(specialized)),
            MethodCandidateMatch::Ambiguous { labels } => {
                Err(ambiguous_method_error(method, type_display, &labels))
            }
        }
    }

    fn infer_struct_literal_type(
        &mut self,
        name: &str,
        field_locals: &HashMap<String, Local>,
    ) -> Option<MIRType> {
        let def = self.struct_defs.get(name)?;
        let mut subst: HashMap<String, MIRType> = HashMap::new();
        for field in &def.fields {
            let local = field_locals.get(&field.name)?;
            let actual_ty = self.get_local_type(*local).clone();
            bind_mir_subst_from_hir_type(&field.ty, &actual_ty, self.struct_defs, &mut subst);
        }

        if !def.type_params.is_empty()
            && !def
                .type_params
                .iter()
                .all(|type_param| subst.contains_key(&type_param.name))
        {
            return None;
        }

        let instance_name = if def.type_params.is_empty() {
            name.to_string()
        } else {
            let parts: Vec<String> = def
                .type_params
                .iter()
                .map(|type_param| {
                    mir_type_to_instance_name(
                        subst
                            .get(&type_param.name)
                            .expect("generic struct literal type param should be inferred"),
                    )
                })
                .collect();
            format!("{}_{}", name, parts.join("_"))
        };

        let concrete_hir_ty = HIRType::named(
            name.to_string(),
            def.type_params
                .iter()
                .map(|type_param| {
                    self.concrete_type_registry
                        .hir_type_for_mir(
                            subst
                                .get(&type_param.name)
                                .expect("generic struct literal type param should be inferred"),
                        )
                        .expect("concrete struct literal arg should resolve to HIR type")
                })
                .collect(),
        );
        self.concrete_type_registry
            .register_instance(instance_name.clone(), concrete_hir_ty);

        Some(MIRType::Struct {
            name: instance_name,
            fields: def
                .fields
                .iter()
                .map(|field| {
                    (
                        field.name.clone(),
                        hir_type_to_mir_with_structs_and_subst(&field.ty, self.struct_defs, &subst),
                    )
                })
                .collect(),
        })
    }

    fn lambda_name(&mut self) -> String {
        let name = format!("$__lambda{}", self.lambda_counter);
        *self.lambda_counter += 1;
        name
    }

    /// 閺夆晜绋戦崣鍡楊嚗椤忓棗绠氶柨娑樿嫰閻?break/continue 闁烩晩鍠楅悥锝夊箳閵娿儱寮抽柨?
    fn push_loop(&mut self, break_block: usize, continue_block: usize) {
        self.loop_stack.push(LoopContext {
            break_block,
            continue_block,
        });
    }

    /// 闁衡偓閸洘鑲?Lambda body 濞戞搩鍘烘繛鍥偨閵娧勭暠闁煎浜為弫閬嶅矗濮椻偓閸ｆ椽鏁嶉崼銉﹀闁告瑥鍊归弳鐔兼儍閸曨偒妯嗛梺顔哄妼瑜板鏌岃箛銉х
    /// 閺夆晜鏌ㄥú鏍嚊椤忓棙鏆犻柛娆愶耿閸ｆ椽宕ュ鍥嗙偤宕氬Δ鍕┾偓鍐椽鐏炵瓔鍤犻幖瀛樻⒒濞?Local
    fn collect_free_vars(
        &self,
        params: &[String],
        body: &crate::hir::HIRExpr,
    ) -> Vec<(String, Local)> {
        let param_names: std::collections::HashSet<String> = params.iter().cloned().collect();

        let mut free_vars = Vec::new();
        self.collect_vars_from_expr(body, &param_names, &mut free_vars);
        free_vars
    }

    /// 闂侇偅甯掔紞濠囧绩閸洘鑲犻悶娑栧姀閹活亜顕ｈ箛搴ゅ幀濞达綀娉曢弫銈夋儍閸曨喖娈伴柣銏犲船瑜板鏁?
    fn collect_vars_from_expr(
        &self,
        expr: &crate::hir::HIRExpr,
        param_names: &std::collections::HashSet<String>,
        free_vars: &mut Vec<(String, Local)>,
    ) {
        use crate::hir::HIRExpr;

        match expr {
            HIRExpr::Var { name, .. } => {
                // 濠碘€冲€归悘澶愬及椤栨艾缍侀梺鎻掔箣缁楁牗绋夊鍡樞﹂柛娆忓€归弳鐔兼晬鐏炶棄鐏熼柡鍕靛灥閸ゆ粓鎮介崡鐐茬秮闁?
                if !param_names.contains(name) {
                    if let Some(&local) = self.local_names.get(name) {
                        if !free_vars.iter().any(|(n, _)| n == name) {
                            free_vars.push((name.clone(), local));
                        }
                    }
                }
            }
            HIRExpr::Lit(_) => {}
            HIRExpr::Unary(_, operand) => {
                self.collect_vars_from_expr(operand, param_names, free_vars);
            }
            HIRExpr::Binary(_op, left, right) => {
                self.collect_vars_from_expr(left, param_names, free_vars);
                self.collect_vars_from_expr(right, param_names, free_vars);
            }
            HIRExpr::Call { func, args } => {
                self.collect_vars_from_expr(func, param_names, free_vars);
                for arg in args {
                    self.collect_vars_from_expr(arg, param_names, free_vars);
                }
            }
            HIRExpr::Lambda {
                params: inner_params,
                body: inner_body,
            } => {
                // 闁告劕鎳橀崕?Lambda 闁哄牆顦抽崵婊冾啅鏉堚晜鐣遍柛娆忓€归弳鐔兼⒖閸℃鍊?
                let inner_param_names: std::collections::HashSet<String> =
                    inner_params.iter().cloned().collect();
                self.collect_vars_from_expr(inner_body, &inner_param_names, free_vars);
            }
            HIRExpr::Block(body) => {
                for stmt in &body.stmts {
                    self.collect_vars_from_stmt(stmt, param_names, free_vars);
                }
                if let Some(expr) = &body.expr {
                    self.collect_vars_from_expr(expr, param_names, free_vars);
                }
            }
            HIRExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.collect_vars_from_expr(cond, param_names, free_vars);
                // then_branch 闁?else_branch 闁?HIRBody闁挎稑鐭傚〒鍓佹啺娴ｅ搫顥楁繛鍫濓工椤︹晠鏁?
                self.collect_vars_from_body(then_branch, param_names, free_vars);
                if let Some(else_b) = else_branch {
                    self.collect_vars_from_body(else_b, param_names, free_vars);
                }
            }
            HIRExpr::Loop(body) => {
                self.collect_vars_from_body(body, param_names, free_vars);
            }
            HIRExpr::While { cond, body } => {
                self.collect_vars_from_expr(cond, param_names, free_vars);
                self.collect_vars_from_body(body, param_names, free_vars);
            }
            HIRExpr::Break(_) | HIRExpr::Continue => {}
            HIRExpr::Array(elems) => {
                for elem in elems {
                    self.collect_vars_from_expr(elem, param_names, free_vars);
                }
            }
            HIRExpr::Index { base, index } => {
                self.collect_vars_from_expr(base, param_names, free_vars);
                self.collect_vars_from_expr(index, param_names, free_vars);
            }
            HIRExpr::Struct { fields, .. } => {
                for (_, field_val) in fields {
                    self.collect_vars_from_expr(field_val, param_names, free_vars);
                }
            }
            HIRExpr::Field { base, .. } => {
                self.collect_vars_from_expr(base, param_names, free_vars);
            }
            HIRExpr::For {
                var_name,
                iter,
                body,
                ..
            } => {
                self.collect_vars_from_expr(iter, param_names, free_vars);
                // for 闁告瑦锕㈤崳娲捶閵娿儲鍎曢柣婊庡灟缂嶅宕橀崨顔叫︾紓浣瑰灥閻ｉ箖鎯冮崟鍓佺濞戞挸绉堕悾濠氭嚊椤忓棙鏆犻柛娆愶耿閸?
                let mut extended_params = param_names.clone();
                extended_params.insert(var_name.clone());
                self.collect_vars_from_body(body, &extended_params, free_vars);
            }
            HIRExpr::Assign { target, value } => {
                self.collect_vars_from_expr(target, param_names, free_vars);
                self.collect_vars_from_expr(value, param_names, free_vars);
            }
            HIRExpr::AssignOp {
                target,
                op: _,
                value,
            } => {
                self.collect_vars_from_expr(target, param_names, free_vars);
                self.collect_vars_from_expr(value, param_names, free_vars);
            }
            HIRExpr::And(left, right) | HIRExpr::Or(left, right) => {
                self.collect_vars_from_expr(left, param_names, free_vars);
                self.collect_vars_from_expr(right, param_names, free_vars);
            }
            HIRExpr::MethodCall { receiver, args, .. } => {
                self.collect_vars_from_expr(receiver, param_names, free_vars);
                for arg in args {
                    self.collect_vars_from_expr(arg, param_names, free_vars);
                }
            }
            HIRExpr::Await(inner) => {
                self.collect_vars_from_expr(inner, param_names, free_vars);
            }
            HIRExpr::AsyncBlock(body) => {
                self.collect_vars_from_body(body, param_names, free_vars);
            }
            _ => {
                // 闁稿繑婀圭划顒傛偘閵娿劍褰х€殿喖绻掔悮顐﹀垂鐎ｎ偅鐣☉鎾崇Т椤︹晠鏁?
            }
        }
    }

    /// 闁?HIRBody 濞戞搩鍘介弫褰掓⒖閸℃缍侀柨?
    fn collect_vars_from_body(
        &self,
        body: &crate::hir::HIRBody,
        param_names: &std::collections::HashSet<String>,
        free_vars: &mut Vec<(String, Local)>,
    ) {
        for stmt in &body.stmts {
            self.collect_vars_from_stmt(stmt, param_names, free_vars);
        }
        if let Some(expr) = &body.expr {
            self.collect_vars_from_expr(expr, param_names, free_vars);
        }
    }

    /// 濞寸姴姘﹂銏ゅ矗閵夈倛鍘柡鈧崼鏇熻偁闁告瑦锕㈤崳?
    fn collect_vars_from_stmt(
        &self,
        stmt: &crate::hir::HIRStmt,
        param_names: &std::collections::HashSet<String>,
        free_vars: &mut Vec<(String, Local)>,
    ) {
        use crate::hir::HIRStmt;

        match stmt {
            HIRStmt::Let { name: _, value, .. } => {
                if let Some(v) = value {
                    self.collect_vars_from_expr(v, param_names, free_vars);
                }
                // let 缂備焦鍨甸悾楣冩儍閸曨偄缍侀梺鎻掔箣缁楀寮伴婵嗘闁汇垹宕ぐ澶愭晸?
            }
            HIRStmt::Expr(expr) => {
                self.collect_vars_from_expr(expr, param_names, free_vars);
            }
            HIRStmt::Item => {}
        }
    }

    /// 闂侇偀鍋撻柛鎴濇惈閹﹪鏁?
    fn pop_loop(&mut self) {
        self.loop_stack.pop();
    }

    /// 闁兼儳鍢茶ぐ鍥亹閹惧啿顤呯€甸偊浜為獮鍡涙晸?break 闁烩晩鍠楅悥锝夋晸?
    fn get_break_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.break_block)
    }

    /// 闁兼儳鍢茶ぐ鍥亹閹惧啿顤呯€甸偊浜為獮鍡涙晸?continue 闁烩晩鍠楅悥锝夋晸?
    fn get_continue_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.continue_block)
    }

    /// 婵烇綀顕ф慨鐐哄棘閹殿喗鐣遍悘鐐╁亾闂侇喓鍔岃ぐ澶愭晸?
    fn add_local(&mut self, name: Option<String>, kind: LocalKind, ty: MIRType) -> Local {
        let local = self.mir_fn.add_local(kind, ty);
        if let Some(name) = name {
            self.local_names.insert(name, local);
        }
        local
    }

    fn bind_local_symbol(&mut self, symbol: SymbolId, local: Local) {
        if symbol.is_valid() {
            self.local_symbols.insert(symbol, local);
        }
    }

    /// 闁兼儳鍢茶ぐ鍥╀沪閳ь剟鏌堥妸銉ョ秮闂佹彃绻掑▓鎴犵尵鐠囪尙鈧兘鏁嶉崼锝囩闁搞儳鍋涚槐鈺呮偨椤帞绀夐梺顒€鐏濋崢銈嗙▔瀹ュ懐绠戦悷鏇氳兌濞?clone闁?
    fn get_local_type(&self, local: Local) -> &MIRType {
        if let Some((_, ty)) = self.mir_fn.locals.get(local.index()) {
            ty
        } else {
            &MIR_UNIT
        }
    }

    /// 閻熸瑱绲鹃悗鐣屼沪閳ь剟鏌堥妸銉ョ秮闁?
    /// 濠碘€冲€归悘澶愬矗濮椻偓閸ｆ椽寮甸鍕毎濞戞柨顧€缁辨繄鎷嬮弶璺ㄧЭ闂佹寧鐟ㄩ銈夌嵁閹壆绠查柛銉у仒缁斿瓨绋夐鍕獥濞达絽绉堕?local
    fn resolve_local(&mut self, name: &str, symbol: SymbolId) -> Local {
        if symbol.is_valid() {
            if let Some(&local) = self.local_symbols.get(&symbol) {
                return local;
            }
        }
        match self.local_names.get(name) {
            Some(&local) => local,
            None => {
                // 閻犱焦婢樼紞宥夋煥濞嗘帩鍤?
                self.errors.push(format!("undefined variable: '{}'", name));
                // 閺夆晜鏌ㄥú鏍ㄧ▔閳ь剚绋夐鍕獥濞达絽绉堕?local闁挎稑鐭侀鈧紓鍌涚墳閻ρ呯磼瑜忛悽?
                self.mir_fn.add_local(LocalKind::Temp, MIR_UNIT)
            }
        }
    }

    /// 闁告帗绋戠紓鎾诲棘閹殿喗鐣遍柛鈺冨劋濠€浼存晸?
    fn new_block(&mut self) -> usize {
        self.mir_fn.add_block()
    }

    /// 閻犱礁澧介悿鍡氥亹閹惧啿顤呴柛鈺冨劋濠€浼存晸?
    fn set_current_block(&mut self, block: usize) {
        self.current_block = Some(block);
    }

    /// 闁兼儳鍢茶ぐ鍥亹閹惧啿顤呴柛鈺冨劋濠€浼存晸?
    fn current_block(&self) -> usize {
        self.current_block.expect("no current block set")
    }

    /// Check if two types are compatible for binary operations and, if not,
    /// try to insert Cast instructions to reconcile them.  Returns the
    /// (possibly cast) left and right locals whose types now match, or pushes
    /// an error and returns the originals unchanged.
    fn reconcile_binary_operand_types(&mut self, left: Local, right: Local) -> (Local, Local) {
        let left_ty = self.get_local_type(left).clone();
        let right_ty = self.get_local_type(right).clone();

        // Types already match 闁?nothing to do.
        if left_ty == right_ty {
            return (left, right);
        }

        // Determine if a cast between two types is valid and, if so,
        // which direction to cast (returns the common target type).
        match (&left_ty, &right_ty) {
            // Int widening: smaller int 闁?larger int
            (MIRType::Int(a), MIRType::Int(b)) => {
                let target_bits = std::cmp::max(*a, *b);
                let target_ty = MIRType::Int(target_bits);
                let new_left = if left_ty != target_ty {
                    self.insert_cast(left, target_ty.clone())
                } else {
                    left
                };
                let new_right = if right_ty != target_ty {
                    self.insert_cast(right, target_ty)
                } else {
                    right
                };
                (new_left, new_right)
            }

            // Float widening: smaller float 闁?larger float
            (MIRType::Float(a), MIRType::Float(b)) => {
                let target_bits = std::cmp::max(*a, *b);
                let target_ty = MIRType::Float(target_bits);
                let new_left = if left_ty != target_ty {
                    self.insert_cast(left, target_ty.clone())
                } else {
                    left
                };
                let new_right = if right_ty != target_ty {
                    self.insert_cast(right, target_ty)
                } else {
                    right
                };
                (new_left, new_right)
            }

            // Int 闁?Float promotion (either direction)
            (MIRType::Int(_), MIRType::Float(b)) => {
                let target_ty = MIRType::Float(*b);
                let new_left = self.insert_cast(left, target_ty);
                (new_left, right)
            }
            (MIRType::Float(a), MIRType::Int(_)) => {
                let target_ty = MIRType::Float(*a);
                let new_right = self.insert_cast(right, target_ty);
                (left, new_right)
            }

            // Bool 闁?Int promotion (either direction)
            (MIRType::Bool, MIRType::Int(b)) => {
                let target_ty = MIRType::Int(*b);
                let new_left = self.insert_cast(left, target_ty);
                (new_left, right)
            }
            (MIRType::Int(a), MIRType::Bool) => {
                let target_ty = MIRType::Int(*a);
                let new_right = self.insert_cast(right, target_ty);
                (left, new_right)
            }

            // Incompatible types 闁?report an error and return originals.
            _ => {
                self.errors.push(format!(
                    "type mismatch in binary operation: left operand has type {:?}, right operand has type {:?}",
                    left_ty, right_ty
                ));
                (left, right)
            }
        }
    }

    /// Insert a Cast instruction that converts `source` to `target_ty`,
    /// returning the new local that holds the cast result.
    fn insert_cast(&mut self, source: Local, target_ty: MIRType) -> Local {
        let dest = self.add_local(None, LocalKind::Temp, target_ty.clone());
        self.push_inst(Instruction::Cast {
            destination: dest,
            value: source,
            to: target_ty,
        });
        dest
    }

    /// 婵烇綀顕ф慨鐐哄箰閸ワ附濮㈤柛鎺撴緲缂嶅宕滃鍛敤闁哄牜鍓欏?
    fn push_inst(&mut self, inst: Instruction) {
        let block_id = self.current_block();
        self.mir_fn.push_inst_to_block(block_id, inst);
    }

    /// 閻犱礁澧介悿鍡氥亹閹惧啿顤呴柛鈺冨劋濠€浼村锤濡ゅ啯鐣辩紓浣哥墛椤掓盯鏁?
    fn set_terminator(&mut self, term: Terminator) {
        let block_id = self.current_block();
        if let Some(block) = self.mir_fn.block_mut(block_id) {
            block.set_terminator(term);
        }
    }

    fn inject_precondition_check(&mut self, precondition: &HIRExpr, entry_block: usize) -> usize {
        self.set_current_block(entry_block);
        let cond_local = self.lower_contract_condition(precondition, None);
        let pass_block = self.new_block();
        let fail_block = self.new_block();
        self.set_terminator(Terminator::If {
            cond: cond_local,
            then_block: pass_block,
            else_block: fail_block,
        });
        self.set_current_block(fail_block);
        self.set_terminator(Terminator::Unreachable);
        pass_block
    }

    fn inject_postcondition_checks(&mut self, postcondition: &HIRExpr) {
        let return_sites = self
            .mir_fn
            .basic_blocks
            .iter()
            .enumerate()
            .filter_map(|(block_id, block)| match block.terminator.clone() {
                Some(Terminator::Return(value)) => Some((block_id, value)),
                _ => None,
            })
            .collect::<Vec<_>>();

        for (return_block, return_value) in return_sites {
            let Some(return_local) = return_value else {
                continue;
            };

            let check_block = self.new_block();
            let success_block = self.new_block();
            let fail_block = self.new_block();

            if let Some(block) = self.mir_fn.block_mut(return_block) {
                block.set_terminator(Terminator::Goto(check_block));
            }

            self.set_current_block(check_block);
            let cond_local = self.lower_contract_condition(postcondition, Some(return_local));
            self.set_terminator(Terminator::If {
                cond: cond_local,
                then_block: success_block,
                else_block: fail_block,
            });

            self.set_current_block(success_block);
            self.set_terminator(Terminator::Return(Some(return_local)));

            self.set_current_block(fail_block);
            self.set_terminator(Terminator::Unreachable);
        }
    }

    fn lower_contract_condition(
        &mut self,
        condition: &HIRExpr,
        result_local: Option<Local>,
    ) -> Local {
        let mut saved_name_bindings = Vec::<(String, Option<Local>)>::new();
        let mut saved_symbol_bindings = Vec::<(SymbolId, Option<Local>)>::new();

        for (name, symbol, local) in &self.contract_param_bindings {
            let previous_name = self.local_names.insert(name.clone(), *local);
            saved_name_bindings.push((name.clone(), previous_name));
            if symbol.is_valid() {
                let previous_symbol = self.local_symbols.insert(*symbol, *local);
                saved_symbol_bindings.push((*symbol, previous_symbol));
            }
        }

        if let Some(result_local) = result_local {
            let result_name = "result".to_string();
            let previous_result_name = self.local_names.insert(result_name.clone(), result_local);
            saved_name_bindings.push((result_name, previous_result_name));

            let mut result_symbols = Vec::new();
            Self::collect_named_symbols(condition, "result", &mut result_symbols);
            for symbol in result_symbols {
                if symbol.is_valid() {
                    let previous_symbol = self.local_symbols.insert(symbol, result_local);
                    saved_symbol_bindings.push((symbol, previous_symbol));
                }
            }
        }

        let cond_local = self.lower_expr(condition);

        for (symbol, previous) in saved_symbol_bindings.into_iter().rev() {
            if let Some(local) = previous {
                self.local_symbols.insert(symbol, local);
            } else {
                self.local_symbols.remove(&symbol);
            }
        }
        for (name, previous) in saved_name_bindings.into_iter().rev() {
            if let Some(local) = previous {
                self.local_names.insert(name, local);
            } else {
                self.local_names.remove(&name);
            }
        }

        cond_local
    }

    fn collect_named_symbols(expr: &HIRExpr, target_name: &str, out: &mut Vec<SymbolId>) {
        match expr {
            HIRExpr::Var { name, symbol } => {
                if name == target_name {
                    out.push(*symbol);
                }
            }
            HIRExpr::Unary(_, operand) => Self::collect_named_symbols(operand, target_name, out),
            HIRExpr::Binary(_, left, right)
            | HIRExpr::And(left, right)
            | HIRExpr::Or(left, right) => {
                Self::collect_named_symbols(left, target_name, out);
                Self::collect_named_symbols(right, target_name, out);
            }
            HIRExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                Self::collect_named_symbols(cond, target_name, out);
                Self::collect_named_symbols_in_body(then_branch, target_name, out);
                if let Some(else_body) = else_branch {
                    Self::collect_named_symbols_in_body(else_body, target_name, out);
                }
            }
            HIRExpr::Match { scrutinee, arms } => {
                Self::collect_named_symbols(scrutinee, target_name, out);
                for arm in arms {
                    Self::collect_named_symbols(&arm.body, target_name, out);
                }
            }
            HIRExpr::Loop(body) | HIRExpr::Block(body) => {
                Self::collect_named_symbols_in_body(body, target_name, out);
            }
            HIRExpr::While { cond, body } => {
                Self::collect_named_symbols(cond, target_name, out);
                Self::collect_named_symbols_in_body(body, target_name, out);
            }
            HIRExpr::For { iter, body, .. } => {
                Self::collect_named_symbols(iter, target_name, out);
                Self::collect_named_symbols_in_body(body, target_name, out);
            }
            HIRExpr::Call { func, args } => {
                Self::collect_named_symbols(func, target_name, out);
                for arg in args {
                    Self::collect_named_symbols(arg, target_name, out);
                }
            }
            HIRExpr::MethodCall { receiver, args, .. } => {
                Self::collect_named_symbols(receiver, target_name, out);
                for arg in args {
                    Self::collect_named_symbols(arg, target_name, out);
                }
            }
            HIRExpr::Struct { fields, .. } => {
                for (_, expr) in fields {
                    Self::collect_named_symbols(expr, target_name, out);
                }
            }
            HIRExpr::Array(items) | HIRExpr::Tuple(items) => {
                for item in items {
                    Self::collect_named_symbols(item, target_name, out);
                }
            }
            HIRExpr::Index { base, index } => {
                Self::collect_named_symbols(base, target_name, out);
                Self::collect_named_symbols(index, target_name, out);
            }
            HIRExpr::Field { base, .. }
            | HIRExpr::Return(Some(base))
            | HIRExpr::Break(Some(base))
            | HIRExpr::Cast(base, _)
            | HIRExpr::Ascribe(base, _)
            | HIRExpr::Ref(_, base)
            | HIRExpr::Deref(base) => Self::collect_named_symbols(base, target_name, out),
            HIRExpr::Assign { target, value } | HIRExpr::AssignOp { target, value, .. } => {
                Self::collect_named_symbols(target, target_name, out);
                Self::collect_named_symbols(value, target_name, out);
            }
            HIRExpr::Range { start, end, .. } => {
                if let Some(start) = start {
                    Self::collect_named_symbols(start, target_name, out);
                }
                if let Some(end) = end {
                    Self::collect_named_symbols(end, target_name, out);
                }
            }
            HIRExpr::Lambda { body, .. } => {
                Self::collect_named_symbols(body, target_name, out);
            }
            HIRExpr::Await(inner) => Self::collect_named_symbols(inner, target_name, out),
            HIRExpr::AsyncBlock(body) => Self::collect_named_symbols_in_body(body, target_name, out),
            HIRExpr::Lit(_) | HIRExpr::Return(None) | HIRExpr::Break(None) | HIRExpr::Continue => {}
        }
    }

    fn collect_named_symbols_in_body(body: &HIRBody, target_name: &str, out: &mut Vec<SymbolId>) {
        for stmt in &body.stmts {
            match stmt {
                HIRStmt::Expr(expr) => {
                    Self::collect_named_symbols(expr, target_name, out);
                }
                HIRStmt::Let { value, .. } => {
                    if let Some(value) = value {
                        Self::collect_named_symbols(value, target_name, out);
                    }
                }
                HIRStmt::Item => {}
            }
        }
        if let Some(expr) = &body.expr {
            Self::collect_named_symbols(expr, target_name, out);
        }
    }

    /// 闂傚嫬绉崇紞?HIR 闁秆勵殔閸╁矂骞愰崶褏鏆伴柨?
    fn lower_body_to_block(&mut self, body: &HIRBody, target_block: usize) {
        self.lower_body_to_block_with_return(body, target_block, true);
    }

    /// 闂傚嫬绉崇紞?HIR 闁秆勵殔閸╁矂骞愰崶褏鏆伴柛褎顨愮槐婵囩▔瀹ュ棗娼戦柨?return闁挎稑鐭佺换鎴﹀炊閻愬瓨浠樼紓浣哥墣閵嗗啯娼忛幆褏纭€闁?Local闁挎稑鐗嗛々褔寮稿鍕畳闁?
    fn lower_body_to_block_val(&mut self, body: &HIRBody, target_block: usize) -> Local {
        self.set_current_block(target_block);

        for stmt in &body.stmts {
            self.lower_stmt(stmt);
        }

        if let Some(expr) = &body.expr {
            self.lower_expr(expr)
        } else {
            self.add_local(None, LocalKind::Temp, MIR_UNIT)
        }
    }

    /// 闂傚嫬绉崇紞?HIR 闁秆勵殔閸╁矂骞愰崶褏鏆伴柛褎顨愮槐娆撳箳瑜嶉崺妤呭及椤栨碍鍎婃繛锝堫嚙婵?return闁?
    fn lower_body_to_block_with_return(
        &mut self,
        body: &HIRBody,
        target_block: usize,
        add_return: bool,
    ) {
        self.set_current_block(target_block);

        // 闂傚嫬绉崇紞鍡涘箥閳ь剟寮垫径搴殧闁?
        for stmt in &body.stmts {
            self.lower_stmt(stmt);
        }

        // 濠㈣泛瀚幃濠囧嫉閳ь剛绱掗崼锝冣偓鍐╂綇閹呯
        if let Some(expr) = &body.expr {
            let result_local = self.lower_expr(expr);
            if add_return {
                // Only add return if the current block doesn't already have a
                // terminator (e.g. set by break/continue/return inside the expr).
                let cur = self.current_block();
                let already_terminated = self
                    .mir_fn
                    .block_mut(cur)
                    .map_or(false, |b| b.terminator.is_some());
                if !already_terminated {
                    // 婵☆偀鍋撻柡灞诲劜濡叉悂宕ラ敂鑺バ?main 闁告垼濮ら弳鐔哥▔閺冨洨绠查柛銉у仧鐞氼偊宕圭€ｎ偅笑闁轰礁鐡ㄩ弳?
                    // 濠碘€冲€归悘澶愬及椤栨瑧鐟悶娑栧姀閹活亜顕ｈ箛鏇犳尝闁哄绮嶅Σ?unit 缂侇偉顕ч悗鐑芥晬鐏炶棄鐏熷☉鎾崇Х缁绘垿鏁?unit 闁稿﹦銆嬬槐婵嬫嚀鐏炵偓笑閺夆晜鏌ㄥú?None闁挎稑鐗呴崬顒勬儘娴ｇ儤鏅搁柟瀛樺姇濞呮帗瀵煎宕囩闁?0闁?
                    let is_main_with_unit_body = self.mir_fn.name == "main"
                        && matches!(self.mir_fn.return_type, MIRType::Int(_))
                        && matches!(*self.get_local_type(result_local), MIRType::Unit);

                    if is_main_with_unit_body {
                        self.set_terminator(Terminator::Return(None));
                    } else {
                        self.set_terminator(Terminator::Return(Some(result_local)));
                    }
                }
            }
            // 濠碘€冲€归悘?add_return = false闁挎稑濂旂粭澶娗庣拠鎻掝潱 terminator闁挎稑鐗忛弫杈╂偘閵娿劍褰х€殿喖绻楅崵婊冾啅鏉堫偒鍟庣紓鍐惧櫙缁辨繈鏁?break闁?
        } else if add_return {
            // 婵炲备鍓濆﹢浣烘偘閵娿劍褰х€殿喖绻嬬徊楣冩閳ь剟鏁?return闁挎稑鏈崸濠囧礉閻樼鏁?return
            // Only set return if the current block doesn't already have a
            // terminator (e.g. set by break/continue/return in a statement).
            let cur = self.current_block();
            let already_terminated = self
                .mir_fn
                .block_mut(cur)
                .map_or(false, |b| b.terminator.is_some());
            if !already_terminated {
                self.set_terminator(Terminator::Return(None));
            }
        }
    }

    /// 闂傚嫬绉崇紞?HIR 闁?
    fn lower_body(&mut self, body: &HIRBody) -> usize {
        let entry_block = self.new_block();
        self.lower_body_to_block(body, entry_block);
        entry_block
    }

    /// 闂傚嫬绉崇紞?HIR 閻犲浂鍘艰ぐ?
    fn lower_stmt(&mut self, stmt: &HIRStmt) {
        match stmt {
            HIRStmt::Let {
                name,
                symbol,
                ty,
                value,
                is_mut,
            } => {
                let kind = if *is_mut {
                    LocalKind::User
                } else {
                    LocalKind::User
                };
                let mir_ty = ty.clone().into();

                if let Some(value_expr) = value {
                    // 闁稿繐鐗撳閿嬫媴鎼淬們鈧啯娼忛幆褏纭€鐎电増顨呴崺宀勬晸?
                    let value_local = self.lower_expr(value_expr);

                    // 婵☆偀鍋撻柡灞诲劜濡叉悂宕ラ敂鑺バ?Lambda闁挎稑鐗嗛崢鐘绘⒕閸℃洑绨伴梺顒€鐏濋崢銈夊磹閻旂儤鏆忛柛鎰皺閻涘﹪鏁?
                    let lambda_name = self.lambda_names.get(&value_local).cloned();

                    if let Some(ln) = lambda_name {
                        let env_vars = self
                            .lambda_environments
                            .get(&ln)
                            .map(|env| env.vars.clone())
                            .unwrap_or_default();

                        if env_vars.is_empty() {
                            self.local_names.insert(name.clone(), value_local);
                            self.bind_local_symbol(*symbol, value_local);
                        } else {
                            let local = self.add_local(Some(name.clone()), kind, mir_ty);
                            self.bind_local_symbol(*symbol, local);
                            self.lambda_names.insert(local, ln.clone());

                            // 闁告帗绋戠紓鎾绘偝椤栨凹鏆旂紓浣规尰閻庮垶鏁?
                            // 闁绘粠鍨伴。銊╁及椤栨瑧顏卞☉鎿冧簼閺嗙喓绱掗崟鍓佺婵絽绻嬮柌婊堝箲閺団€崇闁汇劌瀚ぐ澶愭煂韫囨柨鐦诲銈呮惈缁厾鈧稒锚閸?
                            let env_elem_ty = MIR_I64;
                            let env_ty = MIRType::Array(
                                Box::new(env_elem_ty.clone()),
                                env_vars.len() as u64,
                            );

                            // 闁告帒妫濋崢銈夋偝椤栨凹鏆旂紒灞炬そ濡?- 濞达綀娉曢弫?User 缂侇偉顕ч悗閿嬬閵夈倗鈹掓慨婵撶悼閳?alloca
                            let env_local = self.mir_fn.add_local(LocalKind::User, env_ty);

                            // 閻庢稒锚閸嬪秴袙韫囧酣鍤嬮柟瑙勬礉楠炲繘鎯冮崟顐㈢秮闂佹彃绻愰崺宀勬偝椤栨凹鏆旈柨?
                            for (i, (var_name, _var_local)) in env_vars.iter().enumerate() {
                                // 濞寸姴楠哥紞瀣礈瀹ュ嫮鐟愬☉鎾愁儐閺嬪啴鎳㈠畡鏉跨悼闁硅娲濋獮蹇涘矗濮椻偓閸ｆ椽鏁?local
                                if let Some(&captured_local) = self.local_names.get(var_name) {
                                    // 闁兼儳鍢茶ぐ鍥偝椤栨凹鏆旈柛娆愶耿閸ｆ椽鎯冮崟顐ｅ嬀闁秆€鍋?
                                    let elem_addr_local = self.add_local(
                                        None,
                                        LocalKind::Temp,
                                        MIRType::Ptr(Box::new(env_elem_ty.clone())),
                                    );
                                    let index_local =
                                        self.add_local(None, LocalKind::Temp, MIR_I64);
                                    self.push_inst(Instruction::Assign {
                                        destination: index_local,
                                        value: MirConstant::Int(i as i64),
                                    });
                                    self.push_inst(Instruction::IndexAddr {
                                        destination: elem_addr_local,
                                        base: env_local,
                                        index: index_local,
                                    });

                                    // 闁告梻濮惧ù鍥箲閺団€崇闁汇劌瀚ぐ澶愭煂韫囥儲瀚?
                                    let captured_value_local =
                                        self.add_local(None, LocalKind::Temp, env_elem_ty.clone());
                                    self.push_inst(Instruction::Load {
                                        destination: captured_value_local,
                                        source: captured_local,
                                    });

                                    // 閻庢稒锚閸嬪秹宕氶幍顔肩畾闁?
                                    self.push_inst(Instruction::Store {
                                        destination: elem_addr_local,
                                        value: captured_value_local,
                                    });
                                }
                            }

                            // 闁兼儳鍢茶ぐ鍥偝椤栨凹鏆旈柣銊ュ濠€鎾锤閳ь剟鏁嶉崼婊呯▕濞戞挾鍎ょ€垫岸鏌﹂崼婊呯倞闂侇偅甯炵划?Lambda闁?
                            // 闁烩晛鐡ㄧ敮瀛樻媴鐠恒劍鏆?mir_fn.add_local 闁兼澘濂旂粭澶愭晸?add_local闁挎稑鐭傛导鈺呭礂瀹ュ懐娈洪柣婊庡灠椤ｃ劑宕ｅ鈧崳鍝勄庣拠鎻掝潱闁?local_names
                            let env_ptr_local = self
                                .mir_fn
                                .add_local(LocalKind::Temp, MIRType::Ptr(Box::new(env_elem_ty)));
                            self.push_inst(Instruction::AddrOf {
                                destination: env_ptr_local,
                                source: env_local,
                            });

                            // 閻忓繐妫涢獮鍡樻櫠閸愨晛鐦归梺钘夌墕閻°劑宕掗妸銉ョ厒 lambda_environments 濞戞搩鍙忕槐婵囩閵夈倗鈹掗柛锔哄姀閻ㄧ喖鎮介妸锔筋槯濞达綀娉曢弫?
                            if let Some(env_mut) = self.lambda_environments.get_mut(&ln) {
                                env_mut.env_ptr_local = Some(env_ptr_local);
                            } else {
                                self.errors.push(format!(
                                    "MIR lowering: lambda environment not found for '{}' in Let binding",
                                    ln
                                ));
                            }
                        }
                    } else {
                        // 闁哄拋鍣ｉ埀顒佽壘閳ь剛銆嬬槐婵嬪礆濞戞绱?local 妤犵偠娉涢悺銊╂晸?
                        // 闁绘顫夐悾鈺傚緞閸曨厽鍊為柨娑欒壘椤┭囧几濠婂啫绀侀柛濠傚悑濡叉悂寮幍顔剧煁缂侇偉顕ч悗鐑芥儍閸曨厽鏆忛柟鏉戝槻瑜板鏌岃箛銉х闁烩晛鐡ㄧ敮鎾煂瀹ュ懏鍤掗柛姘Т閻ｇ娀鎳撶仦鑲╃憹闁哄嫷鍨伴崹鍗烆嚈閻戞ɑ鐓€闁告瑦锕㈤崳?
                        let value_ty = self.get_local_type(value_local).clone();
                        let value_info_opt = self
                            .mir_fn
                            .locals
                            .iter()
                            .find(|(l, _)| l == &value_local)
                            .map(|(l, _t)| l.clone());

                        let value_info = match value_info_opt {
                            Some(info) => info,
                            None => {
                                self.errors.push(format!(
                                    "MIR lowering: local info not found for local {:?} in Let binding for '{}'",
                                    value_local, name
                                ));
                                // Fall through to the normal path with a new local
                                let local = self.add_local(Some(name.clone()), kind, mir_ty);
                                self.bind_local_symbol(*symbol, local);
                                if let Some(type_name) = self.type_names.get(&value_local).cloned()
                                {
                                    self.type_names.insert(local, type_name);
                                }
                                self.push_inst(Instruction::Store {
                                    destination: local,
                                    value: value_local,
                                });
                                // Propagate future origin through the let binding.
                                if let Some(origin) = self.future_origins.get(&value_local).cloned() {
                                    self.future_origins.insert(local, origin);
                                }
                                return;
                            }
                        };

                        if matches!(value_ty, MIRType::Array(_, _))
                            && value_info.kind == LocalKind::User
                        {
                            // 闁告瑥鍟块埀顒€鍚嬪Σ鎼佸极閹殿喚鐭嬬紒顐ヮ嚙閻庣兘鎯冮崟顓熸殢闁规潙鍢茶ぐ澶愭煂韫囥儳绀夐柣鈺佺摠鐢浜搁崱妤€寰撻梺鎻掔Т閹筹繝宕ュ鍕闁烩晩鍠楅悥锝夊矗濮椻偓閸?
                            // 闁?local_names 濞戞搩鍘奸崹褰掓⒔閵堝棙锛嬮柣銊ュ濡惭呬焊閸曞墎绀夋繛锝堫嚙婵偤寮幍顔界暠闁哄嫮濮撮惃?
                            self.local_names.insert(name.clone(), value_local);
                            self.bind_local_symbol(*symbol, value_local);
                            // 濞戞挸绉堕弫鎾绘晸?Store 闁圭娲ｉ幎?
                        } else {
                            // 闁哄拋鍣ｉ埀顒佽壘閳ь剛銆嬬槐婵嬪礆濞戞绱?local 妤犵偠娉涢悺銊╂晸?
                            // 濞达綀娉曢弫銈夊磹閼测晜鐣遍悗鍦仱濡绢垳鐚剧拠鑼偓鐑芥晬閸繍娲ら柨?HIR 缂侇偉顕ч悗閿嬬▔瀹ュ拋妾紒顔煎⒔閳ユ﹢鏁嶇仦鑲╀紣濠碘€冲€荤划銊╁几閸曨亞绉肩紒顐ヮ嚙閻庣兘鏁?
                            let actual_ty = value_ty.clone();
                            let local = self.add_local(Some(name.clone()), kind, actual_ty);
                            self.bind_local_symbol(*symbol, local);
                            // 濞磋偐濮甸幐杈╃尵鐠囪尙鈧兘宕ュ鍥嗙偤鏁嶅顒夋搐闁哄绮岃ぐ鎼佸磹閸忓吋绠掔紒顐ヮ嚙閻庣兘宕ュ鍥嗙偤鏁嶇仦鐣屾闁稿繑婀圭槐鍫曞箻椤撶偛鐓傞柡鍌涘濞?local
                            if let Some(type_name) = self.type_names.get(&value_local).cloned() {
                                self.type_names.insert(local, type_name);
                            }
                            self.push_inst(Instruction::Store {
                                destination: local,
                                value: value_local,
                            });
                            // Propagate future origin through the let binding.
                            if let Some(origin) = self.future_origins.get(&value_local).cloned() {
                                self.future_origins.insert(local, origin);
                            }
                        }
                    }
                } else {
                    // 婵炲备鍓濆﹢渚€宕氬┑鍡╂綏闁稿﹨鍋愬▓?let 缂備焦鍨甸悾?
                    let local = self.add_local(Some(name.clone()), kind, mir_ty);
                    self.bind_local_symbol(*symbol, local);
                }
            }
            HIRStmt::Expr(expr) => {
                self.lower_expr(expr);
            }
            HIRStmt::Item => {}
        }
    }

    /// 闂傚嫬绉崇紞?HIR 閻炴稏鍔忛幓顏堟晸?
    fn emit_runtime_print_call(&mut self, func: &str, arg_local: Local) {
        let call_local = self.add_local(None, LocalKind::Temp, MIR_UNIT);
        self.push_inst(Instruction::Call {
            destination: call_local,
            func: func.to_string(),
            args: vec![arg_local],
        });
    }

    fn emit_print_str_literal(&mut self, text: &str) {
        let str_local = self.lower_literal(&HIRLiteral::String(text.to_string()));
        self.emit_runtime_print_call("sengoo_print_str", str_local);
    }

    fn emit_print_value(&mut self, value_local: Local, value_ty: &MIRType) {
        match value_ty {
            MIRType::Struct { name, fields } => {
                self.emit_print_str_literal(&format!("{} {{ ", name));

                let fields = fields.clone();
                for (index, (field_name, field_ty)) in fields.iter().enumerate() {
                    if index > 0 {
                        self.emit_print_str_literal(", ");
                    }
                    self.emit_print_str_literal(&format!("{}: ", field_name));

                    let field_local = self.add_local(None, LocalKind::Temp, field_ty.clone());
                    self.push_inst(Instruction::Extract {
                        destination: field_local,
                        value: value_local,
                        index: index as u32,
                    });

                    self.emit_print_value(field_local, field_ty);
                }

                self.emit_print_str_literal(" }");
            }
            MIRType::Int(_) => self.emit_runtime_print_call("sengoo_print_i64", value_local),
            MIRType::Bool => self.emit_runtime_print_call("sengoo_print_bool", value_local),
            MIRType::Float(_) => self.emit_runtime_print_call("sengoo_print_f64", value_local),
            MIRType::Ptr(_) | MIRType::Ref(_) => {
                self.emit_runtime_print_call("sengoo_print_str", value_local)
            }
            _ => {
                self.errors.push(format!(
                    "print: unsupported MIR type for lowering: {:?}",
                    value_ty
                ));
            }
        }
    }

    fn lower_builtin_print(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "print expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let arg_local = arg_locals[0];
        let arg_ty = self.get_local_type(arg_local).clone();
        self.emit_print_value(arg_local, &arg_ty);
        self.add_local(None, LocalKind::Temp, MIR_UNIT)
    }

    fn lower_expr(&mut self, expr: &HIRExpr) -> Local {
        match expr {
            HIRExpr::Lit(lit) => self.lower_literal(lit),
            HIRExpr::Var { name, symbol } => self.resolve_local(name, *symbol),
            HIRExpr::Unary(op, operand) => {
                // 闁绘顫夐悾鈺傚緞閸曨厽鍊炵€殿喗娲滈弫銈夊椽瀹€鍐冩帒顕ｉ弴鐘虫殢閺夆晜鍔楅悾濠氭晸?
                match op {
                    hir::HIRUnaryOp::Ref | hir::HIRUnaryOp::RefMut => {
                        // &expr - 闁兼儳鍢茶ぐ鍥╂偘閵娿劍褰х€殿喖绻掑▓鎴﹀捶閺夋寧绲?
                        let expr_local = self.lower_expr(operand);
                        let expr_ty = self.get_local_type(expr_local).clone();

                        // 闁告帗绋戠紓鎾诲箰閸ヮ剚瀚涚紒顐ヮ嚙閻?
                        let ptr_ty = MIRType::Ptr(Box::new(expr_ty));
                        let ptr_local = self.add_local(None, LocalKind::Temp, ptr_ty);

                        // 濞达綀娉曢弫?AddrOf 闁圭娲ｉ幎銈夋嚔瀹勬澘绲块柛锔芥緲濞?
                        self.push_inst(Instruction::AddrOf {
                            destination: ptr_local,
                            source: expr_local,
                        });

                        ptr_local
                    }
                    hir::HIRUnaryOp::Deref => {
                        // *ptr - 閻熸瑱绲界槐鈺呮晸?
                        let ptr_local = self.lower_expr(operand);
                        let ptr_ty = self.get_local_type(ptr_local).clone();

                        let elem_ty = match ptr_ty {
                            MIRType::Ptr(inner) | MIRType::Ref(inner) => (*inner).clone(),
                            _ => MIR_I64,
                        };

                        let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                        self.push_inst(Instruction::Load {
                            destination: result_local,
                            source: ptr_local,
                        });

                        result_local
                    }
                    _ => {
                        // 闁稿繑婀圭划顒佺▔閳ь剟宕楅崘顓犵缂佺姵顨堥?
                        let operand_local = self.lower_expr(operand);
                        let mir_op = self.lower_un_op(op);
                        let local = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Unary {
                            destination: local,
                            op: mir_op,
                            operand: operand_local,
                        });
                        local
                    }
                }
            }
            HIRExpr::Binary(op, left, right) => {
                let left_local = self.lower_expr(left);
                let right_local = self.lower_expr(right);
                let mir_op = self.lower_bin_op(op);

                // String concatenation: when both operands are string type
                // (Ptr(Int(8))) and the operation is Add, generate a call to
                // sengoo_str_concat instead of a binary add instruction.
                if mir_op == MirBinOp::Add {
                    let is_string_concat = {
                        let left_ty = self.get_local_type(left_local);
                        let right_ty = self.get_local_type(right_local);
                        let is_string = |ty: &MIRType| matches!(ty, MIRType::Ptr(inner) if matches!(inner.as_ref(), MIRType::Int(8)));
                        is_string(left_ty) && is_string(right_ty)
                    };
                    if is_string_concat {
                        let result_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
                        let result_local = self.add_local(None, LocalKind::Temp, result_ty);
                        self.push_inst(Instruction::Call {
                            destination: result_local,
                            func: "sengoo_str_concat".to_string(),
                            args: vec![left_local, right_local],
                        });
                        return result_local;
                    }
                }

                // String comparison: when both operands are string type
                // (Ptr(Int(8))) and the operation is Eq or Ne, generate a call
                // to sengoo_str_eq instead of a binary comparison instruction.
                // sengoo_str_eq returns i64 (1=equal, 0=not equal), so we
                // convert to bool by comparing the result with 0.
                if mir_op == MirBinOp::Eq || mir_op == MirBinOp::Ne {
                    let is_string_cmp = {
                        let left_ty = self.get_local_type(left_local);
                        let right_ty = self.get_local_type(right_local);
                        let is_string = |ty: &MIRType| matches!(ty, MIRType::Ptr(inner) if matches!(inner.as_ref(), MIRType::Int(8)));
                        is_string(left_ty) && is_string(right_ty)
                    };
                    if is_string_cmp {
                        // Call sengoo_str_eq(left, right) -> i64
                        let call_result = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Call {
                            destination: call_result,
                            func: "sengoo_str_eq".to_string(),
                            args: vec![left_local, right_local],
                        });

                        // Create constant 0 for comparison
                        let zero = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Assign {
                            destination: zero,
                            value: MirConstant::Int(0),
                        });

                        // Convert i64 result to bool:
                        // For Eq: result != 0 means strings are equal 闁?true
                        // For Ne: result == 0 means strings are not equal 闁?true
                        let cmp_op = if mir_op == MirBinOp::Eq {
                            MirBinOp::Ne
                        } else {
                            MirBinOp::Eq
                        };
                        let bool_result = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                        self.push_inst(Instruction::Binary {
                            destination: bool_result,
                            op: cmp_op,
                            left: call_result,
                            right: zero,
                        });

                        return bool_result;
                    }
                }

                // 婵絾妫佺欢婵嬪椽瀹€鍕ㄥ亾閺勫繒甯嗛柟鍨С缂嶆梹娼婚弬鎸庣 bool闁挎稑鑻崣鐐閺嶃劍鎯欏ù锝嗙矎缁绘垿鏁?int(64)
                // Before generating the binary instruction, reconcile operand
                // types: insert Cast instructions for compatible mismatches or
                // record an error for incompatible types (Requirement 7.4).
                let (left_local, right_local) =
                    self.reconcile_binary_operand_types(left_local, right_local);

                // Determine the result type based on the (possibly cast) operand type.
                let operand_ty = self.get_local_type(left_local).clone();
                let result_ty = match mir_op {
                    MirBinOp::Eq
                    | MirBinOp::Ne
                    | MirBinOp::Lt
                    | MirBinOp::Le
                    | MirBinOp::Gt
                    | MirBinOp::Ge
                    | MirBinOp::LogAnd
                    | MirBinOp::LogOr => MIR_BOOL,
                    _ => operand_ty,
                };
                let local = self.add_local(None, LocalKind::Temp, result_ty);
                self.push_inst(Instruction::Binary {
                    destination: local,
                    op: mir_op,
                    left: left_local,
                    right: right_local,
                });
                local
            }
            HIRExpr::Block(body) => {
                self.lower_body(body);
                Local::new(0, LocalKind::Return)
            }
            HIRExpr::If {
                cond,
                then_branch,
                else_branch,
            } => {
                let then_block = self.new_block();
                let else_block = self.new_block();
                let join_block = self.new_block();

                let cond_local = self.lower_expr(cond);
                self.set_terminator(Terminator::If {
                    cond: cond_local,
                    then_block,
                    else_block,
                });

                // 闂傚嫬绉崇紞?then 闁告帒妫欓弫?
                let then_val = self.lower_body_to_block_val(then_branch, then_block);
                let then_end = self.current_block();
                if let Some(block) = self.mir_fn.block_mut(then_end) {
                    if block.terminator.is_none() {
                        block.set_terminator(Terminator::Goto(join_block));
                    }
                }

                // 闂傚嫬绉崇紞?else 闁告帒妫欓弫?
                if let Some(e) = else_branch {
                    let else_val = self.lower_body_to_block_val(e, else_block);
                    let else_end = self.current_block();
                    if let Some(block) = self.mir_fn.block_mut(else_end) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(join_block));
                        }
                    }

                    // 闁?join_block 闁告艾鐗嗛懟鐔哥▔閵堝嫰鍤嬮柛鎺戞閺侇喚绱掗幘瀵镐函闁?
                    // 婵炲鍔嶉崜浼存晬濮濃偓LVM 濞戞挸绉撮崢鎴犳媼?`phi void`闁挎稑鑻ú婊冾潰?Unit 缂侇偉顕ч悗閿嬬▔瀹ュ洦鏅搁柟?Phi闁?
                    self.set_current_block(join_block);
                    let then_ty = self.get_local_type(then_val).clone();
                    let is_void_like = match &then_ty {
                        MIRType::Unit | MIRType::Never => true,
                        MIRType::Tuple(fields) if fields.is_empty() => true,
                        _ => false,
                    };
                    if is_void_like {
                        self.add_local(None, LocalKind::Temp, MIR_UNIT)
                    } else {
                        let result = self.add_local(None, LocalKind::Temp, then_ty);
                        self.push_inst(Instruction::Phi {
                            destination: result,
                            incoming: vec![(then_val, then_end), (else_val, else_end)],
                        });
                        result
                    }
                } else {
                    // 婵炲备鍓濆﹢?else 闁告帒妫欓弫顕€鏁嶇€规攳se_block 闁烩晛鐡ㄧ敮瀵告崉鐎圭姵绁柨?join_block
                    if let Some(block) = self.mir_fn.block_mut(else_block) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(join_block));
                        }
                    }
                    self.set_current_block(join_block);
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
            HIRExpr::Loop(body) => {
                let loop_block = self.new_block();
                let exit_block = self.new_block();

                self.set_terminator(Terminator::Goto(loop_block));

                // 閺夆晜绋戦崣鍡楊嚗椤忓棗绠氬☉鎾筹梗缁楀懘寮崶椋庣獥break -> exit_block, continue -> loop_block
                self.push_loop(exit_block, loop_block);

                // 闂傚嫬绉崇紞?body 闁?loop_block闁挎稑鐗呯粭澶娗庣拠鎻掝潱 return闁?
                self.lower_body_to_block_with_return(body, loop_block, false);

                // 闂侇偀鍋撻柛鎴濇惈閹﹪鎮抽娆戠憪濞戞挸顑嗛弸?
                self.pop_loop();

                // After lowering the body, the current block may differ from
                // loop_block (e.g. when the body contains `if` or other control
                // flow that creates new blocks).  We need to ensure that every
                // block reachable at the end of the body that lacks a terminator
                // unconditionally branches back to loop_block.
                let end_block = self.current_block();
                if end_block != loop_block {
                    // The body introduced extra blocks; make sure the final
                    // block loops back.
                    if let Some(block) = self.mir_fn.block_mut(end_block) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(loop_block));
                        }
                    }
                }

                // Also ensure loop_block itself loops back when it has no
                // terminator (simple body with no control flow).
                if let Some(block) = self.mir_fn.block_mut(loop_block) {
                    if block.terminator.is_none() {
                        block.set_terminator(Terminator::Goto(loop_block));
                    }
                }

                self.set_current_block(exit_block);
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::While { cond, body } => {
                let cond_block = self.new_block();
                let body_block = self.new_block();
                let exit_block = self.new_block();

                self.set_terminator(Terminator::Goto(cond_block));

                // 闂傚嫬绉崇紞鍡涘级閳ュ弶顐介悶娑栧姀閹活亜顕ｈ箛鎾崇厒 cond_block
                self.set_current_block(cond_block);
                let cond_local = self.lower_expr(cond);
                self.set_terminator(Terminator::If {
                    cond: cond_local,
                    then_block: body_block,
                    else_block: exit_block,
                });

                // 閺夆晜绋戦崣鍡楊嚗椤忓棗绠氬☉鎾筹梗缁楀懘寮崶椋庣獥break -> exit_block, continue -> cond_block
                self.push_loop(exit_block, cond_block);

                // 闂傚嫬绉崇紞?body 闁?body_block闁挎稑鐗呯粭澶娗庣拠鎻掝潱 return闁?
                self.lower_body_to_block_with_return(body, body_block, false);

                // 闂侇偀鍋撻柛鎴濇惈閹﹪鎮抽娆戠憪濞戞挸顑嗛弸?
                self.pop_loop();

                // body 缂備焦鎸诲顐﹀触鎼淬倗鍎查弶鐑嗗墮濞?cond_block
                // 婵炲鍔嶉崜浼存晬濮濇笝dy 闁告瑯鍨甸崗姗€宕犻崨顓熷創闁硅矇鍐ㄧ厬婵炵繝绶ょ槐娆撴晸?if/else闁挎稑顧€缁辨繄鈧絻澹堥崵?current_block 濞戞挸绉撮崯鈧柨?body_block
                // 闂傚洠鍋撻悷鏇氱濠€?body 闁汇劌瀚〒鍫曞触鎼存繄顏卞☉鎿冧簼濡炶法鎹勯崘銊﹀仴濞戞挸锕ㄩ鏇㈡晸?Goto(cond_block)
                let body_end_block = self.current_block();
                if body_end_block != body_block {
                    // body 闁告牕鎳庨幆鍫ュ箳瑜嶉崺妤€霉娓氬﹦绀夐柡鍫氬亾闁告艾绨肩粩瀛樼▔椤忓嫭鍋ュ☉鎾崇У濡?body_block
                    if let Some(block) = self.mir_fn.block_mut(body_end_block) {
                        if block.terminator.is_none() {
                            block.set_terminator(Terminator::Goto(cond_block));
                        }
                    }
                }
                // 濞戞梻鍠愰ˉ鍛存晸?body_block 闁哄牜鍓濋棅鈺呮晬閸垻鏆嗛柨?body 闁汇劌瀚崕蹇涘礃绾板绀?
                if let Some(block) = self.mir_fn.block_mut(body_block) {
                    if block.terminator.is_none() {
                        block.set_terminator(Terminator::Goto(cond_block));
                    }
                }

                self.set_current_block(exit_block);
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::For {
                var_name,
                iter,
                body,
                ..
            } => {
                // 婵☆偀鍋撻柡灞诲劜濡叉悂宕ラ敂鑳闁肩厧鍟ú鎸庢交椤撴繂鏁?
                match iter.as_ref() {
                    HIRExpr::Range {
                        start,
                        end,
                        inclusive,
                    } => {
                        // for x in start..end { body }  闂傚嫬绉崇紞鍡涙晸?while 鐎甸偊浜為獮?
                        let cond_block = self.new_block();
                        let body_block = self.new_block();
                        let inc_block = self.new_block(); // 濠⒀呭仜婵偛顕ラ鍡楃畾闁告瑦锕㈤崳娲儍閸曨偅鍋?
                        let exit_block = self.new_block();

                        // 闂傚嫬绉崇紞?start 闁?end
                        let start_local = if let Some(s) = start {
                            self.lower_expr(s)
                        } else {
                            // 濮掓稒顭堥濠氭晸?0 鐎殿喒鍋撻柨?
                            let zero = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Assign {
                                destination: zero,
                                value: MirConstant::Int(0),
                            });
                            zero
                        };

                        let end_local = if let Some(e) = end {
                            self.lower_expr(e)
                        } else {
                            // 婵炲备鍓濆﹢浣虹磼閹惧瓨灏嗛柛濠勩€嬬槐婵嬪礆濞戞绱﹀☉鎾亾濞戞搩浜滃畷鐗堟媴瀹ュ浂鍎婇柨娑樼墛濡倝姊介幇顒佸剷闁绘粠鍨界槐?
                            let max = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Assign {
                                destination: max,
                                value: MirConstant::Int(i64::MAX),
                            });
                            max
                        };

                        // 闁告帗绋戠紓鎾愁嚗椤忓棗绠氶柛娆愶耿閸ｆ椽鐛捄鍝勭仴濠殿喖顑呯€垫煡鏁?start
                        let loop_var =
                            self.add_local(Some(var_name.clone()), LocalKind::User, MIR_I64);
                        self.push_inst(Instruction::Store {
                            destination: loop_var,
                            value: start_local,
                        });

                        // 閻犲搫鐤囧ù鍡涘礆閻楀牊钂嬪ù鐘烘硾濞?
                        self.set_terminator(Terminator::Goto(cond_block));

                        // 闁哄鈧弶顐介柛褎顨愮槐鏉课涢埀顒勫蓟閵夈儲鍎曢柣婊庡灠瑜板鏁?< end
                        self.set_current_block(cond_block);
                        let loop_var_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Load {
                            destination: loop_var_loaded,
                            source: loop_var,
                        });

                        let end_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Load {
                            destination: end_loaded,
                            source: end_local,
                        });

                        // 婵絾妫佺欢婵嬪箼瀹ュ嫮绋?
                        let cond_local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                        let compare_op = if *inclusive {
                            MirBinOp::Le
                        } else {
                            MirBinOp::Lt
                        };
                        self.push_inst(Instruction::Binary {
                            destination: cond_local,
                            op: compare_op,
                            left: loop_var_loaded,
                            right: end_loaded,
                        });

                        self.set_terminator(Terminator::If {
                            cond: cond_local,
                            then_block: body_block,
                            else_block: exit_block,
                        });

                        // 閺夆晜绋戦崣鍡楊嚗椤忓棗绠氬☉鎾筹梗缁楀懘寮崶椋庣獥break -> exit_block, continue -> inc_block
                        self.push_loop(exit_block, inc_block);

                        // 鐎甸偊浜為獮鍡樻媴閹垮嫮绀勫☉鎾崇У閸у﹪鏁?return闁?
                        self.lower_body_to_block_with_return(body, body_block, false);

                        // 闂侇偀鍋撻柛鎴濇惈閹﹪鎮抽娆戠憪濞戞挸顑嗛弸?
                        self.pop_loop();

                        // body_block 缂備焦鎸诲顐﹀触鎼淬倗鍎查弶鐑嗗墮閸?inc_block
                        if let Some(block) = self.mir_fn.block_mut(body_block) {
                            if block.terminator.is_none() {
                                block.set_terminator(Terminator::Goto(inc_block));
                            }
                        }

                        // 濠⒀呭仜婵偤宕稿Δ瀣獥濠⒀呭仜婵偛顕ラ鍡楃畾闁告瑦锕㈤崳?
                        self.set_current_block(inc_block);
                        let inc_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Load {
                            destination: inc_loaded,
                            source: loop_var,
                        });

                        let one = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Assign {
                            destination: one,
                            value: MirConstant::Int(1),
                        });

                        let inc_result = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Binary {
                            destination: inc_result,
                            op: MirBinOp::Add,
                            left: inc_loaded,
                            right: one,
                        });

                        self.push_inst(Instruction::Store {
                            destination: loop_var,
                            value: inc_result,
                        });

                        // 閻犲搫鐤囧ù鍡涘炊閻愬瓨钂嬪ù鐘烘硾濞?
                        self.set_terminator(Terminator::Goto(cond_block));

                        self.set_current_block(exit_block);
                        self.add_local(None, LocalKind::Temp, MIR_UNIT)
                    }
                    _ => {
                        // 閻忓繑绻嗛惁顖炲极閹殿喚鐭嬮弶鈺婂幒閸? for x in [1, 2, 3] 闁?for x in arr
                        let iter_local = self.lower_expr(iter);
                        let iter_ty = self.get_local_type(iter_local).clone();

                        match iter_ty {
                            MIRType::Array(elem_ty, len) => {
                                // 闁轰焦澹嗙划宥嗘交椤撴繂鏁? for x in arr { body }
                                let cond_block = self.new_block();
                                let body_block = self.new_block();
                                let inc_block = self.new_block();
                                let exit_block = self.new_block();

                                // 闁告帗绋戠紓鎾舵閵忕姷绌块柛娆愶耿閸ｆ椽鐛捄鍝勭仴濠殿喖顑呯€垫煡鏁?0
                                // 缂佷究鍨圭槐鈺呭矗濮椻偓閸ｆ椽妫侀埀顒傛啺娴ｅ憡韬€甸偊浜為獮鍡樼▔椤撶喐绾柡鍌氬簻缁辨繃鎷呯捄銊︽殢 User 缂侇偉顕ч悗?
                                let index_var = self.add_local(None, LocalKind::User, MIR_I64);
                                let init_val = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Assign {
                                    destination: init_val,
                                    value: MirConstant::Int(0),
                                });
                                self.push_inst(Instruction::Store {
                                    destination: index_var,
                                    value: init_val,
                                });

                                // 闁告帗绋戠紓鎾愁嚗椤忓棗绠氶柛娆愶耿閸ｆ椽鏁嶉崼婊呯憿闁轰焦澹嗙划宥夊礂閸愵亞顦辩紒顐ヮ嚙閻庣兘鎯勭粙鎸庡€遍柨?
                                let loop_var = self.add_local(
                                    Some(var_name.clone()),
                                    LocalKind::User,
                                    (*elem_ty).clone(),
                                );

                                // 闁告帗绋戠紓鎾诲极閹殿喚鐭嬮梻鈧崹顔碱唺閻㈩垱鎮傞崳?
                                let len_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Assign {
                                    destination: len_local,
                                    value: MirConstant::Int(len as i64),
                                });

                                // 閻犲搫鐤囧ù鍡涘礆閻楀牊钂嬪ù鐘烘硾濞?
                                self.set_terminator(Terminator::Goto(cond_block));

                                // 闁哄鈧弶顐介柛褎顨愮槐鏉课涢埀顒勬晸?index < len
                                self.set_current_block(cond_block);
                                let index_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: index_loaded,
                                    source: index_var,
                                });

                                let len_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: len_loaded,
                                    source: len_local,
                                });

                                // 婵絾妫佺欢?index < len
                                let cond_local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                                self.push_inst(Instruction::Binary {
                                    destination: cond_local,
                                    op: MirBinOp::Lt,
                                    left: index_loaded,
                                    right: len_loaded,
                                });

                                self.set_terminator(Terminator::If {
                                    cond: cond_local,
                                    then_block: body_block,
                                    else_block: exit_block,
                                });

                                // 閺夆晜绋戦崣鍡楊嚗椤忓棗绠氬☉鎾筹梗缁楀懘鏁?
                                self.push_loop(exit_block, inc_block);

                                // 鐎甸偊浜為獮鍡樻媴閹垮嫮绐楀Λ锝嗙墪閸樻盯宕濋悩鐑樼グ arr[index] 闁告帗婢橀幆濠囨偝椤栨艾缍侀柨?
                                self.set_current_block(body_block);

                                // 閻犱緤绱曢悾濠氬礂閸愵亞顦遍柛锔芥緲濞? &arr[index]
                                // 闁?load index_var闁挎稑婀秙er local闁挎稑顦崺?Temp闁挎稑鑻崯鈧ù鑲╁Х缁?IndexAddr
                                let index_for_addr = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: index_for_addr,
                                    source: index_var,
                                });
                                let elem_addr_local = self.add_local(
                                    None,
                                    LocalKind::Temp,
                                    MIRType::Ptr(elem_ty.clone()),
                                );
                                self.push_inst(Instruction::IndexAddr {
                                    destination: elem_addr_local,
                                    base: iter_local,
                                    index: index_for_addr,
                                });

                                // 闁告梻濮惧ù鍥礂閸愵亞顦遍柛濠勫帶閸╁苯顕ラ鍡楃畾闁告瑦锕㈤崳?
                                let elem_loaded =
                                    self.add_local(None, LocalKind::Temp, (*elem_ty).clone());
                                self.push_inst(Instruction::Load {
                                    destination: elem_loaded,
                                    source: elem_addr_local,
                                });

                                // 閻庢稒锚閸嬪秹宕氶弶鎸庡剷闁绘粠鍨拌ぐ澶愭晸?
                                self.push_inst(Instruction::Store {
                                    destination: loop_var,
                                    value: elem_loaded,
                                });

                                // 闂傚嫬绉崇紞鍡楊嚗椤忓棗绠氶柨?
                                self.lower_body_to_block_with_return(body, body_block, false);

                                // 闂侇偀鍋撻柛鎴濇惈閹﹪鎮抽娆戠憪濞戞挸顑嗛弸?
                                self.pop_loop();

                                // body_block 缂備焦鎸诲顐﹀触鎼淬倗鍎查弶鐑嗗墮閸?inc_block
                                if let Some(block) = self.mir_fn.block_mut(body_block) {
                                    if block.terminator.is_none() {
                                        block.set_terminator(Terminator::Goto(inc_block));
                                    }
                                }

                                // 濠⒀呭仜婵偤宕稿Δ瀣獥index++
                                self.set_current_block(inc_block);
                                let inc_loaded = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Load {
                                    destination: inc_loaded,
                                    source: index_var,
                                });

                                let one = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Assign {
                                    destination: one,
                                    value: MirConstant::Int(1),
                                });

                                let inc_result = self.add_local(None, LocalKind::Temp, MIR_I64);
                                self.push_inst(Instruction::Binary {
                                    destination: inc_result,
                                    op: MirBinOp::Add,
                                    left: inc_loaded,
                                    right: one,
                                });

                                self.push_inst(Instruction::Store {
                                    destination: index_var,
                                    value: inc_result,
                                });

                                // 閻犲搫鐤囧ù鍡涘炊閻愬瓨钂嬪ù鐘烘硾濞?
                                self.set_terminator(Terminator::Goto(cond_block));

                                self.set_current_block(exit_block);
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            }
                            _ => {
                                // 濞戞挸绉甸弫顕€骞愭担鐑樼暠閺夆晩鍘洪崬顒勫闯閵娧嗩潶闁?
                                self.errors.push(format!(
                                    "for loop: unsupported iterator type: {:?}",
                                    iter_ty
                                ));
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            }
                        }
                    }
                }
            }
            HIRExpr::Call { func, args } => {
                let arg_locals: Vec<Local> = args.iter().map(|a| self.lower_expr(a)).collect();

                // 闁兼儳鍢茶ぐ鍥礄閼恒儲娈堕柛姘Т閹风増娼婚弬鎸庣缂侇偉顕ч悗鐑芥晬鐏炵偓鏆滈柨?Lambda 閻犲鍟伴弫?
                let (func_name, ret_type, env_ptr_local) = match func.as_ref() {
                    HIRExpr::Var { name, .. } => {
                        // Prefer local function-valued variables (e.g. lambdas) over builtins.
                        if let Some(&var_local) = self.local_names.get(name) {
                            if let Some(lambda_name) = self.lambda_names.get(&var_local) {
                                let ret = self
                                    .function_sigs
                                    .get(lambda_name)
                                    .map(|sig| sig.ret_type.clone())
                                    .unwrap_or(MIR_I64);

                                let env_ptr = self
                                    .lambda_environments
                                    .get(lambda_name)
                                    .and_then(|env| env.env_ptr_local);

                                (lambda_name.clone(), ret, env_ptr)
                            } else {
                                let local_ty = self.get_local_type(var_local).clone();
                                if let MIRType::Fn { ret, .. } = &local_ty {
                                    (mir_local_name(var_local), (**ret).clone(), None)
                                } else {
                                    let ret = self
                                        .function_sigs
                                        .get(name)
                                        .map(|sig| sig.ret_type.clone())
                                        .unwrap_or(MIR_I64);
                                    (name.clone(), ret, None)
                                }
                            }
                        } else if name == "print" {
                            return self.lower_builtin_print(&arg_locals);
                        } else {
                            let ret = self
                                .function_sigs
                                .get(name)
                                .map(|sig| sig.ret_type.clone())
                                .unwrap_or(MIR_I64);
                            (name.clone(), ret, None)
                        }
                    }
                    _ => (String::new(), MIR_UNIT, None),
                };

                let local: Local = self.add_local(None, LocalKind::Temp, ret_type.clone());
                if let MIRType::Struct { name, .. } = &ret_type {
                    self.type_names.insert(local, name.clone());
                }

                // 濠碘€冲€归悘澶愬嫉婢跺苯绠氬褍鍟€垫岸鏌﹂崼顒傜閻忓繐妫楅崣鐐媴濠娾偓鐠愮喓绮璺伇濞戞搩浜滃顒勫极妫颁胶鐐婇柨?
                let mut final_args = Vec::new();
                if let Some(env_ptr) = env_ptr_local {
                    final_args.push(env_ptr);
                }
                final_args.extend(arg_locals);

                let is_async_call = self.options.async_functions.contains(&func_name);
                let actual_func = if is_async_call {
                    format!("{}__start", func_name.clone())
                } else {
                    func_name.clone()
                };
                self.push_inst(Instruction::Call {
                    destination: local,
                    func: actual_func,
                    args: final_args,
                });
                // Track which async function produced this future handle.
                if is_async_call {
                    self.future_origins.insert(local, func_name);
                }
                local
            }
            HIRExpr::And(left, right) => {
                // 闁活収鍙€閻箖鏌呴弰蹇曞竼闁?- 缂佺姭鍋撻柛鏍ㄧ墧鐠愮喐绂嶇仦钘夊笚閺夆晜鍔楅悾?
                let left_local = self.lower_expr(left);
                let right_local = self.lower_expr(right);
                let local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                self.push_inst(Instruction::Binary {
                    destination: local,
                    op: MirBinOp::LogAnd,
                    left: left_local,
                    right: right_local,
                });
                local
            }
            HIRExpr::Or(left, right) => {
                // 闁活収鍙€閻箖鏌呴弰蹇曞竼闁?- 缂佺姭鍋撻柛鏍ㄧ墧鐠愮喐绂嶇仦钘夊笚閺夆晜鍔楅悾?
                let left_local = self.lower_expr(left);
                let right_local = self.lower_expr(right);
                let local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
                self.push_inst(Instruction::Binary {
                    destination: local,
                    op: MirBinOp::LogOr,
                    left: left_local,
                    right: right_local,
                });
                local
            }
            HIRExpr::Break(value) => {
                // 濠㈣泛瀚幃?break
                if let Some(target) = self.get_break_target() {
                    // 闂傚嫬绉崇紞鍡涘矗椤栫偐鍋撴径灞剧暠閺夆晜鏌ㄥú鏍晸?
                    if let Some(v) = value {
                        self.lower_expr(v);
                    }
                    self.set_terminator(Terminator::Break { target });
                    // break 闁告艾绨肩粭澶愬矗椤栨繃褰ч柨娑樼焷缁绘垿宕堕悙鎵伇濞戞搩浜滃畷鐗堟媴瀹ュ浂鍎?Local
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                } else {
                    self.errors.push("break outside of loop".to_string());
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
            HIRExpr::Continue => {
                // 濠㈣泛瀚幃?continue
                if let Some(target) = self.get_continue_target() {
                    self.set_terminator(Terminator::Continue { target });
                    // continue 闁告艾绨肩粭澶愬矗椤栨繃褰ч柨娑樼焷缁绘垿宕堕悙鎵伇濞戞搩浜滃畷鐗堟媴瀹ュ浂鍎?Local
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                } else {
                    self.errors.push("continue outside of loop".to_string());
                    self.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
            HIRExpr::Assign { target, value } => {
                // 閻犙冾儏閳ь剝澹堥妴鍐╂綇閹呯: target = value
                // 闂傚嫬绉崇紞鍡涘矗缁涜瀚?
                let value_local = self.lower_expr(value);

                // 闂傚嫬绉崇紞鍡楊啅閿斿吋瀚?闁?闁兼儳鍢茶ぐ鍥儎椤旂晫鍨奸柛娆愶耿閸?
                match target.as_ref() {
                    HIRExpr::Var { name, symbol } => {
                        let target_local = self.resolve_local(name, *symbol);
                        if value_local == target_local {
                            // Skip no-op self-assignment (`x = x`) to reduce temp churn.
                            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                        }
                        // 濞磋偐濮甸幐杈╃尵鐠囪尙鈧兘宕ュ鍥嗙偤鏁嶅顒夋搐闁哄绮岃ぐ鎼佸磹閸忓吋绠掔紒顐ヮ嚙閻庣兘宕ュ鍥嗙偤鏁嶇仦鐣屾闁稿繑婀圭槐鍫曞箻椤撶偛鐓傞柣鈺婂枟閻?local
                        if let Some(type_name) = self.type_names.get(&value_local).cloned() {
                            self.type_names.insert(target_local, type_name);
                        }
                        self.push_inst(Instruction::Store {
                            destination: target_local,
                            value: value_local,
                        });
                    }
                    HIRExpr::Index { base, index } => {
                        // 闁轰焦澹嗙划宥夊礂閸愵亞顦遍悹褍顑戦幏? arr[i] = value
                        let base_local = self.lower_expr(base);
                        let index_local = self.lower_expr(index);

                        // 閻犱緤绱曢悾濠氬礂閸愵亞顦遍柛锔芥緲濞?
                        let base_ty = self.get_local_type(base_local).clone();
                        let elem_ty = match &base_ty {
                            MIRType::Array(elem, _) => (**elem).clone(),
                            _ => {
                                self.errors
                                    .push("index assignment on non-array type".to_string());
                                return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                            }
                        };

                        let addr_local =
                            self.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(elem_ty)));
                        self.push_inst(Instruction::IndexAddr {
                            destination: addr_local,
                            base: base_local,
                            index: index_local,
                        });

                        // 閻庢稒锚閸嬪秹宕愰悡搴＄厒閻犱緤绱曢悾濠氬礄閾忚鐣遍柛锔芥緲濞?
                        self.push_inst(Instruction::Store {
                            destination: addr_local,
                            value: value_local,
                        });
                    }
                    _ => {
                        self.errors.push(format!("unsupported assignment target"));
                    }
                }
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::AssignOp { target, op, value } => {
                // 濠㈣泛绉撮幃搴ｆ導鐎ｎ亖鍋撻懝鑸偓鍐╂綇閹呯: target op= value (e.g., x += 1)
                // 闂傚嫬绉崇紞鍡涘矗缁涜瀚?
                let value_local = self.lower_expr(value);

                match target.as_ref() {
                    HIRExpr::Var { name, symbol } => {
                        let target_local = self.resolve_local(name, *symbol);
                        // 闁告梻濮惧ù鍥亹閹惧啿顤呴柨?
                        let target_ty = self.get_local_type(target_local).clone();
                        let current_val = self.add_local(None, LocalKind::Temp, target_ty.clone());
                        self.push_inst(Instruction::Load {
                            destination: current_val,
                            source: target_local,
                        });
                        // 闁圭瑳鍡╂斀閺夆晜鍔楅悾?
                        let mir_op = self.lower_bin_op(op);
                        let result = self.add_local(None, LocalKind::Temp, target_ty);
                        self.push_inst(Instruction::Binary {
                            destination: result,
                            op: mir_op,
                            left: current_val,
                            right: value_local,
                        });
                        // 閻庢稒锚閸嬪秶绱掗幘瀵镐函
                        self.push_inst(Instruction::Store {
                            destination: target_local,
                            value: result,
                        });
                    }
                    HIRExpr::Index { base, index } => {
                        // 闁轰焦澹嗙划宥夊礂閸愵亞顦卞璺虹Т閹海鎸х€ｈ埖瀚? arr[i] += value
                        let base_local = self.lower_expr(base);
                        let index_local = self.lower_expr(index);

                        // 閻犱緤绱曢悾濠氬礂閸愵亞顦遍柛锔芥緲濞?
                        let base_ty = self.get_local_type(base_local).clone();
                        let elem_ty = match &base_ty {
                            MIRType::Array(elem, _) => (**elem).clone(),
                            _ => {
                                self.errors.push(
                                    "index compound assignment on non-array type".to_string(),
                                );
                                return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                            }
                        };

                        let addr_local = self.add_local(
                            None,
                            LocalKind::Temp,
                            MIRType::Ptr(Box::new(elem_ty.clone())),
                        );
                        self.push_inst(Instruction::IndexAddr {
                            destination: addr_local,
                            base: base_local,
                            index: index_local,
                        });

                        // 闁告梻濮惧ù鍥亹閹惧啿顤呴柛蹇撳暟缁€宀勬晸?
                        let current_val = self.add_local(None, LocalKind::Temp, elem_ty.clone());
                        self.push_inst(Instruction::Load {
                            destination: current_val,
                            source: addr_local,
                        });

                        // 闁圭瑳鍡╂斀閺夆晜鍔楅悾?
                        let mir_op = self.lower_bin_op(op);
                        let result = self.add_local(None, LocalKind::Temp, elem_ty);
                        self.push_inst(Instruction::Binary {
                            destination: result,
                            op: mir_op,
                            left: current_val,
                            right: value_local,
                        });

                        // 閻庢稒锚閸嬪秶绱掗幘瀵镐函闁搞儳鍋涢崢鎾舵閻樺弶鍕鹃柛褉鍋?
                        self.push_inst(Instruction::Store {
                            destination: addr_local,
                            value: result,
                        });
                    }
                    _ => {
                        self.errors
                            .push(format!("unsupported compound assignment target"));
                    }
                }
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            HIRExpr::Array(elems) => {
                // 闁轰焦澹嗙划宥団偓娑欘殜濞间即鏁?[a, b, c]
                // 闂傚嫬绉崇紞鍡椥掕箛搴ㄥ殝闁稿繐鍟扮粈宀勭嵁閼稿灚鏆梻鍡楁閻ｇ姵绂掗鍌涚暠 locals
                let elem_locals: Vec<Local> = elems.iter().map(|e| self.lower_expr(e)).collect();

                // 缁绢収鍠栭悾楣冨礂閸愵亞顦辩紒顐ヮ嚙閻庣兘宕仦鐐缂備礁瀚悮顐︽晸?
                let elem_ty = if let Some(first_local) = elem_locals.first() {
                    self.get_local_type(*first_local).clone()
                } else {
                    MIR_UNIT
                };
                let array_ty = MIRType::Array(Box::new(elem_ty), elems.len() as u64);

                // 闁轰焦澹嗙划宥夋閳ь剛鎲版担鍛婅含闁告劕鎳庨悺銊︾▔椤撶偛鐎婚梺鏉跨Ф閳规牠姊绘潏鍓х濞达綀娉曢弫?User 缂侇偉顕ч悗?
                let array_local = self.add_local(None, LocalKind::User, array_ty.clone());
                self.push_inst(Instruction::Aggregate {
                    destination: array_local,
                    fields: elem_locals,
                    ty: array_ty,
                });

                array_local
            }
            HIRExpr::Index { base, index } => {
                // 闁轰焦澹嗙划宥囨閵忕姷绌?arr[i]
                let base_local = self.lower_expr(base);
                let index_local = self.lower_expr(index);

                // 闁兼儳鍢茶ぐ鍥极閹殿喚鐭嬬紒顐ヮ嚙閻庨攱绂掗妷褉鈧鈧鑹鹃崢鎾舵閻樹絻顫﹂柨?
                let base_ty = self.get_local_type(base_local).clone();
                let elem_ty = match base_ty {
                    MIRType::Array(elem, _) => *elem,
                    _ => MIR_UNIT,
                };

                // 闁告帗绋戠紓?IndexAddr 闁圭娲ｉ幎銈夊级閵夘煈鍚€缂佺姵顨呴崢鎾舵閻樺弶鍕鹃柛褉鍋?
                let addr_local = self.add_local(
                    None,
                    LocalKind::Temp,
                    MIRType::Ptr(Box::new(elem_ty.clone())),
                );
                self.push_inst(Instruction::IndexAddr {
                    destination: addr_local,
                    base: base_local,
                    index: index_local,
                });

                // 濞寸姴楠稿﹢鎾锤閳ь剟宕濋悩鐑樼グ闁?
                let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                self.push_inst(Instruction::Load {
                    destination: result_local,
                    source: addr_local,
                });

                result_local
            }
            HIRExpr::Struct { name, fields } => {
                let lowered_fields: Vec<(String, Local)> = fields
                    .iter()
                    .map(|(field_name, expr)| (field_name.clone(), self.lower_expr(expr)))
                    .collect();
                let field_locals_by_name: HashMap<String, Local> = lowered_fields
                    .iter()
                    .map(|(field_name, local)| (field_name.clone(), *local))
                    .collect();

                let struct_ty = self
                    .infer_struct_literal_type(name, &field_locals_by_name)
                    .unwrap_or_else(|| MIRType::Struct {
                        name: name.clone(),
                        fields: lowered_fields
                            .iter()
                            .map(|(field_name, local)| {
                                (field_name.clone(), self.get_local_type(*local).clone())
                            })
                            .collect(),
                    });

                let ordered_field_locals: Vec<Local> = match &struct_ty {
                    MIRType::Struct { fields, .. } => fields
                        .iter()
                        .filter_map(|(field_name, _)| field_locals_by_name.get(field_name).copied())
                        .collect(),
                    _ => lowered_fields.iter().map(|(_, local)| *local).collect(),
                };

                let struct_local = self.add_local(None, LocalKind::Temp, struct_ty.clone());
                self.push_inst(Instruction::Aggregate {
                    destination: struct_local,
                    fields: ordered_field_locals,
                    ty: struct_ty.clone(),
                });

                if let MIRType::Struct { name, .. } = &struct_ty {
                    self.type_names.insert(struct_local, name.clone());
                }

                struct_local
            }
            HIRExpr::Field { base, field } => {
                // 閻庢稒顨嗛宀€鎷嬮崸妤侊紪 obj.field
                let base_local = self.lower_expr(base);

                // 閻庣敻鈧稓鑹惧ù锝堟硶閺?Tuple 閻炴稏鍔庨妵姘舵儍閸曨厾娉㈤柡瀣缂嶅鏁嶇仦鐓庘枏闁活潿鍔庨崒銊ヮ嚕閺団槅鍟忛柨?
                // 濞戞挸鐡ㄥ鍌炲棘鐟欏嫷鏀抽柨娑欐皑閳ユ牜绱撻弽顐ゅ灣閻㈩垱鐡曢～鍡欌偓娑欘殕椤斿矂宕ュ鍛厒缂佷究鍨圭槐鈺呮儍閸曨剚衼闁?
                let base_ty = self.get_local_type(base_local).clone();
                let field_index = match &base_ty {
                    MIRType::Struct { fields, .. } => fields
                        .iter()
                        .position(|(name, _)| name == field)
                        .unwrap_or(0),
                    // Tuple fallback for legacy method/struct lowering paths.
                    _ => match field.as_str() {
                        "x" | "left" | "r" => 0,
                        "y" | "right" | "g" => 1,
                        "z" | "b" => 2,
                        "w" | "a" => 3,
                        _ => 0,
                    },
                };
                let elem_ty = match &base_ty {
                    MIRType::Tuple(ref tys) if field_index < tys.len() => tys[field_index].clone(),
                    MIRType::Struct { fields, .. } if field_index < fields.len() => {
                        fields[field_index].1.clone()
                    }
                    _ => MIR_I64,
                };

                // 缂備焦鎸婚悗顖炴晸?闁稿繐鍟扮划宥夊及椤栨埃鍋撻懖鈺勵潶闁搞劌顑戠槐婵囨媴鐠恒劍鏆?Extract (extractvalue) 闁兼澘鐭傚?FieldAddr+Load
                let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                self.push_inst(Instruction::Extract {
                    destination: result_local,
                    value: base_local,
                    index: field_index as u32,
                });

                result_local
            }
            HIRExpr::Ref(_is_mut, expr) => {
                // 鐎殿喗娲滈弫?&expr - 闁哄棗鍊瑰鍌涙交閺傛寧绀€閻炴稏鍔忛幓顏勵嚕韫囨洘鐣遍柛锔芥緲濞?
                let expr_local = self.lower_expr(expr);
                let expr_ty = self.get_local_type(expr_local).clone();

                // 闁告帗绋戠紓鎾诲箰閸ヮ剚瀚涚紒顐ヮ嚙閻?
                let ptr_ty = MIRType::Ptr(Box::new(expr_ty));
                let ptr_local = self.add_local(None, LocalKind::Temp, ptr_ty);

                // 閻庣敻鈧稓鑹鹃悘鐐╁亾闂侇喓鍔岃ぐ澶愭煂韫囥儳绀夐柤鎯у槻瑜板洭宕楃捄鐑樺嬀闁秆€鍋撻柨娑樼墔婵炲洭鏁?IndexAddr with index 0闁?
                let zero_index = self.add_local(None, LocalKind::Temp, MIR_I64);
                self.push_inst(Instruction::Assign {
                    destination: zero_index,
                    value: MirConstant::Int(0),
                });

                self.push_inst(Instruction::IndexAddr {
                    destination: ptr_local,
                    base: expr_local,
                    index: zero_index,
                });

                ptr_local
            }
            HIRExpr::Deref(expr) => {
                // 閻熸瑱绲界槐鈺呮晸?*ptr
                let ptr_local = self.lower_expr(expr);
                let ptr_ty = self.get_local_type(ptr_local).clone();

                let elem_ty = match ptr_ty {
                    MIRType::Ptr(inner) | MIRType::Ref(inner) => *inner,
                    _ => MIR_UNIT,
                };

                let result_local = self.add_local(None, LocalKind::Temp, elem_ty);
                self.push_inst(Instruction::Load {
                    destination: result_local,
                    source: ptr_local,
                });

                result_local
            }
            HIRExpr::Lambda { params, body } => {
                // Lambda 闂傚偆鍘肩€?|args| body
                // 闁告帗绋戠紓鎾寸▔閳ь剚绋夐鍥╃闁告柡鏅涢崵閬嶅极閺夎儻瀚欓弶鈺傛煥濞叉牠宕欓懞銉︽鐎殿喗娲滈弫?

                // 闁汇垻鍠愰崹姘跺船椤栨瑧顏遍柨?Lambda 闁告垼濮ら弳鐔兼晸?
                let lambda_name = self.lambda_name();

                // 闁衡偓閸洘鑲犻柤濂変簽閺侀亶宕ｅ鈧崳娲晬閸垹绠氬褍鍟畷鐔兼嚔閸戙倗绀?
                let free_vars = self.collect_free_vars(params, body);

                // Lambda 缂侇偉顕ч悗鐑芥晬濮樺墎甯涢悹浣靛€曞顒勫极閺夋寧瀚查弶鈺傛煥濞叉牜鐚剧拠鑼偓鐑芥焾閼恒儲笑 i64
                let mut param_types: Vec<MIRType> = (0..params.len()).map(|_| MIR_I64).collect();
                let ret_type = MIR_I64;

                // 濠碘€冲€归悘澶愬嫉婢跺骸娈伴柣銏犲船瑜板鏌岃箛銉х婵烇綀顕ф慨鐐烘偝椤栨凹鏆旈柛娆忓€归弳鐔告媴濠娾偓鐠愮喓绮璺伇濞戞搩浜滃顒勬晸?
                let env_param_offset = if free_vars.is_empty() {
                    0
                } else {
                    // 闁绘粠鍨伴。銊╁矗閸屾稒娈堕柨娑欑煯婵炲洭鎮介妸褏娉㈤柡瀣缂嶅鐚剧拠鑼偓椋庢偘閵娧佷粵闁硅娲濋獮蹇涙儍閸曨厼绠氶柨?
                    // 缂佺姭鍋撻柛鏍ㄧ壄缁辩増鎷呯捄銊︽殢 i64* 闁圭娲幏锟犲箰閸パ勫€婚柣婊庡灠椤?
                    param_types.insert(0, MIRType::Ptr(Box::new(MIR_I64)));
                    1
                };

                // 闁告帗绋戠紓?Lambda 閺夊牆鎳庢慨顏堝礄閼恒儲娈?
                let mut lambda_fn =
                    MirFunction::new(lambda_name.clone(), param_types.clone(), ret_type.clone());
                let lambda_start = lambda_fn.start_block;
                let mut lambda_ctx =
                    LoweringContext::new(
                        &mut lambda_fn,
                        self.lambda_counter,
                        &self.known_functions,
                        &self.function_sigs,
                        self.struct_defs,
                        self.concrete_type_registry.clone(),
                        self.options.clone(),
                        self.inherent_method_templates,
                        self.trait_method_templates,
                    );
                // Set current block for Lambda function entry
                lambda_ctx.current_block = Some(lambda_start);

                // 缂備焦鍨甸悾楣冩偝椤栨凹鏆旈柛娆忓€归弳鐔兼晸?Lambda 闁告瑥鍊归弳鐔兼晸?Lambda 闁告垼濮ら弳?
                if !free_vars.is_empty() {
                    // 缂佹鍏涚粩瀛樼▔椤忓嫬妫橀柡浣哄濡叉悂鎮抽姘兼殧闁挎稑鐗婄€垫岸鏌﹂崼顒傜
                    let env_local = Local::new(1, LocalKind::Param);
                    let env_ptr_name = "__env".to_string();
                    lambda_ctx
                        .local_names
                        .insert(env_ptr_name.clone(), env_local);

                    // 濞寸姴娴烽獮鍡樻櫠閸愩劌顫ｉ弶鐐跺Г瀹曠喖鎳㈡搴㈢暠闁告瑦锕㈤崳?
                    // 闁绘粠鍨伴。銊╁及椤栨瑧顏卞☉鎿冧簽缁劑寮搁崟顏嗙Ъ闁挎稑鏈惁鈩冪▔椤忓懎绀嬮柤楣冾棑濞堟垿宕ｅ鈧崳娲箰婢舵劑鈧孩鎯旇箛鎾舵憼闁?
                    for (i, (var_name, _)) in free_vars.iter().enumerate() {
                        // 濞戞挾鍎ゅ畷鐔兼嚔妞嬪孩鐣遍柛娆愶耿閸ｆ椽宕氬☉妯肩处濞戞挴鍋撻柨?local
                        let captured_local =
                            lambda_ctx.add_local(Some(var_name.clone()), LocalKind::Temp, MIR_I64);

                        // 濞寸姴娴烽獮鍡樻櫠閸愨晛鐦归梺钘夌墕婵偞娼挊澶婄秮闁?
                        // 濞达綀娉曢弫?getelementptr 闁?load
                        let index_local = lambda_ctx.add_local(None, LocalKind::Temp, MIR_I64);
                        lambda_ctx.push_inst(Instruction::Assign {
                            destination: index_local,
                            value: MirConstant::Int(i as i64),
                        });

                        let ptr_local = lambda_ctx.add_local(
                            None,
                            LocalKind::Temp,
                            MIRType::Ptr(Box::new(MIR_I64)),
                        );
                        lambda_ctx.push_inst(Instruction::IndexAddr {
                            destination: ptr_local,
                            base: env_local,
                            index: index_local,
                        });

                        // 闁告梻濮惧ù鍥晸?
                        lambda_ctx.push_inst(Instruction::Load {
                            destination: captured_local,
                            source: ptr_local,
                        });

                        // 閻忓繐妫欏畷鐔兼嚔妞嬪孩鐣遍柛娆愶耿閸ｈ櫣绱掗幋婵堟毎闁告帗婢橀幃鏇犵矓鐢喚绀勯弶鈺傜懄閻?body 濞戞搩鍘煎銊╁矗椤栨瑤绨伴柣鈺佺摠鐢瓨鎷呯捄銊︽殢濞存粌妫寸槐?
                        lambda_ctx
                            .local_names
                            .insert(var_name.clone(), captured_local);
                    }

                    // 缂備焦鍨甸悾?Lambda 闁告瑥鍊归弳鐔兼晬閸繀鐒婇柨?1闁挎稑鑻ú婊勭▔閾忕懓绠氬褍鍟顒勫极閺夊灝绐楅柣顫妺缁ㄢ剝鎷呭鍥╂瀭 1闁?
                    for (i, param_name) in params.iter().enumerate() {
                        let local = Local::new(i + 1 + env_param_offset, LocalKind::Param);
                        lambda_ctx.local_names.insert(param_name.clone(), local);
                    }
                } else {
                    // 婵炲备鍓濆﹢渚€鎮抽姘兼殧闁挎稑鏈婊呮暜閸濄儳鎷ㄩ悗瑙勮壘瀵剟鏁?
                    for (i, param_name) in params.iter().enumerate() {
                        let local = Local::new(i + 1 + env_param_offset, LocalKind::Param);
                        lambda_ctx.local_names.insert(param_name.clone(), local);
                    }
                }

                // 闂傚嫬绉崇紞?body 闁?Lambda 闁告垼濮ら弳?
                // Lambda body 闁?HIRExpr闁挎稑鐭傚〒鍓佹啺娴ｇ鐦堕悷浣告噺閸?HIRBody
                use crate::hir::HIRBody;
                let lambda_body = HIRBody {
                    stmts: vec![],
                    expr: Some(body.clone()),
                };
                lambda_ctx.lower_body_to_block(&lambda_body, lambda_start);

                // 閻忓繐妫涢弫鎾诲箣閹邦喗鐣?Lambda 闁告垼濮ら弳鐔非庣拠鎻掝潱闁告帗婢橀崹顏嗘偘閵娿倛鍘?
                self.lambda_functions.push(lambda_fn);

                // 閻犱焦婢樼紞宥夋偝椤栨凹鏆斿ǎ鍥ｅ墲娴?
                if !free_vars.is_empty() {
                    let env_var_types: Vec<(String, MIRType)> = free_vars
                        .iter()
                        .map(|(name, local)| (name.clone(), self.get_local_type(*local).clone()))
                        .collect();
                    self.lambda_environments.insert(
                        lambda_name.clone(),
                        LambdaEnv {
                            vars: free_vars.clone(),
                            env_type: MIRType::Ptr(Box::new(MIR_I64)),
                            env_ptr_local: None, // 缂佸绉撮幃妤呮晸?Let lowering 濞戞搩鍙€椤旀洟鏁?
                        },
                    );

                    // 閻犱焦婢樼紞宥夊礄閼恒儲娈剁紒娑欏劤閹洟鏁嶉崼婵嗙樁闁告凹鍋嗛獮鍡樻櫠閸愌傜箚闁诡収鍨界槐?
                    self.function_sigs.insert(
                        lambda_name.clone(),
                        FunctionSig {
                            ret_type: ret_type.clone(),
                            param_count: param_types.len(),
                            env: env_var_types,
                        },
                    );
                } else {
                    // 閻犱焦婢樼紞宥夊礄閼恒儲娈剁紒娑欏劤閹洟鏁嶉崼鐔革骏闁绘粠鍨伴。銊╂晸?
                    self.function_sigs.insert(
                        lambda_name.clone(),
                        FunctionSig {
                            ret_type: ret_type.clone(),
                            param_count: param_types.len(),
                            env: vec![],
                        },
                    );
                }

                // 闁告帗绋戠紓鎾寸▔閳ь剚绋夐鍐槻闁?local 闁哄鍎遍悺銊╂晸?Lambda 闁告垼濮ら弳鐔兼晸?
                // 濞达綀娉曢弫銈夊极鐎涙ɑ娈剁紒顐ヮ嚙閻庨攱鎷呭鈧拹?Lambda 闁汇劌瀚妴鍐矆閻氬绀勯柛鎴ｅГ閺嗙喖骞愰崶顒佸珱闁?
                let lambda_local = if free_vars.is_empty() {
                    let fn_ty = MIRType::Fn {
                        params: param_types.clone(),
                        ret: Box::new(ret_type.clone()),
                    };
                    let local = self.add_local(None, LocalKind::Temp, fn_ty);
                    self.push_inst(Instruction::Assign {
                        destination: local,
                        value: MirConstant::GlobalRef(lambda_name.clone()),
                    });
                    local
                } else {
                    self.add_local(None, LocalKind::Temp, MIR_I64)
                };

                // ??? Local -> Lambda ?????????
                self.lambda_names.insert(lambda_local, lambda_name.clone());

                lambda_local
            }
            HIRExpr::Match { scrutinee, arms } => {
                let scrutinee_local = self.lower_expr(scrutinee);
                let scrutinee_ty = self.get_local_type(scrutinee_local).clone();

                match scrutinee_ty {
                    MIRType::Enum { .. } => {
                        let discr_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                        self.push_inst(Instruction::Discriminant {
                            destination: discr_local,
                            source: scrutinee_local,
                        });

                        let arm_blocks: Vec<usize> =
                            arms.iter().map(|_| self.new_block()).collect();
                        let join_block = self.new_block();

                        let mut targets = Vec::new();
                        let mut otherwise_block = join_block;
                        for (i, arm) in arms.iter().enumerate() {
                            if let Some(value) = self.extract_discriminant_from_pattern(&arm.pat) {
                                targets.push((value, arm_blocks[i]));
                            } else {
                                otherwise_block = arm_blocks[i];
                            }
                        }

                        self.set_terminator(Terminator::Switch {
                            discr: discr_local,
                            targets,
                            otherwise: otherwise_block,
                        });

                        let mut incoming_values: Vec<(Local, usize)> = Vec::new();
                        for (i, arm) in arms.iter().enumerate() {
                            let arm_block = arm_blocks[i];
                            self.set_current_block(arm_block);

                            self.lower_pattern_bindings(&arm.pat, scrutinee_local);
                            let arm_result = self.lower_expr(&arm.body);
                            let arm_end = self.current_block();

                            if let Some(block) = self.mir_fn.block_mut(arm_end) {
                                if block.terminator.is_none() {
                                    block.set_terminator(Terminator::Goto(join_block));
                                    incoming_values.push((arm_result, arm_end));
                                }
                            }
                        }

                        self.set_current_block(join_block);
                        if let Some((first_value, _)) = incoming_values.first().copied() {
                            let result_ty = self.get_local_type(first_value).clone();
                            let is_void_like = match &result_ty {
                                MIRType::Unit | MIRType::Never => true,
                                MIRType::Tuple(fields) if fields.is_empty() => true,
                                _ => false,
                            };
                            if is_void_like {
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            } else {
                                let result = self.add_local(None, LocalKind::Temp, result_ty);
                                self.push_inst(Instruction::Phi {
                                    destination: result,
                                    incoming: incoming_values,
                                });
                                result
                            }
                        } else {
                            self.add_local(None, LocalKind::Temp, MIR_UNIT)
                        }
                    }
                    _ => {
                        let join_block = self.new_block();
                        let mut incoming_values: Vec<(Local, usize)> = Vec::new();

                        for (i, arm) in arms.iter().enumerate() {
                            let is_last = i == arms.len() - 1;

                            if is_last {
                                let arm_result = self.lower_expr(&arm.body);
                                let arm_end = self.current_block();
                                if let Some(block) = self.mir_fn.block_mut(arm_end) {
                                    if block.terminator.is_none() {
                                        block.set_terminator(Terminator::Goto(join_block));
                                        incoming_values.push((arm_result, arm_end));
                                    }
                                }
                            } else {
                                let then_block = self.new_block();
                                let next_arm_block = self.new_block();

                                let should_take = self.matches_pattern(&arm.pat, scrutinee_local);
                                self.set_terminator(Terminator::If {
                                    cond: should_take,
                                    then_block,
                                    else_block: next_arm_block,
                                });

                                self.set_current_block(then_block);
                                let arm_result = self.lower_expr(&arm.body);
                                let arm_end = self.current_block();
                                if let Some(block) = self.mir_fn.block_mut(arm_end) {
                                    if block.terminator.is_none() {
                                        block.set_terminator(Terminator::Goto(join_block));
                                        incoming_values.push((arm_result, arm_end));
                                    }
                                }

                                self.set_current_block(next_arm_block);
                            }
                        }

                        self.set_current_block(join_block);
                        if let Some((first_value, _)) = incoming_values.first().copied() {
                            let result_ty = self.get_local_type(first_value).clone();
                            let is_void_like = match &result_ty {
                                MIRType::Unit | MIRType::Never => true,
                                MIRType::Tuple(fields) if fields.is_empty() => true,
                                _ => false,
                            };
                            if is_void_like {
                                self.add_local(None, LocalKind::Temp, MIR_UNIT)
                            } else {
                                let result = self.add_local(None, LocalKind::Temp, result_ty);
                                self.push_inst(Instruction::Phi {
                                    destination: result,
                                    incoming: incoming_values,
                                });
                                result
                            }
                        } else {
                            self.add_local(None, LocalKind::Temp, MIR_UNIT)
                        }
                    }
                }
            }
            HIRExpr::MethodCall {
                receiver,
                method,
                args,
            } => {
                // 闁哄倽顫夌涵鍓佹嫬閸愵亝鏆?receiver.method(args)
                // 闂傚嫬绉崇紞鍡樼▔閻戞ɑ鐝梺顐ｈ壘閸ら亶寮幏宀€娈堕柨? TypeName_method(receiver, args)

                // 闂傚嫬绉崇紞鍡涘箳閵夛附鏆柨?
                let receiver_local = self.lower_expr(receiver);
                let receiver_ty = self.get_local_type(receiver_local).clone();

                // 闂傚嫬绉崇紞鍡涘矗閸屾稒娈?
                let arg_locals: Vec<Local> = args.iter().map(|a| self.lower_expr(a)).collect();

                // String built-in method handling: when receiver is a string
                // (Ptr to i8), intercept known methods and generate runtime calls.
                if let MIRType::Ptr(inner) = &receiver_ty {
                    if let MIRType::Int(8) = inner.as_ref() {
                        if method == "len" {
                            // Generate call to sengoo_str_len(receiver) -> i64
                            let result_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Call {
                                destination: result_local,
                                func: "sengoo_str_len".to_string(),
                                args: vec![receiver_local],
                            });
                            return result_local;
                        }
                    }
                }

                // 闁汇垻鍠愰崹姘跺棘鐟欏嫮銆婇柛鎴ｅГ閺嗙喖宕ュ蹇曠獥TypeName_method
                // 闂侇剟娼ч幆?Sengoo 闁告稖妫勯幃鏇犵棯閿曗偓閻?
                // 濡絾鐗曢崢娑樜涢埀顒勬晸?type_names 濞寸姰鍎撮獮蹇涘矗閺嵮呮澖闂傚嫬鎳愬▓鎴犵磼閹惧鈧垱鎷呴幘鎯邦潶闁搞劌顑呴幃鏇㈡晸?
                let resolved_func_name = match self.resolve_method_call_target(
                    receiver_local,
                    &receiver_ty,
                    method,
                    &arg_locals,
                ) {
                    Ok(name) => name,
                    Err(error) => {
                        self.errors.push(error);
                        return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                    }
                };



                // 缁绢収鍠栭悾鐐交閺傛寧绀€缂侇偉顕ч悗鐑芥晬閸儳甯涢柨?i64闁?
                let ret_type = self
                    .function_sigs
                    .get(&resolved_func_name)
                    .map(|sig| sig.ret_type.clone())
                    .unwrap_or(MIR_I64);
                let result_local = self.add_local(None, LocalKind::Temp, ret_type.clone());
                if let MIRType::Struct { name, .. } = &ret_type {
                    self.type_names.insert(result_local, name.clone());
                }

                // 闁哄瀚紓鎾诲矗閸屾稒娈堕柛鎺擃殙閵嗗啴鏁嶅鐮甤eiver + args
                let mut call_args = vec![receiver_local];
                call_args.extend(arg_locals);

                // 闁汇垻鍠愰崹?Call 闁圭娲ｉ幎?
                self.push_inst(Instruction::Call {
                    destination: result_local,
                    func: resolved_func_name,
                    args: call_args,
                });

                result_local
            }
            // 闁稿繑婀圭划顒勫嫉椤忓嫮鏉介柣婊勫濞?HIR 閻炴稏鍔忛幓顏勵嚕韫囨洝顫﹂柛銊ヮ儜缁辨繃娼婚弬鎸庣闁告濮崇紞鍛存晸?
            HIRExpr::Await(inner) => {
                let future_handle = self.lower_expr(inner);
                let func_name = self.resolve_async_base_name(future_handle);
                if func_name == "unknown" {
                    self.errors.push(
                        "unable to resolve async future origin during MIR lowering".to_string(),
                    );
                    return self.add_local(None, LocalKind::Temp, MIR_UNIT);
                }
                let result_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                let poll_result = self.add_local(None, LocalKind::Temp, MIR_I64);
                let ready_block = self.new_block();
                let pending_block = self.new_block();

                self.set_terminator(Terminator::Suspend {
                    poll_func: format!("{}__poll", func_name),
                    future_handle,
                    destination: poll_result,
                    ready_block,
                    pending_block,
                });

                self.set_current_block(pending_block);
                self.set_terminator(Terminator::Goto(self.current_block()));

                self.set_current_block(ready_block);
                self.push_inst(Instruction::Call {
                    destination: result_local,
                    func: format!("{}__result", func_name),
                    args: vec![future_handle],
                });
                result_local
            }
            HIRExpr::AsyncBlock(_) => {
                self.errors
                    .push("async blocks are not supported in MIR lowering".to_string());
                self.add_local(None, LocalKind::Temp, MIR_UNIT)
            }
            _ => self.add_local(None, LocalKind::Temp, MIR_UNIT),
        }
    }

    fn infer_poll_func_from_last_call(&self) -> String {
        let block = &self.mir_fn.basic_blocks[self.current_block()];
        for inst_id in block.instructions.iter().rev() {
            if let Instruction::Call { func, .. } = self.mir_fn.instruction(*inst_id) {
                if func.ends_with("__start") {
                    return func.trim_end_matches("__start").to_string();
                }
            }
        }
        "unknown".to_string()
    }

    /// Resolve the async function base name for a given future handle local.
    ///
    /// Resolution order:
    ///  1. Direct lookup in `future_origins` — covers `await async_fn(args)`.
    ///  2. If the handle came from a `Load { destination: handle, source: src }`,
    ///     look up `src` in `future_origins` — covers `let f = async_fn(); await f`.
    ///  3. Fall back to backward-scan heuristic via `infer_poll_func_from_last_call`.
    fn resolve_async_base_name(&self, handle: Local) -> String {
        // 1. Direct hit
        if let Some(name) = self.future_origins.get(&handle) {
            return name.clone();
        }
        // 2. Trace through a Load in the current block (let-binding case)
        let block = &self.mir_fn.basic_blocks[self.current_block()];
        for inst_id in block.instructions.iter().rev() {
            if let Instruction::Load { destination, source } = self.mir_fn.instruction(*inst_id) {
                if *destination == handle {
                    if let Some(name) = self.future_origins.get(source) {
                        return name.clone();
                    }
                }
            }
        }
        // 3. Fallback: scan for the last __start call
        self.infer_poll_func_from_last_call()
    }

    /// 濞寸姴瀛╄啯鐎殿喖绻嬮懙鎴﹀箵閹邦剙绲块柛鎺嬪€曢崺鍡涙晸?
    /// 閻庣敻鈧稓鑹鹃悗娑欘殜濞间即鏌岃箛鏂堜礁顕ｈ箛姘辩闁?Some(value)闁挎稑鑻崣鐐閺嶎剛绠查柨?None
    fn extract_discriminant_from_pattern(&self, pat: &crate::hir::HIRPattern) -> Option<u32> {
        use crate::hir::HIRPattern;
        match pat {
            HIRPattern::Lit(lit) => match lit {
                HIRLiteral::Int(n) if *n >= 0 && *n < u32::MAX as i64 => Some(*n as u32),
                _ => None,
            },
            HIRPattern::Wild => None,
            HIRPattern::Var { .. } => None,
            _ => None,
        }
    }

    /// 婵☆偀鍋撻柡灞诲劚閳ь剙鍚嬪Σ鎼佸触閿曗偓鐏忣噣鏌婂鍠ｄ線鏁?
    /// 閺夆晜鏌ㄥú鏍ㄧ▔閳ь剚绋夐鍕樁闁告凹鍋勭粩椋庝焊閺冨倻娉㈤柡瀣矌濞?Local
    fn matches_pattern(&mut self, pat: &crate::hir::HIRPattern, value: Local) -> Local {
        use crate::hir::HIRPattern;
        let result = self.add_local(None, LocalKind::Temp, MIR_BOOL);

        match pat {
            HIRPattern::Wild => {
                // 闂侇偅宀搁崢銈囩箔閿旇В鍋撶紒妯恍﹂柛鏍х秺閸?
                self.push_inst(Instruction::Assign {
                    destination: result,
                    value: MirConstant::Bool(true),
                });
                result
            }
            HIRPattern::Lit(lit) => {
                // 閻庢稒顨婂浼存煂韫囨枅浣割嚕韫囥儳绐楁慨锝嗘缁舵繈鏁?
                let lit_local = self.lower_literal(lit);
                self.push_inst(Instruction::Binary {
                    destination: result,
                    op: MirBinOp::Eq,
                    left: value,
                    right: lit_local,
                });
                result
            }
            HIRPattern::Var { .. } => {
                // 闁告瑦锕㈤崳鍝勎熼垾宕囩闁诡剛绮Σ鎼佸礌瑜版帒甯?
                self.push_inst(Instruction::Assign {
                    destination: result,
                    value: MirConstant::Bool(true),
                });
                result
            }
            _ => {
                // 闁稿繑婀圭划顒€螣閳ュ磭纭€闁哄棗鍊风粭澶屸偓鍦仧楠?
                self.push_inst(Instruction::Assign {
                    destination: result,
                    value: MirConstant::Bool(true),
                });
                result
            }
        }
    }

    /// 闂傚嫬绉崇紞鍡椢熼垾宕囩缂備焦鍨甸悾?
    /// 濠碘€冲€归悘澶娢熼垾宕囩闁告牕鎳庨幆鍫ュ矗濮椻偓閸ｈ櫣绱掗幋婵堟毎闁挎稑濂旂划鐘诲几濮橆偄顩☉鎿冨幗瑜颁線宕ｉ弽顒佺グ闁艰棄鍢查懟鐔虹磼閹存繄鏆?
    fn lower_pattern_bindings(&mut self, pat: &crate::hir::HIRPattern, enum_value: Local) {
        use crate::hir::HIRPattern;
        match pat {
            HIRPattern::Var { name, .. } => {
                // 缂佺姭鍋撻柛妤佹礀瑜板鏌岃箛鏇犳嫧閻庤鐔槐浼村极缂堢娀鍤嬮柡瀣煯婵″洭宕愰懖鈺冩嫧閻庤鑹鹃崺宀勫矗濮椻偓閸?
                let _ = self.add_local(Some(name.clone()), LocalKind::User, MIR_I64);
            }
            HIRPattern::Tuple(patterns) => {
                if !patterns.is_empty() {
                    let payload_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                    self.push_inst(Instruction::ExtractPayload {
                        destination: payload_local,
                        source: enum_value,
                    });
                    for (index, sub_pat) in patterns.iter().enumerate() {
                        if let HIRPattern::Var { name, .. } = sub_pat {
                            let field_local = self.add_local(None, LocalKind::Temp, MIR_I64);
                            self.push_inst(Instruction::Extract {
                                destination: field_local,
                                value: payload_local,
                                index: index as u32,
                            });
                            let bound_local =
                                self.add_local(Some(name.clone()), LocalKind::User, MIR_I64);
                            self.push_inst(Instruction::Store {
                                destination: bound_local,
                                value: field_local,
                            });
                        }
                    }
                }
            }
            _ => {
                // 闁稿繑婀圭划顒€螣閳ュ磭纭€闁哄棗鍊风粭澶嬪緞閸曨厽鍊?
            }
        }
    }

    /// 闂傚嫬绉崇紞鍡欌偓娑欘殜濞间即鏁?
    fn lower_literal(&mut self, lit: &HIRLiteral) -> Local {
        let constant = match lit {
            HIRLiteral::Int(n) => MirConstant::Int(*n),
            HIRLiteral::Float(f) => MirConstant::Float(*f),
            HIRLiteral::String(s) => MirConstant::String(s.clone()),
            HIRLiteral::Bool(b) => MirConstant::Bool(*b),
            HIRLiteral::Char(c) => MirConstant::Char(*c),
            HIRLiteral::Null => MirConstant::Unit,
            HIRLiteral::Bytes(b) => MirConstant::Bytes(b.clone()),
            HIRLiteral::Uint(u) => MirConstant::Uint(*u),
        };
        let ty = constant.ty();
        let local = self.add_local(None, LocalKind::Temp, ty);
        self.push_inst(Instruction::Assign {
            destination: local,
            value: constant,
        });
        local
    }

    /// 闂傚嫬绉崇紞鍡樼▔閳ь剟宕楅崘鈺傛儥濞达絾绮庨?
    fn lower_un_op(&self, op: &hir::HIRUnaryOp) -> MirUnOp {
        match op {
            hir::HIRUnaryOp::Neg => MirUnOp::Neg,
            hir::HIRUnaryOp::Not => MirUnOp::Not,
            hir::HIRUnaryOp::BitNot => MirUnOp::BitNot,
            hir::HIRUnaryOp::Ref | hir::HIRUnaryOp::RefMut | hir::HIRUnaryOp::Deref => MirUnOp::Neg,
        }
    }

    /// 闂傚嫬绉崇紞鍡樼鐏炶棄甯楅柟鍨С缂嶆棃鏁?
    fn lower_bin_op(&self, op: &hir::HIRBinaryOp) -> MirBinOp {
        match op {
            hir::HIRBinaryOp::Add => MirBinOp::Add,
            hir::HIRBinaryOp::Sub => MirBinOp::Sub,
            hir::HIRBinaryOp::Mul => MirBinOp::Mul,
            hir::HIRBinaryOp::Div => MirBinOp::Div,
            hir::HIRBinaryOp::Mod => MirBinOp::Rem,
            hir::HIRBinaryOp::BitAnd => MirBinOp::BitAnd,
            hir::HIRBinaryOp::BitOr => MirBinOp::BitOr,
            hir::HIRBinaryOp::BitXor => MirBinOp::BitXor,
            hir::HIRBinaryOp::Shl => MirBinOp::Shl,
            hir::HIRBinaryOp::Shr => MirBinOp::Shr,
            hir::HIRBinaryOp::LogAnd => MirBinOp::LogAnd,
            hir::HIRBinaryOp::LogOr => MirBinOp::LogOr,
            hir::HIRBinaryOp::Eq => MirBinOp::Eq,
            hir::HIRBinaryOp::NotEq => MirBinOp::Ne,
            hir::HIRBinaryOp::Lt => MirBinOp::Lt,
            hir::HIRBinaryOp::Gt => MirBinOp::Gt,
            hir::HIRBinaryOp::Le => MirBinOp::Le,
            hir::HIRBinaryOp::Ge => MirBinOp::Ge,
            hir::HIRBinaryOp::Assign => MirBinOp::Add,
        }
    }
}










