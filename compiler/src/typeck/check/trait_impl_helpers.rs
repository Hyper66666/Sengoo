use super::*;
use crate::typeck::r#trait::{type_key, FunctionTy, ImplInfo, MethodSig, TraitInfo};

impl TypeChecker {
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

        for item in &trait_decl.items {
            match item {
                TraitItem::Function(method) => {
                    if trait_decl.name.name == "Future" {
                        Self::validate_future_poll_contract(method)?;
                    }

                    self.env.push_scope();
                    let method_generic_meta =
                        self.bind_type_params_with_meta(&method.type_params)?;
                    let mut param_types = Vec::new();
                    let mut has_self = false;

                    for param in &method.params {
                        if param.name.name == "self" {
                            has_self = true;
                        } else {
                            let ty = self.check_type(&param.ty)?;
                            param_types.push(ty);
                        }
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

        let conflicts_with_drop = is_copy_impl
            && Self::ownership_type_set_contains(&self.drop_move_only_type_keys, &target_ty);
        let conflicts_with_copy =
            is_drop_impl && Self::ownership_type_set_contains(&self.copy_type_keys, &target_ty);
        if conflicts_with_drop || conflicts_with_copy {
            self.env.pop_scope();
            return Err(CompileError::from(TypeckError::diagnostic(
                "copy-drop-conflict",
                format!("type `{target_key}` cannot implement both `Copy` and `Drop`"),
                impl_decl.span.lo,
                impl_decl.span.hi,
            )));
        }
        if is_copy_impl {
            if let Err(err) = self.validate_copy_fields(&target_ty) {
                self.env.pop_scope();
                return match err {
                    TypeckError::Other(message) => {
                        Err(CompileError::from(TypeckError::diagnostic(
                            "copy-field-not-copy",
                            message,
                            impl_decl.span.lo,
                            impl_decl.span.hi,
                        )))
                    }
                    err => Err(CompileError::TypeckError(err)),
                };
            }
        }

        let mut impl_info = ImplInfo::new(target_ty.clone(), trait_name, trait_args);

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
            }

            self.impl_registry
                .register_trait_impl(trait_name, target_key, impl_info);
        } else {
            self.impl_registry.register_inherent(target_key, impl_info);
        }

        self.env.pop_scope();
        Ok(())
    }

    fn validate_copy_fields(&mut self, target_ty: &Ty) -> TyResult<()> {
        let TyKind::Adt { name, .. } = &target_ty.kind else {
            return Ok(());
        };
        let Some(field_defs) = self.struct_field_defs.get(name).cloned() else {
            return Ok(());
        };

        for (field_name, field_ty) in field_defs {
            let field_ty = self.check_type(&field_ty)?;
            if !self.ty_is_copy(&field_ty, &mut HashSet::new())? {
                return Err(TypeckError::Other(format!(
                    "type `{}` cannot implement `Copy` because field `{}` has non-Copy type {}",
                    type_key(target_ty),
                    field_name,
                    field_ty.kind
                )));
            }
        }
        Ok(())
    }

    fn ty_is_copy(&mut self, ty: &Ty, visiting: &mut HashSet<String>) -> TyResult<bool> {
        match &ty.kind {
            TyKind::Error
            | TyKind::Unit
            | TyKind::Never
            | TyKind::Bool
            | TyKind::Int(_)
            | TyKind::Float(_)
            | TyKind::Char
            | TyKind::Byte
            | TyKind::Str
            | TyKind::Ref(_, _)
            | TyKind::Ptr(_)
            | TyKind::Fn { .. } => Ok(true),
            TyKind::Tuple(items) => {
                for item in items {
                    if !self.ty_is_copy(item, visiting)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            TyKind::Array(item, _) | TyKind::Slice(item) => self.ty_is_copy(item, visiting),
            TyKind::Adt { name, .. } => {
                if Self::ownership_type_set_contains(&self.copy_type_keys, ty) {
                    return Ok(true);
                }
                if self.drop_move_only_type_keys.contains(name) {
                    return Ok(false);
                }

                let ty_key = type_key(ty);
                if !visiting.insert(ty_key.clone()) {
                    return Ok(true);
                }
                let Some(field_defs) = self.struct_field_defs.get(name).cloned() else {
                    visiting.remove(&ty_key);
                    return Ok(false);
                };
                for (_, field_ty) in field_defs {
                    let field_ty = self.check_type(&field_ty)?;
                    if !self.ty_is_copy(&field_ty, visiting)? {
                        visiting.remove(&ty_key);
                        return Ok(false);
                    }
                }
                visiting.remove(&ty_key);
                Ok(false)
            }
            TyKind::Bytes
            | TyKind::Var(_)
            | TyKind::Dyn(_)
            | TyKind::ImplTrait(_)
            | TyKind::Future(_)
            | TyKind::SelfType
            | TyKind::Inferred => Ok(false),
        }
    }
}
