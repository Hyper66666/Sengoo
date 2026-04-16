use crate::mir::async_dispatch_helpers::build_async_dispatch_registry;
use crate::mir::async_dispatch_synthesis_helpers::{
    select_result_dispatch_name, select_runtime_declaration, select_runtime_function_name,
    select_winner_runtime_declaration, select_winner_runtime_function_name,
    synthesize_result_dispatch, synthesize_spawn_poll_dispatch,
};
use crate::mir::{MIRType, Terminator, MIR_BOOL, MIR_I64};

#[test]
fn async_dispatch_synthesis_helpers_map_scalar_types_to_expected_symbols() {
    assert_eq!(
        select_runtime_function_name(&MIRType::Int(32)).as_deref(),
        Some("sengoo_async_select_i32")
    );
    assert_eq!(
        select_result_dispatch_name(&MIRType::Float(64)).as_deref(),
        Some("sengoo_async_result_dispatch_f64")
    );
    assert_eq!(
        select_runtime_declaration(&MIR_BOOL).as_deref(),
        Some("declare i1 @sengoo_async_select_bool(i64, i64, i64, i64)\n")
    );
    assert_eq!(
        select_winner_runtime_function_name(),
        "sengoo_async_select_winner"
    );
    assert_eq!(
        select_winner_runtime_declaration(),
        "declare i64 @sengoo_async_select_winner(i64, i64, i64, i64)\n"
    );
}

#[test]
fn async_dispatch_synthesis_helpers_use_registry_ordinals_for_switch_targets() {
    let registry = build_async_dispatch_registry([
        "worker_b".to_string(),
        "worker_a".to_string(),
    ]);
    let dispatch = synthesize_spawn_poll_dispatch(
        &registry,
        &[("worker_b".to_string(), "worker_b__poll".to_string())],
    )
    .expect("spawn dispatch should synthesize with stable ordinals");

    let Some(Terminator::Switch { targets, .. }) =
        dispatch.basic_blocks[dispatch.start_block].terminator.as_ref()
    else {
        panic!("spawn poll dispatch should start with a switch terminator");
    };

    let seen = targets.iter().map(|(kind, _)| *kind).collect::<Vec<_>>();
    assert!(seen.contains(&1), "sleep builtin ordinal should be reserved");
    assert!(seen.contains(&2), "timeout builtin ordinal should be reserved");
    assert!(seen.contains(&4), "worker_b should receive stable sorted ordinal");
}

#[test]
fn async_dispatch_synthesis_helpers_report_unsupported_result_dispatch_type() {
    let registry = build_async_dispatch_registry(["worker".to_string()]);
    let err = synthesize_result_dispatch(
        &registry,
        &MIRType::Struct {
            name: "Point".to_string(),
            fields: vec![("x".to_string(), MIR_I64)],
        },
        &[("worker".to_string(), "worker__result".to_string())],
    )
    .expect_err("unsupported result dispatch types should return a compile error");

    assert!(format!("{err}").contains("unsupported async result dispatch type"));
}
