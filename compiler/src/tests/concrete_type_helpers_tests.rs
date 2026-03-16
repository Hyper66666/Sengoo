use crate::hir::{
    HIRBody, HIRExpr, HIRFunction, HIRImpl, HIRItem, HIRLiteral, HIRParam, HIRStmt, HIRType,
    IntKind,
};
use crate::mir::concrete_type_helpers::{
    collect_concrete_named_types_from_body, collect_concrete_named_types_from_items,
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
