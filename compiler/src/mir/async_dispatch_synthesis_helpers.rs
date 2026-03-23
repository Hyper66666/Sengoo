use super::async_dispatch_helpers::AsyncDispatchRegistry;
use crate::mir::{
    Instruction, Local, LocalKind, MIRType, MirConstant, MirFunction, Terminator, MIR_BOOL,
    MIR_I64, MIR_UNIT,
};
use crate::CompileError;

pub fn select_result_runtime_suffix(ty: &MIRType) -> Option<&'static str> {
    match ty {
        MIRType::Bool => Some("bool"),
        MIRType::Int(8) => Some("i8"),
        MIRType::Int(16) => Some("i16"),
        MIRType::Int(32) => Some("i32"),
        MIRType::Int(64) => Some("i64"),
        MIRType::Float(32) => Some("f32"),
        MIRType::Float(64) => Some("f64"),
        _ => None,
    }
}

pub fn select_runtime_function_name(ty: &MIRType) -> Option<String> {
    select_result_runtime_suffix(ty).map(|suffix| format!("sengoo_async_select_{suffix}"))
}

pub fn select_result_dispatch_name(ty: &MIRType) -> Option<String> {
    select_result_runtime_suffix(ty).map(|suffix| format!("sengoo_async_result_dispatch_{suffix}"))
}

pub fn select_runtime_return_type(ty: &MIRType) -> Option<MIRType> {
    match ty {
        MIRType::Bool => Some(MIR_BOOL),
        MIRType::Int(8) => Some(MIRType::Int(8)),
        MIRType::Int(16) => Some(MIRType::Int(16)),
        MIRType::Int(32) => Some(MIRType::Int(32)),
        MIRType::Int(64) => Some(MIR_I64),
        MIRType::Float(32) => Some(MIRType::Float(32)),
        MIRType::Float(64) => Some(MIRType::Float(64)),
        _ => None,
    }
}

pub fn select_runtime_declaration(ty: &MIRType) -> Option<String> {
    let name = select_runtime_function_name(ty)?;
    let ret = match select_runtime_return_type(ty)? {
        MIRType::Bool => "i1".to_string(),
        MIRType::Int(bits) => format!("i{bits}"),
        MIRType::Float(32) => "float".to_string(),
        MIRType::Float(64) => "double".to_string(),
        _ => return None,
    };
    Some(format!("declare {ret} @{name}(i64, i64, i64, i64)\n"))
}

fn select_result_default_constant(ty: &MIRType) -> Option<MirConstant> {
    match ty {
        MIRType::Bool => Some(MirConstant::Bool(false)),
        MIRType::Int(_) => Some(MirConstant::Int(0)),
        MIRType::Float(_) => Some(MirConstant::Float(0.0)),
        _ => None,
    }
}

fn dispatch_switch_key(
    registry: &AsyncDispatchRegistry,
    base_name: &str,
) -> Result<u32, CompileError> {
    let kind = registry.kind_id(base_name).ok_or_else(|| {
        CompileError::MirLower(format!(
            "missing async dispatch id for future origin `{base_name}`"
        ))
    })?;
    u32::try_from(kind).map_err(|_| {
        CompileError::MirLower(format!(
            "async dispatch id for future origin `{base_name}` does not fit switch key width"
        ))
    })
}

pub fn synthesize_spawn_poll_dispatch(
    registry: &AsyncDispatchRegistry,
    entries: &[(String, String)],
) -> Result<MirFunction, CompileError> {
    let mut f = MirFunction::new(
        "sengoo_async_poll_dispatch".to_string(),
        vec![MIR_I64, MIR_I64],
        MIR_I64,
    );

    let bb0 = f.start_block;
    let kind_local = Local::new(1, LocalKind::Param);
    let handle_local = Local::new(2, LocalKind::Param);
    let default_block = f.add_block();
    let mut targets = Vec::with_capacity(entries.len());

    for (base_name, poll_name) in entries {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, MIR_I64);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: poll_name.clone(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((dispatch_switch_key(registry, base_name)?, case_block));
    }

    if !entries.iter().any(|(base_name, _)| base_name == "sengoo_async_sleep") {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, MIR_I64);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_sleep__poll".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((dispatch_switch_key(registry, "sengoo_async_sleep")?, case_block));
    }

    if !entries
        .iter()
        .any(|(base_name, _)| base_name == "sengoo_async_timeout_bool")
    {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, MIR_I64);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_timeout_bool__poll".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((
            dispatch_switch_key(registry, "sengoo_async_timeout_bool")?,
            case_block,
        ));
    }

    f.basic_blocks[bb0].set_terminator(Terminator::Switch {
        discr: kind_local,
        targets,
        otherwise: default_block,
    });

    let ready_local = f.add_local(LocalKind::Temp, MIR_I64);
    let ready_inst = f.alloc_inst(Instruction::Assign {
        destination: ready_local,
        value: MirConstant::Int(1),
    });
    f.basic_blocks[default_block].push(ready_inst);
    f.basic_blocks[default_block].set_terminator(Terminator::Return(Some(ready_local)));

    Ok(f)
}

pub fn synthesize_spawn_cancel_dispatch(
    registry: &AsyncDispatchRegistry,
    entries: &[(String, String)],
) -> Result<MirFunction, CompileError> {
    let mut f = MirFunction::new(
        "sengoo_async_cancel_dispatch".to_string(),
        vec![MIR_I64, MIR_I64],
        MIR_BOOL,
    );

    let bb0 = f.start_block;
    let kind_local = Local::new(1, LocalKind::Param);
    let handle_local = Local::new(2, LocalKind::Param);
    let default_block = f.add_block();
    let mut targets = Vec::with_capacity(entries.len());

    for (base_name, _) in entries {
        let case_block = f.add_block();
        let free_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: free_dest,
            func: "sengoo_async_frame_free".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        let true_local = f.add_local(LocalKind::Temp, MIR_BOOL);
        let true_inst = f.alloc_inst(Instruction::Assign {
            destination: true_local,
            value: MirConstant::Bool(true),
        });
        f.basic_blocks[case_block].push(true_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(true_local)));
        targets.push((dispatch_switch_key(registry, base_name)?, case_block));
    }

    if !entries.iter().any(|(base_name, _)| base_name == "sengoo_async_sleep") {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, MIR_BOOL);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_sleep__cancel".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((dispatch_switch_key(registry, "sengoo_async_sleep")?, case_block));
    }

    if !entries
        .iter()
        .any(|(base_name, _)| base_name == "sengoo_async_timeout_bool")
    {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, MIR_BOOL);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_timeout_bool__cancel".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((
            dispatch_switch_key(registry, "sengoo_async_timeout_bool")?,
            case_block,
        ));
    }

    f.basic_blocks[bb0].set_terminator(Terminator::Switch {
        discr: kind_local,
        targets,
        otherwise: default_block,
    });

    let false_local = f.add_local(LocalKind::Temp, MIR_BOOL);
    let false_inst = f.alloc_inst(Instruction::Assign {
        destination: false_local,
        value: MirConstant::Bool(false),
    });
    f.basic_blocks[default_block].push(false_inst);
    f.basic_blocks[default_block].set_terminator(Terminator::Return(Some(false_local)));

    Ok(f)
}

pub fn synthesize_spawn_drop_dispatch(
    registry: &AsyncDispatchRegistry,
    entries: &[(String, String)],
) -> Result<MirFunction, CompileError> {
    let mut f = MirFunction::new(
        "sengoo_async_drop_dispatch".to_string(),
        vec![MIR_I64, MIR_I64],
        MIR_UNIT,
    );

    let bb0 = f.start_block;
    let kind_local = Local::new(1, LocalKind::Param);
    let handle_local = Local::new(2, LocalKind::Param);
    let default_block = f.add_block();
    let mut targets = Vec::with_capacity(entries.len());

    for (base_name, _) in entries {
        let case_block = f.add_block();
        let free_dest = f.add_local(LocalKind::Temp, MIR_UNIT);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: free_dest,
            func: "sengoo_async_frame_free".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(None));
        targets.push((dispatch_switch_key(registry, base_name)?, case_block));
    }

    if !entries.iter().any(|(base_name, _)| base_name == "sengoo_async_sleep") {
        let case_block = f.add_block();
        let unit_local = f.add_local(LocalKind::Temp, MIR_UNIT);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: unit_local,
            func: "sengoo_async_sleep__drop".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(None));
        targets.push((dispatch_switch_key(registry, "sengoo_async_sleep")?, case_block));
    }

    if !entries
        .iter()
        .any(|(base_name, _)| base_name == "sengoo_async_timeout_bool")
    {
        let case_block = f.add_block();
        let unit_local = f.add_local(LocalKind::Temp, MIR_UNIT);
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: unit_local,
            func: "sengoo_async_timeout_bool__drop".to_string(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(None));
        targets.push((
            dispatch_switch_key(registry, "sengoo_async_timeout_bool")?,
            case_block,
        ));
    }

    f.basic_blocks[bb0].set_terminator(Terminator::Switch {
        discr: kind_local,
        targets,
        otherwise: default_block,
    });
    f.basic_blocks[default_block].set_terminator(Terminator::Return(None));

    Ok(f)
}

pub fn synthesize_result_dispatch(
    registry: &AsyncDispatchRegistry,
    return_ty: &MIRType,
    entries: &[(String, String)],
) -> Result<MirFunction, CompileError> {
    let dispatch_name = select_result_dispatch_name(return_ty).ok_or_else(|| {
        CompileError::MirLower(format!(
            "unsupported async result dispatch type `{:?}`",
            return_ty
        ))
    })?;
    let default_value = select_result_default_constant(return_ty).ok_or_else(|| {
        CompileError::MirLower(format!(
            "async result dispatch has no default value for unsupported type `{:?}`",
            return_ty
        ))
    })?;

    let mut f = MirFunction::new(dispatch_name, vec![MIR_I64, MIR_I64], return_ty.clone());

    let bb0 = f.start_block;
    let kind_local = Local::new(1, LocalKind::Param);
    let handle_local = Local::new(2, LocalKind::Param);
    let default_block = f.add_block();
    let mut targets = Vec::with_capacity(entries.len());

    for (base_name, result_name) in entries {
        let case_block = f.add_block();
        let result_local = f.add_local(LocalKind::Temp, return_ty.clone());
        let call_inst = f.alloc_inst(Instruction::Call {
            destination: result_local,
            func: result_name.clone(),
            args: vec![handle_local],
        });
        f.basic_blocks[case_block].push(call_inst);
        f.basic_blocks[case_block].set_terminator(Terminator::Return(Some(result_local)));
        targets.push((dispatch_switch_key(registry, base_name)?, case_block));
    }

    f.basic_blocks[bb0].set_terminator(Terminator::Switch {
        discr: kind_local,
        targets,
        otherwise: default_block,
    });

    let default_local = f.add_local(LocalKind::Temp, return_ty.clone());
    let default_inst = f.alloc_inst(Instruction::Assign {
        destination: default_local,
        value: default_value,
    });
    f.basic_blocks[default_block].push(default_inst);
    f.basic_blocks[default_block].set_terminator(Terminator::Return(Some(default_local)));

    Ok(f)
}
