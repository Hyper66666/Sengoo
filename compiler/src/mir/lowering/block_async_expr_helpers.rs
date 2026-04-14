use super::*;

pub(super) fn lower_block_expr(ctx: &mut LoweringContext<'_>, body: &HIRBody) -> Local {
    ctx.lower_body(body);
    Local::new(0, LocalKind::Return)
}

pub(super) fn lower_await_expr(ctx: &mut LoweringContext<'_>, inner: &HIRExpr) -> Local {
    let future_handle = ctx.lower_expr(inner);
    ctx.lower_async_wait(future_handle)
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
    fn lower_block_expr_returns_return_local_handle() {
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

        let result = lower_block_expr(&mut ctx, &HIRBody::with_expr(HIRExpr::Lit(HIRLiteral::Int(1))));

        assert_eq!(result, Local::new(0, LocalKind::Return));
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

        let future_local = ctx.add_local(None, LocalKind::Temp, MIRType::Future(Box::new(MIR_BOOL)));
        ctx.local_names.insert("f".to_string(), future_local);
        ctx.bind_local_symbol(SymbolId::new(1), future_local);
        ctx.future_origins.insert(future_local, "worker".to_string());

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

        let result = lower_async_block_expr(&mut ctx, &HIRBody::with_expr(HIRExpr::Lit(HIRLiteral::Int(1))));

        assert!(ctx.future_origins.contains_key(&result));
        assert!(ctx.lambda_functions.iter().any(|f| f.is_async));
    }
}