use crate::{GenericInstanceFingerprint, GenericItemFingerprint};
use sengoo_compiler::{DeclKind, Expr, ExprKind, Parser, Program};
use std::collections::{HashMap, HashSet};

mod collector;
mod type_signatures;

use self::collector::{collect_generic_instances_in_expr, collect_generic_instances_in_stmt};
use self::type_signatures::{
    extract_impl_receiver_template, infer_expr_type_signature, substitute_type_signature,
    type_param_substitution, unify_type_signature_template,
};
use super::generic_items::{
    collect_generic_item_fingerprints_from_decl, collect_impl_method_templates_from_decl,
    GenericCallableMeta, GenericMethodTemplate,
};
use super::signature::type_signature;

pub(super) fn infer_expr_type_signature_with_methods(
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
pub(super) fn push_instance_if_generic_call(
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
pub(super) fn push_instance_if_generic_method_call(
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
