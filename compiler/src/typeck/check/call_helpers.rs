use super::*;
use crate::ast::ExprKind;
use std::collections::HashSet;

const ASSERT_HELPERS: &[&str] = &[
    "assert",
    "assert_true",
    "assert_false",
    "assert_eq_i64",
    "assert_ne_i64",
    "assert_eq_bool",
    "assert_ne_bool",
    "assert_eq_str",
    "assert_ne_str",
    "assert_eq_f64",
    "assert_ne_f64",
];

fn is_assert_helper(name: &str) -> bool {
    ASSERT_HELPERS.contains(&name)
}

fn assert_helper_allows_callsite_injection(
    name: Option<&str>,
    param_count: usize,
    arg_count: usize,
) -> bool {
    name.is_some_and(is_assert_helper) && arg_count + 2 == param_count
}

impl TypeChecker {
    fn instantiate_method_function_ty(
        &mut self,
        fn_ty: &FunctionTy,
        subst: &HashMap<TyVarId, Ty>,
    ) -> FunctionTy {
        let mut call_subst = subst.clone();
        for generic_param in &fn_ty.generic_params {
            call_subst.insert(*generic_param, self.infer.fresh_ty_var());
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

    fn is_spawn_blocking_work_call(name: Option<&str>, path: Option<&[String]>) -> bool {
        if matches!(
            name,
            Some("spawn_blocking_i64") | Some("spawn_blocking_future_i64")
        ) {
            return true;
        }
        path.is_some_and(|segments| {
            segments.len() == 2
                && segments[0] == "async"
                && matches!(
                    segments[1].as_str(),
                    "spawn_blocking_i64" | "spawn_blocking_future_i64"
                )
        })
    }

    fn runtime_async_wrapper_future_ty(&mut self, func_name: &str) -> Option<Ty> {
        match func_name {
            "spawn_blocking_future_i64" => Some(Ty::new(
                0,
                TyKind::Future(Box::new(self.env.int_ty(IntKind::I64))),
            )),
            "channel_send_i64" => Some(Ty::new(
                0,
                TyKind::Future(Box::new(self.env.new_ty(TyKind::Adt {
                    name: "ChannelSendOutcome".to_string(),
                    args: vec![],
                }))),
            )),
            "channel_recv_i64" => Some(Ty::new(
                0,
                TyKind::Future(Box::new(self.env.new_ty(TyKind::Adt {
                    name: "ChannelRecvOutcome".to_string(),
                    args: vec![],
                }))),
            )),
            "mutex_lock_async" => Some(Ty::new(
                0,
                TyKind::Future(Box::new(self.env.new_ty(TyKind::Adt {
                    name: "MutexLockOutcome".to_string(),
                    args: vec![],
                }))),
            )),
            "HttpServer_next_request_async" => Some(Ty::new(
                0,
                TyKind::Future(Box::new(self.env.new_ty(TyKind::Adt {
                    name: "HttpServerNextRequestOutcome".to_string(),
                    args: vec![],
                }))),
            )),
            _ => None,
        }
    }

    fn collect_lambda_capture_names(params: &[Ident], body: &Expr) -> Vec<String> {
        let param_names = params
            .iter()
            .map(|param| param.name.clone())
            .collect::<HashSet<_>>();
        let mut captures = Vec::new();
        let mut seen = HashSet::new();
        Self::collect_capture_idents(body, &param_names, &mut captures, &mut seen);
        captures
    }

    fn collect_capture_from_block(
        block: &crate::ast::Block,
        params: &HashSet<String>,
        captures: &mut Vec<String>,
        seen: &mut HashSet<String>,
    ) {
        for stmt in &block.stmts {
            if let crate::ast::StmtKind::Expr(expr) = &stmt.kind {
                Self::collect_capture_idents(expr, params, captures, seen);
            }
        }
    }

    fn collect_capture_idents(
        expr: &Expr,
        params: &HashSet<String>,
        captures: &mut Vec<String>,
        seen: &mut HashSet<String>,
    ) {
        match &expr.kind {
            ExprKind::Ident(ident) if !params.contains(&ident.name) => {
                if seen.insert(ident.name.clone()) {
                    captures.push(ident.name.clone());
                }
            }
            ExprKind::Path(path) if path.segments.len() == 1 => {
                let name = path.segments[0].name.clone();
                if !params.contains(&name) && seen.insert(name.clone()) {
                    captures.push(name);
                }
            }
            ExprKind::Lambda {
                params: lambda_params,
                body,
            } => {
                let mut nested_params = params.clone();
                for param in lambda_params {
                    nested_params.insert(param.name.clone());
                }
                Self::collect_capture_idents(body, &nested_params, captures, seen);
            }
            ExprKind::Binary { left, right, .. } => {
                Self::collect_capture_idents(left, params, captures, seen);
                Self::collect_capture_idents(right, params, captures, seen);
            }
            ExprKind::Unary { operand, .. } => {
                Self::collect_capture_idents(operand, params, captures, seen);
            }
            ExprKind::Call { func, args, .. } => {
                Self::collect_capture_idents(func, params, captures, seen);
                for arg in args {
                    Self::collect_capture_idents(arg, params, captures, seen);
                }
            }
            ExprKind::Field { base, .. } => {
                Self::collect_capture_idents(base, params, captures, seen);
            }
            ExprKind::Index { base, index, .. } => {
                Self::collect_capture_idents(base, params, captures, seen);
                Self::collect_capture_idents(index, params, captures, seen);
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                Self::collect_capture_idents(cond, params, captures, seen);
                Self::collect_capture_from_block(then_branch, params, captures, seen);
                if let Some(else_branch) = else_branch {
                    Self::collect_capture_idents(else_branch, params, captures, seen);
                }
            }
            ExprKind::Block(block) => {
                Self::collect_capture_from_block(block, params, captures, seen);
            }
            ExprKind::While { cond, body } => {
                Self::collect_capture_idents(cond, params, captures, seen);
                Self::collect_capture_from_block(body, params, captures, seen);
            }
            ExprKind::For { iter, body, .. } => {
                Self::collect_capture_idents(iter, params, captures, seen);
                Self::collect_capture_from_block(body, params, captures, seen);
            }
            ExprKind::Loop(body)
            | ExprKind::AsyncBlock(body)
            | ExprKind::ParallelBlock(body)
            | ExprKind::TryBlock(body) => {
                Self::collect_capture_from_block(body, params, captures, seen);
            }
            ExprKind::Match { scrutinee, arms } => {
                Self::collect_capture_idents(scrutinee, params, captures, seen);
                for arm in arms {
                    Self::collect_capture_idents(&arm.body, params, captures, seen);
                }
            }
            ExprKind::Tuple(items) | ExprKind::Array(items) => {
                for item in items {
                    Self::collect_capture_idents(item, params, captures, seen);
                }
            }
            ExprKind::Struct { fields, base, .. } => {
                for field in fields {
                    Self::collect_capture_idents(&field.value, params, captures, seen);
                }
                if let Some(base) = base {
                    Self::collect_capture_idents(base, params, captures, seen);
                }
            }
            ExprKind::Await(inner) => {
                Self::collect_capture_idents(inner, params, captures, seen);
            }
            _ => {}
        }
    }

    fn check_spawn_blocking_i64_send_captures(&mut self, args: &[Expr]) -> TyResult<()> {
        if args.len() != 1 {
            return Ok(());
        }
        let arg = &args[0];
        if let ExprKind::Lambda { params, body } = &arg.kind {
            let captures = Self::collect_lambda_capture_names(params, body);
            for capture in captures {
                let Some(symbol) = self.env.lookup(&capture) else {
                    continue;
                };
                let Some(ty) = symbol.get_ty() else {
                    continue;
                };
                if !self.is_cross_thread_send_ty(ty) {
                    return Err(Self::cross_thread_send_error(&capture));
                }
            }
            return Ok(());
        }

        let arg_ty = self.check_expr(arg)?;
        if !self.is_cross_thread_send_ty(&arg_ty) {
            return Err(Self::cross_thread_send_error("argument"));
        }
        Ok(())
    }

    pub(super) fn check_call(&mut self, func: &Expr, args: &[Expr]) -> TyResult<Ty> {
        if let ExprKind::Path(path) = &func.kind {
            if let Some(result) = self.check_enum_variant_constructor(path, args) {
                return result;
            }
        }

        let builtin_name = match &func.kind {
            ExprKind::Ident(ident) => Some(ident.name.as_str()),
            ExprKind::Path(path) if path.segments.len() == 1 => {
                Some(path.segments[0].name.as_str())
            }
            _ => None,
        };
        let path_segments = match &func.kind {
            ExprKind::Path(path) => Some(
                path.segments
                    .iter()
                    .map(|segment| segment.name.clone())
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        };

        if Self::is_spawn_blocking_work_call(builtin_name, path_segments.as_deref()) {
            self.check_spawn_blocking_i64_send_captures(args)?;
        }

        if builtin_name == Some("sengoo_http_server_next_request_async__start") {
            if args.len() != 2 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 2,
                    found: args.len(),
                });
            }
            let i64_ty = self.env.int_ty(IntKind::I64);
            for arg in args {
                let arg_ty = self.check_expr(arg)?;
                self.infer.unify(&arg_ty, &i64_ty)?;
            }
            let outcome_ty = self.env.new_ty(TyKind::Adt {
                name: "HttpServerNextRequestOutcome".to_string(),
                args: vec![],
            });
            return Ok(Ty::new(0, TyKind::Future(Box::new(outcome_ty))));
        }

        if let Some(name) = builtin_name {
            if let Some(future_ty) = self.runtime_async_wrapper_future_ty(name) {
                if self.async_context_depth == 0 {
                    return Err(TypeckError::Other(format!(
                        "{name} is only allowed in async contexts"
                    )));
                }
                match name {
                    "spawn_blocking_future_i64" => {
                        if args.len() != 1 {
                            return Err(TypeckError::ArgumentCountMismatch {
                                expected: 1,
                                found: args.len(),
                            });
                        }
                        self.check_expr(&args[0])?;
                    }
                    "channel_send_i64" => {
                        if args.len() != 2 {
                            return Err(TypeckError::ArgumentCountMismatch {
                                expected: 2,
                                found: args.len(),
                            });
                        }
                        self.check_expr(&args[0])?;
                        let value_ty = self.check_expr(&args[1])?;
                        if !self.is_cross_thread_send_ty(&value_ty) {
                            return Err(Self::cross_thread_send_error("value"));
                        }
                    }
                    "channel_recv_i64" | "mutex_lock_async" | "HttpServer_next_request_async" => {
                        if args.len() != 1 {
                            return Err(TypeckError::ArgumentCountMismatch {
                                expected: 1,
                                found: args.len(),
                            });
                        }
                        self.check_expr(&args[0])?;
                    }
                    _ => {}
                }
                return Ok(future_ty);
            }
        }

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

        if builtin_name == Some("timeout_cancel") {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "timeout_cancel is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 2 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 2,
                    found: args.len(),
                });
            }

            let future_ty = self.check_expr(&args[0])?;
            let TyKind::Future(inner_ty) = &future_ty.kind else {
                return Err(TypeckError::Other(
                    "timeout_cancel requires a Future value".to_string(),
                ));
            };

            let duration_ty = self.check_expr(&args[1])?;
            let i64_ty = self.env.int_ty(IntKind::I64);
            self.infer.unify(&duration_ty, &i64_ty)?;

            let result_ty = Ty::new(
                0,
                TyKind::Adt {
                    name: "Result".to_string(),
                    args: vec![inner_ty.as_ref().clone(), i64_ty],
                },
            );
            return Ok(Ty::new(0, TyKind::Future(Box::new(result_ty))));
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
            if !(2..=8).contains(&args.len()) {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 2,
                    found: args.len(),
                });
            }

            let mut inner_ty = None;
            for arg in args {
                let future_ty = self.check_expr(arg)?;
                let TyKind::Future(current_inner) = &future_ty.kind else {
                    return Err(TypeckError::Other(
                        "select requires Future values".to_string(),
                    ));
                };
                if let Some(expected) = &inner_ty {
                    self.infer.unify(expected, current_inner)?;
                } else {
                    inner_ty = Some(current_inner.as_ref().clone());
                }
            }

            return Ok(self
                .infer
                .apply_subst(&inner_ty.expect("select has at least two operands")));
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
        let func_ty = if let Some(ref name) = direct_fn_name {
            self.warn_deprecated_use(name, func.span());
            match self.env.lookup(name).cloned() {
                Some(Symbol {
                    kind: SymbolKind::Function { ty, .. },
                    ..
                }) => {
                    if let Some(meta) = self.generic_function_metas.get(name).cloned() {
                        let (instantiated, var_map) =
                            self.infer.instantiate_with_fresh_vars_and_map(&ty);
                        generic_ctx = Some((name.clone(), meta, var_map));
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
            let direct_name = direct_fn_name.as_deref();
            if params.len() != args.len()
                && !assert_helper_allows_callsite_injection(direct_name, params.len(), args.len())
            {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: params.len(),
                    found: args.len(),
                });
            }

            for (arg_ty, arg_expr) in params.iter().take(args.len()).zip(args.iter()) {
                let actual_ty = self.check_expr(arg_expr)?;
                // Passing an unawaited Future as a function argument is an escape.
                // The caller must `await` it at the call-site first.
                if self.contains_future_escape_ty(&actual_ty) {
                    return Err(TypeckError::Other(
                        "future values cannot be passed as arguments; await the async call first"
                            .to_string(),
                    ));
                }
                if matches!(arg_ty.kind, TyKind::Int(IntKind::I64))
                    && matches!(actual_ty.kind, TyKind::Fn { .. })
                {
                    continue;
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

        if matches!(&receiver_ty.kind, TyKind::Adt { name, .. } if name == "HttpServer")
            && method_name == "next_request_async"
        {
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(
                    "HttpServer.next_request_async is only allowed in async contexts".to_string(),
                ));
            }
            if args.len() != 1 {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 1,
                    found: args.len(),
                });
            }
            let timeout_ty = arg_types
                .first()
                .cloned()
                .unwrap_or_else(|| self.env.int_ty(IntKind::I64));
            let i64_ty = self.env.int_ty(IntKind::I64);
            self.infer.unify(&timeout_ty, &i64_ty)?;
            let outcome_ty = self.env.new_ty(TyKind::Adt {
                name: "HttpServerNextRequestOutcome".to_string(),
                args: vec![],
            });
            return Ok(Ty::new(0, TyKind::Future(Box::new(outcome_ty))));
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
