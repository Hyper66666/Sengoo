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

pub(super) fn lower_tuple_expr(ctx: &mut LoweringContext<'_>, elems: &[HIRExpr]) -> Local {
    let elem_locals: Vec<Local> = elems.iter().map(|e| ctx.lower_expr(e)).collect();
    let elem_tys = elem_locals
        .iter()
        .map(|local| ctx.get_local_type(*local).clone())
        .collect::<Vec<_>>();
    let tuple_ty = MIRType::Tuple(elem_tys);

    let tuple_local = ctx.add_local(None, LocalKind::Temp, tuple_ty.clone());
    ctx.push_inst(Instruction::Aggregate {
        destination: tuple_local,
        fields: elem_locals,
        ty: tuple_ty,
    });

    tuple_local
}

pub(super) fn lower_index_expr(
    ctx: &mut LoweringContext<'_>,
    base: &HIRExpr,
    index: &HIRExpr,
) -> Local {
    let base_local = ctx.lower_expr(base);
    let base_ty = ctx.get_local_type(base_local).clone();

    if let HIRExpr::Range {
        start: Some(start),
        end: Some(end),
        inclusive: false,
    } = index
    {
        if is_owned_string_mir_type(&base_ty) || is_str_ptr_mir_type(&base_ty) {
            return lower_string_range_index_expr(ctx, base_local, &base_ty, start, end);
        }
    }

    let index_local = ctx.lower_expr(index);
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

fn is_owned_string_mir_type(ty: &MIRType) -> bool {
    matches!(ty, MIRType::Struct { name, .. } if name == "String")
}

fn is_str_ptr_mir_type(ty: &MIRType) -> bool {
    matches!(ty, MIRType::Ptr(inner) if matches!(inner.as_ref(), MIRType::Int(8)))
}

fn owned_string_mir_type() -> MIRType {
    MIRType::Struct {
        name: "String".to_string(),
        fields: vec![("handle".to_string(), MIR_I64)],
    }
}

fn lower_string_range_index_expr(
    ctx: &mut LoweringContext<'_>,
    base_local: Local,
    base_ty: &MIRType,
    start: &HIRExpr,
    end: &HIRExpr,
) -> Local {
    let start_local = ctx.lower_expr(start);
    let end_local = ctx.lower_expr(end);
    let raw_handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);

    if is_owned_string_mir_type(base_ty) {
        let handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        ctx.push_inst(Instruction::Extract {
            destination: handle,
            value: base_local,
            index: 0,
        });
        ctx.push_inst(Instruction::Call {
            destination: raw_handle,
            func: "sengoo_string_slice_status".to_string(),
            args: vec![handle, start_local, end_local],
        });
    } else {
        let value_ptr = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        ctx.push_inst(Instruction::Call {
            destination: value_ptr,
            func: "sengoo_stdlib_str_ptr".to_string(),
            args: vec![base_local],
        });
        ctx.push_inst(Instruction::Call {
            destination: raw_handle,
            func: "sengoo_str_slice_copy".to_string(),
            args: vec![value_ptr, start_local, end_local],
        });
    }

    wrap_string_slice_status_or_panic(ctx, raw_handle)
}

fn wrap_string_slice_status_or_panic(ctx: &mut LoweringContext<'_>, raw_handle: Local) -> Local {
    let zero = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: zero,
        value: MirConstant::Int(0),
    });
    let ok = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: ok,
        op: MirBinOp::Ge,
        left: raw_handle,
        right: zero,
    });

    let ok_block = ctx.new_block();
    let err_block = ctx.new_block();
    let join_block = ctx.new_block();
    ctx.set_terminator(Terminator::If {
        cond: ok,
        then_block: ok_block,
        else_block: err_block,
    });

    ctx.set_current_block(ok_block);
    ctx.set_terminator(Terminator::Goto(join_block));
    let ok_end = ctx.current_block();

    ctx.set_current_block(err_block);
    let panic_handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Call {
        destination: panic_handle,
        func: "sengoo_panic_result_unwrap_i64".to_string(),
        args: vec![],
    });
    ctx.set_terminator(Terminator::Goto(join_block));
    let err_end = ctx.current_block();

    ctx.set_current_block(join_block);
    let handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    let incoming = vec![(raw_handle, ok_end), (panic_handle, err_end)];
    ctx.push_inst(Instruction::Phi {
        destination: handle,
        incoming,
    });
    let result = ctx.add_local(None, LocalKind::Temp, owned_string_mir_type());
    ctx.push_inst(Instruction::Aggregate {
        destination: result,
        fields: vec![handle],
        ty: owned_string_mir_type(),
    });
    ctx.record_drop_binding_if_needed(result);
    result
}

pub(super) fn lower_field_expr(
    ctx: &mut LoweringContext<'_>,
    base_local: Local,
    field: &str,
) -> Local {
    let base_ty = ctx.get_local_type(base_local).clone();
    let (base_local, base_ty) = match base_ty {
        MIRType::Ptr(inner) | MIRType::Ref(inner) => {
            let loaded = ctx.add_local(None, LocalKind::Temp, (*inner).clone());
            ctx.push_inst(Instruction::Load {
                destination: loaded,
                source: base_local,
            });
            (loaded, (*inner).clone())
        }
        ty => (base_local, ty),
    };
    let field_index = match &base_ty {
        MIRType::Struct { fields, .. } => fields
            .iter()
            .position(|(name, _)| name == field)
            .unwrap_or(0),
        _ => tuple_field_index(field).unwrap_or(0),
    };
    let elem_ty = match base_ty {
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

fn tuple_field_index(field: &str) -> Option<usize> {
    field.parse::<usize>().ok().or(match field {
        "x" | "left" | "r" => Some(0),
        "y" | "right" | "g" => Some(1),
        "z" | "b" => Some(2),
        "w" | "a" => Some(3),
        _ => None,
    })
}

pub(super) fn lower_struct_expr(
    ctx: &mut LoweringContext<'_>,
    name: &str,
    fields: &[(String, HIRExpr)],
    concrete_type: Option<&HIRType>,
) -> Local {
    let concrete_struct_ty = concrete_type.map(|concrete| {
        ctx.concrete_type_registry.register_instance(
            crate::type_naming::hir_type_instance_name(concrete),
            concrete.clone(),
        );
        crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_enums(
            concrete,
            ctx.struct_defs,
            &ctx.options.enum_defs,
            &HashMap::new(),
        )
    });
    let lowered_fields: Vec<(String, Local)> = fields
        .iter()
        .map(|(field_name, expr)| {
            let expected = match &concrete_struct_ty {
                Some(MIRType::Struct { fields, .. }) => fields
                    .iter()
                    .find(|(name, _)| name == field_name)
                    .map(|(_, ty)| ty.clone()),
                _ => None,
            };
            let local = match (expr, expected) {
                (HIRExpr::Var { name, .. }, Some(fn_ty @ MIRType::Fn { .. }))
                    if ctx.is_known_function(name) =>
                {
                    let local = ctx.add_local(None, LocalKind::Temp, fn_ty);
                    ctx.push_inst(Instruction::Assign {
                        destination: local,
                        value: MirConstant::GlobalRef(name.clone()),
                    });
                    local
                }
                _ => ctx.lower_expr(expr),
            };
            (field_name.clone(), local)
        })
        .collect();
    let field_locals_by_name: HashMap<String, Local> = lowered_fields
        .iter()
        .map(|(field_name, local)| (field_name.clone(), *local))
        .collect();

    let struct_ty = concrete_struct_ty
        .or_else(|| ctx.infer_struct_literal_type(name, &field_locals_by_name))
        .unwrap_or_else(|| MIRType::Struct {
            name: name.to_string(),
            fields: lowered_fields
                .iter()
                .map(|(field_name, local)| (field_name.clone(), ctx.get_local_type(*local).clone()))
                .collect(),
        });

    let ordered_field_locals: Vec<Local> = match &struct_ty {
        MIRType::Struct { fields, .. } => fields
            .iter()
            .filter_map(|(field_name, _)| field_locals_by_name.get(field_name).copied())
            .collect(),
        _ => lowered_fields.iter().map(|(_, local)| *local).collect(),
    };

    let struct_type_name = match &struct_ty {
        MIRType::Struct { name, .. } => Some(name.clone()),
        _ => None,
    };
    let struct_local = ctx.add_local(None, LocalKind::Temp, struct_ty.clone());
    ctx.push_inst(Instruction::Aggregate {
        destination: struct_local,
        fields: ordered_field_locals,
        ty: struct_ty,
    });

    if let Some(name) = struct_type_name {
        ctx.type_names.insert(struct_local, name);
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
    fn lower_array_expr_emits_aggregate_with_element_locals() {
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

        let elems = vec![
            HIRExpr::Lit(HIRLiteral::Int(1)),
            HIRExpr::Lit(HIRLiteral::Int(2)),
        ];
        let result = lower_array_expr(&mut ctx, &elems);

        assert!(matches!(ctx.get_local_type(result), MIRType::Array(_, 2)));
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(inst, Instruction::Aggregate { destination, fields, .. } if *destination == result && fields.len() == 2)));
    }

    #[test]
    fn lower_tuple_expr_emits_tuple_aggregate_with_element_types() {
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

        let tuple = HIRExpr::Tuple(vec![
            HIRExpr::Lit(HIRLiteral::Int(7)),
            HIRExpr::Lit(HIRLiteral::Bool(true)),
        ]);
        let result = ctx.lower_expr(&tuple);

        assert_eq!(
            ctx.get_local_type(result),
            &MIRType::Tuple(vec![MIR_I64, MIR_BOOL])
        );
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Aggregate { destination, fields, ty }
                if *destination == result
                    && fields.len() == 2
                    && matches!(ty, MIRType::Tuple(items) if items.len() == 2)
        )));
    }

    #[test]
    fn lower_index_expr_emits_index_addr_and_load() {
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

        let arr = ctx.add_local(None, LocalKind::User, MIRType::Array(Box::new(MIR_I64), 4));
        ctx.local_names.insert("arr".to_string(), arr);
        ctx.bind_local_symbol(SymbolId::new(1), arr);

        let base = HIRExpr::Var {
            name: "arr".to_string(),
            symbol: SymbolId::new(1),
        };
        let index = HIRExpr::Lit(HIRLiteral::Int(0));
        let result = lower_index_expr(&mut ctx, &base, &index);

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx
            .mir_fn
            .instructions
            .iter()
            .any(|inst| matches!(inst, Instruction::IndexAddr { .. })));
        assert!(ctx.mir_fn.instructions.iter().any(
            |inst| matches!(inst, Instruction::Load { destination, .. } if *destination == result)
        ));
    }

    #[test]
    fn lower_struct_expr_records_struct_type_name_on_result() {
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

        let fields = vec![
            ("right".to_string(), HIRExpr::Lit(HIRLiteral::Bool(true))),
            ("left".to_string(), HIRExpr::Lit(HIRLiteral::Int(7))),
        ];
        let result = lower_struct_expr(&mut ctx, "Pair", &fields, None);

        assert_eq!(
            ctx.type_names.get(&result).map(String::as_str),
            Some("Pair")
        );
        assert!(
            matches!(ctx.get_local_type(result), MIRType::Struct { name, .. } if name == "Pair")
        );
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(inst, Instruction::Aggregate { destination, fields, .. } if *destination == result && fields.len() == 2)));
    }
    #[test]
    fn lower_field_expr_extracts_struct_field_by_name() {
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
