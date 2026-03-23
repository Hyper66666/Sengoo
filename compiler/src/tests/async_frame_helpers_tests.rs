use crate::mir::async_frame_helpers::{async_frame_slot_count, build_async_frame_layout};
use crate::mir::{Local, LocalKind, MIRType};

#[test]
fn async_frame_slot_count_counts_nested_aggregate_storage() {
    let ty = MIRType::Struct {
        name: "Pair".to_string(),
        fields: vec![
            ("left".to_string(), MIRType::Int(32)),
            (
                "right".to_string(),
                MIRType::Array(Box::new(MIRType::Bool), 3),
            ),
        ],
    };

    assert_eq!(async_frame_slot_count(&ty).unwrap(), 4);
}

#[test]
fn build_async_frame_layout_assigns_offsets_after_result_params_and_user_locals() {
    let user_locals = vec![
        (Local::new(3, LocalKind::User), MIRType::Bool),
        (Local::new(4, LocalKind::User), MIRType::Int(32)),
    ];

    let layout = build_async_frame_layout(
        "worker".to_string(),
        vec![MIRType::Int(64), MIRType::Bool],
        MIRType::Int(64),
        2,
        &user_locals,
    )
    .unwrap();

    assert_eq!(layout.param_offsets, vec![2, 3]);
    assert_eq!(layout.user_local_offsets, vec![4, 5]);
    assert_eq!(layout.await_offset_start, 6);
    assert_eq!(layout.total_slots(), 8);
}
