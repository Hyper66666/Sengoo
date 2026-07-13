use super::*;

pub(super) fn lower_if_expr(
    ctx: &mut LoweringContext<'_>,
    cond: &HIRExpr,
    then_branch: &HIRBody,
    else_branch: Option<&HIRBody>,
) -> Local {
    let then_block = ctx.new_block();
    let else_block = ctx.new_block();
    let join_block = ctx.new_block();

    let cond_local = ctx.lower_expr(cond);
    ctx.set_terminator(Terminator::If {
        cond: cond_local,
        then_block,
        else_block,
    });

    let then_val = ctx.lower_scoped_body_to_block_val(then_branch, then_block);
    let then_end = ctx.current_block();
    if ctx
        .mir_fn
        .basic_blocks
        .get(then_end)
        .is_some_and(|block| block.terminator.is_none())
    {
        ctx.set_block_terminator(then_end, Terminator::Goto(join_block));
    }

    if let Some(e) = else_branch {
        let else_val = ctx.lower_scoped_body_to_block_val(e, else_block);
        let else_end = ctx.current_block();
        if ctx
            .mir_fn
            .basic_blocks
            .get(else_end)
            .is_some_and(|block| block.terminator.is_none())
        {
            ctx.set_block_terminator(else_end, Terminator::Goto(join_block));
        }

        ctx.set_current_block(join_block);
        let then_ty = ctx.get_local_type(then_val).clone();
        if is_void_like(&then_ty) {
            ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
        } else {
            let result = ctx.add_local(None, LocalKind::Temp, then_ty);
            let incoming = vec![(then_val, then_end), (else_val, else_end)];
            ctx.push_inst(Instruction::Phi {
                destination: result,
                incoming: incoming.clone(),
            });
            ctx.propagate_future_origin_through_phi(result, &incoming);
            result
        }
    } else {
        if ctx
            .mir_fn
            .basic_blocks
            .get(else_block)
            .is_some_and(|block| block.terminator.is_none())
        {
            ctx.set_block_terminator(else_block, Terminator::Goto(join_block));
        }
        ctx.set_current_block(join_block);
        ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
    }
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
    fn lower_if_expr_emits_phi_for_value_branches() {
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

        let result = lower_if_expr(
            &mut ctx,
            &HIRExpr::Lit(HIRLiteral::Bool(true)),
            &HIRBody::with_expr(HIRExpr::Lit(HIRLiteral::Int(1))),
            Some(&HIRBody::with_expr(HIRExpr::Lit(HIRLiteral::Int(2)))),
        );

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Phi { destination, incoming } if *destination == result && incoming.len() == 2
        )));
    }
}
