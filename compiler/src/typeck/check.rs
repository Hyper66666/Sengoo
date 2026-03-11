//! 类型检查器实现，负责对AST进行类型验证、推断和约束检查。
//! 实现了Sengoo语言的类型系统，包括泛型、trait约束、类型转换等核心功能。
use crate::ast::pattern::Pattern;
use crate::ast::Visibility;
use crate::ast::*;
use crate::error::CompileError;
use crate::method_resolution::{
    ambiguous_method_error, select_method_candidate, MethodCandidate, MethodCandidateMatch,
};
use crate::typeck::env::{Symbol, SymbolKind, TypeEnv};
use crate::typeck::ffi as ffi_check;
use crate::typeck::infer::TypeInfer;
use crate::typeck::r#trait::{type_key, FunctionTy, ImplRegistry, TraitRegistry};
use crate::typeck::ty::{FloatKind, IntKind, Ty, TyKind, TyVarId, TypeckError};
use crate::Result;
use std::collections::{HashMap, HashSet};

type TyResult<T> = std::result::Result<T, TypeckError>;

#[derive(Debug, Clone)]
struct ClassDeclInfo {
    parent: Option<String>,
    fields: Vec<(String, Type)>,
    methods: Vec<Function>,
}

#[derive(Debug, Clone)]
struct GenericTypeParamMeta {
    name: String,
    var_id: TyVarId,
    bounds: Vec<String>,
    default: Option<Ty>,
}

#[derive(Debug, Clone)]
struct GenericFunctionMeta {
    params: Vec<GenericTypeParamMeta>,
}

#[derive(Debug, Clone)]
struct GenericTypeMeta {
    params: Vec<GenericTypeParamMeta>,
}

/// 类型检查器，负责对AST进行类型验证、推断和约束检查。
pub struct TypeChecker {
    /// 类型环境，存储变量和类型的绑定关系。
    env: TypeEnv,
    /// 类型推断器，用于类型变量的推断和统一。
    infer: TypeInfer,
    /// Trait注册表，存储所有已声明的trait信息。
    trait_registry: TraitRegistry,
    /// Impl注册表，存储所有trait实现的信息。
    impl_registry: ImplRegistry,
    struct_field_defs: HashMap<String, Vec<(String, Type)>>,
    struct_type_params: HashMap<String, Vec<TypeParam>>,
    class_decls: HashMap<String, ClassDeclInfo>,
    generic_function_metas: HashMap<String, GenericFunctionMeta>,
    generic_type_metas: HashMap<String, GenericTypeMeta>,
    async_context_depth: usize,
    async_functions: HashSet<String>,
}

impl TypeChecker {
    pub fn new() -> Self {
        let env = TypeEnv::new();
        let infer = TypeInfer::with_env(env.clone());
        Self {
            env,
            infer,
            trait_registry: TraitRegistry::new(),
            impl_registry: ImplRegistry::new(),
            struct_field_defs: HashMap::new(),
            struct_type_params: HashMap::new(),
            class_decls: HashMap::new(),
            generic_function_metas: HashMap::new(),
            generic_type_metas: HashMap::new(),
            async_context_depth: 0,
            async_functions: HashSet::new(),
        }
    }

    pub fn async_function_names(&self) -> &HashSet<String> {
        &self.async_functions
    }

    /// 返回类型环境的不可变引用。
    pub fn env(&self) -> &TypeEnv {
        &self.env
    }

    /// Consumes the checker and returns the owned type environment.
    pub fn into_env(self) -> TypeEnv {
        self.env
    }

    /// 返回类型推断器的不可变引用。
    pub fn infer(&self) -> &TypeInfer {
        &self.infer
    }

    /// 返回trait注册表的不可变引用。
    pub fn trait_registry(&self) -> &TraitRegistry {
        &self.trait_registry
    }

    /// 返回impl注册表的不可变引用。
    pub fn impl_registry(&self) -> &ImplRegistry {
        &self.impl_registry
    }

    /// 返回trait注册表的可变引用。
    pub fn trait_registry_mut(&mut self) -> &mut TraitRegistry {
        &mut self.trait_registry
    }

    /// 返回impl注册表的可变引用。
    pub fn impl_registry_mut(&mut self) -> &mut ImplRegistry {
        &mut self.impl_registry
    }

    /// 对整个程序进行类型检查，包括声明预处理和全量检查。
    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        self.generic_function_metas.clear();
        self.generic_type_metas.clear();
        for decl in &program.decls {
            self.declare_decl(decl)?;
        }

        self.prepare_class_hierarchy(program)?;

        for decl in &program.decls {
            self.check_decl(decl)?;
        }

        Ok(())
    }

    pub fn check_program_with_filtered_function_bodies(
        &mut self,
        program: &Program,
        checked_function_names: &HashSet<String>,
    ) -> Result<()> {
        self.generic_function_metas.clear();
        self.generic_type_metas.clear();
        for decl in &program.decls {
            self.declare_decl(decl)?;
        }

        self.prepare_class_hierarchy(program)?;

        for decl in &program.decls {
            self.check_decl_with_filtered_function_bodies(decl, checked_function_names)?;
        }

        Ok(())
    }

    /// 预声明顶层声明（函数、结构体、枚举等），将其类型信息注册到环境中。
    fn declare_decl(&mut self, decl: &Decl) -> Result<()> {
        match &decl.kind {
            DeclKind::Function(fn_decl) => {
                let name = fn_decl.name.name.clone();

                if fn_decl.abi.is_some() {
                    let mut param_types = Vec::new();
                    for param in &fn_decl.params {
                        let ty = self.check_type(&param.ty)?;
                        param_types.push(ty);
                    }
                    let ret_ty = if let Some(ret) = &fn_decl.return_type {
                        self.check_type(ret)?
                    } else {
                        self.env.unit_ty()
                    };
                    self.validate_ffi_function_decl(fn_decl, &param_types, &ret_ty)?;

                    let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
                    self.env.insert_fn(name.clone(), fn_ty, param_types, ret_ty);
                    self.set_generic_function_meta(name, Vec::new());
                    return Ok(());
                }

                // 处理泛型函数参数类型绑定，进行类型检查。
                let mut param_types = Vec::new();
                let mut fallback = false;
                let mut generic_meta = Vec::new();
                self.env.push_scope();
                match self.bind_type_params_with_meta(&fn_decl.type_params) {
                    Ok(meta) => {
                        generic_meta = meta;
                    }
                    Err(_) => {
                        fallback = true;
                    }
                }
                if !fallback {
                    for param in &fn_decl.params {
                        match self.check_type(&param.ty) {
                            Ok(ty) => param_types.push(ty),
                            Err(_) => {
                                fallback = true;
                                break;
                            }
                        }
                    }
                }

                if fallback {
                    self.env.pop_scope();
                    // 泛型参数绑定失败时回退处理。
                    let unit = self.env.unit_ty();
                    let ty = self.env.fn_ty(vec![], unit.clone());
                    self.env.insert_fn(name.clone(), ty, vec![], unit);
                    self.set_generic_function_meta(name, Vec::new());
                } else {
                    let ret_ty = if let Some(ret) = &fn_decl.return_type {
                        self.check_type(ret).unwrap_or_else(|_| self.env.unit_ty())
                    } else {
                        self.env.unit_ty()
                    };
                    self.env.pop_scope();

                    let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
                    self.env.insert_fn(name.clone(), fn_ty, param_types, ret_ty);
                    self.set_generic_function_meta(name, generic_meta);
                }
            }
            DeclKind::ExternBlock(extern_block) => {
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
                            let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
                            self.env.insert_fn(
                                fn_decl.name.name.clone(),
                                fn_ty,
                                param_types,
                                ret_ty,
                            );
                        }
                        ExternItem::Static(static_decl) => {
                            let ty = self.check_type(&static_decl.ty)?;
                            self.env.insert_var(static_decl.name.name.clone(), ty);
                        }
                    }
                }
            }
            DeclKind::Struct(struct_decl) => {
                let name = struct_decl.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name, ty);
                let type_meta = self.collect_generic_type_meta(&struct_decl.type_params);
                self.set_generic_type_meta(struct_decl.name.name.clone(), type_meta);
                let fields = struct_decl
                    .fields
                    .iter()
                    .map(|field| {
                        let field_name = field
                            .name
                            .as_ref()
                            .map(|ident| ident.name.clone())
                            .unwrap_or_default();
                        (field_name, field.ty.clone())
                    })
                    .collect::<Vec<_>>();
                self.struct_field_defs
                    .insert(struct_decl.name.name.clone(), fields);
                self.struct_type_params
                    .insert(struct_decl.name.name.clone(), struct_decl.type_params.clone());
            }
            DeclKind::Enum(enum_decl) => {
                let name = enum_decl.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name, ty);
                let type_meta = self.collect_generic_type_meta(&enum_decl.type_params);
                self.set_generic_type_meta(enum_decl.name.name.clone(), type_meta);
            }
            DeclKind::Class(class_decl) => {
                let name = class_decl.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name, ty);
                let type_meta = self.collect_generic_type_meta(&class_decl.type_params);
                self.set_generic_type_meta(class_decl.name.name.clone(), type_meta);
            }
            DeclKind::TypeAlias(type_alias) => {
                let name = type_alias.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name, ty);
                let type_meta = self.collect_generic_type_meta(&type_alias.type_params);
                self.set_generic_type_meta(type_alias.name.name.clone(), type_meta);
            }
            DeclKind::Const(const_decl) => {
                let name = const_decl.name.name.clone();
                let ty = self.env.error_ty();
                self.env.insert_var(name, ty);
            }
            DeclKind::Static(static_decl) => {
                let name = static_decl.name.name.clone();
                let ty = self.env.error_ty();
                self.env.insert_var(name, ty);
            }
            DeclKind::Trait(trait_decl) => {
                let name = trait_decl.name.name.clone();
                let symbol = Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Trait { name },
                };
                self.env.insert(trait_decl.name.name.clone(), symbol);
            }
            DeclKind::Impl(_impl_decl) => {}
            DeclKind::Import(_import_decl) => {}
            DeclKind::Module(module_decl) => {
                let name = module_decl.name.name.clone();
                let symbol = Symbol {
                    name: name.clone(),
                    kind: SymbolKind::Module { name },
                };
                self.env.insert(module_decl.name.name.clone(), symbol);
            }
        }
        Ok(())
    }

    fn set_generic_function_meta(&mut self, name: String, params: Vec<GenericTypeParamMeta>) {
        if params.is_empty() {
            self.generic_function_metas.remove(&name);
        } else {
            self.generic_function_metas
                .insert(name, GenericFunctionMeta { params });
        }
    }

    fn set_generic_type_meta(&mut self, name: String, params: Vec<GenericTypeParamMeta>) {
        if params.is_empty() {
            self.generic_type_metas.remove(&name);
        } else {
            self.generic_type_metas
                .insert(name, GenericTypeMeta { params });
        }
    }

    fn collect_generic_type_meta(
        &mut self,
        type_params: &[TypeParam],
    ) -> Vec<GenericTypeParamMeta> {
        if type_params.is_empty() {
            return Vec::new();
        }
        self.env.push_scope();
        let result = self.bind_type_params_with_meta(type_params);
        self.env.pop_scope();
        result.unwrap_or_default()
    }

    /// 对单个顶层声明进行类型检查。
    fn check_decl(&mut self, decl: &Decl) -> Result<()> {
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

    fn check_decl_with_filtered_function_bodies(
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

    fn prepare_class_hierarchy(&mut self, program: &Program) -> Result<()> {
        self.class_decls.clear();
        self.collect_class_decls(program)?;
        self.validate_class_parent_targets()?;
        self.validate_class_cycles()?;

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

            let mut impl_info = crate::typeck::r#trait::ImplInfo::new(target_ty.clone(), None);
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

            self.class_decls.insert(
                class_decl.name.name.clone(),
                ClassDeclInfo {
                    parent,
                    fields,
                    methods,
                },
            );
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

    fn is_result_placeholder(expr: &Expr) -> bool {
        match &expr.kind {
            ExprKind::Ident(ident) => ident.name == "result",
            ExprKind::Path(path) => path
                .as_simple()
                .is_some_and(|segment| segment.name == "result"),
            _ => false,
        }
    }

    fn extract_result_literal_comparison(expr: &Expr) -> Option<(BinOp, Literal)> {
        let ExprKind::Binary { op, left, right } = &expr.kind else {
            return None;
        };

        if !matches!(op, BinOp::Eq | BinOp::NotEq) {
            return None;
        }

        if Self::is_result_placeholder(left) {
            if let ExprKind::Literal(lit) = &right.kind {
                return Some((*op, lit.clone()));
            }
        }

        if Self::is_result_placeholder(right) {
            if let ExprKind::Literal(lit) = &left.kind {
                return Some((*op, lit.clone()));
            }
        }

        None
    }

    fn extract_constant_return_literal(fn_decl: &Function) -> Option<Literal> {
        let stmt = fn_decl.body.stmts.last()?;
        match &stmt.kind {
            StmtKind::Expr(expr) => match &expr.kind {
                ExprKind::Literal(lit) => Some(lit.clone()),
                ExprKind::Return(Some(value)) => {
                    if let ExprKind::Literal(lit) = &value.kind {
                        Some(lit.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }

    fn validate_contracts_for_function(&mut self, fn_decl: &Function, ret_ty: &Ty) -> Result<()> {
        if let Some(precondition) = &fn_decl.precondition {
            let pre_ty = self.check_expr(precondition).map_err(CompileError::from)?;
            self.infer
                .unify(&pre_ty, &self.env.bool_ty())
                .map_err(CompileError::from)?;
        }

        if let Some(postcondition) = &fn_decl.postcondition {
            self.env.push_scope();
            self.env.insert_var("result".to_string(), ret_ty.clone());
            let post_ty = self.check_expr(postcondition);
            self.env.pop_scope();

            let post_ty = post_ty.map_err(CompileError::from)?;
            self.infer
                .unify(&post_ty, &self.env.bool_ty())
                .map_err(CompileError::from)?;

            if matches!(postcondition.kind, ExprKind::Literal(Literal::Bool(false))) {
                return Err(CompileError::from(TypeckError::Other(format!(
                    "postcondition for function `{}` is always false",
                    fn_decl.name.name
                ))));
            }

            if let (Some(return_lit), Some((op, ensured_lit))) = (
                Self::extract_constant_return_literal(fn_decl),
                Self::extract_result_literal_comparison(postcondition),
            ) {
                let contradiction = match op {
                    BinOp::Eq => return_lit != ensured_lit,
                    BinOp::NotEq => return_lit == ensured_lit,
                    _ => false,
                };
                if contradiction {
                    return Err(CompileError::from(TypeckError::Other(format!(
                        "postcondition contradicts constant return value in function `{}`",
                        fn_decl.name.name
                    ))));
                }
            }
        }

        Ok(())
    }

    fn check_function_signature_decl(&mut self, fn_decl: &Function) -> Result<()> {
        self.env.push_scope();
        let signature = (|| -> Result<(Vec<Ty>, Ty, Vec<GenericTypeParamMeta>)> {
            let generic_meta = self.bind_type_params_with_meta(&fn_decl.type_params)?;

            let mut param_types = Vec::new();
            for param in &fn_decl.params {
                let ty = self.check_type(&param.ty).map_err(CompileError::from)?;
                self.env.insert_var(param.name.name.clone(), ty.clone());
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
        let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
        self.env
            .insert_fn(fn_decl.name.name.clone(), fn_ty, param_types, ret_ty);
        self.set_generic_function_meta(fn_decl.name.name.clone(), generic_meta);
        Ok(())
    }

    /// 对函数声明进行完整的类型检查，包括参数、返回值和函数体。
    fn check_function_decl(&mut self, fn_decl: &Function) -> Result<()> {
        self.env.push_scope();
        let generic_meta = self.bind_type_params_with_meta(&fn_decl.type_params)?;

        let mut param_types = Vec::new();
        for param in &fn_decl.params {
            let ty = self.check_type(&param.ty)?;
            self.env.insert_var(param.name.name.clone(), ty.clone());
            param_types.push(ty);
        }

        let ret_ty = if let Some(ret) = &fn_decl.return_type {
            self.check_type(ret)?
        } else {
            self.env.unit_ty()
        };
        self.validate_contracts_for_function(fn_decl, &ret_ty)?;
        self.validate_ffi_function_decl(fn_decl, &param_types, &ret_ty)?;

        // 函数签名中的参数和返回值类型检查完毕，进行注册。
        let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
        self.env.insert_fn(
            fn_decl.name.name.clone(),
            fn_ty,
            param_types.clone(),
            ret_ty.clone(),
        );

        if fn_decl.is_async {
            self.async_functions.insert(fn_decl.name.name.clone());
        }

        // Function.body is always present (Block)
        let body_ty = if fn_decl.is_async {
            self.async_context_depth += 1;
            let result = self.check_block(&fn_decl.body);
            self.async_context_depth = self.async_context_depth.saturating_sub(1);
            result?
        } else {
            self.check_block(&fn_decl.body)?
        };

        // 检查函数体，处理隐式返回值。
        let is_main_with_implicit_return = fn_decl.name.name == "main"
            && matches!(body_ty.kind, TyKind::Unit)
            && matches!(ret_ty.kind, TyKind::Int(_));

        if !is_main_with_implicit_return {
            self.infer
                .unify(&body_ty, &ret_ty)
                .map_err(|e| CompileError::from(e))?;
        }

        self.env.pop_scope();

        // 泛型函数注册，将泛型元信息存入映射表。
        let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
        self.env
            .insert_fn(fn_decl.name.name.clone(), fn_ty, param_types, ret_ty);
        self.set_generic_function_meta(fn_decl.name.name.clone(), generic_meta);

        Ok(())
    }

    fn validate_ffi_function_decl(
        &mut self,
        fn_decl: &Function,
        param_types: &[Ty],
        ret_ty: &Ty,
    ) -> Result<()> {
        if fn_decl.abi.is_none() {
            if fn_decl.no_mangle || fn_decl.export_name.is_some() {
                return Err(CompileError::from(TypeckError::Other(
                    "no_mangle/export_name require `extern \"...\" fn`".to_string(),
                )));
            }
            return Ok(());
        }

        if !fn_decl.type_params.is_empty() {
            return Err(CompileError::from(TypeckError::Other(
                "generic extern functions are not supported in FFI MVP".to_string(),
            )));
        }

        let abi = fn_decl.abi.as_deref().unwrap_or("C");
        ffi_check::validate_signature(abi, param_types, ret_ty, fn_decl.is_unsafe)
            .map_err(CompileError::from)?;

        if fn_decl.export_name.is_some() && !matches!(fn_decl.vis, Visibility::Public) {
            return Err(CompileError::from(TypeckError::Other(
                "export_name requires `pub extern` function".to_string(),
            )));
        }

        Ok(())
    }

    fn check_extern_block_decl(&mut self, extern_block: &ExternBlock) -> Result<()> {
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

    fn bind_type_params_with_meta(
        &mut self,
        type_params: &[TypeParam],
    ) -> Result<Vec<GenericTypeParamMeta>> {
        let mut metas = Vec::with_capacity(type_params.len());
        for type_param in type_params {
            let fresh_var = self.env.new_ty_var();
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

        // Resolve defaults and trait bound paths inside the same generic scope.
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
        }

        Ok(metas)
    }

    /// 对结构体声明进行类型检查，验证字段类型和泛型约束。
    fn check_struct_decl(&mut self, struct_decl: &Struct) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&struct_decl.type_params)?;

        for field in &struct_decl.fields {
            self.check_type(&field.ty)?;
        }

        self.env.pop_scope();
        Ok(())
    }

    /// 对枚举声明进行类型检查，验证各枚举变体的类型。
    fn check_enum_decl(&mut self, enum_decl: &Enum) -> Result<()> {
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

    /// 对类声明进行类型检查，包括继承关系和方法验证。
    fn check_class_decl(&mut self, class_decl: &Class) -> Result<()> {
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

    fn check_class_method_decl(&mut self, class_name: &str, method: &Function) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&method.type_params)?;

        if method.self_param.is_some() {
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
            self.env.insert_var("self".to_string(), self_ty);
        }

        for param in &method.params {
            let ty = self.check_type(&param.ty)?;
            self.env.insert_var(param.name.name.clone(), ty);
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

    fn check_type_alias(&mut self, type_alias: &TypeAlias) -> Result<()> {
        self.env.push_scope();
        self.bind_type_params_with_meta(&type_alias.type_params)?;
        self.check_type(&type_alias.ty)?;
        self.env.pop_scope();
        Ok(())
    }

    /// 对常量声明进行类型检查，确保初始值类型匹配。
    fn check_const_decl(&mut self, const_decl: &Const) -> Result<()> {
        let ty = self.check_type(&const_decl.ty)?;
        let value_ty = self.check_expr(&const_decl.value)?;
        self.infer
            .unify(&ty, &value_ty)
            .map_err(CompileError::from)?;
        Ok(())
    }

    /// 对静态变量声明进行类型检查，确保初始值类型匹配。
    fn check_static_decl(&mut self, static_decl: &Static) -> Result<()> {
        let ty = self.check_type(&static_decl.ty)?;
        // Static.value is always present
        let value_ty = self.check_expr(&static_decl.value)?;
        self.infer
            .unify(&ty, &value_ty)
            .map_err(CompileError::from)?;
        Ok(())
    }

    /// 对trait声明进行类型检查，验证方法签名和关联类型。
    fn check_trait_decl(&mut self, trait_decl: &Trait) -> Result<()> {
        use crate::typeck::r#trait::{MethodSig, TraitInfo};

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

        // 检查trait中各个方法的签名和实现。
        for item in &trait_decl.items {
            match item {
                TraitItem::Function(method) => {
                    self.env.push_scope();
                    let method_generic_meta = self.bind_type_params_with_meta(&method.type_params)?;
                    // 绑定方法级别的泛型类型参数。
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

                    // 检查方法的返回类型。
                    let ret_ty = if let Some(ret) = &method.return_type {
                        self.check_type(ret)?
                    } else {
                        self.env.unit_ty()
                    };

                    // A trait method has a default implementation if its body is non-empty
                    let has_default = !method.body.stmts.is_empty();
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

    /// 对impl块进行类型检查，验证实现是否符合trait约束。
    fn check_impl_decl(&mut self, impl_decl: &Impl) -> Result<()> {
        use crate::typeck::r#trait::type_key;
        use crate::typeck::r#trait::{FunctionTy, ImplInfo};

        self.env.push_scope();
        self.bind_type_params_with_meta(&impl_decl.type_params)?;

        let target_ty = self.check_type(&impl_decl.target_type)?;
        let target_key = type_key(&target_ty);

        let trait_name = impl_decl
            .trait_path
            .as_ref()
            .and_then(|p| p.as_simple())
            .map(|s| s.name.clone());

        let mut impl_info = ImplInfo::new(target_ty.clone(), trait_name);

        // 检查impl块中各个方法的实现。
        for item in &impl_decl.items {
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

        // 验证impl是否满足trait的约束要求。
        if let Some(trait_name) = impl_info.trait_name.clone() {
            // For trait impls, also register default methods from the trait
            // definition that are not overridden by the impl.
            // Also check that all required (non-default) methods are implemented.
            if let Some(trait_info) = self.trait_registry.get(&trait_name) {
                let mut missing_methods = Vec::new();

                for (method_name, method_sig) in &trait_info.methods {
                    if !impl_info.has_method(method_name) {
                        if method_sig.has_default {
                            // This method has a default implementation in the trait
                            // 该方法有默认实现，添加到impl信息中。
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
                            // This method is required but not implemented
                            missing_methods.push(method_name.clone());
                        }
                    }
                }

                if !missing_methods.is_empty() {
                    missing_methods.sort();
                    self.env.pop_scope();
                    let err = TypeckError::Other(format!(
                        "impl `{}` for `{}` is missing required trait methods: {}",
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

    /// 将路径（Path）解析为字符串名称。
    fn path_name(&self, path: &Path) -> TyResult<String> {
        path.as_simple()
            .map(|ident| ident.name.clone())
            .ok_or_else(|| TypeckError::UndefinedType {
                name: path
                    .segments
                    .iter()
                    .map(|seg| seg.name.as_str())
                    .collect::<Vec<_>>()
                    .join("::"),
            })
    }

    fn builtin_type_by_name(&mut self, name: &str) -> Option<Ty> {
        Some(match name {
            "()" => self.env.unit_ty(),
            "bool" => self.env.bool_ty(),
            "i8" => self.env.int_ty(IntKind::I8),
            "i16" => self.env.int_ty(IntKind::I16),
            "i32" => self.env.int_ty(IntKind::I32),
            "i64" => self.env.int_ty(IntKind::I64),
            "i128" => self.env.int_ty(IntKind::I128),
            "isize" => self.env.int_ty(IntKind::ISize),
            "u8" => self.env.int_ty(IntKind::U8),
            "u16" => self.env.int_ty(IntKind::U16),
            "u32" => self.env.int_ty(IntKind::U32),
            "u64" => self.env.int_ty(IntKind::U64),
            "u128" => self.env.int_ty(IntKind::U128),
            "usize" => self.env.int_ty(IntKind::USize),
            "f32" => self.env.float_ty(FloatKind::F32),
            "f64" => self.env.float_ty(FloatKind::F64),
            "str" => self.env.str_ty(),
            "char" => self.env.new_ty(TyKind::Char),
            "!" => self.env.never_ty(),
            _ => return None,
        })
    }

    fn substitute_ty_vars(&self, ty: &Ty, subst: &HashMap<TyVarId, Ty>) -> Ty {
        match &ty.kind {
            TyKind::Var(var_id) => subst.get(var_id).cloned().unwrap_or_else(|| ty.clone()),
            TyKind::Tuple(types) => Ty {
                id: ty.id,
                kind: TyKind::Tuple(
                    types
                        .iter()
                        .map(|inner| self.substitute_ty_vars(inner, subst))
                        .collect(),
                ),
            },
            TyKind::Array(elem, len) => Ty {
                id: ty.id,
                kind: TyKind::Array(Box::new(self.substitute_ty_vars(elem, subst)), *len),
            },
            TyKind::Slice(elem) => Ty {
                id: ty.id,
                kind: TyKind::Slice(Box::new(self.substitute_ty_vars(elem, subst))),
            },
            TyKind::Ref(is_mut, inner) => Ty {
                id: ty.id,
                kind: TyKind::Ref(*is_mut, Box::new(self.substitute_ty_vars(inner, subst))),
            },
            TyKind::Ptr(inner) => Ty {
                id: ty.id,
                kind: TyKind::Ptr(Box::new(self.substitute_ty_vars(inner, subst))),
            },
            TyKind::Fn {
                params,
                ret,
                is_variadic,
            } => Ty {
                id: ty.id,
                kind: TyKind::Fn {
                    params: params
                        .iter()
                        .map(|param| self.substitute_ty_vars(param, subst))
                        .collect(),
                    ret: Box::new(self.substitute_ty_vars(ret, subst)),
                    is_variadic: *is_variadic,
                },
            },
            TyKind::Adt { name, args } => Ty {
                id: ty.id,
                kind: TyKind::Adt {
                    name: name.clone(),
                    args: args
                        .iter()
                        .map(|arg| self.substitute_ty_vars(arg, subst))
                        .collect(),
                },
            },
            _ => ty.clone(),
        }
    }

    fn generic_lookup_key(&self, ty: &Ty) -> String {
        match &ty.kind {
            TyKind::Adt { name, args } => {
                if args.is_empty() {
                    name.clone()
                } else {
                    format!("{}<{}>", name, vec!["?"; args.len()].join(","))
                }
            }
            TyKind::Ref(_, inner) => format!("&{}", self.generic_lookup_key(inner)),
            TyKind::Ptr(inner) => format!("*{}", self.generic_lookup_key(inner)),
            TyKind::Array(elem, len) => format!("[{}; {}]", self.generic_lookup_key(elem), len),
            TyKind::Slice(elem) => format!("[{}]", self.generic_lookup_key(elem)),
            TyKind::Tuple(types) => {
                if types.is_empty() {
                    "()".to_string()
                } else {
                    format!(
                        "({})",
                        types
                            .iter()
                            .map(|ty| self.generic_lookup_key(ty))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )
                }
            }
            _ => type_key(ty),
        }
    }

    fn match_generic_impl_target(
        &self,
        pattern: &Ty,
        concrete: &Ty,
        subst: &mut HashMap<TyVarId, Ty>,
    ) -> bool {
        match (&pattern.kind, &concrete.kind) {
            (TyKind::Var(var_id), _) => {
                if let Some(bound) = subst.get(var_id) {
                    bound == concrete
                } else {
                    subst.insert(*var_id, concrete.clone());
                    true
                }
            }
            (
                TyKind::Adt {
                    name: lhs_name,
                    args: lhs_args,
                },
                TyKind::Adt {
                    name: rhs_name,
                    args: rhs_args,
                },
            ) => {
                lhs_name == rhs_name
                    && lhs_args.len() == rhs_args.len()
                    && lhs_args
                        .iter()
                        .zip(rhs_args.iter())
                        .all(|(lhs, rhs)| self.match_generic_impl_target(lhs, rhs, subst))
            }
            (TyKind::Ref(lhs_mut, lhs_inner), TyKind::Ref(rhs_mut, rhs_inner)) => {
                lhs_mut == rhs_mut && self.match_generic_impl_target(lhs_inner, rhs_inner, subst)
            }
            (TyKind::Ptr(lhs_inner), TyKind::Ptr(rhs_inner)) => {
                self.match_generic_impl_target(lhs_inner, rhs_inner, subst)
            }
            (TyKind::Array(lhs_elem, lhs_len), TyKind::Array(rhs_elem, rhs_len)) => {
                lhs_len == rhs_len && self.match_generic_impl_target(lhs_elem, rhs_elem, subst)
            }
            (TyKind::Slice(lhs_elem), TyKind::Slice(rhs_elem)) => {
                self.match_generic_impl_target(lhs_elem, rhs_elem, subst)
            }
            (TyKind::Tuple(lhs_types), TyKind::Tuple(rhs_types)) => {
                lhs_types.len() == rhs_types.len()
                    && lhs_types
                        .iter()
                        .zip(rhs_types.iter())
                        .all(|(lhs, rhs)| self.match_generic_impl_target(lhs, rhs, subst))
            }
            (
                TyKind::Fn {
                    params: lhs_params,
                    ret: lhs_ret,
                    is_variadic: lhs_variadic,
                },
                TyKind::Fn {
                    params: rhs_params,
                    ret: rhs_ret,
                    is_variadic: rhs_variadic,
                },
            ) => {
                lhs_variadic == rhs_variadic
                    && lhs_params.len() == rhs_params.len()
                    && lhs_params
                        .iter()
                        .zip(rhs_params.iter())
                        .all(|(lhs, rhs)| self.match_generic_impl_target(lhs, rhs, subst))
                    && self.match_generic_impl_target(lhs_ret, rhs_ret, subst)
            }
            _ => pattern.kind == concrete.kind,
        }
    }

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

    fn resolve_generic_type_args(
        &self,
        type_name: &str,
        meta: &GenericTypeMeta,
        explicit_args: Vec<Ty>,
    ) -> TyResult<Vec<Ty>> {
        if explicit_args.len() > meta.params.len() {
            return Err(TypeckError::Other(format!(
                "type {} expects at most {} generic arguments, found {}",
                type_name,
                meta.params.len(),
                explicit_args.len()
            )));
        }

        let mut resolved = Vec::with_capacity(meta.params.len());
        let mut subst = HashMap::<TyVarId, Ty>::new();

        for (index, param) in meta.params.iter().enumerate() {
            let current = if let Some(arg) = explicit_args.get(index) {
                arg.clone()
            } else if let Some(default_ty) = &param.default {
                self.substitute_ty_vars(default_ty, &subst)
            } else {
                return Err(TypeckError::Other(format!(
                    "missing generic argument {} for type {}",
                    param.name, type_name
                )));
            };

            for bound in &param.bounds {
                let concrete_key = type_key(&current);
                if !self.impl_registry.implements_trait(bound, &concrete_key) {
                    return Err(TypeckError::Other(format!(
                        "generic constraint violated in type {}: {} does not implement {} for {}",
                        type_name, current, bound, param.name
                    )));
                }
            }

            subst.insert(param.var_id, current.clone());
            resolved.push(current);
        }

        Ok(resolved)
    }

    fn check_path_type(&mut self, path: &Path, explicit_args: Vec<Ty>) -> TyResult<Ty> {
        let name = self.path_name(path)?;

        if let Some(meta) = self.generic_type_metas.get(&name).cloned() {
            let args = self.resolve_generic_type_args(&name, &meta, explicit_args)?;
            return Ok(self.env.new_ty(TyKind::Adt { name, args }));
        }

        if !explicit_args.is_empty() {
            return Err(TypeckError::Other(format!("type {} is not generic", name)));
        }

        if let Some(symbol) = self.env.lookup(&name) {
            if let Some(ty) = symbol.get_ty() {
                return Ok(ty.clone());
            }
        }

        if let Some(ty) = self.builtin_type_by_name(&name) {
            return Ok(ty);
        }

        Err(TypeckError::UndefinedType { name })
    }

    /// 将AST类型节点转换为内部类型表示（Ty）。
    fn check_type(&mut self, ty: &Type) -> TyResult<Ty> {
        Ok(match &ty.kind {
            TypeKind::Path(path) => self.check_path_type(path, Vec::new())?,
            TypeKind::PathWithArgs { path, args } => {
                let args = args
                    .iter()
                    .map(|arg| self.check_type(arg))
                    .collect::<TyResult<Vec<_>>>()?;
                self.check_path_type(path, args)?
            }
            TypeKind::Tuple(types) => {
                let elem_types = types
                    .iter()
                    .map(|t| self.check_type(t))
                    .collect::<TyResult<Vec<_>>>()?;
                self.env.tuple_ty(elem_types)
            }
            TypeKind::Array(elem, len) => {
                let elem_ty = self.check_type(elem)?;
                self.env.array_ty(elem_ty, *len as usize)
            }
            TypeKind::Slice(elem) => {
                let elem_ty = self.check_type(elem)?;
                self.env.slice_ty(elem_ty)
            }
            TypeKind::Ptr { base, is_mut: _ } => {
                let inner_ty = self.check_type(base)?;
                self.env.new_ty(TyKind::Ptr(Box::new(inner_ty)))
            }
            TypeKind::Ref { base, is_mut } => {
                let inner_ty = self.check_type(base)?;
                self.env.ref_ty(*is_mut, inner_ty)
            }
            TypeKind::Fn { params, ret } => {
                let param_types = params
                    .iter()
                    .map(|p| self.check_type(p))
                    .collect::<TyResult<Vec<_>>>()?;
                let ret_ty = match ret {
                    Some(r) => self.check_type(r)?,
                    None => self.env.unit_ty(),
                };
                self.env.fn_ty(param_types, ret_ty)
            }
            TypeKind::Never => self.env.never_ty(),
            TypeKind::Infer => self.infer.fresh_ty_var(),
            TypeKind::Dyn(trait_bounds) => {
                let names: Vec<String> = trait_bounds
                    .iter()
                    .filter_map(|b| b.path.as_simple())
                    .map(|ident| ident.name.clone())
                    .collect();
                self.env.new_ty(TyKind::Dyn(names))
            }
            TypeKind::ImplTrait(trait_bounds) => {
                let names: Vec<String> = trait_bounds
                    .iter()
                    .filter_map(|b| b.path.as_simple())
                    .map(|ident| ident.name.clone())
                    .collect();
                self.env.new_ty(TyKind::ImplTrait(names))
            }
        })
    }

    fn check_expr(&mut self, expr: &Expr) -> TyResult<Ty> {
        match &expr.kind {
            ExprKind::Literal(lit) => self.check_literal(lit),
            ExprKind::Ident(ident) => self.check_ident(ident),
            ExprKind::Binary { op, left, right } => self.check_binary(op, left, right),
            ExprKind::Unary { op, operand } => self.check_unary(op, operand),
            ExprKind::Assign { target, value } => self.check_assign(target, value),
            ExprKind::AssignOp { op, target, value } => self.check_assign_op(op, target, value),
            ExprKind::Index { base, index } => self.check_index(base, index),
            ExprKind::Field { base, field } => self.check_field(base, field),
            ExprKind::Call { func, args } => self.check_call(func, args),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => self.check_method_call(receiver, method, args),
            ExprKind::Tuple(elems) => self.check_tuple(elems),
            ExprKind::Array(elems) => self.check_array(elems),
            ExprKind::Block(block) => self.check_block(block),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => self.check_if(cond, then_branch, else_branch),
            ExprKind::While { cond, body } => self.check_while(cond, body),
            ExprKind::For {
                pattern,
                iter,
                body,
            } => self.check_for(pattern, iter, body),
            ExprKind::Loop(body) => self.check_loop(body),
            ExprKind::Match { scrutinee, arms } => self.check_match(scrutinee, arms),
            ExprKind::Return(value) => self.check_return(value),
            ExprKind::Break(value) => self.check_break(value),
            ExprKind::Continue => self.check_continue(),
            ExprKind::Path(path) => self.check_path(path),
            ExprKind::Lambda { params, body } => self.check_lambda(params, body),
            ExprKind::Await(expr) => {
                if self.async_context_depth == 0 {
                    return Err(TypeckError::Other(
                        "await is only allowed in async contexts".to_string(),
                    ));
                }
                let inner_ty = self.check_expr(expr)?;
                match &inner_ty.kind {
                    TyKind::Future(result_ty) => Ok(result_ty.as_ref().clone()),
                    _ => Err(TypeckError::Other(
                        "await requires a Future value (call to an async function)".to_string(),
                    )),
                }
            }
            ExprKind::AsyncBlock(_block) => {
                Err(TypeckError::Other(
                    "async blocks are not yet supported in this phase".to_string(),
                ))
            }
            ExprKind::Struct { path, fields, .. } => {
                let name = path
                    .as_simple()
                    .map(|ident| ident.name.clone())
                    .unwrap_or_default();

                let field_defs = self
                    .struct_field_defs
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| TypeckError::UndefinedType { name: name.clone() })?;

                if let Some(type_params) = self.struct_type_params.get(&name).cloned() {
                    if !type_params.is_empty() {
                        self.env.push_scope();
                        let generic_meta = self
                            .bind_type_params_with_meta(&type_params)
                            .map_err(|err| TypeckError::Other(err.to_string()))?;

                        let mut field_types: HashMap<String, Ty> = HashMap::new();
                        for (field_name, field_ty) in &field_defs {
                            field_types.insert(field_name.clone(), self.check_type(field_ty)?);
                        }

                        let mut seen = HashSet::new();
                        for field_value in fields {
                            let field_name = match &field_value.name {
                                crate::ast::FieldName::Ident(ident) => ident.name.clone(),
                                crate::ast::FieldName::String(name) => name.clone(),
                            };

                            if !seen.insert(field_name.clone()) {
                                self.env.pop_scope();
                                return Err(TypeckError::Other(format!(
                                    "duplicate struct literal field `{}` for `{}`",
                                    field_name, name
                                )));
                            }

                            let expected_ty = field_types.get(&field_name).cloned().ok_or_else(|| {
                                TypeckError::FieldNotFound {
                                    type_name: name.clone(),
                                    field_name: field_name.clone(),
                                }
                            })?;

                            let value_ty = self.check_expr(&field_value.value)?;
                            self.infer.unify(&expected_ty, &value_ty)?;
                        }

                        let mut args = Vec::with_capacity(generic_meta.len());
                        for param in &generic_meta {
                            let placeholder = Ty::new(0, TyKind::Var(param.var_id));
                            let mut concrete_ty = self.infer.apply_subst(&placeholder);
                            if matches!(concrete_ty.kind, TyKind::Var(_)) {
                                if let Some(default_ty) = &param.default {
                                    concrete_ty = self.substitute_ty_vars(default_ty, &HashMap::new());
                                } else {
                                    self.env.pop_scope();
                                    return Err(TypeckError::Other(format!(
                                        "cannot infer generic argument `{}` for struct `{}` literal",
                                        param.name, name
                                    )));
                                }
                            }
                            for bound in &param.bounds {
                                let concrete_key = type_key(&concrete_ty);
                                if !self.impl_registry.implements_trait(bound, &concrete_key) {
                                    self.env.pop_scope();
                                    return Err(TypeckError::Other(format!(
                                        "generic constraint violated in struct `{}` literal: `{}` does not implement `{}` for `{}`",
                                        name, concrete_key, bound, param.name
                                    )));
                                }
                            }
                            args.push(concrete_ty);
                        }

                        self.env.pop_scope();
                        Ok(self.env.new_ty(TyKind::Adt { name, args }))
                    } else if let Some(symbol) = self.env.lookup(&name) {
                        if let Some(ty) = symbol.get_ty() {
                            Ok(ty.clone())
                        } else {
                            Ok(self.env.new_ty(TyKind::Adt { name, args: vec![] }))
                        }
                    } else {
                        Err(TypeckError::UndefinedType { name })
                    }
                } else {
                    let mut field_types: HashMap<String, Ty> = HashMap::new();
                    for (field_name, field_ty) in field_defs {
                        field_types.insert(field_name, self.check_type(&field_ty)?);
                    }

                    let mut seen = HashSet::new();
                    for field_value in fields {
                        let field_name = match &field_value.name {
                            crate::ast::FieldName::Ident(ident) => ident.name.clone(),
                            crate::ast::FieldName::String(name) => name.clone(),
                        };

                        if !seen.insert(field_name.clone()) {
                            return Err(TypeckError::Other(format!(
                                "duplicate struct literal field `{}` for `{}`",
                                field_name, name
                            )));
                        }

                        let expected_ty = field_types.get(&field_name).cloned().ok_or_else(|| {
                            TypeckError::FieldNotFound {
                                type_name: name.clone(),
                                field_name: field_name.clone(),
                            }
                        })?;

                        let value_ty = self.check_expr(&field_value.value)?;
                        self.infer.unify(&expected_ty, &value_ty)?;
                    }

                    if let Some(symbol) = self.env.lookup(&name) {
                        if let Some(ty) = symbol.get_ty() {
                            Ok(ty.clone())
                        } else {
                            Ok(self.env.new_ty(TyKind::Adt { name, args: vec![] }))
                        }
                    } else {
                        Err(TypeckError::UndefinedType { name })
                    }
                }
            }
            _ => Ok(self.env.error_ty()),
        }
    }

    /// 检查字面量表达式并返回其类型。
    fn check_literal(&mut self, lit: &Literal) -> TyResult<Ty> {
        Ok(match lit {
            // 检查整数字面量，返回对应的整数类型。
            Literal::Int(_) => self.env.int_ty(IntKind::I64),
            Literal::Float(_) => self.env.float_ty(FloatKind::F64),
            Literal::String(_) => {
                let str_ty = self.env.str_ty();
                self.env.ref_ty(false, str_ty)
            }
            Literal::Char(_) => self.env.new_ty(TyKind::Char),
            Literal::Bytes(_) => self.env.new_ty(TyKind::Bytes),
            Literal::Bool(_) => self.env.bool_ty(),
            Literal::Null => self.env.new_ty(TyKind::Adt {
                name: "Option".to_string(),
                args: vec![],
            }),
            Literal::Unit => self.env.unit_ty(),
        })
    }

    /// 检查标识符表达式，查找变量或函数的类型。
    fn check_ident(&mut self, ident: &Ident) -> TyResult<Ty> {
        let symbol = if let Some(symbol) = self.env.lookup(&ident.name) {
            symbol.clone()
        } else {
            return Err(TypeckError::UndefinedVariable {
                name: ident.name.clone(),
            });
        };

        match &symbol.kind {
            SymbolKind::Function { ty, .. } => {
                Ok(self.infer.instantiate_with_fresh_vars(ty.clone()))
            }
            _ => {
                if let Some(ty) = symbol.get_ty() {
                    Ok(self.infer.instantiate(ty.clone()))
                } else {
                    Err(TypeckError::UndefinedVariable {
                        name: ident.name.clone(),
                    })
                }
            }
        }
    }

    /// 检查路径表达式，解析命名空间路径的类型。
    fn check_path(&mut self, path: &Path) -> TyResult<Ty> {
        if let Some(ident) = path.as_simple() {
            self.check_ident(ident)
        } else {
            Err(TypeckError::UndefinedVariable {
                name: path
                    .segments
                    .iter()
                    .map(|seg| seg.name.as_str())
                    .collect::<Vec<_>>()
                    .join("::"),
            })
        }
    }

    /// 检查二元运算表达式，验证操作数类型兼容性并返回结果类型。
    fn check_binary(&mut self, op: &BinOp, left: &Expr, right: &Expr) -> TyResult<Ty> {
        let left_ty = self.check_expr(left)?;
        let right_ty = self.check_expr(right)?;

        // For arithmetic and bitwise operations, allow compatible integer types
        // The actual type reconciliation will happen in MIR lowering
        let types_compatible = match (&left_ty.kind, &right_ty.kind) {
            // Same types are always compatible
            _ if left_ty.kind == right_ty.kind => true,
            // Different integer widths are compatible for arithmetic/bitwise ops
            (TyKind::Int(_), TyKind::Int(_)) => matches!(
                op,
                BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::Mod
                    | BinOp::BitAnd
                    | BinOp::BitOr
                    | BinOp::BitXor
                    | BinOp::Shl
                    | BinOp::Shr
                    | BinOp::Eq
                    | BinOp::NotEq
                    | BinOp::Lt
                    | BinOp::Le
                    | BinOp::Gt
                    | BinOp::Ge
            ),
            // Different float widths are compatible for arithmetic ops
            (TyKind::Float(_), TyKind::Float(_)) => matches!(
                op,
                BinOp::Add
                    | BinOp::Sub
                    | BinOp::Mul
                    | BinOp::Div
                    | BinOp::Mod
                    | BinOp::Eq
                    | BinOp::NotEq
                    | BinOp::Lt
                    | BinOp::Le
                    | BinOp::Gt
                    | BinOp::Ge
            ),
            _ => false,
        };

        if !types_compatible {
            self.infer
                .unify(&left_ty, &right_ty)
                .map_err(|_| TypeckError::TypeMismatch {
                    expected: right_ty.kind.clone(),
                    found: left_ty.kind.clone(),
                })?;
        }

        // For mixed-width operations, return the wider type
        let result_ty = match (&left_ty.kind, &right_ty.kind) {
            (TyKind::Int(a), TyKind::Int(b)) if a != b => {
                // Return the wider integer type
                use crate::typeck::ty::IntKind;
                let wider = match (a, b) {
                    (IntKind::I64, _) | (_, IntKind::I64) => IntKind::I64,
                    (IntKind::I32, _) | (_, IntKind::I32) => IntKind::I32,
                    (IntKind::I16, _) | (_, IntKind::I16) => IntKind::I16,
                    _ => IntKind::I8,
                };
                self.env.int_ty(wider)
            }
            (TyKind::Float(a), TyKind::Float(b)) if a != b => {
                // F64 is wider than F32
                use crate::typeck::ty::FloatKind;
                let wider = match (a, b) {
                    (FloatKind::F64, _) | (_, FloatKind::F64) => FloatKind::F64,
                    _ => FloatKind::F32,
                };
                self.env.float_ty(wider)
            }
            _ => left_ty.clone(),
        };

        Ok(match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => result_ty,
            BinOp::And | BinOp::Or => self.env.bool_ty(),
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => result_ty,
            BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.env.bool_ty()
            }
            BinOp::Pipe | BinOp::Compose | BinOp::Range | BinOp::RangeInclusive => result_ty,
        })
    }

    /// 检查一元运算表达式，验证操作数类型并返回结果类型。
    fn check_unary(&mut self, op: &UnOp, operand: &Expr) -> TyResult<Ty> {
        let ty = self.check_expr(operand)?;
        Ok(match op {
            UnOp::Neg | UnOp::Not | UnOp::Plus | UnOp::BitNot => ty.clone(),
            UnOp::Deref => {
                if let Some(inner) = ty.ref_inner() {
                    inner.clone()
                } else {
                    return Err(TypeckError::TypeMismatch {
                        expected: TyKind::Ref(false, Box::new(self.env.error_ty())),
                        found: ty.kind.clone(),
                    });
                }
            }
            UnOp::Ref => self.env.ref_ty(false, ty),
            UnOp::RefMut | UnOp::DerefMut => self.env.ref_ty(true, ty),
        })
    }

    /// 检查赋值表达式，验证目标和值的类型兼容性。
    fn check_assign(&mut self, target: &Expr, value: &Expr) -> TyResult<Ty> {
        let target_ty = self.check_expr(target)?;
        let value_ty = self.check_expr(value)?;
        self.infer.unify(&target_ty, &value_ty)?;
        Ok(self.env.unit_ty())
    }

    /// 检查复合赋值表达式（如 +=、-= 等），验证类型兼容性。
    fn check_assign_op(&mut self, _op: &AssignOp, target: &Expr, value: &Expr) -> TyResult<Ty> {
        let target_ty = self.check_expr(target)?;
        let value_ty = self.check_expr(value)?;
        self.infer.unify(&target_ty, &value_ty)?;
        Ok(self.env.unit_ty())
    }

    /// 检查索引表达式，验证基础对象可被索引且索引类型正确。
    fn check_index(&mut self, base: &Expr, index: &Expr) -> TyResult<Ty> {
        let base_ty = self.check_expr(base)?;
        let index_ty = self.check_expr(index)?;

        if !index_ty.is_int() {
            return Err(TypeckError::TypeMismatch {
                expected: TyKind::Int(IntKind::ISize),
                found: index_ty.kind.clone(),
            });
        }

        Ok(match &base_ty.kind {
            TyKind::Array(elem, _) => (**elem).clone(),
            TyKind::Slice(elem) => (**elem).clone(),
            TyKind::Tuple(types) if !types.is_empty() => types[0].clone(),
            _ => self.env.error_ty(),
        })
    }

    /// 检查字段访问表达式，验证字段存在性和类型。
    fn check_field(&mut self, base: &Expr, name: &Ident) -> TyResult<Ty> {
        let base_ty = self.check_expr(base)?;

        match &base_ty.kind {
            TyKind::Adt {
                name: type_name, args
            } => {
                let field_defs =
                    self.struct_field_defs
                        .get(type_name)
                        .cloned()
                        .ok_or_else(|| TypeckError::FieldNotFound {
                            type_name: type_name.clone(),
                            field_name: name.name.clone(),
                        })?;

                let field_ty = field_defs
                    .into_iter()
                    .find(|(field_name, _)| field_name == &name.name)
                    .map(|(_, field_ty)| field_ty)
                    .ok_or_else(|| TypeckError::FieldNotFound {
                        type_name: type_name.clone(),
                        field_name: name.name.clone(),
                    })?;

                if let Some(type_params) = self.struct_type_params.get(type_name).cloned() {
                    if !type_params.is_empty() && type_params.len() == args.len() {
                        self.env.push_scope();
                        for (type_param, concrete_ty) in type_params.iter().zip(args.iter()) {
                            self.env
                                .insert_type(type_param.name.name.clone(), concrete_ty.clone());
                        }
                        let resolved = self.check_type(&field_ty);
                        self.env.pop_scope();
                        resolved
                    } else {
                        self.check_type(&field_ty)
                    }
                } else {
                    self.check_type(&field_ty)
                }
            }
            _ => Err(TypeckError::FieldNotFound {
                type_name: base_ty.kind.to_string(),
                field_name: name.name.clone(),
            }),
        }
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

    fn check_call(&mut self, func: &Expr, args: &[Expr]) -> TyResult<Ty> {
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
                            self.infer.instantiate_with_fresh_vars_and_map(ty);
                        generic_ctx = Some((name, meta, var_map));
                        instantiated
                    } else {
                        self.infer.instantiate_with_fresh_vars(ty)
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
                if actual_ty.is_future() {
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

    /// 检查方法调用表达式，进行方法解析和参数类型检查。
    fn check_method_call(
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

    fn check_tuple(&mut self, elems: &[Expr]) -> TyResult<Ty> {
        let elem_types = elems
            .iter()
            .map(|e| self.check_expr(e))
            .collect::<TyResult<Vec<_>>>()?;
        Ok(self.env.tuple_ty(elem_types))
    }

    /// 检查数组表达式，验证所有元素类型一致。
    fn check_array(&mut self, elems: &[Expr]) -> TyResult<Ty> {
        if elems.is_empty() {
            return Ok(self.env.array_ty(self.infer.fresh_ty_var(), 0));
        }

        let first_ty = self.check_expr(&elems[0])?;
        for elem in &elems[1..] {
            let ty = self.check_expr(elem)?;
            self.infer.unify(&first_ty, &ty)?;
        }

        Ok(self.env.array_ty(first_ty, elems.len()))
    }

    /// Lambda表达式类型检查，推断参数类型和返回类型。
    /// Lambda表达式类型检查，推断参数类型和返回类型。
    fn check_lambda(&mut self, params: &[Ident], body: &Expr) -> TyResult<Ty> {
        // 为每个lambda参数创建新的类型变量。
        let param_tys: Vec<Ty> = params.iter().map(|_| self.infer.fresh_ty_var()).collect();

        // 推断参数类型约束。
        self.env.push_scope();

        // 将参数绑定到作用域中。
        for (param, ty) in params.iter().zip(param_tys.iter()) {
            self.env.insert_var(param.name.clone(), ty.clone());
        }

        // 检查lambda体的类型。
        let body_ty = self.check_expr(body)?;

        // 清理作用域。
        self.env.pop_scope();

        // 返回函数类型。
        Ok(self.env.fn_ty(param_tys, body_ty))
    }

    /// 检查块表达式，按顺序检查所有语句并返回最终类型。
    fn check_block(&mut self, block: &Block) -> TyResult<Ty> {
        self.env.push_scope();

        let mut result_ty = self.env.unit_ty();
        for stmt in &block.stmts {
            if let Some(ty) = self.check_stmt(stmt)? {
                result_ty = ty;
            }
        }

        self.env.pop_scope();
        Ok(result_ty)
    }

    /// 检查单条语句，返回可选的类型（仅表达式语句有类型）。
    fn check_stmt(&mut self, stmt: &Stmt) -> TyResult<Option<Ty>> {
        match &stmt.kind {
            StmtKind::Let {
                name, ty, value, ..
            } => {
                let var_ty = if let Some(ty) = ty {
                    self.check_type(ty)?
                } else {
                    self.infer.fresh_ty_var()
                };

                // 获取let绑定中初始值的类型。
                let value_ty = match value {
                    Some(v) => self.check_expr(v)?,
                    None => self.env.unit_ty(),
                };
                self.infer.unify(&var_ty, &value_ty)?;

                self.env.insert_var(name.name.clone(), var_ty);
                Ok(None)
            }
            StmtKind::Const { name, ty, value } => {
                let var_ty = self.check_type(ty)?;
                let value_ty = self.check_expr(value)?;
                self.infer.unify(&var_ty, &value_ty)?;
                self.env.insert_var(name.name.clone(), var_ty);
                Ok(None)
            }
            StmtKind::Expr(expr) => {
                let ty = self.check_expr(expr)?;
                Ok(Some(ty))
            }
            StmtKind::Item(item) => {
                // 检查内联声明语句。
                self.check_decl(item)
                    .map_err(|e| TypeckError::Other(e.to_string()))?;
                Ok(None)
            }
        }
    }

    /// 检查if条件表达式，验证条件为bool型且分支类型兼容。
    fn check_if(
        &mut self,
        cond: &Expr,
        then_branch: &Block,
        else_branch: &Option<Box<Expr>>,
    ) -> TyResult<Ty> {
        let cond_ty = self.check_expr(cond)?;
        let bool_ty = self.env.bool_ty();
        self.infer.unify(&cond_ty, &bool_ty)?;

        let then_ty = self.check_block(then_branch)?;
        let else_ty = match else_branch {
            Some(e) => self.check_expr(e)?,
            None => self.env.unit_ty(),
        };

        self.infer.unify(&then_ty, &else_ty)?;
        Ok(then_ty)
    }

    /// 检查while循环，验证条件为bool型。
    fn check_while(&mut self, cond: &Expr, body: &Block) -> TyResult<Ty> {
        let cond_ty = self.check_expr(cond)?;
        let bool_ty = self.env.bool_ty();
        self.infer.unify(&cond_ty, &bool_ty)?;

        self.check_block(body)?;
        Ok(self.env.unit_ty())
    }

    /// 检查for循环，验证迭代器类型和模式匹配。
    fn check_for(&mut self, pattern: &Pattern, iter: &Expr, body: &Block) -> TyResult<Ty> {
        let elem_ty = match &iter.kind {
            ExprKind::Range { start, end, .. } => {
                let range_ty = self.env.int_ty(IntKind::I64);
                if let Some(start) = start.as_deref() {
                    let start_ty = self.check_expr(start)?;
                    self.infer.unify(&start_ty, &range_ty)?;
                }
                if let Some(end) = end.as_deref() {
                    let end_ty = self.check_expr(end)?;
                    self.infer.unify(&end_ty, &range_ty)?;
                }
                range_ty
            }
            _ => {
                let iter_ty = self.check_expr(iter)?;
                match &iter_ty.kind {
                    TyKind::Array(elem, _) | TyKind::Slice(elem) => (**elem).clone(),
                    _ => {
                        return Err(TypeckError::Other(
                            "for loop expects an array, slice, or range iterable".to_string(),
                        ));
                    }
                }
            }
        };
        // 检查for循环体。

        self.env.push_scope();

        // 绑定循环变量到模式中。
        let var_name = match &pattern.kind {
            crate::ast::pattern::PatternKind::Ident(name) => name.name.clone(),
            crate::ast::pattern::PatternKind::Wildcard => "_loop".to_string(),
            _ => "_loop".to_string(),
        };

        self.env.insert_var(var_name, elem_ty);
        self.check_block(body)?;
        self.env.pop_scope();

        Ok(self.env.unit_ty())
    }

    /// 检查loop循环体类型。
    fn check_loop(&mut self, body: &Block) -> TyResult<Ty> {
        self.check_block(body)?;
        Ok(self.env.unit_ty())
    }

    /// 检查match表达式，验证所有分支类型一致。
    fn check_match(&mut self, scrutinee: &Expr, arms: &[MatchArm]) -> TyResult<Ty> {
        self.check_expr(scrutinee)?;

        let mut arm_types = Vec::new();
        for arm in arms {
            if let Some(guard) = &arm.guard {
                self.check_expr(guard)?;
            }
            let arm_ty = self.check_expr(&arm.body)?;
            arm_types.push(arm_ty);
        }

        let result_ty = arm_types
            .first()
            .cloned()
            .unwrap_or_else(|| self.env.unit_ty());
        for arm_ty in &arm_types {
            self.infer.unify(&result_ty, arm_ty)?;
        }

        Ok(result_ty)
    }

    /// 检查return语句，验证返回值类型与函数返回类型匹配。
    fn check_return(&mut self, value: &Option<Box<Expr>>) -> TyResult<Ty> {
        match value {
            Some(v) => {
                let ty = self.check_expr(v)?;
                if ty.is_future() {
                    return Err(TypeckError::Other(
                        "phase-1 async future values cannot escape; await the async call directly"
                            .to_string(),
                    ));
                }
            }
            None => {}
        }
        Ok(self.env.never_ty())
    }

    /// 检查break语句，验证可选值类型与循环类型匹配。
    fn check_break(&mut self, value: &Option<Box<Expr>>) -> TyResult<Ty> {
        match value {
            Some(v) => {
                self.check_expr(v)?;
            }
            None => {}
        }
        Ok(self.env.never_ty())
    }

    /// 检查continue语句，返回never类型。
    fn check_continue(&mut self) -> TyResult<Ty> {
        Ok(self.env.never_ty())
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}










