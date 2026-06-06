use super::async_frame_helpers::{push_frame_load_typed, push_frame_store_typed, AsyncFrameLayout};
use crate::mir::{
    Instruction, Local, LocalKind, MirConstant, MirFunction, Terminator, MIR_I64, MIR_UNIT,
};
use crate::CompileError;

pub(crate) fn count_await_points(mir_fn: &MirFunction) -> usize {
    mir_fn
        .basic_blocks
        .iter()
        .filter(|bb| matches!(bb.terminator, Some(Terminator::Suspend { .. })))
        .count()
}

pub(crate) fn synthesize_async_main_wrapper() -> MirFunction {
    let mut f = MirFunction::new("main".to_string(), vec![], MIR_I64);
    let bb0 = f.start_block;

    let result = f.add_local(LocalKind::Temp, MIR_I64);
    let runtime_call = f.alloc_inst(Instruction::Call {
        destination: result,
        func: "sengoo_async_run_main_i64".to_string(),
        args: vec![],
    });
    f.basic_blocks[bb0].push(runtime_call);
    f.basic_blocks[bb0].set_terminator(Terminator::Return(Some(result)));

    f
}

pub(crate) fn synthesize_start(layout: &AsyncFrameLayout) -> Result<MirFunction, CompileError> {
    let name = format!("{}__start", layout.func_name);
    let mut f = MirFunction::new(name, layout.param_types.clone(), MIR_I64);

    let total_slots = layout.total_slots();
    let bb0 = f.start_block;

    let slots_local = f.add_local(LocalKind::Temp, MIR_I64);
    let handle_local = f.add_local(LocalKind::Temp, MIR_I64);

    let alloc_inst = f.alloc_inst(Instruction::Assign {
        destination: slots_local,
        value: MirConstant::Int(total_slots as i64),
    });
    f.basic_blocks[bb0].push(alloc_inst);

    let call_inst = f.alloc_inst(Instruction::Call {
        destination: handle_local,
        func: "sengoo_async_frame_alloc".to_string(),
        args: vec![slots_local],
    });
    f.basic_blocks[bb0].push(call_inst);

    let zero = f.add_local(LocalKind::Temp, MIR_I64);
    let zero_inst = f.alloc_inst(Instruction::Assign {
        destination: zero,
        value: MirConstant::Int(0),
    });
    f.basic_blocks[bb0].push(zero_inst);

    let store_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
    let store_state = f.alloc_inst(Instruction::Call {
        destination: store_dest,
        func: "sengoo_async_frame_store".to_string(),
        args: vec![handle_local, zero, zero],
    });
    f.basic_blocks[bb0].push(store_state);

    for i in 0..layout.param_types.len() {
        let param_local = Local::new(i + 1, LocalKind::Param);
        push_frame_store_typed(
            &mut f,
            bb0,
            handle_local,
            layout.param_offsets[i],
            param_local,
            &layout.param_types[i],
        )?;
    }

    f.basic_blocks[bb0].set_terminator(Terminator::Return(Some(handle_local)));
    Ok(f)
}

pub(crate) fn synthesize_result(layout: &AsyncFrameLayout) -> Result<MirFunction, CompileError> {
    let name = format!("{}__result", layout.func_name);
    let ret = layout.result_storage_ty.clone();
    let mut f = MirFunction::new(name, vec![MIR_I64], ret.clone());

    let handle = Local::new(1, LocalKind::Param);
    let bb0 = f.start_block;

    let result = push_frame_load_typed(&mut f, bb0, handle, 1, ret.clone())?;

    let free_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
    let free_inst = f.alloc_inst(Instruction::Call {
        destination: free_dest,
        func: "sengoo_async_frame_free".to_string(),
        args: vec![handle],
    });
    f.basic_blocks[bb0].push(free_inst);

    f.basic_blocks[bb0].set_terminator(Terminator::Return(Some(result)));
    Ok(f)
}
