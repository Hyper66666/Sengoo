use super::*;

pub(super) fn try_lower_string_len_method_call(
    ctx: &mut LoweringContext<'_>,
    receiver_local: Local,
    method: &str,
) -> Option<Local> {
    let is_string_len = method == "len"
        && matches!(
            ctx.get_local_type(receiver_local),
            MIRType::Ptr(inner) if matches!(inner.as_ref(), MIRType::Int(8))
        );

    if is_string_len {
        let result_local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        ctx.push_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_str_len".to_string(),
            args: vec![receiver_local],
        });
        return Some(result_local);
    }

    None
}

pub(super) fn try_lower_rc_borrow_method_call(
    ctx: &mut LoweringContext<'_>,
    receiver_local: Local,
    method: &str,
) -> Option<Local> {
    if method != "borrow" {
        return None;
    }

    let receiver_ty = ctx.get_local_type(receiver_local).clone();
    let rc_ty = match &receiver_ty {
        MIRType::Struct { name, .. } if name == "Rc" || name.starts_with("Rc_") => {
            receiver_ty.clone()
        }
        MIRType::Ref(inner) | MIRType::Ptr(inner)
            if matches!(
                inner.as_ref(),
                MIRType::Struct { name, .. } if name == "Rc" || name.starts_with("Rc_")
            ) =>
        {
            inner.as_ref().clone()
        }
        _ => return None,
    };

    let Some(crate::hir::HIRType {
        kind: crate::hir::HIRTypeKind::Named { name, args },
    }) = ctx.concrete_type_registry.hir_type_for_mir(&rc_ty)
    else {
        ctx.errors
            .push("Rc.borrow: concrete payload type could not be resolved".to_string());
        return Some(ctx.add_local(None, LocalKind::Temp, MIR_UNIT));
    };
    if name != "Rc" || args.len() != 1 {
        return None;
    }
    let payload_ty = ctx.hir_type_to_mir(&args[0]);

    let handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    match receiver_ty {
        MIRType::Struct { .. } => {
            ctx.push_inst(Instruction::Extract {
                destination: handle,
                value: receiver_local,
                index: 0,
            });
        }
        MIRType::Ref(_) | MIRType::Ptr(_) => {
            let rc_value = ctx.add_local(None, LocalKind::Temp, rc_ty);
            ctx.push_inst(Instruction::Load {
                destination: rc_value,
                source: receiver_local,
            });
            ctx.push_inst(Instruction::Extract {
                destination: handle,
                value: rc_value,
                index: 0,
            });
        }
        _ => return None,
    }

    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let erased = ctx.add_local(None, LocalKind::Temp, i8_ptr);
    ctx.push_inst(Instruction::Call {
        destination: erased,
        func: "sengoo_rc_borrow_ptr".to_string(),
        args: vec![handle],
    });

    let borrow_ty = MIRType::Ref(Box::new(payload_ty));
    let result = ctx.add_local(None, LocalKind::Temp, borrow_ty.clone());
    ctx.push_inst(Instruction::Cast {
        destination: result,
        value: erased,
        to: borrow_ty,
    });
    Some(result)
}

pub(super) fn try_lower_string_as_str_method_call(
    ctx: &mut LoweringContext<'_>,
    receiver_local: Local,
    method: &str,
) -> Option<Local> {
    if method != "as_str" {
        return None;
    }

    if !matches!(ctx.get_local_type(receiver_local), MIRType::Struct { name, .. } if name == "String")
    {
        return None;
    }

    let handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Extract {
        destination: handle,
        value: receiver_local,
        index: 0,
    });

    let result = ctx.add_local(
        None,
        LocalKind::Temp,
        MIRType::Ptr(Box::new(MIRType::Int(8))),
    );
    ctx.push_inst(Instruction::Call {
        destination: result,
        func: "sengoo_string_as_str_ptr".to_string(),
        args: vec![handle],
    });
    Some(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_lower_string_len_method_call_emits_runtime_len() {
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

        let receiver = ctx.add_local(
            None,
            LocalKind::Temp,
            MIRType::Ptr(Box::new(MIRType::Int(8))),
        );

        let result = try_lower_string_len_method_call(&mut ctx, receiver, "len");

        let result = result.expect("expected string len helper to match");
        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, args, .. } if func == "sengoo_str_len" && args == &vec![receiver]
        )));
    }
}
