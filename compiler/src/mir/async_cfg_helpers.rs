use crate::mir::{CallArg, Instruction, Local, LocalKind, MIRType, MirFunction, Terminator};
use crate::CompileError;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone)]
pub(crate) struct PlannedSuspendPoint {
    pub(crate) state_index: usize,
    pub(crate) block: usize,
    pub(crate) poll_func: String,
    #[allow(dead_code)]
    pub(crate) future_handle: Local,
    pub(crate) ready_block: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct AsyncCfgPlan {
    pub(crate) ordered_blocks: Vec<usize>,
    pub(crate) suspend_points: Vec<PlannedSuspendPoint>,
}

#[derive(Debug, Clone)]
pub(crate) struct AsyncCfgPlanError {
    message: String,
}

impl AsyncCfgPlanError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub(crate) fn describe(&self) -> &str {
        &self.message
    }
}

#[derive(Debug, Clone)]
pub(crate) struct LiveUserSlot {
    pub(crate) slot_index: usize,
    pub(crate) local: Local,
    pub(crate) ty: MIRType,
}

fn async_cfg_terminator_name(terminator: &Terminator) -> &'static str {
    match terminator {
        Terminator::Return(_) => "return",
        Terminator::Goto(_) => "goto",
        Terminator::If { .. } => "if",
        Terminator::Switch { .. } => "switch",
        Terminator::Call { .. } => "call",
        Terminator::Break { .. } => "break",
        Terminator::Continue { .. } => "continue",
        Terminator::Unreachable => "unreachable",
        Terminator::Suspend { .. } => "suspend",
    }
}

pub(crate) fn collect_user_locals(mir_fn: &MirFunction) -> Vec<(Local, MIRType)> {
    mir_fn
        .locals
        .iter()
        .filter(|(local, _)| matches!(local.kind, LocalKind::User))
        .map(|(local, ty)| (*local, ty.clone()))
        .collect()
}

pub(crate) fn compute_live_in_user_locals(
    mir_fn: &MirFunction,
    plan: &AsyncCfgPlan,
) -> Result<HashMap<usize, HashSet<Local>>, CompileError> {
    let mut live_in = HashMap::<usize, HashSet<Local>>::new();
    for block in &plan.ordered_blocks {
        live_in.insert(*block, HashSet::new());
    }

    let mut changed = true;
    while changed {
        changed = false;

        for block in plan.ordered_blocks.iter().rev() {
            let basic_block = &mir_fn.basic_blocks[*block];
            let terminator = basic_block.terminator.as_ref().ok_or_else(|| {
                CompileError::MirLower(format!(
                    "async cfg liveness requires every planned block to terminate; block {} is missing a terminator",
                    block
                ))
            })?;

            let mut live = match terminator {
                Terminator::Suspend { ready_block, .. } => {
                    live_in.get(ready_block).cloned().unwrap_or_default()
                }
                Terminator::Goto(target) => live_in.get(target).cloned().unwrap_or_default(),
                Terminator::If {
                    then_block,
                    else_block,
                    ..
                } => {
                    let mut live = live_in.get(then_block).cloned().unwrap_or_default();
                    live.extend(live_in.get(else_block).cloned().unwrap_or_default());
                    live
                }
                Terminator::Switch {
                    targets, otherwise, ..
                } => {
                    let mut live = live_in.get(otherwise).cloned().unwrap_or_default();
                    for (_, target) in targets {
                        live.extend(live_in.get(target).cloned().unwrap_or_default());
                    }
                    live
                }
                Terminator::Return(_) | Terminator::Unreachable => HashSet::new(),
                other => {
                    return Err(CompileError::MirLower(format!(
                        "unsupported terminator in async liveness: {:?}",
                        other
                    )))
                }
            };

            for local in terminator_user_defs(terminator) {
                live.remove(&local);
            }
            live.extend(terminator_user_uses(terminator));

            for inst_id in basic_block.instructions.iter().rev() {
                let inst = mir_fn.instruction(*inst_id);
                for local in instruction_user_defs(inst) {
                    live.remove(&local);
                }
                live.extend(instruction_user_uses(inst));
            }

            let entry = live_in.entry(*block).or_default();
            if *entry != live {
                *entry = live;
                changed = true;
            }
        }
    }

    Ok(live_in)
}

pub(crate) fn collect_spill_user_locals(
    plan: &AsyncCfgPlan,
    user_locals: &[(Local, MIRType)],
    live_in: &HashMap<usize, HashSet<Local>>,
) -> Vec<(Local, MIRType)> {
    let mut spilled = HashSet::new();
    for point in &plan.suspend_points {
        spilled.extend(live_in.get(&point.ready_block).cloned().unwrap_or_default());
    }

    user_locals
        .iter()
        .filter(|(local, _)| spilled.contains(local))
        .cloned()
        .collect()
}

pub(crate) fn build_async_cfg_plan(
    mir_fn: &MirFunction,
) -> Result<AsyncCfgPlan, AsyncCfgPlanError> {
    let mut ordered_blocks = Vec::new();
    let mut suspend_points = Vec::new();
    let mut visited = HashSet::<usize>::new();

    fn visit_async_block(
        mir_fn: &MirFunction,
        block: usize,
        visited: &mut HashSet<usize>,
        ordered_blocks: &mut Vec<usize>,
        suspend_points: &mut Vec<PlannedSuspendPoint>,
    ) -> Result<(), AsyncCfgPlanError> {
        if !visited.insert(block) {
            return Ok(());
        }

        let basic_block = mir_fn.basic_blocks.get(block).ok_or_else(|| {
            AsyncCfgPlanError::new(format!("block {} is missing from the async CFG", block))
        })?;
        let terminator = basic_block.terminator.clone().ok_or_else(|| {
            AsyncCfgPlanError::new(format!(
                "block {} has no terminator; expected goto/if/switch/return or suspend with a self-looping pending block",
                block
            ))
        })?;
        match terminator {
            Terminator::Suspend {
                poll_func,
                future_handle,
                ready_block,
                pending_block,
                ..
            } => {
                match mir_fn
                    .basic_blocks
                    .get(pending_block)
                    .ok_or_else(|| {
                        AsyncCfgPlanError::new(format!(
                            "suspend block {} references missing pending block {}",
                            block, pending_block
                        ))
                    })?
                    .terminator
                    .as_ref()
                {
                    Some(Terminator::Goto(target)) if *target == pending_block => {}
                    Some(other) => {
                        return Err(AsyncCfgPlanError::new(format!(
                            "suspend block {} expects pending block {} to self-loop with `goto`, but found `{}`",
                            block,
                            pending_block,
                            async_cfg_terminator_name(other)
                        )))
                    }
                    None => {
                        return Err(AsyncCfgPlanError::new(format!(
                            "suspend block {} expects pending block {} to self-loop with `goto`, but the pending block has no terminator",
                            block,
                            pending_block
                        )))
                    }
                }
                suspend_points.push(PlannedSuspendPoint {
                    state_index: suspend_points.len() + 1,
                    block,
                    poll_func,
                    future_handle,
                    ready_block,
                });
                visit_async_block(
                    mir_fn,
                    ready_block,
                    visited,
                    ordered_blocks,
                    suspend_points,
                )?;
            }
            Terminator::Goto(target) => {
                visit_async_block(mir_fn, target, visited, ordered_blocks, suspend_points)?;
            }
            Terminator::If {
                then_block,
                else_block,
                ..
            } => {
                visit_async_block(
                    mir_fn,
                    then_block,
                    visited,
                    ordered_blocks,
                    suspend_points,
                )?;
                visit_async_block(
                    mir_fn,
                    else_block,
                    visited,
                    ordered_blocks,
                    suspend_points,
                )?;
            }
            Terminator::Switch {
                targets,
                otherwise,
                ..
            } => {
                for (_, target) in targets {
                    visit_async_block(
                        mir_fn,
                        target,
                        visited,
                        ordered_blocks,
                        suspend_points,
                    )?;
                }
                visit_async_block(
                    mir_fn,
                    otherwise,
                    visited,
                    ordered_blocks,
                    suspend_points,
                )?;
            }
            Terminator::Return(_) | Terminator::Unreachable => {}
            other => {
                return Err(AsyncCfgPlanError::new(format!(
                    "block {} uses unsupported `{}` terminator; async frame lowering currently expects await control flow built from suspend, goto, if, switch, return, and unreachable edges",
                    block,
                    async_cfg_terminator_name(&other)
                )))
            }
        }

        ordered_blocks.push(block);
        Ok(())
    }

    visit_async_block(
        mir_fn,
        mir_fn.start_block,
        &mut visited,
        &mut ordered_blocks,
        &mut suspend_points,
    )?;
    ordered_blocks.reverse();

    Ok(AsyncCfgPlan {
        ordered_blocks,
        suspend_points,
    })
}

fn push_user_local(set: &mut HashSet<Local>, local: Local) {
    if matches!(local.kind, LocalKind::User) {
        set.insert(local);
    }
}

fn instruction_user_uses(inst: &Instruction) -> HashSet<Local> {
    let mut uses = HashSet::new();
    match inst {
        Instruction::Assign { .. } | Instruction::Nop => {}
        Instruction::Unary { operand, .. } => push_user_local(&mut uses, *operand),
        Instruction::Binary { left, right, .. } => {
            push_user_local(&mut uses, *left);
            push_user_local(&mut uses, *right);
        }
        Instruction::Load { source, .. } => push_user_local(&mut uses, *source),
        Instruction::Store { value, .. } => push_user_local(&mut uses, *value),
        Instruction::AddrOf { source, .. } => push_user_local(&mut uses, *source),
        Instruction::FieldAddr { base, .. } => push_user_local(&mut uses, *base),
        Instruction::IndexAddr { base, index, .. } => {
            push_user_local(&mut uses, *base);
            push_user_local(&mut uses, *index);
        }
        Instruction::Extract { value, .. } => push_user_local(&mut uses, *value),
        Instruction::Insert {
            value, new_value, ..
        } => {
            push_user_local(&mut uses, *value);
            push_user_local(&mut uses, *new_value);
        }
        Instruction::Cast { value, .. } | Instruction::Bitcast { value, .. } => {
            push_user_local(&mut uses, *value)
        }
        Instruction::Aggregate { fields, .. } => {
            for field in fields {
                push_user_local(&mut uses, *field);
            }
        }
        Instruction::Call { args, .. } | Instruction::Intrinsic { args, .. } => {
            for arg in args {
                push_user_local(&mut uses, *arg);
            }
        }
        Instruction::Discriminant { source, .. } | Instruction::ExtractPayload { source, .. } => {
            push_user_local(&mut uses, *source)
        }
        Instruction::EnumConstruct { payload, .. } => {
            if let Some(payload) = payload {
                push_user_local(&mut uses, *payload);
            }
        }
        Instruction::Phi { incoming, .. } => {
            for (local, _) in incoming {
                push_user_local(&mut uses, *local);
            }
        }
    }
    uses
}

fn instruction_user_defs(inst: &Instruction) -> HashSet<Local> {
    let mut defs = HashSet::new();
    if let Some(destination) = inst.destination() {
        push_user_local(&mut defs, destination);
    }
    if let Instruction::Store { destination, .. } = inst {
        push_user_local(&mut defs, *destination);
    }
    defs
}

fn terminator_user_uses(term: &Terminator) -> HashSet<Local> {
    let mut uses = HashSet::new();
    match term {
        Terminator::Return(Some(local)) => push_user_local(&mut uses, *local),
        Terminator::If { cond, .. } | Terminator::Switch { discr: cond, .. } => {
            push_user_local(&mut uses, *cond);
        }
        Terminator::Call { args, .. } => {
            for arg in args {
                if let CallArg::Local(local) = arg {
                    push_user_local(&mut uses, *local);
                }
            }
        }
        Terminator::Suspend { future_handle, .. } => push_user_local(&mut uses, *future_handle),
        Terminator::Return(None)
        | Terminator::Goto(_)
        | Terminator::Break { .. }
        | Terminator::Continue { .. }
        | Terminator::Unreachable => {}
    }
    uses
}

fn terminator_user_defs(term: &Terminator) -> HashSet<Local> {
    let mut defs = HashSet::new();
    match term {
        Terminator::Call { destination, .. } | Terminator::Suspend { destination, .. } => {
            push_user_local(&mut defs, *destination);
        }
        Terminator::Return(_)
        | Terminator::Goto(_)
        | Terminator::If { .. }
        | Terminator::Switch { .. }
        | Terminator::Break { .. }
        | Terminator::Continue { .. }
        | Terminator::Unreachable => {}
    }
    defs
}

pub(crate) fn collect_live_user_slots(
    plan: &AsyncCfgPlan,
    spill_user_locals: &[(Local, MIRType)],
    live_in: &HashMap<usize, HashSet<Local>>,
) -> HashMap<usize, Vec<LiveUserSlot>> {
    let slot_map = spill_user_locals
        .iter()
        .enumerate()
        .map(|(slot_index, (local, ty))| (*local, (slot_index, ty.clone())))
        .collect::<HashMap<_, _>>();

    let mut live_slots = HashMap::new();
    for point in &plan.suspend_points {
        let mut slots = live_in
            .get(&point.ready_block)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|local| {
                slot_map.get(&local).map(|(slot_index, ty)| LiveUserSlot {
                    slot_index: *slot_index,
                    local,
                    ty: ty.clone(),
                })
            })
            .collect::<Vec<_>>();
        slots.sort_by_key(|slot| slot.slot_index);
        live_slots.insert(point.block, slots);
    }

    live_slots
}
