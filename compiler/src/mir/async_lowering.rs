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
use std::collections::{HashMap, HashSet};

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
    pub user_local_count: usize,
}

impl AsyncFrameLayout {
    pub fn total_slots(&self) -> usize {
        2 + self.param_count + self.user_local_count + self.await_count
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
            user_local_count: collect_user_locals(mir_fn).len(),
        };

        let mut start = synthesize_start(&layout);
        let mut poll = synthesize_poll(&layout, mir_fn);
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

#[derive(Debug, Clone)]
struct LinearSuspendPoint {
    state_index: usize,
    block: usize,
    poll_func: String,
    future_handle: Local,
    ready_block: usize,
}

#[derive(Debug, Clone)]
struct LinearAsyncPlan {
    ordered_blocks: Vec<usize>,
    suspend_points: Vec<LinearSuspendPoint>,
}

fn collect_user_locals(mir_fn: &MirFunction) -> Vec<(Local, MIRType)> {
    mir_fn
        .locals
        .iter()
        .filter(|(local, _)| matches!(local.kind, LocalKind::User))
        .map(|(local, ty)| (*local, ty.clone()))
        .collect()
}

fn build_linear_async_plan(mir_fn: &MirFunction) -> Option<LinearAsyncPlan> {
    let mut ordered_blocks = Vec::new();
    let mut suspend_points = Vec::new();
    let mut current = mir_fn.start_block;
    let mut seen = HashSet::new();

    loop {
        if !seen.insert(current) {
            return None;
        }
        ordered_blocks.push(current);

        let terminator = mir_fn.basic_blocks.get(current)?.terminator.clone()?;
        match terminator {
            Terminator::Suspend {
                poll_func,
                future_handle,
                ready_block,
                pending_block,
                ..
            } => {
                match mir_fn.basic_blocks.get(pending_block)?.terminator.as_ref() {
                    Some(Terminator::Goto(target)) if *target == pending_block => {}
                    _ => return None,
                }
                suspend_points.push(LinearSuspendPoint {
                    state_index: suspend_points.len() + 1,
                    block: current,
                    poll_func,
                    future_handle,
                    ready_block,
                });
                current = ready_block;
            }
            Terminator::Return(_) => break,
            _ => return None,
        }
    }

    Some(LinearAsyncPlan {
        ordered_blocks,
        suspend_points,
    })
}

fn frame_user_slot(layout: &AsyncFrameLayout, index: usize) -> i64 {
    (2 + layout.param_count + index) as i64
}

fn frame_await_slot(layout: &AsyncFrameLayout, index: usize) -> i64 {
    (2 + layout.param_count + layout.user_local_count + index) as i64
}

fn push_i64_const(f: &mut MirFunction, block: usize, value: i64) -> Local {
    let local = f.add_local(LocalKind::Temp, MIR_I64);
    let inst = f.alloc_inst(Instruction::Assign {
        destination: local,
        value: MirConstant::Int(value),
    });
    f.basic_blocks[block].push(inst);
    local
}

fn push_frame_store(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    value: Local,
) {
    let offset_local = push_i64_const(f, block, offset);
    let dest = f.add_local(LocalKind::Temp, MIR_UNIT);
    let inst = f.alloc_inst(Instruction::Call {
        destination: dest,
        func: "sengoo_async_frame_store".to_string(),
        args: vec![handle, offset_local, value],
    });
    f.basic_blocks[block].push(inst);
}

fn push_frame_load_into(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    destination: Local,
) {
    let offset_local = push_i64_const(f, block, offset);
    let inst = f.alloc_inst(Instruction::Call {
        destination,
        func: "sengoo_async_frame_load".to_string(),
        args: vec![handle, offset_local],
    });
    f.basic_blocks[block].push(inst);
}

fn push_frame_load(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    ty: MIRType,
) -> Local {
    let destination = f.add_local(LocalKind::Temp, ty);
    push_frame_load_into(f, block, handle, offset, destination);
    destination
}

fn clone_local_kind(kind: LocalKind) -> LocalKind {
    match kind {
        LocalKind::Param => LocalKind::Temp,
        other => other,
    }
}

fn remap_local(local: Local, local_map: &HashMap<Local, Local>) -> Local {
    *local_map
        .get(&local)
        .unwrap_or_else(|| panic!("missing remapped local for {:?}", local))
}

fn remap_instruction(inst: &Instruction, local_map: &HashMap<Local, Local>) -> Instruction {
    match inst {
        Instruction::Assign { destination, value } => Instruction::Assign {
            destination: remap_local(*destination, local_map),
            value: value.clone(),
        },
        Instruction::Unary {
            destination,
            op,
            operand,
        } => Instruction::Unary {
            destination: remap_local(*destination, local_map),
            op: op.clone(),
            operand: remap_local(*operand, local_map),
        },
        Instruction::Binary {
            destination,
            op,
            left,
            right,
        } => Instruction::Binary {
            destination: remap_local(*destination, local_map),
            op: op.clone(),
            left: remap_local(*left, local_map),
            right: remap_local(*right, local_map),
        },
        Instruction::Load { destination, source } => Instruction::Load {
            destination: remap_local(*destination, local_map),
            source: remap_local(*source, local_map),
        },
        Instruction::Store { destination, value } => Instruction::Store {
            destination: remap_local(*destination, local_map),
            value: remap_local(*value, local_map),
        },
        Instruction::AddrOf { destination, source } => Instruction::AddrOf {
            destination: remap_local(*destination, local_map),
            source: remap_local(*source, local_map),
        },
        Instruction::FieldAddr {
            destination,
            base,
            field,
        } => Instruction::FieldAddr {
            destination: remap_local(*destination, local_map),
            base: remap_local(*base, local_map),
            field: *field,
        },
        Instruction::IndexAddr {
            destination,
            base,
            index,
        } => Instruction::IndexAddr {
            destination: remap_local(*destination, local_map),
            base: remap_local(*base, local_map),
            index: remap_local(*index, local_map),
        },
        Instruction::Extract {
            destination,
            value,
            index,
        } => Instruction::Extract {
            destination: remap_local(*destination, local_map),
            value: remap_local(*value, local_map),
            index: *index,
        },
        Instruction::Insert {
            destination,
            value,
            field,
            new_value,
        } => Instruction::Insert {
            destination: remap_local(*destination, local_map),
            value: remap_local(*value, local_map),
            field: *field,
            new_value: remap_local(*new_value, local_map),
        },
        Instruction::Cast {
            destination,
            value,
            to,
        } => Instruction::Cast {
            destination: remap_local(*destination, local_map),
            value: remap_local(*value, local_map),
            to: to.clone(),
        },
        Instruction::Aggregate {
            destination,
            fields,
            ty,
        } => Instruction::Aggregate {
            destination: remap_local(*destination, local_map),
            fields: fields
                .iter()
                .map(|local| remap_local(*local, local_map))
                .collect(),
            ty: ty.clone(),
        },
        Instruction::Call {
            destination,
            func,
            args,
        } => Instruction::Call {
            destination: remap_local(*destination, local_map),
            func: func.clone(),
            args: args
                .iter()
                .map(|local| remap_local(*local, local_map))
                .collect(),
        },
        Instruction::Intrinsic {
            destination,
            intrinsic,
            args,
        } => Instruction::Intrinsic {
            destination: destination.map(|local| remap_local(local, local_map)),
            intrinsic: intrinsic.clone(),
            args: args
                .iter()
                .map(|local| remap_local(*local, local_map))
                .collect(),
        },
        Instruction::Discriminant { destination, source } => Instruction::Discriminant {
            destination: remap_local(*destination, local_map),
            source: remap_local(*source, local_map),
        },
        Instruction::EnumConstruct {
            destination,
            discriminant,
            payload,
            enum_type,
        } => Instruction::EnumConstruct {
            destination: remap_local(*destination, local_map),
            discriminant: *discriminant,
            payload: payload.map(|local| remap_local(local, local_map)),
            enum_type: enum_type.clone(),
        },
        Instruction::ExtractPayload { destination, source } => Instruction::ExtractPayload {
            destination: remap_local(*destination, local_map),
            source: remap_local(*source, local_map),
        },
        Instruction::Phi {
            destination,
            incoming,
        } => Instruction::Phi {
            destination: remap_local(*destination, local_map),
            incoming: incoming
                .iter()
                .map(|(local, block)| (remap_local(*local, local_map), *block))
                .collect(),
        },
        Instruction::Nop => Instruction::Nop,
    }
}

fn emit_ready_return(f: &mut MirFunction, block: usize) {
    let ready = push_i64_const(f, block, 1);
    f.basic_blocks[block].set_terminator(Terminator::Return(Some(ready)));
}

fn emit_pending_return(
    f: &mut MirFunction,
    block: usize,
    layout: &AsyncFrameLayout,
    handle: Local,
    state_index: usize,
    future_handle: Local,
    await_slot_index: usize,
    user_locals: &[(Local, MIRType)],
    local_map: &HashMap<Local, Local>,
) {
    for (user_idx, (user_local, _)) in user_locals.iter().enumerate() {
        let remapped = remap_local(*user_local, local_map);
        let loaded = f.add_local(LocalKind::Temp, MIR_I64);
        let load_inst = f.alloc_inst(Instruction::Load {
            destination: loaded,
            source: remapped,
        });
        f.basic_blocks[block].push(load_inst);
        push_frame_store(f, block, handle, frame_user_slot(layout, user_idx), loaded);
    }

    push_frame_store(
        f,
        block,
        handle,
        frame_await_slot(layout, await_slot_index),
        future_handle,
    );
    let next_state = push_i64_const(f, block, state_index as i64);
    push_frame_store(f, block, handle, 0, next_state);

    let pending = push_i64_const(f, block, 0);
    f.basic_blocks[block].set_terminator(Terminator::Return(Some(pending)));
}

fn emit_suspend_transition(
    f: &mut MirFunction,
    block: usize,
    layout: &AsyncFrameLayout,
    handle: Local,
    poll_func: &str,
    future_handle: Local,
    ready_block: usize,
    pending_block: usize,
    state_index: usize,
    await_slot_index: usize,
    user_locals: &[(Local, MIRType)],
    local_map: &HashMap<Local, Local>,
) {
    let poll_result = f.add_local(LocalKind::Temp, MIR_I64);
    let poll_call = f.alloc_inst(Instruction::Call {
        destination: poll_result,
        func: poll_func.to_string(),
        args: vec![future_handle],
    });
    f.basic_blocks[block].push(poll_call);
    f.basic_blocks[block].set_terminator(Terminator::Switch {
        discr: poll_result,
        targets: vec![(1, ready_block)],
        otherwise: pending_block,
    });

    emit_pending_return(
        f,
        pending_block,
        layout,
        handle,
        state_index,
        future_handle,
        await_slot_index,
        user_locals,
        local_map,
    );
}

fn synthesize_linear_poll(
    layout: &AsyncFrameLayout,
    mir_fn: &MirFunction,
    plan: &LinearAsyncPlan,
) -> MirFunction {
    let name = format!("{}__poll", layout.func_name);
    let mut f = MirFunction::new(name, vec![MIR_I64], MIR_I64);
    let handle = Local::new(1, LocalKind::Param);
    let bb0 = f.start_block;

    let user_locals = collect_user_locals(mir_fn);
    let mut local_map = HashMap::new();
    for (local, ty) in mir_fn.locals.iter().skip(1) {
        let remapped = f.add_local(clone_local_kind(local.kind), ty.clone());
        local_map.insert(*local, remapped);
    }

    let mut translated_blocks = HashMap::new();
    for block in &plan.ordered_blocks {
        translated_blocks.insert(*block, f.add_block());
    }
    let mut resume_blocks = HashMap::new();
    let mut pending_blocks = HashMap::new();
    for point in &plan.suspend_points {
        resume_blocks.insert(point.block, f.add_block());
        pending_blocks.insert(point.block, f.add_block());
    }
    let completed_block = f.add_block();

    let state = f.add_local(LocalKind::Temp, MIR_I64);
    push_frame_load_into(&mut f, bb0, handle, 0, state);

    for i in 0..layout.param_count {
        let original = Local::new(i + 1, LocalKind::Param);
        let remapped = remap_local(original, &local_map);
        push_frame_load_into(&mut f, bb0, handle, (2 + i) as i64, remapped);
    }

    let mut targets = vec![(0, translated_blocks[&mir_fn.start_block])];
    for point in &plan.suspend_points {
        targets.push((point.state_index as u32, resume_blocks[&point.block]));
    }
    f.basic_blocks[bb0].set_terminator(Terminator::Switch {
        discr: state,
        targets,
        otherwise: completed_block,
    });
    emit_ready_return(&mut f, completed_block);

    for block in &plan.ordered_blocks {
        let translated = translated_blocks[block];
        let original_block = &mir_fn.basic_blocks[*block];
        for inst_id in &original_block.instructions {
            let cloned = remap_instruction(mir_fn.instruction(*inst_id), &local_map);
            let new_id = f.alloc_inst(cloned);
            f.basic_blocks[translated].push(new_id);
        }

        match original_block.terminator.as_ref().expect("linear block should terminate") {
            Terminator::Return(value) => {
                if let Some(value) = value {
                    let remapped = remap_local(*value, &local_map);
                    push_frame_store(&mut f, translated, handle, 1, remapped);
                }
                let completed_state = push_i64_const(&mut f, translated, (plan.suspend_points.len() + 1) as i64);
                push_frame_store(&mut f, translated, handle, 0, completed_state);
                emit_ready_return(&mut f, translated);
            }
            Terminator::Suspend {
                poll_func,
                future_handle,
                ready_block,
                ..
            } => {
                let point = plan
                    .suspend_points
                    .iter()
                    .find(|point| point.block == *block)
                    .expect("suspend point should exist for linear block");
                let remapped_handle = remap_local(*future_handle, &local_map);
                emit_suspend_transition(
                    &mut f,
                    translated,
                    layout,
                    handle,
                    poll_func,
                    remapped_handle,
                    translated_blocks[ready_block],
                    pending_blocks[block],
                    point.state_index,
                    point.state_index - 1,
                    &user_locals,
                    &local_map,
                );
            }
            other => panic!("unsupported terminator in linear async plan: {:?}", other),
        }
    }

    for point in &plan.suspend_points {
        let resume_block = resume_blocks[&point.block];
        for (user_idx, (user_local, _)) in user_locals.iter().enumerate() {
            let restored = push_frame_load(&mut f, resume_block, handle, frame_user_slot(layout, user_idx), MIR_I64);
            let remapped_user = remap_local(*user_local, &local_map);
            let store_inst = f.alloc_inst(Instruction::Store {
                destination: remapped_user,
                value: restored,
            });
            f.basic_blocks[resume_block].push(store_inst);
        }

        let remapped_handle = remap_local(point.future_handle, &local_map);
        push_frame_load_into(
            &mut f,
            resume_block,
            handle,
            frame_await_slot(layout, point.state_index - 1),
            remapped_handle,
        );
        emit_suspend_transition(
            &mut f,
            resume_block,
            layout,
            handle,
            &point.poll_func,
            remapped_handle,
            translated_blocks[&point.ready_block],
            pending_blocks[&point.block],
            point.state_index,
            point.state_index - 1,
            &user_locals,
            &local_map,
        );
    }

    f
}

/// Generate `foo__poll(handle: i64) -> i64`
/// Returns 0 = pending, 1 = ready
fn synthesize_poll(layout: &AsyncFrameLayout, mir_fn: &MirFunction) -> MirFunction {
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

        let call_original = f.alloc_inst(Instruction::Call {
            destination: result_val,
            func: layout.func_name.clone(),
            args: param_locals,
        });
        f.basic_blocks[body_block].push(call_original);

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

        let ready = f.add_local(LocalKind::Temp, MIR_I64);
        let ready_inst = f.alloc_inst(Instruction::Assign {
            destination: ready,
            value: MirConstant::Int(1),
        });
        f.basic_blocks[done_block].push(ready_inst);
        f.basic_blocks[done_block].set_terminator(Terminator::Return(Some(ready)));
        return f;
    }

    if let Some(plan) = build_linear_async_plan(mir_fn) {
        return synthesize_linear_poll(layout, mir_fn, &plan);
    }

    // Fallback for complex async bodies not yet supported by the frame-backed state machine.
    let done_block = f.add_block();
    let mut state_blocks: Vec<usize> = Vec::new();
    for _ in 0..n_states {
        state_blocks.push(f.add_block());
    }

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

    for &sb in &state_blocks {
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
