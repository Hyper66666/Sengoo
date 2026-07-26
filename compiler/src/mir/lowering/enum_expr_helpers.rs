use super::*;
use crate::hir::HIRType;
use crate::mir::enum_defs::EnumDef;

/// Resolve the monomorphised MIR type for an enum constructor.
///
/// Generic enums such as `Option<T>` only have a layout once their arguments
/// are known, so the concrete type recorded by type checking is preferred and
/// the uninstantiated declaration is the fallback.
pub(super) fn enum_instance_type(
    ctx: &LoweringContext<'_>,
    enum_def: &EnumDef,
    concrete_type: Option<&HIRType>,
) -> MIRType {
    if let Some(concrete) = concrete_type {
        let mapped = ctx.hir_type_to_mir(concrete);
        if matches!(&mapped, MIRType::Enum { name, .. } if name == &enum_def.name || name.starts_with(&format!("{}_", enum_def.name)))
        {
            return mapped;
        }
    }
    enum_def.mir_type()
}

pub(super) fn lower_enum_construct_expr(
    ctx: &mut LoweringContext<'_>,
    enum_name: &str,
    variant_name: &str,
    discriminant: u32,
    args: &[HIRExpr],
    concrete_type: Option<&HIRType>,
) -> Local {
    let Some(enum_def) = ctx.options.enum_defs.get(enum_name).cloned() else {
        ctx.errors.push(format!(
            "undefined enum during MIR lowering: `{enum_name}` (constructing `{variant_name}`)"
        ));
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };

    let enum_type = enum_instance_type(ctx, &enum_def, concrete_type);
    let payload_ty = EnumDef::instance_variant_payload(&enum_type, discriminant);

    let lowered_args = args
        .iter()
        .map(|arg| ctx.lower_expr(arg))
        .collect::<Vec<_>>();
    let payload = match (payload_ty, lowered_args.as_slice()) {
        (None, _) | (_, []) => None,
        (Some(_), [only]) => Some(*only),
        (Some(ty), fields) => {
            let aggregate = ctx.add_local(None, LocalKind::Temp, ty.clone());
            ctx.push_inst(Instruction::Aggregate {
                destination: aggregate,
                fields: fields.to_vec(),
                ty,
            });
            Some(aggregate)
        }
    };

    let destination = ctx.add_local(None, LocalKind::Temp, enum_type.clone());
    ctx.push_inst(Instruction::EnumConstruct {
        destination,
        discriminant,
        payload,
        enum_type,
    });
    // The payload is now owned by the enum value; dropping it separately would
    // double-free.
    if let Some(payload) = payload {
        ctx.mark_drop_local_moved(payload);
    }
    destination
}
