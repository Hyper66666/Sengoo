use super::*;

pub(super) fn lower_lambda_expr(
    ctx: &mut LoweringContext<'_>,
    params: &[String],
    body: &HIRExpr,
) -> Local {
    lower_lambda_expr_with_expected(ctx, params, body, None)
}

pub(super) fn lower_lambda_expr_with_expected(
    ctx: &mut LoweringContext<'_>,
    params: &[String],
    body: &HIRExpr,
    expected_ty: Option<MIRType>,
) -> Local {
    let lambda_name = ctx.lambda_name();
    let free_vars = ctx.collect_free_vars(params, body);

    let (mut param_types, ret_type) = match expected_ty {
        Some(MIRType::Fn {
            params: expected_params,
            ret,
        }) if expected_params.len() == params.len() => (expected_params, *ret),
        _ => ((0..params.len()).map(|_| MIR_I64).collect(), MIR_I64),
    };

    let env_param_offset = if free_vars.is_empty() {
        0
    } else {
        param_types.insert(0, MIRType::Ptr(Box::new(MIR_I64)));
        1
    };

    let mut lambda_fn =
        MirFunction::new(lambda_name.clone(), param_types.clone(), ret_type.clone());
    let lambda_start = lambda_fn.start_block;
    let mut lambda_ctx = LoweringContext::new(
        &mut lambda_fn,
        ctx.lambda_counter,
        &ctx.known_functions,
        &ctx.function_sigs,
        ctx.struct_defs,
        ctx.concrete_type_registry.clone(),
        ctx.options.clone(),
        ctx.inherent_method_templates,
        ctx.trait_method_templates,
    );
    lambda_ctx.current_block = Some(lambda_start);

    if !free_vars.is_empty() {
        let env_local = Local::new(1, LocalKind::Param);
        let env_ptr_name = "__env".to_string();
        lambda_ctx.local_names.insert(env_ptr_name, env_local);

        for (i, (var_name, _)) in free_vars.iter().enumerate() {
            let captured_local =
                lambda_ctx.add_local(Some(var_name.clone()), LocalKind::Temp, MIR_I64);
            let index_local = lambda_ctx.add_local(None, LocalKind::Temp, MIR_I64);
            lambda_ctx.push_inst(Instruction::Assign {
                destination: index_local,
                value: MirConstant::Int(i as i64),
            });
            let ptr_local =
                lambda_ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(MIR_I64)));
            lambda_ctx.push_inst(Instruction::IndexAddr {
                destination: ptr_local,
                base: env_local,
                index: index_local,
            });
            lambda_ctx.push_inst(Instruction::Load {
                destination: captured_local,
                source: ptr_local,
            });
            lambda_ctx
                .local_names
                .insert(var_name.clone(), captured_local);
        }

        for (i, param_name) in params.iter().enumerate() {
            let local = Local::new(i + 1 + env_param_offset, LocalKind::Param);
            lambda_ctx.local_names.insert(param_name.clone(), local);
        }
    } else {
        for (i, param_name) in params.iter().enumerate() {
            let local = Local::new(i + 1 + env_param_offset, LocalKind::Param);
            lambda_ctx.local_names.insert(param_name.clone(), local);
        }
    }

    use crate::hir::HIRBody;
    let lambda_body = HIRBody {
        stmts: vec![],
        expr: Some(Box::new(body.clone())),
    };
    lambda_ctx.lower_body_to_block(&lambda_body, lambda_start);

    ctx.lambda_functions.push(lambda_fn);

    if !free_vars.is_empty() {
        let env_var_types: Vec<(String, MIRType)> = free_vars
            .iter()
            .map(|(name, local)| (name.clone(), ctx.get_local_type(*local).clone()))
            .collect();
        ctx.lambda_environments.insert(
            lambda_name.clone(),
            LambdaEnv {
                vars: free_vars.clone(),
                env_type: MIRType::Ptr(Box::new(MIR_I64)),
                env_ptr_local: None,
            },
        );
        ctx.function_sigs.insert(
            lambda_name.clone(),
            build_function_sig(ret_type.clone(), param_types.len(), env_var_types),
        );
    } else {
        ctx.function_sigs.insert(
            lambda_name.clone(),
            build_function_sig(ret_type.clone(), param_types.len(), vec![]),
        );
    }

    let lambda_local = if free_vars.is_empty() {
        let fn_ty = MIRType::Fn {
            params: param_types.clone(),
            ret: Box::new(ret_type.clone()),
        };
        let local = ctx.add_local(None, LocalKind::Temp, fn_ty);
        ctx.push_inst(Instruction::Assign {
            destination: local,
            value: MirConstant::GlobalRef(lambda_name.clone()),
        });
        local
    } else {
        ctx.add_local(None, LocalKind::Temp, MIR_I64)
    };

    ctx.lambda_names.insert(lambda_local, lambda_name);
    lambda_local
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hir::HIRLiteral;

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
    fn lower_lambda_expr_registers_signature_and_lambda_name() {
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

        let result = lower_lambda_expr(
            &mut ctx,
            &["x".to_string()],
            &HIRExpr::Lit(HIRLiteral::Int(1)),
        );

        assert!(matches!(ctx.get_local_type(result), MIRType::Fn { .. }));
        let lambda_name = ctx
            .lambda_names
            .get(&result)
            .cloned()
            .expect("lambda name should be recorded");
        assert!(ctx.function_sigs.contains_key(&lambda_name));
        assert!(ctx.lambda_functions.iter().any(|f| f.name == lambda_name));
    }
}
