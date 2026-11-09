use super::method_call_helpers::lower_method_call_from_locals_with_expected;
use super::*;

pub(super) fn lower_method_call_expr(
    ctx: &mut LoweringContext<'_>,
    receiver: &HIRExpr,
    method: &str,
    args: &[HIRExpr],
    expected_return_type: Option<&HIRType>,
) -> Local {
    // Preserve an addressable field path until receiver ABI selection. Loading
    // the field first would silently mutate a temporary copy of nested state.
    let mut receiver_local = if let Some(field_ref) =
        super::pointer_expr_helpers::try_lower_addressable_field_ref(ctx, receiver)
    {
        field_ref
    } else {
        ctx.lower_expr(receiver)
    };
    let receiver_ty = ctx.get_local_type(receiver_local).clone();
    let explicit_type_name = ctx.type_names.get(&receiver_local).cloned();
    // Borrowed struct receivers resolve through the same dereference plan as
    // dispatch. Looking up `ptr_method` here while emit selects `Buffer_method`
    // would let the outer HIR layer erase the resolved callee's borrow policy.
    let ownership_receiver = match &receiver_ty {
        MIRType::Ref(inner) | MIRType::Ptr(inner)
            if matches!(inner.as_ref(), MIRType::Struct { .. }) =>
        {
            inner.as_ref()
        }
        ty => ty,
    };
    let plan = crate::mir::method_dispatch_helpers::build_method_dispatch_plan(
        explicit_type_name.as_deref(),
        ownership_receiver,
        method,
    );
    let borrowed_params = ctx.argument_borrow_policy(&plan.func_name);
    // Protect a produced receiver before evaluating any argument which may
    // propagate early. Consuming calls relinquish this temporary after use.
    if receiver_local.kind == LocalKind::Temp
        && super::call_expr_helpers::is_fresh_owned_argument(receiver)
    {
        receiver_local = ctx.retain_temporary_receiver(receiver_local);
    }
    let mut arg_locals = Vec::with_capacity(args.len());
    for (index, arg) in args.iter().enumerate() {
        let previous = ctx.borrowed_output_context;
        ctx.borrowed_output_context =
            ctx.argument_output_is_borrowed(&plan.func_name, index + 1, args.len() + 1);
        let mut local = ctx.lower_expr(arg);
        ctx.borrowed_output_context = previous;
        if super::call_expr_helpers::is_fresh_owned_argument(arg) {
            local = ctx.retain_temporary_receiver(local);
        }
        arg_locals.push(local);
    }
    let consumes_receiver =
        ctx.inherent_method_consumes_receiver(&receiver_ty, method, &arg_locals);
    let releases_receiver =
        super::method_call_helpers::method_explicitly_releases_receiver(&receiver_ty, method);
    let expected_return_name = expected_return_type.map(crate::type_naming::hir_type_instance_name);
    let expected_return_type = expected_return_type.map(|ty| ctx.hir_type_to_mir(ty));
    let result = lower_method_call_from_locals_with_expected(
        ctx,
        receiver_local,
        method,
        &arg_locals,
        expected_return_type.as_ref(),
        expected_return_name.as_deref(),
    );
    for (index, arg) in args.iter().enumerate() {
        if !borrowed_params
            .as_ref()
            .and_then(|params| params.get(index + 1))
            .copied()
            .unwrap_or(false)
        {
            ctx.mark_drop_expr_moved(arg);
        }
    }
    if consumes_receiver || releases_receiver {
        ctx.mark_drop_expr_moved(receiver);
        ctx.mark_drop_local_moved(receiver_local);
    }
    result
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
        let result = lower_method_call_expr(&mut ctx, &expr, "len", &[], None);

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, args, .. } if func == "sengoo_str_len" && args == &vec![receiver_local]
        )));
    }
}
