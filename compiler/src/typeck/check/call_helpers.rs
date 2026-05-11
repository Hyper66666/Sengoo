use super::*;

impl TypeChecker {
    fn instantiate_method_function_ty(
        &mut self,
        fn_ty: &FunctionTy,
        subst: &HashMap<TyVarId, Ty>,
    ) -> FunctionTy {
        let mut call_subst = subst.clone();
        for generic_param in &fn_ty.generic_params {
            call_subst.insert(*generic_param, self.env.new_ty_var());
        }
        FunctionTy::new(
            fn_ty.has_self,
            fn_ty
                .param_types
                .iter()
                .map(|param| self.substitute_ty_vars(param, &call_subst))
                .collect(),
            self.substitute_ty_vars(&fn_ty.return_type, &call_subst),
        )
    }
    fn lookup_generic_inherent_method(
        &mut self,
        receiver_ty: &Ty,
        method_name: &str,
    ) -> Option<FunctionTy> {
        let lookup_key = self.generic_lookup_key(receiver_ty);
        let impls = self.impl_registry.get_inherent_impls(&lookup_key);

        for impl_info in impls {
            let mut subst = HashMap::new();
            if !self.match_generic_impl_target(&impl_info.target_type, receiver_ty, &mut subst) {
                continue;
            }
            if let Some(fn_ty) = impl_info.get_method(method_name).cloned() {
                return Some(self.instantiate_method_function_ty(&fn_ty, &subst));
            }
        }
        None
    }

    fn resolve_struct_field_types(&mut self, struct_name: &str) -> TyResult<Vec<(String, Ty)>> {
        let field_defs = self
            .struct_field_defs
            .get(struct_name)
            .cloned()
            .ok_or_else(|| {
                TypeckError::Other(format!(
                    "print cannot resolve fields for struct `{}`",
                    struct_name
                ))
            })?;

        let mut resolved = Vec::with_capacity(field_defs.len());
        for (field_name, field_ty) in field_defs {
            let ty = self.check_type(&field_ty)?;
            resolved.push((field_name, ty));
        }
        Ok(resolved)
    }

    fn ensure_type_printable_for_print(
        &mut self,
        ty: &Ty,
        context: &str,
        visiting: &mut HashSet<String>,
    ) -> TyResult<()> {
        match &ty.kind {
            TyKind::Int(_) | TyKind::Bool | TyKind::Float(_) | TyKind::Str => Ok(()),
            TyKind::Ref(_, inner) if matches!(inner.kind, TyKind::Str) => Ok(()),
            TyKind::Adt { name, .. } => self.ensure_struct_printable(name, context, visiting),
            _ => Err(TypeckError::Other(format!(
                "print does not support field `{}` of type {}",
                context, ty.kind
            ))),
        }
    }

    fn ensure_struct_printable(
        &mut self,
        struct_name: &str,
        context: &str,
        visiting: &mut HashSet<String>,
    ) -> TyResult<()> {
        if !visiting.insert(struct_name.to_string()) {
            return Ok(());
        }

        let fields = self.resolve_struct_field_types(struct_name)?;
        for (field_name, field_ty) in fields {
            let field_context = format!("{}.{}", context, field_name);
            self.ensure_type_printable_for_print(&field_ty, &field_context, visiting)?;
        }

        visiting.remove(struct_name);
        Ok(())
    }

    pub(super) fn check_call(&mut self, func: &Expr, args: &[Expr]) -> TyResult<Ty> {
        let builtin_name = match &func.kind {
            ExprKind::Ident(ident) => Some(ident.name.as_str()),
            ExprKind::Path(path) if path.segments.len() == 1 => {
                Some(path.segments[0].name.as_str())
            }
            _ => None,
        };

        if builtin_name == Some("spawn") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "spawn is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let future_ty = self.check_expr(&args[0])?;
            if !future_ty.is_future() {
                return Err(TypeckError::Other(
                    "spawn requires a Future value".to_string(),
                ));
            }

            return Ok(future_ty);
        }

        if builtin_name == Some("spawn_task") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "spawn_task is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let future_ty = self.check_expr(&args[0])?;
            if !future_ty.is_future() {
                return Err(TypeckError::Other(
                    "spawn_task requires a Future value".to_string(),
                ));
            }

            return Ok(self.env.int_ty(IntKind::I64));
        }

        if builtin_name == Some("sleep") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "sleep is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let duration_ty = self.check_expr(&args[0])?;
            let i64_ty = self.env.int_ty(IntKind::I64);
            self.infer.unify(&duration_ty, &i64_ty)?;
            return Ok(Ty::new(0, TyKind::Future(Box::new(self.env.unit_ty()))));
        }

        if builtin_name == Some("timeout") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "timeout is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 2 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 2,
                    found: args.len(),
                });
            }

            let future_ty = self.check_expr(&args[0])?;
            if !future_ty.is_future() {
                return Err(TypeckError::Other(
                    "timeout requires a Future value".to_string(),
                ));
            }

            let duration_ty = self.check_expr(&args[1])?;
            let i64_ty = self.env.int_ty(IntKind::I64);
            self.infer.unify(&duration_ty, &i64_ty)?;
            return Ok(Ty::new(0, TyKind::Future(Box::new(self.env.bool_ty()))));
        }

        if builtin_name == Some("join") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "join is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 2 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 2,
                    found: args.len(),
                });
            }

            for arg in args {
                let future_ty = self.check_expr(arg)?;
                if !future_ty.is_future() {
                    return Err(TypeckError::Other(
                        "join requires Future values".to_string(),
                    ));
                }
            }

            return Ok(self.env.unit_ty());
        }

        if builtin_name == Some("cancel_task") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "cancel_task is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let task_ty = self.check_expr(&args[0])?;
            let i64_ty = self.env.int_ty(IntKind::I64);
            self.infer.unify(&task_ty, &i64_ty)?;
            return Ok(self.env.bool_ty());
        }

        if builtin_name == Some("task_status") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "task_status is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let task_ty = self.check_expr(&args[0])?;
            let i64_ty = self.env.int_ty(IntKind::I64);
            self.infer.unify(&task_ty, &i64_ty)?;
            return Ok(i64_ty);
        }

        if builtin_name == Some("select") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "select is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 2 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 2,
                    found: args.len(),
                });
            }

            let left_future = self.check_expr(&args[0])?;
            let right_future = self.check_expr(&args[1])?;
            let TyKind::Future(left_inner) = &left_future.kind else {
                return Err(TypeckError::Other(
                    "select requires Future values".to_string(),
                ));
            };
            let TyKind::Future(right_inner) = &right_future.kind else {
                return Err(TypeckError::Other(
                    "select requires Future values".to_string(),
                ));
            };

            self.infer.unify(left_inner, right_inner)?;
            return Ok(self.infer.apply_subst(left_inner));
        }

        // Special handling for `print` builtin function
        // Check both Ident and Path (single-segment) since the parser may produce either
        let is_print = match &func.kind {
            ExprKind::Ident(ident) => ident.name == "print",
            ExprKind::Path(path) => path.segments.len() == 1 && path.segments[0].name == "print",
            _ => false,
        };
        if is_print {
            // print expects exactly one argument
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }

            let arg_ty = self.check_expr(&args[0])?;
            let mut visiting = HashSet::new();
            let context = match &arg_ty.kind {
                TyKind::Adt { name, .. } => name.clone(),
                _ => "print argument".to_string(),
            };
            self.ensure_type_printable_for_print(&arg_ty, &context, &mut visiting)?;

            // print returns unit
            return Ok(self.env.unit_ty());
        }

        let direct_fn_name = match &func.kind {
            ExprKind::Ident(ident) => Some(ident.name.clone()),
            ExprKind::Path(path) if path.segments.len() == 1 => Some(path.segments[0].name.clone()),
            _ => None,
        };

        let mut generic_ctx: Option<(String, GenericFunctionMeta, HashMap<TyVarId, TyVarId>)> =
            None;
        let func_ty = if let Some(name) = direct_fn_name {
            match self.env.lookup(&name).cloned() {
                Some(Symbol {
                    kind: SymbolKind::Function { ty, .. },
                    ..
                }) => {
                    if let Some(meta) = self.generic_function_metas.get(&name).cloned() {
                        let (instantiated, var_map) =
                            self.infer.instantiate_with_fresh_vars_and_map(&ty);
                        generic_ctx = Some((name, meta, var_map));
                        instantiated
                    } else {
                        self.infer.instantiate_with_fresh_vars(&ty)
                    }
                }
                _ => self.check_expr(func)?,
            }
        } else {
            self.check_expr(func)?
        };

        if let TyKind::Fn { params, ret, .. } = &func_ty.kind {
            if params.len() != args.len() {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: params.len(),
                    found: args.len(),
                });
            }

            for (arg_ty, arg_expr) in params.iter().zip(args.iter()) {
                let actual_ty = self.check_expr(arg_expr)?;
                // Passing an unawaited Future as a function argument is an escape.
                // The caller must `await` it at the call-site first.
                if self.contains_future_escape_ty(&actual_ty) {
                    return Err(TypeckError::Other(
                        "future values cannot be passed as arguments; await the async call first"
                            .to_string(),
                    ));
                }
                self.infer.unify(arg_ty, &actual_ty)?;
            }

            if let Some((name, meta, var_map)) = generic_ctx.as_ref() {
                self.enforce_generic_function_constraints(name, meta, var_map)?;
            }

            let resolved_ret = self.infer.apply_subst(ret);

            let is_async_call = match &func.kind {
                ExprKind::Ident(ident) => self.async_functions.contains(&ident.name),
                ExprKind::Path(path) if path.segments.len() == 1 => {
                    self.async_functions.contains(&path.segments[0].name)
                }
                _ => false,
            };
            if is_async_call {
                Ok(Ty::new(0, TyKind::Future(Box::new(resolved_ret))))
            } else {
                Ok(resolved_ret)
            }
        } else {
            Err(TypeckError::UndefinedFunction {
                name: "closure".to_string(),
            })
        }
    }

    fn enforce_generic_function_constraints(
        &mut self,
        function_name: &str,
        meta: &GenericFunctionMeta,
        var_map: &HashMap<TyVarId, TyVarId>,
    ) -> TyResult<()> {
        for param in &meta.params {
            let mut concrete_ty = if let Some(instantiated_var) = var_map.get(&param.var_id) {
                let placeholder = Ty::new(0, TyKind::Var(*instantiated_var));
                self.infer.apply_subst(&placeholder)
            } else if let Some(default_ty) = &param.default {
                // Generic parameter is not present in function type (phantom generic).
                // In this case, default type is the only inference source.
                self.infer.apply_subst(default_ty)
            } else if param.bounds.is_empty() {
                // Unused unconstrained generic parameter does not affect call typing.
                // Keep backward compatibility for benchmark and existing code.
                continue;
            } else {
                return Err(TypeckError::Other(format!(
                    "cannot infer generic type parameter `{}` in call to `{}`",
                    param.name, function_name
                )));
            };

            if matches!(concrete_ty.kind, TyKind::Var(_)) {
                if let Some(default_ty) = &param.default {
                    let default_ty = self.infer.apply_subst(default_ty);
                    self.infer.unify(&concrete_ty, &default_ty)?;
                    concrete_ty = self.infer.apply_subst(&default_ty);
                }
            }

            if matches!(concrete_ty.kind, TyKind::Var(_)) {
                return Err(TypeckError::Other(format!(
                    "cannot infer generic type parameter `{}` in call to `{}`",
                    param.name, function_name
                )));
            }

            for trait_name in &param.bounds {
                let concrete_key = type_key(&concrete_ty);
                if !self
                    .impl_registry
                    .implements_trait(trait_name, &concrete_key)
                {
                    return Err(TypeckError::Other(format!(
                        "generic constraint violated in `{}`: `{}` does not implement `{}` for `{}`",
                        function_name, concrete_key, trait_name, param.name
                    )));
                }
            }
        }
        Ok(())
    }

    /// Type check a method call by resolving candidates against the receiver type.
    pub(super) fn check_method_call(
        &mut self,
        receiver: &Expr,
        method: &Ident,
        args: &[Expr],
    ) -> TyResult<Ty> {
        use crate::typeck::r#trait::type_key;

        let receiver_ty = self.check_expr(receiver)?;
        let receiver_key = type_key(&receiver_ty);

        let mut arg_types = Vec::new();
        for arg in args {
            arg_types.push(self.check_expr(arg)?);
        }

        let method_name = &method.name;

        // Built-in string method: (&str).len() -> i64
        let is_str_ref =
            matches!(&receiver_ty.kind, TyKind::Ref(_, inner) if matches!(inner.kind, TyKind::Str));
        if is_str_ref && method_name == "len" {
            if !args.is_empty() {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 0,
                    found: args.len(),
                });
            }
            return Ok(self.env.int_ty(crate::typeck::ty::IntKind::I64));
        }

        // Inherent impl lookup first.
        let exact_inherent = self
            .impl_registry
            .lookup_inherent_method(&receiver_key, method_name)
            .cloned();
        if let Some(fn_ty) = exact_inherent
            .map(|fn_ty| self.instantiate_method_function_ty(&fn_ty, &HashMap::new()))
            .or_else(|| self.lookup_generic_inherent_method(&receiver_ty, method_name))
        {
            if fn_ty.param_types.len() != args.len() {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: fn_ty.param_types.len(),
                    found: args.len(),
                });
            }

            for (expected, actual) in fn_ty.param_types.iter().zip(arg_types.iter()) {
                self.infer.unify(expected, actual)?;
            }

            return Ok(self.infer.apply_subst(&fn_ty.return_type));
        }

        // Then trait impl lookup.
        if let Some(fn_ty) =
            self.select_trait_method_call_candidate(&receiver_key, method_name, args.len())?
        {
            for (expected, actual) in fn_ty.param_types.iter().zip(arg_types.iter()) {
                self.infer.unify(expected, actual)?;
            }
            return Ok(self.infer.apply_subst(&fn_ty.return_type));
        }

        Err(TypeckError::MethodNotFound {
            type_name: receiver_key,
            method_name: method_name.clone(),
        })
    }

    fn select_trait_method_call_candidate(
        &mut self,
        receiver_key: &str,
        method_name: &str,
        arg_count: usize,
    ) -> TyResult<Option<FunctionTy>> {
        let mut candidates = Vec::new();
        for trait_name in self.trait_registry.all_traits() {
            if let Some(fn_ty) = self
                .impl_registry
                .lookup_trait_method(&trait_name, receiver_key, method_name)
                .cloned()
            {
                let instantiated = self.instantiate_method_function_ty(&fn_ty, &HashMap::new());
                candidates.push(MethodCandidate {
                    label: trait_name,
                    param_count: instantiated.param_types.len(),
                    value: instantiated,
                });
            }
        }

        match select_method_candidate(candidates, arg_count) {
            MethodCandidateMatch::None => Ok(None),
            MethodCandidateMatch::WrongArity { expected } => {
                Err(TypeckError::ArgumentCountMismatch {
                    expected,
                    found: arg_count,
                })
            }
            MethodCandidateMatch::One(fn_ty) => Ok(Some(fn_ty)),
            MethodCandidateMatch::Ambiguous { labels } => Err(TypeckError::Other(
                ambiguous_method_error(method_name, receiver_key, &labels),
            )),
        }
    }
}
