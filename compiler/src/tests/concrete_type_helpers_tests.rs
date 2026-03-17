use crate::hir::{
    HIRBody, HIRExpr, HIRFunction, HIRImpl, HIRItem, HIRLiteral, HIRParam, HIRStmt, HIRType,
    HIRTypeParam, IntKind,
};
use crate::mir::concrete_type_helpers::{
    collect_concrete_named_types_from_body, collect_concrete_named_types_from_items,
    collect_concrete_named_types_with_impl_variants,
};
use crate::symbol::SymbolId;
use crate::type_naming::hir_type_instance_name;
use std::collections::{HashMap, HashSet};

#[test]
fn collect_concrete_named_types_from_body_finds_nested_named_instances() {
    let known_named_types = HashSet::from(["Vec".to_string(), "Point".to_string()]);
    let vec_i64 = HIRType::named("Vec".to_string(), vec![HIRType::int(IntKind::I64)]);
    let point = HIRType::named("Point".to_string(), vec![]);

    let body = HIRBody {
        stmts: vec![HIRStmt::Let {
            name: "v".to_string(),
            symbol: SymbolId::new(1),
            ty: vec_i64.clone(),
            value: Some(HIRExpr::Ascribe(
                Box::new(HIRExpr::Lit(HIRLiteral::Int(1))),
                point.clone(),
            )),
            is_mut: false,
        }],
        expr: Some(Box::new(HIRExpr::AsyncBlock(Box::new(HIRBody::with_expr(
            HIRExpr::Cast(Box::new(HIRExpr::Lit(HIRLiteral::Int(2))), vec_i64.clone()),
        ))))),
    };

    let mut out = HashMap::new();
    collect_concrete_named_types_from_body(&body, &known_named_types, &mut out);

    assert_eq!(out.get(&hir_type_instance_name(&vec_i64)), Some(&vec_i64));
    assert!(!out.contains_key("Point"));
}

#[test]
fn collect_concrete_named_types_from_items_includes_function_impl_and_struct_fields() {
    let known_named_types =
        HashSet::from(["Vec".to_string(), "Map".to_string(), "Point".to_string()]);
    let vec_i64 = HIRType::named("Vec".to_string(), vec![HIRType::int(IntKind::I64)]);
    let map_point_i64 = HIRType::named(
        "Map".to_string(),
        vec![HIRType::named("Point".to_string(), vec![]), HIRType::int(IntKind::I64)],
    );

    let function = HIRItem::Function(HIRFunction {
        name: "main".to_string(),
        type_params: vec![],
        params: vec![HIRParam::new("arg".to_string(), SymbolId::new(2), vec_i64.clone())],
        return_type: map_point_i64.clone(),
        precondition: None,
        postcondition: None,
        body: HIRBody::new(),
        is_async: false,
        abi: None,
        is_unsafe: false,
        no_mangle: false,
        export_name: None,
        is_pub: false,
    });

    let impl_item = HIRItem::Impl(HIRImpl {
        target_type: vec_i64.clone(),
        trait_name: None,
        items: vec![HIRFunction {
            name: "size".to_string(),
            type_params: vec![],
            params: vec![],
            return_type: HIRType::int(IntKind::I64),
            precondition: None,
            postcondition: None,
            body: HIRBody::with_expr(HIRExpr::Lit(HIRLiteral::Int(0))),
            is_async: false,
            abi: None,
            is_unsafe: false,
            no_mangle: false,
            export_name: None,
            is_pub: false,
        }],
    });

    let out = collect_concrete_named_types_from_items(&[function, impl_item], &known_named_types);

    assert_eq!(out.get(&hir_type_instance_name(&vec_i64)), Some(&vec_i64));
    assert_eq!(
        out.get(&hir_type_instance_name(&map_point_i64)),
        Some(&map_point_i64)
    );
}

#[test]
fn collect_concrete_named_types_with_impl_variants_reaches_fixed_point() {
    let known_named_types =
        HashSet::from(["Vec".to_string(), "Map".to_string(), "Point".to_string()]);
    let placeholder_t = HIRType::named("T".to_string(), vec![]);
    let vec_t = HIRType::named("Vec".to_string(), vec![placeholder_t.clone()]);
    let vec_i64 = HIRType::named("Vec".to_string(), vec![HIRType::int(IntKind::I64)]);
    let map_vec_i64_i64 = HIRType::named(
        "Map".to_string(),
        vec![vec_i64.clone(), HIRType::int(IntKind::I64)],
    );

    let seed_function = HIRItem::Function(HIRFunction {
        name: "seed".to_string(),
        type_params: vec![],
        params: vec![HIRParam::new("value".to_string(), SymbolId::new(3), vec_i64.clone())],
        return_type: HIRType::unit(),
        precondition: None,
        postcondition: None,
        body: HIRBody::new(),
        is_async: false,
        abi: None,
        is_unsafe: false,
        no_mangle: false,
        export_name: None,
        is_pub: false,
    });

    let generic_impl = HIRItem::Impl(HIRImpl {
        target_type: vec_t.clone(),
        trait_name: None,
        items: vec![HIRFunction {
            name: "Vec_into_map".to_string(),
            type_params: vec![HIRTypeParam {
                name: "T".to_string(),
                bounds: vec![],
                default: None,
            }],
            params: vec![],
            return_type: HIRType::named(
                "Map".to_string(),
                vec![vec_t.clone(), HIRType::int(IntKind::I64)],
            ),
            precondition: None,
            postcondition: None,
            body: HIRBody::with_expr(HIRExpr::Lit(HIRLiteral::Int(0))),
            is_async: false,
            abi: None,
            is_unsafe: false,
            no_mangle: false,
            export_name: None,
            is_pub: false,
        }],
    });

    let out = collect_concrete_named_types_with_impl_variants(
        &[seed_function, generic_impl],
        &known_named_types,
    );

    assert_eq!(out.get(&hir_type_instance_name(&vec_i64)), Some(&vec_i64));
    assert_eq!(
        out.get(&hir_type_instance_name(&map_vec_i64_i64)),
        Some(&map_vec_i64_i64)
    );
}
