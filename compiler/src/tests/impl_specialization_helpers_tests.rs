use crate::hir::{HIRBody, HIRFunction, HIRImpl, HIRParam, HIRType, IntKind};
use crate::mir::impl_specialization_helpers::{
    build_inherent_specialized_method, collect_matching_inherent_method_templates,
    expand_impl_variants, impl_type_prefix, match_generic_impl_target,
    resolve_inherent_method_specialization, specialize_matching_inherent_method,
};
use crate::mir::{ConcreteTypeRegistry, InherentMethodTemplate, MIRType, MIR_I64};
use crate::symbol::SymbolId;
use std::collections::{HashMap, HashSet};

fn method(name: &str) -> HIRFunction {
    HIRFunction {
        name: name.to_string(),
        type_params: vec![],
        params: vec![HIRParam::new(
            "self".to_string(),
            SymbolId::new(1),
            HIRType::named("Self".to_string(), vec![]),
        )],
        return_type: HIRType::int(IntKind::I64),
        precondition: None,
        postcondition: None,
        body: HIRBody::with_expr(crate::hir::HIRExpr::Lit(crate::hir::HIRLiteral::Int(0))),
        is_async: false,
        abi: None,
        is_unsafe: false,
        no_mangle: false,
        export_name: None,
        is_pub: false,
    }
}

fn generic_method(name: &str) -> HIRFunction {
    let mut function = method(name);
    function.type_params = vec![crate::hir::HIRTypeParam {
        name: "T".to_string(),
        bounds: vec![],
        default: None,
    }];
    function.params.push(HIRParam::new(
        "value".to_string(),
        SymbolId::new(2),
        HIRType::named("T".to_string(), vec![]),
    ));
    function.return_type = HIRType::named("T".to_string(), vec![]);
    function
}

fn generic_vec_struct() -> crate::hir::HIRStruct {
    crate::hir::HIRStruct {
        name: "Vec".to_string(),
        type_params: vec![crate::hir::HIRTypeParam {
            name: "T".to_string(),
            bounds: vec![],
            default: None,
        }],
        fields: vec![],
        is_pub: false,
    }
}

#[test]
fn match_generic_impl_target_binds_placeholder_names_consistently() {
    let known_named_types = HashSet::from(["Vec".to_string()]);
    let template = HIRType::named("T".to_string(), vec![]);
    let concrete = HIRType::int(IntKind::I64);
    let mut subst = HashMap::new();

    assert!(match_generic_impl_target(
        &template,
        &concrete,
        &known_named_types,
        &mut subst
    ));
    assert_eq!(subst.get("T"), Some(&concrete));

    let mut inconsistent = subst.clone();
    assert!(!match_generic_impl_target(
        &template,
        &HIRType::named("Vec".to_string(), vec![]),
        &known_named_types,
        &mut inconsistent
    ));
}

#[test]
fn expand_impl_variants_instantiates_methods_for_concrete_named_targets() {
    let vec_i64 = HIRType::named("Vec".to_string(), vec![HIRType::int(IntKind::I64)]);
    let known_named_types = HashSet::from(["Vec".to_string()]);
    let concrete_named_types = HashMap::from([(
        crate::type_naming::hir_type_instance_name(&vec_i64),
        vec_i64.clone(),
    )]);
    let impl_item = HIRImpl {
        target_type: HIRType::named(
            "Vec".to_string(),
            vec![HIRType::named("T".to_string(), vec![])],
        ),
        trait_name: None,
        trait_args: Vec::new(),
        items: vec![method("vec_len")],
    };

    let variants = expand_impl_variants(&impl_item, &concrete_named_types, &known_named_types);
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].target_type, vec_i64);
    assert!(variants[0].items[0]
        .name
        .starts_with(&impl_type_prefix(&variants[0].target_type)));
}

#[test]
fn build_inherent_specialized_method_appends_generic_suffixes() {
    let method = generic_method("Vec_push");
    let subst = HashMap::from([("T".to_string(), HIRType::int(IntKind::I64))]);

    let specialized = build_inherent_specialized_method(&method, "Vec", "Vec_i64", &subst);

    assert_eq!(specialized.name, "Vec_i64_push_i64");
    assert!(specialized.type_params.is_empty());
    assert_eq!(specialized.return_type, HIRType::int(IntKind::I64));
}

#[test]
fn build_inherent_specialized_method_keeps_nongeneric_name_without_suffixes() {
    let method = method("Vec_len");
    let specialized = build_inherent_specialized_method(&method, "Vec", "Vec_i64", &HashMap::new());

    assert_eq!(specialized.name, "Vec_i64_len");
    assert!(specialized.type_params.is_empty());
}

#[test]
fn collect_matching_inherent_method_templates_strips_legacy_prefix() {
    let templates = vec![
        InherentMethodTemplate {
            target_type: HIRType::named("Vec".to_string(), vec![]),
            method: method("Vec_len"),
        },
        InherentMethodTemplate {
            target_type: HIRType::named("Vec".to_string(), vec![]),
            method: method("Vec_push"),
        },
    ];

    let matches = collect_matching_inherent_method_templates(&templates, "len");

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].1, "Vec");
    assert_eq!(matches[0].0.method.name, "Vec_len");
}

#[test]
fn collect_matching_inherent_method_templates_keeps_unprefixed_name() {
    let templates = vec![InherentMethodTemplate {
        target_type: HIRType::named("Point".to_string(), vec![]),
        method: method("sum"),
    }];

    let matches = collect_matching_inherent_method_templates(&templates, "sum");

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0].1, "Point");
    assert_eq!(matches[0].0.method.name, "sum");
}

#[test]
fn specialize_matching_inherent_method_builds_generic_specialization() {
    let vec_def = generic_vec_struct();
    let struct_defs = HashMap::from([(vec_def.name.clone(), &vec_def)]);
    let template = InherentMethodTemplate {
        target_type: HIRType::named(
            "Vec".to_string(),
            vec![HIRType::named("T".to_string(), vec![])],
        ),
        method: generic_method("Vec_push"),
    };
    let mut registry = ConcreteTypeRegistry::default();
    let receiver_ty = MIRType::Struct {
        name: "Vec_i64".to_string(),
        fields: vec![],
    };

    let specialized = specialize_matching_inherent_method(
        &template,
        "Vec",
        &receiver_ty,
        &[MIR_I64],
        &struct_defs,
        &mut registry,
    )
    .expect("matching inherent template should specialize");

    assert_eq!(specialized.name, "Vec_i64_push_i64");
    assert!(specialized.type_params.is_empty());
    assert_eq!(specialized.return_type, HIRType::int(IntKind::I64));
}

#[test]
fn resolve_inherent_method_specialization_skips_unrealizable_match_and_uses_next() {
    let vec_def = generic_vec_struct();
    let struct_defs = HashMap::from([(vec_def.name.clone(), &vec_def)]);
    let receiver_ty = MIRType::Struct {
        name: "Vec_i64".to_string(),
        fields: vec![],
    };
    let mut registry = ConcreteTypeRegistry::default();
    let templates = vec![
        InherentMethodTemplate {
            target_type: HIRType::named(
                "Vec".to_string(),
                vec![HIRType::named("T".to_string(), vec![])],
            ),
            method: generic_method("Vec_push"),
        },
        InherentMethodTemplate {
            target_type: HIRType::named(
                "Vec".to_string(),
                vec![HIRType::named("T".to_string(), vec![])],
            ),
            method: {
                let mut method = generic_method("Vec_push");
                method.params[1].ty = HIRType::bool();
                method.return_type = HIRType::bool();
                method
            },
        },
    ];

    let specialized = resolve_inherent_method_specialization(
        &templates,
        "push",
        &receiver_ty,
        &[MIR_I64],
        &struct_defs,
        &mut registry,
    )
    .expect("one matching template should specialize");

    assert_eq!(specialized.name, "Vec_i64_push_i64");
    assert_eq!(specialized.return_type, HIRType::int(IntKind::I64));
}

#[test]
fn resolve_inherent_method_specialization_returns_none_when_no_match_specializes() {
    let vec_def = generic_vec_struct();
    let struct_defs = HashMap::from([(vec_def.name.clone(), &vec_def)]);
    let receiver_ty = MIRType::Struct {
        name: "Vec_i64".to_string(),
        fields: vec![],
    };
    let mut registry = ConcreteTypeRegistry::default();
    let templates = vec![InherentMethodTemplate {
        target_type: HIRType::named(
            "Vec".to_string(),
            vec![HIRType::named("T".to_string(), vec![])],
        ),
        method: {
            let mut method = generic_method("Vec_push");
            method.params[1].ty = HIRType::bool();
            method.return_type = HIRType::bool();
            method
        },
    }];

    let specialized = resolve_inherent_method_specialization(
        &templates,
        "push",
        &receiver_ty,
        &[MIR_I64],
        &struct_defs,
        &mut registry,
    );

    assert!(specialized.is_none());
}
