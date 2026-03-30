use super::*;

pub(super) fn lower_method_call_from_locals(
    ctx: &mut LoweringContext<'_>,
    receiver_local: Local,
    method: &str,
    arg_locals: &[Local],
) -> Local {
    let receiver_ty = ctx.get_local_type(receiver_local).clone();

    if let MIRType::Ptr(inner) = &receiver_ty {
        if let MIRType::Int(8) = inner.as_ref() {
            if method == "len" {
                let result_local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
                ctx.push_inst(Instruction::Call {
                    destination: result_local,
                    func: "sengoo_str_len".to_string(),
                    args: vec![receiver_local],
                });
                return result_local;
            }
        }
    }

    let resolved_func_name = match ctx.resolve_method_call_target(
        receiver_local,
        &receiver_ty,
        method,
        arg_locals,
    ) {
        Ok(name) => name,
        Err(error) => {
            ctx.errors.push(error);
            return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
        }
    };

    let ret_type = ctx
        .function_sigs
        .get(&resolved_func_name)
        .map(|sig| sig.ret_type.clone())
        .unwrap_or(MIR_I64);
    let result_local = ctx.add_local(None, LocalKind::Temp, ret_type.clone());
    if let MIRType::Struct { name, .. } = &ret_type {
        ctx.type_names.insert(result_local, name.clone());
    }

    let mut call_args = vec![receiver_local];
    call_args.extend(arg_locals.iter().copied());
    ctx.push_inst(Instruction::Call {
        destination: result_local,
        func: resolved_func_name,
        args: call_args,
    });

    result_local
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lower_method_call_from_locals_handles_string_len_builtin() {
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

        let receiver = ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(MIRType::Int(8))));
        let result = lower_method_call_from_locals(&mut ctx, receiver, "len", &[]);

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, args, .. } if func == "sengoo_str_len" && args == &vec![receiver]
        )));
    }

    #[test]
    fn lower_method_call_from_locals_records_resolution_error_as_unit_temp() {
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

        let receiver = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        let result = lower_method_call_from_locals(&mut ctx, receiver, "missing", &[]);

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        assert!(ctx.errors.iter().any(|e| e.contains("method 'missing' not found for type 'i64'")));
    }
}
