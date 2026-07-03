use crate::hir::HIRTypeKind;
use crate::hir::{self, HIRType};
use crate::method_resolution::explicit_hir_method_params;
use crate::method_resolution::{
    ambiguous_method_error, explicit_hir_method_param_count, select_method_candidate,
    MethodCandidate, MethodCandidateMatch,
};
use crate::mir::hir_specialization_helpers::substitute_hir_function;
use crate::mir::hir_specialization_helpers::substitute_hir_type;
use crate::mir::impl_specialization_helpers::impl_type_prefix;
use crate::mir::method_dispatch_helpers::receiver_type_prefix;
use crate::mir::type_mapping_helpers::bind_mir_subst_from_hir_type;
use crate::mir::{ConcreteTypeRegistry, MIRType, TraitMethodTemplate};
use crate::type_naming::hir_type_instance_name;
use std::collections::HashMap;

pub(crate) fn bind_method_specialization_subst(
    target_type: &HIRType,
    method: &hir::HIRFunction,
    receiver_ty: &MIRType,
    actual_arg_types: &[MIRType],
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    concrete_type_registry: &ConcreteTypeRegistry,
) -> Option<HashMap<String, MIRType>> {
    let mut mir_subst = HashMap::new();
    bind_mir_subst_from_hir_type(target_type, receiver_ty, struct_defs, &mut mir_subst);
    bind_registered_hir_pair(
        target_type,
        receiver_ty,
        struct_defs,
        concrete_type_registry,
        &mut mir_subst,
    );
    bind_single_arg_receiver_name(
        target_type,
        receiver_ty,
        struct_defs,
        concrete_type_registry,
        &mut mir_subst,
    );

    let explicit_params = explicit_hir_method_params(&method.params);
    if explicit_params.len() != actual_arg_types.len() {
        return None;
    }
    for (param, actual_ty) in explicit_params.iter().zip(actual_arg_types.iter()) {
        bind_mir_subst_from_hir_type(&param.ty, actual_ty, struct_defs, &mut mir_subst);
        bind_registered_hir_pair(
            &param.ty,
            actual_ty,
            struct_defs,
            concrete_type_registry,
            &mut mir_subst,
        );
    }

    Some(mir_subst)
}

fn bind_single_arg_receiver_name(
    target_type: &HIRType,
    receiver_ty: &MIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    concrete_type_registry: &ConcreteTypeRegistry,
    subst: &mut HashMap<String, MIRType>,
) {
    let (
        HIRTypeKind::Named {
            name: target_name,
            args,
        },
        MIRType::Struct {
            name: receiver_name,
            ..
        },
    ) = (&target_type.kind, receiver_ty)
    else {
        return;
    };
    let [template_arg] = args.as_slice() else {
        return;
    };
    let HIRTypeKind::Named {
        name: template_name,
        args: template_args,
    } = &template_arg.kind
    else {
        return;
    };
    if !template_args.is_empty() || struct_defs.contains_key(template_name) {
        return;
    }
    let Some(instance_name) = receiver_name.strip_prefix(&format!("{target_name}_")) else {
        return;
    };
    let Some(actual_hir) = concrete_type_registry.hir_type_for_instance_name(instance_name) else {
        return;
    };
    subst.entry(template_name.clone()).or_insert_with(|| {
        crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_subst(
            &actual_hir,
            struct_defs,
            &HashMap::new(),
        )
    });
}

fn bind_registered_hir_pair(
    template: &HIRType,
    actual_mir: &MIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    concrete_type_registry: &ConcreteTypeRegistry,
    subst: &mut HashMap<String, MIRType>,
) {
    let Some(actual_hir) = concrete_type_registry.hir_type_for_mir(actual_mir) else {
        return;
    };
    bind_hir_pair(template, &actual_hir, struct_defs, subst);
}

fn bind_hir_pair(
    template: &HIRType,
    actual: &HIRType,
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    subst: &mut HashMap<String, MIRType>,
) {
    match (&template.kind, &actual.kind) {
        (
            HIRTypeKind::Named {
                name,
                args: template_args,
            },
            _,
        ) if template_args.is_empty() && !struct_defs.contains_key(name) => {
            subst.entry(name.clone()).or_insert_with(|| {
                crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_subst(
                    actual,
                    struct_defs,
                    &HashMap::new(),
                )
            });
        }
        (
            HIRTypeKind::Named {
                name: template_name,
                args: template_args,
            },
            HIRTypeKind::Named {
                name: actual_name,
                args: actual_args,
            },
        ) if template_name == actual_name => {
            for (template_arg, actual_arg) in template_args.iter().zip(actual_args.iter()) {
                bind_hir_pair(template_arg, actual_arg, struct_defs, subst);
            }
        }
        (HIRTypeKind::Ref(_, template_inner), HIRTypeKind::Ref(_, actual_inner))
        | (HIRTypeKind::Ref(_, template_inner), HIRTypeKind::Ptr(actual_inner))
        | (HIRTypeKind::Ptr(template_inner), HIRTypeKind::Ref(_, actual_inner))
        | (HIRTypeKind::Ptr(template_inner), HIRTypeKind::Ptr(actual_inner)) => {
            bind_hir_pair(template_inner, actual_inner, struct_defs, subst);
        }
        _ => {}
    }
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
        hir_subst.insert(
            name.clone(),
            concrete_type_registry.hir_type_for_mir(mir_ty)?,
        );
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
        concrete_type_registry,
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

pub(crate) fn collect_trait_method_candidates(
    templates: &[TraitMethodTemplate],
    method_name: &str,
    receiver_ty: &MIRType,
    actual_arg_types: &[MIRType],
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    concrete_type_registry: &mut ConcreteTypeRegistry,
) -> Vec<MethodCandidate<hir::HIRFunction>> {
    let mut candidates = Vec::new();
    for template in templates {
        if template.method.name != method_name {
            continue;
        }

        let Some((hir_subst, concrete_prefix)) = prepare_method_specialization(
            &template.target_type,
            &template.method,
            receiver_ty,
            actual_arg_types,
            struct_defs,
            concrete_type_registry,
        ) else {
            continue;
        };

        candidates.push(build_trait_method_candidate(
            template,
            &hir_subst,
            &concrete_prefix,
        ));
    }

    candidates
}

pub(crate) fn resolve_trait_method_candidate(
    candidates: Vec<MethodCandidate<hir::HIRFunction>>,
    arg_count: usize,
    method_name: &str,
    type_display: &str,
) -> Result<Option<hir::HIRFunction>, String> {
    match select_method_candidate(candidates, arg_count) {
        MethodCandidateMatch::None | MethodCandidateMatch::WrongArity { .. } => Ok(None),
        MethodCandidateMatch::One(specialized) => Ok(Some(specialized)),
        MethodCandidateMatch::Ambiguous { labels } => {
            Err(ambiguous_method_error(method_name, type_display, &labels))
        }
    }
}

pub(crate) fn resolve_trait_method_specialization(
    templates: &[TraitMethodTemplate],
    method_name: &str,
    receiver_ty: &MIRType,
    actual_arg_types: &[MIRType],
    struct_defs: &HashMap<String, &hir::HIRStruct>,
    concrete_type_registry: &mut ConcreteTypeRegistry,
    type_display: &str,
) -> Result<Option<hir::HIRFunction>, String> {
    let candidates = collect_trait_method_candidates(
        templates,
        method_name,
        receiver_ty,
        actual_arg_types,
        struct_defs,
        concrete_type_registry,
    );

    resolve_trait_method_candidate(
        candidates,
        actual_arg_types.len(),
        method_name,
        type_display,
    )
}
