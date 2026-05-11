use super::*;

pub(super) fn lower_break_expr(ctx: &mut LoweringContext<'_>, value: Option<&HIRExpr>) -> Local {
    if let Some(target) = ctx.get_break_target() {
        if let Some(v) = value {
            ctx.lower_expr(v);
        }
        ctx.set_terminator(Terminator::Break { target });
        ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
    } else {
        ctx.errors.push("break outside of loop".to_string());
        ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
    }
}

pub(super) fn lower_continue_expr(ctx: &mut LoweringContext<'_>) -> Local {
    if let Some(target) = ctx.get_continue_target() {
        ctx.set_terminator(Terminator::Continue { target });
        ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
    } else {
        ctx.errors.push("continue outside of loop".to_string());
        ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir;

    type TestCtxParts = (
        MirFunction,
        usize,
        HashSet<String>,
        HashMap<String, FunctionSig>,
        HashMap<String, &'static hir::HIRStruct>,
        Vec<InherentMethodTemplate>,
        Vec<TraitMethodTemplate>,
    );

    fn make_ctx() -> TestCtxParts {
        (
            MirFunction::new("test".to_string(), vec![], MIR_UNIT),
            0usize,
            HashSet::new(),
            HashMap::new(),
            HashMap::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    #[test]
    fn lower_break_expr_sets_break_terminator_inside_loop() {
        let (
            mut mir_fn,
            mut lambda_counter,
            known_functions,
            function_sigs,
            struct_defs,
            inherent_templates,
            trait_templates,
        ) = make_ctx();
        let start_block = mir_fn.start_block;
        let break_block = mir_fn.add_block();
        let continue_block = mir_fn.add_block();

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
        ctx.push_loop(break_block, continue_block);

        let result = lower_break_expr(&mut ctx, None);

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        assert!(
            matches!(ctx.mir_fn.basic_blocks[start_block].terminator, Some(Terminator::Break { target }) if target == break_block)
        );
    }

    #[test]
    fn lower_break_expr_records_error_outside_loop() {
        let (
            mut mir_fn,
            mut lambda_counter,
            known_functions,
            function_sigs,
            struct_defs,
            inherent_templates,
            trait_templates,
        ) = make_ctx();
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

        let result = lower_break_expr(&mut ctx, None);

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        assert!(ctx.errors.iter().any(|e| e == "break outside of loop"));
    }

    #[test]
    fn lower_continue_expr_sets_continue_terminator_inside_loop() {
        let (
            mut mir_fn,
            mut lambda_counter,
            known_functions,
            function_sigs,
            struct_defs,
            inherent_templates,
            trait_templates,
        ) = make_ctx();
        let start_block = mir_fn.start_block;
        let break_block = mir_fn.add_block();
        let continue_block = mir_fn.add_block();

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
        ctx.push_loop(break_block, continue_block);

        let result = lower_continue_expr(&mut ctx);

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        assert!(
            matches!(ctx.mir_fn.basic_blocks[start_block].terminator, Some(Terminator::Continue { target }) if target == continue_block)
        );
    }
}
