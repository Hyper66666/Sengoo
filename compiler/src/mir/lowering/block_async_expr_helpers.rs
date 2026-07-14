use super::method_call_helpers::lower_method_call_from_locals;
use super::*;

pub(super) fn lower_block_expr(ctx: &mut LoweringContext<'_>, body: &HIRBody) -> Local {
    let entry_block = ctx.new_block();
    ctx.set_terminator(Terminator::Goto(entry_block));
    ctx.lower_scoped_body_to_block_val(body, entry_block)
}

pub(super) fn lower_await_expr(ctx: &mut LoweringContext<'_>, inner: &HIRExpr) -> Local {
    let future_handle = ctx.lower_expr(inner);
    if ctx.future_origins.contains_key(&future_handle)
        || matches!(ctx.get_local_type(future_handle), MIRType::Future(_))
    {
        ctx.lower_async_wait(future_handle)
    } else {
        lower_user_future_wait(ctx, future_handle)
    }
}

fn lower_user_future_wait(ctx: &mut LoweringContext<'_>, future: Local) -> Local {
    let poll_block = ctx.new_block();
    ctx.set_terminator(Terminator::Goto(poll_block));
    ctx.set_current_block(poll_block);

    let Some(async_context) = ctx.struct_defs.get("AsyncContext") else {
        ctx.errors
            .push("user Future await requires the canonical AsyncContext definition".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let context_ty = MIRType::Struct {
        name: "AsyncContext".to_string(),
        fields: async_context
            .fields
            .iter()
            .map(|field| (field.name.clone(), ctx.hir_type_to_mir(&field.ty)))
            .collect(),
    };
    let context_handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Call {
        destination: context_handle,
        func: "sengoo_async_context_begin".to_string(),
        args: vec![],
    });
    let context = ctx.add_local(None, LocalKind::Temp, context_ty.clone());
    ctx.push_inst(Instruction::Aggregate {
        destination: context,
        fields: vec![context_handle],
        ty: context_ty,
    });

    let poll = lower_method_call_from_locals(ctx, future, "poll", &[context]);
    let poll_ty = ctx.get_local_type(poll).clone();
    let MIRType::Struct { fields, .. } = poll_ty else {
        ctx.errors
            .push("Future<T>::poll must return Poll<T>".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    if fields.len() < 2 || fields[0].1 != MIR_BOOL {
        ctx.errors
            .push("Poll<T> must contain `is_ready: bool` followed by `value: T`".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    }

    let is_ready = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Extract {
        destination: is_ready,
        value: poll,
        index: 0,
    });
    let value = ctx.add_local(None, LocalKind::Temp, fields[1].1.clone());
    ctx.push_inst(Instruction::Extract {
        destination: value,
        value: poll,
        index: 1,
    });

    let ready_block = ctx.new_block();
    let pending_block = ctx.new_block();
    ctx.set_terminator(Terminator::If {
        cond: is_ready,
        then_block: ready_block,
        else_block: pending_block,
    });

    ctx.set_current_block(pending_block);
    let retry_delay = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Call {
        destination: retry_delay,
        func: "sengoo_async_context_finish_delay".to_string(),
        args: vec![context_handle],
    });
    let retry = ctx.add_local(None, LocalKind::Temp, MIRType::Future(Box::new(MIR_UNIT)));
    ctx.push_inst(Instruction::Call {
        destination: retry,
        func: "sengoo_async_sleep__start".to_string(),
        args: vec![retry_delay],
    });
    ctx.future_origins
        .insert(retry, "sengoo_async_sleep".to_string());
    let _ = ctx.lower_async_wait(retry);
    ctx.set_terminator(Terminator::Goto(poll_block));

    ctx.set_current_block(ready_block);
    let context_dropped = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Call {
        destination: context_dropped,
        func: "sengoo_async_context_drop".to_string(),
        args: vec![context_handle],
    });
    value
}

pub(super) fn lower_async_block_expr(ctx: &mut LoweringContext<'_>, body: &HIRBody) -> Local {
    ctx.lower_async_block(body)
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
    fn lower_block_expr_returns_scoped_body_value() {
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

        let result = lower_block_expr(
            &mut ctx,
            &HIRBody::with_expr(HIRExpr::Lit(HIRLiteral::Int(1))),
        );

        assert_ne!(result, Local::new(0, LocalKind::Return));
        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(
            matches!(ctx.mir_fn.basic_blocks[start_block].terminator, Some(Terminator::Goto(target)) if target == ctx.current_block())
        );
    }

    #[test]
    fn lower_await_expr_emits_result_call_for_resolved_future() {
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

        let future_local =
            ctx.add_local(None, LocalKind::Temp, MIRType::Future(Box::new(MIR_BOOL)));
        ctx.local_names.insert("f".to_string(), future_local);
        ctx.bind_local_symbol(SymbolId::new(1), future_local);
        ctx.future_origins
            .insert(future_local, "worker".to_string());

        let result = lower_await_expr(
            &mut ctx,
            &HIRExpr::Var {
                name: "f".to_string(),
                symbol: SymbolId::new(1),
            },
        );

        assert_eq!(ctx.get_local_type(result), &MIR_BOOL);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { destination, func, args } if *destination == result && func == "worker__result" && args == &vec![future_local]
        )));
    }

    #[test]
    fn lower_async_block_expr_tracks_async_block_future_origin() {
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

        let result = lower_async_block_expr(
            &mut ctx,
            &HIRBody::with_expr(HIRExpr::Lit(HIRLiteral::Int(1))),
        );

        assert!(ctx.future_origins.contains_key(&result));
        assert!(ctx.lambda_functions.iter().any(|f| f.is_async));
    }
}
