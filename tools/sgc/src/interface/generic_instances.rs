use crate::{GenericInstanceFingerprint, GenericItemFingerprint};
use sengoo_compiler::{DeclKind, Expr, ExprKind, Parser, Program, Stmt, StmtKind};
use std::collections::{HashMap, HashSet};

use super::function_fingerprints::call_target_signature;
use super::generic_items::{
    collect_generic_item_fingerprints_from_decl, collect_impl_method_templates_from_decl,
    GenericCallableMeta, GenericMethodTemplate,
};
use super::signature::{ast_path_signature, type_signature};

fn split_top_level_type_args(args: &str) -> Vec<String> {
    let mut parts = Vec::new();
    let mut depth_angle = 0usize;
    let mut depth_paren = 0usize;
    let mut depth_bracket = 0usize;
    let mut start = 0usize;

    for (idx, ch) in args.char_indices() {
        match ch {
            '<' => depth_angle += 1,
            '>' => depth_angle = depth_angle.saturating_sub(1),
            '(' => depth_paren += 1,
            ')' => depth_paren = depth_paren.saturating_sub(1),
            '[' => depth_bracket += 1,
            ']' => depth_bracket = depth_bracket.saturating_sub(1),
            ',' if depth_angle == 0 && depth_paren == 0 && depth_bracket == 0 => {
                parts.push(args[start..idx].trim().to_string());
                start = idx + ch.len_utf8();
            }
            _ => {}
        }
    }

    if start < args.len() {
        parts.push(args[start..].trim().to_string());
    }

    parts.into_iter().filter(|part| !part.is_empty()).collect()
}

fn parse_named_type_signature(sig: &str) -> Option<(String, Vec<String>)> {
    let trimmed = sig.trim();
    let start = trimmed.find('<')?;
    if !trimmed.ends_with('>') {
        return None;
    }
    let head = trimmed[..start].trim().to_string();
    let inner = &trimmed[start + 1..trimmed.len() - 1];
    Some((head, split_top_level_type_args(inner)))
}

fn extract_impl_receiver_template(symbol: &str) -> Option<String> {
    let marker = "::impl::";
    let start = symbol.find(marker)? + marker.len();
    let tail = &symbol[start..];
    let method_sep = tail.rfind("::")?;
    Some(tail[..method_sep].to_string())
}

fn infer_expr_type_signature(expr: &Expr, local_types: &HashMap<String, String>) -> String {
    match &expr.kind {
        ExprKind::Literal(lit) => match lit {
            sengoo_compiler::Literal::Int(_) => "i64".to_string(),
            sengoo_compiler::Literal::Float(_) => "f64".to_string(),
            sengoo_compiler::Literal::Bool(_) => "bool".to_string(),
            sengoo_compiler::Literal::String(_) => "str".to_string(),
            sengoo_compiler::Literal::Char(_) => "char".to_string(),
            sengoo_compiler::Literal::Bytes(_) => "bytes".to_string(),
            sengoo_compiler::Literal::Null => "null".to_string(),
            sengoo_compiler::Literal::Unit => "unit".to_string(),
        },
        ExprKind::Array(items) => {
            if let Some(first) = items.first() {
                format!("array<{}>", infer_expr_type_signature(first, local_types))
            } else {
                "array<_>".to_string()
            }
        }
        ExprKind::Tuple(items) => format!("tuple{}", items.len()),
        ExprKind::Struct { path, .. } => format!("struct:{}", ast_path_signature(path)),
        ExprKind::Path(path) => path
            .as_simple()
            .and_then(|ident| local_types.get(&ident.name))
            .cloned()
            .unwrap_or_else(|| format!("path:{}", ast_path_signature(path))),
        ExprKind::Ident(ident) => local_types
            .get(&ident.name)
            .cloned()
            .unwrap_or_else(|| "_".to_string()),
        ExprKind::Paren(inner) => infer_expr_type_signature(inner, local_types),
        _ => "_".to_string(),
    }
}

fn substitute_type_signature(template: &str, subst: &HashMap<String, String>) -> String {
    if let Some(replacement) = subst.get(template.trim()) {
        return replacement.clone();
    }

    let Some((head, args)) = parse_named_type_signature(template) else {
        return template.to_string();
    };

    let resolved_args = args
        .iter()
        .map(|arg| substitute_type_signature(arg, subst))
        .collect::<Vec<_>>()
        .join(",");
    format!("{head}<{resolved_args}>")
}

fn unify_type_signature_template(
    template: &str,
    actual: &str,
    type_param_names: &[String],
    subst: &mut HashMap<String, String>,
) -> bool {
    let template = template.trim();
    let actual = actual.trim();

    if type_param_names.iter().any(|name| name == template) {
        match subst.get(template) {
            Some(existing) => existing == actual,
            None => {
                subst.insert(template.to_string(), actual.to_string());
                true
            }
        }
    } else if let (Some((template_head, template_args)), Some((actual_head, actual_args))) = (
        parse_named_type_signature(template),
        parse_named_type_signature(actual),
    ) {
        template_head == actual_head
            && template_args.len() == actual_args.len()
            && template_args
                .iter()
                .zip(actual_args.iter())
                .all(|(template_arg, actual_arg)| {
                    unify_type_signature_template(template_arg, actual_arg, type_param_names, subst)
                })
    } else {
        template == actual
    }
}

fn type_param_substitution(
    meta: &GenericCallableMeta,
    canonical_type_args: &[String],
) -> HashMap<String, String> {
    meta.type_param_names
        .iter()
        .cloned()
        .zip(canonical_type_args.iter().cloned())
        .collect()
}

fn infer_expr_type_signature_with_methods(
    expr: &Expr,
    local_types: &HashMap<String, String>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) -> String {
    match &expr.kind {
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            let Some((symbol, canonical_type_args)) = generic_method_call_instance_parts(
                receiver,
                &method.name,
                args,
                local_types,
                method_to_symbols,
                callable_meta,
            ) else {
                return "_".to_string();
            };
            let Some(meta) = callable_meta.get(&symbol) else {
                return "_".to_string();
            };
            let Some(return_type_template) = meta.return_type_template.as_deref() else {
                return "_".to_string();
            };
            let subst = type_param_substitution(meta, &canonical_type_args);
            substitute_type_signature(return_type_template, &subst)
        }
        _ => infer_expr_type_signature(expr, local_types),
    }
}

fn generic_instance_base_key(item_stable_id: &str, canonical_type_args: &[String]) -> String {
    if canonical_type_args.is_empty() {
        return format!("{}<>", item_stable_id);
    }
    format!("{}<{}>", item_stable_id, canonical_type_args.join(","))
}

fn resolve_generic_call_symbol(
    call_name: &str,
    simple_to_symbol: &HashMap<String, Option<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) -> Option<String> {
    if callable_meta.contains_key(call_name) {
        return Some(call_name.to_string());
    }
    match simple_to_symbol.get(call_name) {
        Some(Some(symbol)) => Some(symbol.clone()),
        _ => None,
    }
}

#[allow(clippy::too_many_arguments)]
fn push_instance_if_generic_call(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    call_name: &str,
    args: &[Expr],
    local_types: &HashMap<String, String>,
    simple_to_symbol: &HashMap<String, Option<String>>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) {
    let Some(symbol) = resolve_generic_call_symbol(call_name, simple_to_symbol, callable_meta)
    else {
        return;
    };
    let Some(meta) = callable_meta.get(&symbol) else {
        return;
    };
    let _ = &meta.module_id;

    let mut canonical_type_args = args
        .iter()
        .map(|arg| {
            infer_expr_type_signature_with_methods(
                arg,
                local_types,
                method_to_symbols,
                callable_meta,
            )
        })
        .take(meta.type_param_count)
        .collect::<Vec<_>>();
    while canonical_type_args.len() < meta.type_param_count {
        canonical_type_args.push("_".to_string());
    }
    let instance_key = generic_instance_base_key(&meta.stable_item_id, &canonical_type_args);
    if !seen.insert(instance_key.clone()) {
        return;
    }

    out.push(GenericInstanceFingerprint {
        item_stable_id: meta.stable_item_id.clone(),
        module_id: module_path.to_string(),
        canonical_type_args,
        instance_key,
        interface_hash: meta.interface_hash,
        body_hash: meta.body_hash,
    });
}

fn generic_method_call_instance_parts(
    receiver: &Expr,
    method_name: &str,
    args: &[Expr],
    local_types: &HashMap<String, String>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) -> Option<(String, Vec<String>)> {
    let receiver_sig = infer_expr_type_signature_with_methods(
        receiver,
        local_types,
        method_to_symbols,
        callable_meta,
    );
    let candidate_symbols = method_to_symbols.get(method_name)?;

    for symbol in candidate_symbols {
        let Some(meta) = callable_meta.get(symbol) else {
            continue;
        };
        let Some(template) = meta.receiver_type_template.as_deref() else {
            continue;
        };
        let mut subst = HashMap::new();
        if !unify_type_signature_template(
            template,
            &receiver_sig,
            &meta.type_param_names,
            &mut subst,
        ) {
            continue;
        }

        if meta.param_type_templates.len() != args.len() {
            continue;
        }

        let mut param_mismatch = false;
        for (template_arg, actual_arg) in meta.param_type_templates.iter().zip(args.iter()) {
            let actual_sig = infer_expr_type_signature_with_methods(
                actual_arg,
                local_types,
                method_to_symbols,
                callable_meta,
            );
            if !unify_type_signature_template(
                template_arg,
                &actual_sig,
                &meta.type_param_names,
                &mut subst,
            ) {
                param_mismatch = true;
                break;
            }
        }
        if param_mismatch {
            continue;
        }

        let canonical_type_args = meta
            .type_param_names
            .iter()
            .map(|param| subst.get(param).cloned().unwrap_or_else(|| "_".to_string()))
            .take(meta.type_param_count)
            .collect::<Vec<_>>();

        return Some((symbol.clone(), canonical_type_args));
    }

    None
}

#[allow(clippy::too_many_arguments)]
fn push_instance_if_generic_method_call(
    out: &mut Vec<GenericInstanceFingerprint>,
    seen: &mut HashSet<String>,
    module_path: &str,
    receiver: &Expr,
    method_name: &str,
    args: &[Expr],
    local_types: &HashMap<String, String>,
    method_to_symbols: &HashMap<String, Vec<String>>,
    callable_meta: &HashMap<String, GenericCallableMeta>,
) {
    let Some((symbol, canonical_type_args)) = generic_method_call_instance_parts(
        receiver,
        method_name,
        args,
        local_types,
        method_to_symbols,
        callable_meta,
    ) else {
        return;
    };
    let Some(meta) = callable_meta.get(&symbol) else {
        return;
    };
    let instance_key = generic_instance_base_key(&meta.stable_item_id, &canonical_type_args);
    if !seen.insert(instance_key.clone()) {
        return;
    }

    out.push(GenericInstanceFingerprint {
        item_stable_id: meta.stable_item_id.clone(),
        module_id: module_path.to_string(),
        canonical_type_args,
        instance_key,
        interface_hash: meta.interface_hash,
        body_hash: meta.body_hash,
    });
}

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
fn collect_generic_instances_in_expr(
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
        | ExprKind::ParallelBlock(block) => {
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
fn collect_generic_instances_in_stmt(
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
        StmtKind::Let { name, ty, value } => {
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

pub(crate) fn generic_fingerprints_for_module(
    module_path: &str,
    source: &str,
) -> (Vec<GenericItemFingerprint>, Vec<GenericInstanceFingerprint>) {
    let program = match Parser::parse(source) {
        Ok(program) => program,
        Err(_) => return (Vec::new(), Vec::new()),
    };
    generic_fingerprints_for_program(module_path, source, &program)
}

pub(crate) fn generic_fingerprints_for_program(
    module_path: &str,
    source: &str,
    program: &Program,
) -> (Vec<GenericItemFingerprint>, Vec<GenericInstanceFingerprint>) {
    let mut items = Vec::new();
    for decl in &program.decls {
        collect_generic_item_fingerprints_from_decl(&mut items, module_path, &[], decl, source, 0);
    }
    items.sort_by(|a, b| a.stable_item_id.cmp(&b.stable_item_id));
    items.dedup_by(|a, b| a.stable_item_id == b.stable_item_id);

    let mut method_templates = HashMap::<String, GenericMethodTemplate>::new();
    for decl in &program.decls {
        collect_impl_method_templates_from_decl(&mut method_templates, module_path, &[], decl);
    }

    let callable_meta = items
        .iter()
        .filter_map(|item| {
            if item.kind != "function" && item.kind != "impl_method" {
                return None;
            }
            let method_template = method_templates.get(&item.symbol);
            Some((
                item.symbol.clone(),
                GenericCallableMeta {
                    stable_item_id: item.stable_item_id.clone(),
                    module_id: item.module_id.clone(),
                    interface_hash: item.interface_hash,
                    body_hash: item.body_hash,
                    type_param_count: item.type_param_count as usize,
                    type_param_names: method_template
                        .map(|template| template.type_param_names.clone())
                        .unwrap_or_default(),
                    receiver_type_template: if item.kind == "impl_method" {
                        method_template
                            .map(|template| template.receiver_type_template.clone())
                            .or_else(|| extract_impl_receiver_template(&item.symbol))
                    } else {
                        None
                    },
                    param_type_templates: method_template
                        .map(|template| template.param_type_templates.clone())
                        .unwrap_or_default(),
                    return_type_template: method_template
                        .and_then(|template| template.return_type_template.clone()),
                },
            ))
        })
        .collect::<HashMap<_, _>>();

    let mut simple_to_symbol = HashMap::<String, Option<String>>::new();
    let mut method_to_symbols = HashMap::<String, Vec<String>>::new();
    for (symbol, meta) in &callable_meta {
        let simple = symbol.rsplit("::").next().unwrap_or_default().to_string();
        if meta.receiver_type_template.is_some() {
            method_to_symbols
                .entry(simple)
                .or_default()
                .push(symbol.clone());
        } else {
            match simple_to_symbol.get_mut(&simple) {
                Some(entry) => *entry = None,
                None => {
                    simple_to_symbol.insert(simple, Some(symbol.clone()));
                }
            }
        }
    }

    let mut instances = Vec::<GenericInstanceFingerprint>::new();
    let mut seen_instances = HashSet::<String>::new();
    for decl in &program.decls {
        match &decl.kind {
            DeclKind::Function(function) => {
                let mut local_types = function
                    .params
                    .iter()
                    .map(|param| (param.name.name.clone(), type_signature(&param.ty)))
                    .collect::<HashMap<_, _>>();
                for stmt in &function.body.stmts {
                    collect_generic_instances_in_stmt(
                        &mut instances,
                        &mut seen_instances,
                        module_path,
                        stmt,
                        &mut local_types,
                        &simple_to_symbol,
                        &method_to_symbols,
                        &callable_meta,
                    );
                }
            }
            DeclKind::Const(const_decl) => {
                let local_types = HashMap::new();
                collect_generic_instances_in_expr(
                    &mut instances,
                    &mut seen_instances,
                    module_path,
                    &const_decl.value,
                    &local_types,
                    &simple_to_symbol,
                    &method_to_symbols,
                    &callable_meta,
                );
            }
            DeclKind::Static(static_decl) => {
                let local_types = HashMap::new();
                collect_generic_instances_in_expr(
                    &mut instances,
                    &mut seen_instances,
                    module_path,
                    &static_decl.value,
                    &local_types,
                    &simple_to_symbol,
                    &method_to_symbols,
                    &callable_meta,
                );
            }
            _ => {}
        }
    }
    instances.sort_by(|a, b| a.instance_key.cmp(&b.instance_key));
    instances.dedup_by(|a, b| a.instance_key == b.instance_key);
    (items, instances)
}
