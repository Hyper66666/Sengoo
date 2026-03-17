use crate::mir::method_dispatch_helpers::{
    method_dispatch_name, receiver_type_display, receiver_type_prefix,
};
use crate::mir::MIRType;

#[test]
fn receiver_type_prefix_maps_scalar_pointer_and_struct_types() {
    assert_eq!(receiver_type_prefix(&MIRType::Int(32)), "i32");
    assert_eq!(receiver_type_prefix(&MIRType::Float(64)), "f64");
    assert_eq!(receiver_type_prefix(&MIRType::Bool), "bool");
    assert_eq!(
        receiver_type_prefix(&MIRType::Ptr(Box::new(MIRType::Int(8)))),
        "i8_ptr"
    );
    assert_eq!(
        receiver_type_prefix(&MIRType::Struct {
            name: "Point".to_string(),
            fields: vec![],
        }),
        "Point"
    );
}

#[test]
fn method_dispatch_name_prefers_explicit_type_name_over_shape_based_prefix() {
    assert_eq!(
        method_dispatch_name(Some("Vec_i64"), &MIRType::Array(Box::new(MIRType::Int(64)), 3), "len"),
        "Vec_i64_len"
    );
    assert_eq!(
        method_dispatch_name(None, &MIRType::Ptr(Box::new(MIRType::Bool)), "flip"),
        "bool_ptr_flip"
    );
}

#[test]
fn receiver_type_display_prefers_explicit_name_and_has_pointer_fallback() {
    assert_eq!(
        receiver_type_display(Some("Map_i64_bool"), &MIRType::Tuple(vec![MIRType::Int(64)])),
        "Map_i64_bool"
    );
    assert_eq!(
        receiver_type_display(None, &MIRType::Ref(Box::new(MIRType::Int(64)))),
        "ptr"
    );
}
