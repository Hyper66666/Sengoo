use super::call_emission_helpers::emit_call_from_plan;
use super::call_invocation_helpers::build_call_invocation_plan;
use super::*;

pub(super) fn lower_non_named_call(ctx: &mut LoweringContext<'_>, arg_locals: &[Local]) -> Local {
    let plan = build_call_invocation_plan(
        "",
        &MIR_UNIT,
        None,
        arg_locals,
        &ctx.options.async_functions,
    );
    emit_call_from_plan(ctx, plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_non_named_call_uses_unit_call_placeholder() {
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

        let arg = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        let result = lower_non_named_call(&mut ctx, &[arg]);

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, args, .. } if func.is_empty() && args == &vec![arg]
        )));
    }
}