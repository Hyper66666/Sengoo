use super::*;

fn mir_local_name(local: Local) -> String {
    match local.kind {
        LocalKind::Param => format!("%l_{}", local.id),
        LocalKind::Temp => format!("%t_{}", local.id),
        LocalKind::User => format!("%u_{}", local.id),
        LocalKind::Return => format!("%ret_{}", local.id),
    }
}

pub(super) struct CallTargetPlan {
    pub(super) func_name: String,
    pub(super) ret_type: MIRType,
    pub(super) env_ptr_local: Option<Local>,
}

pub(super) enum CallTargetResolution {
    Builtin(Local),
    Planned(CallTargetPlan),
}

impl<'a> LoweringContext<'a> {
    fn fallback_named_call_target(&self, name: &str) -> CallTargetPlan {
        let ret_type = self
            .function_sig(name)
            .map(|sig| sig.ret_type.clone())
            .unwrap_or(MIR_I64);
        CallTargetPlan {
            func_name: name.to_string(),
            ret_type,
            env_ptr_local: None,
        }
    }

    pub(super) fn resolve_named_call_target(
        &mut self,
        name: &str,
        arg_locals: &[Local],
        expected_return_type: Option<&MIRType>,
    ) -> CallTargetResolution {
        if let Some(&var_local) = self.local_names.get(name) {
            if let Some(lambda_name) = self.lambda_names.get(&var_local) {
                let ret_type = self
                    .function_sig(lambda_name)
                    .map(|sig| sig.ret_type.clone())
                    .unwrap_or(MIR_I64);
                let env_ptr_local = self
                    .lambda_environments
                    .get(lambda_name)
                    .and_then(|env| env.env_ptr_local);
                return CallTargetResolution::Planned(CallTargetPlan {
                    func_name: lambda_name.clone(),
                    ret_type,
                    env_ptr_local,
                });
            }

            let fn_ty = self.get_local_type(var_local).clone();
            if let MIRType::Fn { ret, .. } = &fn_ty {
                let ret_type = (**ret).clone();
                let callable_local = if var_local.kind == LocalKind::User {
                    let loaded = self.add_local(None, LocalKind::Temp, fn_ty);
                    self.push_inst(Instruction::Load {
                        destination: loaded,
                        source: var_local,
                    });
                    loaded
                } else {
                    var_local
                };
                return CallTargetResolution::Planned(CallTargetPlan {
                    func_name: mir_local_name(callable_local),
                    ret_type,
                    env_ptr_local: None,
                });
            }

            return CallTargetResolution::Planned(self.fallback_named_call_target(name));
        }

        if let Some(builtin_local) = self.try_lower_builtin_call(name, arg_locals) {
            return CallTargetResolution::Builtin(builtin_local);
        }

        if let Some(plan) =
            self.try_materialize_generic_function(name, arg_locals, expected_return_type)
        {
            return CallTargetResolution::Planned(plan);
        }

        CallTargetResolution::Planned(self.fallback_named_call_target(name))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_named_call_target_prefers_local_fn_value_over_named_function() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::from([(
            "worker".to_string(),
            FunctionSig {
                ret_type: MIR_BOOL,
                param_count: 0,
                env: Vec::new(),
            },
        )]);
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

        let local = ctx.add_local(
            Some("worker".to_string()),
            LocalKind::User,
            MIRType::Fn {
                params: vec![],
                ret: Box::new(MIR_I64),
            },
        );
        ctx.local_names.insert("worker".to_string(), local);

        let resolution = ctx.resolve_named_call_target("worker", &[], None);
        match resolution {
            CallTargetResolution::Planned(plan) => {
                assert!(plan.func_name.starts_with("%t_"));
                assert_eq!(plan.ret_type, MIR_I64);
                assert_eq!(plan.env_ptr_local, None);
                assert!(ctx.mir_fn.instructions.iter().any(|inst| matches!(
                    inst,
                    Instruction::Load { source, .. } if *source == local
                )));
            }
            CallTargetResolution::Builtin(_) => panic!("expected function-value resolution"),
        }
    }

    #[test]
    fn resolve_named_call_target_returns_builtin_when_name_matches_builtin() {
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
        let resolution = ctx.resolve_named_call_target("task_status", &[task], None);
        match resolution {
            CallTargetResolution::Builtin(result_local) => {
                assert_eq!(ctx.get_local_type(result_local), &MIR_I64);
                let has_task_status_call = ctx.mir_fn.instructions.iter().any(|inst| {
                    matches!(
                        inst,
                        Instruction::Call { func, .. } if func == "sengoo_async_task_status"
                    )
                });
                assert!(has_task_status_call);
            }
            CallTargetResolution::Planned(_) => panic!("expected builtin resolution"),
        }
    }
}
