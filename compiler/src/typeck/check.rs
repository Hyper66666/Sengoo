//! 类型检查
//!
//! 对 AST 进行遍历和类型检查。

use crate::ast::pattern::Pattern;
use crate::ast::Visibility;
use crate::ast::*;
use crate::error::CompileError;
use crate::typeck::env::{Symbol, SymbolKind, TypeEnv};
use crate::typeck::infer::TypeInfer;
use crate::typeck::r#trait::{type_key, FunctionTy, ImplRegistry, TraitRegistry};
use crate::typeck::ty::{FloatKind, IntKind, Ty, TyKind, TypeckError};
use crate::Result;
use std::collections::{HashMap, HashSet};

// 内部类型检查结果（返回 TypeckError）
type TyResult<T> = std::result::Result<T, TypeckError>;

#[derive(Debug, Clone)]
struct ClassDeclInfo {
    parent: Option<String>,
    fields: Vec<(String, Type)>,
    methods: Vec<Function>,
}

/// 类型检查器
#[derive(Debug)]
pub struct TypeChecker {
    /// 类型环境
    env: TypeEnv,
    /// 类型推断器
    infer: TypeInfer,
    /// Trait 注册表
    trait_registry: TraitRegistry,
    /// Impl 注册表
    impl_registry: ImplRegistry,
    struct_field_defs: HashMap<String, Vec<(String, Type)>>,
    class_decls: HashMap<String, ClassDeclInfo>,
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
            class_decls: HashMap::new(),
        }
    }

    /// 获取类型环境
    pub fn env(&self) -> &TypeEnv {
        &self.env
    }

    /// Consumes the checker and returns the owned type environment.
    pub fn into_env(self) -> TypeEnv {
        self.env
    }

    /// 获取类型推断器
    pub fn infer(&self) -> &TypeInfer {
        &self.infer
    }

    /// 获取 Trait 注册表
    pub fn trait_registry(&self) -> &TraitRegistry {
        &self.trait_registry
    }

    /// 获取 Impl 注册表
    pub fn impl_registry(&self) -> &ImplRegistry {
        &self.impl_registry
    }

    /// 获取 Trait 注册表的可变引用
    pub fn trait_registry_mut(&mut self) -> &mut TraitRegistry {
        &mut self.trait_registry
    }

    /// 获取 Impl 注册表的可变引用
    pub fn impl_registry_mut(&mut self) -> &mut ImplRegistry {
        &mut self.impl_registry
    }

    /// 检查整个程序
    pub fn check_program(&mut self, program: &Program) -> Result<()> {
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
        for decl in &program.decls {
            self.declare_decl(decl)?;
        }

        self.prepare_class_hierarchy(program)?;

        for decl in &program.decls {
            self.check_decl_with_filtered_function_bodies(decl, checked_function_names)?;
        }

        Ok(())
    }

    /// 声明声明（添加到符号表）
    fn declare_decl(&mut self, decl: &Decl) -> Result<()> {
        match &decl.kind {
            DeclKind::Function(fn_decl) => {
                let name = fn_decl.name.name.clone();

                // 收集实际参数类型和返回类型，支持互递归函数调用
                let mut param_types = Vec::new();
                let mut fallback = false;
                for param in &fn_decl.params {
                    match self.check_type(&param.ty) {
                        Ok(ty) => param_types.push(ty),
                        Err(_) => {
                            fallback = true;
                            break;
                        }
                    }
                }

                if fallback {
                    // 类型解析失败时回退到占位符
                    let unit = self.env.unit_ty();
                    let ty = self.env.fn_ty(vec![], unit.clone());
                    self.env.insert_fn(name, ty, vec![], unit);
                } else {
                    let ret_ty = if let Some(ret) = &fn_decl.return_type {
                        self.check_type(ret).unwrap_or_else(|_| self.env.unit_ty())
                    } else {
                        self.env.unit_ty()
                    };

                    let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
                    self.env.insert_fn(name, fn_ty, param_types, ret_ty);
                }
            }
            DeclKind::Struct(struct_decl) => {
                let name = struct_decl.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name, ty);
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
            }
            DeclKind::Enum(enum_decl) => {
                let name = enum_decl.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name, ty);
            }
            DeclKind::Class(class_decl) => {
                let name = class_decl.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name, ty);
            }
            DeclKind::TypeAlias(type_alias) => {
                let name = type_alias.name.name.clone();
                let ty = self.env.error_ty();
                self.env.insert_type(name, ty);
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

    /// 检查声明
    fn check_decl(&mut self, decl: &Decl) -> Result<()> {
        match &decl.kind {
            DeclKind::Function(fn_decl) => {
                self.check_function_decl(fn_decl)?;
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
                let cycle_start = stack.iter().position(|name| name == class_name).unwrap_or(0);
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
            TypeckError::Other(format!("internal error: class `{}` not collected", class_name))
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
            TypeckError::Other(format!("internal error: class `{}` not collected", class_name))
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
        let mut param_types = Vec::new();
        for param in &method.params {
            param_types.push(self.check_type(&param.ty)?);
        }

        let ret_ty = if let Some(ret) = &method.return_type {
            self.check_type(ret)?
        } else {
            self.env.unit_ty()
        };

        Ok(FunctionTy::new(
            method.self_param.is_some(),
            param_types,
            ret_ty,
        ))
    }
    fn check_function_signature_decl(&mut self, fn_decl: &Function) -> Result<()> {
        let mut param_types = Vec::new();
        for param in &fn_decl.params {
            let ty = self.check_type(&param.ty).map_err(CompileError::from)?;
            param_types.push(ty);
        }

        let ret_ty = if let Some(ret) = &fn_decl.return_type {
            self.check_type(ret).map_err(CompileError::from)?
        } else {
            self.env.unit_ty()
        };

        let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
        self.env
            .insert_fn(fn_decl.name.name.clone(), fn_ty, param_types, ret_ty);
        Ok(())
    }

    /// 检查函数声明
    fn check_function_decl(&mut self, fn_decl: &Function) -> Result<()> {
        self.env.push_scope();

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

        // 预注册函数签名，支持递归调用
        let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
        self.env.insert_fn(
            fn_decl.name.name.clone(),
            fn_ty,
            param_types.clone(),
            ret_ty.clone(),
        );

        // Function.body is always present (Block)
        let body_ty = self.check_block(&fn_decl.body)?;

        // 特殊处理：对于 main 函数，如果返回类型是整数类型且函数体返回 ()，则允许
        // 这样 main 函数可以省略最后的 return 0
        let is_main_with_implicit_return = fn_decl.name.name == "main"
            && matches!(body_ty.kind, TyKind::Unit)
            && matches!(ret_ty.kind, TyKind::Int(_));

        if !is_main_with_implicit_return {
            self.infer
                .unify(&body_ty, &ret_ty)
                .map_err(|e| CompileError::from(e))?;
        }

        self.env.pop_scope();

        // 重新注册（覆盖预注册），确保最终签名正确
        let fn_ty = self.env.fn_ty(param_types.clone(), ret_ty.clone());
        self.env
            .insert_fn(fn_decl.name.name.clone(), fn_ty, param_types, ret_ty);

        Ok(())
    }

    /// 检查结构体声明
    fn check_struct_decl(&mut self, struct_decl: &Struct) -> Result<()> {
        self.env.push_scope();

        for field in &struct_decl.fields {
            self.check_type(&field.ty)?;
        }

        self.env.pop_scope();
        Ok(())
    }

    /// 检查枚举声明
    fn check_enum_decl(&mut self, enum_decl: &Enum) -> Result<()> {
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
        Ok(())
    }

    /// 检查类声明
    fn check_class_decl(&mut self, class_decl: &Class) -> Result<()> {
        self.env.push_scope();

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
        self.check_type(&type_alias.ty)?;
        Ok(())
    }

    /// 检查常量声明
    fn check_const_decl(&mut self, const_decl: &Const) -> Result<()> {
        let ty = self.check_type(&const_decl.ty)?;
        let value_ty = self.check_expr(&const_decl.value)?;
        self.infer
            .unify(&ty, &value_ty)
            .map_err(CompileError::from)?;
        Ok(())
    }

    /// 检查静态变量声明
    fn check_static_decl(&mut self, static_decl: &Static) -> Result<()> {
        let ty = self.check_type(&static_decl.ty)?;
        // Static.value is always present
        let value_ty = self.check_expr(&static_decl.value)?;
        self.infer
            .unify(&ty, &value_ty)
            .map_err(CompileError::from)?;
        Ok(())
    }

    /// 检查 Trait 声明
    fn check_trait_decl(&mut self, trait_decl: &Trait) -> Result<()> {
        use crate::typeck::r#trait::{MethodSig, TraitInfo};

        self.env.push_scope();

        let mut trait_info = TraitInfo::new(
            trait_decl.name.name.clone(),
            trait_decl
                .type_params
                .iter()
                .map(|tp| tp.name.name.clone())
                .collect(),
            matches!(trait_decl.vis, Visibility::Public),
        );

        // 收集方法签名
        for item in &trait_decl.items {
            match item {
                TraitItem::Function(method) => {
                    // 收集参数类型
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

                    // 获取返回类型
                    let ret_ty = if let Some(ret) = &method.return_type {
                        self.check_type(ret)?
                    } else {
                        self.env.unit_ty()
                    };

                    // A trait method has a default implementation if its body is non-empty
                    let has_default = !method.body.stmts.is_empty();
                    let sig = if has_default {
                        MethodSig::with_default(has_self, param_types, ret_ty)
                    } else {
                        MethodSig::new(has_self, param_types, ret_ty)
                    };
                    trait_info.add_method(method.name.name.clone(), sig);
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

    /// 检查 Impl 声明
    fn check_impl_decl(&mut self, impl_decl: &Impl) -> Result<()> {
        use crate::typeck::r#trait::type_key;
        use crate::typeck::r#trait::{FunctionTy, ImplInfo};

        self.env.push_scope();

        let target_ty = self.check_type(&impl_decl.target_type)?;
        let target_key = type_key(&target_ty);

        let trait_name = impl_decl
            .trait_path
            .as_ref()
            .and_then(|p| p.as_simple())
            .map(|s| s.name.clone());

        let mut impl_info = ImplInfo::new(target_ty.clone(), trait_name);

        // 收集方法
        for item in &impl_decl.items {
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
                FunctionTy::new(has_self, param_types, ret_ty),
            );
        }

        // 注册到 Impl 注册表
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
                            // and is not overridden — register it in the impl info
                            impl_info.add_method(
                                method_name.clone(),
                                FunctionTy::new(
                                    method_sig.has_self,
                                    method_sig.param_types.clone(),
                                    method_sig.return_type.clone(),
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

    /// 检查类型
    fn check_type(&mut self, ty: &Type) -> TyResult<Ty> {
        Ok(match &ty.kind {
            TypeKind::Path(path) => {
                let name = path
                    .as_simple()
                    .map(|ident| ident.name.as_str())
                    .unwrap_or("");

                if let Some(symbol) = self.env.lookup(name) {
                    if let Some(ty) = symbol.get_ty() {
                        ty.clone()
                    } else {
                        return Err(TypeckError::UndefinedType {
                            name: name.to_string(),
                        });
                    }
                } else {
                    match name {
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
                        _ => {
                            return Err(TypeckError::UndefinedType {
                                name: name.to_string(),
                            })
                        }
                    }
                }
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
            TypeKind::Ptr { base, is_mut } => {
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

    /// 检查表达式
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
            ExprKind::Struct { path, fields, .. } => {
                let name = path
                    .as_simple()
                    .map(|ident| ident.name.clone())
                    .unwrap_or_default();

                let field_defs = self.struct_field_defs.get(&name).cloned().ok_or_else(|| {
                    TypeckError::UndefinedType { name: name.clone() }
                })?;

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
            _ => Ok(self.env.error_ty()),
        }
    }

    /// 检查字面量
    fn check_literal(&mut self, lit: &Literal) -> TyResult<Ty> {
        Ok(match lit {
            Literal::Int(_) => self.env.int_ty(IntKind::I64), // 默认整数字面量为 i64
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

    /// 检查标识符
    fn check_ident(&mut self, ident: &Ident) -> TyResult<Ty> {
        if let Some(symbol) = self.env.lookup(&ident.name) {
            if let Some(ty) = symbol.get_ty() {
                Ok(self.infer.instantiate(ty.clone()))
            } else {
                Err(TypeckError::UndefinedVariable {
                    name: ident.name.clone(),
                })
            }
        } else {
            Err(TypeckError::UndefinedVariable {
                name: ident.name.clone(),
            })
        }
    }

    /// 检查路径
    fn check_path(&mut self, path: &Path) -> TyResult<Ty> {
        let name = path.as_simple().map(|i| i.name.as_str()).unwrap_or("");
        self.check_ident(&Ident::new(name, path.span))
    }

    /// 检查二元表达式
    fn check_binary(&mut self, op: &BinOp, left: &Expr, right: &Expr) -> TyResult<Ty> {
        let left_ty = self.check_expr(left)?;
        let right_ty = self.check_expr(right)?;

        self.infer
            .unify(&left_ty, &right_ty)
            .map_err(|_| TypeckError::TypeMismatch {
                expected: right_ty.kind.clone(),
                found: left_ty.kind.clone(),
            })?;

        Ok(match op {
            BinOp::Add | BinOp::Sub | BinOp::Mul | BinOp::Div | BinOp::Mod => left_ty,
            BinOp::And | BinOp::Or => self.env.bool_ty(),
            BinOp::BitAnd | BinOp::BitOr | BinOp::BitXor | BinOp::Shl | BinOp::Shr => left_ty,
            BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                self.env.bool_ty()
            }
            BinOp::Pipe | BinOp::Compose | BinOp::Range | BinOp::RangeInclusive => left_ty,
        })
    }

    /// 检查一元表达式
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

    /// 检查赋值
    fn check_assign(&mut self, target: &Expr, value: &Expr) -> TyResult<Ty> {
        let target_ty = self.check_expr(target)?;
        let value_ty = self.check_expr(value)?;
        self.infer.unify(&target_ty, &value_ty)?;
        Ok(self.env.unit_ty())
    }

    /// 检查复合赋值
    fn check_assign_op(&mut self, _op: &AssignOp, target: &Expr, value: &Expr) -> TyResult<Ty> {
        let target_ty = self.check_expr(target)?;
        let value_ty = self.check_expr(value)?;
        self.infer.unify(&target_ty, &value_ty)?;
        Ok(self.env.unit_ty())
    }

    /// 检查索引表达式
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

    /// 检查字段访问
    fn check_field(&mut self, base: &Expr, name: &Ident) -> TyResult<Ty> {
        let base_ty = self.check_expr(base)?;

        match &base_ty.kind {
            TyKind::Adt { name: type_name, .. } => {
                let field_defs = self.struct_field_defs.get(type_name).cloned().ok_or_else(|| {
                    TypeckError::FieldNotFound {
                        type_name: type_name.clone(),
                        field_name: name.name.clone(),
                    }
                })?;

                let field_ty = field_defs
                    .into_iter()
                    .find(|(field_name, _)| field_name == &name.name)
                    .map(|(_, field_ty)| field_ty)
                    .ok_or_else(|| TypeckError::FieldNotFound {
                        type_name: type_name.clone(),
                        field_name: name.name.clone(),
                    })?;

                self.check_type(&field_ty)
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

        let func_ty = self.check_expr(func)?;

        if let TyKind::Fn { params, ret, .. } = &func_ty.kind {
            if params.len() != args.len() {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: params.len(),
                    found: args.len(),
                });
            }

            for (arg_ty, arg_expr) in params.iter().zip(args.iter()) {
                let actual_ty = self.check_expr(arg_expr)?;
                self.infer.unify(arg_ty, &actual_ty)?;
            }

            Ok((**ret).clone())
        } else {
            Err(TypeckError::UndefinedFunction {
                name: "closure".to_string(),
            })
        }
    }

    /// 检查方法调用
    fn check_method_call(
        &mut self,
        receiver: &Expr,
        method: &Ident,
        args: &[Expr],
    ) -> TyResult<Ty> {
        use crate::typeck::r#trait::type_key;

        // 获取接收者类型
        let receiver_ty = self.check_expr(receiver)?;
        let receiver_key = type_key(&receiver_ty);

        // 检查参数类型
        let mut arg_types = Vec::new();
        for arg in args {
            arg_types.push(self.check_expr(arg)?);
        }

        let method_name = &method.name;

        // 0. Built-in string methods: .len() on &str returns i64
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

        // 1. 首先查找固有 impl 的方法
        if let Some(fn_ty) = self
            .impl_registry
            .lookup_inherent_method(&receiver_key, method_name)
        {
            // 检查参数数量
            if fn_ty.param_types.len() != args.len() {
                return Err(TypeckError::ArgumentCountMismatch {
                    expected: fn_ty.param_types.len(),
                    found: args.len(),
                });
            }

            // 检查参数类型
            for (expected, actual) in fn_ty.param_types.iter().zip(arg_types.iter()) {
                self.infer.unify(expected, actual)?;
            }

            return Ok(fn_ty.return_type.clone());
        }

        // 2. 查找 Trait impl 的方法
        // 遍历所有 Trait，查找实现的方法
        for trait_name in self.trait_registry.all_traits() {
            if let Some(fn_ty) =
                self.impl_registry
                    .lookup_trait_method(&trait_name, &receiver_key, method_name)
            {
                // 检查参数数量
                if fn_ty.param_types.len() != args.len() {
                    return Err(TypeckError::ArgumentCountMismatch {
                        expected: fn_ty.param_types.len(),
                        found: args.len(),
                    });
                }

                // 检查参数类型
                for (expected, actual) in fn_ty.param_types.iter().zip(arg_types.iter()) {
                    self.infer.unify(expected, actual)?;
                }

                return Ok(fn_ty.return_type.clone());
            }
        }

        // 3. 未找到方法
        Err(TypeckError::MethodNotFound {
            type_name: receiver_key,
            method_name: method_name.clone(),
        })
    }

    /// 检查元组
    fn check_tuple(&mut self, elems: &[Expr]) -> TyResult<Ty> {
        let elem_types = elems
            .iter()
            .map(|e| self.check_expr(e))
            .collect::<TyResult<Vec<_>>>()?;
        Ok(self.env.tuple_ty(elem_types))
    }

    /// 检查数组
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

    /// 检查重复数组
    fn check_repeat(&mut self, elem: &Expr, count: &Expr) -> TyResult<Ty> {
        let elem_ty = self.check_expr(elem)?;
        let count_ty = self.check_expr(count)?;

        if !count_ty.is_int() {
            return Err(TypeckError::TypeMismatch {
                expected: TyKind::Int(IntKind::USize),
                found: count_ty.kind.clone(),
            });
        }

        Ok(self.env.array_ty(elem_ty, 0))
    }

    /// 检查 Lambda 闭包表达式 `|params| body`
    /// Lambda 的类型是函数类型，参数类型会被推断为新的类型变量
    fn check_lambda(&mut self, params: &[Ident], body: &Expr) -> TyResult<Ty> {
        // 为每个参数创建新的类型变量
        let param_tys: Vec<Ty> = params.iter().map(|_| self.infer.fresh_ty_var()).collect();

        // 创建新的作用域来绑定参数
        self.env.push_scope();

        // 将参数绑定到作用域中
        for (param, ty) in params.iter().zip(param_tys.iter()) {
            self.env.insert_var(param.name.clone(), ty.clone());
        }

        // 检查 body 的类型
        let body_ty = self.check_expr(body)?;

        // 弹出参数作用域
        self.env.pop_scope();

        // Lambda 的类型是函数类型 (params -> ret)
        Ok(self.env.fn_ty(param_tys, body_ty))
    }

    /// 检查块
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

    /// 检查语句
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

                // value 是 Option<Box<Expr>>
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
                // check_decl 返回 Result<()>，需要转换错误
                self.check_decl(item)
                    .map_err(|e| TypeckError::Other(e.to_string()))?;
                Ok(None)
            }
        }
    }

    /// 检查 if 表达式
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

    /// 检查 while 循环
    fn check_while(&mut self, cond: &Expr, body: &Block) -> TyResult<Ty> {
        let cond_ty = self.check_expr(cond)?;
        let bool_ty = self.env.bool_ty();
        self.infer.unify(&cond_ty, &bool_ty)?;

        self.check_block(body)?;
        Ok(self.env.unit_ty())
    }

    /// 检查 for 循环
    fn check_for(&mut self, pattern: &Pattern, iter: &Expr, body: &Block) -> TyResult<Ty> {
        self.check_expr(iter)?;
        let elem_ty = self.env.int_ty(IntKind::I64); // 使用 I64 而不是 I32

        self.env.push_scope();

        // 从 pattern 中提取变量名
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

    /// 检查 loop 循环
    fn check_loop(&mut self, body: &Block) -> TyResult<Ty> {
        self.check_block(body)?;
        Ok(self.env.unit_ty())
    }

    /// 检查 match 表达式
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

    /// 检查 return 表达式
    fn check_return(&mut self, value: &Option<Box<Expr>>) -> TyResult<Ty> {
        match value {
            Some(v) => {
                self.check_expr(v)?;
            }
            None => {}
        }
        Ok(self.env.never_ty())
    }

    /// 检查 break 表达式
    fn check_break(&mut self, value: &Option<Box<Expr>>) -> TyResult<Ty> {
        match value {
            Some(v) => {
                self.check_expr(v)?;
            }
            None => {}
        }
        Ok(self.env.never_ty())
    }

    /// 检查 continue 表达式
    fn check_continue(&mut self) -> TyResult<Ty> {
        Ok(self.env.never_ty())
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}
