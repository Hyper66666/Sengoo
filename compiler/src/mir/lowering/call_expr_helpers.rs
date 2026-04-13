use super::named_call_helpers::lower_named_call;
use super::non_named_call_helpers::lower_non_named_call;
use super::*;

pub(super) fn lower_call_expr(
    ctx: &mut LoweringContext<'_>,
    func: &HIRExpr,
    args: &[HIRExpr],
) -> Local {
    let arg_locals: Vec<Local> = args.iter().map(|a| ctx.lower_expr(a)).collect();

    match func {
        HIRExpr::Var { name, .. } => lower_named_call(ctx, name, &arg_locals),
        _ => lower_non_named_call(ctx, &arg_locals),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_call_expr_dispatches_named_var_to_named_call_helper() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::from([(
            "worker".to_string(),
            FunctionSig {
                ret_type: MIR_BOOL,
                param_count: 0,
                env: Vec::new(),
            },
        )]);
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();
        let options = MirLowerOptions {
            async_functions: ["worker".to_string()].into_iter().collect(),
            ..MirLowerOptions::default()
        };
        let start_block = mir_fn.start_block;

        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            options,
            &inherent_templates,
            &trait_templates,
        );
        ctx.set_current_block(start_block);

        let func = HIRExpr::Var {
            name: "worker".to_string(),
            symbol: SymbolId::new(1),
        };
        let result = lower_call_expr(&mut ctx, &func, &[]);

        assert_eq!(ctx.get_local_type(result), &MIRType::Future(Box::new(MIR_BOOL)));
        assert_eq!(ctx.future_origins.get(&result).map(String::as_str), Some("worker"));
    }
}