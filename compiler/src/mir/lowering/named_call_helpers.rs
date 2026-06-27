use super::*;

pub(super) fn lower_named_call(
    ctx: &mut LoweringContext<'_>,
    name: &str,
    arg_locals: &[Local],
) -> Local {
    let arg_locals = coerce_dyn_call_args(ctx, name, arg_locals);
    let arg_locals = arg_locals.as_slice();
    match ctx.resolve_named_call_target(name, arg_locals) {
        CallTargetResolution::Builtin(local) => local,
        CallTargetResolution::Planned(plan) => {
            ctx.mark_drop_locals_moved(arg_locals);
            let invocation = {
                let async_functions = ctx.options.async_functions.borrow();
                build_call_invocation_plan(
                    &plan.func_name,
                    &plan.ret_type,
                    plan.env_ptr_local,
                    arg_locals,
                    &async_functions,
                )
            };
            emit_call_from_plan(ctx, invocation)
        }
    }
}

/// Apply `&Concrete -> &dyn Trait` unsize coercions to arguments of `name` whose
/// parameter is declared as `&dyn Trait`, building the `{ data, vtable }` fat
/// pointer and recording the `(trait, concrete)` pair so its vtable + shims get
/// synthesized. Non-dyn parameters pass through unchanged.
fn coerce_dyn_call_args(
    ctx: &mut LoweringContext<'_>,
    name: &str,
    arg_locals: &[Local],
) -> Vec<Local> {
    let Some(param_traits) = ctx.options.dyn_param_traits.get(name).cloned() else {
        return arg_locals.to_vec();
    };

    let mut coerced = Vec::with_capacity(arg_locals.len());
    for (idx, &arg_local) in arg_locals.iter().enumerate() {
        match param_traits.get(idx).and_then(|t| t.as_ref()) {
            Some(trait_name) => {
                coerced.push(emit_dyn_coercion(ctx, arg_local, trait_name));
            }
            None => coerced.push(arg_local),
        }
    }
    coerced
}

/// Build a `&dyn Trait` fat pointer from a reference to a concrete value and
/// register the `(trait, concrete)` vtable requirement.
fn emit_dyn_coercion(
    ctx: &mut LoweringContext<'_>,
    concrete_ref: Local,
    trait_name: &str,
) -> Local {
    use crate::mir::dyn_dispatch;

    let arg_ty = ctx.get_local_type(concrete_ref).clone();
    let Some(concrete_name) = concrete_struct_name(&arg_ty) else {
        // Typeck guarantees a concrete reference here; if not, leave it to the
        // normal path which will surface a diagnostic.
        return concrete_ref;
    };

    ctx.options
        .dyn_vtable_requests
        .borrow_mut()
        .insert((trait_name.to_string(), concrete_name.clone()));

    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));

    // data: reinterpret `&Concrete` as the type-erased `i8*` fat-pointer slot.
    let data_i8 = ctx.add_local(None, LocalKind::Temp, i8_ptr.clone());
    ctx.push_inst(Instruction::Cast {
        destination: data_i8,
        value: concrete_ref,
        to: i8_ptr.clone(),
    });

    // vtable: address of the `(trait, concrete)` table, type-erased to `i8*`.
    let vtable_local = ctx.add_local(None, LocalKind::Temp, i8_ptr);
    ctx.push_inst(Instruction::Assign {
        destination: vtable_local,
        value: MirConstant::GlobalRef(dyn_dispatch::vtable_global_name(trait_name, &concrete_name)),
    });

    let fat_ty = dyn_dispatch::dyn_fat_ptr_type(trait_name);
    let fat_name = match &fat_ty {
        MIRType::Struct { name, .. } => Some(name.clone()),
        _ => None,
    };
    let fat_local = ctx.add_local(None, LocalKind::Temp, fat_ty.clone());
    if let Some(name) = fat_name {
        ctx.type_names.insert(fat_local, name);
    }
    ctx.push_inst(Instruction::Aggregate {
        destination: fat_local,
        fields: vec![data_i8, vtable_local],
        ty: fat_ty,
    });
    fat_local
}

fn concrete_struct_name(ty: &MIRType) -> Option<String> {
    match ty {
        MIRType::Ref(inner) | MIRType::Ptr(inner) => match inner.as_ref() {
            MIRType::Struct { name, .. } => Some(name.clone()),
            _ => None,
        },
        MIRType::Struct { name, .. } => Some(name.clone()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_named_call_prefers_builtin_dispatch() {
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

        let task = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        let result = lower_named_call(&mut ctx, "task_status", &[task]);

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, .. } if func == "sengoo_async_task_status"
        )));
    }

    #[test]
    fn lower_named_call_wraps_async_function_start_and_tracks_origin() {
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

        let result = lower_named_call(&mut ctx, "worker", &[]);

        assert_eq!(
            ctx.get_local_type(result),
            &MIRType::Future(Box::new(MIR_BOOL))
        );
        assert_eq!(
            ctx.future_origins.get(&result).map(String::as_str),
            Some("worker")
        );
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, .. } if func == "worker__start"
        )));
    }
}
