use super::*;

pub(super) fn lower_for_expr(
    ctx: &mut LoweringContext<'_>,
    var_name: &str,
    iter: &HIRExpr,
    body: &HIRBody,
) -> Local {
    match iter {
        HIRExpr::Range {
            start,
            end,
            inclusive,
        } => lower_range_for_expr(
            ctx,
            var_name,
            start.as_deref(),
            end.as_deref(),
            *inclusive,
            body,
        ),
        _ => {
            let iter_local = ctx.lower_expr(iter);
            let iter_ty = ctx.get_local_type(iter_local).clone();

            match iter_ty {
                MIRType::Array(elem_ty, len) => {
                    lower_array_for_expr(ctx, var_name, iter_local, &elem_ty, len, body)
                }
                _ => {
                    ctx.errors.push(format!(
                        "for loop: unsupported iterator type: {:?}",
                        iter_ty
                    ));
                    ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
                }
            }
        }
    }
}
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
    let compare_op = if inclusive {
        MirBinOp::Le
    } else {
        MirBinOp::Lt
    };
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
    ctx.push_drop_scope();
    ctx.lower_body_to_block_with_return(body, body_block, false);
    ctx.pop_drop_scope(None);
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
pub(super) fn lower_array_for_expr(
    ctx: &mut LoweringContext<'_>,
    var_name: &str,
    iter_local: Local,
    elem_ty: &MIRType,
    len: u64,
    body: &HIRBody,
) -> Local {
    let cond_block = ctx.new_block();
    let body_block = ctx.new_block();
    let inc_block = ctx.new_block();
    let exit_block = ctx.new_block();

    let index_var = ctx.add_local(None, LocalKind::User, MIR_I64);
    let init_val = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: init_val,
        value: MirConstant::Int(0),
    });
    ctx.push_inst(Instruction::Store {
        destination: index_var,
        value: init_val,
    });

    let loop_var = ctx.add_local(Some(var_name.to_string()), LocalKind::User, elem_ty.clone());

    let len_local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: len_local,
        value: MirConstant::Int(len as i64),
    });

    ctx.set_terminator(Terminator::Goto(cond_block));

    ctx.set_current_block(cond_block);
    let index_loaded = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Load {
        destination: index_loaded,
        source: index_var,
    });

    let len_loaded = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Load {
        destination: len_loaded,
        source: len_local,
    });

    let cond_local = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: cond_local,
        op: MirBinOp::Lt,
        left: index_loaded,
        right: len_loaded,
    });

    ctx.set_terminator(Terminator::If {
        cond: cond_local,
        then_block: body_block,
        else_block: exit_block,
    });

    ctx.push_loop(exit_block, inc_block);
    ctx.push_drop_scope();
    ctx.set_current_block(body_block);

    let index_for_addr = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Load {
        destination: index_for_addr,
        source: index_var,
    });
    let elem_addr_local = ctx.add_local(
        None,
        LocalKind::Temp,
        MIRType::Ptr(Box::new(elem_ty.clone())),
    );
    ctx.push_inst(Instruction::IndexAddr {
        destination: elem_addr_local,
        base: iter_local,
        index: index_for_addr,
    });

    let elem_loaded = ctx.add_local(None, LocalKind::Temp, elem_ty.clone());
    ctx.push_inst(Instruction::Load {
        destination: elem_loaded,
        source: elem_addr_local,
    });
    ctx.push_inst(Instruction::Store {
        destination: loop_var,
        value: elem_loaded,
    });

    ctx.lower_body_to_block_with_return(body, body_block, false);
    ctx.pop_drop_scope(None);
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
        source: index_var,
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
        destination: index_var,
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
    fn lower_array_for_expr_emits_index_lt_check_and_element_load() {
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

        let iter_local = ctx.add_local(None, LocalKind::User, MIRType::Array(Box::new(MIR_I64), 3));
        let result =
            lower_array_for_expr(&mut ctx, "x", iter_local, &MIR_I64, 3, &HIRBody::empty());

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Binary {
                op: MirBinOp::Lt,
                ..
            }
        )));
        assert!(ctx
            .mir_fn
            .instructions
            .iter()
            .any(|inst| matches!(inst, Instruction::Load { .. })));
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
            Instruction::Binary {
                op: MirBinOp::Le,
                ..
            }
        )));
    }
}
