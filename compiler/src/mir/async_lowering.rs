//! Async lowering: synthesize frame-backed __start/__poll/__result helpers
//!
//! For each `async def foo(params...) -> T`, we generate three helper functions:
//!   - `foo__start(params...) -> i64`  鈥?allocates a frame, stores params, sets state=0, returns handle
//!   - `foo__poll(handle: i64) -> i64`  鈥?runs until next suspend or completion, returns 0=pending, 1=ready
//!   - `foo__result(handle: i64) -> T`  鈥?reads the result from the frame, frees it, returns T

#[cfg(test)]
use super::async_cfg_helpers::AsyncCfgPlan;
use super::async_cfg_helpers::{
    build_async_cfg_plan, collect_spill_user_locals, collect_user_locals,
    compute_live_in_user_locals,
};
use super::async_dispatch_helpers::{
    build_async_dispatch_registry, build_async_dispatch_registry_with_extras,
    OPTIONAL_ASYNC_DISPATCH_NAMES,
};
use super::async_dispatch_synthesis_helpers::{
    select_cancel_n_winner_runtime_function_name, select_cancel_winner_runtime_function_name,
    select_n_winner_runtime_function_name, select_result_runtime_suffix,
    select_winner_runtime_function_name, synthesize_result_dispatch,
    synthesize_spawn_cancel_dispatch, synthesize_spawn_drop_dispatch,
    synthesize_spawn_poll_dispatch,
};
#[cfg(test)]
use super::async_dispatch_synthesis_helpers::{
    select_result_dispatch_name, select_runtime_declaration, select_runtime_function_name,
};
use super::async_entry_helpers::{
    count_await_points, synthesize_async_main_wrapper, synthesize_result, synthesize_start,
};
use super::async_frame_helpers::{
    build_async_frame_layout, push_frame_load_into_or_value_typed, push_frame_store_typed,
    AsyncFrameLayout,
};
use super::async_poll_helpers::{collect_rebasable_pointer_locals, synthesize_cfg_poll};
use crate::mir::{
    Instruction, Local, LocalKind, MIRType, MirConstant, MirFunction, Terminator, MIR_BOOL,
    MIR_I64, MIR_UNIT,
};
use crate::CompileError;
use std::collections::{BTreeMap, HashSet};

/// Given a list of MIR functions, expand each async function into its original body
/// plus three synthesized helpers. Returns additional functions to add.
///
/// For async `main`, the original body is renamed to `main__body` and a new
/// `main` wrapper is generated that drives the async helpers.
pub fn expand_async_functions(
    mir_fns: &mut [MirFunction],
) -> Result<Vec<MirFunction>, CompileError> {
    let async_fn_names: Vec<String> = mir_fns
        .iter()
        .filter(|f| f.is_async)
        .map(|f| f.name.clone())
        .collect();
    let needs_optional_async_dispatch = mir_fns.iter().any(|mir_fn| {
        mir_fn.instructions.iter().any(|inst| match inst {
            Instruction::Call { func, .. } => OPTIONAL_ASYNC_DISPATCH_NAMES
                .iter()
                .any(|name| func == &format!("{name}__start")),
            _ => false,
        })
    });
    let dispatch_registry = if needs_optional_async_dispatch {
        build_async_dispatch_registry_with_extras(
            async_fn_names.iter().cloned(),
            OPTIONAL_ASYNC_DISPATCH_NAMES,
        )
    } else {
        build_async_dispatch_registry(async_fn_names.iter().cloned())
    };

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
        let mut spill_user_locals = if await_count == 0 {
            Vec::new()
        } else if let Ok(plan) = build_async_cfg_plan(mir_fn) {
            let live_in = compute_live_in_user_locals(mir_fn, &plan)?;
            collect_spill_user_locals(&plan, &user_locals, &live_in)
        } else {
            Vec::new()
        };
        if await_count != 0 {
            let rebase_pointer_locals =
                collect_rebasable_pointer_locals(mir_fn, &spill_user_locals);
            if !rebase_pointer_locals.is_empty() {
                let mut needed = spill_user_locals
                    .iter()
                    .map(|(local, _)| *local)
                    .collect::<HashSet<_>>();
                needed.extend(rebase_pointer_locals.values().copied());
                spill_user_locals = user_locals
                    .iter()
                    .filter(|(local, _)| needed.contains(local))
                    .cloned()
                    .collect();
            }
        }
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

    let needs_select_runtime = mir_fns.iter().any(|mir_fn| {
        mir_fn.instructions.iter().any(|inst| match inst {
            Instruction::Call { func, .. } => {
                func == select_winner_runtime_function_name()
                    || func == select_n_winner_runtime_function_name()
                    || func == select_cancel_winner_runtime_function_name()
                    || func == select_cancel_n_winner_runtime_function_name()
                    || func.starts_with("sengoo_async_select_")
            }
            _ => false,
        })
    });
    // The native Rust staticlib groups all scalar select entry points into one
    // archive member. Unix linkers therefore require every dispatch symbol once
    // any async runtime entry point pulls that member into the executable.
    if needs_select_runtime || !spawn_dispatch_entries.is_empty() {
        for (suffix, return_ty) in [
            ("bool", MIR_BOOL),
            ("i8", MIRType::Int(8)),
            ("i16", MIRType::Int(16)),
            ("i32", MIRType::Int(32)),
            ("i64", MIR_I64),
            ("f32", MIRType::Float(32)),
            ("f64", MIRType::Float(64)),
        ] {
            result_dispatch_entries
                .entry(suffix.to_string())
                .or_insert_with(|| (return_ty, Vec::new()));
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

        // Fresh frames start at state 0 and execute the body. Completed frames
        // should report ready without re-running no-await async bodies.
        f.basic_blocks[bb0].set_terminator(Terminator::Switch {
            discr: state,
            targets: vec![(0, body_block)],
            otherwise: done_block,
        });

        // Body block: load params, call original, store result
        let result_val = f.add_local(LocalKind::Temp, result_storage_ty.clone());

        // Load params from frame
        let mut param_locals = Vec::new();
        for i in 0..layout.param_types.len() {
            let p = f.add_local(LocalKind::Temp, layout.param_types[i].clone());
            let loaded = push_frame_load_into_or_value_typed(
                &mut f,
                body_block,
                handle,
                layout.param_offsets[i],
                p,
                &layout.param_types[i],
            )?;
            param_locals.push(loaded);
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
        Ok(plan) => synthesize_cfg_poll(layout, mir_fn, &plan, spill_user_locals),
        Err(reason) => {
            let _ = (bb0, state, result_storage_ty, n_states);
            Err(CompileError::Codegen(format!(
                "async frame lowering requires await control flow that can be expressed with suspend points, self-looping pending blocks, and goto/if/switch/return/unreachable edges; {}",
                reason.describe()
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn async_dispatch_registry_assigns_reserved_builtins_and_order_independent_stable_ids() {
        let first = build_async_dispatch_registry(["worker_b".to_string(), "worker_a".to_string()]);
        let second =
            build_async_dispatch_registry(["worker_a".to_string(), "worker_b".to_string()]);

        assert_eq!(first.kind_id("sengoo_async_sleep"), Some(1));
        assert_eq!(first.kind_id("sengoo_async_timeout_bool"), Some(2));
        assert_eq!(first.kind_id("worker_a"), second.kind_id("worker_a"));
        assert_eq!(first.kind_id("worker_b"), second.kind_id("worker_b"));
        assert_ne!(first.kind_id("worker_a"), first.kind_id("worker_b"));
    }

    #[test]
    fn spawn_poll_dispatch_switch_targets_use_registry_ordinals() {
        let registry =
            build_async_dispatch_registry(["worker_b".to_string(), "worker_a".to_string()]);
        let dispatch = synthesize_spawn_poll_dispatch(
            &registry,
            &[("worker_b".to_string(), "worker_b__poll".to_string())],
        )
        .expect("spawn dispatch should synthesize with stable ordinals");

        let Some(Terminator::Switch { targets, .. }) = dispatch.basic_blocks[dispatch.start_block]
            .terminator
            .as_ref()
        else {
            panic!("spawn poll dispatch should start with a switch terminator");
        };

        let seen: HashSet<u32> = targets.iter().map(|(kind, _)| *kind).collect();
        assert!(
            seen.contains(&1),
            "sleep builtin ordinal should be reserved"
        );
        assert!(
            seen.contains(&2),
            "timeout builtin ordinal should be reserved"
        );
        let worker_kind = u32::try_from(
            registry
                .kind_id("worker_b")
                .expect("worker_b should have a collision-free stable id"),
        )
        .expect("worker_b kind should fit the MIR switch width");
        assert!(seen.contains(&worker_kind));
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
    fn expand_async_functions_supports_spilled_payload_enum_local() {
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
        let helpers = expand_async_functions(&mut mir_fns)
            .expect("payload-carrying enum locals now spill word-wise across awaits");
        let poll = helpers
            .iter()
            .find(|f| f.name == "main__poll")
            .expect("poll helper should be generated");
        // The spill uses one discriminant slot plus one payload word: both a
        // Discriminant read (store side) and an enum Aggregate rebuild (load
        // side) must appear in the poll body.
        let has_discriminant = poll
            .instructions
            .iter()
            .any(|inst| matches!(inst, Instruction::Discriminant { .. }));
        let rebuilds_enum = poll.instructions.iter().any(|inst| {
            matches!(
                inst,
                Instruction::Aggregate {
                    ty: MIRType::Enum { .. },
                    ..
                }
            )
        });
        assert!(
            has_discriminant && rebuilds_enum,
            "expected word-wise enum spill in poll body"
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
            err.to_string()
                .contains("unsupported terminator in async liveness"),
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
