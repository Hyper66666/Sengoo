use super::*;

impl<'a> LoweringContext<'a> {
    pub(super) fn try_lower_builtin_call(
        &mut self,
        name: &str,
        arg_locals: &[Local],
    ) -> Option<Local> {
        match name {
            "print" => Some(self.lower_builtin_print(arg_locals)),
            "spawn" => Some(self.lower_builtin_spawn(arg_locals)),
            "spawn_task" => Some(self.lower_builtin_spawn_task(arg_locals)),
            "sleep" => Some(self.lower_builtin_sleep(arg_locals)),
            "timeout" => Some(self.lower_builtin_timeout(arg_locals)),
            "join" => Some(self.lower_builtin_join(arg_locals)),
            "cancel_task" => Some(self.lower_builtin_cancel_task(arg_locals)),
            "task_status" => Some(self.lower_builtin_task_status(arg_locals)),
            "select" => Some(self.lower_builtin_select(arg_locals)),
            _ => None,
        }
    }

    pub(super) fn lower_builtin_print(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "print expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let arg_local = arg_locals[0];
        let arg_ty = self.get_local_type(arg_local).clone();
        self.emit_print_value(arg_local, &arg_ty);
        self.add_local(None, LocalKind::Temp, MIR_UNIT)
    }

    pub(super) fn lower_builtin_spawn(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "spawn expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let future_handle = arg_locals[0];
        let base_name = self.resolve_async_base_name(future_handle);
        if base_name == "unknown" {
            self.errors.push(
                "spawn requires a future produced by an async function or async block".to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let Some(kind_id) = self.async_dispatch_kind_id(&base_name) else {
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        };
        let kind_local = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: kind_local,
            value: MirConstant::Int(kind_id),
        });

        let task_id = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Call {
            destination: task_id,
            func: "sengoo_async_spawn_raw".to_string(),
            args: vec![kind_local, future_handle],
        });

        future_handle
    }

    pub(super) fn lower_builtin_spawn_task(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "spawn_task expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let future_handle = arg_locals[0];
        let base_name = self.resolve_async_base_name(future_handle);
        if base_name == "unknown" {
            self.errors.push(
                "spawn_task requires a future produced by an async function or async block"
                    .to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let Some(kind_id) = self.async_dispatch_kind_id(&base_name) else {
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        };
        let kind_local = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: kind_local,
            value: MirConstant::Int(kind_id),
        });

        let task_id = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Call {
            destination: task_id,
            func: "sengoo_async_spawn_raw".to_string(),
            args: vec![kind_local, future_handle],
        });

        task_id
    }

    pub(super) fn lower_builtin_sleep(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "sleep expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let duration_local = arg_locals[0];
        let future_local =
            self.add_local(None, LocalKind::Temp, MIRType::Future(Box::new(MIR_UNIT)));
        self.push_inst(Instruction::Call {
            destination: future_local,
            func: "sengoo_async_sleep__start".to_string(),
            args: vec![duration_local],
        });
        self.future_origins
            .insert(future_local, "sengoo_async_sleep".to_string());
        future_local
    }

    pub(super) fn lower_builtin_timeout(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 2 {
            self.errors.push(format!(
                "timeout expects exactly two arguments, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let future_handle = arg_locals[0];
        let duration_local = arg_locals[1];
        let base_name = self.resolve_async_base_name(future_handle);
        if base_name == "unknown" {
            self.errors.push(
                "timeout requires a future produced by an async function or async block"
                    .to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let Some(kind_id) = self.async_dispatch_kind_id(&base_name) else {
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        };
        let kind_local = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: kind_local,
            value: MirConstant::Int(kind_id),
        });

        let future_local =
            self.add_local(None, LocalKind::Temp, MIRType::Future(Box::new(MIR_BOOL)));
        self.push_inst(Instruction::Call {
            destination: future_local,
            func: "sengoo_async_timeout_bool__start".to_string(),
            args: vec![kind_local, future_handle, duration_local],
        });
        self.future_origins
            .insert(future_local, "sengoo_async_timeout_bool".to_string());
        future_local
    }

    pub(super) fn async_await_result_type(&self, future_handle: Local) -> MIRType {
        match self.get_local_type(future_handle) {
            MIRType::Future(inner) => (**inner).clone(),
            _ => MIR_I64,
        }
    }

    pub(super) fn lower_async_wait(&mut self, future_handle: Local) -> Local {
        let func_name = self.resolve_async_base_name(future_handle);
        if func_name == "unknown" {
            self.errors.push(
                "unable to resolve async future origin during MIR lowering".to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let result_ty = self.async_await_result_type(future_handle);
        let result_local = self.add_local(None, LocalKind::Temp, result_ty);
        let poll_result = self.add_local(None, LocalKind::Temp, MIR_I64);
        let ready_block = self.new_block();
        let pending_block = self.new_block();

        self.set_terminator(Terminator::Suspend {
            poll_func: format!("{}__poll", func_name),
            future_handle,
            destination: poll_result,
            ready_block,
            pending_block,
        });

        self.set_current_block(pending_block);
        self.set_terminator(Terminator::Goto(self.current_block()));

        self.set_current_block(ready_block);
        self.push_inst(Instruction::Call {
            destination: result_local,
            func: format!("{}__result", func_name),
            args: vec![future_handle],
        });
        result_local
    }

    pub(super) fn lower_builtin_join(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 2 {
            self.errors.push(format!(
                "join expects exactly two arguments, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let _first_result = self.lower_async_wait(arg_locals[0]);
        let _second_result = self.lower_async_wait(arg_locals[1]);
        self.add_local(None, LocalKind::Temp, MIR_UNIT)
    }

    pub(super) fn lower_builtin_cancel_task(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "cancel_task expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let result_local = self.add_local(None, LocalKind::Temp, MIR_BOOL);
        self.push_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_cancel_task".to_string(),
            args: vec![arg_locals[0]],
        });
        result_local
    }

    pub(super) fn lower_builtin_task_status(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "task_status expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let result_local = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Call {
            destination: result_local,
            func: "sengoo_async_task_status".to_string(),
            args: vec![arg_locals[0]],
        });
        result_local
    }

    pub(super) fn lower_builtin_select(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 2 {
            self.errors.push(format!(
                "select expects exactly two arguments, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let first_handle = arg_locals[0];
        let second_handle = arg_locals[1];
        let first_name = self.resolve_async_base_name(first_handle);
        let second_name = self.resolve_async_base_name(second_handle);
        if first_name == "unknown" || second_name == "unknown" {
            self.errors.push(
                "select requires futures produced by async functions or async blocks".to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let result_ty = self.async_await_result_type(first_handle);
        let Some(select_runtime) = select_runtime_function_name(&result_ty) else {
            self.errors.push(
                "select currently only supports Future values whose results are bool, integer, or float scalars during MIR lowering"
                    .to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        };

        let second_result_ty = self.async_await_result_type(second_handle);
        if second_result_ty != result_ty {
            self.errors.push(
                "select requires futures with matching result types during MIR lowering"
                    .to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let Some(first_kind_id) = self.async_dispatch_kind_id(&first_name) else {
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        };
        let first_kind = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: first_kind,
            value: MirConstant::Int(first_kind_id),
        });

        let Some(second_kind_id) = self.async_dispatch_kind_id(&second_name) else {
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        };
        let second_kind = self.add_local(None, LocalKind::Temp, MIR_I64);
        self.push_inst(Instruction::Assign {
            destination: second_kind,
            value: MirConstant::Int(second_kind_id),
        });

        let result_local = self.add_local(None, LocalKind::Temp, result_ty);
        self.push_inst(Instruction::Call {
            destination: result_local,
            func: select_runtime,
            args: vec![first_kind, first_handle, second_kind, second_handle],
        });
        result_local
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_lower_builtin_call_dispatches_known_builtin_name() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::new();
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();

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

        let task = ctx.add_local(None, LocalKind::Temp, MIR_I64);
        let result = ctx.try_lower_builtin_call("task_status", &[task]);

        assert!(result.is_some(), "expected builtin dispatch to return Some");
        let has_task_status_call = ctx.mir_fn.instructions.iter().any(|inst| matches!(
            inst,
            Instruction::Call { func, .. } if func == "sengoo_async_task_status"
        ));
        assert!(
            has_task_status_call,
            "expected builtin dispatch to emit task_status runtime call"
        );
    }

    #[test]
    fn try_lower_builtin_call_ignores_non_builtin_name() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::new();
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();

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

        assert!(
            ctx.try_lower_builtin_call("user_function", &[]).is_none(),
            "non-builtin names should bypass builtin dispatch"
        );
        assert!(ctx.errors.is_empty(), "non-builtin dispatch should stay silent");
    }

    #[test]
    fn lower_builtin_select_records_mismatched_future_result_types() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::new();
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();

        let mut options = MirLowerOptions::default();
        options.async_functions = ["left".to_string(), "right".to_string()]
            .into_iter()
            .collect();

        let start_block = mir_fn.start_block;
        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            options,
            &inherent_templates,
            &trait_templates,
        );
        ctx.set_current_block(start_block);

        let left = ctx.add_local(None, LocalKind::Temp, MIRType::Future(Box::new(MIR_I64)));
        let right = ctx.add_local(None, LocalKind::Temp, MIRType::Future(Box::new(MIR_BOOL)));
        ctx.future_origins.insert(left, "left".to_string());
        ctx.future_origins.insert(right, "right".to_string());

        let result = ctx.lower_builtin_select(&[left, right]);

        assert_eq!(ctx.get_local_type(result), &MIR_UNIT);
        assert!(
            ctx.errors
                .iter()
                .any(|err| err.contains("matching result types")),
            "expected mismatched future type diagnostic, got {:?}",
            ctx.errors
        );
    }
}
