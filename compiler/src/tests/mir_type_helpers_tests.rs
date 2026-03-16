use crate::mir::type_helpers::is_void_like;
use crate::mir::{MIRType, MIR_I64, MIR_UNIT};

#[test]
fn is_void_like_accepts_unit_never_and_empty_tuple() {
    assert!(is_void_like(&MIR_UNIT));
    assert!(is_void_like(&MIRType::Never));
    assert!(is_void_like(&MIRType::Tuple(vec![])));
}

#[test]
fn is_void_like_rejects_value_carrying_types() {
    assert!(!is_void_like(&MIR_I64));
    assert!(!is_void_like(&MIRType::Bool));
    assert!(!is_void_like(&MIRType::Tuple(vec![MIR_I64])));
}
