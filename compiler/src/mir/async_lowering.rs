//! Async lowering: synthesize frame-backed __start/__poll/__result helpers
//!
//! For each `async def foo(params...) -> T`, we generate three helper functions:
//!   - `foo__start(params...) -> i64`  — allocates a frame, stores params, sets state=0, returns handle
//!   - `foo__poll(handle: i64) -> i64`  — runs until next suspend or completion, returns 0=pending, 1=ready
//!   - `foo__result(handle: i64) -> T`  — reads the result from the frame, frees it, returns T

use super::async_dispatch_helpers::{build_async_dispatch_registry, AsyncDispatchRegistry};
use super::async_cfg_helpers::{
    build_async_cfg_plan, collect_live_user_slots, collect_spill_user_locals,
    collect_user_locals, compute_live_in_user_locals, AsyncCfgPlan, LiveUserSlot,
};
use crate::mir::{
    Instruction, Local, LocalKind, MIRType, MirConstant,
    MirFunction, Terminator, MIR_BOOL, MIR_I64, MIR_UNIT,
};
use crate::CompileError;
use std::collections::{BTreeMap, HashMap};
#[cfg(test)]
use std::collections::HashSet;

/// Per-async-function frame layout (logical, not physical — fields stored at offsets in a malloc'd block)
///   offset 0: state (i64)
///   offset 1: result (i64, holds return value once ready)
///   offset 2..2+N: parameter copies
///   offset 2+N..: child future handles + spilled locals
#[derive(Debug, Clone)]
pub struct AsyncFrameLayout {
    pub func_name: String,
    pub param_types: Vec<MIRType>,
    pub param_offsets: Vec<i64>,
    pub return_type: MIRType,
    pub result_storage_ty: MIRType,
    pub await_count: usize,
    pub user_local_count: usize,
    pub user_local_offsets: Vec<i64>,
    pub await_offset_start: i64,
    pub total_slots: usize,
}

impl AsyncFrameLayout {
    pub fn total_slots(&self) -> usize {
        self.total_slots
    }
}

fn frame_storage_ty(ty: &MIRType) -> MIRType {
    match ty {
        MIRType::Unit => MIR_I64,
        other => other.clone(),
    }
}

fn enum_is_payloadless(ty: &MIRType) -> bool {
    matches!(
        ty,
        MIRType::Enum { variants, .. } if variants.iter().all(|(_, payload)| payload.is_none())
    )
}

fn async_frame_slot_count(ty: &MIRType) -> Result<usize, CompileError> {
    let storage_ty = frame_storage_ty(ty);
    match &storage_ty {
        MIRType::Bool
        | MIRType::Int(8 | 16 | 32 | 64)
        | MIRType::Float(32 | 64)
        | MIRType::Ref(_)
        | MIRType::Ptr(_)
        | MIRType::Future(_) => Ok(1),
        MIRType::Tuple(items) => items.iter().try_fold(0usize, |acc, item| {
            Ok(acc + async_frame_slot_count(item)?)
        }),
        MIRType::Array(elem, len) => {
            let elem_slots = async_frame_slot_count(elem)?;
            Ok(elem_slots.saturating_mul(*len as usize))
        }
        MIRType::Struct { fields, .. } => fields.iter().try_fold(0usize, |acc, (_, field_ty)| {
            Ok(acc + async_frame_slot_count(field_ty)?)
        }),
        MIRType::Enum { .. } if enum_is_payloadless(&storage_ty) => Ok(1),
        MIRType::Enum { .. } => Err(unsupported_async_frame_type(
            &storage_ty,
            "payload-carrying enum values cannot cross await points yet",
        )),
        _ => Err(unsupported_async_frame_type(
            &storage_ty,
            "only scalar, pointer-like, tuple/struct/array, and Future values are supported in async frames yet",
        )),
    }
}

fn build_async_frame_layout(
    func_name: String,
    param_types: Vec<MIRType>,
    return_type: MIRType,
    await_count: usize,
    user_locals: &[(Local, MIRType)],
) -> Result<AsyncFrameLayout, CompileError> {
    let result_storage_ty = frame_storage_ty(&return_type);
    let result_slots = async_frame_slot_count(&result_storage_ty)?;

    let mut next_offset = 1 + result_slots as i64;

    let mut param_offsets = Vec::with_capacity(param_types.len());
    for ty in &param_types {
        param_offsets.push(next_offset);
        next_offset += async_frame_slot_count(ty)? as i64;
    }

    let mut user_local_offsets = Vec::with_capacity(user_locals.len());
    for (_, ty) in user_locals {
        user_local_offsets.push(next_offset);
        next_offset += async_frame_slot_count(ty)? as i64;
    }

    let await_offset_start = next_offset;
    let total_slots = (await_offset_start as usize) + await_count;

    Ok(AsyncFrameLayout {
        func_name,
        param_types,
        param_offsets,
        return_type,
        result_storage_ty,
        await_count,
        user_local_count: user_locals.len(),
        user_local_offsets,
        await_offset_start,
        total_slots,
    })
}

/// Count the number of await points in a MIR function by scanning for Suspend terminators
pub fn count_await_points(mir_fn: &MirFunction) -> usize {
    mir_fn
        .basic_blocks
        .iter()
        .filter(|bb| matches!(bb.terminator, Some(Terminator::Suspend { .. })))
        .count()
}

pub fn select_result_runtime_suffix(ty: &MIRType) -> Option<&'static str> {
    match ty {
        MIRType::Bool => Some("bool"),
        MIRType::Int(8) => Some("i8"),
        MIRType::Int(16) => Some("i16"),
        MIRType::Int(32) => Some("i32"),
        MIRType::Int(64) => Some("i64"),
        MIRType::Float(32) => Some("f32"),
        MIRType::Float(64) => Some("f64"),
        _ => None,
    }
}

pub fn select_runtime_function_name(ty: &MIRType) -> Option<String> {
    select_result_runtime_suffix(ty).map(|suffix| format!("sengoo_async_select_{suffix}"))
}

pub fn select_result_dispatch_name(ty: &MIRType) -> Option<String> {
    select_result_runtime_suffix(ty).map(|suffix| format!("sengoo_async_result_dispatch_{suffix}"))
}

pub fn select_runtime_return_type(ty: &MIRType) -> Option<MIRType> {
    match ty {
        MIRType::Bool => Some(MIR_BOOL),
        MIRType::Int(8) => Some(MIRType::Int(8)),
        MIRType::Int(16) => Some(MIRType::Int(16)),
        MIRType::Int(32) => Some(MIRType::Int(32)),
        MIRType::Int(64) => Some(MIR_I64),
        MIRType::Float(32) => Some(MIRType::Float(32)),
        MIRType::Float(64) => Some(MIRType::Float(64)),
        _ => None,
    }
}

pub fn select_runtime_declaration(ty: &MIRType) -> Option<String> {
    let name = select_runtime_function_name(ty)?;
    let ret = match select_runtime_return_type(ty)? {
        MIRType::Bool => "i1".to_string(),
        MIRType::Int(bits) => format!("i{bits}"),
        MIRType::Float(32) => "float".to_string(),
        MIRType::Float(64) => "double".to_string(),
        _ => return None,
    };
    Some(format!("declare {ret} @{name}(i64, i64, i64, i64)\n"))
}

fn select_result_default_constant(ty: &MIRType) -> Option<MirConstant> {
    match ty {
        MIRType::Bool => Some(MirConstant::Bool(false)),
        MIRType::Int(_) => Some(MirConstant::Int(0)),
        MIRType::Float(_) => Some(MirConstant::Float(0.0)),
        _ => None,
    }
}

/// Given a list of MIR functions, expand each async function into its original body
/// plus three synthesized helpers. Returns additional functions to add.
///
/// For async `main`, the original body is renamed to `main__body` and a new
/// `main` wrapper is generated that drives the async helpers.
pub fn expand_async_functions(
    mir_fns: &mut Vec<MirFunction>,
) -> Result<Vec<MirFunction>, CompileError> {
    let async_fn_names: Vec<String> = mir_fns
        .iter()
        .filter(|f| f.is_async)
        .map(|f| f.name.clone())
        .collect();
    let dispatch_registry = build_async_dispatch_registry(async_fn_names.iter().cloned());

    let has_async_main = async_fn_names.iter().any(|n| n == "main");

    let mut new_fns = Vec::new();
    let mut spawn_dispatch_entries = Vec::new();
    let mut result_dispatch_entries: BTreeMap<String, (MIRType, Vec<(String, String)>)> =
        BTreeMap::new();

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

        let user_locals = collect_user_locals(mir_fn);
        let await_count = count_await_points(mir_fn);
        let spill_user_locals = if await_count == 0 {
            Vec::new()
        } else if let Ok(plan) = build_async_cfg_plan(mir_fn) {
            let live_in = compute_live_in_user_locals(mir_fn, &plan)?;
            collect_spill_user_locals(&plan, &user_locals, &live_in)
        } else {
            Vec::new()
        };
        let layout = build_async_frame_layout(
            body_name,
            mir_fn.params.clone(),
            mir_fn.return_type.clone(),
            await_count,
            &spill_user_locals,
        )?;

        let mut start = synthesize_start(&layout)?;
        let mut poll = synthesize_poll(&layout, mir_fn, &spill_user_locals)?;
        let mut result = synthesize_result(&layout)?;

        if name == "main" {
            start.name = "main__start".to_string();
            poll.name = "main__poll".to_string();
            result.name = "main__result".to_string();
        }

        spawn_dispatch_entries.push((name.clone(), poll.name.clone()));
        if let Some(suffix) = select_result_runtime_suffix(&mir_fn.return_type) {
            let (_, entries) = result_dispatch_entries
                .entry(suffix.to_string())
                .or_insert_with(|| (mir_fn.return_type.clone(), Vec::new()));
            entries.push((name.clone(), result.name.clone()));
        }

        new_fns.push(start);
        new_fns.push(poll);
        new_fns.push(result);
    }

    if !spawn_dispatch_entries.is_empty() {
        new_fns.push(synthesize_spawn_poll_dispatch(
            &dispatch_registry,
            &spawn_dispatch_entries,
        )?);
        new_fns.push(synthesize_spawn_cancel_dispatch(
            &dispatch_registry,
            &spawn_dispatch_entries,
        )?);
        new_fns.push(synthesize_spawn_drop_dispatch(
            &dispatch_registry,
            &spawn_dispatch_entries,
        )?);
    }
    let needs_timeout_bool_dispatch = mir_fns.iter().any(|mir_fn| {
        mir_fn.instructions.iter().any(|inst| match inst {
            Instruction::Call { func, .. } => func == "sengoo_async_timeout_bool__start",
            _ => false,
        })
    });
    if needs_timeout_bool_dispatch {
        let (_, entries) = result_dispatch_entries
            .entry("bool".to_string())
            .or_insert_with(|| (MIR_BOOL, Vec::new()));
        if !entries
            .iter()
            .any(|(base_name, _)| base_name == "sengoo_async_timeout_bool")
        {
            entries.push((
                "sengoo_async_timeout_bool".to_string(),
                "sengoo_async_timeout_bool__result".to_string(),
            ));
        }
    }

    for (_suffix, (return_ty, entries)) in result_dispatch_entries {
        new_fns.push(synthesize_result_dispatch(
            &dispatch_registry,
            &return_ty,
            &entries,
        )?);
    }

    if has_async_main {
        if let Some(main_fn) = mir_fns.iter_mut().find(|f| f.name == "main") {
            main_fn.name = "main__body".to_string();
        }
        new_fns.push(synthesize_async_main_wrapper());
    }

    Ok(new_fns)
}

fn dispatch_switch_key(
    registry: &AsyncDispatchRegistry,
    base_name: &str,
) -> Result<u32, CompileError> {
    let kind = registry.kind_id(base_name).ok_or_else(|| {
        CompileError::MirLower(format!(
            "missing async dispatch id for future origin `{base_name}`"
        ))
    })?;
    u32::try_from(kind).map_err(|_| {
        CompileError::MirLower(format!(
            "async dispatch id for future origin `{base_name}` does not fit switch key width"
        ))
    })
}

fn synthesize_spawn_poll_dispatch(
    registry: &AsyncDispatchRegistry,
    entries: &[(String, String)],
) -> Result<MirFunction, CompileError> {
    let mut f = MirFunction::new(
        "sengoo_async_poll_dispatch".to_string(),
        vec![MIR_I64, MIR_I64],
        MIR_I64,
    );

    let bb0 = f.start_block;
    let kind_local = Local::new(1, LocalKind::Param);
    let handle_local = Local::new(2, LocalKind::Param);
    let default_block = f.add_block();
    let mut targets = Vec::with_capacity(entries.len());

    for (base_name, poll_name) in entries {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, MIR_I64);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: poll_name.clone(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((dispatch_switch_key(registry, base_name)?, case_block));
    }

    if !entries.iter().any(|(base_name, _)| base_name == "sengoo_async_sleep") {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, MIR_I64);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_sleep__poll".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((dispatch_switch_key(registry, "sengoo_async_sleep")?, case_block));
    }

    if !entries
        .iter()
        .any(|(base_name, _)| base_name == "sengoo_async_timeout_bool")
    {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, MIR_I64);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_timeout_bool__poll".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((
            dispatch_switch_key(registry, "sengoo_async_timeout_bool")?,
            case_block,
        ));
    }

    f.basic_blocks[bb0].set_terminator(Terminator::Switch {
        discr: kind_local,
        targets,
        otherwise: default_block,
    });

    let ready_local = f.add_local(LocalKind::Temp, MIR_I64);
    let ready_inst = f.alloc_inst(Instruction::Assign {
        destination: ready_local,
        value: MirConstant::Int(1),
    });
    f.basic_blocks[default_block].push(ready_inst);
    f.basic_blocks[default_block].set_terminator(Terminator::Return(Some(ready_local)));

    Ok(f)
}

fn synthesize_spawn_cancel_dispatch(
    registry: &AsyncDispatchRegistry,
    entries: &[(String, String)],
) -> Result<MirFunction, CompileError> {
    let mut f = MirFunction::new(
        "sengoo_async_cancel_dispatch".to_string(),
        vec![MIR_I64, MIR_I64],
        MIR_BOOL,
    );

    let bb0 = f.start_block;
    let kind_local = Local::new(1, LocalKind::Param);
    let handle_local = Local::new(2, LocalKind::Param);
    let default_block = f.add_block();
    let mut targets = Vec::with_capacity(entries.len());

    for (base_name, _) in entries {
        let case_block = f.add_block();
        let free_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: free_dest,
            func: "sengoo_async_frame_free".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        let true_local = f.add_local(LocalKind::Temp, MIR_BOOL);
        let true_inst = f.alloc_inst(Instruction::Assign {
            destination: true_local,
            value: MirConstant::Bool(true),
        });
        f.basic_blocks[case_block].push(true_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(true_local)));
        targets.push((dispatch_switch_key(registry, base_name)?, case_block));
    }

    if !entries.iter().any(|(base_name, _)| base_name == "sengoo_async_sleep") {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, MIR_BOOL);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_sleep__cancel".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((dispatch_switch_key(registry, "sengoo_async_sleep")?, case_block));
    }

    if !entries
        .iter()
        .any(|(base_name, _)| base_name == "sengoo_async_timeout_bool")
    {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, MIR_BOOL);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_timeout_bool__cancel".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((
            dispatch_switch_key(registry, "sengoo_async_timeout_bool")?,
            case_block,
        ));
    }

    f.basic_blocks[bb0].set_terminator(Terminator::Switch {
        discr: kind_local,
        targets,
        otherwise: default_block,
    });

    let false_local = f.add_local(LocalKind::Temp, MIR_BOOL);
    let false_inst = f.alloc_inst(Instruction::Assign {
        destination: false_local,
        value: MirConstant::Bool(false),
    });
    f.basic_blocks[default_block].push(false_inst);
    f.basic_blocks[default_block].set_terminator(Terminator::Return(Some(false_local)));

    Ok(f)
}

fn synthesize_spawn_drop_dispatch(
    registry: &AsyncDispatchRegistry,
    entries: &[(String, String)],
) -> Result<MirFunction, CompileError> {
    let mut f = MirFunction::new(
        "sengoo_async_drop_dispatch".to_string(),
        vec![MIR_I64, MIR_I64],
        MIR_UNIT,
    );

    let bb0 = f.start_block;
    let kind_local = Local::new(1, LocalKind::Param);
    let handle_local = Local::new(2, LocalKind::Param);
    let default_block = f.add_block();
    let mut targets = Vec::with_capacity(entries.len());

    for (base_name, _) in entries {
        let case_block = f.add_block();
        let free_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: free_dest,
            func: "sengoo_async_frame_free".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(None));
        targets.push((dispatch_switch_key(registry, base_name)?, case_block));
    }

    if !entries.iter().any(|(base_name, _)| base_name == "sengoo_async_sleep") {
        let case_block = f.add_block();
        let unit_local = f.add_local(LocalKind::Temp, MIR_UNIT);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: unit_local,
            func: "sengoo_async_sleep__drop".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(None));
        targets.push((dispatch_switch_key(registry, "sengoo_async_sleep")?, case_block));
    }

    if !entries
        .iter()
        .any(|(base_name, _)| base_name == "sengoo_async_timeout_bool")
    {
        let case_block = f.add_block();
        let unit_local = f.add_local(LocalKind::Temp, MIR_UNIT);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: unit_local,
            func: "sengoo_async_timeout_bool__drop".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(None));
        targets.push((
            dispatch_switch_key(registry, "sengoo_async_timeout_bool")?,
            case_block,
        ));
    }

    f.basic_blocks[bb0].set_terminator(Terminator::Switch {
        discr: kind_local,
        targets,
        otherwise: default_block,
    });
    f.basic_blocks[default_block].set_terminator(Terminator::Return(None));

    Ok(f)
}

fn synthesize_result_dispatch(
    registry: &AsyncDispatchRegistry,
    return_ty: &MIRType,
    entries: &[(String, String)],
) -> Result<MirFunction, CompileError> {
    let dispatch_name = select_result_dispatch_name(return_ty).ok_or_else(|| {
        CompileError::MirLower(format!(
            "async result dispatch only supports scalar select result types, got `{:?}`",
            return_ty
        ))
    })?;
    let default_value = select_result_default_constant(return_ty).ok_or_else(|| {
        CompileError::MirLower(format!(
            "async result dispatch has no default value for unsupported type `{:?}`",
            return_ty
        ))
    })?;

    let mut f = MirFunction::new(dispatch_name, vec![MIR_I64, MIR_I64], return_ty.clone());

    let bb0 = f.start_block;
    let kind_local = Local::new(1, LocalKind::Param);
    let handle_local = Local::new(2, LocalKind::Param);
    let default_block = f.add_block();
    let mut targets = Vec::with_capacity(entries.len());

    for (base_name, result_name) in entries {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, return_ty.clone());
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: result_name.clone(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((dispatch_switch_key(registry, base_name)?, case_block));
    }

    f.basic_blocks[bb0].set_terminator(Terminator::Switch {
        discr: kind_local,
        targets,
        otherwise: default_block,
    });

    let default_local = f.add_local(LocalKind::Temp, return_ty.clone());
    let default_inst = f.alloc_inst(Instruction::Assign {
        destination: default_local,
        value: default_value,
    });
    f.basic_blocks[default_block].push(default_inst);
    f.basic_blocks[default_block].set_terminator(Terminator::Return(Some(default_local)));

    Ok(f)
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
fn synthesize_start(layout: &AsyncFrameLayout) -> Result<MirFunction, CompileError> {
    let name = format!("{}__start", layout.func_name);
    let mut f = MirFunction::new(name, layout.param_types.clone(), MIR_I64);

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

    // Store each parameter at its assigned frame offset.
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

    // Return the handle
    f.basic_blocks[bb0].set_terminator(Terminator::Return(Some(handle_local)));

    Ok(f)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AsyncFrameValueKind {
    I64,
    NarrowInt,
    Bool,
    Float32,
    Float64,
    PointerLike,
}

fn describe_async_frame_type(ty: &MIRType) -> String {
    match ty {
        MIRType::Unit => "unit".to_string(),
        MIRType::Never => "never".to_string(),
        MIRType::Bool => "bool".to_string(),
        MIRType::Int(bits) => format!("i{}", bits),
        MIRType::Float(bits) => format!("f{}", bits),
        MIRType::Ref(inner) => format!("&{}", describe_async_frame_type(inner)),
        MIRType::Ptr(inner) => format!("*{}", describe_async_frame_type(inner)),
        MIRType::Array(elem, len) => format!("[{}; {}]", describe_async_frame_type(elem), len),
        MIRType::Tuple(types) => format!(
            "({})",
            types
                .iter()
                .map(describe_async_frame_type)
                .collect::<Vec<_>>()
                .join(", ")
        ),
        MIRType::Fn { .. } => "fn".to_string(),
        MIRType::Struct { name, .. } => name.clone(),
        MIRType::Enum { .. } => "enum".to_string(),
        MIRType::Future(inner) => format!("Future<{}>", describe_async_frame_type(inner)),
    }
}

fn unsupported_async_frame_type(ty: &MIRType, reason: &str) -> CompileError {
    CompileError::AsyncUnsupportedType {
        ty: describe_async_frame_type(ty),
        reason: reason.to_string(),
    }
}

fn classify_async_frame_type(ty: &MIRType) -> Result<AsyncFrameValueKind, CompileError> {
    match ty {
        MIRType::Bool => Ok(AsyncFrameValueKind::Bool),
        MIRType::Int(8 | 16 | 32) => Ok(AsyncFrameValueKind::NarrowInt),
        MIRType::Int(64) | MIRType::Future(_) => Ok(AsyncFrameValueKind::I64),
        MIRType::Float(32) => Ok(AsyncFrameValueKind::Float32),
        MIRType::Float(64) => Ok(AsyncFrameValueKind::Float64),
        MIRType::Ref(_) | MIRType::Ptr(_) => Ok(AsyncFrameValueKind::PointerLike),
        MIRType::Tuple(_) | MIRType::Struct { .. } | MIRType::Array(_, _) | MIRType::Enum { .. } => {
            Err(unsupported_async_frame_type(
                ty,
                "aggregate types (tuple/struct/array/enum) cannot cross await points yet",
            ))
        }
        _ => Err(unsupported_async_frame_type(
            ty,
            "only bool, i8/i16/i32/i64, f32/f64, ref/ptr, and Future handles are supported in async frames yet",
        )),
    }
}

fn frame_user_slot(layout: &AsyncFrameLayout, index: usize) -> i64 {
    layout.user_local_offsets[index]
}

fn frame_await_slot(layout: &AsyncFrameLayout, index: usize) -> i64 {
    layout.await_offset_start + index as i64
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

fn encode_async_frame_value(
    f: &mut MirFunction,
    block: usize,
    value: Local,
    ty: &MIRType,
) -> Result<Local, CompileError> {
    match classify_async_frame_type(ty)? {
        AsyncFrameValueKind::I64 => Ok(value),
        AsyncFrameValueKind::Bool | AsyncFrameValueKind::NarrowInt | AsyncFrameValueKind::PointerLike => {
            let encoded = f.add_local(LocalKind::Temp, MIR_I64);
            let cast = f.alloc_inst(Instruction::Cast {
                destination: encoded,
                value,
                to: MIR_I64,
            });
            f.basic_blocks[block].push(cast);
            Ok(encoded)
        }
        AsyncFrameValueKind::Float32 => {
            let bitcast_i32 = f.add_local(LocalKind::Temp, MIRType::Int(32));
            let bitcast = f.alloc_inst(Instruction::Bitcast {
                destination: bitcast_i32,
                value,
                to: MIRType::Int(32),
            });
            f.basic_blocks[block].push(bitcast);

            let encoded = f.add_local(LocalKind::Temp, MIR_I64);
            let cast = f.alloc_inst(Instruction::Cast {
                destination: encoded,
                value: bitcast_i32,
                to: MIR_I64,
            });
            f.basic_blocks[block].push(cast);
            Ok(encoded)
        }
        AsyncFrameValueKind::Float64 => {
            let encoded = f.add_local(LocalKind::Temp, MIR_I64);
            let bitcast = f.alloc_inst(Instruction::Bitcast {
                destination: encoded,
                value,
                to: MIR_I64,
            });
            f.basic_blocks[block].push(bitcast);
            Ok(encoded)
        }
    }
}

fn push_extract_value(
    f: &mut MirFunction,
    block: usize,
    value: Local,
    index: usize,
    field_ty: MIRType,
) -> Local {
    let extracted = f.add_local(LocalKind::Temp, field_ty);
    let inst = f.alloc_inst(Instruction::Extract {
        destination: extracted,
        value,
        index: index as u32,
    });
    f.basic_blocks[block].push(inst);
    extracted
}

fn push_aggregate_value(
    f: &mut MirFunction,
    block: usize,
    ty: MIRType,
    fields: Vec<Local>,
) -> Local {
    let aggregate = f.add_local(LocalKind::Temp, ty.clone());
    let inst = f.alloc_inst(Instruction::Aggregate {
        destination: aggregate,
        fields,
        ty,
    });
    f.basic_blocks[block].push(inst);
    aggregate
}

fn push_frame_store_typed(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    value: Local,
    ty: &MIRType,
) -> Result<(), CompileError> {
    let storage_ty = frame_storage_ty(ty);
    match &storage_ty {
        MIRType::Enum { .. } if enum_is_payloadless(&storage_ty) => {
            let discr = f.add_local(LocalKind::Temp, MIR_I64);
            let inst = f.alloc_inst(Instruction::Discriminant {
                destination: discr,
                source: value,
            });
            f.basic_blocks[block].push(inst);
            push_frame_store(f, block, handle, offset, discr);
            Ok(())
        }
        MIRType::Enum { .. } => Err(unsupported_async_frame_type(
            &storage_ty,
            "payload-carrying enum values cannot cross await points yet",
        )),
        MIRType::Tuple(items) => {
            let mut next_offset = offset;
            for (index, item_ty) in items.iter().enumerate() {
                let extracted = push_extract_value(f, block, value, index, item_ty.clone());
                push_frame_store_typed(f, block, handle, next_offset, extracted, item_ty)?;
                next_offset += async_frame_slot_count(item_ty)? as i64;
            }
            Ok(())
        }
        MIRType::Array(elem, len) => {
            let mut next_offset = offset;
            for index in 0..(*len as usize) {
                let extracted =
                    push_extract_value(f, block, value, index, (**elem).clone());
                push_frame_store_typed(f, block, handle, next_offset, extracted, elem)?;
                next_offset += async_frame_slot_count(elem)? as i64;
            }
            Ok(())
        }
        MIRType::Struct { fields, .. } => {
            let mut next_offset = offset;
            for (index, (_, field_ty)) in fields.iter().enumerate() {
                let extracted =
                    push_extract_value(f, block, value, index, field_ty.clone());
                push_frame_store_typed(f, block, handle, next_offset, extracted, field_ty)?;
                next_offset += async_frame_slot_count(field_ty)? as i64;
            }
            Ok(())
        }
        _ => {
            let encoded = encode_async_frame_value(f, block, value, &storage_ty)?;
            push_frame_store(f, block, handle, offset, encoded);
            Ok(())
        }
    }
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

fn push_frame_load_into_typed(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    destination: Local,
    ty: &MIRType,
) -> Result<(), CompileError> {
    let storage_ty = frame_storage_ty(ty);
    match &storage_ty {
        MIRType::Tuple(_) | MIRType::Array(_, _) | MIRType::Struct { .. } | MIRType::Enum { .. } => {
            let loaded = push_frame_load_typed(f, block, handle, offset, storage_ty.clone())?;
            let store = f.alloc_inst(Instruction::Store {
                destination,
                value: loaded,
            });
            f.basic_blocks[block].push(store);
        }
        _ => match classify_async_frame_type(&storage_ty)? {
            AsyncFrameValueKind::I64 => push_frame_load_into(f, block, handle, offset, destination),
            AsyncFrameValueKind::Bool
            | AsyncFrameValueKind::NarrowInt
            | AsyncFrameValueKind::PointerLike => {
                let encoded = push_frame_load(f, block, handle, offset, MIR_I64);
                let cast = f.alloc_inst(Instruction::Cast {
                    destination,
                    value: encoded,
                    to: storage_ty,
                });
                f.basic_blocks[block].push(cast);
            }
            AsyncFrameValueKind::Float32 => {
                let encoded = push_frame_load(f, block, handle, offset, MIR_I64);
                let narrowed = f.add_local(LocalKind::Temp, MIRType::Int(32));
                let cast = f.alloc_inst(Instruction::Cast {
                    destination: narrowed,
                    value: encoded,
                    to: MIRType::Int(32),
                });
                f.basic_blocks[block].push(cast);
                let bitcast = f.alloc_inst(Instruction::Bitcast {
                    destination,
                    value: narrowed,
                    to: storage_ty,
                });
                f.basic_blocks[block].push(bitcast);
            }
            AsyncFrameValueKind::Float64 => {
                let encoded = push_frame_load(f, block, handle, offset, MIR_I64);
                let bitcast = f.alloc_inst(Instruction::Bitcast {
                    destination,
                    value: encoded,
                    to: storage_ty,
                });
                f.basic_blocks[block].push(bitcast);
            }
        },
    }
    Ok(())
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

fn push_frame_load_typed(
    f: &mut MirFunction,
    block: usize,
    handle: Local,
    offset: i64,
    ty: MIRType,
) -> Result<Local, CompileError> {
    let storage_ty = frame_storage_ty(&ty);
    match &storage_ty {
        MIRType::Enum { .. } if enum_is_payloadless(&storage_ty) => {
            let discr = push_frame_load(f, block, handle, offset, MIR_I64);
            let zero_payload = push_i64_const(f, block, 0);
            Ok(push_aggregate_value(
                f,
                block,
                storage_ty,
                vec![discr, zero_payload],
            ))
        }
        MIRType::Enum { .. } => Err(unsupported_async_frame_type(
            &storage_ty,
            "payload-carrying enum values cannot cross await points yet",
        )),
        MIRType::Tuple(items) => {
            let mut fields = Vec::with_capacity(items.len());
            let mut next_offset = offset;
            for item_ty in items {
                let loaded = push_frame_load_typed(f, block, handle, next_offset, item_ty.clone())?;
                fields.push(loaded);
                next_offset += async_frame_slot_count(item_ty)? as i64;
            }
            Ok(push_aggregate_value(f, block, storage_ty, fields))
        }
        MIRType::Array(elem, len) => {
            let mut fields = Vec::with_capacity(*len as usize);
            let mut next_offset = offset;
            for _ in 0..(*len as usize) {
                let loaded =
                    push_frame_load_typed(f, block, handle, next_offset, (**elem).clone())?;
                fields.push(loaded);
                next_offset += async_frame_slot_count(elem)? as i64;
            }
            Ok(push_aggregate_value(f, block, storage_ty, fields))
        }
        MIRType::Struct { fields, .. } => {
            let mut values = Vec::with_capacity(fields.len());
            let mut next_offset = offset;
            for (_, field_ty) in fields {
                let loaded =
                    push_frame_load_typed(f, block, handle, next_offset, field_ty.clone())?;
                values.push(loaded);
                next_offset += async_frame_slot_count(field_ty)? as i64;
            }
            Ok(push_aggregate_value(f, block, storage_ty, values))
        }
        _ => {
            let destination = f.add_local(LocalKind::Temp, storage_ty.clone());
            push_frame_load_into_typed(f, block, handle, offset, destination, &storage_ty)?;
            Ok(destination)
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

fn synthesize_cfg_poll(
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
            let cloned =
                remap_instruction(mir_fn.instruction(*inst_id), &local_map, &translated_blocks)?;
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
                    live_user_slots
                        .get(block)
                        .map(|slots| slots.as_slice())
                        .unwrap_or(&[]),
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

/// Generate `foo__poll(handle: i64) -> i64`
/// Returns 0 = pending, 1 = ready
fn synthesize_poll(
    layout: &AsyncFrameLayout,
    mir_fn: &MirFunction,
    spill_user_locals: &[(Local, MIRType)],
) -> Result<MirFunction, CompileError> {
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

    let n_states = layout.await_count + 1;
    let result_storage_ty = layout.result_storage_ty.clone();

    if layout.await_count == 0 {
        // No-await async bodies can still run as a single call into the original body.
        let body_block = f.add_block();
        let done_block = f.add_block();

        // Jump to body
        f.basic_blocks[bb0].set_terminator(Terminator::Goto(body_block));

        // Body block: load params, call original, store result
        let result_val = f.add_local(LocalKind::Temp, result_storage_ty.clone());

        // Load params from frame
        let mut param_locals = Vec::new();
        for i in 0..layout.param_types.len() {
            let p = f.add_local(LocalKind::Temp, layout.param_types[i].clone());
            push_frame_load_into_typed(
                &mut f,
                body_block,
                handle,
                layout.param_offsets[i],
                p,
                &layout.param_types[i],
            )?;
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

        push_frame_store_typed(
            &mut f,
            body_block,
            handle,
            1,
            result_val,
            &layout.result_storage_ty,
        )?;

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
        return Ok(f);
    }

    match build_async_cfg_plan(mir_fn) {
        Ok(plan) => return synthesize_cfg_poll(layout, mir_fn, &plan, spill_user_locals),
        Err(reason) => {
            let _ = (bb0, state, result_storage_ty, n_states);
            Err(CompileError::Codegen(format!(
                "async frame lowering requires await control flow that can be expressed with suspend points, self-looping pending blocks, and goto/if/switch/return edges; {}",
                reason.describe()
            )))
        }
    }
}

/// Generate `foo__result(handle: i64) -> T`
fn synthesize_result(layout: &AsyncFrameLayout) -> Result<MirFunction, CompileError> {
    let name = format!("{}__result", layout.func_name);
    let ret = layout.result_storage_ty.clone();
    let mut f = MirFunction::new(name, vec![MIR_I64], ret.clone());

    let handle = Local::new(1, LocalKind::Param);
    let bb0 = f.start_block;

    let result = f.add_local(LocalKind::Temp, ret.clone());
    push_frame_load_into_typed(&mut f, bb0, handle, 1, result, &ret)?;

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

    Ok(f)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn async_dispatch_registry_assigns_reserved_builtin_ordinals_then_sorted_async_functions() {
        let registry = build_async_dispatch_registry([
            "worker_b".to_string(),
            "worker_a".to_string(),
        ]);

        assert_eq!(registry.kind_id("sengoo_async_sleep"), Some(1));
        assert_eq!(registry.kind_id("sengoo_async_timeout_bool"), Some(2));
        assert_eq!(registry.kind_id("worker_a"), Some(3));
        assert_eq!(registry.kind_id("worker_b"), Some(4));
    }

    #[test]
    fn spawn_poll_dispatch_switch_targets_use_registry_ordinals() {
        let registry = build_async_dispatch_registry([
            "worker_b".to_string(),
            "worker_a".to_string(),
        ]);
        let dispatch = synthesize_spawn_poll_dispatch(
            &registry,
            &[("worker_b".to_string(), "worker_b__poll".to_string())],
        )
        .expect("spawn dispatch should synthesize with stable ordinals");

        let Some(Terminator::Switch { targets, .. }) =
            dispatch.basic_blocks[dispatch.start_block].terminator.as_ref()
        else {
            panic!("spawn poll dispatch should start with a switch terminator");
        };

        let seen: HashSet<u32> = targets.iter().map(|(kind, _)| *kind).collect();
        assert!(seen.contains(&1), "sleep builtin ordinal should be reserved");
        assert!(seen.contains(&2), "timeout builtin ordinal should be reserved");
        assert!(seen.contains(&4), "worker_b should receive stable sorted ordinal");
    }

    #[test]
    fn synthesize_result_dispatch_reports_unsupported_result_type_instead_of_panicking() {
        let registry = build_async_dispatch_registry(["worker".to_string()]);
        let err = synthesize_result_dispatch(
            &registry,
            &MIRType::Struct {
                name: "Point".to_string(),
                fields: vec![("x".to_string(), MIR_I64)],
            },
            &[("worker".to_string(), "worker__result".to_string())],
        )
        .expect_err("unsupported result dispatch types should return a compile error");

        assert!(matches!(err, CompileError::MirLower(_)));
    }

    #[test]
    fn build_async_cfg_plan_reports_missing_terminator_instead_of_panicking() {
        let mut mir_fn = MirFunction::new("main".to_string(), vec![], MIR_I64);
        mir_fn.is_async = true;

        let ready = mir_fn.add_block();
        let pending = mir_fn.add_block();
        let future_handle = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let suspend_result = mir_fn.add_local(LocalKind::Temp, MIR_I64);

        mir_fn.basic_blocks[mir_fn.start_block].set_terminator(Terminator::Suspend {
            poll_func: "child__poll".to_string(),
            future_handle,
            destination: suspend_result,
            ready_block: ready,
            pending_block: pending,
        });
        mir_fn.basic_blocks[pending].set_terminator(Terminator::Goto(pending));

        let err = build_async_cfg_plan(&mir_fn)
            .expect_err("missing terminator should return a planning error");

        assert!(
            err.describe().contains("has no terminator"),
            "missing terminator error should remain explicit"
        );
    }

    #[test]
    fn select_runtime_family_maps_scalar_types_to_expected_symbols() {
        assert_eq!(
            select_runtime_function_name(&MIRType::Int(32)).as_deref(),
            Some("sengoo_async_select_i32")
        );
        assert_eq!(
            select_result_dispatch_name(&MIRType::Float(64)).as_deref(),
            Some("sengoo_async_result_dispatch_f64")
        );
        assert_eq!(
            select_runtime_declaration(&MIR_BOOL).as_deref(),
            Some("declare i1 @sengoo_async_select_bool(i64, i64, i64, i64)\n")
        );
        assert!(select_runtime_function_name(&MIRType::Struct {
            name: "Point".to_string(),
            fields: vec![("x".to_string(), MIR_I64)],
        })
        .is_none());
    }

    #[test]
    fn expand_async_functions_supports_tuple_local_crossing_await() {
        let tuple_ty = MIRType::Tuple(vec![MIR_I64, MIR_I64]);
        let mut mir_fn = MirFunction::new("main".to_string(), vec![], MIR_I64);
        mir_fn.is_async = true;

        let tuple_local = mir_fn.add_local(LocalKind::User, tuple_ty.clone());
        let future_handle_1 = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let future_handle_2 = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let suspend_result_1 = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let suspend_result_2 = mir_fn.add_local(LocalKind::Temp, MIR_I64);

        let one = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let one_inst = mir_fn.alloc_inst(Instruction::Assign {
            destination: one,
            value: MirConstant::Int(1),
        });
        mir_fn.basic_blocks[mir_fn.start_block].push(one_inst);

        let two = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let two_inst = mir_fn.alloc_inst(Instruction::Assign {
            destination: two,
            value: MirConstant::Int(2),
        });
        mir_fn.basic_blocks[mir_fn.start_block].push(two_inst);

        let tuple_value = mir_fn.add_local(LocalKind::Temp, tuple_ty.clone());
        let aggregate = mir_fn.alloc_inst(Instruction::Aggregate {
            destination: tuple_value,
            fields: vec![one, two],
            ty: tuple_ty.clone(),
        });
        mir_fn.basic_blocks[mir_fn.start_block].push(aggregate);

        let store_tuple = mir_fn.alloc_inst(Instruction::Store {
            destination: tuple_local,
            value: tuple_value,
        });
        mir_fn.basic_blocks[mir_fn.start_block].push(store_tuple);

        let first_ready = mir_fn.add_block();
        let first_pending = mir_fn.add_block();
        let second_ready = mir_fn.add_block();
        let second_pending = mir_fn.add_block();

        mir_fn.basic_blocks[mir_fn.start_block].set_terminator(Terminator::Suspend {
            poll_func: "child1__poll".to_string(),
            future_handle: future_handle_1,
            destination: suspend_result_1,
            ready_block: first_ready,
            pending_block: first_pending,
        });
        mir_fn.basic_blocks[first_pending].set_terminator(Terminator::Goto(first_pending));

        mir_fn.basic_blocks[first_ready].set_terminator(Terminator::Suspend {
            poll_func: "child2__poll".to_string(),
            future_handle: future_handle_2,
            destination: suspend_result_2,
            ready_block: second_ready,
            pending_block: second_pending,
        });
        mir_fn.basic_blocks[second_pending].set_terminator(Terminator::Goto(second_pending));

        let loaded_tuple = mir_fn.add_local(LocalKind::Temp, tuple_ty);
        let load_tuple = mir_fn.alloc_inst(Instruction::Load {
            destination: loaded_tuple,
            source: tuple_local,
        });
        mir_fn.basic_blocks[second_ready].push(load_tuple);

        let result = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let result_inst = mir_fn.alloc_inst(Instruction::Assign {
            destination: result,
            value: MirConstant::Int(0),
        });
        mir_fn.basic_blocks[second_ready].push(result_inst);
        mir_fn.basic_blocks[second_ready].set_terminator(Terminator::Return(Some(result)));

        let mut mir_fns = vec![mir_fn];
        let helpers = expand_async_functions(&mut mir_fns)
            .expect("tuple local crossing await should now be supported");
        assert!(
            helpers.iter().any(|f| f.name == "main__poll"),
            "async expansion should synthesize main__poll when tuple locals cross await"
        );
    }

    #[test]
    fn expand_async_functions_allows_non_spilled_enum_local() {
        let enum_ty = MIRType::enum_type(MIR_I64, vec![(0, None), (1, None)]);
        let mut mir_fn = MirFunction::new("main".to_string(), vec![], MIR_I64);
        mir_fn.is_async = true;

        let _enum_local = mir_fn.add_local(LocalKind::User, enum_ty);
        let future_handle = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let suspend_result = mir_fn.add_local(LocalKind::Temp, MIR_I64);

        let ready = mir_fn.add_block();
        let pending = mir_fn.add_block();

        mir_fn.basic_blocks[mir_fn.start_block].set_terminator(Terminator::Suspend {
            poll_func: "child__poll".to_string(),
            future_handle,
            destination: suspend_result,
            ready_block: ready,
            pending_block: pending,
        });
        mir_fn.basic_blocks[pending].set_terminator(Terminator::Goto(pending));

        let result = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let result_inst = mir_fn.alloc_inst(Instruction::Assign {
            destination: result,
            value: MirConstant::Int(0),
        });
        mir_fn.basic_blocks[ready].push(result_inst);
        mir_fn.basic_blocks[ready].set_terminator(Terminator::Return(Some(result)));

        let mut mir_fns = vec![mir_fn];
        let helpers = expand_async_functions(&mut mir_fns)
            .expect("enum locals that do not cross await should not require frame storage");
        assert!(helpers.iter().any(|f| f.name == "main__poll"));
    }

    #[test]
    fn expand_async_functions_supports_spilled_payloadless_enum_local() {
        let enum_ty = MIRType::enum_type(MIR_I64, vec![(0, None), (1, None)]);
        let mut mir_fn = MirFunction::new("main".to_string(), vec![], MIR_I64);
        mir_fn.is_async = true;

        let enum_local = mir_fn.add_local(LocalKind::User, enum_ty.clone());
        let future_handle = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let suspend_result = mir_fn.add_local(LocalKind::Temp, MIR_I64);

        let ready = mir_fn.add_block();
        let pending = mir_fn.add_block();

        mir_fn.basic_blocks[mir_fn.start_block].set_terminator(Terminator::Suspend {
            poll_func: "child__poll".to_string(),
            future_handle,
            destination: suspend_result,
            ready_block: ready,
            pending_block: pending,
        });
        mir_fn.basic_blocks[pending].set_terminator(Terminator::Goto(pending));

        let loaded_enum = mir_fn.add_local(LocalKind::Temp, enum_ty);
        let load_enum = mir_fn.alloc_inst(Instruction::Load {
            destination: loaded_enum,
            source: enum_local,
        });
        mir_fn.basic_blocks[ready].push(load_enum);

        let result = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let result_inst = mir_fn.alloc_inst(Instruction::Assign {
            destination: result,
            value: MirConstant::Int(0),
        });
        mir_fn.basic_blocks[ready].push(result_inst);
        mir_fn.basic_blocks[ready].set_terminator(Terminator::Return(Some(result)));

        let mut mir_fns = vec![mir_fn];
        let helpers = expand_async_functions(&mut mir_fns)
            .expect("payloadless enum locals that cross await should now be supported");
        assert!(helpers.iter().any(|f| f.name == "main__poll"));
    }

    #[test]
    fn expand_async_functions_rejects_spilled_payload_enum_local() {
        let enum_ty = MIRType::enum_type(MIR_I64, vec![(0, Some(MIR_I64)), (1, None)]);
        let mut mir_fn = MirFunction::new("main".to_string(), vec![], MIR_I64);
        mir_fn.is_async = true;

        let enum_local = mir_fn.add_local(LocalKind::User, enum_ty.clone());
        let future_handle = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let suspend_result = mir_fn.add_local(LocalKind::Temp, MIR_I64);

        let ready = mir_fn.add_block();
        let pending = mir_fn.add_block();

        mir_fn.basic_blocks[mir_fn.start_block].set_terminator(Terminator::Suspend {
            poll_func: "child__poll".to_string(),
            future_handle,
            destination: suspend_result,
            ready_block: ready,
            pending_block: pending,
        });
        mir_fn.basic_blocks[pending].set_terminator(Terminator::Goto(pending));

        let loaded_enum = mir_fn.add_local(LocalKind::Temp, enum_ty);
        let load_enum = mir_fn.alloc_inst(Instruction::Load {
            destination: loaded_enum,
            source: enum_local,
        });
        mir_fn.basic_blocks[ready].push(load_enum);

        let result = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let result_inst = mir_fn.alloc_inst(Instruction::Assign {
            destination: result,
            value: MirConstant::Int(0),
        });
        mir_fn.basic_blocks[ready].push(result_inst);
        mir_fn.basic_blocks[ready].set_terminator(Terminator::Return(Some(result)));

        let mut mir_fns = vec![mir_fn];
        let err = expand_async_functions(&mut mir_fns)
            .expect_err("payload-carrying enum locals should still be rejected");
        let message = format!("{err}");
        assert!(
            message.contains("payload-carrying enum values cannot cross await points yet"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn expand_async_functions_supports_switch_async_cfg() {
        let mut mir_fn = MirFunction::new("main".to_string(), vec![], MIR_I64);
        mir_fn.is_async = true;

        let discr = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let discr_inst = mir_fn.alloc_inst(Instruction::Assign {
            destination: discr,
            value: MirConstant::Int(0),
        });
        mir_fn.basic_blocks[mir_fn.start_block].push(discr_inst);

        let branch_a = mir_fn.add_block();
        let branch_b = mir_fn.add_block();
        let ready_a = mir_fn.add_block();
        let pending_a = mir_fn.add_block();
        let ready_b = mir_fn.add_block();
        let pending_b = mir_fn.add_block();

        mir_fn.basic_blocks[mir_fn.start_block].set_terminator(Terminator::Switch {
            discr,
            targets: vec![(0, branch_a)],
            otherwise: branch_b,
        });

        let future_handle_a = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let suspend_result_a = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        mir_fn.basic_blocks[branch_a].set_terminator(Terminator::Suspend {
            poll_func: "a__poll".to_string(),
            future_handle: future_handle_a,
            destination: suspend_result_a,
            ready_block: ready_a,
            pending_block: pending_a,
        });
        mir_fn.basic_blocks[pending_a].set_terminator(Terminator::Goto(pending_a));

        let result_a = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let result_a_inst = mir_fn.alloc_inst(Instruction::Assign {
            destination: result_a,
            value: MirConstant::Int(10),
        });
        mir_fn.basic_blocks[ready_a].push(result_a_inst);
        mir_fn.basic_blocks[ready_a].set_terminator(Terminator::Return(Some(result_a)));

        let future_handle_b = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let suspend_result_b = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        mir_fn.basic_blocks[branch_b].set_terminator(Terminator::Suspend {
            poll_func: "b__poll".to_string(),
            future_handle: future_handle_b,
            destination: suspend_result_b,
            ready_block: ready_b,
            pending_block: pending_b,
        });
        mir_fn.basic_blocks[pending_b].set_terminator(Terminator::Goto(pending_b));

        let result_b = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let result_b_inst = mir_fn.alloc_inst(Instruction::Assign {
            destination: result_b,
            value: MirConstant::Int(20),
        });
        mir_fn.basic_blocks[ready_b].push(result_b_inst);
        mir_fn.basic_blocks[ready_b].set_terminator(Terminator::Return(Some(result_b)));

        let mut mir_fns = vec![mir_fn];
        let helpers = expand_async_functions(&mut mir_fns)
            .expect("switch-based async cfg should lower to helpers");
        let poll_fn = helpers
            .iter()
            .find(|f| f.name == "main__poll")
            .expect("main__poll helper should exist");
        let call_names = poll_fn
            .instructions
            .iter()
            .filter_map(|inst| match inst {
                Instruction::Call { func, .. } => Some(func.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(call_names.contains(&"a__poll"));
        assert!(call_names.contains(&"b__poll"));
        assert!(!call_names.contains(&"main__body"));
    }

    #[test]
    fn expand_async_functions_reports_unsupported_cfg_terminator() {
        let mut mir_fn = MirFunction::new("main".to_string(), vec![], MIR_I64);
        mir_fn.is_async = true;

        let future_handle = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let suspend_result = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let ready = mir_fn.add_block();
        let pending = mir_fn.add_block();

        mir_fn.basic_blocks[mir_fn.start_block].set_terminator(Terminator::Suspend {
            poll_func: "child__poll".to_string(),
            future_handle,
            destination: suspend_result,
            ready_block: ready,
            pending_block: pending,
        });
        mir_fn.basic_blocks[pending].set_terminator(Terminator::Goto(pending));
        mir_fn.basic_blocks[ready].set_terminator(Terminator::Break { target: ready });

        let mut mir_fns = vec![mir_fn];
        let err = expand_async_functions(&mut mir_fns)
            .expect_err("unsupported async cfg terminators should produce a diagnostic");
        let message = format!("{err}");
        assert!(
            message.contains("unsupported `break` terminator"),
            "unexpected error: {message}"
        );
        assert!(
            message.contains("self-looping pending blocks"),
            "error should describe expected async cfg shape, got: {message}"
        );
    }

    #[test]
    fn compute_live_in_user_locals_reports_unsupported_terminator() {
        let mut mir_fn = MirFunction::new("main".to_string(), vec![], MIR_I64);
        mir_fn.is_async = true;

        let start = mir_fn.start_block;
        mir_fn.basic_blocks[start].set_terminator(Terminator::Break { target: start });

        let plan = AsyncCfgPlan {
            ordered_blocks: vec![start],
            suspend_points: Vec::new(),
        };

        let err = compute_live_in_user_locals(&mir_fn, &plan)
            .expect_err("unsupported liveness terminator should return an error");
        assert!(
            err.to_string().contains("unsupported terminator in async liveness"),
            "unexpected liveness diagnostic: {err}"
        );
    }

    #[test]
    fn expand_async_functions_returns_error_instead_of_panicking_on_unmapped_local() {
        let mut mir_fn = MirFunction::new("main".to_string(), vec![], MIR_I64);
        mir_fn.is_async = true;

        let future_handle = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let suspend_result = mir_fn.add_local(LocalKind::Temp, MIR_I64);

        let ready = mir_fn.add_block();
        let pending = mir_fn.add_block();

        mir_fn.basic_blocks[mir_fn.start_block].set_terminator(Terminator::Suspend {
            poll_func: "child__poll".to_string(),
            future_handle,
            destination: suspend_result,
            ready_block: ready,
            pending_block: pending,
        });
        mir_fn.basic_blocks[pending].set_terminator(Terminator::Goto(pending));

        let bogus_local = Local::new(9999, LocalKind::Temp);
        let loaded = mir_fn.add_local(LocalKind::Temp, MIR_I64);
        let load_inst = mir_fn.alloc_inst(Instruction::Load {
            destination: loaded,
            source: bogus_local,
        });
        mir_fn.basic_blocks[ready].push(load_inst);
        mir_fn.basic_blocks[ready].set_terminator(Terminator::Return(Some(loaded)));

        let mut mir_fns = vec![mir_fn];
        let result = catch_unwind(AssertUnwindSafe(|| expand_async_functions(&mut mir_fns)));
        assert!(
            result.is_ok(),
            "async lowering should report malformed remap state as an error instead of panicking"
        );
        let err = result
            .unwrap()
            .expect_err("malformed remap state should surface as a lowering error");
        assert!(
            err.to_string().contains("missing remapped local"),
            "unexpected remap diagnostic: {err}"
        );
    }

    #[test]
    fn expand_async_functions_synthesizes_cancel_dispatch_with_builtin_cases() {
        let source = r#"
async def worker() -> i64 { 1 }
async def main() -> i64 {
    let a = worker();
    let b = sleep(1);
    let c = timeout(worker(), 2);
    0
}
"#;

        let mut mir_fns = crate::compile_to_mir(source).expect("source should lower to MIR");
        let helpers = expand_async_functions(&mut mir_fns).expect("async helpers should expand");
        let cancel_dispatch = helpers
            .iter()
            .find(|f| f.name == "sengoo_async_cancel_dispatch")
            .expect("cancel dispatch helper should exist");
        let call_names = cancel_dispatch
            .instructions
            .iter()
            .filter_map(|inst| match inst {
                Instruction::Call { func, .. } => Some(func.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(call_names.contains(&"sengoo_async_frame_free"));
        assert!(call_names.contains(&"sengoo_async_sleep__cancel"));
        assert!(call_names.contains(&"sengoo_async_timeout_bool__cancel"));
    }

    #[test]
    fn expand_async_functions_synthesizes_drop_dispatch_with_builtin_cases() {
        let source = r#"
async def worker() -> i64 { 1 }
async def main() -> i64 {
    let a = worker();
    let b = sleep(1);
    let c = timeout(worker(), 2);
    0
}
"#;

        let mut mir_fns = crate::compile_to_mir(source).expect("source should lower to MIR");
        let helpers = expand_async_functions(&mut mir_fns).expect("async helpers should expand");
        let drop_dispatch = helpers
            .iter()
            .find(|f| f.name == "sengoo_async_drop_dispatch")
            .expect("drop dispatch helper should exist");
        let call_names = drop_dispatch
            .instructions
            .iter()
            .filter_map(|inst| match inst {
                Instruction::Call { func, .. } => Some(func.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>();

        assert!(call_names.contains(&"sengoo_async_frame_free"));
        assert!(call_names.contains(&"sengoo_async_sleep__drop"));
        assert!(call_names.contains(&"sengoo_async_timeout_bool__drop"));
    }
}
