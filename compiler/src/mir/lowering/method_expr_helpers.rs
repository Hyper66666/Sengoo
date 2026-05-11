use super::method_call_helpers::lower_method_call_from_locals;
use super::*;

pub(super) fn lower_method_call_expr(
    ctx: &mut LoweringContext<'_>,
    receiver: &HIRExpr,
    method: &str,
    args: &[HIRExpr],
) -> Local {
    let receiver_local = ctx.lower_expr(receiver);
    let arg_locals: Vec<Local> = args.iter().map(|a| ctx.lower_expr(a)).collect();
    lower_method_call_from_locals(ctx, receiver_local, method, &arg_locals)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_method_call_expr_lowers_receiver_and_delegates_to_method_call_logic() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::new();
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();
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

        let receiver_local = ctx.add_local(
            Some("s".to_string()),
            LocalKind::User,
            MIRType::Ptr(Box::new(MIRType::Int(8))),
        );
        ctx.local_names.insert("s".to_string(), receiver_local);
        ctx.bind_local_symbol(SymbolId::new(1), receiver_local);

        let expr = HIRExpr::Var {
            name: "s".to_string(),
            symbol: SymbolId::new(1),
        };
        let result = lower_method_call_expr(&mut ctx, &expr, "len", &[]);

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, args, .. } if func == "sengoo_str_len" && args == &vec![receiver_local]
        )));
    }
}
