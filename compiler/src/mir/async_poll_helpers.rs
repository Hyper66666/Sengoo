use super::async_cfg_helpers::{collect_live_user_slots, compute_live_in_user_locals, AsyncCfgPlan, LiveUserSlot};
use super::async_frame_helpers::{
    frame_await_slot, frame_user_slot, push_frame_load_into, push_frame_load_into_typed,
    push_frame_load_typed, push_frame_store, push_frame_store_typed, push_i64_const,
    AsyncFrameLayout,
};
use crate::mir::{Instruction, Local, LocalKind, MIRType, MirFunction, Terminator, MIR_I64};
use crate::CompileError;
use std::collections::HashMap;

fn clone_local_kind(kind: LocalKind) -> LocalKind {
    match kind {
        LocalKind::Param => LocalKind::Temp,
        other => other,
    }
}

fn remap_local(local: Local, local_map: &HashMap<Local, Local>) -> Result<Local, CompileError> {
    local_map
        .get(&local)
        .copied()
        .ok_or_else(|| CompileError::MirLower(format!("missing remapped local for {:?}", local)))
}

fn remap_instruction(
    inst: &Instruction,
    local_map: &HashMap<Local, Local>,
    block_map: &HashMap<usize, usize>,
) -> Result<Instruction, CompileError> {
    Ok(match inst {
        Instruction::Assign { destination, value } => Instruction::Assign {
            destination: remap_local(*destination, local_map)?,
            value: value.clone(),
        },
        Instruction::Unary {
            destination,
            op,
            operand,
        } => Instruction::Unary {
            destination: remap_local(*destination, local_map)?,
            op: op.clone(),
            operand: remap_local(*operand, local_map)?,
        },
        Instruction::Binary {
            destination,
            op,
            left,
            right,
        } => Instruction::Binary {
            destination: remap_local(*destination, local_map)?,
            op: op.clone(),
            left: remap_local(*left, local_map)?,
            right: remap_local(*right, local_map)?,
        },
        Instruction::Load { destination, source } => Instruction::Load {
            destination: remap_local(*destination, local_map)?,
            source: remap_local(*source, local_map)?,
        },
        Instruction::Store { destination, value } => Instruction::Store {
            destination: remap_local(*destination, local_map)?,
            value: remap_local(*value, local_map)?,
        },
        Instruction::AddrOf { destination, source } => Instruction::AddrOf {
            destination: remap_local(*destination, local_map)?,
            source: remap_local(*source, local_map)?,
        },
        Instruction::FieldAddr {
            destination,
            base,
            field,
        } => Instruction::FieldAddr {
            destination: remap_local(*destination, local_map)?,
            base: remap_local(*base, local_map)?,
            field: *field,
        },
        Instruction::IndexAddr {
            destination,
            base,
            index,
        } => Instruction::IndexAddr {
            destination: remap_local(*destination, local_map)?,
            base: remap_local(*base, local_map)?,
            index: remap_local(*index, local_map)?,
        },
        Instruction::Extract {
            destination,
            value,
            index,
        } => Instruction::Extract {
            destination: remap_local(*destination, local_map)?,
            value: remap_local(*value, local_map)?,
            index: *index,
        },
        Instruction::Insert {
            destination,
            value,
            field,
            new_value,
        } => Instruction::Insert {
            destination: remap_local(*destination, local_map)?,
            value: remap_local(*value, local_map)?,
            field: *field,
            new_value: remap_local(*new_value, local_map)?,
        },
        Instruction::Cast {
            destination,
            value,
            to,
        } => Instruction::Cast {
            destination: remap_local(*destination, local_map)?,
            value: remap_local(*value, local_map)?,
            to: to.clone(),
        },
        Instruction::Bitcast {
            destination,
            value,
            to,
        } => Instruction::Bitcast {
            destination: remap_local(*destination, local_map)?,
            value: remap_local(*value, local_map)?,
            to: to.clone(),
        },
        Instruction::Aggregate {
            destination,
            fields,
            ty,
        } => Instruction::Aggregate {
            destination: remap_local(*destination, local_map)?,
            fields: fields
                .iter()
                .map(|local| remap_local(*local, local_map))
                .collect::<Result<Vec<_>, _>>()?,
            ty: ty.clone(),
        },
        Instruction::Call {
            destination,
            func,
            args,
        } => Instruction::Call {
            destination: remap_local(*destination, local_map)?,
            func: func.clone(),
            args: args
                .iter()
                .map(|local| remap_local(*local, local_map))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Instruction::Intrinsic {
            destination,
            intrinsic,
            args,
        } => Instruction::Intrinsic {
            destination: destination.map(|local| remap_local(local, local_map)).transpose()?,
            intrinsic: intrinsic.clone(),
            args: args
                .iter()
                .map(|local| remap_local(*local, local_map))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Instruction::Discriminant { destination, source } => Instruction::Discriminant {
            destination: remap_local(*destination, local_map)?,
            source: remap_local(*source, local_map)?,
        },
        Instruction::EnumConstruct {
            destination,
            discriminant,
            payload,
            enum_type,
        } => Instruction::EnumConstruct {
            destination: remap_local(*destination, local_map)?,
            discriminant: *discriminant,
            payload: payload.map(|local| remap_local(local, local_map)).transpose()?,
            enum_type: enum_type.clone(),
        },
        Instruction::ExtractPayload { destination, source } => Instruction::ExtractPayload {
            destination: remap_local(*destination, local_map)?,
            source: remap_local(*source, local_map)?,
        },
        Instruction::Phi {
            destination,
            incoming,
        } => Instruction::Phi {
            destination: remap_local(*destination, local_map)?,
            incoming: incoming
                .iter()
                .map(|(local, block)| -> Result<(Local, usize), CompileError> {
                    let remapped_block = block_map.get(block).copied().ok_or_else(|| {
                        CompileError::MirLower(format!("missing remapped block for {}", block))
                    })?;
                    Ok((remap_local(*local, local_map)?, remapped_block))
                })
                .collect::<Result<Vec<_>, _>>()?,
        },
        Instruction::Nop => Instruction::Nop,
    })
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
    live_user_slots: &[LiveUserSlot],
    local_map: &HashMap<Local, Local>,
) -> Result<(), CompileError> {
    for slot in live_user_slots {
        let remapped = remap_local(slot.local, local_map)?;
        let loaded = f.add_local(LocalKind::Temp, slot.ty.clone());
        let load_inst = f.alloc_inst(Instruction::Load {
            destination: loaded,
            source: remapped,
        });
        f.basic_blocks[block].push(load_inst);
        push_frame_store_typed(
            f,
            block,
            handle,
            frame_user_slot(layout, slot.slot_index),
            loaded,
            &slot.ty,
        )?;
    }

    push_frame_store_typed(
        f,
        block,
        handle,
        frame_await_slot(layout, await_slot_index),
        future_handle,
        &MIR_I64,
    )?;
    let next_state = push_i64_const(f, block, state_index as i64);
    push_frame_store(f, block, handle, 0, next_state);

    let pending = push_i64_const(f, block, 0);
    f.basic_blocks[block].set_terminator(Terminator::Return(Some(pending)));
    Ok(())
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
    live_user_slots: &[LiveUserSlot],
    local_map: &HashMap<Local, Local>,
) -> Result<(), CompileError> {
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
        live_user_slots,
        local_map,
    )?;
    Ok(())
}

pub(crate) fn synthesize_cfg_poll(
    layout: &AsyncFrameLayout,
    mir_fn: &MirFunction,
    plan: &AsyncCfgPlan,
    spill_user_locals: &[(Local, MIRType)],
) -> Result<MirFunction, CompileError> {
    let name = format!("{}__poll", layout.func_name);
    let mut f = MirFunction::new(name, vec![MIR_I64], MIR_I64);
    let handle = Local::new(1, LocalKind::Param);
    let bb0 = f.start_block;

    let live_in = compute_live_in_user_locals(mir_fn, plan)?;
    let live_user_slots = collect_live_user_slots(plan, spill_user_locals, &live_in);
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

    for i in 0..layout.param_types.len() {
        let original = Local::new(i + 1, LocalKind::Param);
        let remapped = remap_local(original, &local_map)?;
        push_frame_load_into_typed(
            &mut f,
            bb0,
            handle,
            layout.param_offsets[i],
            remapped,
            &layout.param_types[i],
        )?;
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
            let cloned = remap_instruction(mir_fn.instruction(*inst_id), &local_map, &translated_blocks)?;
            let new_id = f.alloc_inst(cloned);
            f.basic_blocks[translated].push(new_id);
        }

        match original_block
            .terminator
            .as_ref()
            .ok_or_else(|| CompileError::MirLower("async cfg block should terminate".to_string()))?
        {
            Terminator::Return(value) => {
                if let Some(value) = value {
                    let remapped = remap_local(*value, &local_map)?;
                    push_frame_store_typed(
                        &mut f,
                        translated,
                        handle,
                        1,
                        remapped,
                        &layout.result_storage_ty,
                    )?;
                }
                let completed_state = push_i64_const(
                    &mut f,
                    translated,
                    (plan.suspend_points.len() + 1) as i64,
                );
                push_frame_store(&mut f, translated, handle, 0, completed_state);
                emit_ready_return(&mut f, translated);
            }
            Terminator::Goto(target) => {
                f.basic_blocks[translated].set_terminator(Terminator::Goto(translated_blocks[target]));
            }
            Terminator::If {
                cond,
                then_block,
                else_block,
            } => {
                f.basic_blocks[translated].set_terminator(Terminator::If {
                    cond: remap_local(*cond, &local_map)?,
                    then_block: translated_blocks[then_block],
                    else_block: translated_blocks[else_block],
                });
            }
            Terminator::Switch {
                discr,
                targets,
                otherwise,
            } => {
                f.basic_blocks[translated].set_terminator(Terminator::Switch {
                    discr: remap_local(*discr, &local_map)?,
                    targets: targets
                        .iter()
                        .map(|(value, block)| (*value, translated_blocks[block]))
                        .collect(),
                    otherwise: translated_blocks[otherwise],
                });
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
                    .ok_or_else(|| {
                        CompileError::MirLower(format!(
                            "missing suspend metadata for async block {}",
                            block
                        ))
                    })?;
                let remapped_handle = remap_local(*future_handle, &local_map)?;
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
                    live_user_slots.get(block).map(|slots| slots.as_slice()).unwrap_or(&[]),
                    &local_map,
                )?;
            }
            other => {
                return Err(CompileError::MirLower(format!(
                    "unsupported terminator in async poll plan: {:?}",
                    other
                )));
            }
        }
    }

    for point in &plan.suspend_points {
        let resume_block = resume_blocks[&point.block];
        for slot in live_user_slots
            .get(&point.block)
            .map(|slots| slots.as_slice())
            .unwrap_or(&[])
        {
            let restored = push_frame_load_typed(
                &mut f,
                resume_block,
                handle,
                frame_user_slot(layout, slot.slot_index),
                slot.ty.clone(),
            )?;
            let remapped_user = remap_local(slot.local, &local_map)?;
            let store_inst = f.alloc_inst(Instruction::Store {
                destination: remapped_user,
                value: restored,
            });
            f.basic_blocks[resume_block].push(store_inst);
        }

        let remapped_handle = remap_local(point.future_handle, &local_map)?;
        push_frame_load_into_typed(
            &mut f,
            resume_block,
            handle,
            frame_await_slot(layout, point.state_index - 1),
            remapped_handle,
            &MIR_I64,
        )?;
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
            live_user_slots
                .get(&point.block)
                .map(|slots| slots.as_slice())
                .unwrap_or(&[]),
            &local_map,
        )?;
    }

    Ok(f)
}
