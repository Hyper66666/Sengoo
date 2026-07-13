use super::call_invocation_helpers::CallInvocationPlan;
use super::*;

fn runtime_async_wrapper_name(func_name: &str) -> &str {
    const GENERIC_WRAPPERS: &[&str] = &["raw_mutex_lock_async"];

    GENERIC_WRAPPERS
        .iter()
        .copied()
        .find(|wrapper| {
            func_name == *wrapper
                || func_name
                    .strip_prefix(*wrapper)
                    .is_some_and(|suffix| suffix.starts_with('_'))
        })
        .unwrap_or(func_name)
}

fn runtime_async_wrapper_origin(func_name: &str) -> Option<&'static str> {
    match runtime_async_wrapper_name(func_name) {
        "spawn_blocking_future_i64" => Some("sengoo_async_spawn_blocking_i64"),
        "channel_send_i64" => Some("sengoo_async_channel_send_i64"),
        "channel_recv_i64" => Some("sengoo_async_channel_recv_i64"),
        "mutex_lock_async" => Some("sengoo_async_mutex_lock_i64"),
        "raw_mutex_lock_async" => Some("sengoo_async_mutex_lock"),
        "HttpServer_next_request_async" => Some("sengoo_http_server_next_request_async"),
        _ => None,
    }
}

fn runtime_async_wrapper_future_ty(func_name: &str) -> Option<MIRType> {
    match runtime_async_wrapper_name(func_name) {
        "spawn_blocking_future_i64" => Some(MIRType::Future(Box::new(MIR_I64))),
        "channel_send_i64" => Some(MIRType::Future(Box::new(MIRType::Struct {
            name: "ChannelSendOutcome".to_string(),
            fields: vec![
                ("is_ok".to_string(), MIR_BOOL),
                ("error".to_string(), MIR_I64),
            ],
        }))),
        "channel_recv_i64" => Some(MIRType::Future(Box::new(MIRType::Struct {
            name: "ChannelRecvOutcome".to_string(),
            fields: vec![
                ("is_ok".to_string(), MIR_BOOL),
                ("value".to_string(), MIR_I64),
                ("error".to_string(), MIR_I64),
            ],
        }))),
        "mutex_lock_async" => Some(MIRType::Future(Box::new(MIRType::Struct {
            name: "MutexLockOutcome".to_string(),
            fields: vec![
                ("is_ok".to_string(), MIR_BOOL),
                ("value".to_string(), MIR_I64),
                ("error".to_string(), MIR_I64),
            ],
        }))),
        "raw_mutex_lock_async" => Some(MIRType::Future(Box::new(MIR_I64))),
        "HttpServer_next_request_async" => Some(MIRType::Future(Box::new(
            http_server_next_request_outcome_mir_type(),
        ))),
        _ => None,
    }
}

fn runtime_async_start_origin(func_name: &str) -> Option<String> {
    func_name
        .strip_suffix("__start")
        .filter(|name| name.starts_with("sengoo_async_"))
        .map(|name| name.to_string())
}

pub(super) fn emit_call_from_plan(
    ctx: &mut LoweringContext<'_>,
    plan: CallInvocationPlan,
) -> Local {
    let CallInvocationPlan {
        actual_func,
        mut local_ty,
        final_args,
        mut future_origin,
        struct_type_name,
    } = plan;

    if let Some(origin) = runtime_async_wrapper_origin(&actual_func) {
        future_origin = Some(origin.to_string());
        if let Some(future_ty) = runtime_async_wrapper_future_ty(&actual_func) {
            local_ty = future_ty;
        }
    } else if let Some(origin) = runtime_async_start_origin(&actual_func) {
        future_origin = Some(origin);
    }

    let local = ctx.add_local(None, LocalKind::Temp, local_ty);
    if let Some(type_name) = struct_type_name {
        ctx.type_names.insert(local, type_name);
    }
    ctx.push_inst(Instruction::Call {
        destination: local,
        func: actual_func,
        args: final_args,
    });
    if let Some(origin) = future_origin {
        ctx.future_origins.insert(local, origin);
    }
    local
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_call_from_plan_tracks_async_future_origin() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::new();
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();

        let start_block = mir_fn.start_block;
        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            MirLowerOptions::default(),
            &inherent_templates,
            &trait_templates,
        );
        ctx.set_current_block(start_block);

        let result = emit_call_from_plan(
            &mut ctx,
            CallInvocationPlan {
                actual_func: "worker__start".to_string(),
                local_ty: MIRType::Future(Box::new(MIR_BOOL)),
                final_args: vec![],
                future_origin: Some("worker".to_string()),
                struct_type_name: None,
            },
        );

        assert_eq!(
            ctx.future_origins.get(&result).map(String::as_str),
            Some("worker")
        );
    }

    #[test]
    fn emit_call_from_plan_tracks_monomorphized_runtime_async_wrapper() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::new();
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();

        let start_block = mir_fn.start_block;
        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            MirLowerOptions::default(),
            &inherent_templates,
            &trait_templates,
        );
        ctx.set_current_block(start_block);

        let result = emit_call_from_plan(
            &mut ctx,
            CallInvocationPlan {
                actual_func: "raw_mutex_lock_async_Payload".to_string(),
                local_ty: MIR_I64,
                final_args: vec![],
                future_origin: None,
                struct_type_name: None,
            },
        );

        assert_eq!(
            ctx.future_origins.get(&result).map(String::as_str),
            Some("sengoo_async_mutex_lock")
        );
        assert_eq!(
            ctx.get_local_type(result),
            &MIRType::Future(Box::new(MIR_I64))
        );
    }

    #[test]
    fn emit_call_from_plan_tracks_sync_struct_type_name() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::new();
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();

        let start_block = mir_fn.start_block;
        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            MirLowerOptions::default(),
            &inherent_templates,
            &trait_templates,
        );
        ctx.set_current_block(start_block);

        let result = emit_call_from_plan(
            &mut ctx,
            CallInvocationPlan {
                actual_func: "pair_make".to_string(),
                local_ty: MIRType::Struct {
                    name: "Pair".to_string(),
                    fields: vec![("value".to_string(), MIR_I64)],
                },
                final_args: vec![],
                future_origin: None,
                struct_type_name: Some("Pair".to_string()),
            },
        );

        assert_eq!(
            ctx.type_names.get(&result).map(String::as_str),
            Some("Pair")
        );
    }
}
