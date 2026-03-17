use crate::hir::{self, HIRType};
use crate::method_resolution::{explicit_hir_method_param_count, MethodCandidate};
use crate::method_resolution::explicit_hir_method_params;
use crate::mir::hir_specialization_helpers::substitute_hir_function;
use crate::mir::hir_specialization_helpers::substitute_hir_type;
use crate::mir::impl_specialization_helpers::impl_type_prefix;
use crate::mir::method_dispatch_helpers::receiver_type_prefix;
use crate::mir::type_mapping_helpers::bind_mir_subst_from_hir_type;
use crate::mir::{ConcreteTypeRegistry, MIRType, TraitMethodTemplate};
use crate::type_naming::hir_type_instance_name;
use crate::hir::HIRTypeKind;
use std::collections::HashMap;

pub(crate) fn bind_method_specialization_subst(
    target_type: &HIRType,
    method: &hir::HIRFunction,
    receiver_ty: &MIRType,
    actual_arg_types: &[MIRType],
    struct_defs: &HashMap<String, &hir::HIRStruct>,
) -> Option<HashMap<String, MIRType>> {
    let mut mir_subst = HashMap::new();
    bind_mir_subst_from_hir_type(target_type, receiver_ty, struct_defs, &mut mir_subst);

    let explicit_params = explicit_hir_method_params(&method.params);
    if explicit_params.len() != actual_arg_types.len() {
        return None;
    }
    for (param, actual_ty) in explicit_params.iter().zip(actual_arg_types.iter()) {
        bind_mir_subst_from_hir_type(&param.ty, actual_ty, struct_defs, &mut mir_subst);
    }

    Some(mir_subst)
}

pub(crate) fn realize_method_specialization(
    target_type: &HIRType,
    method: &hir::HIRFunction,
    receiver_ty: &MIRType,
    mir_subst: HashMap<String, MIRType>,
    concrete_type_registry: &mut ConcreteTypeRegistry,
) -> Option<(HashMap<String, HIRType>, String)> {
    let receiver_prefix = receiver_type_prefix(receiver_ty);
    let mut hir_subst = HashMap::new();
    for (name, mir_ty) in &mir_subst {
        hir_subst.insert(name.clone(), concrete_type_registry.hir_type_for_mir(mir_ty)?);
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
    concrete_type_registry.register_instance(concrete_prefix.clone(), concrete_target.clone());
    for ty in hir_subst.values() {
        if matches!(ty.kind, HIRTypeKind::Named { .. }) {
            concrete_type_registry.register_instance(hir_type_instance_name(ty), ty.clone());
        }
    }
    if concrete_prefix != receiver_prefix {
        return None;
    }

    Some((hir_subst, concrete_prefix))
}

pub(crate) fn prepare_method_specialization(
    target_type: &HIRType,
    method: &hir::HIRFunction,
    receiver_ty: &MIRType,
    actual_arg_types: &[MIRType],
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    concrete_type_registry: &mut ConcreteTypeRegistry,
) -> Option<(HashMap<String, HIRType>, String)> {
    let mir_subst = bind_method_specialization_subst(
        target_type,
        method,
        receiver_ty,
        actual_arg_types,
        struct_defs,
    )?;
    realize_method_specialization(
        target_type,
        method,
        receiver_ty,
        mir_subst,
        concrete_type_registry,
    )
}

pub(crate) fn build_trait_method_candidate(
    template: &TraitMethodTemplate,
    hir_subst: &HashMap<String, HIRType>,
    concrete_prefix: &str,
) -> MethodCandidate<hir::HIRFunction> {
    let mut specialized = substitute_hir_function(&template.method, hir_subst);
    specialized.type_params.clear();
    if !template.method.type_params.is_empty() {
        let suffixes: Vec<String> = template
            .method
            .type_params
            .iter()
            .filter_map(|param| hir_subst.get(&param.name))
            .map(hir_type_instance_name)
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
            concrete_prefix, template.trait_name, template.method.name,
        );
    }

    MethodCandidate {
        label: format!("{} ({})", specialized.name, template.trait_name),
        param_count: explicit_hir_method_param_count(&specialized),
        value: specialized,
    }
}
