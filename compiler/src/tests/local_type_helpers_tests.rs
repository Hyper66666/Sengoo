use crate::mir::local_type_helpers::collect_local_types;
use crate::mir::{Local, LocalKind, MIRType, MIR_BOOL, MIR_I64};

#[test]
fn collect_local_types_preserves_local_order() {
    let first = Local::new(1, LocalKind::Param);
    let second = Local::new(2, LocalKind::User);
    let third = Local::new(3, LocalKind::Temp);

    let types = collect_local_types(&[second, first, third], |local| match local.id {
        1 => MIR_I64,
        2 => MIR_BOOL,
        3 => MIRType::float(64),
        _ => unreachable!("unexpected local id"),
    });

    assert_eq!(types, vec![MIR_BOOL, MIR_I64, MIRType::float(64)]);
}
