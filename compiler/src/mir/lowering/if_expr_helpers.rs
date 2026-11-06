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

    let initial_locals = ctx.moved_drop_locals.clone();
    let initial_fields = ctx.moved_drop_fields.clone();
    let outer_bindings = ctx.drop_bindings.clone();

    let then_val = ctx.lower_scoped_body_to_block_val(then_branch, then_block);
    let then_end = ctx.current_block();
    let then_continues = ctx
        .mir_fn
        .basic_blocks
        .get(then_end)
        .is_some_and(|block| block.terminator.is_none());
    let then_locals = ctx.moved_drop_locals.clone();
    let then_fields = ctx.moved_drop_fields.clone();
    let then_moved = outer_bindings
        .iter()
        .map(|binding| ctx.drop_binding_is_moved(binding))
        .collect::<Vec<_>>();
    if then_continues {
        ctx.set_block_terminator(then_end, Terminator::Goto(join_block));
    }

    ctx.moved_drop_locals = initial_locals;
    ctx.moved_drop_fields = initial_fields;

    let else_val = if let Some(e) = else_branch {
        ctx.lower_scoped_body_to_block_val(e, else_block)
    } else {
        ctx.set_current_block(else_block);
        ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
    };
    let else_end = ctx.current_block();
    let else_continues = ctx
        .mir_fn
        .basic_blocks
        .get(else_end)
        .is_some_and(|block| block.terminator.is_none());
    if else_continues {
        ctx.set_block_terminator(else_end, Terminator::Goto(join_block));
    }

    if then_continues && else_continues {
        // An owner transferred on one path is released on the other path,
        // before its obligation can disappear into a merged ownership set.
        for (binding, moved_then) in outer_bindings.iter().zip(then_moved) {
            let moved_else = ctx.drop_binding_is_moved(binding);
            if moved_then && !moved_else {
                ctx.push_drop_call(else_end, binding);
            }
            if moved_else && !moved_then {
                ctx.push_drop_call(then_end, binding);
            }
        }
        ctx.moved_drop_locals.extend(then_locals);
        ctx.moved_drop_fields.extend(then_fields);
    } else if then_continues {
        ctx.moved_drop_locals = then_locals;
        ctx.moved_drop_fields = then_fields;
    }

    ctx.set_current_block(join_block);
    if else_branch.is_some() {
        // Only a branch that reaches the join can determine its value type.
        // In particular, `if c { return x; } else { y }` has y's type.
        let result_ty = if then_continues {
            ctx.get_local_type(then_val).clone()
        } else if else_continues {
            ctx.get_local_type(else_val).clone()
        } else {
            MIR_UNIT
        };
        if is_void_like(&result_ty) {
            ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
        } else {
            let result = ctx.add_local(None, LocalKind::Temp, result_ty);
            let incoming = [
                (then_val, then_end, then_continues),
                (else_val, else_end, else_continues),
            ]
            .into_iter()
            .filter_map(|(value, block, continues)| continues.then_some((value, block)))
            .collect::<Vec<_>>();
            ctx.push_inst(Instruction::Phi {
                destination: result,
                incoming: incoming.clone(),
            });
            ctx.propagate_future_origin_through_phi(result, &incoming);
            result
        }
    } else {
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
