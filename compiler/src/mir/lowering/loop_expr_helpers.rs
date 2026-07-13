use super::*;

pub(super) fn lower_loop_expr(ctx: &mut LoweringContext<'_>, body: &HIRBody) -> Local {
    let loop_block = ctx.new_block();
    let exit_block = ctx.new_block();

    ctx.set_terminator(Terminator::Goto(loop_block));
    ctx.push_loop(exit_block, loop_block);
    ctx.push_drop_scope();
    ctx.lower_body_to_block_with_return(body, loop_block, false);
    ctx.pop_drop_scope(None);
    ctx.pop_loop();

    let end_block = ctx.current_block();
    if end_block != loop_block
        && ctx
            .mir_fn
            .basic_blocks
            .get(end_block)
            .is_some_and(|block| block.terminator.is_none())
    {
        ctx.set_block_terminator(end_block, Terminator::Goto(loop_block));
    }

    if ctx
        .mir_fn
        .basic_blocks
        .get(loop_block)
        .is_some_and(|block| block.terminator.is_none())
    {
        ctx.set_block_terminator(loop_block, Terminator::Goto(loop_block));
    }

    ctx.set_current_block(exit_block);
    ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn lower_loop_expr_empty_body_loops_back_to_loop_block() {
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

        let result = lower_loop_expr(&mut ctx, &HIRBody::empty());

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        let loop_block = match ctx.mir_fn.basic_blocks[start_block].terminator.clone() {
            Some(Terminator::Goto(target)) => target,
            other => panic!("expected entry goto loop block, got {:?}", other),
        };
        assert!(
            matches!(ctx.mir_fn.basic_blocks[loop_block].terminator, Some(Terminator::Goto(target)) if target == loop_block)
        );
    }
}
