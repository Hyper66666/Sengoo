use super::*;
use crate::typeck::r#trait::{type_key, FunctionTy, ImplInfo, MethodSig, TraitInfo};

#[derive(Default)]
struct FuturePollBodyFacts {
    has_evident_pending: bool,
    missing_wakeup: bool,
    context_param: Option<String>,
}

fn is_context_param_receiver(expr: &Expr, context_param: Option<&str>) -> bool {
    match &expr.kind {
        ExprKind::Ident(ident) => context_param == Some(ident.name.as_str()),
        ExprKind::Path(path) if path.segments.len() == 1 => {
            context_param == path.segments.first().map(|segment| segment.name.as_str())
        }
        _ => false,
    }
}

fn scan_future_poll_block(
    block: &Block,
    facts: &mut FuturePollBodyFacts,
    mut wake_registered: bool,
) -> bool {
    for statement in &block.stmts {
        wake_registered = match &statement.kind {
            StmtKind::Let { value, .. } => {
                if let Some(value) = value {
                    scan_future_poll_expr(value, facts, wake_registered)
                } else {
                    wake_registered
                }
            }
            StmtKind::Const { value, .. } | StmtKind::Expr(value) => {
                scan_future_poll_expr(value, facts, wake_registered)
            }
            StmtKind::Item(_) => wake_registered,
        };
    }
    wake_registered
}

fn scan_future_poll_expr(
    expr: &Expr,
    facts: &mut FuturePollBodyFacts,
    wake_registered: bool,
) -> bool {
    match &expr.kind {
        ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => {
            let registers_wakeup = matches!(method.name.as_str(), "wake" | "wake_after")
                && is_context_param_receiver(receiver, facts.context_param.as_deref());
            let mut wake_registered = scan_future_poll_expr(receiver, facts, wake_registered);
            for arg in args {
                wake_registered = scan_future_poll_expr(arg, facts, wake_registered);
            }
            wake_registered || registers_wakeup
        }
        ExprKind::Call { func, args } => {
            let mut wake_registered = scan_future_poll_expr(func, facts, wake_registered);
            for arg in args {
                wake_registered = scan_future_poll_expr(arg, facts, wake_registered);
            }
            wake_registered
        }
        ExprKind::Struct { path, fields, base } => {
            let mut wake_registered = wake_registered;
            for field in fields {
                wake_registered = scan_future_poll_expr(&field.value, facts, wake_registered);
            }
            if let Some(base) = base {
                wake_registered = scan_future_poll_expr(base, facts, wake_registered);
            }
            let is_poll = path
                .segments
                .last()
                .is_some_and(|segment| segment.name == "Poll");
            if is_poll && fields.iter().any(|field| {
                matches!(&field.name, crate::ast::FieldName::Ident(name) if name.name == "is_ready")
                    && matches!(field.value.kind, ExprKind::Literal(Literal::Bool(false)))
            }) {
                facts.has_evident_pending = true;
                if !wake_registered {
                    facts.missing_wakeup = true;
                }
            }
            wake_registered
        }
        ExprKind::Binary { left, right, .. }
        | ExprKind::Index {
            base: left,
            index: right,
        }
        | ExprKind::Assign {
            target: left,
            value: right,
        }
        | ExprKind::AssignOp {
            target: left,
            value: right,
            ..
        } => {
            let wake_registered = scan_future_poll_expr(left, facts, wake_registered);
            scan_future_poll_expr(right, facts, wake_registered)
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::Await(operand)
        | ExprKind::Try(operand)
        | ExprKind::Cast { expr: operand, .. }
        | ExprKind::Is { expr: operand, .. }
        | ExprKind::Paren(operand)
        | ExprKind::Field { base: operand, .. } => {
            scan_future_poll_expr(operand, facts, wake_registered)
        }
        ExprKind::Block(block) | ExprKind::TryBlock(block) => {
            scan_future_poll_block(block, facts, wake_registered)
        }
        ExprKind::AsyncBlock(_) | ExprKind::ParallelBlock(_) => wake_registered,
        ExprKind::Loop(block) => {
            scan_future_poll_block(block, facts, wake_registered);
            wake_registered
        }
        ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            let wake_registered = scan_future_poll_expr(cond, facts, wake_registered);
            let then_wake = scan_future_poll_block(then_branch, facts, wake_registered);
            let else_wake = else_branch.as_ref().map_or(wake_registered, |else_branch| {
                scan_future_poll_expr(else_branch, facts, wake_registered)
            });
            then_wake && else_wake
        }
        ExprKind::While { cond, body } => {
            let wake_registered = scan_future_poll_expr(cond, facts, wake_registered);
            scan_future_poll_block(body, facts, wake_registered);
            wake_registered
        }
        ExprKind::For { iter, body, .. } => {
            let wake_registered = scan_future_poll_expr(iter, facts, wake_registered);
            scan_future_poll_block(body, facts, wake_registered);
            wake_registered
        }
        ExprKind::Match { scrutinee, arms } => {
            let wake_registered = scan_future_poll_expr(scrutinee, facts, wake_registered);
            let mut all_arms_wake = !arms.is_empty();
            for arm in arms {
                let mut arm_wake = wake_registered;
                if let Some(guard) = &arm.guard {
                    arm_wake = scan_future_poll_expr(guard, facts, arm_wake);
                }
                arm_wake = scan_future_poll_expr(&arm.body, facts, arm_wake);
                all_arms_wake &= arm_wake;
            }
            wake_registered || all_arms_wake
        }
        ExprKind::Return(value)
        | ExprKind::Break(value)
        | ExprKind::Yield(value)
        | ExprKind::Range {
            start: value,
            end: None,
            ..
        } => {
            if let Some(value) = value {
                scan_future_poll_expr(value, facts, wake_registered)
            } else {
                wake_registered
            }
        }
        ExprKind::Range { start, end, .. } => {
            let mut wake_registered = wake_registered;
            if let Some(start) = start {
                wake_registered = scan_future_poll_expr(start, facts, wake_registered);
            }
            if let Some(end) = end {
                wake_registered = scan_future_poll_expr(end, facts, wake_registered);
            }
            wake_registered
        }
        ExprKind::Lambda { .. } => wake_registered,
        ExprKind::Array(elements) | ExprKind::Tuple(elements) => {
            let mut wake_registered = wake_registered;
            for element in elements {
                wake_registered = scan_future_poll_expr(element, facts, wake_registered);
            }
            wake_registered
        }
        ExprKind::Literal(_) | ExprKind::Ident(_) | ExprKind::Path(_) | ExprKind::Continue => {
            wake_registered
        }
    }
}

impl TypeChecker {
    fn validate_future_poll_contract(method: &Function) -> Result<()> {
        if method.name.name != "poll" {
            return Ok(());
        }

        if !matches!(method.self_param, Some(SelfParam::BorrowedMut)) {
            return Err(CompileError::from(TypeckError::Other(
                "Future<T>::poll must use `&mut self` receiver".to_string(),
            )));
        }

        Ok(())
    }

    fn validate_future_poll_wakeup_contract(method: &Function) -> Result<()> {
        if method.name.name != "poll" {
            return Ok(());
        }
        let mut facts = FuturePollBodyFacts {
            context_param: method.params.last().map(|param| param.name.name.clone()),
            ..FuturePollBodyFacts::default()
        };
        scan_future_poll_block(&method.body, &mut facts, false);
        if facts.has_evident_pending && facts.missing_wakeup {
            return Err(CompileError::from(TypeckError::diagnostic(
                "async::user_future_missing_wakeup",
                "Future<T>::poll returns Pending without calling AsyncContext.wake() or wake_after()",
                method.name.span.lo,
                method.name.span.hi,
            )));
        }
        Ok(())
    }

    /// Enforce the compiler-known `Drop` trait contract: `def drop(&mut self)`.
    /// `drop` is compiler-inserted, so its only method must take `&mut self` and
    /// accept no other parameters; a malformed signature is rejected here rather
    /// than producing surprising drop glue later.
    fn validate_drop_contract(method: &Function) -> Result<()> {
        if method.name.name != "drop" {
            return Err(CompileError::from(TypeckError::diagnostic(
                "drop-trait-contract",
                "`Drop` impls may only define `def drop(&mut self)`",
                method.name.span.lo,
                method.name.span.hi,
            )));
        }

        if !matches!(method.self_param, Some(SelfParam::BorrowedMut))
            || !method.params.is_empty()
            || method.return_type.is_some()
            || method.is_async
            || method.abi.is_some()
        {
            return Err(CompileError::from(TypeckError::diagnostic(
                "drop-trait-contract",
                "`Drop::drop` must be a synchronous `def drop(&mut self)` method with no parameters and no return type",
                method.name.span.lo,
                method.name.span.hi,
            )));
        }

        Ok(())
    }

    /// Whether `ty` is the compiler-known owned `String` type.
    fn is_owned_string_return_ty(ty: &Ty) -> bool {
        matches!(&ty.kind, TyKind::Adt { name, .. } if name == "String")
    }

    fn validate_operator_trait_declaration_contract(
        trait_info: &TraitInfo,
        type_params: &[GenericTypeParamMeta],
        span: crate::lexer::Span,
    ) -> TyResult<()> {
        let Some(contract) = operator_trait_contract(&trait_info.name) else {
            return Ok(());
        };
        let expected_type_params = if contract.has_rhs { 2 } else { 1 };
        if type_params.len() != expected_type_params {
            return Err(TypeckError::diagnostic(
                "operator-trait-contract",
                format!(
                    "operator trait `{}` must declare {} type parameter(s): {}",
                    contract.trait_name,
                    expected_type_params,
                    if contract.has_rhs {
                        "`Rhs, Output`"
                    } else {
                        "`Output`"
                    }
                ),
                span.lo,
                span.hi,
            ));
        }

        let Some(method) = trait_info.get_method(contract.method_name) else {
            return Err(TypeckError::diagnostic(
                "operator-trait-contract",
                format!(
                    "operator trait `{}` must define `def {}(...)`",
                    contract.trait_name, contract.method_name
                ),
                span.lo,
                span.hi,
            ));
        };
        let expected_params = usize::from(contract.has_rhs);
        if !method.has_self || method.param_types.len() != expected_params {
            return Err(TypeckError::diagnostic(
                "operator-trait-contract",
                format!(
                    "operator trait `{}` method `{}` must take `self`{}",
                    contract.trait_name,
                    contract.method_name,
                    if contract.has_rhs {
                        " and exactly one `Rhs` argument"
                    } else {
                        " and no explicit arguments"
                    }
                ),
                span.lo,
                span.hi,
            ));
        }

        if contract.has_rhs {
            let rhs_var = type_params[0].var_id;
            if !matches!(method.param_types[0].kind, TyKind::Var(var_id) if var_id == rhs_var) {
                return Err(TypeckError::diagnostic(
                    "operator-trait-contract",
                    format!(
                        "operator trait `{}` method `{}` must accept its first trait parameter as `Rhs`",
                        contract.trait_name, contract.method_name
                    ),
                    span.lo,
                    span.hi,
                ));
            }
        }

        let output_var = type_params[type_params.len() - 1].var_id;
        if !matches!(method.return_type.kind, TyKind::Var(var_id) if var_id == output_var) {
            return Err(TypeckError::diagnostic(
                "operator-trait-contract",
                format!(
                    "operator trait `{}` method `{}` must return its final `Output` type parameter",
                    contract.trait_name, contract.method_name
                ),
                span.lo,
                span.hi,
            ));
        }
        Ok(())
    }

    fn validate_operator_trait_impl_contract(
        impl_info: &ImplInfo,
        target_key: &str,
        span: crate::lexer::Span,
    ) -> TyResult<()> {
        let Some(trait_name) = impl_info.trait_name.as_deref() else {
            return Ok(());
        };
        let Some(contract) = operator_trait_contract(trait_name) else {
            return Ok(());
        };
        let expected_trait_args = if contract.has_rhs { 2 } else { 1 };
        if impl_info.trait_args.len() != expected_trait_args {
            return Err(TypeckError::diagnostic(
                "operator-trait-contract",
                format!(
                    "impl `{}` for `{target_key}` must provide {} trait argument(s): {}",
                    contract.trait_name,
                    expected_trait_args,
                    if contract.has_rhs {
                        "`Rhs, Output`"
                    } else {
                        "`Output`"
                    }
                ),
                span.lo,
                span.hi,
            ));
        }

        let Some(method) = impl_info.get_method(contract.method_name) else {
            return Ok(());
        };
        let expected_params = usize::from(contract.has_rhs);
        if !method.has_self || method.param_types.len() != expected_params {
            return Err(TypeckError::diagnostic(
                "operator-trait-contract",
                format!(
                    "impl `{}` for `{target_key}` method `{}` has the wrong receiver or argument count",
                    contract.trait_name, contract.method_name
                ),
                span.lo,
                span.hi,
            ));
        }
        if contract.has_rhs
            && type_key(&method.param_types[0]) != type_key(&impl_info.trait_args[0])
        {
            return Err(TypeckError::diagnostic(
                "operator-trait-rhs-mismatch",
                format!(
                    "impl `{}` for `{target_key}` declares Rhs `{}` but method `{}` accepts `{}`",
                    contract.trait_name,
                    impl_info.trait_args[0],
                    contract.method_name,
                    method.param_types[0]
                ),
                span.lo,
                span.hi,
            ));
        }
        let output = &impl_info.trait_args[impl_info.trait_args.len() - 1];
        if type_key(&method.return_type) != type_key(output) {
            return Err(TypeckError::diagnostic(
                "operator-trait-output-mismatch",
                format!(
                    "impl `{}` for `{target_key}` declares Output `{output}` but method `{}` returns `{}`",
                    contract.trait_name, contract.method_name, method.return_type
                ),
                span.lo,
                span.hi,
            ));
        }
        Ok(())
    }

    pub(super) fn check_trait_decl(&mut self, trait_decl: &Trait) -> Result<()> {
        if trait_decl.name.name == "Drop" {
            return Err(CompileError::from(TypeckError::diagnostic(
                "drop-trait-reserved",
                "`Drop` is a compiler-known trait; user code must not redeclare it",
                trait_decl.name.span.lo,
                trait_decl.name.span.hi,
            )));
        }
        if trait_decl.name.name == "Copy" {
            return Err(CompileError::from(TypeckError::diagnostic(
                "copy-trait-reserved",
                "`Copy` is a compiler-known trait; user code must not redeclare it",
                trait_decl.name.span.lo,
                trait_decl.name.span.hi,
            )));
        }

        self.env.push_scope();
        let trait_type_params = self.bind_type_params_with_meta(&trait_decl.type_params)?;

        let mut trait_info = TraitInfo::new(
            trait_decl.name.name.clone(),
            trait_decl
                .type_params
                .iter()
                .map(|tp| tp.name.name.clone())
                .collect(),
            matches!(trait_decl.vis, Visibility::Public),
        );
        self.current_trait_associated_types = Some((
            trait_decl.name.name.clone(),
            trait_decl
                .items
                .iter()
                .filter_map(|item| match item {
                    TraitItem::Type(type_alias) => Some(type_alias.name.name.clone()),
                    _ => None,
                })
                .collect(),
        ));

        for bound in &trait_decl.bounds {
            if let Some(ident) = bound.path.as_simple() {
                trait_info.add_supertrait(ident.name.clone());
                self.pending_supertrait_links.push((
                    trait_decl.name.name.clone(),
                    ident.name.clone(),
                    trait_decl.span,
                ));
            } else {
                self.env.pop_scope();
                return Err(CompileError::from(TypeckError::Other(
                    "unsupported supertrait path in trait declaration".to_string(),
                )));
            }
        }

        for item in &trait_decl.items {
            match item {
                TraitItem::Function(method) => {
                    if trait_decl.name.name == "Future" {
                        Self::validate_future_poll_contract(method)?;
                    }
                    if trait_decl.name.name == "Drop" {
                        Self::validate_drop_contract(method)?;
                    }

                    self.env.push_scope();
                    let method_generic_meta =
                        self.bind_type_params_with_meta(&method.type_params)?;
                    let mut param_types = Vec::new();
                    let has_self = method.self_param.is_some();

                    for param in &method.params {
                        let ty = self.check_type(&param.ty)?;
                        param_types.push(ty);
                    }

                    let ret_ty = if let Some(ret) = &method.return_type {
                        self.check_type(ret)?
                    } else {
                        self.env.unit_ty()
                    };

                    let has_default = !method.body.stmts.is_empty();
                    if has_default {
                        self.trait_default_methods
                            .entry(trait_decl.name.name.clone())
                            .or_default()
                            .insert(method.name.name.clone(), method.clone());
                    }
                    let sig = if has_default {
                        MethodSig::with_default(
                            has_self,
                            param_types,
                            ret_ty,
                            method_generic_meta.iter().map(|meta| meta.var_id).collect(),
                        )
                    } else {
                        MethodSig::new(
                            has_self,
                            param_types,
                            ret_ty,
                            method_generic_meta.iter().map(|meta| meta.var_id).collect(),
                        )
                    };
                    trait_info.add_method(method.name.name.clone(), sig);
                    self.env.pop_scope();
                }
                TraitItem::Const(const_decl) => {
                    let ty = self.check_type(&const_decl.ty)?;
                    trait_info.add_const(const_decl.name.name.clone(), ty);
                }
                TraitItem::Type(type_alias) => {
                    trait_info.add_assoc_type(type_alias.name.name.clone());
                }
            }
        }

        if let Err(error) = Self::validate_operator_trait_declaration_contract(
            &trait_info,
            &trait_type_params,
            trait_decl.span,
        ) {
            self.env.pop_scope();
            return Err(CompileError::from(error));
        }
        self.trait_registry.register(trait_info);
        self.current_trait_associated_types = None;

        self.env.pop_scope();
        Ok(())
    }

    fn specialize_trait_contract_ty(
        ty: &Ty,
        target_ty: &Ty,
        trait_name: &str,
        associated_types: &HashMap<String, Ty>,
    ) -> Ty {
        let kind = match &ty.kind {
            TyKind::SelfType => return target_ty.clone(),
            TyKind::AssocProjection {
                base,
                trait_name: projection_trait,
                name,
            } if projection_trait == trait_name && matches!(base.kind, TyKind::SelfType) => {
                return associated_types
                    .get(name)
                    .cloned()
                    .unwrap_or_else(|| ty.clone());
            }
            TyKind::Tuple(items) => TyKind::Tuple(
                items
                    .iter()
                    .map(|item| {
                        Self::specialize_trait_contract_ty(
                            item,
                            target_ty,
                            trait_name,
                            associated_types,
                        )
                    })
                    .collect(),
            ),
            TyKind::Array(item, len) => TyKind::Array(
                Box::new(Self::specialize_trait_contract_ty(
                    item,
                    target_ty,
                    trait_name,
                    associated_types,
                )),
                *len,
            ),
            TyKind::Slice(item) => TyKind::Slice(Box::new(Self::specialize_trait_contract_ty(
                item,
                target_ty,
                trait_name,
                associated_types,
            ))),
            TyKind::Ref(is_mut, item) => TyKind::Ref(
                *is_mut,
                Box::new(Self::specialize_trait_contract_ty(
                    item,
                    target_ty,
                    trait_name,
                    associated_types,
                )),
            ),
            TyKind::Ptr(item) => TyKind::Ptr(Box::new(Self::specialize_trait_contract_ty(
                item,
                target_ty,
                trait_name,
                associated_types,
            ))),
            TyKind::Fn {
                params,
                ret,
                is_variadic,
            } => TyKind::Fn {
                params: params
                    .iter()
                    .map(|param| {
                        Self::specialize_trait_contract_ty(
                            param,
                            target_ty,
                            trait_name,
                            associated_types,
                        )
                    })
                    .collect(),
                ret: Box::new(Self::specialize_trait_contract_ty(
                    ret,
                    target_ty,
                    trait_name,
                    associated_types,
                )),
                is_variadic: *is_variadic,
            },
            TyKind::Adt { name, args } => TyKind::Adt {
                name: name.clone(),
                args: args
                    .iter()
                    .map(|arg| {
                        Self::specialize_trait_contract_ty(
                            arg,
                            target_ty,
                            trait_name,
                            associated_types,
                        )
                    })
                    .collect(),
            },
            TyKind::AssocProjection {
                base,
                trait_name,
                name,
            } => TyKind::AssocProjection {
                base: Box::new(Self::specialize_trait_contract_ty(
                    base,
                    target_ty,
                    trait_name,
                    associated_types,
                )),
                trait_name: trait_name.clone(),
                name: name.clone(),
            },
            TyKind::Future(inner) => TyKind::Future(Box::new(Self::specialize_trait_contract_ty(
                inner,
                target_ty,
                trait_name,
                associated_types,
            ))),
            _ => return ty.clone(),
        };
        Ty { id: ty.id, kind }
    }

    fn validate_trait_method_contracts(
        trait_name: &str,
        trait_info: &TraitInfo,
        impl_info: &ImplInfo,
        target_key: &str,
        span: crate::lexer::Span,
    ) -> Result<()> {
        fn contains_associated_projection(ty: &Ty) -> bool {
            match &ty.kind {
                TyKind::AssocProjection { .. } => true,
                TyKind::Tuple(items) => items.iter().any(contains_associated_projection),
                TyKind::Array(item, _)
                | TyKind::Slice(item)
                | TyKind::Ref(_, item)
                | TyKind::Ptr(item)
                | TyKind::Future(item) => contains_associated_projection(item),
                TyKind::Fn { params, ret, .. } => {
                    params.iter().any(contains_associated_projection)
                        || contains_associated_projection(ret)
                }
                TyKind::Adt { args, .. } => args.iter().any(contains_associated_projection),
                _ => false,
            }
        }

        for (method_name, actual) in &impl_info.methods {
            let Some(expected) = trait_info.methods.get(method_name) else {
                continue;
            };
            let expected_params = expected
                .param_types
                .iter()
                .map(|ty| {
                    Self::specialize_trait_contract_ty(
                        ty,
                        &impl_info.target_type,
                        trait_name,
                        &impl_info.assoc_types,
                    )
                })
                .collect::<Vec<_>>();
            let expected_return = Self::specialize_trait_contract_ty(
                &expected.return_type,
                &impl_info.target_type,
                trait_name,
                &impl_info.assoc_types,
            );
            let shape_matches = actual.has_self == expected.has_self
                && actual.param_types.len() == expected_params.len()
                && actual.generic_params.len() == expected.generic_params.len();
            let contract_uses_projection = expected
                .param_types
                .iter()
                .any(contains_associated_projection)
                || contains_associated_projection(&expected.return_type);
            let types_match = !contract_uses_projection
                || !expected.generic_params.is_empty()
                || (actual
                    .param_types
                    .iter()
                    .zip(&expected_params)
                    .all(|(actual, expected)| type_key(actual) == type_key(expected))
                    && type_key(&actual.return_type) == type_key(&expected_return));
            if !shape_matches || !types_match {
                return Err(CompileError::from(TypeckError::diagnostic(
                    "trait-method-signature-mismatch",
                    format!(
                        "trait method `{trait_name}::{method_name}` implementation for `{target_key}` does not match its declared signature"
                    ),
                    span.lo,
                    span.hi,
                )));
            }
        }
        Ok(())
    }

    pub(super) fn check_impl_decl(&mut self, impl_decl: &Impl) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&impl_decl.type_params)?;

        let target_ty = self.check_type(&impl_decl.target_type)?;
        let target_key = type_key(&target_ty);

        let trait_name = impl_decl
            .trait_path
            .as_ref()
            .and_then(|p| p.as_simple())
            .map(|s| s.name.clone());
        let trait_args = impl_decl
            .trait_args
            .iter()
            .map(|arg| self.check_type(arg))
            .collect::<TyResult<Vec<_>>>()?;
        let is_future_impl = matches!(trait_name.as_deref(), Some("Future"));
        let is_drop_impl = matches!(trait_name.as_deref(), Some("Drop"));
        let is_copy_impl = matches!(trait_name.as_deref(), Some("Copy"));
        let is_display_impl = matches!(trait_name.as_deref(), Some("Display"));
        let is_debug_impl = matches!(trait_name.as_deref(), Some("Debug"));
        if let Some(name) = trait_name.as_deref() {
            if let Err(err) = self.validate_orphan_rule(name, &target_ty, impl_decl.span) {
                self.env.pop_scope();
                return Err(CompileError::from(err));
            }
        }

        if impl_decl.is_negative {
            let Some(trait_name) = trait_name.clone() else {
                self.env.pop_scope();
                return Err(CompileError::from(TypeckError::diagnostic(
                    "negative-impl-requires-trait",
                    "negative impl syntax requires `impl !Trait for Type {}`",
                    impl_decl.span.lo,
                    impl_decl.span.hi,
                )));
            };
            if !matches!(trait_name.as_str(), "Send" | "Sync") {
                self.env.pop_scope();
                return Err(CompileError::from(TypeckError::diagnostic(
                    "negative-impl-non-marker",
                    "negative impls are only supported for marker traits `Send` and `Sync`",
                    impl_decl.span.lo,
                    impl_decl.span.hi,
                )));
            }
            if !impl_decl.trait_args.is_empty() {
                self.env.pop_scope();
                return Err(CompileError::from(TypeckError::diagnostic(
                    "negative-impl-trait-args",
                    format!("negative impl `!{trait_name}` must not have trait arguments"),
                    impl_decl.span.lo,
                    impl_decl.span.hi,
                )));
            }
            if !impl_decl.items.is_empty() || !impl_decl.associated_types.is_empty() {
                self.env.pop_scope();
                return Err(CompileError::from(TypeckError::diagnostic(
                    "negative-impl-items",
                    format!(
                        "negative impl `!{trait_name}` for `{target_key}` must not define methods or associated types"
                    ),
                    impl_decl.span.lo,
                    impl_decl.span.hi,
                )));
            }
            if self
                .impl_registry
                .implements_trait(&trait_name, &target_key)
                || self.negative_auto_marker_impl_overlaps(&trait_name, &target_ty)
            {
                self.env.pop_scope();
                return Err(CompileError::from(TypeckError::diagnostic(
                    "positive-negative-impl-conflict",
                    format!(
                        "conflicting positive and negative implementations of trait `{trait_name}` for type `{target_key}`"
                    ),
                    impl_decl.span.lo,
                    impl_decl.span.hi,
                )));
            }
            self.impl_registry
                .register_negative_trait_impl(trait_name, target_ty);
            self.env.pop_scope();
            return Ok(());
        }
        if is_copy_impl {
            if let Err(err) = self.validate_copy_impl(&target_ty, &target_key, impl_decl.span) {
                self.env.pop_scope();
                return Err(CompileError::from(err));
            }
        }
        if is_drop_impl {
            if self.impl_registry.implements_trait("Copy", &target_key) {
                self.env.pop_scope();
                return Err(CompileError::from(TypeckError::diagnostic(
                    "copy-drop-conflict",
                    format!("type `{target_key}` cannot implement both `Copy` and `Drop`"),
                    impl_decl.span.lo,
                    impl_decl.span.hi,
                )));
            }
            self.env.mark_drop_owned_type(&target_ty);
        }

        let mut impl_info = ImplInfo::new(target_ty.clone(), trait_name, trait_args);

        for item in &impl_decl.associated_types {
            let ty = self.check_type(&item.ty)?;
            impl_info.add_assoc_type(item.name.name.clone(), ty);
        }

        for item in &impl_decl.items {
            if is_future_impl {
                Self::validate_future_poll_contract(item)?;
                Self::validate_future_poll_wakeup_contract(item)?;
            }
            if is_drop_impl {
                Self::validate_drop_contract(item)?;
            }

            self.env.push_scope();
            let method_generic_meta = self.bind_type_params_with_meta(&item.type_params)?;
            let mut param_types = Vec::new();
            let mut has_self = item.self_param.is_some();
            for param in &item.params {
                if param.name.name == "self" {
                    has_self = true;
                } else {
                    let ty = self.check_type(&param.ty)?;
                    param_types.push(ty);
                }
            }
            let ret_ty = if let Some(ret) = &item.return_type {
                self.check_type(ret)?
            } else {
                self.env.unit_ty()
            };
            impl_info.add_method(
                item.name.name.clone(),
                FunctionTy::with_generic_params(
                    has_self,
                    param_types,
                    ret_ty,
                    method_generic_meta.iter().map(|meta| meta.var_id).collect(),
                ),
            );
            self.env.pop_scope();
        }

        if let Err(error) =
            Self::validate_operator_trait_impl_contract(&impl_info, &target_key, impl_decl.span)
        {
            self.env.pop_scope();
            return Err(CompileError::from(error));
        }

        if is_display_impl || is_debug_impl {
            let to_string = impl_decl
                .items
                .iter()
                .find(|method| method.name.name == "to_string");
            let has_derived_debug_marker = is_debug_impl
                && impl_decl
                    .items
                    .iter()
                    .any(|method| method.name.name == "__derived_debug_marker");
            let contract_ok = has_derived_debug_marker
                || match to_string {
                    Some(method) => {
                        let has_self = method.self_param.is_some();
                        let returns_string = match &method.return_type {
                            Some(ret) => Self::is_owned_string_return_ty(&self.check_type(ret)?),
                            None => false,
                        };
                        has_self && returns_string
                    }
                    None => false,
                };
            if !contract_ok {
                let (code, trait_label) = if is_debug_impl {
                    ("debug-contract", "Debug")
                } else {
                    ("display-contract", "Display")
                };
                self.env.pop_scope();
                return Err(CompileError::from(TypeckError::diagnostic(
                    code,
                    format!(
                        "impl {trait_label} for {target_key} must define `def to_string(&self) -> String`"
                    ),
                    impl_decl.span.lo,
                    impl_decl.span.hi,
                )));
            }
        }

        if let Some(trait_name) = impl_info.trait_name.clone() {
            if self.negative_auto_marker_impl_overlaps(&trait_name, &target_ty) {
                self.env.pop_scope();
                return Err(CompileError::from(TypeckError::diagnostic(
                    "positive-negative-impl-conflict",
                    format!(
                        "conflicting positive and negative implementations of trait `{trait_name}` for type `{target_key}`"
                    ),
                    impl_decl.span.lo,
                    impl_decl.span.hi,
                )));
            }
            if let Some(trait_info) = self.trait_registry.get(&trait_name) {
                Self::validate_trait_method_contracts(
                    &trait_name,
                    &trait_info,
                    &impl_info,
                    &target_key,
                    impl_decl.span,
                )?;
                let mut missing_methods = Vec::new();

                for (method_name, method_sig) in &trait_info.methods {
                    if !impl_info.has_method(method_name) {
                        if method_sig.has_default {
                            let param_types = method_sig
                                .param_types
                                .iter()
                                .map(|ty| {
                                    Self::specialize_trait_contract_ty(
                                        ty,
                                        &impl_info.target_type,
                                        &trait_name,
                                        &impl_info.assoc_types,
                                    )
                                })
                                .collect();
                            let return_type = Self::specialize_trait_contract_ty(
                                &method_sig.return_type,
                                &impl_info.target_type,
                                &trait_name,
                                &impl_info.assoc_types,
                            );
                            impl_info.add_method(
                                method_name.clone(),
                                FunctionTy::with_generic_params(
                                    method_sig.has_self,
                                    param_types,
                                    return_type,
                                    method_sig.generic_params.clone(),
                                ),
                            );
                        } else {
                            missing_methods.push(method_name.clone());
                        }
                    }
                }

                if !missing_methods.is_empty() {
                    missing_methods.sort();
                    self.env.pop_scope();
                    let err = TypeckError::Other(format!(
                        "impl {} for {} is missing required trait methods: {}",
                        trait_name,
                        target_key,
                        missing_methods.join(", ")
                    ));
                    return Err(CompileError::TypeckError(err));
                }

                let mut missing_associated_types = trait_info
                    .assoc_types
                    .iter()
                    .filter(|name| !impl_info.assoc_types.contains_key(*name))
                    .cloned()
                    .collect::<Vec<_>>();
                if !missing_associated_types.is_empty() {
                    missing_associated_types.sort();
                    self.env.pop_scope();
                    let err = TypeckError::Other(format!(
                        "impl {} for {} is missing required associated types: {}",
                        trait_name,
                        target_key,
                        missing_associated_types.join(", ")
                    ));
                    return Err(CompileError::TypeckError(err));
                }

                let mut unknown_associated_types = impl_info
                    .assoc_types
                    .keys()
                    .filter(|name| !trait_info.assoc_types.contains(*name))
                    .cloned()
                    .collect::<Vec<_>>();
                if !unknown_associated_types.is_empty() {
                    unknown_associated_types.sort();
                    self.env.pop_scope();
                    let err = TypeckError::Other(format!(
                        "impl {} for {} defines unknown associated types: {}",
                        trait_name,
                        target_key,
                        unknown_associated_types.join(", ")
                    ));
                    return Err(CompileError::TypeckError(err));
                }
            }

            if self.impl_registry.has_trait_impl_with_args(
                &trait_name,
                &target_key,
                &impl_info.trait_args,
            ) {
                self.env.pop_scope();
                let trait_label =
                    crate::typeck::r#trait::trait_impl_label(&trait_name, &impl_info.trait_args);
                return Err(CompileError::from(TypeckError::diagnostic(
                    "conflicting-impl",
                    format!(
                        "conflicting implementations of trait `{trait_label}` for type `{target_key}`"
                    ),
                    impl_decl.span.lo,
                    impl_decl.span.hi,
                )));
            }

            if self
                .trait_registry
                .get(&trait_name)
                .map(|info| !info.supertraits.is_empty())
                .unwrap_or(false)
            {
                self.pending_supertrait_obligations.push((
                    trait_name.clone(),
                    target_key.clone(),
                    impl_decl.span,
                ));
            }
            self.impl_registry
                .register_trait_impl(trait_name, target_key, impl_info);
        } else {
            self.impl_registry.register_inherent(target_key, impl_info);
        }

        self.env.pop_scope();
        Ok(())
    }

    /// Compute the transitive set of supertraits of `trait_name`. Uses a `seen`
    /// set so a cyclic supertrait graph terminates instead of looping forever.
    pub(super) fn transitive_supertraits(&self, trait_name: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut stack: Vec<String> = Vec::new();
        if let Some(info) = self.trait_registry.get(trait_name) {
            stack.extend(info.supertraits.iter().cloned());
        }
        while let Some(current) = stack.pop() {
            if !seen.insert(current.clone()) {
                continue;
            }
            out.push(current.clone());
            if let Some(info) = self.trait_registry.get(&current) {
                stack.extend(info.supertraits.iter().cloned());
            }
        }
        out
    }

    /// Validate supertrait declarations and impl obligations once every trait and
    /// impl has been registered (ordering-independent): declared supertraits must
    /// name known traits, the supertrait graph must be acyclic, and any
    /// `impl Sub for T` requires `T` to also implement each supertrait of `Sub`.
    pub(super) fn validate_supertrait_obligations(&mut self) -> Result<()> {
        let links = std::mem::take(&mut self.pending_supertrait_links);
        for (owner, supertrait, span) in &links {
            if !self.trait_registry.contains(supertrait) {
                self.pending_supertrait_obligations.clear();
                return Err(CompileError::from(TypeckError::diagnostic(
                    "unknown-supertrait",
                    format!("trait `{owner}` lists unknown supertrait `{supertrait}`"),
                    span.lo,
                    span.hi,
                )));
            }
        }
        for (owner, _supertrait, span) in &links {
            if self
                .transitive_supertraits(owner)
                .iter()
                .any(|s| s == owner)
            {
                self.pending_supertrait_obligations.clear();
                return Err(CompileError::from(TypeckError::diagnostic(
                    "supertrait-cycle",
                    format!("trait `{owner}` is part of a supertrait cycle"),
                    span.lo,
                    span.hi,
                )));
            }
        }

        let obligations = std::mem::take(&mut self.pending_supertrait_obligations);
        for (trait_name, target_key, span) in obligations {
            for supertrait in self.transitive_supertraits(&trait_name) {
                if !self
                    .impl_registry
                    .implements_trait(&supertrait, &target_key)
                {
                    return Err(CompileError::from(TypeckError::diagnostic(
                        "missing-supertrait-impl",
                        format!(
                            "`{target_key}` implements `{trait_name}` but not its supertrait `{supertrait}`; add `impl {supertrait} for {target_key}`"
                        ),
                        span.lo,
                        span.hi,
                    )));
                }
            }
        }
        Ok(())
    }

    fn validate_orphan_rule(
        &self,
        trait_name: &str,
        target_ty: &Ty,
        span: crate::lexer::Span,
    ) -> TyResult<()> {
        if self.is_package_local_trait(trait_name) || self.is_package_local_type(target_ty) {
            return Ok(());
        }

        Err(TypeckError::diagnostic(
            "orphan-rule",
            format!(
                "orphan impl rejected: trait `{}` and type `{}` are both external to this package",
                trait_name, target_ty
            ),
            span.lo,
            span.hi,
        ))
    }

    fn is_package_local_trait(&self, trait_name: &str) -> bool {
        matches!(
            self.env.lookup(trait_name).map(|symbol| &symbol.kind),
            Some(SymbolKind::Trait { .. })
        )
    }

    fn is_package_local_type(&self, ty: &Ty) -> bool {
        match &ty.kind {
            TyKind::Adt { name, .. } => {
                self.generic_type_metas.contains_key(name)
                    || self.struct_field_defs.contains_key(name)
                    || self.enum_variants.contains_key(name)
                    || self.class_decls.contains_key(name)
            }
            _ => false,
        }
    }

    fn validate_copy_impl(
        &mut self,
        target_ty: &Ty,
        target_key: &str,
        span: crate::lexer::Span,
    ) -> TyResult<()> {
        if self.env.is_drop_owned_type(target_ty) {
            return Err(TypeckError::diagnostic(
                "copy-drop-conflict",
                format!("type `{target_key}` cannot implement both `Copy` and `Drop`"),
                span.lo,
                span.hi,
            ));
        }

        let TyKind::Adt { name, .. } = &target_ty.kind else {
            return Ok(());
        };
        let Some(field_defs) = self.struct_field_defs.get(name).cloned() else {
            return Ok(());
        };

        for (field_name, field_ty) in field_defs {
            let resolved = self.check_type(&field_ty)?;
            if !self.type_is_copy_eligible(&resolved) {
                return Err(TypeckError::diagnostic(
                    "copy-field-not-copy",
                    format!(
                        "type `{target_key}` cannot implement `Copy` because field `{field_name}` has non-Copy type `{resolved}`"
                    ),
                    span.lo,
                    span.hi,
                ));
            }
        }

        Ok(())
    }

    fn type_is_copy_eligible(&self, ty: &Ty) -> bool {
        if ty.is_copy_value() {
            return true;
        }
        match &ty.kind {
            TyKind::Tuple(types) => types.iter().all(|ty| self.type_is_copy_eligible(ty)),
            TyKind::Array(elem, _) => self.type_is_copy_eligible(elem),
            TyKind::Adt { .. } => {
                let key = type_key(ty);
                !self.env.is_drop_owned_type(ty)
                    && self.impl_registry.implements_trait("Copy", &key)
            }
            _ => false,
        }
    }
}
