use super::*;
use crate::typeck::r#trait::{type_key, FunctionTy, ImplInfo};

impl TypeChecker {
    pub(super) fn prepare_class_hierarchy(&mut self, program: &Program) -> Result<()> {
        self.class_decls.clear();
        self.collect_class_decls(program)?;

        for decl in program.decls.iter() {
            if let DeclKind::Trait(trait_decl) = &decl.kind {
                self.check_trait_decl(trait_decl)?;
            }
        }

        self.resolve_class_headers(program)?;
        self.validate_class_parent_targets()?;
        self.validate_class_cycles()?;
        self.register_class_header_traits(program)?;

        let mut class_names: Vec<String> = self.class_decls.keys().cloned().collect();
        class_names.sort();

        let mut field_cache: HashMap<String, Vec<(String, Type)>> = HashMap::new();
        for class_name in &class_names {
            let mut stack = HashSet::new();
            let fields = self
                .resolve_class_fields_for(class_name, &mut field_cache, &mut stack)
                .map_err(CompileError::from)?;
            self.struct_field_defs.insert(class_name.clone(), fields);
        }

        let mut method_cache: HashMap<String, HashMap<String, Function>> = HashMap::new();
        for class_name in class_names {
            let mut stack = HashSet::new();
            let methods = self
                .resolve_class_methods_for(&class_name, &mut method_cache, &mut stack)
                .map_err(CompileError::from)?;

            let target_ty = self
                .env
                .lookup(&class_name)
                .and_then(|symbol| symbol.get_ty())
                .cloned()
                .unwrap_or_else(|| {
                    self.env.new_ty(TyKind::Adt {
                        name: class_name.clone(),
                        args: vec![],
                    })
                });

            let mut impl_info =
                crate::typeck::r#trait::ImplInfo::new(target_ty.clone(), None, Vec::new());
            let mut method_names: Vec<String> = methods.keys().cloned().collect();
            method_names.sort();

            for method_name in method_names {
                if let Some(method) = methods.get(&method_name) {
                    let fn_ty = self
                        .class_method_signature(method)
                        .map_err(CompileError::from)?;
                    impl_info.add_method(method_name, fn_ty);
                }
            }

            self.impl_registry
                .register_inherent(type_key(&target_ty), impl_info);
        }

        Ok(())
    }

    fn collect_class_decls(&mut self, program: &Program) -> Result<()> {
        for decl in &program.decls {
            let DeclKind::Class(class_decl) = &decl.kind else {
                continue;
            };

            let parent = class_decl.extends.as_ref().and_then(|path| {
                path.as_simple()
                    .map(|ident| ident.name.clone())
                    .or_else(|| path.segments.last().map(|ident| ident.name.clone()))
            });

            let mut fields = Vec::new();
            let mut methods = Vec::new();

            for (field_index, member) in class_decl.members.iter().enumerate() {
                match member {
                    ClassMember::Field(field) => {
                        let field_name = field
                            .name
                            .as_ref()
                            .map(|ident| ident.name.clone())
                            .unwrap_or_else(|| format!("_{}", field_index));
                        fields.push((field_name, field.ty.clone()));
                    }
                    ClassMember::Method(method) => {
                        methods.push(method.clone());
                    }
                }
            }

            let header_traits = class_decl
                .implements
                .iter()
                .filter_map(|bound| bound.path.as_simple().map(|ident| ident.name.clone()))
                .collect::<Vec<_>>();

            self.class_decls.insert(
                class_decl.name.name.clone(),
                ClassDeclInfo {
                    parent,
                    header_traits,
                    fields,
                    methods,
                },
            );
        }

        Ok(())
    }

    fn is_known_trait_name(&self, name: &str) -> bool {
        if self.trait_registry.get(name).is_some() {
            return true;
        }
        self.env
            .lookup(name)
            .is_some_and(|symbol| matches!(symbol.kind, SymbolKind::Trait { .. }))
    }

    fn path_simple_name(path: &crate::ast::Path) -> Option<String> {
        path.as_simple()
            .map(|ident| ident.name.clone())
            .or_else(|| path.segments.last().map(|ident| ident.name.clone()))
    }

    fn resolve_class_headers(&mut self, program: &Program) -> Result<()> {
        for decl in &program.decls {
            let DeclKind::Class(class_decl) = &decl.kind else {
                continue;
            };

            let class_name = class_decl.name.name.clone();
            let mut paths = Vec::new();
            if let Some(path) = &class_decl.extends {
                if let Some(name) = Self::path_simple_name(path) {
                    paths.push(name);
                }
            }
            paths.extend(
                class_decl
                    .implements
                    .iter()
                    .filter_map(|bound| Self::path_simple_name(&bound.path)),
            );

            if paths.is_empty() {
                continue;
            }

            let first = paths[0].clone();
            let first_is_trait = self.is_known_trait_name(&first);
            let first_is_class = self.class_decls.contains_key(&first);

            if !first_is_trait && !first_is_class {
                return Err(CompileError::TypeckError(TypeckError::Other(format!(
                    "invalid class header: `{first}` is not a known class or trait"
                ))));
            }

            let (parent, traits) = if first_is_trait {
                for path in &paths {
                    if self.class_decls.contains_key(path) {
                        return Err(CompileError::TypeckError(TypeckError::Other(
                            "invalid class header: class base must come before traits".to_string(),
                        )));
                    }
                    if !self.is_known_trait_name(path) {
                        return Err(CompileError::TypeckError(TypeckError::Other(format!(
                            "invalid class header: `{path}` is not a known trait"
                        ))));
                    }
                }
                (None, paths)
            } else {
                let mut traits = Vec::new();
                for path in paths.iter().skip(1) {
                    if self.class_decls.contains_key(path) {
                        return Err(CompileError::TypeckError(TypeckError::Other(format!(
                            "invalid class header: only one class base is allowed (`{path}`)"
                        ))));
                    }
                    if !self.is_known_trait_name(path) {
                        return Err(CompileError::TypeckError(TypeckError::Other(format!(
                            "invalid class header: `{path}` is not a known trait"
                        ))));
                    }
                    traits.push(path.clone());
                }
                (Some(first), traits)
            };

            if let Some(info) = self.class_decls.get_mut(&class_name) {
                info.parent = parent;
                info.header_traits = traits;
            }
        }

        Ok(())
    }

    fn register_class_header_traits(&mut self, program: &Program) -> Result<()> {
        for decl in &program.decls {
            let DeclKind::Class(class_decl) = &decl.kind else {
                continue;
            };

            let class_name = class_decl.name.name.clone();
            let Some(class_info) = self.class_decls.get(&class_name).cloned() else {
                continue;
            };

            let target_ty = self
                .env
                .lookup(&class_name)
                .and_then(|symbol| symbol.get_ty())
                .cloned()
                .unwrap_or_else(|| {
                    self.env.new_ty(TyKind::Adt {
                        name: class_name.clone(),
                        args: vec![],
                    })
                });
            let target_key = type_key(&target_ty);

            for trait_name in &class_info.header_traits {
                if self.impl_registry.implements_trait(trait_name, &target_key) {
                    continue;
                }

                let mut impl_info =
                    ImplInfo::new(target_ty.clone(), Some(trait_name.clone()), Vec::new());
                if let Some(trait_info) = self.trait_registry.get(trait_name) {
                    for (method_name, method_sig) in &trait_info.methods {
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
                        }
                    }
                }

                self.impl_registry.register_trait_impl(
                    trait_name.clone(),
                    target_key.clone(),
                    impl_info,
                );
            }
        }

        Ok(())
    }

    fn validate_class_parent_targets(&self) -> Result<()> {
        for (class_name, class_info) in &self.class_decls {
            if let Some(parent) = &class_info.parent {
                if !self.class_decls.contains_key(parent) {
                    return Err(CompileError::TypeckError(TypeckError::Other(format!(
                        "class `{}` has unknown parent class `{}`",
                        class_name, parent
                    ))));
                }
            }
        }

        Ok(())
    }

    fn validate_class_cycles(&self) -> Result<()> {
        let mut state: HashMap<String, u8> = HashMap::new();
        let mut stack = Vec::new();
        let mut class_names: Vec<String> = self.class_decls.keys().cloned().collect();
        class_names.sort();

        for class_name in class_names {
            self.detect_class_cycle(&class_name, &mut state, &mut stack)
                .map_err(CompileError::from)?;
        }

        Ok(())
    }

    fn detect_class_cycle(
        &self,
        class_name: &str,
        state: &mut HashMap<String, u8>,
        stack: &mut Vec<String>,
    ) -> TyResult<()> {
        match state.get(class_name).copied() {
            Some(2) => return Ok(()),
            Some(1) => {
                let cycle_start = stack
                    .iter()
                    .position(|name| name == class_name)
                    .unwrap_or(0);
                let mut cycle: Vec<String> = stack[cycle_start..].to_vec();
                cycle.push(class_name.to_string());
                return Err(TypeckError::Other(format!(
                    "cyclic class inheritance detected: {}",
                    cycle.join(" -> ")
                )));
            }
            _ => {}
        }

        state.insert(class_name.to_string(), 1);
        stack.push(class_name.to_string());

        if let Some(parent) = self
            .class_decls
            .get(class_name)
            .and_then(|class_info| class_info.parent.as_ref())
        {
            self.detect_class_cycle(parent, state, stack)?;
        }

        stack.pop();
        state.insert(class_name.to_string(), 2);
        Ok(())
    }

    fn resolve_class_fields_for(
        &self,
        class_name: &str,
        cache: &mut HashMap<String, Vec<(String, Type)>>,
        stack: &mut HashSet<String>,
    ) -> TyResult<Vec<(String, Type)>> {
        if let Some(cached) = cache.get(class_name) {
            return Ok(cached.clone());
        }

        if !stack.insert(class_name.to_string()) {
            return Err(TypeckError::Other(format!(
                "cyclic class inheritance detected near `{}`",
                class_name
            )));
        }

        let class_info = self.class_decls.get(class_name).ok_or_else(|| {
            TypeckError::Other(format!(
                "internal error: class `{}` not collected",
                class_name
            ))
        })?;

        let mut merged = Vec::new();
        let mut seen = HashSet::new();

        if let Some(parent) = &class_info.parent {
            let parent_fields = self.resolve_class_fields_for(parent, cache, stack)?;
            for (field_name, field_ty) in parent_fields {
                seen.insert(field_name.clone());
                merged.push((field_name, field_ty));
            }
        }

        for (field_name, field_ty) in &class_info.fields {
            if !seen.insert(field_name.clone()) {
                return Err(TypeckError::Other(format!(
                    "duplicate inherited field `{}` in class `{}`",
                    field_name, class_name
                )));
            }
            merged.push((field_name.clone(), field_ty.clone()));
        }

        stack.remove(class_name);
        cache.insert(class_name.to_string(), merged.clone());
        Ok(merged)
    }

    fn resolve_class_methods_for(
        &self,
        class_name: &str,
        cache: &mut HashMap<String, HashMap<String, Function>>,
        stack: &mut HashSet<String>,
    ) -> TyResult<HashMap<String, Function>> {
        if let Some(cached) = cache.get(class_name) {
            return Ok(cached.clone());
        }

        if !stack.insert(class_name.to_string()) {
            return Err(TypeckError::Other(format!(
                "cyclic class inheritance detected near `{}`",
                class_name
            )));
        }

        let class_info = self.class_decls.get(class_name).ok_or_else(|| {
            TypeckError::Other(format!(
                "internal error: class `{}` not collected",
                class_name
            ))
        })?;

        let mut resolved = HashMap::new();
        if let Some(parent) = &class_info.parent {
            resolved = self.resolve_class_methods_for(parent, cache, stack)?;
        }

        let mut local_seen = HashSet::new();
        for method in &class_info.methods {
            let method_name = method.name.name.clone();
            if !local_seen.insert(method_name.clone()) {
                return Err(TypeckError::Other(format!(
                    "duplicate method `{}` in class `{}`",
                    method_name, class_name
                )));
            }
            resolved.insert(method_name, method.clone());
        }

        for trait_name in &class_info.header_traits {
            if let Some(defaults) = self.trait_default_methods.get(trait_name) {
                for (method_name, method) in defaults {
                    resolved
                        .entry(method_name.clone())
                        .or_insert_with(|| method.clone());
                }
            }
        }

        stack.remove(class_name);
        cache.insert(class_name.to_string(), resolved.clone());
        Ok(resolved)
    }

    fn class_method_signature(&mut self, method: &Function) -> TyResult<FunctionTy> {
        self.env.push_scope();
        if let Err(err) = self.bind_type_params_with_meta(&method.type_params) {
            self.env.pop_scope();
            return Err(TypeckError::Other(err.to_string()));
        }

        let mut param_types = Vec::new();
        for param in &method.params {
            param_types.push(self.check_type(&param.ty)?);
        }

        let ret_ty = if let Some(ret) = &method.return_type {
            self.check_type(ret)?
        } else {
            self.env.unit_ty()
        };

        let sig = FunctionTy::new(method.self_param.is_some(), param_types, ret_ty);
        self.env.pop_scope();
        Ok(sig)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_class_cycle_reports_simple_two_node_cycle() {
        let mut checker = TypeChecker::new();
        checker.class_decls.insert(
            "A".to_string(),
            ClassDeclInfo {
                parent: Some("B".to_string()),
                header_traits: Vec::new(),
                fields: Vec::new(),
                methods: Vec::new(),
            },
        );
        checker.class_decls.insert(
            "B".to_string(),
            ClassDeclInfo {
                parent: Some("A".to_string()),
                header_traits: Vec::new(),
                fields: Vec::new(),
                methods: Vec::new(),
            },
        );

        let mut state = HashMap::new();
        let mut stack = Vec::new();
        let err = checker
            .detect_class_cycle("A", &mut state, &mut stack)
            .unwrap_err();
        assert!(
            matches!(err, TypeckError::Other(msg) if msg.contains("cyclic class inheritance detected"))
        );
    }
}
