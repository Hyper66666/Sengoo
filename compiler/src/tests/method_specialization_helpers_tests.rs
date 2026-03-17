use crate::hir::{HIRFunction, HIRParam, HIRStruct, HIRType, HIRTypeParam, IntKind};
use crate::mir::method_specialization_helpers::{
    bind_method_specialization_subst, build_trait_method_candidate,
    prepare_method_specialization, realize_method_specialization,
};
use crate::method_resolution::MethodCandidate;
use crate::mir::{ConcreteTypeRegistry, MIRType, MIR_I64, TraitMethodTemplate};
use crate::symbol::SymbolId;
use std::collections::HashMap;

fn generic_vec_struct() -> HIRStruct {
    HIRStruct {
        name: "Vec".to_string(),
        type_params: vec![HIRTypeParam {
            name: "T".to_string(),
            bounds: vec![],
            default: None,
        }],
        fields: vec![],
        is_pub: false,
    }
}

fn generic_push_method() -> HIRFunction {
    HIRFunction {
        name: "Vec_push".to_string(),
        type_params: vec![HIRTypeParam {
            name: "T".to_string(),
            bounds: vec![],
            default: None,
        }],
        params: vec![
            HIRParam::new(
                "self".to_string(),
                SymbolId::new(1),
                HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]),
            ),
            HIRParam::new(
                "value".to_string(),
                SymbolId::new(2),
                HIRType::named("T".to_string(), vec![]),
            ),
        ],
        return_type: HIRType::int(IntKind::I64),
        precondition: None,
        postcondition: None,
        body: crate::hir::HIRBody::new(),
        is_async: false,
        abi: None,
        is_unsafe: false,
        no_mangle: false,
        export_name: None,
        is_pub: false,
    }
}

fn generic_trait_template() -> TraitMethodTemplate {
    TraitMethodTemplate {
        target_type: HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]),
        trait_name: "Iterable".to_string(),
        method: HIRFunction {
            name: "next".to_string(),
            type_params: vec![HIRTypeParam {
                name: "T".to_string(),
                bounds: vec![],
                default: None,
            }],
            params: vec![HIRParam::new(
                "self".to_string(),
                SymbolId::new(1),
                HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]),
            )],
            return_type: HIRType::named("T".to_string(), vec![]),
            precondition: None,
            postcondition: None,
            body: crate::hir::HIRBody::new(),
            is_async: false,
            abi: None,
            is_unsafe: false,
            no_mangle: false,
            export_name: None,
            is_pub: false,
        },
    }
}

fn nongeneric_trait_template() -> TraitMethodTemplate {
    TraitMethodTemplate {
        target_type: HIRType::named("Vec".to_string(), vec![]),
        trait_name: "Sized".to_string(),
        method: HIRFunction {
            name: "len".to_string(),
            type_params: vec![],
            params: vec![HIRParam::new(
                "self".to_string(),
                SymbolId::new(1),
                HIRType::named("Vec".to_string(), vec![]),
            )],
            return_type: HIRType::int(IntKind::I64),
            precondition: None,
            postcondition: None,
            body: crate::hir::HIRBody::new(),
            is_async: false,
            abi: None,
            is_unsafe: false,
            no_mangle: false,
            export_name: None,
            is_pub: false,
        },
    }
}

#[test]
fn bind_method_specialization_subst_infers_from_args() {
    let vec_def = generic_vec_struct();
    let struct_defs = HashMap::from([(vec_def.name.clone(), &vec_def)]);
    let target_type = HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]);
    let method = generic_push_method();
    let receiver_ty = MIRType::Struct { name: "Vec_i64".to_string(), fields: vec![] };

    let subst = bind_method_specialization_subst(
        &target_type,
        &method,
        &receiver_ty,
        &[MIR_I64],
        &struct_defs,
    )
    .expect("specialization should bind T from explicit arg types");

    assert_eq!(subst.get("T"), Some(&MIR_I64));
}

#[test]
fn bind_method_specialization_subst_rejects_wrong_arity() {
    let vec_def = generic_vec_struct();
    let struct_defs = HashMap::from([(vec_def.name.clone(), &vec_def)]);
    let target_type = HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]);
    let method = generic_push_method();
    let receiver_ty = MIRType::Struct { name: "Vec_i64".to_string(), fields: vec![] };

    let subst =
        bind_method_specialization_subst(&target_type, &method, &receiver_ty, &[], &struct_defs);

    assert!(subst.is_none());
}

#[test]
fn realize_method_specialization_registers_concrete_receiver_instance() {
    let target_type = HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]);
    let method = generic_push_method();
    let receiver_ty = MIRType::Struct { name: "Vec_i64".to_string(), fields: vec![] };
    let mir_subst = HashMap::from([("T".to_string(), MIR_I64)]);
    let mut registry = ConcreteTypeRegistry::default();

    let (hir_subst, concrete_prefix) = realize_method_specialization(
        &target_type,
        &method,
        &receiver_ty,
        mir_subst,
        &mut registry,
    )
    .expect("specialization should realize matching receiver");

    assert_eq!(concrete_prefix, "Vec_i64");
    assert_eq!(hir_subst.get("T"), Some(&HIRType::int(IntKind::I64)));
    assert_eq!(
        registry.hir_type_for_mir(&receiver_ty),
        Some(HIRType::named("Vec".to_string(), vec![HIRType::int(IntKind::I64)]))
    );
}

#[test]
fn realize_method_specialization_rejects_receiver_prefix_mismatch() {
    let target_type = HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]);
    let method = generic_push_method();
    let receiver_ty = MIRType::Struct { name: "Vec_bool".to_string(), fields: vec![] };
    let mir_subst = HashMap::from([("T".to_string(), MIR_I64)]);
    let mut registry = ConcreteTypeRegistry::default();

    let realized = realize_method_specialization(
        &target_type,
        &method,
        &receiver_ty,
        mir_subst,
        &mut registry,
    );

    assert!(realized.is_none());
}

#[test]
fn prepare_method_specialization_combines_binding_and_realization() {
    let vec_def = generic_vec_struct();
    let struct_defs = HashMap::from([(vec_def.name.clone(), &vec_def)]);
    let target_type = HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]);
    let method = generic_push_method();
    let receiver_ty = MIRType::Struct { name: "Vec_i64".to_string(), fields: vec![] };
    let mut registry = ConcreteTypeRegistry::default();

    let (hir_subst, concrete_prefix) = prepare_method_specialization(
        &target_type,
        &method,
        &receiver_ty,
        &[MIR_I64],
        &struct_defs,
        &mut registry,
    )
    .expect("combined helper should bind and realize the method specialization");

    assert_eq!(concrete_prefix, "Vec_i64");
    assert_eq!(hir_subst.get("T"), Some(&HIRType::int(IntKind::I64)));
}

#[test]
fn prepare_method_specialization_rejects_arity_mismatch_before_realization() {
    let vec_def = generic_vec_struct();
    let struct_defs = HashMap::from([(vec_def.name.clone(), &vec_def)]);
    let target_type = HIRType::named("Vec".to_string(), vec![HIRType::named("T".to_string(), vec![])]);
    let method = generic_push_method();
    let receiver_ty = MIRType::Struct { name: "Vec_i64".to_string(), fields: vec![] };
    let mut registry = ConcreteTypeRegistry::default();

    let prepared = prepare_method_specialization(
        &target_type,
        &method,
        &receiver_ty,
        &[],
        &struct_defs,
        &mut registry,
    );

    assert!(prepared.is_none());
}

#[test]
fn build_trait_method_candidate_names_generic_specialization_with_suffixes() {
    let template = generic_trait_template();
    let hir_subst = HashMap::from([("T".to_string(), HIRType::int(IntKind::I64))]);

    let candidate: MethodCandidate<HIRFunction> =
        build_trait_method_candidate(&template, &hir_subst, "Vec_i64");

    assert_eq!(candidate.value.name, "Vec_i64_Iterable_next_i64");
    assert_eq!(candidate.label, "Vec_i64_Iterable_next_i64 (Iterable)");
    assert_eq!(candidate.param_count, 0);
    assert!(candidate.value.type_params.is_empty());
    assert_eq!(candidate.value.return_type, HIRType::int(IntKind::I64));
}

#[test]
fn build_trait_method_candidate_names_nongeneric_specialization_without_suffixes() {
    let template = nongeneric_trait_template();
    let hir_subst = HashMap::new();

    let candidate: MethodCandidate<HIRFunction> =
        build_trait_method_candidate(&template, &hir_subst, "Vec");

    assert_eq!(candidate.value.name, "Vec_Sized_len");
    assert_eq!(candidate.label, "Vec_Sized_len (Sized)");
    assert_eq!(candidate.param_count, 0);
}
