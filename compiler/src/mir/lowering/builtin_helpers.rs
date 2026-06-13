use super::*;

impl<'a> LoweringContext<'a> {
    pub(super) fn try_lower_builtin_call(
        &mut self,
        name: &str,
        arg_locals: &[Local],
    ) -> Option<Local> {
        match name {
            "print" | "println" => Some(self.lower_builtin_print(arg_locals)),
            "eprintln" => Some(self.lower_builtin_eprint(arg_locals)),
            "spawn" => Some(self.lower_builtin_spawn(arg_locals)),
            "spawn_task" => Some(self.lower_builtin_spawn_task(arg_locals)),
            "sleep" => Some(self.lower_builtin_sleep(arg_locals)),
            "timeout" => Some(self.lower_builtin_timeout(arg_locals)),
            "timeout_cancel" => Some(self.lower_builtin_timeout_cancel(arg_locals)),
            "join" => Some(self.lower_builtin_join(arg_locals)),
            "cancel_task" => Some(self.lower_builtin_cancel_task(arg_locals)),
            "task_status" => Some(self.lower_builtin_task_status(arg_locals)),
            "select" => Some(self.lower_builtin_select(arg_locals)),
            "select_cancel" => Some(self.lower_builtin_select_with_cancel(arg_locals)),
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

    pub(super) fn lower_builtin_eprint(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 1 {
            self.errors.push(format!(
                "eprintln expects exactly one argument, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let arg_local = arg_locals[0];
        let arg_ty = self.get_local_type(arg_local).clone();
        self.emit_eprint_value(arg_local, &arg_ty);
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

    pub(super) fn lower_builtin_timeout_cancel(&mut self, arg_locals: &[Local]) -> Local {
        if arg_locals.len() != 2 {
            self.errors.push(format!(
                "timeout_cancel expects exactly two arguments, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let future_handle = arg_locals[0];
        let duration_local = arg_locals[1];
        let base_name = self.resolve_async_base_name(future_handle);
        if base_name == "unknown" {
            self.errors.push(
                "timeout_cancel requires a future produced by an async function or async block"
                    .to_string(),
            );
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let result_ty = self.async_await_result_type(future_handle);
        if result_ty != MIR_I64 {
            self.errors.push(
                "timeout_cancel currently supports i64 inner futures during MIR lowering"
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

        let result_struct = MIRType::Struct {
            name: "Result".to_string(),
            fields: vec![
                ("is_ok".to_string(), MIR_BOOL),
                ("value".to_string(), MIR_I64),
                ("error".to_string(), MIR_I64),
            ],
        };
        let future_local = self.add_local(
            None,
            LocalKind::Temp,
            MIRType::Future(Box::new(result_struct)),
        );
        self.push_inst(Instruction::Call {
            destination: future_local,
            func: "sengoo_async_timeout_cancel_i64__start".to_string(),
            args: vec![kind_local, future_handle, duration_local],
        });
        self.future_origins
            .insert(future_local, "sengoo_async_timeout_cancel_i64".to_string());
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
            self.errors
                .push("unable to resolve async future origin during MIR lowering".to_string());
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
        self.lower_builtin_select_impl(arg_locals, false)
    }

    pub(super) fn lower_builtin_select_with_cancel(&mut self, arg_locals: &[Local]) -> Local {
        self.lower_builtin_select_impl(arg_locals, true)
    }

    fn lower_builtin_select_impl(&mut self, arg_locals: &[Local], cancel_losers: bool) -> Local {
        let builtin = if cancel_losers {
            "select_cancel"
        } else {
            "select"
        };
        if !(2..=8).contains(&arg_locals.len()) {
            self.errors.push(format!(
                "{builtin} expects between two and eight arguments, got {}",
                arg_locals.len()
            ));
            return self.add_local(None, LocalKind::Temp, MIR_UNIT);
        }

        let mut operand_names = Vec::with_capacity(arg_locals.len());
        for handle in arg_locals {
            let name = self.resolve_async_base_name(*handle);
            if name == "unknown" {
                self.errors.push(format!(
                    "{builtin} requires futures produced by async functions or async blocks"
                ));
                return self.add_local(None, LocalKind::Temp, MIR_UNIT);
            }
            operand_names.push(name);
        }

        let result_ty = self.async_await_result_type(arg_locals[0]);
        for handle in &arg_locals[1..] {
            if self.async_await_result_type(*handle) != result_ty {
                self.errors.push(format!(
                    "{builtin} requires futures with matching result types during MIR lowering"
                ));
                return self.add_local(None, LocalKind::Temp, MIR_UNIT);
            }
        }

        let mut kind_locals = Vec::with_capacity(arg_locals.len());
        for name in &operand_names {
            let Some(kind_id) = self.async_dispatch_kind_id(name) else {
                return self.add_local(None, LocalKind::Temp, MIR_UNIT);
            };
            let kind_local = self.add_local(None, LocalKind::Temp, MIR_I64);
            self.push_inst(Instruction::Assign {
                destination: kind_local,
                value: MirConstant::Int(kind_id),
            });
            kind_locals.push(kind_local);
        }

        let entry_block = self.current_block();
        let winner = self.add_local(None, LocalKind::Temp, MIR_I64);
        let winner_args = if arg_locals.len() == 2 {
            vec![kind_locals[0], arg_locals[0], kind_locals[1], arg_locals[1]]
        } else {
            let mut args = Vec::with_capacity(17);
            let count_local = self.add_local(None, LocalKind::Temp, MIR_I64);
            self.push_inst(Instruction::Assign {
                destination: count_local,
                value: MirConstant::Int(arg_locals.len() as i64),
            });
            args.push(count_local);
            for index in 0..8 {
                if index < arg_locals.len() {
                    args.push(kind_locals[index]);
                    args.push(arg_locals[index]);
                } else {
                    let zero_kind = self.add_local(None, LocalKind::Temp, MIR_I64);
                    self.push_inst(Instruction::Assign {
                        destination: zero_kind,
                        value: MirConstant::Int(0),
                    });
                    let zero_handle = self.add_local(None, LocalKind::Temp, MIR_I64);
                    self.push_inst(Instruction::Assign {
                        destination: zero_handle,
                        value: MirConstant::Int(0),
                    });
                    args.push(zero_kind);
                    args.push(zero_handle);
                }
            }
            args
        };
        let winner_fn = match (cancel_losers, arg_locals.len() == 2) {
            (false, true) => {
                crate::mir::async_dispatch_synthesis_helpers::select_winner_runtime_function_name()
            }
            (false, false) => {
                crate::mir::async_dispatch_synthesis_helpers::select_n_winner_runtime_function_name()
            }
            (true, true) => crate::mir::async_dispatch_synthesis_helpers::
                select_cancel_winner_runtime_function_name(),
            (true, false) => crate::mir::async_dispatch_synthesis_helpers::
                select_cancel_n_winner_runtime_function_name(),
        };
        self.push_inst(Instruction::Call {
            destination: winner,
            func: winner_fn.to_string(),
            args: winner_args,
        });

        let join_block = self.new_block();
        let mut ready_blocks = Vec::with_capacity(arg_locals.len());
        let mut branch_results = Vec::with_capacity(arg_locals.len());

        for (handle, name) in arg_locals.iter().zip(operand_names.iter()) {
            let ready_block = self.new_block();
            ready_blocks.push(ready_block);
            self.set_current_block(ready_block);
            let branch_result = self.add_local(None, LocalKind::Temp, result_ty.clone());
            self.push_inst(Instruction::Call {
                destination: branch_result,
                func: format!("{name}__result"),
                args: vec![*handle],
            });
            self.set_terminator(Terminator::Goto(join_block));
            branch_results.push((branch_result, self.current_block()));
        }

        self.set_current_block(entry_block);
        self.emit_select_winner_switch(winner, &ready_blocks);

        self.set_current_block(join_block);
        if is_void_like(&result_ty) {
            self.add_local(None, LocalKind::Temp, MIR_UNIT)
        } else {
            let result_local = self.add_local(None, LocalKind::Temp, result_ty);
            let incoming: Vec<(Local, usize)> = branch_results
                .iter()
                .map(|(local, block)| (*local, *block))
                .collect();
            self.push_inst(Instruction::Phi {
                destination: result_local,
                incoming: incoming.clone(),
            });
            self.propagate_future_origin_through_phi(result_local, &incoming);
            result_local
        }
    }

    fn emit_select_winner_switch(&mut self, winner: Local, ready_blocks: &[usize]) {
        if ready_blocks.len() == 2 {
            self.set_terminator(Terminator::If {
                cond: winner,
                then_block: ready_blocks[1],
                else_block: ready_blocks[0],
            });
            return;
        }

        let targets = ready_blocks
            .iter()
            .enumerate()
            .take(ready_blocks.len().saturating_sub(1))
            .map(|(index, &block)| (index as u32, block))
            .collect();
        let otherwise = *ready_blocks
            .last()
            .expect("select has at least two operands");
        self.set_terminator(Terminator::Switch {
            discr: winner,
            targets,
            otherwise,
        });
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
        let has_task_status_call = ctx.mir_fn.instructions.iter().any(|inst| {
            matches!(
                inst,
                Instruction::Call { func, .. } if func == "sengoo_async_task_status"
            )
        });
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
        assert!(
            ctx.errors.is_empty(),
            "non-builtin dispatch should stay silent"
        );
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

        let options = MirLowerOptions::default().with_async_functions(
            ["left".to_string(), "right".to_string()]
                .into_iter()
                .collect(),
        );

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

    #[test]
    fn lower_builtin_select_branches_on_winner_and_merges_non_scalar_results() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::new();
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();

        let options = MirLowerOptions::default().with_async_functions(
            ["left".to_string(), "right".to_string()]
                .into_iter()
                .collect(),
        );

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

        let tuple_ty = MIRType::Tuple(vec![MIR_I64, MIR_BOOL]);
        let left = ctx.add_local(
            None,
            LocalKind::Temp,
            MIRType::Future(Box::new(tuple_ty.clone())),
        );
        let right = ctx.add_local(None, LocalKind::Temp, MIRType::Future(Box::new(tuple_ty)));
        ctx.future_origins.insert(left, "left".to_string());
        ctx.future_origins.insert(right, "right".to_string());

        let result = ctx.lower_builtin_select(&[left, right]);

        assert!(
            ctx.errors.is_empty(),
            "unexpected lowering errors: {:?}",
            ctx.errors
        );
        assert_eq!(
            ctx.get_local_type(result),
            &MIRType::Tuple(vec![MIR_I64, MIR_BOOL])
        );
        assert!(
            ctx.mir_fn.instructions.iter().any(|inst| matches!(
                inst,
                Instruction::Call { func, .. } if func == "sengoo_async_select_winner"
            )),
            "select lowering should call the winner runtime"
        );
        assert!(
            ctx.mir_fn.instructions.iter().any(|inst| matches!(
                inst,
                Instruction::Call { func, args, .. } if func == "left__result" && args == &vec![left]
            )),
            "select lowering should collect the left future result"
        );
        assert!(
            ctx.mir_fn.instructions.iter().any(|inst| matches!(
                inst,
                Instruction::Call { func, args, .. } if func == "right__result" && args == &vec![right]
            )),
            "select lowering should collect the right future result"
        );
        assert!(
            ctx.mir_fn.instructions.iter().any(|inst| matches!(
                inst,
                Instruction::Phi { destination, incoming }
                    if *destination == result && incoming.len() == 2
            )),
            "select lowering should merge branch results with a phi"
        );
        assert!(
            matches!(
                ctx.mir_fn.basic_blocks[start_block].terminator,
                Some(Terminator::If { .. })
            ),
            "select lowering should branch on the winner result"
        );
    }
}
