use super::*;

pub(super) fn lower_range_for_expr(
    ctx: &mut LoweringContext<'_>,
    var_name: &str,
    start: Option<&HIRExpr>,
    end: Option<&HIRExpr>,
    inclusive: bool,
    body: &HIRBody,
) -> Local {
    let cond_block = ctx.new_block();
    let body_block = ctx.new_block();
    let inc_block = ctx.new_block();
    let exit_block = ctx.new_block();

    let start_local = if let Some(s) = start {
        ctx.lower_expr(s)
    } else {
        let zero = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        ctx.push_inst(Instruction::Assign {
            destination: zero,
            value: MirConstant::Int(0),
        });
        zero
    };

    let end_local = if let Some(e) = end {
        ctx.lower_expr(e)
    } else {
        let max = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        ctx.push_inst(Instruction::Assign {
            destination: max,
            value: MirConstant::Int(i64::MAX),
        });
        max
    };

    let loop_var = ctx.add_local(Some(var_name.to_string()), LocalKind::User, MIR_I64);
    ctx.push_inst(Instruction::Store {
        destination: loop_var,
        value: start_local,
    });

    ctx.set_terminator(Terminator::Goto(cond_block));

    ctx.set_current_block(cond_block);
    let loop_var_loaded = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Load {
        destination: loop_var_loaded,
        source: loop_var,
    });

    let end_loaded = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Load {
        destination: end_loaded,
        source: end_local,
    });

    let cond_local = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    let compare_op = if inclusive { MirBinOp::Le } else { MirBinOp::Lt };
    ctx.push_inst(Instruction::Binary {
        destination: cond_local,
        op: compare_op,
        left: loop_var_loaded,
        right: end_loaded,
    });

    ctx.set_terminator(Terminator::If {
        cond: cond_local,
        then_block: body_block,
        else_block: exit_block,
    });

    ctx.push_loop(exit_block, inc_block);
    ctx.lower_body_to_block_with_return(body, body_block, false);
    ctx.pop_loop();

    if let Some(block) = ctx.mir_fn.block_mut(body_block) {
        if block.terminator.is_none() {
            block.set_terminator(Terminator::Goto(inc_block));
        }
    }

    ctx.set_current_block(inc_block);
    let inc_loaded = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Load {
        destination: inc_loaded,
        source: loop_var,
    });

    let one = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: one,
        value: MirConstant::Int(1),
    });

    let inc_result = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Binary {
        destination: inc_result,
        op: MirBinOp::Add,
        left: inc_loaded,
        right: one,
    });

    ctx.push_inst(Instruction::Store {
        destination: loop_var,
        value: inc_result,
    });

    ctx.set_terminator(Terminator::Goto(cond_block));

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
    fn lower_range_for_expr_uses_inclusive_comparison_when_requested() {
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

        let result = lower_range_for_expr(
            &mut ctx,
            "i",
            Some(&HIRExpr::Lit(HIRLiteral::Int(0))),
            Some(&HIRExpr::Lit(HIRLiteral::Int(3))),
            true,
            &HIRBody::empty(),
        );

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Binary { op: MirBinOp::Le, .. }
        )));
    }
}