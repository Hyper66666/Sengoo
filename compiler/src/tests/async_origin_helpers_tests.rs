use crate::mir::{Instruction, Local, LocalKind};
use std::collections::HashMap;

use crate::mir::async_origin_helpers::{
    infer_async_base_name_from_instructions, infer_last_async_start_base,
};

fn temp(id: usize) -> Local {
    Local::new(id, LocalKind::Temp)
}

#[test]
fn infer_async_base_name_from_instructions_traces_load_sources() {
    let source_future = temp(1);
    let loaded_future = temp(2);
    let mut future_origins = HashMap::new();
    future_origins.insert(source_future, "worker".to_string());

    let instructions = vec![Instruction::Load {
        destination: loaded_future,
        source: source_future,
    }];

    assert_eq!(
        infer_async_base_name_from_instructions(loaded_future, &instructions, &future_origins),
        Some("worker".to_string())
    );
}

#[test]
fn infer_async_base_name_from_instructions_prefers_direct_origin_hit() {
    let direct_future = temp(3);
    let source_future = temp(4);
    let mut future_origins = HashMap::new();
    future_origins.insert(direct_future, "direct".to_string());
    future_origins.insert(source_future, "source".to_string());

    let instructions = vec![Instruction::Load {
        destination: direct_future,
        source: source_future,
    }];

    assert_eq!(
        infer_async_base_name_from_instructions(direct_future, &instructions, &future_origins),
        Some("direct".to_string())
    );
}

#[test]
fn infer_last_async_start_base_detects_most_recent_start_call() {
    let instructions = vec![
        Instruction::Call {
            destination: temp(5),
            func: "first__start".to_string(),
            args: vec![],
        },
        Instruction::Call {
            destination: temp(6),
            func: "plain_call".to_string(),
            args: vec![],
        },
        Instruction::Call {
            destination: temp(7),
            func: "second__start".to_string(),
            args: vec![],
        },
    ];

    assert_eq!(
        infer_last_async_start_base(&instructions),
        Some("second".to_string())
    );
}
