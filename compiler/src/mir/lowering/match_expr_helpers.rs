use super::*;
use crate::hir::HIRMatchArm;

pub(super) fn lower_match_expr(
    ctx: &mut LoweringContext<'_>,
    scrutinee: &HIRExpr,
    arms: &[HIRMatchArm],
) -> Local {
    let scrutinee_local = ctx.lower_expr(scrutinee);
    let scrutinee_ty = ctx.get_local_type(scrutinee_local).clone();
    let has_guards = arms.iter().any(|arm| arm.guard.is_some());

    match scrutinee_ty {
        MIRType::Enum { .. } if !has_guards => lower_enum_match_expr(ctx, scrutinee_local, arms),
        _ => lower_non_enum_match_expr(ctx, scrutinee_local, arms),
    }
}

fn lower_unguarded_arm_header(
    ctx: &mut LoweringContext<'_>,
    arm: &HIRMatchArm,
    scrutinee_local: Local,
    then_block: usize,
    else_block: usize,
) {
    let cond = ctx.matches_pattern(&arm.pat, scrutinee_local);
    ctx.set_terminator(Terminator::If {
        cond,
        then_block,
        else_block,
    });
}

fn lower_guarded_arm_header(
    ctx: &mut LoweringContext<'_>,
    arm: &HIRMatchArm,
    scrutinee_local: Local,
    then_block: usize,
    else_block: usize,
) {
    let guard_block = ctx.new_block();
    let pattern_ok = ctx.matches_pattern(&arm.pat, scrutinee_local);
    ctx.set_terminator(Terminator::If {
        cond: pattern_ok,
        then_block: guard_block,
        else_block,
    });

    ctx.set_current_block(guard_block);
    ctx.lower_pattern_bindings(&arm.pat, scrutinee_local);
    let guard_ok = ctx.lower_expr(arm.guard.as_ref().expect("guarded arm"));
    ctx.set_terminator(Terminator::If {
        cond: guard_ok,
        then_block,
        else_block,
    });
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
    let unreachable_block = ctx.new_block();

    let switch_plan = build_match_switch_plan(arms, &arm_blocks, unreachable_block);

    ctx.set_terminator(Terminator::Switch {
        discr: discr_local,
        targets: switch_plan.targets,
        otherwise: switch_plan.otherwise_block,
    });
    ctx.set_block_terminator(unreachable_block, Terminator::Unreachable);

    let mut incoming_values: Vec<(Local, usize)> = Vec::new();
    for (i, arm) in arms.iter().enumerate() {
        ctx.set_current_block(arm_blocks[i]);
        ctx.lower_pattern_bindings(&arm.pat, scrutinee_local);
        let arm_result = ctx.lower_expr(&arm.body);
        let arm_end = ctx.current_block();

        if ctx
            .mir_fn
            .basic_blocks
            .get(arm_end)
            .is_some_and(|block| block.terminator.is_none())
        {
            ctx.set_block_terminator(arm_end, Terminator::Goto(join_block));
            incoming_values.push((arm_result, arm_end));
        }
    }

    ctx.set_current_block(join_block);
    phi_join(ctx, incoming_values)
}

pub(super) fn lower_non_enum_match_expr(
    ctx: &mut LoweringContext<'_>,
    scrutinee_local: Local,
    arms: &[HIRMatchArm],
) -> Local {
    let join_block = ctx.new_block();
    let mut incoming_values: Vec<(Local, usize)> = Vec::new();

    for (i, arm) in arms.iter().enumerate() {
        let is_last = i == arms.len() - 1;

        if is_last && arm.guard.is_none() {
            ctx.lower_pattern_bindings(&arm.pat, scrutinee_local);
            let arm_result = ctx.lower_expr(&arm.body);
            let arm_end = ctx.current_block();
            if ctx
                .mir_fn
                .basic_blocks
                .get(arm_end)
                .is_some_and(|block| block.terminator.is_none())
            {
                ctx.set_block_terminator(arm_end, Terminator::Goto(join_block));
                incoming_values.push((arm_result, arm_end));
            }
            continue;
        }

        let body_block = ctx.new_block();
        let else_block = if is_last { join_block } else { ctx.new_block() };

        if arm.guard.is_some() {
            lower_guarded_arm_header(ctx, arm, scrutinee_local, body_block, else_block);
        } else {
            lower_unguarded_arm_header(ctx, arm, scrutinee_local, body_block, else_block);
        }

        ctx.set_current_block(body_block);
        if arm.guard.is_none() {
            ctx.lower_pattern_bindings(&arm.pat, scrutinee_local);
        }
        let arm_result = ctx.lower_expr(&arm.body);
        let arm_end = ctx.current_block();
        if ctx
            .mir_fn
            .basic_blocks
            .get(arm_end)
            .is_some_and(|block| block.terminator.is_none())
        {
            ctx.set_block_terminator(arm_end, Terminator::Goto(join_block));
            incoming_values.push((arm_result, arm_end));
        }

        if !is_last {
            ctx.set_current_block(else_block);
        }
    }

    ctx.set_current_block(join_block);
    phi_join(ctx, incoming_values)
}

fn phi_join(ctx: &mut LoweringContext<'_>, incoming_values: Vec<(Local, usize)>) -> Local {
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
