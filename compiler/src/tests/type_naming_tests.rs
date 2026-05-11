use crate::hir::{FloatKind, HIRType, IntKind};
use crate::mir::{MIRType, MIR_I64};
use crate::type_naming::{hir_type_instance_name, hir_type_prefix, mir_type_instance_name};

#[test]
fn hir_type_prefix_uses_scalar_or_named_prefixes() {
    assert_eq!(hir_type_prefix(&HIRType::int(IntKind::I32)), "i32");
    assert_eq!(hir_type_prefix(&HIRType::float(FloatKind::F64)), "f64");
    assert_eq!(hir_type_prefix(&HIRType::bool()), "bool");
    assert_eq!(
        hir_type_prefix(&HIRType::named("Point".to_string(), vec![])),
        "Point"
    );
}

#[test]
fn hir_type_instance_name_expands_nested_type_shapes() {
    let ty = HIRType::tuple(vec![
        HIRType::reference(false, HIRType::int(IntKind::I64)),
        HIRType::array(HIRType::named("Vec".to_string(), vec![HIRType::bool()]), 4),
    ]);

    assert_eq!(
        hir_type_instance_name(&ty),
        "tuple_ref_i64_array_4_Vec_bool"
    );
}

#[test]
fn mir_type_instance_name_matches_scalar_and_struct_shapes() {
    let tuple_ty = MIRType::Tuple(vec![
        MIRType::Ref(Box::new(MIR_I64)),
        MIRType::Array(Box::new(MIRType::Float(32)), 2),
    ]);
    let struct_ty = MIRType::Struct {
        name: "Point".to_string(),
        fields: vec![("x".to_string(), MIR_I64), ("y".to_string(), MIR_I64)],
    };

    assert_eq!(
        mir_type_instance_name(&tuple_ty),
        "tuple_ref_i64_array_2_f32"
    );
    assert_eq!(mir_type_instance_name(&struct_ty), "Point");
}
