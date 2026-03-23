use crate::mir::async_entry_helpers::{count_await_points, synthesize_async_main_wrapper};
use crate::mir::{MirFunction, Terminator, MIR_I64};

#[test]
fn async_entry_helpers_count_await_points_counts_suspend_terminators() {
    let mut mir_fn = MirFunction::new("main".to_string(), vec![], MIR_I64);
    let ready = mir_fn.add_block();
    let pending = mir_fn.add_block();
    let future_handle = mir_fn.add_local(crate::mir::LocalKind::Temp, MIR_I64);
    let suspend_result = mir_fn.add_local(crate::mir::LocalKind::Temp, MIR_I64);
    mir_fn.basic_blocks[mir_fn.start_block].set_terminator(Terminator::Suspend {
        poll_func: "child__poll".to_string(),
        future_handle,
        destination: suspend_result,
        ready_block: ready,
        pending_block: pending,
    });
    mir_fn.basic_blocks[ready].set_terminator(Terminator::Return(Some(suspend_result)));
    mir_fn.basic_blocks[pending].set_terminator(Terminator::Goto(pending));

    assert_eq!(count_await_points(&mir_fn), 1);
}

#[test]
fn async_entry_helpers_main_wrapper_calls_start_poll_and_result() {
    let wrapper = synthesize_async_main_wrapper();
    assert_eq!(wrapper.name, "main");
    let entry = &wrapper.basic_blocks[wrapper.start_block];
    assert!(entry.instructions.iter().any(|id| matches!(wrapper.instruction(*id), crate::mir::Instruction::Call { func, .. } if func == "main__start")));
    assert!(wrapper.basic_blocks.iter().any(|bb| bb.instructions.iter().any(|id| matches!(wrapper.instruction(*id), crate::mir::Instruction::Call { func, .. } if func == "main__poll"))));
    assert!(wrapper.basic_blocks.iter().any(|bb| bb.instructions.iter().any(|id| matches!(wrapper.instruction(*id), crate::mir::Instruction::Call { func, .. } if func == "main__result"))));
}
