use super::*;

pub(super) fn lower_enum_construct_expr(
    ctx: &mut LoweringContext<'_>,
    enum_name: &str,
    variant_name: &str,
    discriminant: u32,
    args: &[HIRExpr],
) -> Local {
    let Some(enum_def) = ctx.options.enum_defs.get(enum_name).cloned() else {
        ctx.errors
            .push(format!("undefined enum during MIR lowering: `{enum_name}`"));
        return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
    };
    let payload_ty = enum_def
        .variants
        .iter()
        .find(|(_, name, _)| name == variant_name)
        .and_then(|(_, _, payload)| payload.clone());

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

    let enum_type = enum_def.mir_type();
    let destination = ctx.add_local(None, LocalKind::Temp, enum_type.clone());
    ctx.push_inst(Instruction::EnumConstruct {
        destination,
        discriminant,
        payload,
        enum_type,
    });
    destination
}
