use super::*;

pub(super) fn lower_array_expr(ctx: &mut LoweringContext<'_>, elems: &[HIRExpr]) -> Local {
    let elem_locals: Vec<Local> = elems.iter().map(|e| ctx.lower_expr(e)).collect();

    let elem_ty = if let Some(first_local) = elem_locals.first() {
        ctx.get_local_type(*first_local).clone()
    } else {
        MIR_UNIT
    };
    let array_ty = MIRType::Array(Box::new(elem_ty), elems.len() as u64);

    let array_local = ctx.add_local(None, LocalKind::User, array_ty.clone());
    ctx.push_inst(Instruction::Aggregate {
        destination: array_local,
        fields: elem_locals,
        ty: array_ty,
    });

    array_local
}

pub(super) fn lower_index_expr(
    ctx: &mut LoweringContext<'_>,
    base: &HIRExpr,
    index: &HIRExpr,
) -> Local {
    let base_local = ctx.lower_expr(base);
    let index_local = ctx.lower_expr(index);

    let base_ty = ctx.get_local_type(base_local).clone();
    let elem_ty = match base_ty {
        MIRType::Array(elem, _) => *elem,
        _ => MIR_UNIT,
    };

    let addr_local = ctx.add_local(
        None,
        LocalKind::Temp,
        MIRType::Ptr(Box::new(elem_ty.clone())),
    );
    ctx.push_inst(Instruction::IndexAddr {
        destination: addr_local,
        base: base_local,
        index: index_local,
    });

    let result_local = ctx.add_local(None, LocalKind::Temp, elem_ty);
    ctx.push_inst(Instruction::Load {
        destination: result_local,
        source: addr_local,
    });

    result_local
}

pub(super) fn lower_field_expr(
    ctx: &mut LoweringContext<'_>,
    base_local: Local,
    field: &str,
) -> Local {
    let base_ty = ctx.get_local_type(base_local).clone();
    let field_index = match &base_ty {
        MIRType::Struct { fields, .. } => fields
            .iter()
            .position(|(name, _)| name == field)
            .unwrap_or(0),
        _ => match field {
            "x" | "left" | "r" => 0,
            "y" | "right" | "g" => 1,
            "z" | "b" => 2,
            "w" | "a" => 3,
            _ => 0,
        },
    };
    let elem_ty = match &base_ty {
        MIRType::Tuple(tys) if field_index < tys.len() => tys[field_index].clone(),
        MIRType::Struct { fields, .. } if field_index < fields.len() => {
            fields[field_index].1.clone()
        }
        _ => MIR_I64,
    };

    let result_local = ctx.add_local(None, LocalKind::Temp, elem_ty);
    ctx.push_inst(Instruction::Extract {
        destination: result_local,
        value: base_local,
        index: field_index as u32,
    });

    result_local
}
pub(super) fn lower_struct_expr(
    ctx: &mut LoweringContext<'_>,
    name: &str,
    fields: &[(String, HIRExpr)],
) -> Local {
    let lowered_fields: Vec<(String, Local)> = fields
        .iter()
        .map(|(field_name, expr)| (field_name.clone(), ctx.lower_expr(expr)))
        .collect();
    let field_locals_by_name: HashMap<String, Local> = lowered_fields
        .iter()
        .map(|(field_name, local)| (field_name.clone(), *local))
        .collect();

    let struct_ty = ctx
        .infer_struct_literal_type(name, &field_locals_by_name)
        .unwrap_or_else(|| MIRType::Struct {
            name: name.to_string(),
            fields: lowered_fields
                .iter()
                .map(|(field_name, local)| {
                    (field_name.clone(), ctx.get_local_type(*local).clone())
                })
                .collect(),
        });

    let ordered_field_locals: Vec<Local> = match &struct_ty {
        MIRType::Struct { fields, .. } => fields
            .iter()
            .filter_map(|(field_name, _)| field_locals_by_name.get(field_name).copied())
            .collect(),
        _ => lowered_fields.iter().map(|(_, local)| *local).collect(),
    };

    let struct_local = ctx.add_local(None, LocalKind::Temp, struct_ty.clone());
    ctx.push_inst(Instruction::Aggregate {
        destination: struct_local,
        fields: ordered_field_locals,
        ty: struct_ty.clone(),
    });

    if let MIRType::Struct { name, .. } = &struct_ty {
        ctx.type_names.insert(struct_local, name.clone());
    }

    struct_local
}


#[cfg(test)]
mod tests {
    use super::*;

    type TestCtxParts = (
        MirFunction,
        usize,
        HashSet<String>,
        HashMap<String, FunctionSig>,
        HashMap<String, & 'static hir::HIRStruct>,
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
    fn lower_array_expr_emits_aggregate_with_element_locals() {
        let (mut mir_fn, mut lambda_counter, known_functions, function_sigs, struct_defs, inherent_templates, trait_templates) = make_ctx();
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

        let elems = vec![HIRExpr::Lit(HIRLiteral::Int(1)), HIRExpr::Lit(HIRLiteral::Int(2))];
        let result = lower_array_expr(&mut ctx, &elems);

        assert!(matches!(ctx.get_local_type(result), MIRType::Array(_, 2)));
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(inst, Instruction::Aggregate { destination, fields, .. } if *destination == result && fields.len() == 2)));
    }

    #[test]
    fn lower_index_expr_emits_index_addr_and_load() {
        let (mut mir_fn, mut lambda_counter, known_functions, function_sigs, struct_defs, inherent_templates, trait_templates) = make_ctx();
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

        let arr = ctx.add_local(None, LocalKind::User, MIRType::Array(Box::new(MIR_I64), 4));
        ctx.local_names.insert("arr".to_string(), arr);
        ctx.bind_local_symbol(SymbolId::new(1), arr);

        let base = HIRExpr::Var { name: "arr".to_string(), symbol: SymbolId::new(1) };
        let index = HIRExpr::Lit(HIRLiteral::Int(0));
        let result = lower_index_expr(&mut ctx, &base, &index);

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(inst, Instruction::IndexAddr { .. })));
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(inst, Instruction::Load { destination, .. } if *destination == result)));
    }

    #[test]
    fn lower_struct_expr_records_struct_type_name_on_result() {
        let (mut mir_fn, mut lambda_counter, known_functions, function_sigs, struct_defs, inherent_templates, trait_templates) = make_ctx();
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

        let fields = vec![
            ("right".to_string(), HIRExpr::Lit(HIRLiteral::Bool(true))),
            ("left".to_string(), HIRExpr::Lit(HIRLiteral::Int(7))),
        ];
        let result = lower_struct_expr(&mut ctx, "Pair", &fields);

        assert_eq!(ctx.type_names.get(&result).map(String::as_str), Some("Pair"));
        assert!(matches!(ctx.get_local_type(result), MIRType::Struct { name, .. } if name == "Pair"));
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(inst, Instruction::Aggregate { destination, fields, .. } if *destination == result && fields.len() == 2)));
    }
    #[test]
    fn lower_field_expr_extracts_struct_field_by_name() {
        let (mut mir_fn, mut lambda_counter, known_functions, function_sigs, struct_defs, inherent_templates, trait_templates) = make_ctx();
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

        let pair = ctx.add_local(
            None,
            LocalKind::User,
            MIRType::Struct {
                name: "Pair".to_string(),
                fields: vec![
                    ("left".to_string(), MIR_I64),
                    ("right".to_string(), MIR_BOOL),
                ],
            },
        );
        let result = lower_field_expr(&mut ctx, pair, "right");

        assert_eq!(ctx.get_local_type(result), &MIR_BOOL);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(inst, Instruction::Extract { destination, index, .. } if *destination == result && *index == 1)));
    }
}