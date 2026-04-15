use crate::hir::HIRMatchArm;
use super::*;

pub(super) fn lower_match_expr(
    ctx: &mut LoweringContext<'_>,
    scrutinee: &HIRExpr,
    arms: &[HIRMatchArm],
) -> Local {
    let scrutinee_local = ctx.lower_expr(scrutinee);
    let scrutinee_ty = ctx.get_local_type(scrutinee_local).clone();

    match scrutinee_ty {
        MIRType::Enum { .. } => lower_enum_match_expr(ctx, scrutinee_local, arms),
        _ => lower_non_enum_match_expr(ctx, scrutinee_local, arms),
    }
}
pub(super) fn lower_enum_match_expr(
    ctx: &mut LoweringContext<'_>,
    scrutinee_local: Local,
    arms: &[HIRMatchArm],
) -> Local {
    let discr_local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Discriminant {
        destination: discr_local,
        source: scrutinee_local,
    });

    let arm_blocks: Vec<usize> = arms.iter().map(|_| ctx.new_block()).collect();
    let join_block = ctx.new_block();

    let switch_plan = build_match_switch_plan(arms, &arm_blocks, join_block);

    ctx.set_terminator(Terminator::Switch {
        discr: discr_local,
        targets: switch_plan.targets,
        otherwise: switch_plan.otherwise_block,
    });

    let mut incoming_values: Vec<(Local, usize)> = Vec::new();
    for (i, arm) in arms.iter().enumerate() {
        let arm_block = arm_blocks[i];
        ctx.set_current_block(arm_block);

        ctx.lower_pattern_bindings(&arm.pat, scrutinee_local);
        let arm_result = ctx.lower_expr(&arm.body);
        let arm_end = ctx.current_block();

        if let Some(block) = ctx.mir_fn.block_mut(arm_end) {
            if block.terminator.is_none() {
                block.set_terminator(Terminator::Goto(join_block));
                incoming_values.push((arm_result, arm_end));
            }
        }
    }

    ctx.set_current_block(join_block);
    if let Some((first_value, _)) = incoming_values.first().copied() {
        let result_ty = ctx.get_local_type(first_value).clone();
        if is_void_like(&result_ty) {
            ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
        } else {
            let result = ctx.add_local(None, LocalKind::Temp, result_ty);
            ctx.push_inst(Instruction::Phi {
                destination: result,
                incoming: incoming_values.clone(),
            });
            ctx.propagate_future_origin_through_phi(result, &incoming_values);
            result
        }
    } else {
        ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
    }
}

#[allow(dead_code)]
pub(super) fn lower_non_enum_match_expr(
    ctx: &mut LoweringContext<'_>,
    scrutinee_local: Local,
    arms: &[HIRMatchArm],
) -> Local {
    let join_block = ctx.new_block();
    let mut incoming_values: Vec<(Local, usize)> = Vec::new();

    for (i, arm) in arms.iter().enumerate() {
        let is_last = i == arms.len() - 1;

        if is_last {
            let arm_result = ctx.lower_expr(&arm.body);
            let arm_end = ctx.current_block();
            if let Some(block) = ctx.mir_fn.block_mut(arm_end) {
                if block.terminator.is_none() {
                    block.set_terminator(Terminator::Goto(join_block));
                    incoming_values.push((arm_result, arm_end));
                }
            }
        } else {
            let then_block = ctx.new_block();
            let next_arm_block = ctx.new_block();

            let should_take = ctx.matches_pattern(&arm.pat, scrutinee_local);
            ctx.set_terminator(Terminator::If {
                cond: should_take,
                then_block,
                else_block: next_arm_block,
            });

            ctx.set_current_block(then_block);
            let arm_result = ctx.lower_expr(&arm.body);
            let arm_end = ctx.current_block();
            if let Some(block) = ctx.mir_fn.block_mut(arm_end) {
                if block.terminator.is_none() {
                    block.set_terminator(Terminator::Goto(join_block));
                    incoming_values.push((arm_result, arm_end));
                }
            }

            ctx.set_current_block(next_arm_block);
        }
    }

    ctx.set_current_block(join_block);
    if let Some((first_value, _)) = incoming_values.first().copied() {
        let result_ty = ctx.get_local_type(first_value).clone();
        if is_void_like(&result_ty) {
            ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
        } else {
            let result = ctx.add_local(None, LocalKind::Temp, result_ty);
            ctx.push_inst(Instruction::Phi {
                destination: result,
                incoming: incoming_values.clone(),
            });
            ctx.propagate_future_origin_through_phi(result, &incoming_values);
            result
        }
    } else {
        ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::{HIRMatchArm, HIRPattern};

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
    fn lower_match_expr_routes_non_enum_scrutinee_to_if_chain_helper() {
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

        let scrutinee = ctx.add_local(None, LocalKind::User, MIR_I64);
        ctx.local_names.insert("value".to_string(), scrutinee);
        ctx.bind_local_symbol(SymbolId::new(9), scrutinee);
        let arms = vec![
            HIRMatchArm::new(HIRPattern::Lit(HIRLiteral::Int(0)), HIRExpr::Lit(HIRLiteral::Int(10))),
            HIRMatchArm::new(HIRPattern::Wild, HIRExpr::Lit(HIRLiteral::Int(20))),
        ];

        let result = lower_match_expr(
            &mut ctx,
            &HIRExpr::Var {
                name: "value".to_string(),
                symbol: SymbolId::new(9),
            },
            &arms,
        );

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx
            .mir_fn
            .basic_blocks
            .iter()
            .any(|block| matches!(block.terminator, Some(Terminator::If { .. }))));
    }
    #[test]
    fn lower_non_enum_match_expr_emits_if_chain_and_phi() {
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

        let scrutinee = ctx.add_local(None, LocalKind::User, MIR_I64);
        let arms = vec![
            HIRMatchArm::new(HIRPattern::Lit(HIRLiteral::Int(0)), HIRExpr::Lit(HIRLiteral::Int(10))),
            HIRMatchArm::new(HIRPattern::Wild, HIRExpr::Lit(HIRLiteral::Int(20))),
        ];

        let result = lower_non_enum_match_expr(&mut ctx, scrutinee, &arms);

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Phi { destination, incoming } if *destination == result && incoming.len() == 2
        )));
        assert!(ctx
            .mir_fn
            .basic_blocks
            .iter()
            .any(|block| matches!(block.terminator, Some(Terminator::If { .. }))));
    }

    #[test]
    fn lower_enum_match_expr_emits_switch_and_phi() {
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

        let scrutinee = ctx.add_local(
            None,
            LocalKind::User,
            MIRType::Enum {
                discr_type: Box::new(MIR_I64),
                variants: vec![(0, None), (1, None)],
            },
        );
        let arms = vec![
            HIRMatchArm::new(HIRPattern::Lit(HIRLiteral::Int(0)), HIRExpr::Lit(HIRLiteral::Int(10))),
            HIRMatchArm::new(HIRPattern::Wild, HIRExpr::Lit(HIRLiteral::Int(20))),
        ];

        let result = lower_enum_match_expr(&mut ctx, scrutinee, &arms);

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Phi { destination, incoming } if *destination == result && incoming.len() == 2
        )));
        assert!(matches!(ctx.mir_fn.basic_blocks[start_block].terminator, Some(Terminator::Switch { .. })));
    }
}