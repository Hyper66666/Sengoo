use super::*;

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
            let local = ctx.add_local(None, LocalKind::Temp, MIR_I64);
            ctx.push_inst(Instruction::Unary {
                destination: local,
                op: mir_op,
                operand: operand_local,
            });
            local
        }
    }
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
}