use super::*;

impl<'a> LoweringContext<'a> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        mir_fn: &'a mut MirFunction,
        lambda_counter: &'a mut usize,
        known_functions: &'a HashSet<String>,
        known_function_sigs: &'a HashMap<String, FunctionSig>,
        struct_defs: &'a HashMap<String, &'a hir::HIRStruct>,
        concrete_type_registry: ConcreteTypeRegistry,
        options: MirLowerOptions,
        inherent_method_templates: &'a [InherentMethodTemplate],
        trait_method_templates: &'a [TraitMethodTemplate],
    ) -> Self {
        let async_names = options
            .async_functions
            .borrow()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let async_dispatch_registry = build_async_dispatch_registry(async_names);
        Self {
            mir_fn,
            local_names: HashMap::new(),
            local_symbols: HashMap::new(),
            contract_param_bindings: Vec::new(),
            current_block: None,
            errors: Vec::new(),
            loop_stack: Vec::new(),
            lambda_counter,
            lambda_functions: Vec::new(),
            lambda_names: HashMap::new(),
            function_sigs_base: known_function_sigs,
            function_sigs_overlay: HashMap::new(),
            lambda_environments: HashMap::new(),
            type_names: HashMap::new(),
            known_functions_base: known_functions,
            known_functions_overlay: HashSet::new(),
            struct_defs,
            concrete_type_registry,
            options,
            inherent_method_templates,
            trait_method_templates,
            async_dispatch_registry,
            future_origins: HashMap::new(),
            try_scope_stack: Vec::new(),
        }
    }

    pub(super) fn push_try_scope(&mut self, scope: try_expr_helpers::TryScope) {
        self.try_scope_stack.push(scope);
    }

    pub(super) fn pop_try_scope(&mut self) {
        self.try_scope_stack.pop();
    }

    pub(super) fn async_dispatch_kind_id(&mut self, base_name: &str) -> Option<i64> {
        self.async_dispatch_registry.kind_id(base_name).or_else(|| {
            self.errors.push(format!(
                "unable to assign a stable async dispatch id for future origin `{base_name}` during MIR lowering"
            ));
            None
        })
    }

    fn lower_materialized_function(
        &mut self,
        specialized: hir::HIRFunction,
        param_count: usize,
    ) -> Option<String> {
        if self.is_known_function(&specialized.name) {
            return Some(specialized.name);
        }

        self.insert_function_sig(
            specialized.name.clone(),
            build_hir_function_sig(&specialized.return_type, param_count, self.struct_defs),
        );
        self.insert_known_function(specialized.name.clone());

        match lower_function(
            &specialized,
            self.lambda_counter,
            self.known_functions_base,
            self.function_sigs_base,
            self.struct_defs,
            self.concrete_type_registry.clone(),
            &self.options,
            self.inherent_method_templates,
            self.trait_method_templates,
            self.known_functions_overlay.clone(),
            self.function_sigs_overlay.clone(),
        ) {
            Ok((mir_fn, nested)) => {
                self.lambda_functions.push(mir_fn);
                self.lambda_functions.extend(nested);
                Some(specialized.name)
            }
            Err(error) => {
                self.errors.push(error);
                None
            }
        }
    }

    fn lower_materialized_method(&mut self, specialized: hir::HIRFunction) -> Option<String> {
        let param_count = explicit_hir_method_param_count(&specialized);
        self.lower_materialized_function(specialized, param_count)
    }

    pub(super) fn try_materialize_generic_function(
        &mut self,
        name: &str,
        arg_locals: &[Local],
    ) -> Option<CallTargetPlan> {
        let template = self.options.generic_function_templates.get(name)?.clone();
        if template.params.len() != arg_locals.len() {
            return None;
        }

        let actual_arg_types =
            collect_local_types(arg_locals, |local| self.get_local_type(local).clone());
        let mut mir_subst = HashMap::new();
        for (param, actual_ty) in template.params.iter().zip(actual_arg_types.iter()) {
            bind_mir_subst_from_hir_type(&param.ty, actual_ty, self.struct_defs, &mut mir_subst);
        }

        let mut hir_subst = HashMap::new();
        for type_param in &template.type_params {
            let Some(mir_ty) = mir_subst.get(&type_param.name) else {
                self.errors.push(format!(
                    "generic function {}: type parameter {} could not be inferred during MIR lowering",
                    name, type_param.name
                ));
                return None;
            };
            let Some(hir_ty) = self.concrete_type_registry.hir_type_for_mir(mir_ty) else {
                self.errors.push(format!(
                    "generic function {}: concrete type argument for {} could not be resolved during MIR lowering",
                    name, type_param.name
                ));
                return None;
            };
            hir_subst.insert(type_param.name.clone(), hir_ty);
        }

        for ty in hir_subst.values() {
            if matches!(ty.kind, hir::HIRTypeKind::Named { .. }) {
                self.concrete_type_registry
                    .register_instance(crate::type_naming::hir_type_instance_name(ty), ty.clone());
            }
        }

        let mut specialized =
            crate::mir::hir_specialization_helpers::substitute_hir_function(&template, &hir_subst);
        specialized.type_params.clear();
        let suffixes = template
            .type_params
            .iter()
            .filter_map(|param| hir_subst.get(&param.name))
            .map(crate::type_naming::hir_type_instance_name)
            .collect::<Vec<_>>();
        specialized.name = format!("{}_{}", template.name, suffixes.join("_"));

        let specialized_name =
            self.lower_materialized_function(specialized.clone(), specialized.params.len())?;
        let ret_type = self
            .function_sig(&specialized_name)
            .map(|sig| sig.ret_type.clone())
            .unwrap_or_else(|| {
                hir_type_to_mir_with_structs(&specialized.return_type, self.struct_defs)
            });

        Some(CallTargetPlan {
            func_name: specialized_name,
            ret_type,
            env_ptr_local: None,
        })
    }

    pub(super) fn try_materialize_inherent_method(
        &mut self,
        receiver_ty: &MIRType,
        method: &str,
        arg_locals: &[Local],
    ) -> Option<String> {
        let actual_arg_types =
            collect_local_types(arg_locals, |local| self.get_local_type(local).clone());
        let specialized = resolve_inherent_method_specialization(
            self.inherent_method_templates,
            method,
            receiver_ty,
            &actual_arg_types,
            self.struct_defs,
            &mut self.concrete_type_registry,
        )?;

        self.lower_materialized_method(specialized)
    }

    pub(super) fn try_materialize_trait_method(
        &mut self,
        receiver_ty: &MIRType,
        method: &str,
        arg_locals: &[Local],
        type_display: &str,
    ) -> Result<Option<String>, String> {
        let actual_arg_types =
            collect_local_types(arg_locals, |local| self.get_local_type(local).clone());
        let specialized = resolve_trait_method_specialization(
            self.trait_method_templates,
            method,
            receiver_ty,
            &actual_arg_types,
            self.struct_defs,
            &mut self.concrete_type_registry,
            type_display,
        )?;

        match specialized {
            Some(specialized) => Ok(self.lower_materialized_method(specialized)),
            None => Ok(None),
        }
    }

    pub(super) fn infer_struct_literal_type(
        &mut self,
        name: &str,
        field_locals: &HashMap<String, Local>,
    ) -> Option<MIRType> {
        let def = self.struct_defs.get(name)?;
        let mut subst: HashMap<String, MIRType> = HashMap::new();
        for field in &def.fields {
            let local = field_locals.get(&field.name)?;
            bind_mir_subst_from_hir_type(
                &field.ty,
                self.get_local_type(*local),
                self.struct_defs,
                &mut subst,
            );
        }

        if !def.type_params.is_empty()
            && !def
                .type_params
                .iter()
                .all(|type_param| subst.contains_key(&type_param.name))
        {
            return None;
        }

        let (instance_name, concrete_hir_ty) = if def.type_params.is_empty() {
            (
                name.to_string(),
                HIRType::named(name.to_string(), Vec::new()),
            )
        } else {
            let mut parts: Vec<String> = Vec::with_capacity(def.type_params.len());
            let mut concrete_args: Vec<HIRType> = Vec::with_capacity(def.type_params.len());

            for type_param in &def.type_params {
                let Some(mir_arg) = subst.get(&type_param.name) else {
                    self.errors.push(format!(
                        "struct literal {}: generic type parameter {} could not be inferred during MIR lowering",
                        name, type_param.name
                    ));
                    return None;
                };
                parts.push(mir_type_to_instance_name(mir_arg));

                let Some(hir_arg) = self.concrete_type_registry.hir_type_for_mir(mir_arg) else {
                    self.errors.push(format!(
                        "struct literal {}: concrete type argument for {} could not be resolved during MIR lowering",
                        name, type_param.name
                    ));
                    return None;
                };
                concrete_args.push(hir_arg);
            }

            (
                format!("{}_{}", name, parts.join("_")),
                HIRType::named(name.to_string(), concrete_args),
            )
        };
        self.concrete_type_registry
            .register_instance(instance_name.clone(), concrete_hir_ty);

        Some(MIRType::Struct {
            name: instance_name,
            fields: def
                .fields
                .iter()
                .map(|field| {
                    (
                        field.name.clone(),
                        hir_type_to_mir_with_structs_and_subst(&field.ty, self.struct_defs, &subst),
                    )
                })
                .collect(),
        })
    }

    pub(super) fn lambda_name(&mut self) -> String {
        let name = format!("$__lambda{}", self.lambda_counter);
        *self.lambda_counter += 1;
        name
    }

    pub(super) fn async_block_name(&mut self) -> String {
        let name = format!("$__async_block{}", self.lambda_counter);
        *self.lambda_counter += 1;
        name
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn materialized_entries_are_kept_in_overlay_without_mutating_base() {
        let mut mir_fn = MirFunction::new("probe".to_string(), Vec::new(), MIR_I64);
        let mut lambda_counter = 0;
        let mut known_functions = HashSet::new();
        known_functions.insert("base".to_string());
        let mut function_sigs = HashMap::new();
        function_sigs.insert(
            "base".to_string(),
            build_function_sig(MIR_I64, 0, Vec::new()),
        );
        let struct_defs = HashMap::new();
        let concrete_named_types = HashMap::new();

        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::new(&struct_defs, &concrete_named_types),
            MirLowerOptions::default(),
            &[],
            &[],
        );

        ctx.insert_known_function("materialized".to_string());
        ctx.insert_function_sig(
            "materialized".to_string(),
            build_function_sig(MIR_BOOL, 1, Vec::new()),
        );

        assert!(ctx.is_known_function("base"));
        assert!(ctx.is_known_function("materialized"));
        assert_eq!(
            ctx.function_sig("materialized").map(|sig| &sig.ret_type),
            Some(&MIR_BOOL)
        );
        assert!(!known_functions.contains("materialized"));
        assert!(!function_sigs.contains_key("materialized"));
    }
}
