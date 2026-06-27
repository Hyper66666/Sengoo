use super::method_builtin_helpers::try_lower_string_len_method_call;
use super::*;
use crate::mir::method_dispatch_helpers::{build_method_dispatch_plan, MethodDispatchPlan};
use crate::mir::trait_dispatch_helpers::resolve_known_trait_method_name;

#[cfg(test)]
pub(super) fn resolve_method_call_name<'a, I, FInherent, FTrait>(
    dispatch_plan: &MethodDispatchPlan,
    method: &str,
    arg_count: usize,
    known_functions: I,
    try_materialize_inherent: FInherent,
    try_materialize_trait: FTrait,
) -> Result<String, String>
where
    I: IntoIterator<Item = (&'a str, usize)>,
    FInherent: FnOnce() -> Option<String>,
    FTrait: FnOnce(&str) -> Result<Option<String>, String>,
{
    let known_functions: Vec<(&str, usize)> = known_functions.into_iter().collect();

    if known_functions
        .iter()
        .any(|(name, _)| *name == dispatch_plan.func_name.as_str())
    {
        return Ok(dispatch_plan.func_name.clone());
    }
    if let Some(generated_name) = try_materialize_inherent() {
        return Ok(generated_name);
    }
    if let Some(generated_name) = try_materialize_trait(&dispatch_plan.type_display)? {
        return Ok(generated_name);
    }

    resolve_known_trait_method_name(
        known_functions.iter().copied(),
        &dispatch_plan.type_prefix,
        method,
        &dispatch_plan.func_name,
        arg_count,
        &dispatch_plan.type_display,
    )
}

pub(super) fn resolve_method_call_name_with_ctx<'a, I>(
    ctx: &mut LoweringContext<'_>,
    dispatch_plan: &MethodDispatchPlan,
    receiver_ty: &MIRType,
    method: &str,
    arg_locals: &[Local],
    known_functions: I,
) -> Result<String, String>
where
    I: IntoIterator<Item = (&'a str, usize)>,
{
    let known_functions: Vec<(&str, usize)> = known_functions.into_iter().collect();

    if known_functions
        .iter()
        .any(|(name, _)| *name == dispatch_plan.func_name.as_str())
    {
        return Ok(dispatch_plan.func_name.clone());
    }
    if let Some(generated_name) =
        ctx.try_materialize_inherent_method(receiver_ty, method, arg_locals)
    {
        return Ok(generated_name);
    }
    if let Some(generated_name) = ctx.try_materialize_trait_method(
        receiver_ty,
        method,
        arg_locals,
        &dispatch_plan.type_display,
    )? {
        return Ok(generated_name);
    }

    resolve_known_trait_method_name(
        known_functions.iter().copied(),
        &dispatch_plan.type_prefix,
        method,
        &dispatch_plan.func_name,
        arg_locals.len(),
        &dispatch_plan.type_display,
    )
}

pub(super) fn resolve_method_call_target_with_ctx(
    ctx: &mut LoweringContext<'_>,
    receiver_local: Local,
    method: &str,
    arg_locals: &[Local],
) -> Result<String, String> {
    let receiver_ty = ctx.get_local_type(receiver_local).clone();
    let explicit_type_name = ctx.type_names.get(&receiver_local).map(String::as_str);
    let dispatch_plan = build_method_dispatch_plan(explicit_type_name, &receiver_ty, method);

    let known_function_entries: Vec<(String, usize)> = ctx
        .known_function_names()
        .map(|name| {
            (
                name.clone(),
                ctx.function_sig(name)
                    .map(|sig| sig.param_count)
                    .unwrap_or(0),
            )
        })
        .collect();

    resolve_method_call_name_with_ctx(
        ctx,
        &dispatch_plan,
        &receiver_ty,
        method,
        arg_locals,
        known_function_entries
            .iter()
            .map(|(name, arity)| (name.as_str(), *arity)),
    )
}

pub(super) fn emit_resolved_method_call(
    ctx: &mut LoweringContext<'_>,
    receiver_local: Local,
    arg_locals: &[Local],
    resolved_func_name: &str,
) -> Local {
    ctx.mark_drop_locals_moved(arg_locals);

    let mut ret_type = ctx
        .function_sig(resolved_func_name)
        .map(|sig| sig.ret_type.clone())
        .unwrap_or(MIR_I64);
    let mut future_origin = None;
    if resolved_func_name == "HttpServer_next_request_async" {
        ret_type = MIRType::Future(Box::new(http_server_next_request_outcome_mir_type()));
        future_origin = Some("sengoo_http_server_next_request_async".to_string());
    }
    let struct_type_name = match &ret_type {
        MIRType::Struct { name, .. } => Some(name.clone()),
        _ => None,
    };
    let result_local = ctx.add_local(None, LocalKind::Temp, ret_type);
    if let Some(name) = struct_type_name {
        ctx.type_names.insert(result_local, name);
    }

    let mut call_args = vec![receiver_local];
    call_args.extend(arg_locals.iter().copied());
    ctx.push_inst(Instruction::Call {
        destination: result_local,
        func: resolved_func_name.to_string(),
        args: call_args,
    });
    if let Some(origin) = future_origin {
        ctx.future_origins.insert(result_local, origin);
    }

    result_local
}

pub(super) fn lower_method_call_from_locals(
    ctx: &mut LoweringContext<'_>,
    receiver_local: Local,
    method: &str,
    arg_locals: &[Local],
) -> Local {
    if is_explicit_release_method(method) {
        ctx.mark_drop_local_moved(receiver_local);
    }

    if let Some(result_local) = try_lower_string_len_method_call(ctx, receiver_local, method) {
        return result_local;
    }

    if let Some(result_local) =
        try_lower_dyn_method_dispatch(ctx, receiver_local, method, arg_locals)
    {
        return result_local;
    }

    let resolved_func_name =
        match resolve_method_call_target_with_ctx(ctx, receiver_local, method, arg_locals) {
            Ok(name) => name,
            Err(error) => {
                ctx.errors.push(error);
                return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
            }
        };
    emit_resolved_method_call(ctx, receiver_local, arg_locals, &resolved_func_name)
}

fn is_explicit_release_method(method: &str) -> bool {
    matches!(method, "drop" | "free" | "close")
}

/// Lower a method call on a `&dyn Trait` receiver into a vtable-backed indirect
/// call. Returns `None` for non-`dyn` receivers so the normal monomorphic path
/// runs. The fat pointer is `{ data: i8*, vtable: i8* }`; the slot index is the
/// method's position in the trait's deterministic vtable layout.
fn try_lower_dyn_method_dispatch(
    ctx: &mut LoweringContext<'_>,
    receiver_local: Local,
    method: &str,
    arg_locals: &[Local],
) -> Option<Local> {
    let receiver_ty = ctx.get_local_type(receiver_local).clone();
    let trait_name = crate::mir::dyn_dispatch::dyn_trait_of_type(&receiver_ty)?.to_string();

    let resolved = ctx
        .options
        .trait_method_order
        .get(&trait_name)
        .and_then(|slots| {
            slots
                .iter()
                .position(|slot| slot.name == method)
                .map(|index| (index, slots[index].ret.clone()))
        });
    let (slot, ret_ty) = match resolved {
        Some(found) => found,
        None => {
            ctx.errors.push(format!(
                "dyn dispatch: method '{}' not found on trait '{}'",
                method, trait_name
            ));
            return Some(ctx.add_local(None, LocalKind::Temp, MIR_UNIT));
        }
    };

    ctx.mark_drop_locals_moved(arg_locals);

    let i8_ptr = MIRType::Ptr(Box::new(MIRType::Int(8)));
    let i64_ptr = MIRType::Ptr(Box::new(MIR_I64));

    // data = extractvalue receiver, 0  (concrete receiver as i8*)
    let data_local = ctx.add_local(None, LocalKind::Temp, i8_ptr.clone());
    ctx.push_inst(Instruction::Extract {
        destination: data_local,
        value: receiver_local,
        index: 0,
    });

    // vtable = extractvalue receiver, 1  (i8* to the function-pointer table)
    let vtable_i8 = ctx.add_local(None, LocalKind::Temp, i8_ptr);
    ctx.push_inst(Instruction::Extract {
        destination: vtable_i8,
        value: receiver_local,
        index: 1,
    });

    // Reinterpret the table as `i64*` so each slot holds one pointer-sized word.
    let vtable_words = ctx.add_local(None, LocalKind::Temp, i64_ptr.clone());
    ctx.push_inst(Instruction::Cast {
        destination: vtable_words,
        value: vtable_i8,
        to: i64_ptr,
    });

    // slot index constant
    let slot_local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: slot_local,
        value: MirConstant::Int(slot as i64),
    });

    // slot_addr = &vtable[slot]
    let slot_addr = ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(MIR_I64)));
    ctx.push_inst(Instruction::IndexAddr {
        destination: slot_addr,
        base: vtable_words,
        index: slot_local,
    });

    // fnptr (as an integer word) = load slot_addr; CallIndirect reinterprets it.
    let fnptr = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Load {
        destination: fnptr,
        source: slot_addr,
    });

    let struct_type_name = match &ret_ty {
        MIRType::Struct { name, .. } => Some(name.clone()),
        _ => None,
    };
    let result_local = ctx.add_local(None, LocalKind::Temp, ret_ty);
    if let Some(name) = struct_type_name {
        ctx.type_names.insert(result_local, name);
    }

    let mut call_args = vec![data_local];
    call_args.extend(arg_locals.iter().copied());
    ctx.push_inst(Instruction::CallIndirect {
        destination: result_local,
        func_ptr: fnptr,
        args: call_args,
    });

    Some(result_local)
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

        let receiver = ctx.add_local(
            None,
            LocalKind::Temp,
            MIRType::Ptr(Box::new(MIRType::Int(8))),
        );
        let result = lower_method_call_from_locals(&mut ctx, receiver, "len", &[]);

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, args, .. } if func == "sengoo_str_len" && args == &vec![receiver]
        )));
    }

    #[test]
    fn resolve_method_call_name_prefers_known_function_before_materializers() {
        let plan = MethodDispatchPlan {
            func_name: "i64_abs".to_string(),
            type_display: "i64".to_string(),
            type_prefix: "i64".to_string(),
        };

        let result = resolve_method_call_name(
            &plan,
            "abs",
            0,
            [("i64_abs", 0usize)],
            || panic!("inherent materialization should not run"),
            |_| panic!("trait materialization should not run"),
        );

        assert_eq!(result.unwrap(), "i64_abs");
    }

    #[test]
    fn resolve_method_call_name_falls_back_to_known_trait_candidates() {
        let plan = MethodDispatchPlan {
            func_name: "i64_abs".to_string(),
            type_display: "i64".to_string(),
            type_prefix: "i64".to_string(),
        };

        let result = resolve_method_call_name(
            &plan,
            "abs",
            0,
            [("i64_Number_abs", 0usize)],
            || None,
            |_| Ok(None),
        );

        assert_eq!(result.unwrap(), "i64_Number_abs");
    }

    #[test]
    fn resolve_method_call_target_with_ctx_uses_explicit_type_name_for_known_function() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::from(["Point_sum".to_string()]);
        let mut function_sigs = HashMap::new();
        function_sigs.insert(
            "Point_sum".to_string(),
            FunctionSig {
                env: Vec::new(),
                param_count: 1,
                ret_type: MIR_I64,
            },
        );
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
            MIRType::Struct {
                name: "Point".to_string(),
                fields: vec![],
            },
        );
        ctx.type_names.insert(receiver, "Point".to_string());

        let result = resolve_method_call_target_with_ctx(&mut ctx, receiver, "sum", &[]);

        assert_eq!(result.unwrap(), "Point_sum");
    }
    #[test]
    fn emit_resolved_method_call_tracks_struct_return_type_name() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let mut function_sigs = HashMap::new();
        function_sigs.insert(
            "Point_sum".to_string(),
            FunctionSig {
                env: Vec::new(),
                param_count: 1,
                ret_type: MIRType::Struct {
                    name: "Point".to_string(),
                    fields: vec![],
                },
            },
        );
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
            MIRType::Struct {
                name: "Point".to_string(),
                fields: vec![],
            },
        );

        let result = emit_resolved_method_call(&mut ctx, receiver, &[], "Point_sum");

        assert_eq!(
            ctx.get_local_type(result),
            &MIRType::Struct {
                name: "Point".to_string(),
                fields: vec![],
            }
        );
        assert_eq!(
            ctx.type_names.get(&result).map(String::as_str),
            Some("Point")
        );
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
        assert!(ctx
            .errors
            .iter()
            .any(|e| e.contains("method 'missing' not found for type 'i64'")));
    }
}
