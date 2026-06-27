use super::*;

#[derive(Debug, Clone)]
pub(super) enum TryScope {
    Function,
    TryBlock {
        merge_block: usize,
        drop_scope_depth: usize,
        result_local: Option<Local>,
        /// Inferred `Result` / `Option` container from the first `?` in this block.
        container_ty: Option<MIRType>,
    },
}

fn result_struct_ty(value_ty: MIRType, err_ty: MIRType) -> MIRType {
    MIRType::Struct {
        name: "Result".to_string(),
        fields: vec![
            ("is_ok".to_string(), MIR_BOOL),
            ("value".to_string(), value_ty),
            ("error".to_string(), err_ty),
        ],
    }
}

fn mir_is_result_or_option(ty: &MIRType) -> bool {
    match ty {
        MIRType::Struct { name, .. } => {
            name == "Result"
                || name == "Option"
                || name.starts_with("Result_")
                || name.starts_with("Option_")
        }
        _ => false,
    }
}

fn default_error_value(ctx: &mut LoweringContext<'_>, err_ty: &MIRType) -> Local {
    match err_ty {
        MIRType::Bool => ctx.lower_literal(&HIRLiteral::Bool(false)),
        MIRType::Int(_) => ctx.lower_literal(&HIRLiteral::Int(0)),
        _ => ctx.lower_literal(&HIRLiteral::Int(0)),
    }
}

fn ensure_try_block_result_local(ctx: &mut LoweringContext<'_>, ty: MIRType) -> Local {
    let scope_idx = ctx.try_scope_stack.len() - 1;
    if let TryScope::TryBlock {
        result_local: Some(local),
        ..
    } = ctx.try_scope_stack[scope_idx]
    {
        return local;
    }
    let local = ctx.add_local(None, LocalKind::User, ty);
    if let TryScope::TryBlock { result_local, .. } = &mut ctx.try_scope_stack[scope_idx] {
        *result_local = Some(local);
    }
    local
}

fn record_try_block_container(ctx: &mut LoweringContext<'_>, container_ty: MIRType) {
    let scope_idx = ctx.try_scope_stack.len() - 1;
    if let TryScope::TryBlock {
        container_ty: slot, ..
    } = &mut ctx.try_scope_stack[scope_idx]
    {
        if slot.is_none() {
            *slot = Some(container_ty);
        }
    }
}

fn try_block_container_ty(ctx: &LoweringContext<'_>) -> Option<MIRType> {
    match ctx.try_scope_stack.last() {
        Some(TryScope::TryBlock {
            container_ty: Some(ty),
            ..
        }) => Some(ty.clone()),
        _ => None,
    }
}

fn wrap_success(
    ctx: &mut LoweringContext<'_>,
    value_local: Local,
    container_ty: &MIRType,
) -> Local {
    match container_ty {
        MIRType::Struct { name, fields, .. } if name == "Option" || name.starts_with("Option_") => {
            let out = ctx.add_local(None, LocalKind::Temp, container_ty.clone());
            let flag = ctx.lower_literal(&HIRLiteral::Bool(true));
            ctx.push_inst(Instruction::Aggregate {
                destination: out,
                fields: vec![flag, value_local],
                ty: container_ty.clone(),
            });
            out
        }
        MIRType::Struct { name, fields, .. } if name == "Result" || name.starts_with("Result_") => {
            let out = ctx.add_local(None, LocalKind::Temp, container_ty.clone());
            let flag = ctx.lower_literal(&HIRLiteral::Bool(true));
            let err_ty = fields
                .iter()
                .find(|(n, _)| n == "error")
                .map(|(_, ty)| ty.clone())
                .unwrap_or(MIR_I64);
            let err_local = ctx.add_local(None, LocalKind::Temp, err_ty.clone());
            let zero = default_error_value(ctx, &err_ty);
            ctx.push_inst(Instruction::Store {
                destination: err_local,
                value: zero,
            });
            ctx.push_inst(Instruction::Aggregate {
                destination: out,
                fields: vec![flag, value_local, err_local],
                ty: container_ty.clone(),
            });
            out
        }
        _ => value_local,
    }
}

pub(super) fn lower_try_expr(ctx: &mut LoweringContext<'_>, operand: &HIRExpr) -> Local {
    let operand_local = ctx.lower_expr(operand);
    let operand_ty = ctx.get_local_type(operand_local).clone();
    let flag_field = match &operand_ty {
        MIRType::Struct { name, .. } if name == "Option" || name.starts_with("Option_") => {
            "is_some"
        }
        _ => "is_ok",
    };

    let flag_local = lower_field_expr(ctx, operand_local, flag_field);
    let cont_block = ctx.new_block();
    let fail_block = ctx.new_block();

    ctx.set_terminator(Terminator::If {
        cond: flag_local,
        then_block: cont_block,
        else_block: fail_block,
    });

    ctx.set_current_block(fail_block);
    match ctx.try_scope_stack.last().cloned() {
        Some(TryScope::Function) | None => {
            ctx.emit_active_drop_scopes_before_exit();
            ctx.set_terminator(Terminator::Return(Some(operand_local)));
        }
        Some(TryScope::TryBlock {
            merge_block,
            drop_scope_depth,
            ..
        }) => {
            ctx.emit_drop_scopes_from_depth(drop_scope_depth);
            record_try_block_container(ctx, operand_ty.clone());
            let result_local = ensure_try_block_result_local(ctx, operand_ty.clone());
            ctx.push_inst(Instruction::Store {
                destination: result_local,
                value: operand_local,
            });
            ctx.set_terminator(Terminator::Goto(merge_block));
        }
    }

    ctx.set_current_block(cont_block);
    lower_field_expr(ctx, operand_local, "value")
}

pub(super) fn lower_try_block_expr(ctx: &mut LoweringContext<'_>, body: &HIRBody) -> Local {
    let entry_block = ctx.current_block();
    let merge_block = ctx.new_block();
    let drop_scope_depth = ctx.drop_scope_depth();

    ctx.push_try_scope(TryScope::TryBlock {
        merge_block,
        drop_scope_depth,
        result_local: None,
        container_ty: None,
    });

    let body_val = ctx.lower_scoped_body_to_block_val(body, entry_block);
    let body_ty = ctx.get_local_type(body_val).clone();

    let mut out_local = body_val;
    let end = ctx.current_block();
    if ctx
        .mir_fn
        .block_mut(end)
        .is_some_and(|b| b.terminator.is_none())
    {
        if mir_is_result_or_option(&body_ty) {
            if let Some(result_local) = match ctx.try_scope_stack.last() {
                Some(TryScope::TryBlock { result_local, .. }) => *result_local,
                _ => None,
            } {
                ctx.push_inst(Instruction::Store {
                    destination: result_local,
                    value: body_val,
                });
                out_local = result_local;
            }
        } else if let Some(container_ty) = try_block_container_ty(ctx) {
            let wrapped = wrap_success(ctx, body_val, &container_ty);
            let result_local = ensure_try_block_result_local(ctx, container_ty);
            ctx.push_inst(Instruction::Store {
                destination: result_local,
                value: wrapped,
            });
            out_local = result_local;
        } else {
            let container_ty = result_struct_ty(ctx.get_local_type(body_val).clone(), MIR_I64);
            let wrapped = wrap_success(ctx, body_val, &container_ty);
            let result_local = ensure_try_block_result_local(ctx, container_ty);
            ctx.push_inst(Instruction::Store {
                destination: result_local,
                value: wrapped,
            });
            out_local = result_local;
        }
        ctx.set_terminator(Terminator::Goto(merge_block));
    } else if let Some(TryScope::TryBlock {
        result_local: Some(local),
        ..
    }) = ctx.try_scope_stack.last()
    {
        out_local = *local;
    }

    ctx.pop_try_scope();
    ctx.set_current_block(merge_block);
    out_local
}
