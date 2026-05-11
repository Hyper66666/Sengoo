use super::call_invocation_helpers::CallInvocationPlan;
use super::*;

pub(super) fn emit_call_from_plan(
    ctx: &mut LoweringContext<'_>,
    plan: CallInvocationPlan,
) -> Local {
    let CallInvocationPlan {
        actual_func,
        local_ty,
        final_args,
        future_origin,
        struct_type_name,
    } = plan;

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
