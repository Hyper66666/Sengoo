use super::*;
use crate::ast::ExprKind;
use crate::typeck::r#trait::type_key;
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
    fn resolve_associated_projection(&mut self, ty: &Ty) -> TyResult<Ty> {
        let kind = match &ty.kind {
            TyKind::AssocProjection {
                base,
                trait_name,
                name,
            } => {
                let base = self.infer.apply_subst(base);
                let base_key = type_key(&base);
                if matches!(base.kind, TyKind::Var(_) | TyKind::Inferred)
                    || matches!(&base.kind, TyKind::Adt { name, .. } if name == "<unknown>")
                    || base_key.contains("<unknown>")
                {
                    TyKind::AssocProjection {
                        base: Box::new(base),
                        trait_name: trait_name.clone(),
                        name: name.clone(),
                    }
                } else {
                    let key = base_key;
                    if let Some(resolved) = self
                        .impl_registry
                        .get_trait_impl(trait_name, &key)
                        .and_then(|impl_info| impl_info.assoc_types.get(name))
                        .cloned()
                    {
                        return if type_key(&resolved) == type_key(ty) {
                            Ok(resolved)
                        } else {
                            self.resolve_associated_projection(&resolved)
                        };
                    }

                    let generic_key = self.generic_lookup_key(&base);
                    let candidates = self
                        .impl_registry
                        .get_trait_impls(trait_name, &generic_key)
                        .to_vec();
                    for impl_info in candidates {
                        let mut subst = HashMap::new();
                        if self.match_generic_impl_target(&impl_info.target_type, &base, &mut subst)
                        {
                            if let Some(resolved) = impl_info.assoc_types.get(name) {
                                let resolved = self.substitute_ty_vars(resolved, &subst);
                                return if type_key(&resolved) == type_key(ty) {
                                    Ok(resolved)
                                } else {
                                    self.resolve_associated_projection(&resolved)
                                };
                            }
                        }
                    }
                    return Err(TypeckError::Other(format!(
                        "cannot resolve associated type `<{key} as {trait_name}>::{name}` from visible trait impls"
                    )));
                }
            }
            TyKind::Tuple(items) => TyKind::Tuple(
                items
                    .iter()
                    .map(|item| self.resolve_associated_projection(item))
                    .collect::<TyResult<Vec<_>>>()?,
            ),
            TyKind::Array(item, len) => {
                TyKind::Array(Box::new(self.resolve_associated_projection(item)?), *len)
            }
            TyKind::Slice(item) => {
                TyKind::Slice(Box::new(self.resolve_associated_projection(item)?))
            }
            TyKind::Ref(is_mut, item) => {
                TyKind::Ref(*is_mut, Box::new(self.resolve_associated_projection(item)?))
            }
            TyKind::Ptr(item) => TyKind::Ptr(Box::new(self.resolve_associated_projection(item)?)),
            TyKind::Fn {
                params,
                ret,
                is_variadic,
            } => TyKind::Fn {
                params: params
                    .iter()
                    .map(|param| self.resolve_associated_projection(param))
                    .collect::<TyResult<Vec<_>>>()?,
                ret: Box::new(self.resolve_associated_projection(ret)?),
                is_variadic: *is_variadic,
            },
            TyKind::Adt { name, args } => TyKind::Adt {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| self.resolve_associated_projection(arg))
                    .collect::<TyResult<Vec<_>>>()?,
            },
            TyKind::Future(item) => {
                TyKind::Future(Box::new(self.resolve_associated_projection(item)?))
            }
            _ => return Ok(ty.clone()),
        };
        Ok(Ty::new(ty.id, kind))
    }

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
        let receiver_ty = match &receiver_ty.kind {
            TyKind::Ref(_, inner) => inner.as_ref(),
            _ => receiver_ty,
        };
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
            TyKind::Adt { name, .. } => {
                // The owned `String` prints its own text directly.
                if name == "String" {
                    return Ok(());
                }
                if self.enum_variants.contains_key(name) {
                    return Ok(());
                }
                // Any type with a user `Display` impl prints through that impl
                // rather than requiring every field to be structurally printable.
                if self.type_implements_display(ty) {
                    return Ok(());
                }
                self.ensure_struct_printable(name, context, visiting)
            }
            _ => {
                if self.type_implements_display(ty) {
                    return Ok(());
                }
                Err(TypeckError::Other(format!(
                    "print does not support field `{}` of type {}",
                    context, ty.kind
                )))
            }
        }
    }

    /// Whether `ty` has a user-provided `impl Display`, which lets `print`
    /// dispatch through that impl instead of the built-in structural printer.
    fn type_implements_display(&self, ty: &Ty) -> bool {
        self.impl_registry
            .implements_trait("Display", &type_key(ty))
    }

    /// Type-check a `format(template, args...)` call: the template must be a
    /// string literal, its `{}` placeholders must match the argument count, and
    /// every argument must be renderable (built-in printable or `impl Display`).
    /// Returns the owned `String` type the call produces.
    fn check_format_call(&mut self, args: &[Expr]) -> TyResult<Ty> {
        let Some((template_arg, value_args)) = args.split_first() else {
            return Err(TypeckError::diagnostic(
                "invalid-format-template",
                "format requires a string literal template",
                0,
                0,
            ));
        };
        let ExprKind::Literal(crate::ast::Literal::String(template)) = &template_arg.kind else {
            return Err(TypeckError::diagnostic(
                "invalid-format-template",
                "format template must be a string literal",
                template_arg.span.lo,
                template_arg.span.hi,
            ));
        };
        let segments = crate::format_template::parse_format_template(template).map_err(|err| {
            TypeckError::diagnostic(
                "invalid-format-template",
                err.message(),
                template_arg.span.lo,
                template_arg.span.hi,
            )
        })?;
        let expected = crate::format_template::required_arg_count(&segments);
        if expected != value_args.len() {
            return Err(TypeckError::diagnostic(
                "format-argument-count",
                format!(
                    "format template requires {expected} value argument(s), found {}",
                    value_args.len()
                ),
                template_arg.span.lo,
                template_arg.span.hi,
            ));
        }
        for arg in value_args {
            let arg_ty = self.check_expr(arg)?;
            let mut visiting = HashSet::new();
            let context = match &arg_ty.kind {
                TyKind::Adt { name, .. } => name.clone(),
                _ => "format argument".to_string(),
            };
            self.ensure_type_printable_for_print(&arg_ty, &context, &mut visiting)?;
        }
        Ok(self.env.new_ty(TyKind::Adt {
            name: "String".to_string(),
            args: Vec::new(),
        }))
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

    fn is_shared_state_spawn_call(name: Option<&str>, path: Option<&[String]>) -> bool {
        if name == Some("spawn_shared_counter_i64") {
            return true;
        }
        path.is_some_and(|segments| {
            segments.len() == 2
                && segments[0] == "async"
                && segments[1] == "spawn_shared_counter_i64"
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

    fn check_shared_state_send_argument(&mut self, args: &[Expr]) -> TyResult<()> {
        let Some(shared) = args.first() else {
            return Ok(());
        };
        let shared_ty = self.check_expr(shared)?;
        if !self.is_cross_thread_send_ty(&shared_ty) {
            return Err(TypeckError::Other(
                "cross-thread shared state argument is not Send".to_string(),
            ));
        }
        if !self.type_satisfies_auto_marker_bound("Sync", &shared_ty) {
            return Err(TypeckError::Other(
                "cross-thread shared state argument is not Sync".to_string(),
            ));
        }
        Ok(())
    }

    pub(super) fn check_call(
        &mut self,
        func: &Expr,
        args: &[Expr],
        call_span: crate::lexer::Span,
    ) -> TyResult<Ty> {
        self.check_call_with_expected(func, args, call_span, None)
    }

    pub(super) fn check_call_with_expected(
        &mut self,
        func: &Expr,
        args: &[Expr],
        call_span: crate::lexer::Span,
        expected_return: Option<&Ty>,
    ) -> TyResult<Ty> {
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
        if Self::is_shared_state_spawn_call(builtin_name, path_segments.as_deref()) {
            self.check_shared_state_send_argument(args)?;
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

        if builtin_name == Some("select") || builtin_name == Some("select_cancel") {
            let builtin = builtin_name.expect("checked select builtin name");
            if self.async_context_depth == 0 {
                return Err(TypeckError::Other(format!(
                    "{builtin} is only allowed in async contexts"
                )));
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
                    return Err(TypeckError::Other(format!(
                        "{builtin} requires Future values"
                    )));
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

        // Special handling for the `format` builtin: a compile-time template
        // literal followed by the positional arguments it renders.
        let is_format = match &func.kind {
            ExprKind::Ident(ident) => ident.name == "format",
            ExprKind::Path(path) => path.segments.len() == 1 && path.segments[0].name == "format",
            _ => false,
        };
        if is_format {
            return self.check_format_call(args);
        }

        // Special handling for `print`/`println`/`eprintln` builtin functions
        // Check both Ident and Path (single-segment) since the parser may produce either
        let is_print = match &func.kind {
            ExprKind::Ident(ident) => {
                matches!(ident.name.as_str(), "print" | "println" | "eprintln")
            }
            ExprKind::Path(path) => {
                path.segments.len() == 1
                    && matches!(
                        path.segments[0].name.as_str(),
                        "print" | "println" | "eprintln"
                    )
            }
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

        if let ExprKind::Path(path) = &func.kind {
            if let Some(result) = self.check_associated_function(path, args, call_span.lo)? {
                return Ok(result);
            }
        }

        let direct_fn_name = Self::direct_callable_name(func);

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
                let expected_ty =
                    self.resolve_associated_projection(&self.infer.apply_subst(arg_ty))?;
                let actual_ty = self.check_expr_with_expected(arg_expr, &expected_ty)?;
                // Passing an unawaited Future as a function argument is an escape.
                // The caller must `await` it at the call-site first.
                if self.contains_future_escape_ty(&actual_ty) {
                    return Err(TypeckError::Other(
                        "future values cannot be passed as arguments; await the async call first"
                            .to_string(),
                    ));
                }
                if matches!(expected_ty.kind, TyKind::Int(IntKind::I64))
                    && matches!(actual_ty.kind, TyKind::Fn { .. })
                {
                    continue;
                }
                if self.is_dyn_unsize_coercion(&expected_ty, &actual_ty) {
                    continue;
                }
                self.infer.unify(&expected_ty, &actual_ty)?;
            }

            let is_async_call = direct_fn_name
                .as_ref()
                .is_some_and(|name| self.async_functions.contains(name));
            if let Some(expected) = expected_return.filter(|_| generic_ctx.is_some()) {
                let return_ty = self.resolve_associated_projection(&self.infer.apply_subst(ret))?;
                let call_result = if is_async_call {
                    Ty::new(0, TyKind::Future(Box::new(return_ty)))
                } else {
                    return_ty
                };
                self.infer.unify(&call_result, expected)?;
            }

            if let Some((name, meta, var_map)) = generic_ctx.as_ref() {
                let span = func.span();
                self.enforce_generic_function_constraints(name, meta, var_map, span.lo, span.hi)?;
            }

            let resolved_ret = self.resolve_associated_projection(&self.infer.apply_subst(ret))?;
            if generic_ctx.is_some() {
                self.env
                    .record_call_return_type(call_span.lo, resolved_ret.clone());
            }

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

    fn direct_callable_name(func: &Expr) -> Option<String> {
        match &func.kind {
            ExprKind::Ident(ident) => Some(ident.name.clone()),
            ExprKind::Path(path) if !path.segments.is_empty() => Some(
                path.segments
                    .iter()
                    .map(|segment| segment.name.as_str())
                    .collect::<Vec<_>>()
                    .join("_"),
            ),
            _ => None,
        }
    }

    fn check_associated_function(
        &mut self,
        path: &crate::ast::Path,
        args: &[Expr],
        call_site: u32,
    ) -> TyResult<Option<Ty>> {
        if path.segments.len() != 2 {
            return Ok(None);
        }
        let type_name = &path.segments[0].name;
        let method_name = &path.segments[1].name;
        let target_ty = self
            .env
            .lookup(type_name)
            .and_then(|symbol| symbol.get_ty())
            .cloned()
            .unwrap_or_else(|| {
                self.env.new_ty(TyKind::Adt {
                    name: type_name.clone(),
                    args: Vec::new(),
                })
            });
        let target_key = type_key(&target_ty);

        if let Some(method_ty) = self
            .impl_registry
            .lookup_inherent_method(&target_key, method_name)
            .cloned()
        {
            if method_ty.has_self {
                return Ok(None);
            }
            if method_ty.param_types.len() != args.len() {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: method_ty.param_types.len(),
                    found: args.len(),
                });
            }
            for (expected, arg) in method_ty.param_types.iter().zip(args) {
                let actual = self.check_expr_with_expected(arg, expected)?;
                self.infer.unify(expected, &actual)?;
            }
            return Ok(Some(self.infer.apply_subst(&method_ty.return_type)));
        }

        let actual_types = args
            .iter()
            .map(|arg| self.check_expr(arg))
            .collect::<TyResult<Vec<_>>>()?;
        let mut candidates = Vec::new();
        for trait_name in self.trait_registry.all_traits() {
            for (impl_info, method_ty) in
                self.impl_registry
                    .lookup_trait_methods(&trait_name, &target_key, method_name)
            {
                if method_ty.has_self || method_ty.param_types.len() != actual_types.len() {
                    continue;
                }
                if method_ty.param_types.iter().zip(actual_types.iter()).all(
                    |(expected, actual)| {
                        type_key(&self.infer.apply_subst(expected))
                            == type_key(&self.infer.apply_subst(actual))
                    },
                ) {
                    candidates.push((trait_name.clone(), impl_info.clone(), method_ty.clone()));
                }
            }
        }

        let (trait_name, impl_info, method_ty) = match candidates.as_slice() {
            [] => return Ok(None),
            [candidate] => candidate.clone(),
            many => {
                let labels = many
                    .iter()
                    .map(|(trait_name, impl_info, _)| {
                        crate::typeck::r#trait::trait_impl_label(trait_name, &impl_info.trait_args)
                    })
                    .collect::<Vec<_>>();
                return Err(TypeckError::Other(ambiguous_method_error(
                    method_name,
                    &target_key,
                    &labels,
                )));
            }
        };
        for (expected, actual) in method_ty.param_types.iter().zip(actual_types.iter()) {
            self.infer.unify(expected, actual)?;
        }
        let trait_args = impl_info
            .trait_args
            .iter()
            .map(type_key)
            .collect::<Vec<_>>();
        let suffix = if trait_args.is_empty() {
            String::new()
        } else {
            format!("_{}", trait_args.join("_"))
        };
        self.env.record_associated_function(
            call_site,
            format!("{target_key}_{trait_name}{suffix}_{method_name}"),
        );
        Ok(Some(self.infer.apply_subst(&method_ty.return_type)))
    }

    fn enforce_generic_function_constraints(
        &mut self,
        function_name: &str,
        meta: &GenericFunctionMeta,
        var_map: &HashMap<TyVarId, TyVarId>,
        span_lo: u32,
        span_hi: u32,
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
                if function_name == "__sengoo_option_none" {
                    continue;
                }
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
                    && !self.type_satisfies_auto_marker_bound(trait_name, &concrete_ty)
                {
                    return Err(Self::unsatisfied_trait_bound_error(
                        function_name,
                        &concrete_key,
                        trait_name,
                        &param.name,
                        span_lo,
                        span_hi,
                    ));
                }
            }

            if !param.trait_bounds.is_empty() {
                let concrete_by_original_var = var_map
                    .iter()
                    .map(|(original, instantiated)| {
                        let placeholder = Ty::new(0, TyKind::Var(*instantiated));
                        (*original, self.infer.apply_subst(&placeholder))
                    })
                    .collect::<HashMap<_, _>>();
                for bound in &param.trait_bounds {
                    if bound.args.is_empty() {
                        continue;
                    }
                    let resolved_args = bound
                        .args
                        .iter()
                        .map(|arg| {
                            self.infer.apply_subst(
                                &self.substitute_ty_vars(arg, &concrete_by_original_var),
                            )
                        })
                        .collect::<Vec<_>>();
                    let concrete_key = type_key(&concrete_ty);
                    let has_exact_impl = self
                        .impl_registry
                        .get_trait_impls(&bound.trait_name, &concrete_key)
                        .iter()
                        .any(|impl_info| {
                            impl_info.trait_args.len() == resolved_args.len()
                                && impl_info.trait_args.iter().zip(resolved_args.iter()).all(
                                    |(actual, expected)| type_key(actual) == type_key(expected),
                                )
                        });
                    if !has_exact_impl {
                        return Err(Self::unsatisfied_trait_bound_error(
                            function_name,
                            &concrete_key,
                            crate::typeck::r#trait::trait_impl_label(
                                &bound.trait_name,
                                &resolved_args,
                            ),
                            &param.name,
                            span_lo,
                            span_hi,
                        ));
                    }
                }
            }
        }
        Ok(())
    }

    /// Whether `actual` can be unsize-coerced to `expected` as a trait object,
    /// i.e. `&Concrete -> &dyn Trait` where `Concrete` implements every listed
    /// trait. This mirrors the fat-pointer coercion performed in MIR lowering.
    pub(super) fn is_dyn_unsize_coercion(&self, expected: &Ty, actual: &Ty) -> bool {
        use crate::typeck::r#trait::type_key;
        let (TyKind::Ref(_, exp_inner), TyKind::Ref(_, act_inner)) = (&expected.kind, &actual.kind)
        else {
            return false;
        };
        let TyKind::Dyn(traits) = &exp_inner.kind else {
            return false;
        };
        if traits.is_empty() {
            return false;
        }
        let concrete = self.infer.apply_subst(act_inner);
        if matches!(concrete.kind, TyKind::Dyn(_) | TyKind::Var(_)) {
            return false;
        }
        let concrete_key = type_key(&concrete);
        traits
            .iter()
            .all(|t| self.impl_registry.implements_trait(t, &concrete_key))
    }

    /// Whether `actual` can be unsize-coerced to `expected` as an *owned* trait
    /// object, i.e. `Concrete -> dyn Trait` (by value) where `Concrete`
    /// implements every listed trait. The owned dyn value takes over drop
    /// responsibility and destroys the payload through the vtable drop slot.
    pub(super) fn is_owned_dyn_unsize_coercion(&self, expected: &Ty, actual: &Ty) -> bool {
        use crate::typeck::r#trait::type_key;
        let expected = self.infer.apply_subst(expected);
        let TyKind::Dyn(traits) = &expected.kind else {
            return false;
        };
        if traits.is_empty() {
            return false;
        }
        let concrete = self.infer.apply_subst(actual);
        if matches!(
            concrete.kind,
            TyKind::Dyn(_) | TyKind::Var(_) | TyKind::Ref(_, _)
        ) {
            return false;
        }
        let concrete_key = type_key(&concrete);
        traits
            .iter()
            .all(|t| self.impl_registry.implements_trait(t, &concrete_key))
    }

    /// Type check a method call by resolving candidates against the receiver type.
    pub(super) fn check_method_call(
        &mut self,
        receiver: &Expr,
        method: &Ident,
        args: &[Expr],
        expected_return: Option<&Ty>,
        call_span: crate::lexer::Span,
    ) -> TyResult<Ty> {
        use crate::typeck::r#trait::type_key;

        let receiver_ty = self.check_expr(receiver)?;
        let receiver_key = type_key(&receiver_ty);

        let mut arg_types = Vec::new();
        for arg in args {
            arg_types.push(self.check_expr(arg)?);
        }

        let method_name = &method.name;

        // Dynamic dispatch through a `dyn Trait` (or `&dyn Trait`) receiver:
        // resolve the method against the trait object's declared traits.
        let dyn_traits = match &receiver_ty.kind {
            TyKind::Dyn(traits) => Some(traits.clone()),
            TyKind::Ref(_, inner) => match &inner.kind {
                TyKind::Dyn(traits) => Some(traits.clone()),
                _ => None,
            },
            _ => None,
        };
        if let Some(traits) = dyn_traits {
            // Explicit early drop of an owned `dyn Trait` value: dispatched
            // through the vtable drop slot, not a trait method.
            if method_name == "drop"
                && args.is_empty()
                && matches!(receiver_ty.kind, TyKind::Dyn(_))
            {
                return Ok(self.env.unit_ty());
            }
            for trait_name in &traits {
                let Some(trait_info) = self.trait_registry.get(trait_name) else {
                    continue;
                };
                let Some(method_sig) = trait_info.get_method(method_name) else {
                    continue;
                };
                if method_sig.param_types.len() != args.len() {
                    return Err(TypeckError::ArgumentCountMismatch {
                        expected: method_sig.param_types.len(),
                        found: args.len(),
                    });
                }
                for (expected, actual) in method_sig.param_types.iter().zip(arg_types.iter()) {
                    let expected = self.infer.apply_subst(expected);
                    self.infer.unify(&expected, actual)?;
                }
                return Ok(self.infer.apply_subst(&method_sig.return_type));
            }
            return Err(TypeckError::MethodNotFound {
                type_name: format!("dyn {}", traits.join(" + ")),
                method_name: method_name.clone(),
            });
        }

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

        if matches!(&receiver_ty.kind, TyKind::Adt { name, .. } if name == "String")
            && method_name == "as_str"
        {
            if !args.is_empty() {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: 0,
                    found: args.len(),
                });
            }
            let str_ty = self.env.str_ty();
            return Ok(self.env.ref_ty(false, str_ty));
        }

        if method_name == "borrow" && args.is_empty() {
            let rc_payload = match &receiver_ty.kind {
                TyKind::Adt { name, args } if name == "Rc" && args.len() == 1 => {
                    Some(args[0].clone())
                }
                TyKind::Ref(_, inner) => match &inner.kind {
                    TyKind::Adt { name, args } if name == "Rc" && args.len() == 1 => {
                        Some(args[0].clone())
                    }
                    _ => None,
                },
                _ => None,
            };
            if let Some(payload_ty) = rc_payload {
                return Ok(self.env.ref_ty(false, payload_ty));
            }
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

            return self.resolve_associated_projection(&self.infer.apply_subst(&fn_ty.return_type));
        }

        // A generic parameter may call methods promised by its declared bounds
        // even though no concrete impl is selected until monomorphization.
        if let Some(fn_ty) =
            self.select_generic_bound_method_candidate(&receiver_ty, method_name, args.len())?
        {
            for (expected, actual) in fn_ty.param_types.iter().zip(arg_types.iter()) {
                self.infer.unify(expected, actual)?;
            }
            return self.resolve_associated_projection(&self.infer.apply_subst(&fn_ty.return_type));
        }

        // Then trait impl lookup.
        if let Some(fn_ty) = self.select_trait_method_call_candidate(
            &receiver_key,
            method_name,
            args.len(),
            expected_return,
            call_span,
        )? {
            for (expected, actual) in fn_ty.param_types.iter().zip(arg_types.iter()) {
                self.infer.unify(expected, actual)?;
            }
            let resolved =
                self.resolve_associated_projection(&self.infer.apply_subst(&fn_ty.return_type))?;
            if method_name == "into" {
                self.env
                    .record_method_return_type(call_span, resolved.clone());
            }
            return Ok(resolved);
        }

        Err(TypeckError::MethodNotFound {
            type_name: receiver_key,
            method_name: method_name.clone(),
        })
    }

    fn select_generic_bound_method_candidate(
        &mut self,
        receiver_ty: &Ty,
        method_name: &str,
        arg_count: usize,
    ) -> TyResult<Option<FunctionTy>> {
        let receiver_ty = self.infer.apply_subst(receiver_ty);
        let var_id = match &receiver_ty.kind {
            TyKind::Var(var_id) => Some(*var_id),
            TyKind::Ref(_, inner) => match &inner.kind {
                TyKind::Var(var_id) => Some(*var_id),
                _ => None,
            },
            _ => None,
        };
        let Some(var_id) = var_id else {
            return Ok(None);
        };
        let bounds = self
            .generic_var_bounds
            .get(&var_id)
            .cloned()
            .unwrap_or_default();

        let mut candidates = Vec::new();
        for trait_name in bounds {
            let Some(method_sig) = self
                .trait_registry
                .get(&trait_name)
                .and_then(|trait_info| trait_info.get_method(method_name).cloned())
            else {
                continue;
            };
            if !method_sig.has_self {
                continue;
            }
            let fn_ty = FunctionTy::with_generic_params(
                true,
                method_sig.param_types,
                method_sig.return_type,
                method_sig.generic_params,
            );
            candidates.push(MethodCandidate {
                label: trait_name,
                param_count: fn_ty.param_types.len(),
                value: self.instantiate_method_function_ty(&fn_ty, &HashMap::new()),
            });
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
                ambiguous_method_error(method_name, &format!("type variable {var_id}"), &labels),
            )),
        }
    }

    fn select_trait_method_call_candidate(
        &mut self,
        receiver_key: &str,
        method_name: &str,
        arg_count: usize,
        expected_return: Option<&Ty>,
        call_span: crate::lexer::Span,
    ) -> TyResult<Option<FunctionTy>> {
        let mut candidates = Vec::new();
        for trait_name in self.trait_registry.all_traits() {
            let trait_methods = self
                .impl_registry
                .lookup_trait_methods(&trait_name, receiver_key, method_name)
                .into_iter()
                .map(|(impl_info, fn_ty)| {
                    (
                        crate::typeck::r#trait::trait_impl_label(
                            &trait_name,
                            &impl_info.trait_args,
                        ),
                        fn_ty.clone(),
                    )
                })
                .collect::<Vec<_>>();
            for (trait_label, fn_ty) in trait_methods {
                if trait_name == "Drop" && method_name == "drop" {
                    return Err(TypeckError::diagnostic(
                        "drop-direct-call",
                        "`Drop::drop` is compiler-inserted and cannot be called directly",
                        call_span.lo,
                        call_span.hi,
                    ));
                }
                let instantiated = self.instantiate_method_function_ty(&fn_ty, &HashMap::new());
                candidates.push(MethodCandidate {
                    label: trait_label,
                    param_count: instantiated.param_types.len(),
                    value: instantiated,
                });
            }
        }

        if method_name == "into" {
            if let Some(expected) = expected_return
                .map(|ty| self.infer.apply_subst(ty))
                .filter(|ty| !matches!(ty.kind, TyKind::Var(_) | TyKind::Error))
            {
                let expected_key = type_key(&expected);
                let had_into_candidate = !candidates.is_empty();
                candidates.retain(|candidate| {
                    type_key(&self.infer.apply_subst(&candidate.value.return_type)) == expected_key
                });
                if had_into_candidate && candidates.is_empty() {
                    return Err(TypeckError::diagnostic(
                        "into-target-missing",
                        format!(
                            "type `{receiver_key}` has no lossless `Into<{expected_key}>` implementation"
                        ),
                        call_span.lo,
                        call_span.hi,
                    ));
                }
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
