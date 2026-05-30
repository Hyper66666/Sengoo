use crate::{implementation_fingerprint, source_fingerprint, GenericItemFingerprint};
use sengoo_compiler::{ClassMember, Decl, DeclKind, TraitItem, TypeParam};
use std::collections::HashMap;

use super::function_fingerprints::collect_calls_in_stmt;
use super::signature::{function_signature, trait_bound_signature, type_signature};
use super::{function_symbol, source_span_slice};

#[derive(Debug, Clone)]
pub(super) struct GenericCallableMeta {
    pub(super) stable_item_id: String,
    pub(super) module_id: String,
    pub(super) interface_hash: u64,
    pub(super) body_hash: u64,
    pub(super) type_param_count: usize,
    pub(super) type_param_names: Vec<String>,
    pub(super) receiver_type_template: Option<String>,
    pub(super) param_type_templates: Vec<String>,
    pub(super) return_type_template: Option<String>,
}

#[derive(Debug, Clone)]
pub(super) struct GenericMethodTemplate {
    pub(super) receiver_type_template: String,
    pub(super) param_type_templates: Vec<String>,
    pub(super) return_type_template: Option<String>,
    pub(super) type_param_names: Vec<String>,
}

fn generic_item_id(module_path: &str, scope: &[String], kind: &str, name: &str) -> String {
    let mut parts = Vec::with_capacity(scope.len() + 3);
    parts.push(module_path.to_string());
    parts.extend(scope.iter().cloned());
    parts.push(kind.to_string());
    parts.push(name.to_string());
    parts.join("::")
}

fn generic_type_param_signature(type_params: &[TypeParam]) -> String {
    type_params
        .iter()
        .map(|tp| {
            let mut repr = tp.name.name.clone();
            if !tp.bounds.is_empty() {
                repr.push(':');
                repr.push_str(
                    &tp.bounds
                        .iter()
                        .map(trait_bound_signature)
                        .collect::<Vec<_>>()
                        .join("+"),
                );
            }
            if let Some(default) = &tp.default {
                repr.push('=');
                repr.push_str(&type_signature(default));
            }
            repr
        })
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn collect_impl_method_templates_from_decl(
    out: &mut HashMap<String, GenericMethodTemplate>,
    module_path: &str,
    scope: &[String],
    decl: &Decl,
) {
    match &decl.kind {
        DeclKind::Impl(impl_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("impl".to_string());
            scoped.push(type_signature(&impl_decl.target_type));

            for function in &impl_decl.items {
                let effective_type_params =
                    impl_decl.type_params.len() + function.type_params.len();
                if effective_type_params == 0 {
                    continue;
                }
                let symbol = function_symbol(module_path, &scoped, &function.name.name);
                let type_param_names = impl_decl
                    .type_params
                    .iter()
                    .chain(function.type_params.iter())
                    .map(|param| param.name.name.clone())
                    .collect::<Vec<_>>();

                out.insert(
                    symbol,
                    GenericMethodTemplate {
                        receiver_type_template: type_signature(&impl_decl.target_type),
                        param_type_templates: function
                            .params
                            .iter()
                            .map(|param| type_signature(&param.ty))
                            .collect(),
                        return_type_template: function.return_type.as_ref().map(type_signature),
                        type_param_names,
                    },
                );
            }
        }
        DeclKind::Module(module_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("mod".to_string());
            scoped.push(module_decl.name.name.clone());
            for item in &module_decl.items {
                collect_impl_method_templates_from_decl(out, module_path, &scoped, item);
            }
        }
        _ => {}
    }
}

#[allow(clippy::too_many_arguments)]
fn push_generic_item(
    out: &mut Vec<GenericItemFingerprint>,
    kind: &str,
    stable_item_id: String,
    symbol: String,
    module_id: &str,
    interface_hash: u64,
    body_hash: u64,
    type_param_count: usize,
    calls: Vec<String>,
) {
    out.push(GenericItemFingerprint {
        stable_item_id,
        symbol,
        module_id: module_id.to_string(),
        kind: kind.to_string(),
        interface_hash,
        body_hash,
        type_param_count: type_param_count as u32,
        calls,
    });
}

pub(super) fn collect_generic_item_fingerprints_from_decl(
    out: &mut Vec<GenericItemFingerprint>,
    module_path: &str,
    scope: &[String],
    decl: &Decl,
    source: &str,
    inherited_generic_params: usize,
) {
    match &decl.kind {
        DeclKind::Function(function) => {
            let effective_type_params = inherited_generic_params + function.type_params.len();
            if effective_type_params > 0 {
                let mut calls = Vec::new();
                for stmt in &function.body.stmts {
                    collect_calls_in_stmt(stmt, &mut calls);
                }
                calls.sort();
                calls.dedup();
                let symbol = function_symbol(module_path, scope, &function.name.name);
                let stable_item_id = symbol.clone();
                let interface_hash = source_fingerprint(&function_signature(function));
                let body_hash = source_span_slice(source, function.body.span)
                    .map(implementation_fingerprint)
                    .unwrap_or_else(|| source_fingerprint(&format!("{:?}", function.body.stmts)));
                push_generic_item(
                    out,
                    "function",
                    stable_item_id,
                    symbol,
                    module_path,
                    interface_hash,
                    body_hash,
                    effective_type_params,
                    calls,
                );
            }
        }
        DeclKind::Struct(struct_decl) => {
            if !struct_decl.type_params.is_empty() {
                let stable_item_id =
                    generic_item_id(module_path, scope, "struct", &struct_decl.name.name);
                let interface_hash = source_fingerprint(&format!(
                    "struct:{}<{}>",
                    struct_decl.name.name,
                    generic_type_param_signature(&struct_decl.type_params)
                ));
                let body_hash = source_span_slice(source, struct_decl.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "struct",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    struct_decl.type_params.len(),
                    Vec::new(),
                );
            }
        }
        DeclKind::Enum(enum_decl) => {
            if !enum_decl.type_params.is_empty() {
                let stable_item_id =
                    generic_item_id(module_path, scope, "enum", &enum_decl.name.name);
                let interface_hash = source_fingerprint(&format!(
                    "enum:{}<{}>",
                    enum_decl.name.name,
                    generic_type_param_signature(&enum_decl.type_params)
                ));
                let body_hash = source_span_slice(source, enum_decl.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "enum",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    enum_decl.type_params.len(),
                    Vec::new(),
                );
            }
        }
        DeclKind::Class(class_decl) => {
            if !class_decl.type_params.is_empty() {
                let stable_item_id =
                    generic_item_id(module_path, scope, "class", &class_decl.name.name);
                let interface_hash = source_fingerprint(&format!(
                    "class:{}<{}>",
                    class_decl.name.name,
                    generic_type_param_signature(&class_decl.type_params)
                ));
                let body_hash = source_span_slice(source, class_decl.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "class",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    class_decl.type_params.len(),
                    Vec::new(),
                );
            }

            let mut scoped = scope.to_vec();
            scoped.push("class".to_string());
            scoped.push(class_decl.name.name.clone());
            for member in &class_decl.members {
                if let ClassMember::Method(function) = member {
                    let effective_type_params =
                        class_decl.type_params.len() + function.type_params.len();
                    if effective_type_params == 0 {
                        continue;
                    }
                    let mut calls = Vec::new();
                    for stmt in &function.body.stmts {
                        collect_calls_in_stmt(stmt, &mut calls);
                    }
                    calls.sort();
                    calls.dedup();
                    let symbol = function_symbol(module_path, &scoped, &function.name.name);
                    let stable_item_id = symbol.clone();
                    let interface_hash = source_fingerprint(&function_signature(function));
                    let body_hash = source_span_slice(source, function.body.span)
                        .map(implementation_fingerprint)
                        .unwrap_or_else(|| {
                            source_fingerprint(&format!("{:?}", function.body.stmts))
                        });
                    push_generic_item(
                        out,
                        "method",
                        stable_item_id,
                        symbol,
                        module_path,
                        interface_hash,
                        body_hash,
                        effective_type_params,
                        calls,
                    );
                }
            }
        }
        DeclKind::Trait(trait_decl) => {
            if !trait_decl.type_params.is_empty() {
                let stable_item_id =
                    generic_item_id(module_path, scope, "trait", &trait_decl.name.name);
                let interface_hash = source_fingerprint(&format!(
                    "trait:{}<{}>",
                    trait_decl.name.name,
                    generic_type_param_signature(&trait_decl.type_params)
                ));
                let body_hash = source_span_slice(source, trait_decl.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "trait",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    trait_decl.type_params.len(),
                    Vec::new(),
                );
            }
            let mut scoped = scope.to_vec();
            scoped.push("trait".to_string());
            scoped.push(trait_decl.name.name.clone());
            for item in &trait_decl.items {
                if let TraitItem::Function(function) = item {
                    let effective_type_params =
                        trait_decl.type_params.len() + function.type_params.len();
                    if effective_type_params == 0 {
                        continue;
                    }
                    let symbol = function_symbol(module_path, &scoped, &function.name.name);
                    let stable_item_id = symbol.clone();
                    let interface_hash = source_fingerprint(&function_signature(function));
                    let body_hash = source_span_slice(source, function.body.span)
                        .map(implementation_fingerprint)
                        .unwrap_or_else(|| {
                            source_fingerprint(&format!("{:?}", function.body.stmts))
                        });
                    push_generic_item(
                        out,
                        "trait_method",
                        stable_item_id,
                        symbol,
                        module_path,
                        interface_hash,
                        body_hash,
                        effective_type_params,
                        Vec::new(),
                    );
                }
            }
        }
        DeclKind::Impl(impl_decl) => {
            if !impl_decl.type_params.is_empty() {
                let stable_item_id = generic_item_id(
                    module_path,
                    scope,
                    "impl",
                    &type_signature(&impl_decl.target_type),
                );
                let interface_hash = source_fingerprint(&format!(
                    "impl:{}<{}>",
                    type_signature(&impl_decl.target_type),
                    generic_type_param_signature(&impl_decl.type_params)
                ));
                let body_hash = source_span_slice(source, impl_decl.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "impl",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    impl_decl.type_params.len(),
                    Vec::new(),
                );
            }
            let mut scoped = scope.to_vec();
            scoped.push("impl".to_string());
            scoped.push(type_signature(&impl_decl.target_type));
            for function in &impl_decl.items {
                let effective_type_params =
                    impl_decl.type_params.len() + function.type_params.len();
                if effective_type_params == 0 {
                    continue;
                }
                let mut calls = Vec::new();
                for stmt in &function.body.stmts {
                    collect_calls_in_stmt(stmt, &mut calls);
                }
                calls.sort();
                calls.dedup();
                let symbol = function_symbol(module_path, &scoped, &function.name.name);
                let stable_item_id = symbol.clone();
                let interface_hash = source_fingerprint(&function_signature(function));
                let body_hash = source_span_slice(source, function.body.span)
                    .map(implementation_fingerprint)
                    .unwrap_or_else(|| source_fingerprint(&format!("{:?}", function.body.stmts)));
                push_generic_item(
                    out,
                    "impl_method",
                    stable_item_id,
                    symbol,
                    module_path,
                    interface_hash,
                    body_hash,
                    effective_type_params,
                    calls,
                );
            }
        }
        DeclKind::TypeAlias(alias) => {
            if !alias.type_params.is_empty() {
                let stable_item_id =
                    generic_item_id(module_path, scope, "type_alias", &alias.name.name);
                let interface_hash = source_fingerprint(&format!(
                    "type_alias:{}<{}>",
                    alias.name.name,
                    generic_type_param_signature(&alias.type_params)
                ));
                let body_hash = source_span_slice(source, alias.span)
                    .map(implementation_fingerprint)
                    .unwrap_or(interface_hash);
                push_generic_item(
                    out,
                    "type_alias",
                    stable_item_id.clone(),
                    stable_item_id,
                    module_path,
                    interface_hash,
                    body_hash,
                    alias.type_params.len(),
                    Vec::new(),
                );
            }
        }
        DeclKind::Module(module_decl) => {
            let mut scoped = scope.to_vec();
            scoped.push("mod".to_string());
            scoped.push(module_decl.name.name.clone());
            for item in &module_decl.items {
                collect_generic_item_fingerprints_from_decl(
                    out,
                    module_path,
                    &scoped,
                    item,
                    source,
                    inherited_generic_params,
                );
            }
        }
        _ => {}
    }
}
