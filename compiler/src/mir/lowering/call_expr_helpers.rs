use super::named_call_helpers::lower_named_call;
use super::non_named_call_helpers::lower_non_named_call;
use super::*;

fn is_spawn_blocking_call(name: &str) -> bool {
    matches!(name, "spawn_blocking_i64" | "spawn_blocking_future_i64")
}

fn async_context_capture_name(ctx: &LoweringContext<'_>, arg: &HIRExpr) -> Option<String> {
    let HIRExpr::Lambda { params, body } = arg else {
        return None;
    };

    ctx.collect_free_vars(params, body)
        .into_iter()
        .find_map(|(name, local)| {
            if matches!(
                ctx.get_local_type(local),
                MIRType::Struct { name: ty_name, .. } if ty_name == "AsyncContext"
            ) {
                Some(name)
            } else {
                None
            }
        })
}

pub(super) fn lower_call_expr(
    ctx: &mut LoweringContext<'_>,
    func: &HIRExpr,
    args: &[HIRExpr],
    site_lo: Option<u32>,
    expected_return_type: Option<&HIRType>,
) -> Local {
    let expected_mir = expected_return_type.map(|ty| {
        crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_enums(
            ty,
            ctx.struct_defs,
            &ctx.options.enum_defs,
            &HashMap::new(),
        )
    });
    lower_call_expr_with_expected_mir(ctx, func, args, site_lo, expected_mir.as_ref())
}

pub(super) fn lower_call_expr_with_expected_mir(
    ctx: &mut LoweringContext<'_>,
    func: &HIRExpr,
    args: &[HIRExpr],
    site_lo: Option<u32>,
    expected_return_type: Option<&MIRType>,
) -> Local {
    if let HIRExpr::Var { name, .. } = func {
        if name == "format" {
            return ctx.lower_format_call(args);
        }
        if is_spawn_blocking_call(name) {
            if let Some(capture) = args
                .first()
                .and_then(|arg| async_context_capture_name(ctx, arg))
            {
                ctx.errors.push(format!(
                    "cross-thread {name} capture `{capture}` is not Send because AsyncContext is poll-scoped"
                ));
                return ctx.add_local(None, LocalKind::Temp, MIR_I64);
            }
        }
    }

    let borrowed_params = match func {
        HIRExpr::Var { name, symbol }
            if !ctx.local_names.contains_key(name) && !ctx.local_symbols.contains_key(symbol) =>
        {
            ctx.argument_borrow_policy(name)
        }
        _ => None,
    };
    let mut arg_locals = Vec::with_capacity(args.len());
    for (index, arg) in args.iter().enumerate() {
        let previous = ctx.borrowed_output_context;
        ctx.borrowed_output_context = match func {
            HIRExpr::Var { name, symbol }
                if !ctx.local_names.contains_key(name)
                    && !ctx.local_symbols.contains_key(symbol) =>
            {
                ctx.argument_output_is_borrowed(name, index, args.len())
            }
            _ => false,
        };
        let mut local = ctx.lower_expr(arg);
        ctx.borrowed_output_context = previous;
        // Register each produced owner before evaluating the next argument:
        // a later `?` may return before the call transfers its arguments.
        if is_fresh_owned_argument(arg) {
            local = ctx.retain_temporary_receiver(local);
        }
        arg_locals.push(local);
    }

    let result = match func {
        HIRExpr::Var { name, .. } => {
            super::assert_callsite_helpers::append_assert_callsite_args(
                ctx,
                name,
                site_lo,
                &mut arg_locals,
            );
            lower_named_call(ctx, name, &arg_locals, expected_return_type)
        }
        _ => lower_non_named_call(ctx, &arg_locals),
    };
    for (index, arg) in args.iter().enumerate() {
        if !borrowed_params
            .as_ref()
            .and_then(|params| params.get(index))
            .copied()
            .unwrap_or(false)
        {
            ctx.mark_drop_expr_moved(arg);
        }
    }
    result
}

pub(super) fn is_fresh_owned_argument(expr: &HIRExpr) -> bool {
    matches!(
        expr,
        HIRExpr::Call { .. }
            | HIRExpr::MethodCall { .. }
            | HIRExpr::Try(_)
            | HIRExpr::TryBlock(_)
            | HIRExpr::Struct { .. }
            | HIRExpr::Block(_)
            | HIRExpr::If { .. }
            | HIRExpr::Match { .. }
    )
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
                receiver_is_ref: false,
                ret_type: MIR_BOOL,
                param_count: 0,
                env: Vec::new(),
            },
        )]);
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();
        let options = MirLowerOptions::default()
            .with_async_functions(["worker".to_string()].into_iter().collect());
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
        let result = lower_call_expr(&mut ctx, &func, &[], None, None);

        assert_eq!(
            ctx.get_local_type(result),
            &MIRType::Future(Box::new(MIR_BOOL))
        );
        assert_eq!(
            ctx.future_origins.get(&result).map(String::as_str),
            Some("worker")
        );
    }
}
