//! Async lowering: synthesize frame-backed __start/__poll/__result helpers
//!
//! For each `async def foo(params...) -> T`, we generate three helper functions:
//!   - `foo__start(params...) -> i64`  — allocates a frame, stores params, sets state=0, returns handle
//!   - `foo__poll(handle: i64) -> i64`  — runs until next suspend or completion, returns 0=pending, 1=ready
//!   - `foo__result(handle: i64) -> T`  — reads the result from the frame, frees it, returns T

use crate::mir::{
    Instruction, Local, LocalKind, MIRType, MirConstant,
    MirFunction, Terminator, MIR_I64, MIR_UNIT,
};

/// Per-async-function frame layout (logical, not physical — fields stored at offsets in a malloc'd block)
///   offset 0: state (i64)
///   offset 1: result (i64, holds return value once ready)
///   offset 2..2+N: parameter copies
///   offset 2+N..: child future handles + spilled locals
#[derive(Debug, Clone)]
pub struct AsyncFrameLayout {
    pub func_name: String,
    pub param_count: usize,
    pub return_type: MIRType,
    pub await_count: usize,
}

impl AsyncFrameLayout {
    pub fn total_slots(&self) -> usize {
        2 + self.param_count + self.await_count
    }
}

/// Count the number of await points in a MIR function by scanning for Suspend terminators
pub fn count_await_points(mir_fn: &MirFunction) -> usize {
    mir_fn
        .basic_blocks
        .iter()
        .filter(|bb| matches!(bb.terminator, Some(Terminator::Suspend { .. })))
        .count()
}

/// Given a list of MIR functions, expand each async function into its original body
/// plus three synthesized helpers. Returns additional functions to add.
///
/// For async `main`, the original body is renamed to `main__body` and a new
/// `main` wrapper is generated that drives the async helpers.
pub fn expand_async_functions(mir_fns: &mut Vec<MirFunction>) -> Vec<MirFunction> {
    let async_fn_names: Vec<String> = mir_fns
        .iter()
        .filter(|f| f.is_async)
        .map(|f| f.name.clone())
        .collect();

    let has_async_main = async_fn_names.iter().any(|n| n == "main");

    let mut new_fns = Vec::new();

    for name in &async_fn_names {
        let mir_fn = match mir_fns.iter().find(|f| &f.name == name) {
            Some(f) => f,
            None => continue,
        };

        let body_name = if name == "main" {
            "main__body".to_string()
        } else {
            name.clone()
        };

        let layout = AsyncFrameLayout {
            func_name: body_name,
            param_count: mir_fn.params.len(),
            return_type: mir_fn.return_type.clone(),
            await_count: count_await_points(mir_fn),
        };

        let mut start = synthesize_start(&layout);
        let mut poll = synthesize_poll(&layout);
        let mut result = synthesize_result(&layout);

        if name == "main" {
            start.name = "main__start".to_string();
            poll.name = "main__poll".to_string();
            result.name = "main__result".to_string();
        }

        new_fns.push(start);
        new_fns.push(poll);
        new_fns.push(result);
    }

    if has_async_main {
        if let Some(main_fn) = mir_fns.iter_mut().find(|f| f.name == "main") {
            main_fn.name = "main__body".to_string();
        }
        new_fns.push(synthesize_async_main_wrapper());
    }

    new_fns
}

/// Generate a `main` wrapper that drives async main through the helper ABI:
///   handle = main__start()
///   while main__poll(handle) == 0 { }
///   return main__result(handle)
fn synthesize_async_main_wrapper() -> MirFunction {
    let mut f = MirFunction::new("main".to_string(), vec![], MIR_I64);
    let bb0 = f.start_block;

    // handle = main__start()
    let handle = f.add_local(LocalKind::Temp, MIR_I64);
    let start_call = f.alloc_inst(Instruction::Call {
        destination: handle,
        func: "main__start".to_string(),
        args: vec![],
    });
    f.basic_blocks[bb0].push(start_call);

    // poll loop
    let poll_block = f.add_block();
    let ready_block = f.add_block();

    f.basic_blocks[bb0].set_terminator(Terminator::Goto(poll_block));

    // poll_block: status = main__poll(handle)
    let status = f.add_local(LocalKind::Temp, MIR_I64);
    let poll_call = f.alloc_inst(Instruction::Call {
        destination: status,
        func: "main__poll".to_string(),
        args: vec![handle],
    });
    f.basic_blocks[poll_block].push(poll_call);

    // branch: status == 1 -> ready_block, else -> poll_block
    let one = f.add_local(LocalKind::Temp, MIR_I64);
    let one_inst = f.alloc_inst(Instruction::Assign {
        destination: one,
        value: MirConstant::Int(1),
    });
    f.basic_blocks[poll_block].push(one_inst);

    f.basic_blocks[poll_block].set_terminator(Terminator::Switch {
        discr: status,
        targets: vec![(1, ready_block)],
        otherwise: poll_block,
    });

    // ready_block: result = main__result(handle); return result
    let result = f.add_local(LocalKind::Temp, MIR_I64);
    let result_call = f.alloc_inst(Instruction::Call {
        destination: result,
        func: "main__result".to_string(),
        args: vec![handle],
    });
    f.basic_blocks[ready_block].push(result_call);
    f.basic_blocks[ready_block].set_terminator(Terminator::Return(Some(result)));

    f
}

/// Generate `foo__start(params...) -> i64`
fn synthesize_start(layout: &AsyncFrameLayout) -> MirFunction {
    let name = format!("{}__start", layout.func_name);
    let params: Vec<MIRType> = (0..layout.param_count).map(|_| MIR_I64).collect();
    let mut f = MirFunction::new(name, params, MIR_I64);

    let total_slots = layout.total_slots();

    // _0 = return local (handle)
    // _1.._N = param locals (already created by MirFunction::new)
    let bb0 = f.start_block;

    // Allocate frame: handle = sengoo_async_frame_alloc(total_slots)
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

    // Store state = 0 at offset 0
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

    // Store each parameter at offset 2+i
    for i in 0..layout.param_count {
        let param_local = Local::new(i + 1, LocalKind::Param);
        let offset = f.add_local(LocalKind::Temp, MIR_I64);
        let offset_inst = f.alloc_inst(Instruction::Assign {
            destination: offset,
            value: MirConstant::Int((2 + i) as i64),
        });
        f.basic_blocks[bb0].push(offset_inst);

        let sp_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
        let store_param = f.alloc_inst(Instruction::Call {
            destination: sp_dest,
            func: "sengoo_async_frame_store".to_string(),
            args: vec![handle_local, offset, param_local],
        });
        f.basic_blocks[bb0].push(store_param);
    }

    // Return the handle
    f.basic_blocks[bb0].set_terminator(Terminator::Return(Some(handle_local)));

    f
}

/// Generate `foo__poll(handle: i64) -> i64`
/// Returns 0 = pending, 1 = ready
fn synthesize_poll(layout: &AsyncFrameLayout) -> MirFunction {
    let name = format!("{}__poll", layout.func_name);
    let mut f = MirFunction::new(name, vec![MIR_I64], MIR_I64);

    let handle = Local::new(1, LocalKind::Param);
    let bb0 = f.start_block;

    // Load state from frame[0]
    let zero = f.add_local(LocalKind::Temp, MIR_I64);
    let state = f.add_local(LocalKind::Temp, MIR_I64);

    let zero_inst = f.alloc_inst(Instruction::Assign {
        destination: zero,
        value: MirConstant::Int(0),
    });
    f.basic_blocks[bb0].push(zero_inst);

    let load_state = f.alloc_inst(Instruction::Call {
        destination: state,
        func: "sengoo_async_frame_load".to_string(),
        args: vec![handle, zero],
    });
    f.basic_blocks[bb0].push(load_state);

    let n_states = layout.await_count.max(1);

    if n_states <= 1 {
        // Single-await or no-await: call the original function body, store result, return ready
        let body_block = f.add_block();
        let done_block = f.add_block();

        // Jump to body
        f.basic_blocks[bb0].set_terminator(Terminator::Goto(body_block));

        // Body block: load params, call original, store result
        let result_val = f.add_local(LocalKind::Temp, MIR_I64);

        // Load params from frame
        let mut param_locals = Vec::new();
        for i in 0..layout.param_count {
            let off = f.add_local(LocalKind::Temp, MIR_I64);
            let off_inst = f.alloc_inst(Instruction::Assign {
                destination: off,
                value: MirConstant::Int((2 + i) as i64),
            });
            f.basic_blocks[body_block].push(off_inst);

            let p = f.add_local(LocalKind::Temp, MIR_I64);
            let load_p = f.alloc_inst(Instruction::Call {
                destination: p,
                func: "sengoo_async_frame_load".to_string(),
                args: vec![handle, off],
            });
            f.basic_blocks[body_block].push(load_p);
            param_locals.push(p);
        }

        // Call original function
        let call_original = f.alloc_inst(Instruction::Call {
            destination: result_val,
            func: layout.func_name.clone(),
            args: param_locals,
        });
        f.basic_blocks[body_block].push(call_original);

        // Store result at frame[1]
        let one = f.add_local(LocalKind::Temp, MIR_I64);
        let one_inst = f.alloc_inst(Instruction::Assign {
            destination: one,
            value: MirConstant::Int(1),
        });
        f.basic_blocks[body_block].push(one_inst);

        let sr_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
        let store_result = f.alloc_inst(Instruction::Call {
            destination: sr_dest,
            func: "sengoo_async_frame_store".to_string(),
            args: vec![handle, one, result_val],
        });
        f.basic_blocks[body_block].push(store_result);

        // Set state to completed (n_states)
        let final_state = f.add_local(LocalKind::Temp, MIR_I64);
        let final_state_inst = f.alloc_inst(Instruction::Assign {
            destination: final_state,
            value: MirConstant::Int(n_states as i64),
        });
        f.basic_blocks[body_block].push(final_state_inst);

        let sfs_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
        let store_final_state = f.alloc_inst(Instruction::Call {
            destination: sfs_dest,
            func: "sengoo_async_frame_store".to_string(),
            args: vec![handle, zero, final_state],
        });
        f.basic_blocks[body_block].push(store_final_state);

        f.basic_blocks[body_block].set_terminator(Terminator::Goto(done_block));

        // Done block: return 1 (ready)
        let ready = f.add_local(LocalKind::Temp, MIR_I64);
        let ready_inst = f.alloc_inst(Instruction::Assign {
            destination: ready,
            value: MirConstant::Int(1),
        });
        f.basic_blocks[done_block].push(ready_inst);
        f.basic_blocks[done_block].set_terminator(Terminator::Return(Some(ready)));
    } else {
        // Multi-await: state machine with switch on state
        let done_block = f.add_block();
        let mut state_blocks: Vec<usize> = Vec::new();
        for _ in 0..n_states {
            state_blocks.push(f.add_block());
        }

        // Switch on state
        let targets: Vec<(u32, usize)> = state_blocks
            .iter()
            .enumerate()
            .map(|(i, &bb)| (i as u32, bb))
            .collect();
        f.basic_blocks[bb0].set_terminator(Terminator::Switch {
            discr: state,
            targets,
            otherwise: done_block,
        });

        // Each state block: for now, calls original function and returns ready
        // (proper resume-point splitting would go here in a future iteration)
        for (_i, &sb) in state_blocks.iter().enumerate() {
            let result_val = f.add_local(LocalKind::Temp, MIR_I64);

            let mut param_locals = Vec::new();
            for pi in 0..layout.param_count {
                let off = f.add_local(LocalKind::Temp, MIR_I64);
                let off_inst = f.alloc_inst(Instruction::Assign {
                    destination: off,
                    value: MirConstant::Int((2 + pi) as i64),
                });
                f.basic_blocks[sb].push(off_inst);

                let p = f.add_local(LocalKind::Temp, MIR_I64);
                let load_p = f.alloc_inst(Instruction::Call {
                    destination: p,
                    func: "sengoo_async_frame_load".to_string(),
                    args: vec![handle, off],
                });
                f.basic_blocks[sb].push(load_p);
                param_locals.push(p);
            }

            let call_inst = f.alloc_inst(Instruction::Call {
                destination: result_val,
                func: layout.func_name.clone(),
                args: param_locals,
            });
            f.basic_blocks[sb].push(call_inst);

            // Store result
            let one = f.add_local(LocalKind::Temp, MIR_I64);
            let one_inst = f.alloc_inst(Instruction::Assign {
                destination: one,
                value: MirConstant::Int(1),
            });
            f.basic_blocks[sb].push(one_inst);

            let sres_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
            let store_res = f.alloc_inst(Instruction::Call {
                destination: sres_dest,
                func: "sengoo_async_frame_store".to_string(),
                args: vec![handle, one, result_val],
            });
            f.basic_blocks[sb].push(store_res);

            // Advance state
            let next = f.add_local(LocalKind::Temp, MIR_I64);
            let next_inst = f.alloc_inst(Instruction::Assign {
                destination: next,
                value: MirConstant::Int(n_states as i64),
            });
            f.basic_blocks[sb].push(next_inst);

            let snext_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
            let store_next = f.alloc_inst(Instruction::Call {
                destination: snext_dest,
                func: "sengoo_async_frame_store".to_string(),
                args: vec![handle, zero, next],
            });
            f.basic_blocks[sb].push(store_next);

            f.basic_blocks[sb].set_terminator(Terminator::Goto(done_block));
        }

        let ready = f.add_local(LocalKind::Temp, MIR_I64);
        let ready_inst = f.alloc_inst(Instruction::Assign {
            destination: ready,
            value: MirConstant::Int(1),
        });
        f.basic_blocks[done_block].push(ready_inst);
        f.basic_blocks[done_block].set_terminator(Terminator::Return(Some(ready)));
    }

    f
}

/// Generate `foo__result(handle: i64) -> T`
fn synthesize_result(layout: &AsyncFrameLayout) -> MirFunction {
    let name = format!("{}__result", layout.func_name);
    let ret = match &layout.return_type {
        MIRType::Unit => MIR_I64,
        other => other.clone(),
    };
    let mut f = MirFunction::new(name, vec![MIR_I64], ret);

    let handle = Local::new(1, LocalKind::Param);
    let bb0 = f.start_block;

    // Load result from frame[1]
    let one = f.add_local(LocalKind::Temp, MIR_I64);
    let one_inst = f.alloc_inst(Instruction::Assign {
        destination: one,
        value: MirConstant::Int(1),
    });
    f.basic_blocks[bb0].push(one_inst);

    let result = f.add_local(LocalKind::Temp, MIR_I64);
    let load_result = f.alloc_inst(Instruction::Call {
        destination: result,
        func: "sengoo_async_frame_load".to_string(),
        args: vec![handle, one],
    });
    f.basic_blocks[bb0].push(load_result);

    // Free the frame
    let free_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
    let free_inst = f.alloc_inst(Instruction::Call {
        destination: free_dest,
        func: "sengoo_async_frame_free".to_string(),
        args: vec![handle],
    });
    f.basic_blocks[bb0].push(free_inst);

    // Return result
    f.basic_blocks[bb0].set_terminator(Terminator::Return(Some(result)));

    f
}
