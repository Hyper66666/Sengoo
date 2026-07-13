use super::async_cfg_helpers::{
    collect_live_user_slots, compute_live_in_user_locals, AsyncCfgPlan, LiveUserSlot,
};
use super::async_frame_helpers::{
    frame_await_slot, frame_user_slot, push_frame_load_into, push_frame_load_into_or_value_typed,
    push_frame_load_typed, push_frame_store, push_frame_store_typed, push_i64_const,
    AsyncFrameLayout,
};
use crate::mir::{
    Instruction, Local, LocalKind, MIRType, MirConstant, MirFunction, Terminator, MIR_I64,
};
use crate::CompileError;
use std::collections::{HashMap, HashSet};

fn stamp_new_mir_with_source_site(
    function: &mut MirFunction,
    first_instruction: usize,
    terminated_before: &[bool],
    source_site: Option<u32>,
) {
    let Some(source_site) = source_site else {
        return;
    };
    for instruction_index in first_instruction..function.instructions.len() {
        if function.instruction_source_sites[instruction_index].is_none() {
            function.instruction_source_sites[instruction_index] = Some(source_site);
        }
    }
    for (block_index, block) in function.basic_blocks.iter_mut().enumerate() {
        if !terminated_before.get(block_index).copied().unwrap_or(false)
            && block.terminator.is_some()
            && block.terminator_source_site.is_none()
        {
            block.terminator_source_site = Some(source_site);
        }
    }
}

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
            op: *op,
            operand: remap_local(*operand, local_map)?,
        },
        Instruction::Binary {
            destination,
            op,
            left,
            right,
        } => Instruction::Binary {
            destination: remap_local(*destination, local_map)?,
            op: *op,
            left: remap_local(*left, local_map)?,
            right: remap_local(*right, local_map)?,
        },
        Instruction::Load {
            destination,
            source,
        } => Instruction::Load {
            destination: remap_local(*destination, local_map)?,
            source: remap_local(*source, local_map)?,
        },
        Instruction::Store { destination, value } => Instruction::Store {
            destination: remap_local(*destination, local_map)?,
            value: remap_local(*value, local_map)?,
        },
        Instruction::AddrOf {
            destination,
            source,
        } => Instruction::AddrOf {
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
        Instruction::CallIndirect {
            destination,
            func_ptr,
            args,
        } => Instruction::CallIndirect {
            destination: remap_local(*destination, local_map)?,
            func_ptr: remap_local(*func_ptr, local_map)?,
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
            destination: destination
                .map(|local| remap_local(local, local_map))
                .transpose()?,
            intrinsic: intrinsic.clone(),
            args: args
                .iter()
                .map(|local| remap_local(*local, local_map))
                .collect::<Result<Vec<_>, _>>()?,
        },
        Instruction::Discriminant {
            destination,
            source,
        } => Instruction::Discriminant {
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
            payload: payload
                .map(|local| remap_local(local, local_map))
                .transpose()?,
            enum_type: enum_type.clone(),
        },
        Instruction::ExtractPayload {
            destination,
            source,
        } => Instruction::ExtractPayload {
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

fn is_pointer_like_type(ty: &MIRType) -> bool {
    matches!(ty, MIRType::Ref(_) | MIRType::Ptr(_))
}

pub(crate) fn collect_rebasable_pointer_locals(
    mir_fn: &MirFunction,
    spill_user_locals: &[(Local, MIRType)],
) -> HashMap<Local, Local> {
    let spilled = spill_user_locals
        .iter()
        .map(|(local, _)| *local)
        .collect::<HashSet<_>>();
    let mut zero_locals = HashSet::new();
    let mut address_sources = HashMap::<Local, Local>::new();
    let mut pointer_store_sources = HashMap::<Local, Option<Local>>::new();

    for block in &mir_fn.basic_blocks {
        for inst_id in &block.instructions {
            match mir_fn.instruction(*inst_id) {
                Instruction::Assign {
                    destination,
                    value: MirConstant::Int(0),
                } => {
                    zero_locals.insert(*destination);
                }
                Instruction::IndexAddr {
                    destination,
                    base,
                    index,
                } if zero_locals.contains(index) && base.kind == LocalKind::User => {
                    address_sources.insert(*destination, *base);
                }
                Instruction::AddrOf {
                    destination,
                    source,
                } if source.kind == LocalKind::User => {
                    address_sources.insert(*destination, *source);
                }
                Instruction::Store { destination, value }
                    if destination.kind == LocalKind::User
                        && mir_fn
                            .locals
                            .get(destination.index())
                            .is_some_and(|(_, ty)| is_pointer_like_type(ty))
                        && spilled.contains(destination) =>
                {
                    let source = address_sources.get(value).copied();
                    match pointer_store_sources.get_mut(destination) {
                        Some(existing) if *existing == source => {}
                        Some(existing) => *existing = None,
                        None => {
                            pointer_store_sources.insert(*destination, source);
                        }
                    }
                }
                _ => {}
            }
        }
    }

    pointer_store_sources
        .into_iter()
        .filter_map(|(pointer, source)| source.map(|source| (pointer, source)))
        .collect()
}

fn add_rebase_source_slots(
    live_user_slots: &mut HashMap<usize, Vec<LiveUserSlot>>,
    spill_user_locals: &[(Local, MIRType)],
    rebase_pointer_locals: &HashMap<Local, Local>,
) {
    let slot_map = spill_user_locals
        .iter()
        .enumerate()
        .map(|(slot_index, (local, ty))| (*local, (slot_index, ty.clone())))
        .collect::<HashMap<_, _>>();

    for slots in live_user_slots.values_mut() {
        let mut present = slots.iter().map(|slot| slot.local).collect::<HashSet<_>>();
        let mut extra = Vec::new();
        for slot in slots.iter() {
            let Some(source) = rebase_pointer_locals.get(&slot.local).copied() else {
                continue;
            };
            if !present.insert(source) {
                continue;
            }
            if let Some((slot_index, ty)) = slot_map.get(&source) {
                extra.push(LiveUserSlot {
                    slot_index: *slot_index,
                    local: source,
                    ty: ty.clone(),
                });
            }
        }
        slots.extend(extra);
        slots.sort_by_key(|slot| slot.slot_index);
    }
}

#[allow(clippy::too_many_arguments)]
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
    rebase_pointer_locals: &HashMap<Local, Local>,
) -> Result<(), CompileError> {
    for slot in live_user_slots {
        if rebase_pointer_locals.contains_key(&slot.local) {
            continue;
        }
        let remapped = remap_local(slot.local, local_map)?;
        let value = if is_pointer_like_type(&slot.ty) {
            remapped
        } else {
            let loaded = f.add_local(LocalKind::Temp, slot.ty.clone());
            let load_inst = f.alloc_inst(Instruction::Load {
                destination: loaded,
                source: remapped,
            });
            f.basic_blocks[block].push(load_inst);
            loaded
        };
        push_frame_store_typed(
            f,
            block,
            handle,
            frame_user_slot(layout, slot.slot_index),
            value,
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

#[allow(clippy::too_many_arguments)]
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
    rebase_pointer_locals: &HashMap<Local, Local>,
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
        rebase_pointer_locals,
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
    let rebase_pointer_locals = collect_rebasable_pointer_locals(mir_fn, spill_user_locals);
    let mut live_user_slots = collect_live_user_slots(plan, spill_user_locals, &live_in);
    add_rebase_source_slots(
        &mut live_user_slots,
        spill_user_locals,
        &rebase_pointer_locals,
    );
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
    let mut initial_pending_blocks = HashMap::new();
    let mut resume_pending_blocks = HashMap::new();
    let mut ready_handle_blocks = HashMap::new();
    let mut ready_handle_locals = HashMap::new();
    let mut ready_handle_initial_values = HashMap::new();
    let mut ready_handle_resume_values = HashMap::new();
    for point in &plan.suspend_points {
        resume_blocks.insert(point.block, f.add_block());
        initial_pending_blocks.insert(point.block, f.add_block());
        resume_pending_blocks.insert(point.block, f.add_block());
        ready_handle_blocks.insert(point.block, f.add_block());
    }
    let completed_block = f.add_block();

    let state = f.add_local(LocalKind::Temp, MIR_I64);
    push_frame_load_into(&mut f, bb0, handle, 0, state);

    for i in 0..layout.param_types.len() {
        let original = Local::new(i + 1, LocalKind::Param);
        let remapped = remap_local(original, &local_map)?;
        let restored = push_frame_load_into_or_value_typed(
            &mut f,
            bb0,
            handle,
            layout.param_offsets[i],
            remapped,
            &layout.param_types[i],
        )?;
        local_map.insert(original, restored);
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

    let mut local_maps_by_block = HashMap::<usize, HashMap<Local, Local>>::new();
    for block in &plan.ordered_blocks {
        let mut block_local_map = local_map.clone();
        for point in plan
            .suspend_points
            .iter()
            .filter(|point| point.ready_block == *block)
        {
            let original_handle = point.future_handle;
            let Some((_, ty)) = mir_fn.locals.get(original_handle.index()) else {
                continue;
            };
            let phi_local = *ready_handle_locals
                .entry(point.block)
                .or_insert_with(|| f.add_local(LocalKind::Temp, ty.clone()));
            block_local_map.insert(original_handle, phi_local);
        }
        local_maps_by_block.insert(*block, block_local_map);
    }

    for block in &plan.ordered_blocks {
        let translated = translated_blocks[block];
        let original_block = &mir_fn.basic_blocks[*block];
        let block_local_map = local_maps_by_block.get(block).ok_or_else(|| {
            CompileError::MirLower(format!("missing local map for block {}", block))
        })?;
        for inst_id in &original_block.instructions {
            let cloned = remap_instruction(
                mir_fn.instruction(*inst_id),
                block_local_map,
                &translated_blocks,
            )?;
            let new_id = f.alloc_inst(cloned);
            let source_site = mir_fn
                .instruction_source_sites
                .get(inst_id.0 as usize)
                .copied()
                .flatten();
            f.set_instruction_source_site(new_id, source_site);
            if mir_fn.debug_hidden_instructions.contains(inst_id) {
                f.hide_instruction_from_debug(new_id);
            }
            f.basic_blocks[translated].push(new_id);
        }

        let first_generated_instruction = f.instructions.len();
        let terminated_before = f
            .basic_blocks
            .iter()
            .map(|block| block.terminator.is_some())
            .collect::<Vec<_>>();
        let terminator_source_site = original_block.terminator_source_site;

        match original_block
            .terminator
            .as_ref()
            .ok_or_else(|| CompileError::MirLower("async cfg block should terminate".to_string()))?
        {
            Terminator::Return(value) => {
                if let Some(value) = value {
                    let remapped = remap_local(*value, block_local_map)?;
                    push_frame_store_typed(
                        &mut f,
                        translated,
                        handle,
                        1,
                        remapped,
                        &layout.result_storage_ty,
                    )?;
                }
                let completed_state =
                    push_i64_const(&mut f, translated, (plan.suspend_points.len() + 1) as i64);
                push_frame_store(&mut f, translated, handle, 0, completed_state);
                emit_ready_return(&mut f, translated);
            }
            Terminator::Goto(target) => {
                f.basic_blocks[translated]
                    .set_terminator(Terminator::Goto(translated_blocks[target]));
            }
            Terminator::If {
                cond,
                then_block,
                else_block,
            } => {
                f.basic_blocks[translated].set_terminator(Terminator::If {
                    cond: remap_local(*cond, block_local_map)?,
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
                    discr: remap_local(*discr, block_local_map)?,
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
                let remapped_handle = remap_local(*future_handle, block_local_map)?;
                ready_handle_initial_values.insert(*block, remapped_handle);
                emit_suspend_transition(
                    &mut f,
                    translated,
                    layout,
                    handle,
                    poll_func,
                    remapped_handle,
                    ready_handle_blocks[block],
                    initial_pending_blocks[block],
                    point.state_index,
                    point.state_index - 1,
                    live_user_slots
                        .get(block)
                        .map(|slots| slots.as_slice())
                        .unwrap_or(&[]),
                    block_local_map,
                    &rebase_pointer_locals,
                )?;
            }
            Terminator::Unreachable => {
                f.basic_blocks[translated].set_terminator(Terminator::Unreachable);
            }
            other => {
                return Err(CompileError::MirLower(format!(
                    "unsupported terminator in async poll plan: {:?}",
                    other
                )));
            }
        }
        stamp_new_mir_with_source_site(
            &mut f,
            first_generated_instruction,
            &terminated_before,
            terminator_source_site,
        );
    }

    for point in &plan.suspend_points {
        let first_generated_instruction = f.instructions.len();
        let terminated_before = f
            .basic_blocks
            .iter()
            .map(|block| block.terminator.is_some())
            .collect::<Vec<_>>();
        let source_site = mir_fn.basic_blocks[point.block].terminator_source_site;
        let resume_block = resume_blocks[&point.block];
        for slot in live_user_slots
            .get(&point.block)
            .map(|slots| slots.as_slice())
            .unwrap_or(&[])
        {
            if rebase_pointer_locals.contains_key(&slot.local) {
                continue;
            }
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
        for slot in live_user_slots
            .get(&point.block)
            .map(|slots| slots.as_slice())
            .unwrap_or(&[])
        {
            let Some(source) = rebase_pointer_locals.get(&slot.local).copied() else {
                continue;
            };
            let remapped_pointer = remap_local(slot.local, &local_map)?;
            let remapped_source = remap_local(source, &local_map)?;
            let pointer_value = f.add_local(LocalKind::Temp, slot.ty.clone());
            let addr_inst = f.alloc_inst(Instruction::AddrOf {
                destination: pointer_value,
                source: remapped_source,
            });
            f.basic_blocks[resume_block].push(addr_inst);
            let store_inst = f.alloc_inst(Instruction::Store {
                destination: remapped_pointer,
                value: pointer_value,
            });
            f.basic_blocks[resume_block].push(store_inst);
        }

        let restored_handle = push_frame_load_typed(
            &mut f,
            resume_block,
            handle,
            frame_await_slot(layout, point.state_index - 1),
            MIR_I64,
        )?;
        ready_handle_resume_values.insert(point.block, restored_handle);
        let resume_local_map = local_maps_by_block.get(&point.block).ok_or_else(|| {
            CompileError::MirLower(format!(
                "missing resume local map for block {}",
                point.block
            ))
        })?;
        emit_suspend_transition(
            &mut f,
            resume_block,
            layout,
            handle,
            &point.poll_func,
            restored_handle,
            ready_handle_blocks[&point.block],
            resume_pending_blocks[&point.block],
            point.state_index,
            point.state_index - 1,
            live_user_slots
                .get(&point.block)
                .map(|slots| slots.as_slice())
                .unwrap_or(&[]),
            resume_local_map,
            &rebase_pointer_locals,
        )?;
        stamp_new_mir_with_source_site(
            &mut f,
            first_generated_instruction,
            &terminated_before,
            source_site,
        );
    }

    for point in &plan.suspend_points {
        let source_site = mir_fn.basic_blocks[point.block].terminator_source_site;
        let ready_handle_block = ready_handle_blocks[&point.block];
        let initial_handle = ready_handle_initial_values
            .get(&point.block)
            .copied()
            .ok_or_else(|| {
                CompileError::MirLower("missing initial ready handle value".to_string())
            })?;
        let restored_handle = ready_handle_resume_values
            .get(&point.block)
            .copied()
            .ok_or_else(|| {
                CompileError::MirLower("missing resumed ready handle value".to_string())
            })?;
        let phi_local = *ready_handle_locals
            .get(&point.block)
            .ok_or_else(|| CompileError::MirLower("missing ready handle phi local".to_string()))?;
        let phi_inst = f.alloc_inst(Instruction::Phi {
            destination: phi_local,
            incoming: vec![
                (initial_handle, translated_blocks[&point.block]),
                (restored_handle, resume_blocks[&point.block]),
            ],
        });
        f.set_instruction_source_site(phi_inst, source_site);
        f.basic_blocks[ready_handle_block]
            .instructions
            .insert(0, phi_inst);
        f.basic_blocks[ready_handle_block].set_terminator_at(
            Terminator::Goto(translated_blocks[&point.ready_block]),
            source_site,
        );
    }

    Ok(f)
}
