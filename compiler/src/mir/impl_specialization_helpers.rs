use crate::hir::{self, HIRType, HIRTypeKind};
use crate::mir::hir_specialization_helpers::{
    hir_type_is_concrete, hir_type_is_placeholder_name, substitute_hir_function,
};
use crate::type_naming::{hir_type_instance_name, hir_type_prefix};
use std::collections::{HashMap, HashSet};

pub(crate) fn match_generic_impl_target(
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
                    && lhs
                        .iter()
                        .zip(rhs.iter())
                        .all(|(lhs, rhs)| match_generic_impl_target(lhs, rhs, known_named_types, subst))
            }
            (
                HIRTypeKind::Fn {
                    params: lhs_params,
                    ret: lhs_ret,
                },
                HIRTypeKind::Fn {
                    params: rhs_params,
                    ret: rhs_ret,
                },
            ) => {
                lhs_params.len() == rhs_params.len()
                    && lhs_params
                        .iter()
                        .zip(rhs_params.iter())
                        .all(|(lhs, rhs)| match_generic_impl_target(lhs, rhs, known_named_types, subst))
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
                    && lhs_args
                        .iter()
                        .zip(rhs_args.iter())
                        .all(|(lhs, rhs)| match_generic_impl_target(lhs, rhs, known_named_types, subst))
            }
            _ => false,
        }
    }
}

pub(crate) fn impl_type_prefix(ty: &HIRType) -> String {
    match &ty.kind {
        HIRTypeKind::Named { args, .. } if !args.is_empty() => hir_type_instance_name(ty),
        _ => hir_type_prefix(ty),
    }
}

pub(crate) fn instantiate_impl_method(
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

pub(crate) fn expand_impl_variants(
    impl_item: &hir::HIRImpl,
    concrete_named_types: &HashMap<String, HIRType>,
    known_named_types: &HashSet<String>,
) -> Vec<hir::HIRImpl> {
    let legacy_prefix = hir_type_prefix(&impl_item.target_type);
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
