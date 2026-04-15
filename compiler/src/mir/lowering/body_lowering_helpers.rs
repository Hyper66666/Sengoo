use super::*;

pub(super) fn lower_body_expr_to_new_block(
    ctx: &mut LoweringContext<'_>,
    body: &HIRBody,
) -> usize {
    let entry_block = ctx.new_block();
    ctx.lower_body_to_block(body, entry_block);
    entry_block
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
    fn lower_body_expr_to_new_block_returns_distinct_entry_block() {
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

        let block = lower_body_expr_to_new_block(
            &mut ctx,
            &HIRBody::with_expr(HIRExpr::Lit(HIRLiteral::Int(1))),
        );

        assert_ne!(block, start_block);
        assert!(ctx.mir_fn.basic_blocks.get(block).is_some());
    }
}