use super::*;
use crate::typeck::r#trait::{type_key, FunctionTy, ImplInfo, MethodSig, TraitInfo};

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

    /// Enforce the compiler-known `Drop` trait contract: `def drop(&mut self)`.
    /// `drop` is compiler-inserted, so its only method must take `&mut self` and
    /// accept no other parameters; a malformed signature is rejected here rather
    /// than producing surprising drop glue later.
    fn validate_drop_contract(method: &Function) -> Result<()> {
        if method.name.name != "drop" {
            return Ok(());
        }

        if !matches!(method.self_param, Some(SelfParam::BorrowedMut)) {
            return Err(CompileError::from(TypeckError::Other(
                "Drop::drop must use `&mut self` receiver".to_string(),
            )));
        }

        let extra_params = method
            .params
            .iter()
            .filter(|param| param.name.name != "self")
            .count();
        if extra_params != 0 {
            return Err(CompileError::from(TypeckError::Other(
                "Drop::drop must take no parameters other than `&mut self`".to_string(),
            )));
        }

        Ok(())
    }

    /// Whether `ty` is the compiler-known owned `String` type.
    fn is_owned_string_ty(ty: &Ty) -> bool {
        matches!(&ty.kind, TyKind::Adt { name, .. } if name == "String")
    }

    pub(super) fn check_trait_decl(&mut self, trait_decl: &Trait) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&trait_decl.type_params)?;

        let mut trait_info = TraitInfo::new(
            trait_decl.name.name.clone(),
            trait_decl
                .type_params
                .iter()
                .map(|tp| tp.name.name.clone())
                .collect(),
            matches!(trait_decl.vis, Visibility::Public),
        );

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

        self.trait_registry.register(trait_info);

        self.env.pop_scope();
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
        if let Some(name) = trait_name.as_deref() {
            if let Err(err) = self.validate_orphan_rule(name, &target_ty, impl_decl.span) {
                self.env.pop_scope();
                return Err(CompileError::from(err));
            }
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
            }
            if is_drop_impl {
                Self::validate_drop_contract(item)?;
            }

            self.env.push_scope();
            let method_generic_meta = self.bind_type_params_with_meta(&item.type_params)?;
            let mut param_types = Vec::new();
            let mut has_self = false;
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

        if is_display_impl {
            let to_string = impl_decl
                .items
                .iter()
                .find(|method| method.name.name == "to_string");
            let contract_ok = match to_string {
                Some(method) => {
                    let has_self = method.self_param.is_some();
                    let returns_string = match &method.return_type {
                        Some(ret) => Self::is_owned_string_ty(&self.check_type(ret)?),
                        None => false,
                    };
                    has_self && returns_string
                }
                None => false,
            };
            if !contract_ok {
                self.env.pop_scope();
                return Err(CompileError::from(TypeckError::diagnostic(
                    "display-contract",
                    format!(
                        "impl Display for {target_key} must define `def to_string(&self) -> String`"
                    ),
                    impl_decl.span.lo,
                    impl_decl.span.hi,
                )));
            }
        }

        if let Some(trait_name) = impl_info.trait_name.clone() {
            if let Some(trait_info) = self.trait_registry.get(&trait_name) {
                let mut missing_methods = Vec::new();

                for (method_name, method_sig) in &trait_info.methods {
                    if !impl_info.has_method(method_name) {
                        if method_sig.has_default {
                            impl_info.add_method(
                                method_name.clone(),
                                FunctionTy::with_generic_params(
                                    method_sig.has_self,
                                    method_sig.param_types.clone(),
                                    method_sig.return_type.clone(),
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

            if self
                .impl_registry
                .get_trait_impl(&trait_name, &target_key)
                .is_some()
            {
                self.env.pop_scope();
                return Err(CompileError::from(TypeckError::diagnostic(
                    "conflicting-impl",
                    format!(
                        "conflicting implementations of trait `{trait_name}` for type `{target_key}`"
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
