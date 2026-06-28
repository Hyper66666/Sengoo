use super::*;

fn is_string_ptr(ty: &MIRType) -> bool {
    matches!(ty, MIRType::Ptr(inner) if matches!(inner.as_ref(), MIRType::Int(8)))
}

fn is_owned_string(ty: &MIRType) -> bool {
    matches!(ty, MIRType::Struct { name, .. } if name == "String")
}

fn is_async_context_type(ty: &MIRType) -> bool {
    matches!(ty, MIRType::Struct { name, .. } if name == "AsyncContext")
}

fn owned_string_mir_type() -> MIRType {
    MIRType::Struct {
        name: "String".to_string(),
        fields: vec![("handle".to_string(), MIR_I64)],
    }
}

fn extract_owned_string_handle(ctx: &mut LoweringContext<'_>, value: Local) -> Local {
    let handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Extract {
        destination: handle,
        value,
        index: 0,
    });
    handle
}

fn lower_i64_status_to_bool(
    ctx: &mut LoweringContext<'_>,
    value: Local,
    op: MirBinOp,
    zero_value: i64,
) -> Local {
    let zero = ctx.add_local(None, LocalKind::Temp, MIR_I64);
    ctx.push_inst(Instruction::Assign {
        destination: zero,
        value: MirConstant::Int(zero_value),
    });

    let bool_result = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: bool_result,
        op,
        left: value,
        right: zero,
    });
    bool_result
}

pub(super) fn lower_unary_expr(
    ctx: &mut LoweringContext<'_>,
    op: &hir::HIRUnaryOp,
    operand: &HIRExpr,
) -> Local {
    match op {
        hir::HIRUnaryOp::Ref | hir::HIRUnaryOp::RefMut => {
            let expr_local = ctx.lower_expr(operand);
            let expr_ty = ctx.get_local_type(expr_local).clone();

            let ptr_ty = MIRType::Ptr(Box::new(expr_ty));
            let ptr_local = ctx.add_local(None, LocalKind::Temp, ptr_ty);
            ctx.push_inst(Instruction::AddrOf {
                destination: ptr_local,
                source: expr_local,
            });

            ptr_local
        }
        hir::HIRUnaryOp::Deref => {
            let ptr_local = ctx.lower_expr(operand);
            let ptr_ty = ctx.get_local_type(ptr_local).clone();

            let elem_ty = match ptr_ty {
                MIRType::Ptr(inner) | MIRType::Ref(inner) => (*inner).clone(),
                _ => MIR_I64,
            };

            let result_local = ctx.add_local(None, LocalKind::Temp, elem_ty);
            ctx.push_inst(Instruction::Load {
                destination: result_local,
                source: ptr_local,
            });

            result_local
        }
        _ => {
            let operand_local = ctx.lower_expr(operand);
            let mir_op = ctx.lower_un_op(op);
            let result_ty = match op {
                hir::HIRUnaryOp::Not => MIR_BOOL,
                _ => MIR_I64,
            };
            let local = ctx.add_local(None, LocalKind::Temp, result_ty);
            ctx.push_inst(Instruction::Unary {
                destination: local,
                op: mir_op,
                operand: operand_local,
            });
            local
        }
    }
}

pub(super) fn lower_binary_expr(
    ctx: &mut LoweringContext<'_>,
    op: &hir::HIRBinaryOp,
    left: &HIRExpr,
    right: &HIRExpr,
) -> Local {
    let left_local = ctx.lower_expr(left);
    let right_local = ctx.lower_expr(right);
    let mir_op = ctx.lower_bin_op(op);

    if mir_op == MirBinOp::Add {
        let (is_string_concat, is_owned_string_concat) = {
            let left_ty = ctx.get_local_type(left_local);
            let right_ty = ctx.get_local_type(right_local);
            (
                is_string_ptr(left_ty) && is_string_ptr(right_ty),
                is_owned_string(left_ty) && is_string_ptr(right_ty),
            )
        };
        if is_string_concat {
            let result_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
            let result_local = ctx.add_local(None, LocalKind::Temp, result_ty);
            ctx.push_inst(Instruction::Call {
                destination: result_local,
                func: "sengoo_str_concat".to_string(),
                args: vec![left_local, right_local],
            });
            return result_local;
        }
        if is_owned_string_concat {
            let left_handle = extract_owned_string_handle(ctx, left_local);
            let right_ptr = ctx.add_local(None, LocalKind::Temp, MIR_I64);
            ctx.push_inst(Instruction::Call {
                destination: right_ptr,
                func: "sengoo_stdlib_str_ptr".to_string(),
                args: vec![right_local],
            });
            let result_handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
            ctx.push_inst(Instruction::Call {
                destination: result_handle,
                func: "sengoo_string_concat_str_status".to_string(),
                args: vec![left_handle, right_ptr],
            });
            let result = ctx.add_local(None, LocalKind::Temp, owned_string_mir_type());
            ctx.push_inst(Instruction::Aggregate {
                destination: result,
                fields: vec![result_handle],
                ty: owned_string_mir_type(),
            });
            ctx.record_drop_binding_if_needed(result);
            return result;
        }
    }

    if mir_op == MirBinOp::Eq || mir_op == MirBinOp::Ne {
        let is_string_cmp = {
            let left_ty = ctx.get_local_type(left_local);
            let right_ty = ctx.get_local_type(right_local);
            is_string_ptr(left_ty) && is_string_ptr(right_ty)
        };
        if is_string_cmp {
            let call_result = ctx.add_local(None, LocalKind::Temp, MIR_I64);
            ctx.push_inst(Instruction::Call {
                destination: call_result,
                func: "sengoo_str_eq".to_string(),
                args: vec![left_local, right_local],
            });

            let zero = ctx.add_local(None, LocalKind::Temp, MIR_I64);
            ctx.push_inst(Instruction::Assign {
                destination: zero,
                value: MirConstant::Int(0),
            });

            let cmp_op = if mir_op == MirBinOp::Eq {
                MirBinOp::Ne
            } else {
                MirBinOp::Eq
            };
            let bool_result = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
            ctx.push_inst(Instruction::Binary {
                destination: bool_result,
                op: cmp_op,
                left: call_result,
                right: zero,
            });

            return bool_result;
        }
    }

    if mir_op.is_comparison() {
        let is_owned_string_cmp = {
            let left_ty = ctx.get_local_type(left_local);
            let right_ty = ctx.get_local_type(right_local);
            is_owned_string(left_ty) && is_owned_string(right_ty)
        };
        if is_owned_string_cmp {
            let left_handle = extract_owned_string_handle(ctx, left_local);
            let right_handle = extract_owned_string_handle(ctx, right_local);
            if mir_op == MirBinOp::Eq || mir_op == MirBinOp::Ne {
                let call_result = ctx.add_local(None, LocalKind::Temp, MIR_I64);
                ctx.push_inst(Instruction::Call {
                    destination: call_result,
                    func: "sengoo_string_eq".to_string(),
                    args: vec![left_handle, right_handle],
                });
                let cmp_op = if mir_op == MirBinOp::Eq {
                    MirBinOp::Ne
                } else {
                    MirBinOp::Eq
                };
                return lower_i64_status_to_bool(ctx, call_result, cmp_op, 0);
            }

            let compare_result = ctx.add_local(None, LocalKind::Temp, MIR_I64);
            ctx.push_inst(Instruction::Call {
                destination: compare_result,
                func: "sengoo_string_compare".to_string(),
                args: vec![left_handle, right_handle],
            });
            return lower_i64_status_to_bool(ctx, compare_result, mir_op, 0);
        }
    }

    let (left_local, right_local) = ctx.reconcile_binary_operand_types(left_local, right_local);
    let operand_ty = ctx.get_local_type(left_local).clone();
    if mir_op.is_comparison()
        && (is_async_context_type(ctx.get_local_type(left_local))
            || is_async_context_type(ctx.get_local_type(right_local)))
    {
        ctx.errors
            .push("AsyncContext is poll-scoped and cannot be compared".to_string());
        return ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    }
    let result_ty = match mir_op {
        MirBinOp::Eq
        | MirBinOp::Ne
        | MirBinOp::Lt
        | MirBinOp::Le
        | MirBinOp::Gt
        | MirBinOp::Ge
        | MirBinOp::LogAnd
        | MirBinOp::LogOr => MIR_BOOL,
        _ => operand_ty,
    };
    let local = ctx.add_local(None, LocalKind::Temp, result_ty);
    ctx.push_inst(Instruction::Binary {
        destination: local,
        op: mir_op,
        left: left_local,
        right: right_local,
    });
    local
}

pub(super) fn lower_logical_and_expr(
    ctx: &mut LoweringContext<'_>,
    left: &HIRExpr,
    right: &HIRExpr,
) -> Local {
    let left_local = ctx.lower_expr(left);
    let right_local = ctx.lower_expr(right);
    let local = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: local,
        op: MirBinOp::LogAnd,
        left: left_local,
        right: right_local,
    });
    local
}

pub(super) fn lower_logical_or_expr(
    ctx: &mut LoweringContext<'_>,
    left: &HIRExpr,
    right: &HIRExpr,
) -> Local {
    let left_local = ctx.lower_expr(left);
    let right_local = ctx.lower_expr(right);
    let local = ctx.add_local(None, LocalKind::Temp, MIR_BOOL);
    ctx.push_inst(Instruction::Binary {
        destination: local,
        op: MirBinOp::LogOr,
        left: left_local,
        right: right_local,
    });
    local
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
    fn lower_unary_expr_emits_addrof_for_ref() {
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

        let result = lower_unary_expr(
            &mut ctx,
            &hir::HIRUnaryOp::Ref,
            &HIRExpr::Var {
                name: "x".to_string(),
                symbol: SymbolId::new(1),
            },
        );

        assert!(matches!(ctx.get_local_type(result), MIRType::Ptr(_)));
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::AddrOf { destination, source } if *destination == result && *source == x
        )));
    }

    #[test]
    fn lower_unary_expr_emits_unary_instruction_for_neg() {
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

        let result = lower_unary_expr(
            &mut ctx,
            &hir::HIRUnaryOp::Neg,
            &HIRExpr::Lit(HIRLiteral::Int(7)),
        );

        assert_eq!(ctx.get_local_type(result), &MIR_I64);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Unary {
                destination,
                op: MirUnOp::Neg,
                ..
            } if *destination == result
        )));
    }

    #[test]
    fn lower_unary_expr_preserves_bool_type_for_not() {
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

        let result = lower_unary_expr(
            &mut ctx,
            &hir::HIRUnaryOp::Not,
            &HIRExpr::Lit(HIRLiteral::Bool(true)),
        );

        assert_eq!(ctx.get_local_type(result), &MIR_BOOL);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Unary {
                destination,
                op: MirUnOp::Not,
                ..
            } if *destination == result
        )));
    }

    #[test]
    fn lower_binary_expr_emits_string_concat_call() {
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

        let str_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
        let lhs = ctx.add_local(Some("lhs".to_string()), LocalKind::User, str_ty.clone());
        let rhs = ctx.add_local(Some("rhs".to_string()), LocalKind::User, str_ty);
        ctx.local_names.insert("lhs".to_string(), lhs);
        ctx.local_names.insert("rhs".to_string(), rhs);
        ctx.bind_local_symbol(SymbolId::new(2), lhs);
        ctx.bind_local_symbol(SymbolId::new(3), rhs);

        let result = lower_binary_expr(
            &mut ctx,
            &hir::HIRBinaryOp::Add,
            &HIRExpr::Var {
                name: "lhs".to_string(),
                symbol: SymbolId::new(2),
            },
            &HIRExpr::Var {
                name: "rhs".to_string(),
                symbol: SymbolId::new(3),
            },
        );

        assert!(matches!(ctx.get_local_type(result), MIRType::Ptr(_)));
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { destination, func, args } if *destination == result && func == "sengoo_str_concat" && args == &vec![lhs, rhs]
        )));
    }

    #[test]
    fn lower_binary_expr_emits_string_eq_and_bool_compare() {
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

        let str_ty = MIRType::Ptr(Box::new(MIRType::Int(8)));
        let lhs = ctx.add_local(Some("lhs".to_string()), LocalKind::User, str_ty.clone());
        let rhs = ctx.add_local(Some("rhs".to_string()), LocalKind::User, str_ty);
        ctx.local_names.insert("lhs".to_string(), lhs);
        ctx.local_names.insert("rhs".to_string(), rhs);
        ctx.bind_local_symbol(SymbolId::new(4), lhs);
        ctx.bind_local_symbol(SymbolId::new(5), rhs);

        let result = lower_binary_expr(
            &mut ctx,
            &hir::HIRBinaryOp::Eq,
            &HIRExpr::Var {
                name: "lhs".to_string(),
                symbol: SymbolId::new(4),
            },
            &HIRExpr::Var {
                name: "rhs".to_string(),
                symbol: SymbolId::new(5),
            },
        );

        assert_eq!(ctx.get_local_type(result), &MIR_BOOL);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, args, .. } if func == "sengoo_str_eq" && args == &vec![lhs, rhs]
        )));
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Binary { destination, op: MirBinOp::Ne, .. } if *destination == result
        )));
    }

    #[test]
    fn lower_logical_and_expr_emits_logand_binary() {
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

        let result = lower_logical_and_expr(
            &mut ctx,
            &HIRExpr::Lit(HIRLiteral::Bool(true)),
            &HIRExpr::Lit(HIRLiteral::Bool(false)),
        );

        assert_eq!(ctx.get_local_type(result), &MIR_BOOL);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Binary { destination, op: MirBinOp::LogAnd, .. } if *destination == result
        )));
    }

    #[test]
    fn lower_logical_or_expr_emits_logor_binary() {
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

        let result = lower_logical_or_expr(
            &mut ctx,
            &HIRExpr::Lit(HIRLiteral::Bool(true)),
            &HIRExpr::Lit(HIRLiteral::Bool(false)),
        );

        assert_eq!(ctx.get_local_type(result), &MIR_BOOL);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Binary { destination, op: MirBinOp::LogOr, .. } if *destination == result
        )));
    }
}
