//! Conservative boundary for cancellation until frames have owned-state drop maps.
//! Track actual future/task values, so importing the async API never activates it.
use super::async_cfg_helpers::{
    build_async_cfg_plan, collect_spill_user_locals, collect_user_locals,
    compute_live_in_user_locals,
};
use super::{Instruction, Local, LocalKind, MIRType, MirFunction, Terminator};
use crate::CompileError;
use std::collections::{HashMap, HashSet};

type Origins = HashSet<String>;

pub(super) fn check_owned_cancellation(functions: &[MirFunction]) -> Result<(), CompileError> {
    let indices = functions
        .iter()
        .enumerate()
        .map(|(i, f)| (f.name.as_str(), i))
        .collect::<HashMap<_, _>>();
    let mut reachable = HashSet::new();
    let mut pending = if let Some(main) = indices.get("main") {
        vec![*main]
    } else {
        (0..functions.len()).collect()
    };
    while let Some(index) = pending.pop() {
        if !reachable.insert(index) {
            continue;
        }
        for instruction in &functions[index].instructions {
            let name = match instruction {
                Instruction::Call { func, .. } => Some(func.as_str()),
                Instruction::Assign {
                    value: super::MirConstant::GlobalRef(name),
                    ..
                } => Some(name.as_str()),
                _ => None,
            };
            if let Some(name) = name {
                if let Some(target) = indices.get(name).or_else(|| {
                    name.strip_suffix("__start")
                        .and_then(|base| indices.get(base))
                }) {
                    pending.push(*target);
                }
            }
        }
    }

    let mut origins = HashMap::<(usize, Local), Origins>::new();
    let mut returns = HashMap::<usize, Origins>::new();
    let read = |origins: &HashMap<(usize, Local), Origins>, index, local| {
        origins.get(&(index, local)).cloned().unwrap_or_default()
    };
    loop {
        let mut changed = false;
        for &index in &reachable {
            let function = &functions[index];
            for instruction in &function.instructions {
                let (destination, sources) = match instruction {
                    Instruction::Load {
                        destination,
                        source,
                    }
                    | Instruction::AddrOf {
                        destination,
                        source,
                    } => (*destination, vec![*source]),
                    Instruction::Store { destination, value }
                    | Instruction::Cast {
                        destination, value, ..
                    }
                    | Instruction::Bitcast {
                        destination, value, ..
                    } => (*destination, vec![*value]),
                    Instruction::Extract {
                        destination, value, ..
                    } => (*destination, vec![*value]),
                    Instruction::FieldAddr {
                        destination, base, ..
                    } => (*destination, vec![*base]),
                    Instruction::Aggregate {
                        destination,
                        fields,
                        ..
                    } => (*destination, fields.clone()),
                    Instruction::Phi {
                        destination,
                        incoming,
                    } => (
                        *destination,
                        incoming.iter().map(|(local, _)| *local).collect(),
                    ),
                    Instruction::Call {
                        destination,
                        func,
                        args,
                    } => {
                        let base = func.strip_suffix("__start");
                        if let Some(base) = base.filter(|base| {
                            indices
                                .get(base)
                                .is_some_and(|index| functions[*index].is_async)
                        }) {
                            changed |= origins
                                .entry((index, *destination))
                                .or_default()
                                .insert(base.to_string());
                        }
                        if let Some(&target) = indices
                            .get(func.as_str())
                            .or_else(|| base.and_then(|base| indices.get(base)))
                        {
                            for (i, argument) in args.iter().enumerate() {
                                let values = read(&origins, index, *argument);
                                let entry = origins
                                    .entry((target, Local::new(i + 1, LocalKind::Param)))
                                    .or_default();
                                let before = entry.len();
                                entry.extend(values);
                                changed |= before != entry.len();
                            }
                            let values = returns.get(&target).cloned().unwrap_or_default();
                            let entry = origins.entry((index, *destination)).or_default();
                            let before = entry.len();
                            entry.extend(values);
                            changed |= before != entry.len();
                        }
                        let sources = match func.as_str() {
                            "sengoo_async_spawn_task_raw" | "sengoo_async_spawn_raw" => {
                                args.get(1).copied().into_iter().collect()
                            }
                            "sengoo_async_task_scope_spawn_raw" => {
                                args.get(2).copied().into_iter().collect()
                            }
                            _ => Vec::new(),
                        };
                        (*destination, sources)
                    }
                    _ => continue,
                };
                let values = sources
                    .into_iter()
                    .flat_map(|local| read(&origins, index, local))
                    .collect::<Origins>();
                let entry = origins.entry((index, destination)).or_default();
                let before = entry.len();
                entry.extend(values);
                changed |= before != entry.len();
            }
            for block in &function.basic_blocks {
                if let Some(Terminator::Return(Some(local))) = block.terminator {
                    let values = read(&origins, index, local);
                    let entry = returns.entry(index).or_default();
                    let before = entry.len();
                    entry.extend(values);
                    changed |= before != entry.len();
                }
            }
        }
        if !changed {
            break;
        }
    }

    let mut canceled = Origins::new();
    for &index in &reachable {
        for instruction in &functions[index].instructions {
            let Instruction::Call { func, args, .. } = instruction else {
                continue;
            };
            let handles = match func.as_str() {
                "sengoo_async_cancel_task" => args.first().copied().into_iter().collect::<Vec<_>>(),
                "sengoo_async_timeout_cancel_i64__start" => {
                    args.get(1).copied().into_iter().collect()
                }
                // Scope destruction may cancel every unfinished scoped child.
                "sengoo_async_task_scope_spawn_raw" => args.get(2).copied().into_iter().collect(),
                "sengoo_async_select_cancel_winner" | "sengoo_async_select_cancel_n_winner" => {
                    args.clone()
                }
                _ => Vec::new(),
            };
            for handle in handles {
                canceled.extend(read(&origins, index, handle));
            }
        }
    }
    // A canceled parent can leave its currently awaited child pending as well.
    loop {
        let mut descendants = Origins::new();
        for name in &canceled {
            if let Some(&index) = indices.get(name.as_str()) {
                for instruction in &functions[index].instructions {
                    if let Instruction::Call { func, .. } = instruction {
                        if let Some(base) = func
                            .strip_suffix("__start")
                            .filter(|base| indices.contains_key(base))
                        {
                            descendants.insert(base.to_string());
                        }
                    }
                }
            }
        }
        let before = canceled.len();
        canceled.extend(descendants);
        if canceled.len() == before {
            break;
        }
    }
    let drop_types = functions
        .iter()
        .filter_map(|f| f.name.strip_suffix("_Drop_drop"))
        .collect::<HashSet<_>>();
    fn owns(ty: &MIRType, drop_types: &HashSet<&str>) -> bool {
        match ty {
            MIRType::Struct { name, fields } => {
                drop_types.contains(name.as_str())
                    || fields.iter().any(|(_, ty)| owns(ty, drop_types))
            }
            MIRType::Tuple(fields) => fields.iter().any(|ty| owns(ty, drop_types)),
            MIRType::Array(ty, count) => *count > 0 && owns(ty, drop_types),
            _ => false,
        }
    }
    let mut canceled = canceled.into_iter().collect::<Vec<_>>();
    canceled.sort();
    for name in canceled {
        let Some(&index) = indices.get(name.as_str()) else {
            continue;
        };
        let function = &functions[index];
        let user_locals = collect_user_locals(function);
        let spilled = if let Ok(plan) = build_async_cfg_plan(function) {
            collect_spill_user_locals(
                &plan,
                &user_locals,
                &compute_live_in_user_locals(function, &plan)?,
            )
        } else {
            Vec::new()
        };
        if function
            .params
            .iter()
            .chain(std::iter::once(&function.return_type))
            .any(|ty| owns(ty, &drop_types))
            || spilled.iter().any(|(_, ty)| owns(ty, &drop_types))
        {
            return Err(CompileError::MirLower(format!("[unsupported-owned-async-cancellation] async function `{name}` can be canceled while its frame owns a resource; cancellation Drop state is not implemented. Await it to completion and keep cancellation limited to resource-free futures")));
        }
    }
    Ok(())
}
