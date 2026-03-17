use crate::hir::{HIRItem, HIRStruct, HIRType, IntKind};
use crate::mir::type_mapping_helpers::{
    bind_mir_subst_from_hir_type, hir_type_to_mir_with_structs,
};
use crate::mir::{MIRType, MIR_I64};
use crate::{lower_ast, Parser, TypeChecker};
use std::collections::HashMap;

fn lower_struct_defs(source: &str) -> HashMap<String, HIRStruct> {
    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("typecheck should succeed");
    let env = checker.into_env();
    let module = lower_ast(&program, &env);

    module
        .items
        .into_iter()
        .filter_map(|item| match item {
            HIRItem::Struct(struct_item) => Some((struct_item.name.clone(), struct_item)),
            _ => None,
        })
        .collect()
}

#[test]
fn hir_type_to_mir_with_structs_expands_generic_struct_fields() {
    let defs = lower_struct_defs(
        r#"
struct Box<T> { value: T }
struct Pair<T> { left: T, right: Box<T> }
"#,
    );
    let struct_refs = defs
        .iter()
        .map(|(name, def)| (name.clone(), def))
        .collect::<HashMap<_, _>>();

    let hir_ty = HIRType::named("Pair".to_string(), vec![HIRType::int(IntKind::I64)]);
    let mir_ty = hir_type_to_mir_with_structs(&hir_ty, &struct_refs);

    let MIRType::Struct { name, fields } = mir_ty else {
        panic!("expected MIR struct");
    };
    assert_eq!(name, "Pair_i64");
    assert_eq!(fields[0].0, "left");
    assert_eq!(fields[0].1, MIR_I64);
    let MIRType::Struct {
        name: nested_name,
        fields: nested_fields,
    } = &fields[1].1
    else {
        panic!("expected nested Box<T> to lower as MIR struct");
    };
    assert_eq!(nested_name, "Box_i64");
    assert_eq!(nested_fields[0].1, MIR_I64);
}

#[test]
fn bind_mir_subst_from_hir_type_collects_struct_field_bindings() {
    let defs = lower_struct_defs(
        r#"
struct Box<T> { value: T }
"#,
    );
    let struct_refs = defs
        .iter()
        .map(|(name, def)| (name.clone(), def))
        .collect::<HashMap<_, _>>();

    let template = HIRType::named("Box".to_string(), vec![HIRType::named("T".to_string(), vec![])]);
    let actual = MIRType::Struct {
        name: "Box_i64".to_string(),
        fields: vec![("value".to_string(), MIR_I64)],
    };
    let mut subst = HashMap::new();

    bind_mir_subst_from_hir_type(&template, &actual, &struct_refs, &mut subst);

    assert_eq!(subst.get("T"), Some(&MIR_I64));
}
