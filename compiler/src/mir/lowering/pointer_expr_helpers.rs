use super::*;

pub(super) fn lower_ref_expr(ctx: &mut LoweringContext<'_>, expr: &HIRExpr) -> Local {
    if let Some(field_ref) = try_lower_addressable_field_ref(ctx, expr) {
        return field_ref;
    }

    let expr_local = ctx.lower_expr(expr);
    let expr_ty = ctx.get_local_type(expr_local).clone();

    let ptr_ty = MIRType::Ptr(Box::new(expr_ty));
    let ptr_local = ctx.add_local(None, LocalKind::Temp, ptr_ty);

    let zero_index = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: zero_index,
        value: MirConstant::Int(0),
    });

    ctx.push_inst(Instruction::IndexAddr {
        destination: ptr_local,
        base: expr_local,
        index: zero_index,
    });

    ptr_local
}

pub(super) fn try_lower_addressable_field_ref(
    ctx: &mut LoweringContext<'_>,
    expr: &HIRExpr,
) -> Option<Local> {
    if let HIRExpr::Field { base, field } = expr {
        let base_local = ctx.lower_expr(base);
        let base_ty = ctx.get_local_type(base_local).clone();
        if base_local.kind == LocalKind::User {
            let field_info = match &base_ty {
                MIRType::Struct { fields, .. } => fields
                    .iter()
                    .position(|(name, _)| name == field)
                    .map(|index| (index, fields[index].1.clone())),
                MIRType::Tuple(items) => field
                    .parse::<usize>()
                    .ok()
                    .filter(|index| *index < items.len())
                    .map(|index| (index, items[index].clone())),
                _ => None,
            };
            if let Some((field_index, field_ty)) = field_info {
                let ptr_local =
                    ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(field_ty)));
                ctx.push_inst(Instruction::FieldAddr {
                    destination: ptr_local,
                    base: base_local,
                    field: field_index as u32,
                });
                return Some(ptr_local);
            }
        }
    }
    None
}

pub(super) fn lower_deref_expr(ctx: &mut LoweringContext<'_>, expr: &HIRExpr) -> Local {
    let ptr_local = ctx.lower_expr(expr);
    let ptr_ty = ctx.get_local_type(ptr_local).clone();

    let elem_ty = match ptr_ty {
        MIRType::Ptr(inner) | MIRType::Ref(inner) => *inner,
        _ => MIR_UNIT,
    };

    let result_local = ctx.add_local(None, LocalKind::Temp, elem_ty);
    ctx.push_inst(Instruction::Load {
        destination: result_local,
        source: ptr_local,
    });

    result_local
}

#[cfg(test)]
mod tests {
    use super::*;

    type TestCtxParts = (
        MirFunction,
        usize,
        HashSet<String>,
        HashMap<String, FunctionSig>,
        HashMap<String, &'static hir::HIRStruct>,
        Vec<InherentMethodTemplate>,
        Vec<TraitMethodTemplate>,
    );

    fn make_ctx() -> TestCtxParts {
        (
            MirFunction::new("test".to_string(), vec![], MIR_UNIT),
            0usize,
            HashSet::new(),
            HashMap::new(),
            HashMap::new(),
            Vec::new(),
            Vec::new(),
        )
    }

    #[test]
    fn lower_ref_expr_emits_index_addr() {
        let (
            mut mir_fn,
            mut lambda_counter,
            known_functions,
            function_sigs,
            struct_defs,
            inherent_templates,
            trait_templates,
        ) = make_ctx();
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

        let x = ctx.add_local(Some("x".to_string()), LocalKind::User, MIR_I64);
        ctx.local_names.insert("x".to_string(), x);
        ctx.bind_local_symbol(SymbolId::new(1), x);

        let expr = HIRExpr::Var {
            name: "x".to_string(),
            symbol: SymbolId::new(1),
        };
        let result = lower_ref_expr(&mut ctx, &expr);

        assert!(matches!(ctx.get_local_type(result), MIRType::Ptr(_)));
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(inst, Instruction::IndexAddr { destination, base, .. } if *destination == result && *base == x)));
    }

    #[test]
    fn lower_ref_expr_uses_field_addr_for_addressable_struct_field() {
        let (
            mut mir_fn,
            mut lambda_counter,
            known_functions,
            function_sigs,
            struct_defs,
            inherent_templates,
            trait_templates,
        ) = make_ctx();
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

        let value = ctx.add_local(
            Some("value".to_string()),
            LocalKind::User,
            MIRType::Struct {
                name: "Pair".to_string(),
                fields: vec![
                    ("first".to_string(), MIR_I64),
                    ("flag".to_string(), MIR_BOOL),
                ],
            },
        );
        ctx.local_names.insert("value".to_string(), value);
        ctx.bind_local_symbol(SymbolId::new(3), value);
        let expr = HIRExpr::Field {
            base: Box::new(HIRExpr::Var {
                name: "value".to_string(),
                symbol: SymbolId::new(3),
            }),
            field: "flag".to_string(),
        };

        let result = lower_ref_expr(&mut ctx, &expr);

        assert_eq!(
            ctx.get_local_type(result),
            &MIRType::Ptr(Box::new(MIR_BOOL))
        );
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::FieldAddr { destination, base, field }
                if *destination == result && *base == value && *field == 1
        )));
    }

    #[test]
    fn lower_deref_expr_emits_load_from_pointer() {
        let (
            mut mir_fn,
            mut lambda_counter,
            known_functions,
            function_sigs,
            struct_defs,
            inherent_templates,
            trait_templates,
        ) = make_ctx();
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

        let ptr = ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(MIR_I64)));
        ctx.local_names.insert("p".to_string(), ptr);
        ctx.bind_local_symbol(SymbolId::new(2), ptr);

        let expr = HIRExpr::Var {
            name: "p".to_string(),
            symbol: SymbolId::new(2),
        };
        let result = lower_deref_expr(&mut ctx, &expr);

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(inst, Instruction::Load { destination, source } if *destination == result && *source == ptr)));
    }
}
