use crate::hir::{HIRBody, HIRFunction, HIRImpl, HIRParam, HIRType, IntKind};
use crate::mir::impl_specialization_helpers::{
    expand_impl_variants, impl_type_prefix, match_generic_impl_target,
};
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
    let concrete_named_types = HashMap::from([(crate::type_naming::hir_type_instance_name(&vec_i64), vec_i64.clone())]);
    let impl_item = HIRImpl {
        target_type: HIRType::named(
            "Vec".to_string(),
            vec![HIRType::named("T".to_string(), vec![])],
        ),
        trait_name: None,
        items: vec![method("vec_len")],
    };

    let variants = expand_impl_variants(&impl_item, &concrete_named_types, &known_named_types);
    assert_eq!(variants.len(), 1);
    assert_eq!(variants[0].target_type, vec_i64);
    assert!(variants[0].items[0].name.starts_with(&impl_type_prefix(&variants[0].target_type)));
}
