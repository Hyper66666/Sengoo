//! Type checking for Sengoo programs.
//!
//! This module resolves declarations, checks expressions and statements, records
//! generic metadata, and owns the shared trait/impl registries used by later
//! compiler phases.
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
use std::collections::{BTreeSet, HashMap, HashSet};

type TyResult<T> = std::result::Result<T, TypeckError>;

mod call_helpers;
mod class_hierarchy_helpers;
mod contract_helpers;
mod decl_helpers;
mod expr_helpers;
mod generic_meta_helpers;
mod match_helpers;
mod stmt_helpers;
mod trait_impl_helpers;
mod try_helpers;

#[derive(Debug, Clone)]
struct ClassDeclInfo {
    parent: Option<String>,
    header_traits: Vec<String>,
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

/// Type checker state shared across declaration and body checking.
pub struct TypeChecker {
    /// Type environment populated during declaration and body checking.
    env: TypeEnv,
    /// Type inference engine used to create and solve type variables.
    infer: TypeInfer,
    /// Registered trait declarations and method requirements.
    trait_registry: TraitRegistry,
    /// Registered impl blocks used for method and trait resolution.
    impl_registry: ImplRegistry,
    struct_field_defs: HashMap<String, Vec<(String, Type)>>,
    enum_variants: HashMap<String, Vec<String>>,
    enum_variant_field_tys: HashMap<String, HashMap<String, Vec<Ty>>>,
    struct_type_params: HashMap<String, Vec<TypeParam>>,
    class_decls: HashMap<String, ClassDeclInfo>,
    generic_function_metas: HashMap<String, GenericFunctionMeta>,
    generic_type_metas: HashMap<String, GenericTypeMeta>,
    async_context_depth: usize,
    async_functions: HashSet<String>,
    propagation_stack: Vec<try_helpers::PropagationContext>,
    try_block_mode_stack: Vec<try_helpers::TryBlockMode>,
    warnings: Vec<crate::error::CompileWarning>,
    deprecated_decls: HashMap<String, crate::parser::DeprecatedDecl>,
    trait_default_methods: HashMap<String, HashMap<String, Function>>,
}

impl TypeChecker {
    fn is_async_context_ty(ty: &Ty) -> bool {
        matches!(&ty.kind, TyKind::Adt { name, .. } if name == "AsyncContext")
    }

    pub fn new() -> Self {
        let env = TypeEnv::new();
        let infer = TypeInfer::with_env(env.clone());
        Self {
            env,
            infer,
            trait_registry: TraitRegistry::new(),
            impl_registry: ImplRegistry::new(),
            struct_field_defs: HashMap::new(),
            enum_variants: HashMap::new(),
            enum_variant_field_tys: HashMap::new(),
            struct_type_params: HashMap::new(),
            class_decls: HashMap::new(),
            generic_function_metas: HashMap::new(),
            generic_type_metas: HashMap::new(),
            async_context_depth: 0,
            async_functions: HashSet::new(),
            propagation_stack: Vec::new(),
            try_block_mode_stack: Vec::new(),
            warnings: Vec::new(),
            deprecated_decls: HashMap::new(),
            trait_default_methods: HashMap::new(),
        }
    }

    pub fn warnings(&self) -> &[crate::error::CompileWarning] {
        &self.warnings
    }

    fn load_deprecated_decls(&mut self) {
        for decl in crate::parser::take_deprecated_decls() {
            self.deprecated_decls.insert(decl.name.clone(), decl);
        }
    }

    pub(super) fn warn_deprecated_use(&mut self, name: &str, span: crate::lexer::Span) {
        let Some(info) = self.deprecated_decls.get(name).cloned() else {
            return;
        };
        self.warnings
            .push(crate::error::CompileWarning::deprecated_use(
                info.kind,
                info.name,
                info.message,
                Some((span.lo, span.hi)),
            ));
    }

    pub fn async_function_names(&self) -> &HashSet<String> {
        &self.async_functions
    }

    /// Borrow the current type environment.
    pub fn env(&self) -> &TypeEnv {
        &self.env
    }

    /// Consumes the checker and returns the owned type environment.
    pub fn into_env(self) -> TypeEnv {
        self.env
    }

    /// Borrow the current inference state.
    pub fn infer(&self) -> &TypeInfer {
        &self.infer
    }

    /// Borrow the trait registry.
    pub fn trait_registry(&self) -> &TraitRegistry {
        &self.trait_registry
    }

    /// Borrow the impl registry.
    pub fn impl_registry(&self) -> &ImplRegistry {
        &self.impl_registry
    }

    /// Mutably borrow the trait registry for registration passes.
    pub fn trait_registry_mut(&mut self) -> &mut TraitRegistry {
        &mut self.trait_registry
    }

    /// Mutably borrow the impl registry for registration passes.
    pub fn impl_registry_mut(&mut self) -> &mut ImplRegistry {
        &mut self.impl_registry
    }

    /// Type check a full program, including declarations and function bodies.
    pub fn check_program(&mut self, program: &Program) -> Result<()> {
        self.load_deprecated_decls();
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

    /// Register declarations so later bodies can resolve names.
    fn declare_decl(&mut self, decl: &Decl) -> Result<()> {
        match &decl.kind {
            DeclKind::Function(fn_decl) => {
                let name = fn_decl.name.name.clone();

                if fn_decl.abi.is_some() {
                    if !fn_decl.type_params.is_empty() {
                        return Err(CompileError::from(TypeckError::ffi_signature(
                            "ffi::generic_extern",
                            "generic extern functions are not supported in FFI MVP",
                            fn_decl.span.lo,
                            fn_decl.span.hi,
                        )));
                    }

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

                    self.env.declare_fn(name.clone(), param_types, ret_ty);
                    self.set_generic_function_meta(name, Vec::new());
                    return Ok(());
                }

                // Generic metadata is collected before body checking.
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
                    // If a generic signature cannot be resolved yet, register a unit fallback.
                    let unit = self.env.unit_ty();
                    self.env.declare_fn(name.clone(), vec![], unit);
                    self.set_generic_function_meta(name, Vec::new());
                } else {
                    let ret_ty = if let Some(ret) = &fn_decl.return_type {
                        self.check_type(ret).unwrap_or_else(|_| self.env.unit_ty())
                    } else {
                        self.env.unit_ty()
                    };
                    self.env.pop_scope();

                    self.env.declare_fn(name.clone(), param_types, ret_ty);
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
                            self.env
                                .declare_fn(fn_decl.name.name.clone(), param_types, ret_ty);
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
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                if self.env.owned_string_ty.is_none()
                    && name == "String"
                    && fields.len() == 1
                    && fields[0].0 == "handle"
                {
                    self.env.owned_string_ty = Some(ty.clone());
                }
                self.env.insert_type(name, ty);
                let type_meta = self.collect_generic_type_meta(&struct_decl.type_params);
                self.set_generic_type_meta(struct_decl.name.name.clone(), type_meta);
                self.struct_field_defs
                    .insert(struct_decl.name.name.clone(), fields);
                self.struct_type_params.insert(
                    struct_decl.name.name.clone(),
                    struct_decl.type_params.clone(),
                );
            }
            DeclKind::Enum(enum_decl) => {
                let name = enum_decl.name.name.clone();
                let ty = self.env.new_ty(TyKind::Adt {
                    name: name.clone(),
                    args: vec![],
                });
                self.env.insert_type(name.clone(), ty);
                let variants = enum_decl
                    .variants
                    .iter()
                    .map(|variant| variant.name.name.clone())
                    .collect::<Vec<_>>();
                self.enum_variants.insert(name.clone(), variants);
                self.env.push_scope();
                let enum_meta_and_fields =
                    (|| -> TyResult<(Vec<GenericTypeParamMeta>, HashMap<String, Vec<Ty>>)> {
                        let type_meta = self
                            .bind_type_params_with_meta(&enum_decl.type_params)
                            .map_err(|err| TypeckError::Other(err.to_string()))?;
                        let mut variant_fields = HashMap::new();
                        for variant in &enum_decl.variants {
                            let field_tys = variant
                                .fields
                                .iter()
                                .map(|field| match field {
                                    crate::ast::VariantField::Named(_, ty) => self.check_type(ty),
                                    crate::ast::VariantField::Unnamed(ty) => self.check_type(ty),
                                })
                                .collect::<TyResult<Vec<_>>>()?;
                            variant_fields.insert(variant.name.name.clone(), field_tys);
                        }
                        Ok((type_meta, variant_fields))
                    })();
                self.env.pop_scope();
                let (type_meta, variant_fields) = enum_meta_and_fields?;
                self.enum_variant_field_tys.insert(name, variant_fields);
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

    /// Convert an AST path into the lookup key used by type checking.
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

    pub(super) fn substitute_ty_vars(&self, ty: &Ty, subst: &HashMap<TyVarId, Ty>) -> Ty {
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

    pub(super) fn generic_lookup_key(&self, ty: &Ty) -> String {
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

    pub(super) fn match_generic_impl_target(
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

    pub(super) fn unsatisfied_trait_bound_error(
        context: impl AsRef<str>,
        concrete_type: impl AsRef<str>,
        trait_name: impl AsRef<str>,
        type_param: impl AsRef<str>,
        span_lo: u32,
        span_hi: u32,
    ) -> TypeckError {
        TypeckError::diagnostic(
            "unsatisfied-trait-bound",
            format!(
                "generic constraint violated in `{}`: `{}` does not implement `{}` for `{}`",
                context.as_ref(),
                concrete_type.as_ref(),
                trait_name.as_ref(),
                type_param.as_ref()
            ),
            span_lo,
            span_hi,
        )
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

        let mut explicit_args = explicit_args.into_iter();
        for param in &meta.params {
            let current = if let Some(arg) = explicit_args.next() {
                arg
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

        if name == "Future" {
            if explicit_args.len() != 1 {
                return Err(TypeckError::Other(format!(
                    "type Future expects exactly 1 generic argument, found {}",
                    explicit_args.len()
                )));
            }
            let mut args = explicit_args;
            return Ok(self.env.new_ty(TyKind::Future(Box::new(args.remove(0)))));
        }

        if let Some(meta) = self.generic_type_metas.get(&name) {
            let args = self.resolve_generic_type_args(&name, meta, explicit_args)?;
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

    /// Lower an AST type annotation into an internal type.
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
            ExprKind::Match { scrutinee, arms } => self.check_match(scrutinee, arms, expr.span),
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
                    _ => {
                        let type_key = crate::typeck::r#trait::type_key(&inner_ty);
                        self.impl_registry
                            .get_trait_impl("Future", &type_key)
                            .and_then(|info| info.trait_args.first())
                            .cloned()
                            .ok_or_else(|| {
                                TypeckError::Other(
                                    "await requires a Future value or a type implementing Future<T>"
                                        .to_string(),
                                )
                            })
                    }
                }
            }
            ExprKind::AsyncBlock(block) => {
                self.async_context_depth += 1;
                let result = self.check_block(block);
                self.async_context_depth = self.async_context_depth.saturating_sub(1);
                let inner_ty = result?;
                Ok(Ty::new(0, TyKind::Future(Box::new(inner_ty))))
            }
            ExprKind::Struct { path, fields, .. } => {
                let name = path
                    .as_simple()
                    .map(|ident| ident.name.clone())
                    .unwrap_or_default();
                if name == "AsyncContext" {
                    return Err(TypeckError::Other(
                        "AsyncContext is opaque and cannot be constructed by user code".to_string(),
                    ));
                }

                let field_defs = self
                    .struct_field_defs
                    .get(&name)
                    .cloned()
                    .ok_or_else(|| TypeckError::UndefinedType { name: name.clone() })?;
                let type_params = self
                    .struct_type_params
                    .get(&name)
                    .cloned()
                    .unwrap_or_default();

                if !type_params.is_empty() {
                    self.env.push_scope();
                    let result = (|| -> TyResult<Ty> {
                        let generic_meta = self
                            .bind_type_params_with_meta(&type_params)
                            .map_err(|err| TypeckError::Other(err.to_string()))?;

                        let mut field_types: HashMap<String, Ty> = HashMap::new();
                        for (field_name, field_ty) in &field_defs {
                            field_types.insert(field_name.clone(), self.check_type(field_ty)?);
                        }

                        self.check_struct_literal_fields(&name, fields, &field_types)?;

                        let mut args = Vec::with_capacity(generic_meta.len());
                        for param in &generic_meta {
                            let placeholder = Ty::new(0, TyKind::Var(param.var_id));
                            let mut concrete_ty = self.infer.apply_subst(&placeholder);
                            if matches!(concrete_ty.kind, TyKind::Var(var_id) if var_id == param.var_id)
                            {
                                if let Some(default_ty) = &param.default {
                                    concrete_ty =
                                        self.substitute_ty_vars(default_ty, &HashMap::new());
                                } else {
                                    return Err(TypeckError::Other(format!(
                                        "cannot infer generic argument `{}` for struct `{}` literal",
                                        param.name, name
                                    )));
                                }
                            }
                            for bound in &param.bounds {
                                let concrete_key = type_key(&concrete_ty);
                                if !self.impl_registry.implements_trait(bound, &concrete_key) {
                                    let span = path.segments.last().map(|segment| segment.span);
                                    let (span_lo, span_hi) =
                                        span.map(|span| (span.lo, span.hi)).unwrap_or((0, 0));
                                    return Err(Self::unsatisfied_trait_bound_error(
                                        format!("struct `{name}` literal"),
                                        &concrete_key,
                                        bound,
                                        &param.name,
                                        span_lo,
                                        span_hi,
                                    ));
                                }
                            }
                            args.push(concrete_ty);
                        }

                        Ok(self.env.new_ty(TyKind::Adt { name, args }))
                    })();
                    self.env.pop_scope();
                    result
                } else {
                    let mut field_types: HashMap<String, Ty> = HashMap::new();
                    for (field_name, field_ty) in field_defs {
                        field_types.insert(field_name, self.check_type(&field_ty)?);
                    }

                    self.check_struct_literal_fields(&name, fields, &field_types)?;

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
            ExprKind::Try(operand) => self.check_try_expr(operand),
            ExprKind::TryBlock(block) => self.check_try_block_expr(block),
            _ => Ok(self.env.error_ty()),
        }
    }

    fn check_struct_literal_fields(
        &mut self,
        struct_name: &str,
        fields: &[FieldValue],
        field_types: &HashMap<String, Ty>,
    ) -> TyResult<()> {
        let mut seen = HashSet::new();
        let mut provided_known = HashSet::new();
        let mut missing = BTreeSet::new();
        let mut duplicates = BTreeSet::new();
        let mut unknown = BTreeSet::new();

        for field_value in fields {
            let field_name = match &field_value.name {
                crate::ast::FieldName::Ident(ident) => ident.name.clone(),
                crate::ast::FieldName::String(name) => name.clone(),
            };

            let is_first = seen.insert(field_name.clone());
            if !is_first {
                duplicates.insert(field_name.clone());
            }

            let Some(expected_ty) = field_types.get(&field_name).cloned() else {
                unknown.insert(field_name);
                continue;
            };

            if !is_first {
                continue;
            }

            provided_known.insert(field_name);
            let value_ty = self.check_expr(&field_value.value)?;
            if self.contains_future_escape_ty(&value_ty) {
                return Err(Self::future_escape_error());
            }
            self.infer.unify(&expected_ty, &value_ty)?;
        }

        for field_name in field_types.keys() {
            if !provided_known.contains(field_name) {
                missing.insert(field_name.clone());
            }
        }

        if missing.is_empty() && duplicates.is_empty() && unknown.is_empty() {
            return Ok(());
        }

        Err(Self::invalid_struct_literal_error(
            struct_name,
            &missing,
            &duplicates,
            &unknown,
        ))
    }

    fn invalid_struct_literal_error(
        struct_name: &str,
        missing: &BTreeSet<String>,
        duplicates: &BTreeSet<String>,
        unknown: &BTreeSet<String>,
    ) -> TypeckError {
        let mut parts = Vec::new();
        if !missing.is_empty() {
            parts.push(format!(
                "missing fields: {}",
                Self::format_struct_field_names(missing)
            ));
        }
        if !duplicates.is_empty() {
            parts.push(format!(
                "duplicate fields: {}",
                Self::format_struct_field_names(duplicates)
            ));
        }
        if !unknown.is_empty() {
            parts.push(format!(
                "unknown fields: {}",
                Self::format_struct_field_names(unknown)
            ));
        }

        TypeckError::Other(format!(
            "invalid struct literal `{}`: {}",
            struct_name,
            parts.join("; ")
        ))
    }

    fn format_struct_field_names(fields: &BTreeSet<String>) -> String {
        fields
            .iter()
            .map(|field| format!("`{}`", field))
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn future_escape_error() -> TypeckError {
        TypeckError::Other("future values cannot escape; await the async call directly".to_string())
    }

    fn contains_future_escape_ty(&self, ty: &Ty) -> bool {
        let resolved = self.infer.apply_subst(ty);
        Self::ty_contains_future_escape(&resolved)
    }

    fn ty_contains_future_escape(ty: &Ty) -> bool {
        match &ty.kind {
            TyKind::Future(_) => true,
            TyKind::Tuple(types) => types.iter().any(Self::ty_contains_future_escape),
            TyKind::Array(elem, _) | TyKind::Slice(elem) => Self::ty_contains_future_escape(elem),
            TyKind::Ref(_, inner) | TyKind::Ptr(inner) => Self::ty_contains_future_escape(inner),
            TyKind::Fn { params, ret, .. } => {
                params.iter().any(Self::ty_contains_future_escape)
                    || Self::ty_contains_future_escape(ret)
            }
            TyKind::Adt { args, .. } => args.iter().any(Self::ty_contains_future_escape),
            _ => false,
        }
    }

    pub(super) fn is_cross_thread_send_ty(&self, ty: &Ty) -> bool {
        let resolved = self.infer.apply_subst(ty);
        self.ty_is_cross_thread_send(&resolved)
    }

    fn ty_is_cross_thread_send(&self, ty: &Ty) -> bool {
        match &ty.kind {
            TyKind::Int(_) | TyKind::Bool | TyKind::Float(_) | TyKind::Unit | TyKind::Str => true,
            TyKind::Tuple(types) => types
                .iter()
                .all(|inner| self.ty_is_cross_thread_send(inner)),
            TyKind::Array(elem, _) | TyKind::Slice(elem) => self.ty_is_cross_thread_send(elem),
            TyKind::Adt { name, args } => {
                if Self::is_non_send_runtime_adt(name) {
                    return false;
                }
                args.iter().all(|inner| self.ty_is_cross_thread_send(inner))
            }
            TyKind::Future(_) | TyKind::Ref(_, _) | TyKind::Ptr(_) => false,
            TyKind::Fn { .. } => true,
            _ => false,
        }
    }

    fn is_non_send_runtime_adt(name: &str) -> bool {
        matches!(
            name,
            "Buffer"
                | "AsyncContext"
                | "JsonValue"
                | "ProcessHandle"
                | "ProcessCommand"
                | "ProcessOutput"
                | "DirHandle"
                | "FileHandle"
                | "FfiLibrary"
                | "FfiSymbol"
        )
    }

    pub(super) fn cross_thread_send_error(binding: &str) -> TypeckError {
        TypeckError::Other(format!(
            "cross-thread spawn_blocking_i64 capture `{binding}` is not Send"
        ))
    }
}

impl Default for TypeChecker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::TypeChecker;
    use crate::typeck::ty::{IntKind, Ty, TyKind};

    fn mk(id: usize, kind: TyKind) -> Ty {
        Ty::new(id, kind)
    }

    #[test]
    fn ty_contains_future_escape_rejects_ref_wrapped_future() {
        let future = mk(
            1,
            TyKind::Future(Box::new(mk(2, TyKind::Int(IntKind::I64)))),
        );
        let wrapped = mk(3, TyKind::Ref(false, Box::new(future)));
        assert!(TypeChecker::ty_contains_future_escape(&wrapped));
    }

    #[test]
    fn ty_contains_future_escape_rejects_ptr_wrapped_future() {
        let future = mk(
            1,
            TyKind::Future(Box::new(mk(2, TyKind::Int(IntKind::I64)))),
        );
        let wrapped = mk(3, TyKind::Ptr(Box::new(future)));
        assert!(TypeChecker::ty_contains_future_escape(&wrapped));
    }

    #[test]
    fn ty_contains_future_escape_rejects_fn_returning_future() {
        let future = mk(
            3,
            TyKind::Future(Box::new(mk(4, TyKind::Int(IntKind::I64)))),
        );
        let fn_ty = mk(
            1,
            TyKind::Fn {
                params: vec![mk(2, TyKind::Int(IntKind::I32))],
                ret: Box::new(future),
                is_variadic: false,
            },
        );
        assert!(TypeChecker::ty_contains_future_escape(&fn_ty));
    }
}
