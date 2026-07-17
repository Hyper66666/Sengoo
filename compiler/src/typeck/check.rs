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
use crate::typeck::r#trait::{
    operator_trait_contract, type_key, FunctionTy, ImplInfo, ImplRegistry, MethodSig, TraitInfo,
    TraitRegistry,
};
use crate::typeck::ty::{FloatKind, IntKind, Ty, TyKind, TyVarId, TypeckError};
use crate::Result;
use std::collections::{BTreeSet, HashMap, HashSet};

type TyResult<T> = std::result::Result<T, TypeckError>;
type EnumMetaAndFields = (Vec<GenericTypeParamMeta>, HashMap<String, Vec<Ty>>);

fn type_contains_dyn_trait(ty: &Ty) -> bool {
    match &ty.kind {
        TyKind::Dyn(_) => true,
        TyKind::Ref(_, inner)
        | TyKind::Ptr(inner)
        | TyKind::Array(inner, _)
        | TyKind::Slice(inner)
        | TyKind::Future(inner) => type_contains_dyn_trait(inner),
        TyKind::Tuple(items) => items.iter().any(type_contains_dyn_trait),
        TyKind::Fn { params, ret, .. } => {
            params.iter().any(type_contains_dyn_trait) || type_contains_dyn_trait(ret)
        }
        TyKind::AssocProjection { base, .. } => type_contains_dyn_trait(base),
        TyKind::Adt { args, .. } => args.iter().any(type_contains_dyn_trait),
        _ => false,
    }
}

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
    trait_bounds: Vec<GenericTraitBoundMeta>,
    default: Option<Ty>,
}

#[derive(Debug, Clone)]
struct GenericTraitBoundMeta {
    trait_name: String,
    args: Vec<Ty>,
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
    generic_var_bounds: HashMap<TyVarId, Vec<String>>,
    generic_var_trait_bounds: HashMap<TyVarId, Vec<GenericTraitBoundMeta>>,
    async_context_depth: usize,
    async_functions: HashSet<String>,
    propagation_stack: Vec<try_helpers::PropagationContext>,
    try_block_mode_stack: Vec<try_helpers::TryBlockMode>,
    expected_return_types: Vec<Ty>,
    warnings: Vec<crate::error::CompileWarning>,
    deprecated_decls: HashMap<String, crate::parser::DeprecatedDecl>,
    trait_default_methods: HashMap<String, HashMap<String, Function>>,
    /// Declared supertrait links `(owner_trait, supertrait, span)` collected while
    /// checking trait declarations; validated once all traits are registered.
    pending_supertrait_links: Vec<(String, String, crate::lexer::Span)>,
    /// Trait-impl supertrait obligations `(trait, type_key, span)` collected while
    /// checking impls; validated once every impl has been registered.
    pending_supertrait_obligations: Vec<(String, String, crate::lexer::Span)>,
    current_trait_associated_types: Option<(String, HashSet<String>)>,
}

impl TypeChecker {
    fn is_async_context_ty(ty: &Ty) -> bool {
        matches!(&ty.kind, TyKind::Adt { name, .. } if name == "AsyncContext")
    }

    pub fn new() -> Self {
        let mut env = TypeEnv::new();
        let trait_registry = Self::compiler_known_traits(&mut env);
        Self::compiler_known_support_types(&mut env);
        let impl_registry = Self::compiler_known_impls(&mut env);
        let infer = TypeInfer::with_env(env.clone());
        Self {
            env,
            infer,
            trait_registry,
            impl_registry,
            struct_field_defs: HashMap::new(),
            enum_variants: HashMap::new(),
            enum_variant_field_tys: HashMap::new(),
            struct_type_params: HashMap::new(),
            class_decls: HashMap::new(),
            generic_function_metas: HashMap::new(),
            generic_type_metas: HashMap::new(),
            generic_var_bounds: HashMap::new(),
            generic_var_trait_bounds: HashMap::new(),
            async_context_depth: 0,
            async_functions: HashSet::new(),
            propagation_stack: Vec::new(),
            try_block_mode_stack: Vec::new(),
            expected_return_types: Vec::new(),
            warnings: Vec::new(),
            deprecated_decls: HashMap::new(),
            trait_default_methods: HashMap::new(),
            pending_supertrait_links: Vec::new(),
            pending_supertrait_obligations: Vec::new(),
            current_trait_associated_types: None,
        }
    }

    fn compiler_known_traits(env: &mut TypeEnv) -> TraitRegistry {
        let mut registry = TraitRegistry::new();
        let mut drop_trait = TraitInfo::new("Drop".to_string(), Vec::new(), true);
        drop_trait.add_method(
            "drop".to_string(),
            MethodSig::new(true, Vec::new(), env.unit_ty(), Vec::new()),
        );
        registry.register(drop_trait);
        for trait_name in [
            "Clone",
            "Copy",
            "PartialEq",
            "Eq",
            "PartialOrd",
            "Ord",
            "Default",
            "Display",
            "Debug",
            "Send",
            "Sync",
            "Add",
            "Sub",
            "Mul",
            "Div",
            "Rem",
            "Neg",
        ] {
            registry.register(TraitInfo::new(trait_name.to_string(), Vec::new(), true));
        }
        let mut hash_trait = TraitInfo::new("Hash".to_string(), Vec::new(), true);
        hash_trait.add_method(
            "hash".to_string(),
            MethodSig::new(
                true,
                Vec::new(),
                env.int_ty(crate::typeck::ty::IntKind::I64),
                Vec::new(),
            ),
        );
        registry.register(hash_trait);
        let mut iterator = TraitInfo::new("Iterator".to_string(), Vec::new(), true);
        iterator.add_assoc_type("Item".to_string());
        registry.register(iterator);

        let mut into_iterator = TraitInfo::new("IntoIterator".to_string(), Vec::new(), true);
        into_iterator.add_assoc_type("Item".to_string());
        into_iterator.add_assoc_type("IntoIter".to_string());
        registry.register(into_iterator);
        registry
    }

    fn compiler_known_support_types(env: &mut TypeEnv) {
        for type_name in ["Ordering", "Formatter", "Hasher"] {
            let ty = env.new_ty(TyKind::Adt {
                name: type_name.to_string(),
                args: Vec::new(),
            });
            env.insert_type(type_name.to_string(), ty);
        }
    }

    fn compiler_known_impls(env: &mut TypeEnv) -> ImplRegistry {
        let mut registry = ImplRegistry::new();
        let str_ty = env.new_ty(TyKind::Str);
        let str_ref_ty = env.new_ty(TyKind::Ref(false, Box::new(str_ty.clone())));
        for trait_name in ["PartialEq", "Eq", "PartialOrd", "Ord"] {
            for (key, ty) in [("str", str_ty.clone()), ("&str", str_ref_ty.clone())] {
                let info = ImplInfo::new(ty, Some(trait_name.to_string()), Vec::new());
                registry.register_trait_impl(trait_name.to_string(), key.to_string(), info);
            }
        }
        let mut numeric_types = [
            IntKind::I8,
            IntKind::I16,
            IntKind::I32,
            IntKind::I64,
            IntKind::ISize,
            IntKind::U8,
            IntKind::U16,
            IntKind::U32,
            IntKind::U64,
            IntKind::USize,
        ]
        .into_iter()
        .map(|kind| env.int_ty(kind))
        .collect::<Vec<_>>();
        numeric_types.push(env.float_ty(FloatKind::F32));
        numeric_types.push(env.float_ty(FloatKind::F64));
        for ty in numeric_types {
            let key = type_key(&ty);
            for trait_name in ["PartialEq", "PartialOrd"] {
                let info = ImplInfo::new(ty.clone(), Some(trait_name.to_string()), Vec::new());
                registry.register_trait_impl(trait_name.to_string(), key.clone(), info);
            }
            if !matches!(ty.kind, TyKind::Float(_)) {
                for trait_name in ["Eq", "Ord", "Hash"] {
                    let info = ImplInfo::new(ty.clone(), Some(trait_name.to_string()), Vec::new());
                    registry.register_trait_impl(trait_name.to_string(), key.clone(), info);
                }
            }
            for trait_name in ["Add", "Sub", "Mul", "Div", "Rem"] {
                let info = ImplInfo::new(
                    ty.clone(),
                    Some(trait_name.to_string()),
                    vec![ty.clone(), ty.clone()],
                );
                registry.register_trait_impl(trait_name.to_string(), key.clone(), info);
            }
            let info = ImplInfo::new(ty.clone(), Some("Neg".to_string()), vec![ty]);
            registry.register_trait_impl("Neg".to_string(), key, info);
        }
        let bool_ty = env.bool_ty();
        let bool_key = type_key(&bool_ty);
        for trait_name in ["PartialEq", "Eq", "PartialOrd", "Ord", "Hash"] {
            let info = ImplInfo::new(bool_ty.clone(), Some(trait_name.to_string()), Vec::new());
            registry.register_trait_impl(trait_name.to_string(), bool_key.clone(), info);
        }
        registry
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
            .push(crate::error::CompileWarning::deprecated_use_with_metadata(
                info.kind,
                info.name,
                info.message,
                info.replacement,
                info.removal,
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
        self.generic_var_bounds.clear();
        self.generic_var_trait_bounds.clear();
        self.pending_supertrait_links.clear();
        self.pending_supertrait_obligations.clear();
        for decl in &program.decls {
            self.declare_decl(decl)?;
        }

        self.prepare_class_hierarchy(program)?;

        // Auto-marker opt-outs must be visible while every function body is
        // checked, regardless of where the impl appears in source order.
        for decl in &program.decls {
            if let DeclKind::Impl(impl_decl) = &decl.kind {
                if impl_decl.is_negative {
                    self.check_impl_decl(impl_decl)?;
                }
            }
        }

        for decl in &program.decls {
            if matches!(&decl.kind, DeclKind::Impl(impl_decl) if impl_decl.is_negative) {
                continue;
            }
            self.check_decl(decl)?;
        }

        self.validate_supertrait_obligations()?;

        Ok(())
    }

    pub fn check_program_with_filtered_function_bodies(
        &mut self,
        program: &Program,
        checked_function_names: &HashSet<String>,
    ) -> Result<()> {
        self.generic_function_metas.clear();
        self.generic_type_metas.clear();
        self.generic_var_bounds.clear();
        self.generic_var_trait_bounds.clear();
        self.pending_supertrait_links.clear();
        self.pending_supertrait_obligations.clear();
        for decl in &program.decls {
            self.declare_decl(decl)?;
        }

        self.prepare_class_hierarchy(program)?;

        for decl in &program.decls {
            if let DeclKind::Impl(impl_decl) = &decl.kind {
                if impl_decl.is_negative {
                    self.check_impl_decl(impl_decl)?;
                }
            }
        }

        for decl in &program.decls {
            if matches!(&decl.kind, DeclKind::Impl(impl_decl) if impl_decl.is_negative) {
                continue;
            }
            self.check_decl_with_filtered_function_bodies(decl, checked_function_names)?;
        }

        self.validate_supertrait_obligations()?;

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
                    self.env.mark_drop_owned_type(&ty);
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
                let enum_meta_and_fields = (|| -> TyResult<EnumMetaAndFields> {
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
            TyKind::AssocProjection {
                base,
                trait_name,
                name,
            } => Ty {
                id: ty.id,
                kind: TyKind::AssocProjection {
                    base: Box::new(self.substitute_ty_vars(base, subst)),
                    trait_name: trait_name.clone(),
                    name: name.clone(),
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
        if path.segments.len() == 2 && explicit_args.is_empty() {
            let base_name = &path.segments[0].name;
            let assoc_name = &path.segments[1].name;
            if base_name == "Self" {
                if let Some((trait_name, associated_types)) = &self.current_trait_associated_types {
                    if associated_types.contains(assoc_name) {
                        let self_ty = self.env.new_ty(TyKind::SelfType);
                        return Ok(self.env.new_ty(TyKind::AssocProjection {
                            base: Box::new(self_ty),
                            trait_name: trait_name.clone(),
                            name: assoc_name.clone(),
                        }));
                    }
                }
            }
            if let Some(base) = self.env.lookup(base_name).and_then(Symbol::get_ty).cloned() {
                let TyKind::Var(var_id) = &base.kind else {
                    return Err(TypeckError::Other(format!(
                        "associated type projection `{base_name}::{assoc_name}` currently requires a generic type parameter base"
                    )));
                };
                let bounds = self
                    .generic_var_bounds
                    .get(var_id)
                    .cloned()
                    .unwrap_or_default();
                let mut declaring_traits = bounds.into_iter().filter(|trait_name| {
                    self.trait_registry
                        .get(trait_name)
                        .is_some_and(|info| info.assoc_types.iter().any(|name| name == assoc_name))
                });
                let Some(trait_name) = declaring_traits.next() else {
                    return Err(TypeckError::Other(format!(
                        "associated type `{assoc_name}` is not declared by a bound on `{base_name}`"
                    )));
                };
                if declaring_traits.next().is_some() {
                    return Err(TypeckError::Other(format!(
                        "associated type `{assoc_name}` is ambiguous for `{base_name}`; add an explicit trait binding"
                    )));
                }
                return Ok(self.env.new_ty(TyKind::AssocProjection {
                    base: Box::new(base),
                    trait_name,
                    name: assoc_name.clone(),
                }));
            }
        }

        let name = self.path_name(path)?;

        if name == "Box" && explicit_args.iter().any(type_contains_dyn_trait) {
            return Err(TypeckError::diagnostic(
                "dyn-box-unsupported",
                "`Box<dyn Trait>` is not supported yet",
                path.span.lo,
                path.span.hi,
            ));
        }

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

        if let Some(ty) = self.builtin_type_by_name(&name) {
            return Ok(ty);
        }

        if let Some(symbol) = self.env.lookup(&name) {
            if let Some(ty) = symbol.get_ty() {
                return Ok(ty.clone());
            }
        }

        Err(TypeckError::UndefinedType { name })
    }

    /// Lower an AST type annotation into an internal type.
    fn check_type(&mut self, ty: &Type) -> TyResult<Ty> {
        Ok(match &ty.kind {
            TypeKind::SelfType => self.env.new_ty(TyKind::SelfType),
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
                if trait_bounds.is_empty() {
                    return Err(TypeckError::diagnostic(
                        "invalid-dyn-trait",
                        "`dyn` requires at least one trait bound",
                        ty.span.lo,
                        ty.span.hi,
                    ));
                }
                if trait_bounds.len() > 1 {
                    return Err(TypeckError::diagnostic(
                        "dyn-multi-trait-unsupported",
                        "`dyn A + B` trait objects are not supported yet",
                        ty.span.lo,
                        ty.span.hi,
                    ));
                }

                let mut names = Vec::with_capacity(trait_bounds.len());
                for bound in trait_bounds {
                    if !bound.params.is_empty() {
                        return Err(TypeckError::diagnostic(
                            "invalid-dyn-trait",
                            "`dyn` trait bounds with type arguments are not supported yet",
                            bound.span().lo,
                            bound.span().hi,
                        ));
                    }

                    let Some(ident) = bound.path.as_simple() else {
                        return Err(TypeckError::diagnostic(
                            "invalid-dyn-trait",
                            "`dyn` currently requires a simple trait name",
                            bound.span().lo,
                            bound.span().hi,
                        ));
                    };

                    if !self.is_declared_trait(&ident.name) {
                        return Err(TypeckError::diagnostic(
                            "undefined-dyn-trait",
                            format!("undefined trait `{}` in dyn trait object", ident.name),
                            ident.span.lo,
                            ident.span.hi,
                        ));
                    }

                    self.validate_dyn_associated_type_bindings(bound, &ident.name)?;
                    self.ensure_dyn_trait_object_safe(&ident.name, ident.span.lo, ident.span.hi)?;
                    names.push(ident.name.clone());
                }

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

    fn is_declared_trait(&self, name: &str) -> bool {
        self.trait_registry.contains(name)
            || matches!(
                self.env.lookup(name).map(|symbol| &symbol.kind),
                Some(SymbolKind::Trait { .. })
            )
    }

    fn validate_dyn_associated_type_bindings(
        &mut self,
        bound: &TraitBound,
        trait_name: &str,
    ) -> TyResult<()> {
        let Some(info) = self.trait_registry.get(trait_name) else {
            return Ok(());
        };

        let required = info.assoc_types.clone();
        let required_set = required.iter().cloned().collect::<HashSet<_>>();
        let mut seen = HashSet::new();
        let mut fixed = HashSet::new();

        for binding in &bound.assoc_bindings {
            if !seen.insert(binding.name.clone()) {
                return Err(TypeckError::diagnostic(
                    "dyn-associated-type",
                    format!(
                        "trait object `dyn {trait_name}` fixes associated type `{}` more than once",
                        binding.name
                    ),
                    binding.ty.span.lo,
                    binding.ty.span.hi,
                ));
            }
            if !required_set.contains(&binding.name) {
                return Err(TypeckError::diagnostic(
                    "dyn-associated-type",
                    format!(
                        "trait object `dyn {trait_name}` fixes unknown associated type `{}`",
                        binding.name
                    ),
                    binding.ty.span.lo,
                    binding.ty.span.hi,
                ));
            }
            self.check_type(&binding.ty)?;
            fixed.insert(binding.name.clone());
        }

        let mut missing = required
            .into_iter()
            .filter(|name| !fixed.contains(name))
            .collect::<Vec<_>>();
        if !missing.is_empty() {
            missing.sort();
            return Err(TypeckError::diagnostic(
                "dyn-associated-type",
                format!(
                    "trait object `dyn {trait_name}` must fix associated types: {}",
                    missing.join(", ")
                ),
                bound.span().lo,
                bound.span().hi,
            ));
        }

        Ok(())
    }

    fn ensure_dyn_trait_object_safe(&self, name: &str, lo: u32, hi: u32) -> TyResult<()> {
        let Some(info) = self.trait_registry.get(name) else {
            return Ok(());
        };

        let reason = if !info.type_params.is_empty() {
            Some("traits with type parameters are not object-safe yet".to_string())
        } else if !info.consts.is_empty() {
            Some("traits with associated consts are not object-safe".to_string())
        } else {
            let mut methods = info.methods.iter().collect::<Vec<_>>();
            methods.sort_by_key(|(method_name, _)| method_name.as_str());
            methods.into_iter().find_map(|(method_name, method)| {
                if !method.has_self {
                    return Some(format!("method `{method_name}` has no `self` receiver"));
                }
                if !method.generic_params.is_empty() {
                    return Some(format!("method `{method_name}` has generic parameters"));
                }
                if method.param_types.iter().any(Self::type_contains_bare_self) {
                    return Some(format!(
                        "method `{method_name}` uses `Self` in a parameter type"
                    ));
                }
                if Self::type_contains_unindirected_self(&method.return_type) {
                    return Some(format!("method `{method_name}` returns `Self` by value"));
                }
                None
            })
        };

        if let Some(reason) = reason {
            return Err(TypeckError::diagnostic(
                "not-object-safe",
                format!("trait `{name}` is not object-safe: {reason}"),
                lo,
                hi,
            ));
        }

        Ok(())
    }

    fn type_contains_bare_self(ty: &Ty) -> bool {
        match &ty.kind {
            TyKind::SelfType => true,
            TyKind::Tuple(types) => types.iter().any(Self::type_contains_bare_self),
            TyKind::Array(inner, _) | TyKind::Slice(inner) | TyKind::Ptr(inner) => {
                Self::type_contains_bare_self(inner)
            }
            TyKind::Ref(_, inner) | TyKind::Future(inner) => Self::type_contains_bare_self(inner),
            TyKind::Fn { params, ret, .. } => {
                params.iter().any(Self::type_contains_bare_self)
                    || Self::type_contains_bare_self(ret)
            }
            TyKind::Adt { args, .. } => args.iter().any(Self::type_contains_bare_self),
            _ => false,
        }
    }

    fn type_contains_unindirected_self(ty: &Ty) -> bool {
        match &ty.kind {
            TyKind::SelfType => true,
            TyKind::Ref(_, _) | TyKind::Ptr(_) => false,
            TyKind::Tuple(types) => types.iter().any(Self::type_contains_unindirected_self),
            TyKind::Array(inner, _) | TyKind::Slice(inner) | TyKind::Future(inner) => {
                Self::type_contains_unindirected_self(inner)
            }
            TyKind::Fn { params, ret, .. } => {
                params.iter().any(Self::type_contains_unindirected_self)
                    || Self::type_contains_unindirected_self(ret)
            }
            TyKind::Adt { args, .. } => args.iter().any(Self::type_contains_unindirected_self),
            _ => false,
        }
    }

    fn type_is_fully_concrete(ty: &Ty) -> bool {
        match &ty.kind {
            TyKind::Var(_) | TyKind::Inferred | TyKind::SelfType | TyKind::Error => false,
            TyKind::Tuple(items) => items.iter().all(Self::type_is_fully_concrete),
            TyKind::Array(inner, _)
            | TyKind::Slice(inner)
            | TyKind::Ref(_, inner)
            | TyKind::Ptr(inner)
            | TyKind::Future(inner) => Self::type_is_fully_concrete(inner),
            TyKind::Fn { params, ret, .. } => {
                params.iter().all(Self::type_is_fully_concrete) && Self::type_is_fully_concrete(ret)
            }
            TyKind::Adt { args, .. } => args.iter().all(Self::type_is_fully_concrete),
            TyKind::AssocProjection { .. } => false,
            _ => true,
        }
    }

    fn check_expr_with_expected(&mut self, expr: &Expr, expected: &Ty) -> TyResult<Ty> {
        match &expr.kind {
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                let ty =
                    self.check_method_call(receiver, method, args, Some(expected), expr.span)?;
                if matches!(
                    method.name.as_str(),
                    "get" | "front" | "back" | "iter" | "iter_keys"
                ) {
                    self.env.record_method_return_type(expr.span, ty.clone());
                }
                Ok(ty)
            }
            ExprKind::Paren(inner) => self.check_expr_with_expected(inner, expected),
            ExprKind::Call { func, args } => {
                self.check_call_with_expected(func, args, expr.span, Some(expected))
            }
            ExprKind::Lambda { params, body } => {
                self.check_lambda_with_expected(params, body, expected)
            }
            ExprKind::Struct { .. } => {
                self.expected_return_types.push(expected.clone());
                let result = self.check_expr(expr);
                self.expected_return_types.pop();
                result
            }
            _ => self.check_expr(expr),
        }
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
            ExprKind::Call { func, args } => self.check_call(func, args, expr.span),
            ExprKind::MethodCall {
                receiver,
                method,
                args,
            } => {
                let ty = self.check_method_call(receiver, method, args, None, expr.span)?;
                if matches!(
                    method.name.as_str(),
                    "get" | "front" | "back" | "iter" | "iter_keys"
                ) {
                    self.env.record_method_return_type(expr.span, ty.clone());
                }
                Ok(ty)
            }
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
                if name == "TaskScope" {
                    return Err(TypeckError::Other(
                        "TaskScope is opaque and cannot be constructed by user code".to_string(),
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

                let result = if !type_params.is_empty() {
                    let lexical_type_args = type_params
                        .iter()
                        .filter_map(|param| {
                            self.env
                                .lookup(&param.name.name)
                                .and_then(|symbol| symbol.get_ty())
                                .cloned()
                                .map(|ty| (param.name.name.clone(), ty))
                        })
                        .collect::<HashMap<_, _>>();
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
                        for (param_index, param) in generic_meta.iter().enumerate() {
                            let placeholder = Ty::new(0, TyKind::Var(param.var_id));
                            let mut concrete_ty = self.infer.apply_subst(&placeholder);
                            if matches!(concrete_ty.kind, TyKind::Var(var_id) if var_id == param.var_id)
                            {
                                if let Some(default_ty) = &param.default {
                                    concrete_ty =
                                        self.substitute_ty_vars(default_ty, &HashMap::new());
                                } else if let Some(lexical_ty) = lexical_type_args.get(&param.name)
                                {
                                    concrete_ty = lexical_ty.clone();
                                } else if let Some(expected_arg) =
                                    self.expected_return_types.last().and_then(|expected| {
                                        let TyKind::Adt {
                                            name: expected_name,
                                            args,
                                        } = &expected.kind
                                        else {
                                            return None;
                                        };
                                        (expected_name == &name)
                                            .then(|| args.get(param_index).cloned())
                                            .flatten()
                                    })
                                {
                                    concrete_ty = expected_arg;
                                } else {
                                    return Err(TypeckError::Other(format!(
                                        "cannot infer generic argument `{}` for struct `{}` literal",
                                        param.name, name
                                    )));
                                }
                            }
                            for bound in &param.bounds {
                                let concrete_key = type_key(&concrete_ty);
                                if !self.impl_registry.implements_trait(bound, &concrete_key)
                                    && !self.type_satisfies_auto_marker_bound(bound, &concrete_ty)
                                {
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
                };
                if let Ok(resolved) = &result {
                    if Self::type_is_fully_concrete(resolved) {
                        self.env
                            .record_struct_literal_type(expr.span, resolved.clone());
                    }
                }
                result
            }
            ExprKind::Try(operand) => self.check_try_expr(operand),
            ExprKind::TryBlock(block) => self.check_try_block_expr(block),
            ExprKind::Cast { expr, ty } => self.check_cast(expr, ty),
            ExprKind::Paren(inner) => self.check_expr(inner),
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
        self.ty_satisfies_auto_marker("Send", &resolved, &mut HashSet::new())
    }

    pub(super) fn type_satisfies_auto_marker_bound(&self, trait_name: &str, ty: &Ty) -> bool {
        let resolved = self.infer.apply_subst(ty);
        if trait_name == "Copy" {
            return resolved.is_copy_value();
        }
        if !matches!(trait_name, "Send" | "Sync") {
            return false;
        }
        self.ty_satisfies_auto_marker(trait_name, &resolved, &mut HashSet::new())
    }

    pub(super) fn has_negative_auto_marker_impl(&self, trait_name: &str, ty: &Ty) -> bool {
        self.impl_registry
            .negative_trait_impls(trait_name)
            .iter()
            .any(|pattern| self.match_generic_impl_target(pattern, ty, &mut HashMap::new()))
    }

    pub(super) fn negative_auto_marker_impl_overlaps(&self, trait_name: &str, ty: &Ty) -> bool {
        self.has_negative_auto_marker_impl(trait_name, ty)
            || self
                .impl_registry
                .negative_trait_impls(trait_name)
                .iter()
                .any(|pattern| self.match_generic_impl_target(ty, pattern, &mut HashMap::new()))
    }

    fn ty_satisfies_auto_marker(
        &self,
        trait_name: &str,
        ty: &Ty,
        visiting_adts: &mut HashSet<String>,
    ) -> bool {
        match &ty.kind {
            TyKind::Int(_) | TyKind::Bool | TyKind::Float(_) | TyKind::Unit | TyKind::Str => true,
            TyKind::Tuple(types) => types
                .iter()
                .all(|inner| self.ty_satisfies_auto_marker(trait_name, inner, visiting_adts)),
            TyKind::Array(elem, _) | TyKind::Slice(elem) => {
                self.ty_satisfies_auto_marker(trait_name, elem, visiting_adts)
            }
            TyKind::Adt { name, args } => {
                if self.has_negative_auto_marker_impl(trait_name, ty) {
                    return false;
                }
                if Self::is_non_send_runtime_adt(name) {
                    return false;
                }
                if !args
                    .iter()
                    .all(|inner| self.ty_satisfies_auto_marker(trait_name, inner, visiting_adts))
                {
                    return false;
                }
                if !visiting_adts.insert(name.clone()) {
                    return true;
                }
                let fields_are_send = self.env.struct_field_types_for(ty).is_none_or(|fields| {
                    fields.iter().all(|(_, field_ty)| {
                        self.ty_satisfies_auto_marker(trait_name, field_ty, visiting_adts)
                    })
                });
                let enum_payloads = self.enum_variant_field_tys.get(name).map(|variants| {
                    let subst = self
                        .generic_type_metas
                        .get(name)
                        .map(|meta| {
                            meta.params
                                .iter()
                                .map(|param| param.var_id)
                                .zip(args.iter().cloned())
                                .collect::<HashMap<_, _>>()
                        })
                        .unwrap_or_default();
                    variants
                        .values()
                        .flat_map(|fields| fields.iter())
                        .map(|field_ty| self.substitute_ty_vars(field_ty, &subst))
                        .collect::<Vec<_>>()
                });
                let enum_payloads_are_send = enum_payloads.is_none_or(|fields| {
                    fields.iter().all(|field_ty| {
                        self.ty_satisfies_auto_marker(trait_name, field_ty, visiting_adts)
                    })
                });
                visiting_adts.remove(name);
                fields_are_send && enum_payloads_are_send
            }
            TyKind::Future(_) | TyKind::Ref(_, _) | TyKind::Ptr(_) => false,
            TyKind::Fn { .. } => true,
            _ => false,
        }
    }

    fn is_non_send_runtime_adt(name: &str) -> bool {
        name == "AsyncContext"
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
