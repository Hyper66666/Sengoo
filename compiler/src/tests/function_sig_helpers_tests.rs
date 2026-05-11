use crate::hir::{HIRType, IntKind};
use crate::mir::function_sig_helpers::{build_function_sig, build_hir_function_sig};
use crate::mir::{MIR_BOOL, MIR_I64};
use std::collections::HashMap;

#[test]
fn build_function_sig_preserves_env_and_counts() {
    let sig = build_function_sig(
        MIR_BOOL,
        2,
        vec![
            ("capture".to_string(), MIR_I64),
            ("flag".to_string(), MIR_BOOL),
        ],
    );

    assert_eq!(sig.ret_type, MIR_BOOL);
    assert_eq!(sig.param_count, 2);
    assert_eq!(
        sig.env,
        vec![
            ("capture".to_string(), MIR_I64),
            ("flag".to_string(), MIR_BOOL)
        ]
    );
}

#[test]
fn build_hir_function_sig_maps_return_type_and_uses_empty_env() {
    let struct_defs = HashMap::new();
    let sig = build_hir_function_sig(&HIRType::int(IntKind::I64), 1, &struct_defs);

    assert_eq!(sig.ret_type, MIR_I64);
    assert_eq!(sig.param_count, 1);
    assert!(sig.env.is_empty());
}
