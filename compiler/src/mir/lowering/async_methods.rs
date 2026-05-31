use super::*;

impl<'a> LoweringContext<'a> {
    fn collect_async_block_free_vars(&self, body: &crate::hir::HIRBody) -> Vec<(String, Local)> {
        collect_free_vars_in_body(body, &self.local_names)
    }

    pub(super) fn lower_async_block(&mut self, body: &HIRBody) -> Local {
        let async_block_name = self.async_block_name();
        let free_vars = self.collect_async_block_free_vars(body);
        let capture_types: Vec<MIRType> = free_vars
            .iter()
            .map(|(_, local)| self.get_local_type(*local).clone())
            .collect();
        let capture_args: Vec<Local> = free_vars.iter().map(|(_, local)| *local).collect();

        let capture_arity = capture_types.len();
        let mut async_fn = MirFunction::new(async_block_name.clone(), capture_types, MIR_UNIT);
        async_fn.is_async = true;
        let async_start = async_fn.start_block;

        let mut async_ctx = LoweringContext::new(
            &mut async_fn,
            self.lambda_counter,
            &self.known_functions,
            &self.function_sigs,
            self.struct_defs,
            self.concrete_type_registry.clone(),
            self.options.clone(),
            self.inherent_method_templates,
            self.trait_method_templates,
        );
        async_ctx.current_block = Some(async_start);

        for (index, (var_name, outer_local)) in free_vars.iter().enumerate() {
            let param_local = Local::new(index + 1, LocalKind::Param);
            async_ctx.local_names.insert(var_name.clone(), param_local);
            if let Some(type_name) = self.type_names.get(outer_local).cloned() {
                async_ctx.type_names.insert(param_local, type_name);
            }
            if let Some(origin) = self.future_origins.get(outer_local).cloned() {
                async_ctx.future_origins.insert(param_local, origin);
            }
        }

        let result_local = async_ctx.lower_body_to_block_val(body, async_start);
        let result_ty = async_ctx.get_local_type(result_local).clone();
        async_ctx.mir_fn.return_type = result_ty.clone();
        if let Some((_, slot_ty)) = async_ctx.mir_fn.locals.get_mut(0) {
            *slot_ty = result_ty.clone();
        }

        let cur = async_ctx.current_block();
        let already_terminated = async_ctx
            .mir_fn
            .block_mut(cur)
            .is_some_and(|block| block.terminator.is_some());
        if !already_terminated {
            if matches!(result_ty, MIRType::Unit) {
                async_ctx.set_terminator(Terminator::Return(None));
            } else {
                async_ctx.set_terminator(Terminator::Return(Some(result_local)));
            }
        }

        let async_errors = std::mem::take(&mut async_ctx.errors);
        let nested_functions = std::mem::take(&mut async_ctx.lambda_functions);
        drop(async_ctx);

        if !async_errors.is_empty() {
            self.errors.push(format!(
                "async block lowering failed for '{}':\n  {}",
                async_block_name,
                async_errors.join("\n  ")
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        self.known_functions.to_mut().insert(async_block_name.clone());
        self.options
            .async_functions
            .borrow_mut()
            .insert(async_block_name.clone());
        self.function_sigs.to_mut().insert(
            async_block_name.clone(),
            build_function_sig(result_ty.clone(), capture_arity, vec![]),
        );

        self.lambda_functions.push(async_fn);
        self.lambda_functions.extend(nested_functions);

        let future_local = self.add_local(None, LocalKind::Temp, result_ty);
        self.push_inst(Instruction::Call {
            destination: future_local,
            func: format!("{}__start", async_block_name),
            args: capture_args,
        });
        self.future_origins.insert(future_local, async_block_name);
        future_local
    }

    fn infer_poll_func_from_last_call(&self) -> String {
        let block = &self.mir_fn.basic_blocks[self.current_block()];
        let instructions = block
            .instructions
            .iter()
            .map(|inst_id| self.mir_fn.instruction(*inst_id));
        infer_last_async_start_base(instructions).unwrap_or_else(|| "unknown".to_string())
    }

    /// Resolve the async function base name for a given future handle local.
    ///
    /// Resolution order:
    ///  1. Direct lookup in `future_origins` — covers `await async_fn(args)`.
    ///  2. If the handle came from a `Load { destination: handle, source: src }`,
    ///     look up `src` in `future_origins` — covers `let f = async_fn(); await f`.
    ///  3. Fall back to backward-scan heuristic via `infer_poll_func_from_last_call`.
    pub(super) fn resolve_async_base_name(&self, handle: Local) -> String {
        let block = &self.mir_fn.basic_blocks[self.current_block()];
        let instructions = block
            .instructions
            .iter()
            .map(|inst_id| self.mir_fn.instruction(*inst_id));

        infer_async_base_name_from_instructions(handle, instructions, &self.future_origins)
            .unwrap_or_else(|| self.infer_poll_func_from_last_call())
    }
}
