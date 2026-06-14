use super::*;

pub(super) fn lower_return_expr(ctx: &mut LoweringContext<'_>, value: Option<&HIRExpr>) -> Local {
    let return_value = value.map(|expr| ctx.lower_expr(expr));
    if let Some(local) = return_value {
        ctx.mark_drop_local_moved(local);
    }
    ctx.set_terminator(Terminator::Return(return_value));
    ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
}
