use super::*;

pub(super) fn lower_named_call(
    ctx: &mut LoweringContext<'_>,
    name: &str,
    arg_locals: &[Local],
    expected_return_type: Option<&MIRType>,
) -> Local {
    if name == "rc_new" && arg_locals.len() == 1 {
        return lower_rc_new_call(ctx, arg_locals[0]);
    }
    if name == "option_none"
        && arg_locals.is_empty()
        && expected_return_type.is_some_and(is_option_mir_type)
    {
        return lower_option_none_call(ctx, expected_return_type);
    }
    if name == "vec_new" && arg_locals.is_empty() {
        return lower_vec_new_call(ctx, expected_return_type);
    }
    if name == "vecdeque_new" && arg_locals.is_empty() {
        return lower_vec_new_call(ctx, expected_return_type);
    }
    if name == "hashmap_new" && arg_locals.is_empty() {
        return lower_hashmap_new_call(ctx, expected_return_type);
    }
    if name == "hashset_new" && arg_locals.is_empty() {
        return lower_hashmap_new_call(ctx, expected_return_type);
    }
    if matches!(name, "btreemap_new" | "btreeset_new") && arg_locals.is_empty() {
        return lower_hashmap_new_call(ctx, expected_return_type);
    }
    if name == "raw_vec_push" && arg_locals.len() == 2 {
        return lower_raw_vec_value_call(
            ctx,
            "sengoo_raw_vec_push",
            arg_locals[0],
            None,
            arg_locals[1],
        );
    }
    if name == "raw_vec_set" && arg_locals.len() == 3 {
        return lower_raw_vec_value_call(
            ctx,
            "sengoo_raw_vec_set",
            arg_locals[0],
            Some(arg_locals[1]),
            arg_locals[2],
        );
    }
    if name == "raw_vec_insert" && arg_locals.len() == 3 {
        return lower_raw_vec_value_call(
            ctx,
            "sengoo_raw_vec_insert",
            arg_locals[0],
            Some(arg_locals[1]),
            arg_locals[2],
        );
    }
    if name == "raw_vec_get" && arg_locals.len() == 2 {
        return lower_raw_vec_get_call(ctx, arg_locals[0], arg_locals[1], expected_return_type);
    }
    if name == "raw_vec_pop" && arg_locals.len() == 1 {
        return lower_raw_vec_take_call(
            ctx,
            "sengoo_raw_vec_pop",
            arg_locals[0],
            None,
            expected_return_type,
        );
    }
    if name == "raw_vec_remove" && arg_locals.len() == 2 {
        return lower_raw_vec_take_call(
            ctx,
            "sengoo_raw_vec_remove",
            arg_locals[0],
            Some(arg_locals[1]),
            expected_return_type,
        );
    }
    if name == "raw_vec_iter_next" && arg_locals.len() == 1 {
        return lower_raw_vec_iter_next_call(
            ctx,
            "sengoo_raw_vec_iter_next",
            arg_locals[0],
            expected_return_type,
        );
    }
    if name == "raw_map_key_iter_next" && arg_locals.len() == 1 {
        return lower_raw_vec_iter_next_call(
            ctx,
            "sengoo_raw_map_key_iter_next",
            arg_locals[0],
            expected_return_type,
        );
    }
    if name == "raw_hashmap_insert" && arg_locals.len() == 3 {
        return lower_raw_hashmap_insert_call(ctx, arg_locals[0], arg_locals[1], arg_locals[2]);
    }
    if name == "raw_hashmap_get" && arg_locals.len() == 2 {
        return lower_raw_hashmap_get_call(ctx, arg_locals[0], arg_locals[1], expected_return_type);
    }
    if name == "raw_hashmap_contains" && arg_locals.len() == 2 {
        return lower_raw_hashmap_contains_call(ctx, arg_locals[0], arg_locals[1]);
    }
    if name == "raw_hashmap_remove" && arg_locals.len() == 2 {
        return lower_raw_hashmap_remove_call(
            ctx,
            arg_locals[0],
            arg_locals[1],
            expected_return_type,
        );
    }
    if name == "raw_hashset_insert" && arg_locals.len() == 2 {
        let unit = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        ctx.push_inst(Instruction::Assign {
            destination: unit,
            value: MirConstant::Int(0),
        });
        return lower_raw_hashmap_insert_call(ctx, arg_locals[0], arg_locals[1], unit);
    }
    if name == "raw_hashset_remove" && arg_locals.len() == 2 {
        return lower_raw_hashset_remove_call(ctx, arg_locals[0], arg_locals[1]);
    }

    let arg_locals = coerce_dyn_call_args(ctx, name, arg_locals);
    let arg_locals = arg_locals.as_slice();
    match ctx.resolve_named_call_target(name, arg_locals, expected_return_type) {
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

fn is_option_mir_type(ty: &MIRType) -> bool {
    matches!(ty, MIRType::Struct { name, fields }
        if (name == "Option" || name.starts_with("Option_"))
            && matches!(fields.first(), Some((_, MIRType::Bool)))
            && fields.len() == 2)
}

fn lower_option_none_call(
    ctx: &mut LoweringContext<'_>,
    expected_return_type: Option<&MIRType>,
) -> Local {
    let Some(option_ty) = expected_return_type.cloned() else {
        ctx.errors
            .push("option_none<T>: expected concrete Option<T> return type".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let MIRType::Struct { name, fields } = &option_ty else {
        ctx.errors
            .push("option_none<T>: expected concrete Option<T> return type".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    if !(name == "Option" || name.starts_with("Option_"))
        || !matches!(fields.first(), Some((_, MIRType::Bool)))
    {
        ctx.errors
            .push("option_none<T>: malformed Option<T> return type".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    }
    let Some((_, value_ty)) = fields.get(1) else {
        ctx.errors
            .push("option_none<T>: malformed Option<T> return type".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };

    let is_some = ctx.lower_literal(&HIRLiteral::Bool(false));
    let value = super::try_expr_helpers::default_value_for_type(ctx, value_ty);
    let result = ctx.add_local(None, LocalKind::Temp, option_ty.clone());
    ctx.push_inst(Instruction::Aggregate {
        destination: result,
        fields: vec![is_some, value],
        ty: option_ty,
    });
    ctx.mark_drop_local_moved(value);
    result
}

fn lower_raw_hashset_remove_call(
    ctx: &mut LoweringContext<'_>,
    handle: Local,
    key: Local,
) -> Local {
    let unit = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: unit,
        value: MirConstant::Int(0),
    });
    let unit_slot = materialize_rc_payload_source(ctx, unit, &MIR_I64);
    let unit_ptr = ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(MIR_I64)));
    ctx.push_inst(Instruction::AddrOf {
        destination: unit_ptr,
        source: unit_slot,
    });
    let key = erase_borrowed_pointer(ctx, key);
    let unit_ptr = erase_borrowed_pointer(ctx, unit_ptr);
    let status = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Call {
        destination: status,
        func: "sengoo_raw_hashmap_remove".to_string(),
        args: vec![handle, key, unit_ptr],
    });
    let zero = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: zero,
        value: MirConstant::Int(0),
    });
    let result = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: result,
        op: MirBinOp::Eq,
        left: status,
        right: zero,
    });
    result
}

fn erase_borrowed_pointer(ctx: &mut LoweringContext<'_>, value: Local) -> Local {
    let erased_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let erased = ctx.add_local(None, LocalKind::Temp, erased_ty.clone());
    ctx.push_inst(Instruction::Cast {
        destination: erased,
        value,
        to: erased_ty,
    });
    erased
}

fn lower_raw_hashmap_insert_call(
    ctx: &mut LoweringContext<'_>,
    handle: Local,
    key: Local,
    value: Local,
) -> Local {
    let key_ty = ctx.get_local_type(key).clone();
    let value_ty = ctx.get_local_type(value).clone();
    let key_slot = materialize_rc_payload_source(ctx, key, &key_ty);
    let value_slot = materialize_rc_payload_source(ctx, value, &value_ty);
    let key_ptr = ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(key_ty)));
    let value_ptr = ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(value_ty)));
    ctx.push_inst(Instruction::AddrOf {
        destination: key_ptr,
        source: key_slot,
    });
    ctx.push_inst(Instruction::AddrOf {
        destination: value_ptr,
        source: value_slot,
    });
    let key_ptr = erase_borrowed_pointer(ctx, key_ptr);
    let value_ptr = erase_borrowed_pointer(ctx, value_ptr);
    let status = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Call {
        destination: status,
        func: "sengoo_raw_hashmap_insert".to_string(),
        args: vec![handle, key_ptr, value_ptr],
    });
    let zero = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: zero,
        value: MirConstant::Int(0),
    });
    let result = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: result,
        op: MirBinOp::Eq,
        left: status,
        right: zero,
    });
    ctx.mark_drop_local_moved(key_slot);
    ctx.mark_drop_local_moved(value_slot);
    result
}

fn lower_raw_hashmap_get_call(
    ctx: &mut LoweringContext<'_>,
    handle: Local,
    key: Local,
    expected_return_type: Option<&MIRType>,
) -> Local {
    let result_ty = expected_return_type
        .cloned()
        .unwrap_or_else(|| ctx.mir_fn.return_type.clone());
    let result_ty @ MIRType::Ref(_) = result_ty else {
        ctx.errors
            .push("raw_hashmap_get<K,V>: expected borrowed value return".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let key = erase_borrowed_pointer(ctx, key);
    let erased_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let erased = ctx.add_local(None, LocalKind::Temp, erased_ty);
    ctx.push_inst(Instruction::Call {
        destination: erased,
        func: "sengoo_raw_hashmap_get".to_string(),
        args: vec![handle, key],
    });
    let result = ctx.add_local(None, LocalKind::Temp, result_ty.clone());
    ctx.push_inst(Instruction::Cast {
        destination: result,
        value: erased,
        to: result_ty,
    });
    result
}

fn lower_raw_hashmap_contains_call(
    ctx: &mut LoweringContext<'_>,
    handle: Local,
    key: Local,
) -> Local {
    let key = erase_borrowed_pointer(ctx, key);
    let raw = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Call {
        destination: raw,
        func: "sengoo_raw_hashmap_contains".to_string(),
        args: vec![handle, key],
    });
    let zero = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: zero,
        value: MirConstant::Int(0),
    });
    let result = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: result,
        op: MirBinOp::Ne,
        left: raw,
        right: zero,
    });
    result
}

fn lower_raw_hashmap_remove_call(
    ctx: &mut LoweringContext<'_>,
    handle: Local,
    key: Local,
    expected_return_type: Option<&MIRType>,
) -> Local {
    let option_ty = expected_return_type
        .cloned()
        .unwrap_or_else(|| ctx.mir_fn.return_type.clone());
    let MIRType::Struct { fields, .. } = &option_ty else {
        ctx.errors
            .push("raw_hashmap_remove<K,V>: expected Option<V>".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let Some((_, value_ty)) = fields.get(1) else {
        ctx.errors
            .push("raw_hashmap_remove<K,V>: malformed Option<V>".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let value_ty = value_ty.clone();
    let value = super::try_expr_helpers::default_value_for_type(ctx, &value_ty);
    let value_slot = materialize_rc_payload_source(ctx, value, &value_ty);
    let value_ptr = ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(value_ty)));
    ctx.push_inst(Instruction::AddrOf {
        destination: value_ptr,
        source: value_slot,
    });
    let key = erase_borrowed_pointer(ctx, key);
    let value_ptr = erase_borrowed_pointer(ctx, value_ptr);
    let status = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Call {
        destination: status,
        func: "sengoo_raw_hashmap_remove".to_string(),
        args: vec![handle, key, value_ptr],
    });
    let zero = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: zero,
        value: MirConstant::Int(0),
    });
    let is_some = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: is_some,
        op: MirBinOp::Eq,
        left: status,
        right: zero,
    });
    let result = ctx.add_local(None, LocalKind::Temp, option_ty.clone());
    ctx.push_inst(Instruction::Aggregate {
        destination: result,
        fields: vec![is_some, value_slot],
        ty: option_ty,
    });
    ctx.mark_drop_local_moved(value_slot);
    result
}

fn lower_raw_vec_iter_next_call(
    ctx: &mut LoweringContext<'_>,
    runtime_function: &str,
    handle: Local,
    expected_return_type: Option<&MIRType>,
) -> Local {
    let option_ty = expected_return_type
        .cloned()
        .unwrap_or_else(|| ctx.mir_fn.return_type.clone());
    let MIRType::Struct { fields, .. } = &option_ty else {
        ctx.errors
            .push("raw_vec_iter_next<T>: expected concrete Option<&T>".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let Some((_, ref_ty @ MIRType::Ref(_))) = fields.get(1) else {
        ctx.errors
            .push("raw_vec_iter_next<T>: expected concrete Option<&T>".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let erased_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let erased = ctx.add_local(None, LocalKind::Temp, erased_ty);
    ctx.push_inst(Instruction::Call {
        destination: erased,
        func: runtime_function.to_string(),
        args: vec![handle],
    });
    let borrowed = ctx.add_local(None, LocalKind::Temp, ref_ty.clone());
    ctx.push_inst(Instruction::Cast {
        destination: borrowed,
        value: erased,
        to: ref_ty.clone(),
    });
    let address = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Cast {
        destination: address,
        value: borrowed,
        to: MIR_I64,
    });
    let zero = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: zero,
        value: MirConstant::Int(0),
    });
    let is_some = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: is_some,
        op: MirBinOp::Ne,
        left: address,
        right: zero,
    });
    let result = ctx.add_local(None, LocalKind::Temp, option_ty.clone());
    ctx.push_inst(Instruction::Aggregate {
        destination: result,
        fields: vec![is_some, borrowed],
        ty: option_ty,
    });
    result
}

fn lower_raw_vec_get_call(
    ctx: &mut LoweringContext<'_>,
    handle: Local,
    index: Local,
    expected_return_type: Option<&MIRType>,
) -> Local {
    let result_ty = expected_return_type
        .cloned()
        .unwrap_or_else(|| ctx.mir_fn.return_type.clone());
    let result_ty @ MIRType::Ref(_) = result_ty else {
        ctx.errors.push(
            "raw_vec_get<T>: concrete borrowed return type could not be resolved".to_string(),
        );
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let erased_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let erased = ctx.add_local(None, LocalKind::Temp, erased_ty);
    ctx.push_inst(Instruction::Call {
        destination: erased,
        func: "sengoo_raw_vec_get".to_string(),
        args: vec![handle, index],
    });
    let result = ctx.add_local(None, LocalKind::Temp, result_ty.clone());
    ctx.push_inst(Instruction::Cast {
        destination: result,
        value: erased,
        to: result_ty,
    });
    result
}

fn lower_raw_vec_take_call(
    ctx: &mut LoweringContext<'_>,
    runtime_function: &str,
    handle: Local,
    index: Option<Local>,
    expected_return_type: Option<&MIRType>,
) -> Local {
    let option_ty = expected_return_type
        .cloned()
        .unwrap_or_else(|| ctx.mir_fn.return_type.clone());
    let MIRType::Struct { fields, .. } = &option_ty else {
        ctx.errors.push(format!(
            "{runtime_function}: concrete Option<T> return type could not be resolved"
        ));
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let Some((_, value_ty)) = fields.get(1) else {
        ctx.errors.push(format!(
            "{runtime_function}: malformed Option<T> return type"
        ));
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let value_ty = value_ty.clone();
    let value = super::try_expr_helpers::default_value_for_type(ctx, &value_ty);
    let value_slot = materialize_rc_payload_source(ctx, value, &value_ty);
    let value_ptr = ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(value_ty)));
    ctx.push_inst(Instruction::AddrOf {
        destination: value_ptr,
        source: value_slot,
    });
    let erased_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let erased = ctx.add_local(None, LocalKind::Temp, erased_ty.clone());
    ctx.push_inst(Instruction::Cast {
        destination: erased,
        value: value_ptr,
        to: erased_ty,
    });
    let mut args = vec![handle];
    if let Some(index) = index {
        args.push(index);
    }
    args.push(erased);
    let status = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Call {
        destination: status,
        func: runtime_function.to_string(),
        args,
    });
    let ok_status = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: ok_status,
        value: MirConstant::Int(0),
    });
    let is_some = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: is_some,
        op: MirBinOp::Eq,
        left: status,
        right: ok_status,
    });
    let result = ctx.add_local(None, LocalKind::Temp, option_ty.clone());
    ctx.push_inst(Instruction::Aggregate {
        destination: result,
        fields: vec![is_some, value_slot],
        ty: option_ty,
    });
    ctx.mark_drop_local_moved(value_slot);
    result
}

fn lower_raw_vec_value_call(
    ctx: &mut LoweringContext<'_>,
    runtime_function: &str,
    handle: Local,
    index: Option<Local>,
    value: Local,
) -> Local {
    let value_ty = ctx.get_local_type(value).clone();
    let value_slot = materialize_rc_payload_source(ctx, value, &value_ty);
    let value_ptr = ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(value_ty)));
    ctx.push_inst(Instruction::AddrOf {
        destination: value_ptr,
        source: value_slot,
    });
    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let erased = ctx.add_local(None, LocalKind::Temp, i8_ptr.clone());
    ctx.push_inst(Instruction::Cast {
        destination: erased,
        value: value_ptr,
        to: i8_ptr,
    });
    let mut args = vec![handle];
    if let Some(index) = index {
        args.push(index);
    }
    args.push(erased);
    let status = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Call {
        destination: status,
        func: runtime_function.to_string(),
        args,
    });
    let ok_status = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: ok_status,
        value: MirConstant::Int(0),
    });
    let result = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: result,
        op: MirBinOp::Eq,
        left: status,
        right: ok_status,
    });
    ctx.mark_drop_local_moved(value_slot);
    result
}

fn lower_hashmap_new_call(
    ctx: &mut LoweringContext<'_>,
    expected_return_type: Option<&MIRType>,
) -> Local {
    let Some(map_ty @ MIRType::Struct { .. }) = expected_return_type.cloned() else {
        ctx.errors
            .push("hashmap_new<K,V>: concrete return type could not be resolved".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let MIRType::Struct { name: map_name, .. } = &map_ty else {
        unreachable!();
    };
    let Some(HIRType {
        kind: hir::HIRTypeKind::Named { name, args },
        ..
    }) = ctx.concrete_type_registry.hir_type_for_mir(&map_ty)
    else {
        ctx.errors
            .push("hashmap_new<K,V>: concrete key/value types could not be resolved".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    if !((matches!(name.as_str(), "HashMap" | "BTreeMap") && args.len() == 2)
        || (matches!(name.as_str(), "HashSet" | "BTreeSet") && args.len() == 1))
    {
        ctx.errors
            .push("generic hash constructor expected HashMap<K,V> or HashSet<T>".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    }
    let key_ty = crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_enums(
        &args[0],
        ctx.struct_defs,
        &ctx.options.enum_defs,
        &HashMap::new(),
    );
    let value_ty = if matches!(name.as_str(), "HashMap" | "BTreeMap") {
        crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_enums(
            &args[1],
            ctx.struct_defs,
            &ctx.options.enum_defs,
            &HashMap::new(),
        )
    } else {
        MIR_I64
    };
    let key_suffix = crate::type_naming::mir_type_instance_name(&key_ty);
    let value_suffix = crate::type_naming::mir_type_instance_name(&value_ty);
    let key_move = synthesize_vec_move_thunk(ctx, &key_ty, &format!("MapKey_{key_suffix}"));
    let key_drop = synthesize_rc_drop_thunk(ctx, &key_ty, &format!("MapKey_{key_suffix}"));
    let value_move = synthesize_vec_move_thunk(ctx, &value_ty, &format!("MapValue_{value_suffix}"));
    let value_drop = synthesize_rc_drop_thunk(ctx, &value_ty, &format!("MapValue_{value_suffix}"));
    let ordered = matches!(name.as_str(), "BTreeMap" | "BTreeSet");
    let key_hash = (!ordered).then(|| synthesize_hash_thunk(ctx, &key_ty, &key_suffix));
    let key_eq = (!ordered).then(|| synthesize_eq_thunk(ctx, &key_ty, &key_suffix));
    let key_compare = ordered.then(|| synthesize_compare_thunk(ctx, &key_ty, &key_suffix));
    synthesize_hashmap_owner_drop_if_missing(ctx, &map_ty, map_name);

    let (key_size, key_align) = crate::codegen::common::mir_type_size_align(&key_ty);
    let (value_size, value_align) = crate::codegen::common::mir_type_size_align(&value_ty);
    let scalar = |ctx: &mut LoweringContext<'_>, value: i64| {
        let local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        ctx.push_inst(Instruction::Assign {
            destination: local,
            value: MirConstant::Int(value),
        });
        local
    };
    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let key_move = erased_typed_function_pointer(
        ctx,
        key_move,
        vec![i8_ptr.clone(), i8_ptr.clone()],
        MIR_UNIT,
    );
    let key_drop = erased_typed_function_pointer(ctx, key_drop, vec![i8_ptr.clone()], MIR_UNIT);
    let key_hash = key_hash
        .map(|name| erased_typed_function_pointer(ctx, name, vec![i8_ptr.clone()], MIR_I64));
    let key_eq = key_eq.map(|name| {
        erased_typed_function_pointer(ctx, name, vec![i8_ptr.clone(), i8_ptr.clone()], MIR_I64)
    });
    let key_compare = key_compare.map(|name| {
        erased_typed_function_pointer(ctx, name, vec![i8_ptr.clone(), i8_ptr.clone()], MIR_I64)
    });
    let value_move = erased_typed_function_pointer(
        ctx,
        value_move,
        vec![i8_ptr.clone(), i8_ptr.clone()],
        MIR_UNIT,
    );
    let value_drop = erased_typed_function_pointer(ctx, value_drop, vec![i8_ptr], MIR_UNIT);
    let key_size = scalar(ctx, key_size as i64);
    let key_align = scalar(ctx, key_align as i64);
    let value_size = scalar(ctx, value_size as i64);
    let value_align = scalar(ctx, value_align as i64);
    let handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    let (runtime_function, args) = if ordered {
        (
            "sengoo_raw_btreemap_new_parts",
            vec![
                key_size,
                key_align,
                key_move,
                key_drop,
                key_compare.expect("ordered constructor has compare callback"),
                value_size,
                value_align,
                value_move,
                value_drop,
            ],
        )
    } else {
        (
            "sengoo_raw_hashmap_new_parts",
            vec![
                key_size,
                key_align,
                key_move,
                key_drop,
                key_hash.expect("hash constructor has hash callback"),
                key_eq.expect("hash constructor has eq callback"),
                value_size,
                value_align,
                value_move,
                value_drop,
            ],
        )
    };
    ctx.push_inst(Instruction::Call {
        destination: handle,
        func: runtime_function.to_string(),
        args,
    });
    let marker = scalar(ctx, 0);
    let result = ctx.add_local(None, LocalKind::Temp, map_ty.clone());
    ctx.type_names.insert(result, map_name.clone());
    let fields = if matches!(name.as_str(), "HashMap" | "BTreeMap") {
        vec![handle, marker, marker]
    } else {
        vec![handle, marker]
    };
    ctx.push_inst(Instruction::Aggregate {
        destination: result,
        fields,
        ty: map_ty,
    });
    result
}

fn erased_typed_function_pointer(
    ctx: &mut LoweringContext<'_>,
    name: String,
    params: Vec<MIRType>,
    ret: MIRType,
) -> Local {
    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let function = ctx.add_local(
        None,
        LocalKind::Temp,
        MIRType::Fn {
            params,
            ret: Box::new(ret),
        },
    );
    ctx.push_inst(Instruction::Assign {
        destination: function,
        value: MirConstant::GlobalRef(name),
    });
    let erased = ctx.add_local(None, LocalKind::Temp, i8_ptr.clone());
    ctx.push_inst(Instruction::Cast {
        destination: erased,
        value: function,
        to: i8_ptr,
    });
    erased
}

fn synthesize_hashmap_owner_drop_if_missing(
    ctx: &mut LoweringContext<'_>,
    map_ty: &MIRType,
    map_name: &str,
) {
    let drop_name = format!("{map_name}_Drop_drop");
    if ctx.is_known_function(&drop_name) {
        return;
    }
    let mut function = MirFunction::new(drop_name.clone(), vec![map_ty.clone()], MIR_UNIT);
    let owner = Local::new(1, LocalKind::Param);
    let handle = function.add_local(LocalKind::Temp, MIR_I64);
    function.push_inst_to_block(
        function.start_block,
        Instruction::Extract {
            destination: handle,
            value: owner,
            index: 0,
        },
    );
    let status = function.add_local(LocalKind::Temp, MIR_I64);
    function.push_inst_to_block(
        function.start_block,
        Instruction::Call {
            destination: status,
            func: "sengoo_raw_hashmap_free".to_string(),
            args: vec![handle],
        },
    );
    function.basic_blocks[function.start_block].set_terminator(Terminator::Return(None));
    ctx.lambda_functions.push(function);
    ctx.insert_known_function(drop_name.clone());
    ctx.insert_function_sig(
        drop_name,
        FunctionSig {
            ret_type: MIR_UNIT,
            param_count: 1,
            env: Vec::new(),
        },
    );
}

fn lower_vec_new_call(
    ctx: &mut LoweringContext<'_>,
    expected_return_type: Option<&MIRType>,
) -> Local {
    let Some(vec_ty @ MIRType::Struct { .. }) = expected_return_type.cloned() else {
        ctx.errors
            .push("vec_new<T>: concrete Vec<T> return type could not be resolved".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let MIRType::Struct { name: vec_name, .. } = &vec_ty else {
        unreachable!();
    };
    let Some(HIRType {
        kind: hir::HIRTypeKind::Named { name, args },
        ..
    }) = ctx.concrete_type_registry.hir_type_for_mir(&vec_ty)
    else {
        ctx.errors
            .push("vec_new<T>: concrete element type could not be resolved".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    if !matches!(name.as_str(), "Vec" | "VecDeque") || args.len() != 1 {
        ctx.errors
            .push("generic sequence constructor expected a concrete element type".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    }
    let element_ty = crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_enums(
        &args[0],
        ctx.struct_defs,
        &ctx.options.enum_defs,
        &HashMap::new(),
    );
    let callback_suffix = crate::type_naming::mir_type_instance_name(&element_ty);
    let drop_thunk =
        synthesize_rc_drop_thunk(ctx, &element_ty, &format!("VecElement_{callback_suffix}"));
    let move_thunk = synthesize_vec_move_thunk(ctx, &element_ty, &callback_suffix);
    synthesize_vec_owner_drop_if_missing(ctx, &vec_ty, vec_name);
    let (size, align) = crate::codegen::common::mir_type_size_align(&element_ty);
    let size_local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: size_local,
        value: MirConstant::Int(size as i64),
    });
    let align_local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: align_local,
        value: MirConstant::Int(align as i64),
    });
    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let move_fn = erased_function_pointer(ctx, move_thunk, vec![i8_ptr.clone(), i8_ptr.clone()]);
    let drop_fn = erased_function_pointer(ctx, drop_thunk, vec![i8_ptr.clone()]);
    let handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Call {
        destination: handle,
        func: "sengoo_raw_vec_new_parts".to_string(),
        args: vec![size_local, align_local, move_fn, drop_fn],
    });
    let marker = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: marker,
        value: MirConstant::Int(0),
    });
    let result = ctx.add_local(None, LocalKind::Temp, vec_ty.clone());
    ctx.type_names.insert(result, vec_name.clone());
    ctx.push_inst(Instruction::Aggregate {
        destination: result,
        fields: vec![handle, marker],
        ty: vec_ty,
    });
    result
}

fn synthesize_vec_owner_drop_if_missing(
    ctx: &mut LoweringContext<'_>,
    vec_ty: &MIRType,
    vec_name: &str,
) {
    let drop_name = format!("{vec_name}_Drop_drop");
    if ctx.is_known_function(&drop_name) {
        return;
    }
    let mut function = MirFunction::new(drop_name.clone(), vec![vec_ty.clone()], MIR_UNIT);
    let owner = Local::new(1, LocalKind::Param);
    let handle = function.add_local(LocalKind::Temp, MIR_I64);
    function.push_inst_to_block(
        function.start_block,
        Instruction::Extract {
            destination: handle,
            value: owner,
            index: 0,
        },
    );
    let status = function.add_local(LocalKind::Temp, MIR_I64);
    function.push_inst_to_block(
        function.start_block,
        Instruction::Call {
            destination: status,
            func: "sengoo_raw_vec_free".to_string(),
            args: vec![handle],
        },
    );
    function.basic_blocks[function.start_block].set_terminator(Terminator::Return(None));
    ctx.lambda_functions.push(function);
    ctx.insert_known_function(drop_name.clone());
    ctx.insert_function_sig(
        drop_name,
        FunctionSig {
            ret_type: MIR_UNIT,
            param_count: 1,
            env: Vec::new(),
        },
    );
}

fn erased_function_pointer(
    ctx: &mut LoweringContext<'_>,
    name: String,
    params: Vec<MIRType>,
) -> Local {
    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let function = ctx.add_local(
        None,
        LocalKind::Temp,
        MIRType::Fn {
            params,
            ret: Box::new(MIR_UNIT),
        },
    );
    ctx.push_inst(Instruction::Assign {
        destination: function,
        value: MirConstant::GlobalRef(name),
    });
    let erased = ctx.add_local(None, LocalKind::Temp, i8_ptr.clone());
    ctx.push_inst(Instruction::Cast {
        destination: erased,
        value: function,
        to: i8_ptr,
    });
    erased
}

fn synthesize_vec_move_thunk(
    ctx: &mut LoweringContext<'_>,
    element_ty: &MIRType,
    suffix: &str,
) -> String {
    let name = format!("__sengoo_vec_move_{suffix}");
    if ctx.is_known_function(&name) {
        return name;
    }
    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let mut function =
        MirFunction::new(name.clone(), vec![i8_ptr.clone(), i8_ptr.clone()], MIR_UNIT);
    let destination = Local::new(1, LocalKind::Param);
    let source = Local::new(2, LocalKind::Param);
    let typed_destination =
        function.add_local(LocalKind::Temp, MIRType::Ptr(Box::new(element_ty.clone())));
    function.push_inst_to_block(
        function.start_block,
        Instruction::Cast {
            destination: typed_destination,
            value: destination,
            to: MIRType::Ptr(Box::new(element_ty.clone())),
        },
    );
    let typed_source =
        function.add_local(LocalKind::Temp, MIRType::Ptr(Box::new(element_ty.clone())));
    function.push_inst_to_block(
        function.start_block,
        Instruction::Cast {
            destination: typed_source,
            value: source,
            to: MIRType::Ptr(Box::new(element_ty.clone())),
        },
    );
    let value = function.add_local(LocalKind::Temp, element_ty.clone());
    function.push_inst_to_block(
        function.start_block,
        Instruction::Load {
            destination: value,
            source: typed_source,
        },
    );
    function.push_inst_to_block(
        function.start_block,
        Instruction::Store {
            destination: typed_destination,
            value,
        },
    );
    function.basic_blocks[function.start_block].set_terminator(Terminator::Return(None));
    ctx.lambda_functions.push(function);
    ctx.insert_known_function(name.clone());
    ctx.insert_function_sig(
        name.clone(),
        FunctionSig {
            ret_type: MIR_UNIT,
            param_count: 2,
            env: Vec::new(),
        },
    );
    name
}

fn synthesize_hash_thunk(ctx: &mut LoweringContext<'_>, key_ty: &MIRType, suffix: &str) -> String {
    let name = format!("__sengoo_hash_{suffix}");
    if ctx.is_known_function(&name) {
        return name;
    }
    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let mut function = MirFunction::new(name.clone(), vec![i8_ptr], MIR_I64);
    let raw = Local::new(1, LocalKind::Param);
    let typed = function.add_local(LocalKind::Temp, MIRType::Ptr(Box::new(key_ty.clone())));
    function.push_inst_to_block(
        function.start_block,
        Instruction::Cast {
            destination: typed,
            value: raw,
            to: MIRType::Ptr(Box::new(key_ty.clone())),
        },
    );
    let value = function.add_local(LocalKind::Temp, key_ty.clone());
    function.push_inst_to_block(
        function.start_block,
        Instruction::Load {
            destination: value,
            source: typed,
        },
    );
    let result = match key_ty {
        MIRType::Int(64) => value,
        MIRType::Int(_) | MIRType::UInt(_) | MIRType::Bool => {
            let result = function.add_local(LocalKind::Temp, MIR_I64);
            function.push_inst_to_block(
                function.start_block,
                Instruction::Cast {
                    destination: result,
                    value,
                    to: MIR_I64,
                },
            );
            result
        }
        _ => {
            let candidates = [format!("{suffix}_Hash_hash"), format!("{suffix}_hash")];
            let candidate = candidates
                .into_iter()
                .find(|candidate| ctx.is_known_function(candidate));
            let Some(candidate) = candidate else {
                ctx.errors.push(format!(
                    "HashMap key type `{suffix}` has no concrete Hash::hash implementation"
                ));
                let zero = function.add_local(LocalKind::Temp, MIR_I64);
                function.push_inst_to_block(
                    function.start_block,
                    Instruction::Assign {
                        destination: zero,
                        value: MirConstant::Int(0),
                    },
                );
                function.basic_blocks[function.start_block]
                    .set_terminator(Terminator::Return(Some(zero)));
                ctx.lambda_functions.push(function);
                ctx.insert_known_function(name.clone());
                ctx.insert_function_sig(
                    name.clone(),
                    FunctionSig {
                        ret_type: MIR_I64,
                        param_count: 1,
                        env: Vec::new(),
                    },
                );
                return name;
            };
            let result = function.add_local(LocalKind::Temp, MIR_I64);
            function.push_inst_to_block(
                function.start_block,
                Instruction::Call {
                    destination: result,
                    func: candidate,
                    args: vec![value],
                },
            );
            result
        }
    };
    function.basic_blocks[function.start_block].set_terminator(Terminator::Return(Some(result)));
    ctx.lambda_functions.push(function);
    ctx.insert_known_function(name.clone());
    ctx.insert_function_sig(
        name.clone(),
        FunctionSig {
            ret_type: MIR_I64,
            param_count: 1,
            env: Vec::new(),
        },
    );
    name
}

fn synthesize_eq_thunk(ctx: &mut LoweringContext<'_>, key_ty: &MIRType, suffix: &str) -> String {
    let name = format!("__sengoo_eq_{suffix}");
    if ctx.is_known_function(&name) {
        return name;
    }
    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let mut function = MirFunction::new(name.clone(), vec![i8_ptr.clone(), i8_ptr], MIR_I64);
    let raw_left = Local::new(1, LocalKind::Param);
    let raw_right = Local::new(2, LocalKind::Param);
    let ptr_ty = MIRType::Ptr(Box::new(key_ty.clone()));
    let left_ptr = function.add_local(LocalKind::Temp, ptr_ty.clone());
    let right_ptr = function.add_local(LocalKind::Temp, ptr_ty.clone());
    for (destination, value) in [(left_ptr, raw_left), (right_ptr, raw_right)] {
        function.push_inst_to_block(
            function.start_block,
            Instruction::Cast {
                destination,
                value,
                to: ptr_ty.clone(),
            },
        );
    }
    let left = function.add_local(LocalKind::Temp, key_ty.clone());
    let right = function.add_local(LocalKind::Temp, key_ty.clone());
    for (destination, source) in [(left, left_ptr), (right, right_ptr)] {
        function.push_inst_to_block(
            function.start_block,
            Instruction::Load {
                destination,
                source,
            },
        );
    }
    let equal = function.add_local(LocalKind::Temp, MIR_BOOL);
    if matches!(
        key_ty,
        MIRType::Int(_) | MIRType::UInt(_) | MIRType::Bool | MIRType::Float(_)
    ) {
        function.push_inst_to_block(
            function.start_block,
            Instruction::Binary {
                destination: equal,
                op: MirBinOp::Eq,
                left,
                right,
            },
        );
    } else {
        let candidates = [format!("{suffix}_PartialEq_eq"), format!("{suffix}_eq")];
        let candidate = candidates
            .into_iter()
            .find(|candidate| ctx.is_known_function(candidate));
        let Some(candidate) = candidate else {
            ctx.errors.push(format!(
                "HashMap key type `{suffix}` has no concrete PartialEq::eq implementation"
            ));
            function.push_inst_to_block(
                function.start_block,
                Instruction::Assign {
                    destination: equal,
                    value: MirConstant::Bool(false),
                },
            );
            let result = function.add_local(LocalKind::Temp, MIR_I64);
            function.push_inst_to_block(
                function.start_block,
                Instruction::Cast {
                    destination: result,
                    value: equal,
                    to: MIR_I64,
                },
            );
            function.basic_blocks[function.start_block]
                .set_terminator(Terminator::Return(Some(result)));
            ctx.lambda_functions.push(function);
            ctx.insert_known_function(name.clone());
            ctx.insert_function_sig(
                name.clone(),
                FunctionSig {
                    ret_type: MIR_I64,
                    param_count: 2,
                    env: Vec::new(),
                },
            );
            return name;
        };
        function.push_inst_to_block(
            function.start_block,
            Instruction::Call {
                destination: equal,
                func: candidate,
                args: vec![left, right_ptr],
            },
        );
    }
    let result = function.add_local(LocalKind::Temp, MIR_I64);
    function.push_inst_to_block(
        function.start_block,
        Instruction::Cast {
            destination: result,
            value: equal,
            to: MIR_I64,
        },
    );
    function.basic_blocks[function.start_block].set_terminator(Terminator::Return(Some(result)));
    ctx.lambda_functions.push(function);
    ctx.insert_known_function(name.clone());
    ctx.insert_function_sig(
        name.clone(),
        FunctionSig {
            ret_type: MIR_I64,
            param_count: 2,
            env: Vec::new(),
        },
    );
    name
}

fn synthesize_compare_thunk(
    ctx: &mut LoweringContext<'_>,
    key_ty: &MIRType,
    suffix: &str,
) -> String {
    let name = format!("__sengoo_compare_{suffix}");
    if ctx.is_known_function(&name) {
        return name;
    }
    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let mut function = MirFunction::new(name.clone(), vec![i8_ptr.clone(), i8_ptr], MIR_I64);
    let raw_left = Local::new(1, LocalKind::Param);
    let raw_right = Local::new(2, LocalKind::Param);
    let ptr_ty = MIRType::Ptr(Box::new(key_ty.clone()));
    let left_ptr = function.add_local(LocalKind::Temp, ptr_ty.clone());
    let right_ptr = function.add_local(LocalKind::Temp, ptr_ty.clone());
    for (destination, value) in [(left_ptr, raw_left), (right_ptr, raw_right)] {
        function.push_inst_to_block(
            function.start_block,
            Instruction::Cast {
                destination,
                value,
                to: ptr_ty.clone(),
            },
        );
    }
    let left = function.add_local(LocalKind::Temp, key_ty.clone());
    let right = function.add_local(LocalKind::Temp, key_ty.clone());
    for (destination, source) in [(left, left_ptr), (right, right_ptr)] {
        function.push_inst_to_block(
            function.start_block,
            Instruction::Load {
                destination,
                source,
            },
        );
    }
    if matches!(
        key_ty,
        MIRType::Int(_) | MIRType::UInt(_) | MIRType::Bool | MIRType::Float(_)
    ) {
        let less = function.add_local(LocalKind::Temp, MIR_BOOL);
        function.push_inst_to_block(
            function.start_block,
            Instruction::Binary {
                destination: less,
                op: MirBinOp::Lt,
                left,
                right,
            },
        );
        let less_block = function.add_block();
        let not_less_block = function.add_block();
        function.basic_blocks[function.start_block].set_terminator(Terminator::If {
            cond: less,
            then_block: less_block,
            else_block: not_less_block,
        });
        let equal = function.add_local(LocalKind::Temp, MIR_BOOL);
        function.push_inst_to_block(
            not_less_block,
            Instruction::Binary {
                destination: equal,
                op: MirBinOp::Eq,
                left,
                right,
            },
        );
        let equal_block = function.add_block();
        let greater_block = function.add_block();
        function.basic_blocks[not_less_block].set_terminator(Terminator::If {
            cond: equal,
            then_block: equal_block,
            else_block: greater_block,
        });
        for (block, value) in [(less_block, -1), (equal_block, 0), (greater_block, 1)] {
            let result = function.add_local(LocalKind::Temp, MIR_I64);
            function.push_inst_to_block(
                block,
                Instruction::Assign {
                    destination: result,
                    value: MirConstant::Int(value),
                },
            );
            function.basic_blocks[block].set_terminator(Terminator::Return(Some(result)));
        }
    } else {
        let candidates = [format!("{suffix}_Ord_compare"), format!("{suffix}_compare")];
        let candidate = candidates
            .into_iter()
            .find(|candidate| ctx.is_known_function(candidate));
        let Some(candidate) = candidate else {
            ctx.errors.push(format!(
                "BTree key type `{suffix}` has no concrete Ord::compare implementation"
            ));
            let zero = function.add_local(LocalKind::Temp, MIR_I64);
            function.push_inst_to_block(
                function.start_block,
                Instruction::Assign {
                    destination: zero,
                    value: MirConstant::Int(0),
                },
            );
            function.basic_blocks[function.start_block]
                .set_terminator(Terminator::Return(Some(zero)));
            ctx.lambda_functions.push(function);
            ctx.insert_known_function(name.clone());
            ctx.insert_function_sig(
                name.clone(),
                FunctionSig {
                    ret_type: MIR_I64,
                    param_count: 2,
                    env: Vec::new(),
                },
            );
            return name;
        };
        let result = function.add_local(LocalKind::Temp, MIR_I64);
        function.push_inst_to_block(
            function.start_block,
            Instruction::Call {
                destination: result,
                func: candidate,
                args: vec![left, right_ptr],
            },
        );
        function.basic_blocks[function.start_block]
            .set_terminator(Terminator::Return(Some(result)));
    }
    ctx.lambda_functions.push(function);
    ctx.insert_known_function(name.clone());
    ctx.insert_function_sig(
        name.clone(),
        FunctionSig {
            ret_type: MIR_I64,
            param_count: 2,
            env: Vec::new(),
        },
    );
    name
}

fn lower_rc_new_call(ctx: &mut LoweringContext<'_>, value_local: Local) -> Local {
    let value_ty = ctx.get_local_type(value_local).clone();
    let Some(value_hir_ty) = ctx.concrete_type_registry.hir_type_for_mir(&value_ty) else {
        ctx.errors
            .push("rc_new<T>: concrete payload type could not be resolved".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let rc_hir_ty = HIRType::named("Rc".to_string(), vec![value_hir_ty]);
    let rc_name = crate::type_naming::hir_type_instance_name(&rc_hir_ty);
    ctx.concrete_type_registry
        .register_instance(rc_name.clone(), rc_hir_ty);
    let rc_ty = MIRType::Struct {
        name: rc_name.clone(),
        fields: vec![("handle".to_string(), MIR_I64)],
    };

    let drop_thunk = synthesize_rc_drop_thunk(ctx, &value_ty, &rc_name);
    let payload_source = materialize_rc_payload_source(ctx, value_local, &value_ty);

    let value_ptr = ctx.add_local(
        None,
        LocalKind::Temp,
        MIRType::Ptr(Box::new(value_ty.clone())),
    );
    ctx.push_inst(Instruction::AddrOf {
        destination: value_ptr,
        source: payload_source,
    });

    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let erased_value_ptr = ctx.add_local(None, LocalKind::Temp, i8_ptr.clone());
    ctx.push_inst(Instruction::Cast {
        destination: erased_value_ptr,
        value: value_ptr,
        to: i8_ptr.clone(),
    });

    let (payload_size, _) = crate::codegen::common::mir_type_size_align(&value_ty);
    let size_local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: size_local,
        value: MirConstant::Int(payload_size as i64),
    });

    let drop_fn_local = ctx.add_local(
        None,
        LocalKind::Temp,
        MIRType::Fn {
            params: vec![i8_ptr.clone()],
            ret: Box::new(MIR_UNIT),
        },
    );
    ctx.push_inst(Instruction::Assign {
        destination: drop_fn_local,
        value: MirConstant::GlobalRef(drop_thunk),
    });
    let erased_drop_fn = ctx.add_local(None, LocalKind::Temp, i8_ptr);
    ctx.push_inst(Instruction::Cast {
        destination: erased_drop_fn,
        value: drop_fn_local,
        to: MIRType::Ptr(Box::new(MIRType::Int(8))),
    });

    let handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Call {
        destination: handle,
        func: "sengoo_rc_new_copy".to_string(),
        args: vec![erased_value_ptr, size_local, erased_drop_fn],
    });

    let result = ctx.add_local(None, LocalKind::Temp, rc_ty.clone());
    ctx.type_names.insert(result, rc_name);
    ctx.push_inst(Instruction::Aggregate {
        destination: result,
        fields: vec![handle],
        ty: rc_ty,
    });
    ctx.mark_drop_local_moved(payload_source);
    result
}

/// Coerce an owned concrete value into an owned `dyn Trait` fat pointer. The
/// value is materialized into a stack slot the fat pointer's data field points
/// at; the slot's own drop responsibility transfers to the dyn value, which
/// drops through the vtable drop slot via the per-trait owned drop helper.
pub(super) fn emit_owned_dyn_coercion(
    ctx: &mut LoweringContext<'_>,
    value_local: Local,
    trait_name: &str,
) -> Local {
    let value_ty = ctx.get_local_type(value_local).clone();
    let slot = materialize_rc_payload_source(ctx, value_local, &value_ty);
    ctx.mark_drop_local_moved(slot);

    let value_ref = ctx.add_local(
        None,
        LocalKind::Temp,
        MIRType::Ref(Box::new(value_ty.clone())),
    );
    ctx.push_inst(Instruction::AddrOf {
        destination: value_ref,
        source: slot,
    });

    ensure_owned_dyn_drop_helper(ctx, trait_name);
    emit_dyn_coercion(ctx, value_ref, trait_name)
}

/// Register (and make known) the per-trait owned `dyn Trait` drop helper
/// `__dyn_Trait_Drop_drop`, synthesized after lowering from the recorded
/// requests. Its name matches the generic `{type}_Drop_drop` drop-glue lookup
/// for locals of the fat-pointer struct type.
pub(super) fn ensure_owned_dyn_drop_helper(
    ctx: &mut LoweringContext<'_>,
    trait_name: &str,
) -> String {
    let helper = format!(
        "{}_Drop_drop",
        crate::mir::dyn_dispatch::dyn_struct_name(trait_name)
    );
    ctx.options
        .dyn_owned_drop_requests
        .borrow_mut()
        .insert(trait_name.to_string());
    if !ctx.is_known_function(&helper) {
        ctx.insert_known_function(helper.clone());
        ctx.insert_function_sig(
            helper.clone(),
            FunctionSig {
                ret_type: MIR_UNIT,
                param_count: 1,
                env: Vec::new(),
            },
        );
    }
    helper
}

fn materialize_rc_payload_source(
    ctx: &mut LoweringContext<'_>,
    value_local: Local,
    value_ty: &MIRType,
) -> Local {
    if ctx.mir_fn.locals[value_local.index()].0.kind == LocalKind::User {
        return value_local;
    }

    let slot = ctx.add_local(None, LocalKind::User, value_ty.clone());
    if let Some(type_name) = ctx.type_names.get(&value_local).cloned() {
        ctx.type_names.insert(slot, type_name);
    }
    ctx.push_inst(Instruction::Store {
        destination: slot,
        value: value_local,
    });
    slot
}

fn synthesize_rc_drop_thunk(
    ctx: &mut LoweringContext<'_>,
    payload_ty: &MIRType,
    rc_name: &str,
) -> String {
    let thunk_name = format!("__sengoo_rc_drop_{}", rc_name);
    if ctx.is_known_function(&thunk_name) {
        return thunk_name;
    }

    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let mut thunk = MirFunction::new(thunk_name.clone(), vec![i8_ptr.clone()], MIR_UNIT);
    let start_block = thunk.start_block;
    let mut thunk_ctx = LoweringContext::new(
        &mut thunk,
        ctx.lambda_counter,
        ctx.known_functions_base,
        ctx.function_sigs_base,
        ctx.struct_defs,
        ctx.concrete_type_registry.clone(),
        ctx.options.clone(),
        ctx.inherent_method_templates,
        ctx.trait_method_templates,
    );
    thunk_ctx.known_functions_overlay = ctx.known_functions_overlay.clone();
    thunk_ctx.function_sigs_overlay = ctx.function_sigs_overlay.clone();
    thunk_ctx.insert_known_function(thunk_name.clone());
    thunk_ctx.insert_function_sig(
        thunk_name.clone(),
        FunctionSig {
            ret_type: MIR_UNIT,
            param_count: 1,
            env: Vec::new(),
        },
    );
    thunk_ctx.set_current_block(start_block);

    let data = Local::new(1, LocalKind::Param);
    let typed_ptr = thunk_ctx.add_local(
        None,
        LocalKind::Temp,
        MIRType::Ptr(Box::new(payload_ty.clone())),
    );
    thunk_ctx.push_inst(Instruction::Cast {
        destination: typed_ptr,
        value: data,
        to: MIRType::Ptr(Box::new(payload_ty.clone())),
    });
    let loaded_payload = thunk_ctx.add_local(None, LocalKind::Temp, payload_ty.clone());
    thunk_ctx.push_inst(Instruction::Load {
        destination: loaded_payload,
        source: typed_ptr,
    });
    let payload = thunk_ctx.add_local(None, LocalKind::User, payload_ty.clone());
    thunk_ctx.push_inst(Instruction::Store {
        destination: payload,
        value: loaded_payload,
    });
    thunk_ctx.record_drop_binding_if_needed(payload);
    thunk_ctx
        .mir_fn
        .basic_blocks
        .get_mut(start_block)
        .expect("rc drop thunk start block exists")
        .set_terminator(Terminator::Return(None));
    thunk_ctx.insert_drop_glue();

    let generated = thunk_ctx.mir_fn.clone();
    ctx.lambda_functions.push(generated);
    ctx.insert_known_function(thunk_name.clone());
    ctx.insert_function_sig(
        thunk_name.clone(),
        FunctionSig {
            ret_type: MIR_UNIT,
            param_count: 1,
            env: Vec::new(),
        },
    );
    thunk_name
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
        let result = lower_named_call(&mut ctx, "task_status", &[task], None);

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

        let result = lower_named_call(&mut ctx, "worker", &[], None);

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
