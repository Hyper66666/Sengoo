use super::*;

impl TypeChecker {
    pub(super) fn check_decl(&mut self, decl: &Decl) -> Result<()> {
        match &decl.kind {
            DeclKind::Function(fn_decl) => {
                self.check_function_decl(fn_decl)?;
            }
            DeclKind::ExternBlock(extern_block) => {
                self.check_extern_block_decl(extern_block)?;
            }
            DeclKind::Struct(struct_decl) => {
                self.check_struct_decl(struct_decl)?;
            }
            DeclKind::Enum(enum_decl) => {
                self.check_enum_decl(enum_decl)?;
            }
            DeclKind::Class(class_decl) => {
                self.check_class_decl(class_decl)?;
            }
            DeclKind::TypeAlias(type_alias) => {
                self.check_type_alias(type_alias)?;
            }
            DeclKind::Const(const_decl) => {
                self.check_const_decl(const_decl)?;
            }
            DeclKind::Static(static_decl) => {
                self.check_static_decl(static_decl)?;
            }
            DeclKind::Trait(trait_decl) => {
                self.check_trait_decl(trait_decl)?;
            }
            DeclKind::Impl(impl_decl) => {
                self.check_impl_decl(impl_decl)?;
            }
            DeclKind::Import(_) | DeclKind::Module(_) => {}
        }
        Ok(())
    }

    pub(super) fn check_decl_with_filtered_function_bodies(
        &mut self,
        decl: &Decl,
        checked_function_names: &HashSet<String>,
    ) -> Result<()> {
        match &decl.kind {
            DeclKind::Function(fn_decl) => {
                if checked_function_names.contains(&fn_decl.name.name) {
                    self.check_function_decl(fn_decl)?;
                } else {
                    self.check_function_signature_decl(fn_decl)?;
                }
            }
            _ => {
                self.check_decl(decl)?;
            }
        }
        Ok(())
    }

    pub(super) fn check_function_signature_decl(&mut self, fn_decl: &Function) -> Result<()> {
        self.env.push_scope();
        let signature = (|| -> Result<(Vec<Ty>, Ty, Vec<GenericTypeParamMeta>)> {
            let generic_meta = self.bind_type_params_with_meta(&fn_decl.type_params)?;

            let mut param_types = Vec::new();
            for param in &fn_decl.params {
                let ty = self.check_type(&param.ty).map_err(CompileError::from)?;
                self.env.insert_var_with_mutability(
                    param.name.name.clone(),
                    ty.clone(),
                    param.is_mut,
                );
                param_types.push(ty);
            }

            let ret_ty = if let Some(ret) = &fn_decl.return_type {
                self.check_type(ret).map_err(CompileError::from)?
            } else {
                self.env.unit_ty()
            };

            self.validate_contracts_for_function(fn_decl, &ret_ty)?;
            self.validate_ffi_function_decl(fn_decl, &param_types, &ret_ty)?;

            Ok((param_types, ret_ty, generic_meta))
        })();
        self.env.pop_scope();

        let (param_types, ret_ty, generic_meta) = signature?;
        self.env
            .declare_fn(fn_decl.name.name.clone(), param_types, ret_ty);
        self.set_generic_function_meta(fn_decl.name.name.clone(), generic_meta);
        Ok(())
    }

    pub(super) fn check_function_decl(&mut self, fn_decl: &Function) -> Result<()> {
        // Each function body is inferred with fresh, monotonically-increasing type
        // variable ids, so substitution entries from previously-checked functions
        // can never be looked up again. Dropping them here keeps `subst` bounded to
        // the current body instead of growing with the whole program — which made
        // every `unify`/checkpoint clone O(total vars) and the phase O(n²).
        self.infer.reset_subst();
        self.env.push_scope();
        let generic_meta = self.bind_type_params_with_meta(&fn_decl.type_params)?;

        let mut param_types = Vec::new();
        for param in &fn_decl.params {
            let ty = self.check_type(&param.ty)?;
            self.env
                .insert_var_with_mutability(param.name.name.clone(), ty.clone(), param.is_mut);
            param_types.push(ty);
        }

        let ret_ty = if let Some(ret) = &fn_decl.return_type {
            self.check_type(ret)?
        } else {
            self.env.unit_ty()
        };
        if Self::is_async_context_ty(&ret_ty) {
            return Err(CompileError::from(TypeckError::Other(
                "AsyncContext is poll-scoped and cannot be returned".to_string(),
            )));
        }
        self.validate_contracts_for_function(fn_decl, &ret_ty)?;
        self.validate_ffi_function_decl(fn_decl, &param_types, &ret_ty)?;

        self.env.declare_fn(
            fn_decl.name.name.clone(),
            param_types.clone(),
            ret_ty.clone(),
        );

        if fn_decl.is_async {
            self.async_functions.insert(fn_decl.name.name.clone());
        }

        let propagation = Self::propagation_from_ty(&ret_ty);
        let body_ty = if fn_decl.is_async {
            self.async_context_depth += 1;
            let result =
                self.with_propagation(propagation.clone(), |this| this.check_block(&fn_decl.body));
            self.async_context_depth = self.async_context_depth.saturating_sub(1);
            result?
        } else {
            self.with_propagation(propagation, |this| this.check_block(&fn_decl.body))?
        };

        let is_main_with_implicit_return = fn_decl.name.name == "main"
            && matches!(body_ty.kind, TyKind::Unit)
            && matches!(ret_ty.kind, TyKind::Int(_));

        if !is_main_with_implicit_return {
            self.infer
                .unify(&body_ty, &ret_ty)
                .map_err(CompileError::from)?;
        }

        self.env.pop_scope();

        self.env
            .declare_fn(fn_decl.name.name.clone(), param_types, ret_ty);
        self.set_generic_function_meta(fn_decl.name.name.clone(), generic_meta);

        Ok(())
    }

    pub(super) fn validate_ffi_function_decl(
        &mut self,
        fn_decl: &Function,
        param_types: &[Ty],
        ret_ty: &Ty,
    ) -> Result<()> {
        if fn_decl.abi.is_none() {
            if fn_decl.no_mangle || fn_decl.export_name.is_some() {
                return Err(CompileError::from(TypeckError::Other(
                    "no_mangle/export_name require extern \"...\" fn".to_string(),
                )));
            }
            return Ok(());
        }

        if !fn_decl.type_params.is_empty() {
            return Err(CompileError::from(TypeckError::ffi_signature(
                "ffi::generic_extern",
                "generic extern functions are not supported in FFI MVP",
                fn_decl.span.lo,
                fn_decl.span.hi,
            )));
        }

        let abi = fn_decl.abi.as_deref().unwrap_or("C");
        ffi_check::validate_signature(abi, param_types, ret_ty, fn_decl.is_unsafe)
            .map_err(CompileError::from)?;

        if fn_decl.export_name.is_some() && !matches!(fn_decl.vis, Visibility::Public) {
            return Err(CompileError::from(TypeckError::Other(
                "export_name requires pub extern function".to_string(),
            )));
        }

        Ok(())
    }

    pub(super) fn bind_type_params_with_meta(
        &mut self,
        type_params: &[TypeParam],
    ) -> Result<Vec<GenericTypeParamMeta>> {
        let mut metas = Vec::with_capacity(type_params.len());
        for type_param in type_params {
            let fresh_var = self.infer.fresh_ty_var();
            let var_id = match fresh_var.kind {
                TyKind::Var(id) => id,
                _ => {
                    return Err(CompileError::from(TypeckError::Other(
                        "internal error: expected fresh type variable".to_string(),
                    )))
                }
            };
            self.env
                .insert_type(type_param.name.name.clone(), fresh_var);
            metas.push(GenericTypeParamMeta {
                name: type_param.name.name.clone(),
                var_id,
                bounds: Vec::new(),
                default: None,
            });
        }

        for (type_param, meta) in type_params.iter().zip(metas.iter_mut()) {
            for bound in &type_param.bounds {
                let trait_name = bound
                    .path
                    .as_simple()
                    .map(|ident| ident.name.clone())
                    .ok_or_else(|| {
                        CompileError::from(TypeckError::Other(
                            "unsupported trait bound path in type parameter".to_string(),
                        ))
                    })?;
                if !matches!(
                    self.env.lookup(&trait_name).map(|symbol| &symbol.kind),
                    Some(SymbolKind::Trait { .. })
                ) {
                    return Err(CompileError::from(TypeckError::UndefinedType {
                        name: trait_name,
                    }));
                }
                meta.bounds.push(trait_name);
            }

            if let Some(default_ty) = &type_param.default {
                meta.default = Some(self.check_type(default_ty).map_err(CompileError::from)?);
            }

            self.generic_var_bounds
                .insert(meta.var_id, meta.bounds.clone());
        }

        Ok(metas)
    }

    pub(super) fn check_extern_block_decl(&mut self, extern_block: &ExternBlock) -> Result<()> {
        ffi_check::validate_abi(&extern_block.abi).map_err(CompileError::from)?;
        for item in &extern_block.items {
            match item {
                ExternItem::Function(fn_decl) => {
                    let mut param_types = Vec::new();
                    for param in &fn_decl.params {
                        param_types.push(self.check_type(&param.ty)?);
                    }
                    let ret_ty = if let Some(ret) = &fn_decl.return_type {
                        self.check_type(ret)?
                    } else {
                        self.env.unit_ty()
                    };
                    ffi_check::validate_signature(
                        &extern_block.abi,
                        &param_types,
                        &ret_ty,
                        fn_decl.is_unsafe,
                    )
                    .map_err(CompileError::from)?;
                }
                ExternItem::Static(static_decl) => {
                    self.check_type(&static_decl.ty)?;
                }
            }
        }

        Ok(())
    }

    pub(super) fn check_struct_decl(&mut self, struct_decl: &Struct) -> Result<()> {
        self.env.push_scope();
        let type_params = self.bind_type_params_with_meta(&struct_decl.type_params)?;
        let mut field_types = Vec::with_capacity(struct_decl.fields.len());

        for field in &struct_decl.fields {
            let field_ty = self.check_type(&field.ty)?;
            if Self::is_async_context_ty(&field_ty) {
                return Err(CompileError::from(TypeckError::Other(
                    "AsyncContext is poll-scoped and cannot be stored in a field".to_string(),
                )));
            }
            let field_name = field
                .name
                .as_ref()
                .map(|name| name.name.clone())
                .unwrap_or_default();
            field_types.push((field_name, field_ty));
        }

        self.env.pop_scope();
        self.env.register_struct_field_types(
            struct_decl.name.name.clone(),
            type_params.into_iter().map(|param| param.var_id).collect(),
            field_types,
        );
        Ok(())
    }

    pub(super) fn check_enum_decl(&mut self, enum_decl: &Enum) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&enum_decl.type_params)?;
        for variant in &enum_decl.variants {
            for field in &variant.fields {
                match field {
                    VariantField::Named(_, ty) => {
                        self.check_type(ty)?;
                    }
                    VariantField::Unnamed(ty) => {
                        self.check_type(ty)?;
                    }
                }
            }
        }
        self.env.pop_scope();
        Ok(())
    }

    pub(super) fn check_class_decl(&mut self, class_decl: &Class) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&class_decl.type_params)?;

        for member in &class_decl.members {
            match member {
                ClassMember::Field(field) => {
                    self.check_type(&field.ty)?;
                }
                ClassMember::Method(method) => {
                    self.check_class_method_decl(&class_decl.name.name, method)?;
                }
            }
        }

        self.env.pop_scope();
        Ok(())
    }

    pub(super) fn check_class_method_decl(
        &mut self,
        class_name: &str,
        method: &Function,
    ) -> Result<()> {
        // See `check_function_decl`: method bodies likewise allocate fresh type
        // variables, so resetting the substitution per body keeps it bounded.
        self.infer.reset_subst();
        self.env.push_scope();
        self.bind_type_params_with_meta(&method.type_params)?;

        if let Some(self_param) = method.self_param {
            let self_ty = self
                .env
                .lookup(class_name)
                .and_then(|symbol| symbol.get_ty())
                .cloned()
                .unwrap_or_else(|| {
                    self.env.new_ty(TyKind::Adt {
                        name: class_name.to_string(),
                        args: vec![],
                    })
                });
            self.env
                .insert_var_with_mutability("self".to_string(), self_ty, self_param.is_mut());
        }

        for param in &method.params {
            let ty = self.check_type(&param.ty)?;
            self.env
                .insert_var_with_mutability(param.name.name.clone(), ty, param.is_mut);
        }

        let ret_ty = if let Some(ret) = &method.return_type {
            self.check_type(ret)?
        } else {
            self.env.unit_ty()
        };

        let body_ty = self.check_block(&method.body)?;
        self.infer
            .unify(&body_ty, &ret_ty)
            .map_err(CompileError::from)?;

        self.env.pop_scope();
        Ok(())
    }

    pub(super) fn check_type_alias(&mut self, type_alias: &TypeAlias) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&type_alias.type_params)?;
        self.check_type(&type_alias.ty)?;
        self.env.pop_scope();
        Ok(())
    }

    pub(super) fn check_const_decl(&mut self, const_decl: &Const) -> Result<()> {
        let ty = self.check_type(&const_decl.ty)?;
        let value_ty = self.check_expr(&const_decl.value)?;
        self.infer
            .unify(&ty, &value_ty)
            .map_err(CompileError::from)?;
        Ok(())
    }

    pub(super) fn check_static_decl(&mut self, static_decl: &Static) -> Result<()> {
        let ty = self.check_type(&static_decl.ty)?;
        let value_ty = self.check_expr(&static_decl.value)?;
        self.infer
            .unify(&ty, &value_ty)
            .map_err(CompileError::from)?;
        Ok(())
    }
}
