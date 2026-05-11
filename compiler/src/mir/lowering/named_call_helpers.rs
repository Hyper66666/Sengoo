use super::*;

pub(super) fn lower_named_call(
    ctx: &mut LoweringContext<'_>,
    name: &str,
    arg_locals: &[Local],
) -> Local {
    match ctx.resolve_named_call_target(name, arg_locals) {
        CallTargetResolution::Builtin(local) => local,
        CallTargetResolution::Planned(plan) => {
            let invocation = build_call_invocation_plan(
                &plan.func_name,
                &plan.ret_type,
                plan.env_ptr_local,
                arg_locals,
                &ctx.options.async_functions,
            );
            emit_call_from_plan(ctx, invocation)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_named_call_prefers_builtin_dispatch() {
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

        let task = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        let result = lower_named_call(&mut ctx, "task_status", &[task]);

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, .. } if func == "sengoo_async_task_status"
        )));
    }

    #[test]
    fn lower_named_call_wraps_async_function_start_and_tracks_origin() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::from([(
            "worker".to_string(),
            FunctionSig {
                ret_type: MIR_BOOL,
                param_count: 0,
                env: Vec::new(),
            },
        )]);
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();
        let options = MirLowerOptions {
            async_functions: ["worker".to_string()].into_iter().collect(),
            ..MirLowerOptions::default()
        };

        let start_block = mir_fn.start_block;
        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            options,
            &inherent_templates,
            &trait_templates,
        );
        ctx.set_current_block(start_block);

        let result = lower_named_call(&mut ctx, "worker", &[]);

        assert_eq!(
            ctx.get_local_type(result),
            &MIRType::Future(Box::new(MIR_BOOL))
        );
        assert_eq!(
            ctx.future_origins.get(&result).map(String::as_str),
            Some("worker")
        );
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, .. } if func == "worker__start"
        )));
    }
}
