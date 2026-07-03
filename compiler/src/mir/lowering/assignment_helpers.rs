use super::*;

pub(super) fn lower_assign_expr(
    ctx: &mut LoweringContext<'_>,
    target: &HIRExpr,
    value: &HIRExpr,
) -> Local {
    let value_local = ctx.lower_expr(value);

    match target {
        HIRExpr::Var { name, symbol } => {
            let target_local = ctx.resolve_local(name, *symbol);
            if value_local == target_local {
                return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
            }
            ctx.mark_drop_expr_moved(value);
            ctx.drop_local_now_if_initialized(target_local);
            if let Some(type_name) = ctx.type_names.get(&value_local).cloned() {
                ctx.type_names.insert(target_local, type_name);
            }
            ctx.push_inst(Instruction::Store {
                destination: target_local,
                value: value_local,
            });
            ctx.mark_drop_local_reinitialized(target_local);
        }
        HIRExpr::Field { .. } => {
            let Some((root, field_path)) = ctx.resolve_drop_place(target) else {
                ctx.errors.push("unsupported assignment target".to_string());
                return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
            };
            ctx.mark_drop_expr_moved(value);
            ctx.drop_field_now_if_initialized(root, &field_path);
            let Some(rebuilt) = ctx.rebuild_drop_place_with_value(root, &field_path, value_local)
            else {
                ctx.errors.push("unsupported assignment target".to_string());
                return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
            };
            ctx.push_inst(Instruction::Store {
                destination: root,
                value: rebuilt,
            });
            ctx.mark_drop_field_reinitialized(root, &field_path);
        }
        HIRExpr::Index { base, index } => {
            let base_local = ctx.lower_expr(base);
            let index_local = ctx.lower_expr(index);

            let base_ty = ctx.get_local_type(base_local).clone();
            let elem_ty = match &base_ty {
                MIRType::Array(elem, _) => (**elem).clone(),
                _ => {
                    ctx.errors
                        .push("index assignment on non-array type".to_string());
                    return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
                }
            };

            let addr_local = ctx.add_local(None, LocalKind::Temp, MIRType::Ptr(Box::new(elem_ty)));
            ctx.push_inst(Instruction::IndexAddr {
                destination: addr_local,
                base: base_local,
                index: index_local,
            });
            ctx.push_inst(Instruction::Store {
                destination: addr_local,
                value: value_local,
            });
        }
        _ => {
            ctx.errors.push("unsupported assignment target".to_string());
        }
    }

    ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
}

pub(super) fn lower_assign_op_expr(
    ctx: &mut LoweringContext<'_>,
    target: &HIRExpr,
    op: &hir::HIRBinaryOp,
    value: &HIRExpr,
) -> Local {
    let value_local = ctx.lower_expr(value);

    match target {
        HIRExpr::Var { name, symbol } => {
            let target_local = ctx.resolve_local(name, *symbol);
            let target_ty = ctx.get_local_type(target_local).clone();
            let current_val = ctx.add_local(None, LocalKind::Temp, target_ty.clone());
            ctx.push_inst(Instruction::Load {
                destination: current_val,
                source: target_local,
            });
            if matches!(op, hir::HIRBinaryOp::Add)
                && matches!(&target_ty, MIRType::Struct { name, .. } if name == "String")
                && matches!(ctx.get_local_type(value_local), MIRType::Ptr(inner) if matches!(inner.as_ref(), MIRType::Int(8)))
            {
                let handle = ctx.add_local(None, LocalKind::Temp, MIR_I64);
                ctx.push_inst(Instruction::Extract {
                    destination: handle,
                    value: current_val,
                    index: 0,
                });
                let value_ptr = ctx.add_local(None, LocalKind::Temp, MIR_I64);
                ctx.push_inst(Instruction::Call {
                    destination: value_ptr,
                    func: "sengoo_stdlib_str_ptr".to_string(),
                    args: vec![value_local],
                });
                let _status = ctx.add_local(None, LocalKind::Temp, MIR_I64);
                ctx.push_inst(Instruction::Call {
                    destination: _status,
                    func: "sengoo_string_push_str_status".to_string(),
                    args: vec![handle, value_ptr],
                });
                unwrap_nonnegative_i64_or_panic(ctx, _status);
                return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
            }
            let mir_op = ctx.lower_bin_op(op);
            let result = ctx.add_local(None, LocalKind::Temp, target_ty);
            ctx.push_inst(Instruction::Binary {
                destination: result,
                op: mir_op,
                left: current_val,
                right: value_local,
            });
            ctx.push_inst(Instruction::Store {
                destination: target_local,
                value: result,
            });
        }
        HIRExpr::Index { base, index } => {
            let base_local = ctx.lower_expr(base);
            let index_local = ctx.lower_expr(index);

            let base_ty = ctx.get_local_type(base_local).clone();
            let elem_ty = match &base_ty {
                MIRType::Array(elem, _) => (**elem).clone(),
                _ => {
                    ctx.errors
                        .push("index compound assignment on non-array type".to_string());
                    return ctx.add_local(None, LocalKind::Temp, MIR_UNIT);
                }
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
            let current_val = ctx.add_local(None, LocalKind::Temp, elem_ty.clone());
            ctx.push_inst(Instruction::Load {
                destination: current_val,
                source: addr_local,
            });
            let mir_op = ctx.lower_bin_op(op);
            let result = ctx.add_local(None, LocalKind::Temp, elem_ty);
            ctx.push_inst(Instruction::Binary {
                destination: result,
                op: mir_op,
                left: current_val,
                right: value_local,
            });
            ctx.push_inst(Instruction::Store {
                destination: addr_local,
                value: result,
            });
        }
        _ => {
            ctx.errors
                .push("unsupported compound assignment target".to_string());
        }
    }

    ctx.add_local(None, LocalKind::Temp, MIR_UNIT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::SymbolId;

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
    fn lower_assign_expr_skips_self_assignment_store() {
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

        let target = HIRExpr::Var {
            name: "x".to_string(),
            symbol: SymbolId::new(1),
        };
        let value = HIRExpr::Var {
            name: "x".to_string(),
            symbol: SymbolId::new(1),
        };
        let result = lower_assign_expr(&mut ctx, &target, &value);

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        assert!(!ctx.mir_fn.instructions.iter().any(|inst| matches!(inst, Instruction::Store { destination, value } if *destination == x && *value == x)));
    }

    #[test]
    fn lower_assign_op_expr_emits_binary_then_store_for_var_target() {
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

        let target = HIRExpr::Var {
            name: "x".to_string(),
            symbol: SymbolId::new(1),
        };
        let value = HIRExpr::Lit(HIRLiteral::Int(1));
        let result = lower_assign_op_expr(&mut ctx, &target, &hir::HIRBinaryOp::Add, &value);

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Binary {
                op: MirBinOp::Add,
                ..
            }
        )));
        assert!(ctx.mir_fn.instructions.iter().any(
            |inst| matches!(inst, Instruction::Store { destination, .. } if *destination == x)
        ));
    }
}
