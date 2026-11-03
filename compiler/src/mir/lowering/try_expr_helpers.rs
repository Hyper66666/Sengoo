use super::*;
use crate::mir::enum_defs::EnumDef;

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
        MIRType::Struct { name, .. } | MIRType::Enum { name, .. } => {
            name == "Result"
                || name == "Option"
                || name.starts_with("Result_")
                || name.starts_with("Option_")
        }
        _ => false,
    }
}

/// Declaration behind a monomorphised enum instance name such as `Option_i64`.
///
/// Instance names are `{enum}_{args}`, so the longest declared name that
/// prefixes the instance is the owner; an exact hit covers non-generic enums.
fn enum_def_for_instance<'d>(
    ctx: &'d LoweringContext<'_>,
    instance_name: &str,
) -> Option<&'d EnumDef> {
    if let Some(def) = ctx.options.enum_defs.get(instance_name) {
        return Some(def);
    }
    ctx.options
        .enum_defs
        .iter()
        .filter(|(name, _)| instance_name.starts_with(&format!("{name}_")))
        .max_by_key(|(name, _)| name.len())
        .map(|(_, def)| def)
}

/// `(success, failure)` discriminants of an `Option`/`Result`-shaped enum.
///
/// Resolved from variant names rather than positions: `Result` succeeds on its
/// first variant and `Option` on its second, so an index would be wrong for one
/// of them.
fn try_variant_discriminants(ctx: &LoweringContext<'_>, ty: &MIRType) -> Option<(u32, u32)> {
    let MIRType::Enum { name, .. } = ty else {
        return None;
    };
    let def = enum_def_for_instance(ctx, name)?;
    let success = def
        .variant_discriminant("Ok")
        .or_else(|| def.variant_discriminant("Some"))?;
    let failure = def
        .variant_discriminant("Err")
        .or_else(|| def.variant_discriminant("None"))?;
    Some((success, failure))
}

/// Payload carried by the failure arm of `operand_local`, already known to be on
/// the failure branch. `None` when the failure variant is payload-free (`None`).
fn failure_payload(
    ctx: &mut LoweringContext<'_>,
    operand_local: Local,
    operand_ty: &MIRType,
) -> Option<Local> {
    match operand_ty {
        MIRType::Enum { .. } => {
            let failure = try_variant_discriminants(ctx, operand_ty)?.1;
            let payload_ty = EnumDef::instance_variant_payload(operand_ty, failure)?;
            let payload = ctx.add_local(None, LocalKind::Temp, payload_ty);
            ctx.push_inst(Instruction::ExtractPayload {
                destination: payload,
                source: operand_local,
            });
            Some(payload)
        }
        MIRType::Struct { fields, .. } if fields.iter().any(|(name, _)| name == "error") => {
            Some(lower_field_expr(ctx, operand_local, "error"))
        }
        _ => None,
    }
}

fn default_error_value(ctx: &mut LoweringContext<'_>, err_ty: &MIRType) -> Local {
    match err_ty {
        MIRType::Bool => ctx.lower_literal(&HIRLiteral::Bool(false)),
        MIRType::Int(_) => ctx.lower_literal(&HIRLiteral::Int(0)),
        MIRType::UInt(_) => ctx.lower_literal(&HIRLiteral::Uint(0)),
        _ => ctx.lower_literal(&HIRLiteral::Int(0)),
    }
}

pub(super) fn default_value_for_type(ctx: &mut LoweringContext<'_>, ty: &MIRType) -> Local {
    let destination = ctx.add_local(None, LocalKind::Temp, ty.clone());
    match ty {
        MIRType::Bool => ctx.push_inst(Instruction::Assign {
            destination,
            value: MirConstant::Bool(false),
        }),
        MIRType::Int(_) => ctx.push_inst(Instruction::Assign {
            destination,
            value: MirConstant::Int(0),
        }),
        MIRType::UInt(_) => ctx.push_inst(Instruction::Assign {
            destination,
            value: MirConstant::Uint(0),
        }),
        MIRType::Float(_) => ctx.push_inst(Instruction::Assign {
            destination,
            value: MirConstant::Float(0.0),
        }),
        MIRType::Unit | MIRType::Never => ctx.push_inst(Instruction::Assign {
            destination,
            value: MirConstant::Unit,
        }),
        MIRType::Struct { fields, .. } => {
            let fields = fields
                .iter()
                .map(|(_, field_ty)| default_value_for_type(ctx, field_ty))
                .collect();
            ctx.push_inst(Instruction::Aggregate {
                destination,
                fields,
                ty: ty.clone(),
            });
        }
        MIRType::Tuple(field_tys) => {
            let fields = field_tys
                .iter()
                .map(|field_ty| default_value_for_type(ctx, field_ty))
                .collect();
            ctx.push_inst(Instruction::Aggregate {
                destination,
                fields,
                ty: ty.clone(),
            });
        }
        MIRType::Array(elem_ty, len) => {
            let fields = (0..*len)
                .map(|_| default_value_for_type(ctx, elem_ty))
                .collect();
            ctx.push_inst(Instruction::Aggregate {
                destination,
                fields,
                ty: ty.clone(),
            });
        }
        MIRType::Enum { .. } => ctx.push_inst(Instruction::EnumConstruct {
            destination,
            discriminant: 0,
            payload: None,
            enum_type: ty.clone(),
        }),
        MIRType::Ptr(_) | MIRType::Ref(_) | MIRType::Fn { .. } | MIRType::Future(_) => {
            let zero = ctx.add_local(None, LocalKind::Temp, MIR_I64);
            ctx.push_inst(Instruction::Assign {
                destination: zero,
                value: MirConstant::Int(0),
            });
            ctx.push_inst(Instruction::Cast {
                destination,
                value: zero,
                to: ty.clone(),
            });
        }
    }
    destination
}

fn rebuild_failure_for_target(
    ctx: &mut LoweringContext<'_>,
    operand_local: Local,
    operand_ty: &MIRType,
    target_ty: &MIRType,
) -> Local {
    if operand_ty == target_ty {
        return operand_local;
    }

    if matches!(target_ty, MIRType::Enum { .. }) {
        let Some((_, failure)) = try_variant_discriminants(ctx, target_ty) else {
            return operand_local;
        };
        // `Err(e)` carries the operand's error across; `None` carries nothing.
        let payload = EnumDef::instance_variant_payload(target_ty, failure)
            .and_then(|_| failure_payload(ctx, operand_local, operand_ty));
        let destination = ctx.add_local(None, LocalKind::Temp, target_ty.clone());
        ctx.push_inst(Instruction::EnumConstruct {
            destination,
            discriminant: failure,
            payload,
            enum_type: target_ty.clone(),
        });
        if let Some(payload) = payload {
            ctx.mark_drop_local_moved(payload);
        }
        return destination;
    }

    let MIRType::Struct {
        name: target_name,
        fields: target_fields,
    } = target_ty
    else {
        return operand_local;
    };
    let value_ty = target_fields
        .iter()
        .find(|(name, _)| name == "value")
        .map(|(_, ty)| ty.clone())
        .unwrap_or(MIR_UNIT);
    let flag = ctx.lower_literal(&HIRLiteral::Bool(false));
    let value = default_value_for_type(ctx, &value_ty);
    let destination = ctx.add_local(None, LocalKind::Temp, target_ty.clone());

    if target_name == "Result" || target_name.starts_with("Result_") {
        let error = lower_field_expr(ctx, operand_local, "error");
        ctx.push_inst(Instruction::Aggregate {
            destination,
            fields: vec![flag, value, error],
            ty: target_ty.clone(),
        });
        return destination;
    }
    if target_name == "Option" || target_name.starts_with("Option_") {
        ctx.push_inst(Instruction::Aggregate {
            destination,
            fields: vec![flag, value],
            ty: target_ty.clone(),
        });
        return destination;
    }
    operand_local
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
    if matches!(container_ty, MIRType::Enum { .. }) {
        let Some((success, _)) = try_variant_discriminants(ctx, container_ty) else {
            return value_local;
        };
        let payload = EnumDef::instance_variant_payload(container_ty, success).map(|_| value_local);
        let destination = ctx.add_local(None, LocalKind::Temp, container_ty.clone());
        ctx.push_inst(Instruction::EnumConstruct {
            destination,
            discriminant: success,
            payload,
            enum_type: container_ty.clone(),
        });
        if let Some(payload) = payload {
            ctx.mark_drop_local_moved(payload);
        }
        return destination;
    }
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
    // Enum operands branch on the discriminant; the legacy struct form still
    // branches on its `is_ok`/`is_some` flag field.
    let success_discriminant = try_variant_discriminants(ctx, &operand_ty).map(|(ok, _)| ok);
    let flag_local = match success_discriminant {
        Some(success) => {
            let discr_local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
            ctx.push_inst(Instruction::Discriminant {
                destination: discr_local,
                source: operand_local,
            });
            let expected_local = ctx.lower_literal(&HIRLiteral::Int(i64::from(success)));
            let cond_local = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
            ctx.push_inst(Instruction::Binary {
                destination: cond_local,
                op: MirBinOp::Eq,
                left: discr_local,
                right: expected_local,
            });
            cond_local
        }
        None => {
            let flag_field = match &operand_ty {
                MIRType::Struct { name, .. } if name == "Option" || name.starts_with("Option_") => {
                    "is_some"
                }
                _ => "is_ok",
            };
            lower_field_expr(ctx, operand_local, flag_field)
        }
    };
    if success_discriminant.is_some() {
        // Both branches transfer the active enum payload: success returns it
        // to the surrounding expression, while failure rebuilds or forwards
        // the residual container. The original owner must not run Drop again.
        ctx.mark_drop_local_moved(operand_local);
    }
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
            let return_ty = ctx.mir_fn.return_type.clone();
            let failure = rebuild_failure_for_target(ctx, operand_local, &operand_ty, &return_ty);
            ctx.emit_active_drop_scopes_before_exit();
            ctx.set_terminator(Terminator::Return(Some(failure)));
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
    match success_discriminant {
        Some(success) => {
            let payload_ty =
                EnumDef::instance_variant_payload(&operand_ty, success).unwrap_or(MIR_UNIT);
            let payload_local = ctx.add_local(None, LocalKind::Temp, payload_ty);
            ctx.push_inst(Instruction::ExtractPayload {
                destination: payload_local,
                source: operand_local,
            });
            payload_local
        }
        None => lower_field_expr(ctx, operand_local, "value"),
    }
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
