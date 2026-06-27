use super::*;

pub(super) fn lower_while_expr(
    ctx: &mut LoweringContext<'_>,
    cond: &HIRExpr,
    body: &HIRBody,
) -> Local {
    let cond_block = ctx.new_block();
    let body_block = ctx.new_block();
    let exit_block = ctx.new_block();

    ctx.set_terminator(Terminator::Goto(cond_block));

    ctx.set_current_block(cond_block);
    let cond_local = ctx.lower_expr(cond);
    ctx.set_terminator(Terminator::If {
        cond: cond_local,
        then_block: body_block,
        else_block: exit_block,
    });

    ctx.push_loop(exit_block, cond_block);
    ctx.push_drop_scope();
    ctx.lower_body_to_block_with_return(body, body_block, false);
    ctx.pop_drop_scope(None);
    ctx.pop_loop();

    let body_end_block = ctx.current_block();
    if body_end_block != body_block {
        if let Some(block) = ctx.mir_fn.block_mut(body_end_block) {
            if block.terminator.is_none() {
                block.set_terminator(Terminator::Goto(cond_block));
            }
        }
    }
    if let Some(block) = ctx.mir_fn.block_mut(body_block) {
        if block.terminator.is_none() {
            block.set_terminator(Terminator::Goto(cond_block));
        }
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
    fn lower_while_expr_empty_body_loops_back_to_condition_block() {
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

        let result = lower_while_expr(
            &mut ctx,
            &HIRExpr::Lit(HIRLiteral::Bool(true)),
            &HIRBody::empty(),
        );

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        let cond_block = match ctx.mir_fn.basic_blocks[start_block].terminator.clone() {
            Some(Terminator::Goto(target)) => target,
            other => panic!("expected entry goto cond block, got {:?}", other),
        };
        let (body_block, exit_block) = match ctx.mir_fn.basic_blocks[cond_block].terminator.clone()
        {
            Some(Terminator::If {
                then_block,
                else_block,
                ..
            }) => (then_block, else_block),
            other => panic!("expected while condition branch, got {:?}", other),
        };
        assert!(matches!(
            ctx.mir_fn.basic_blocks[body_block].terminator,
            Some(Terminator::Goto(target)) if target == cond_block
        ));
        assert_eq!(ctx.current_block(), exit_block);
    }
}
