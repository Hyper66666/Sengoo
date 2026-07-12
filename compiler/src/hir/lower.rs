//! AST 到 HIR 的转换

use super::item::*;
use super::*;
use crate::ast::{self, Decl, Program, VariantField};
use crate::symbol::SymbolId;
use crate::typeck::TypeEnv;
use std::collections::{HashMap, HashSet};

mod enum_index;
mod expressions;
mod types;

use expressions::{lower_body, lower_expr, with_coverage_markers};
use types::lower_type;

/// 将 AST 程序转换为 HIR 模块
pub fn lower_ast(program: &Program, type_env: &TypeEnv) -> Module {
    let enum_index = enum_index::build_enum_variant_index(program);
    enum_index::with_enum_index(enum_index, || lower_ast_inner(program, type_env))
}

pub fn lower_ast_with_coverage(program: &Program, type_env: &TypeEnv) -> Module {
    with_coverage_markers(true, || lower_ast(program, type_env))
}

fn lower_ast_inner(program: &Program, type_env: &TypeEnv) -> Module {
    let mut module = Module::new("main".to_string());
    let class_index = build_class_index(program);
    let trait_index = build_trait_index(program);

    for decl in &program.decls {
        match &decl.kind {
            ast::DeclKind::Class(class_decl) => {
                if let Ok((class_struct, class_impl)) =
                    lower_class_bundle(class_decl, &class_index, &trait_index, type_env)
                {
                    module.add_item(HIRItem::Struct(class_struct));
                    if let Some(impl_item) = class_impl {
                        module.add_item(HIRItem::Impl(impl_item));
                    }
                }
            }
            _ => {
                if let Ok(hir_item) = lower_decl(decl, type_env) {
                    module.add_item(hir_item);
                }
            }
        }
    }

    module
}

#[allow(clippy::needless_lifetimes)]
fn build_class_index(program: &Program) -> HashMap<String, &ast::Class> {
    let mut index = HashMap::new();
    for decl in &program.decls {
        if let ast::DeclKind::Class(class_decl) = &decl.kind {
            index.insert(class_decl.name.name.clone(), class_decl);
        }
    }
    index
}

fn build_trait_index(program: &Program) -> HashMap<String, &ast::Trait> {
    let mut index = HashMap::new();
    for decl in &program.decls {
        if let ast::DeclKind::Trait(trait_decl) = &decl.kind {
            index.insert(trait_decl.name.name.clone(), trait_decl);
        }
    }
    index
}

fn path_simple_name(path: &ast::Path) -> Option<String> {
    path.as_simple()
        .map(|ident| ident.name.clone())
        .or_else(|| path.segments.last().map(|ident| ident.name.clone()))
}

/// 降低 AST 声明到 HIR 项
fn lower_decl(decl: &Decl, type_env: &TypeEnv) -> Result<HIRItem, String> {
    match &decl.kind {
        ast::DeclKind::Function(fn_decl) => {
            lower_function(fn_decl, type_env).map(HIRItem::Function)
        }
        ast::DeclKind::ExternBlock(extern_block) => {
            lower_extern_block(extern_block, type_env).map(HIRItem::ExternBlock)
        }
        ast::DeclKind::Struct(struct_decl) => {
            lower_struct(struct_decl, type_env).map(HIRItem::Struct)
        }
        ast::DeclKind::Enum(enum_decl) => lower_enum(enum_decl, type_env).map(HIRItem::Enum),
        ast::DeclKind::Class(class_decl) => lower_class(class_decl, type_env).map(HIRItem::Struct),
        ast::DeclKind::Trait(trait_decl) => lower_trait(trait_decl, type_env).map(HIRItem::Trait),
        ast::DeclKind::Impl(impl_decl) => lower_impl(impl_decl, type_env).map(HIRItem::Impl),
        ast::DeclKind::Const(const_decl) => lower_const(const_decl, type_env).map(HIRItem::Const),
        ast::DeclKind::Static(static_decl) => {
            lower_static(static_decl, type_env).map(HIRItem::Static)
        }
        ast::DeclKind::TypeAlias(type_alias) => {
            lower_type_alias(type_alias, type_env).map(HIRItem::TypeAlias)
        }
        _ => Err("Unsupported item type".to_string()),
    }
}

fn path_to_string(path: &ast::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.name.clone())
        .collect::<Vec<_>>()
        .join("::")
}

fn lower_type_param(param: &ast::TypeParam, type_env: &TypeEnv) -> HIRTypeParam {
    let bounds = param
        .bounds
        .iter()
        .map(|bound| HIRTypeParamBound {
            trait_path: path_to_string(&bound.path),
        })
        .collect::<Vec<_>>();
    let default = param.default.as_ref().map(|ty| lower_type(ty, type_env));
    HIRTypeParam {
        name: param.name.name.clone(),
        bounds,
        default,
    }
}

/// 降低函数声明
fn lower_function(fn_decl: &ast::Function, type_env: &TypeEnv) -> Result<HIRFunction, String> {
    lower_function_with_self(fn_decl, type_env, None, None)
}

/// 降低函数声明（带 self 类型和类型前缀）
fn lower_function_with_self(
    fn_decl: &ast::Function,
    type_env: &TypeEnv,
    self_type: Option<HIRType>,
    type_prefix: Option<String>,
) -> Result<HIRFunction, String> {
    let mut name = fn_decl.name.name.clone();

    // 如果有类型前缀（impl 块中的方法），则修饰函数名
    if let Some(prefix) = type_prefix {
        name = format!("{}_{}", prefix, name);
    }

    let is_pub = matches!(fn_decl.vis, ast::Visibility::Public);
    let is_async = fn_decl.is_async;

    let type_params = fn_decl
        .type_params
        .iter()
        .map(|p| lower_type_param(p, type_env))
        .collect();

    // 处理参数：如果有 self_param，先将其转换为普通参数
    let mut params = Vec::new();

    // 处理 self 参数（在 impl 块中）
    if let Some(_self_param) = &fn_decl.self_param {
        if let Some(ty) = &self_type {
            // self 参数的类型是 impl 块的目标类型
            let self_param = if _self_param.is_ref() {
                HIRParam::borrowed("self".to_string(), SymbolId::INVALID, ty.clone())
            } else {
                HIRParam::new("self".to_string(), SymbolId::INVALID, ty.clone())
            };
            params.push(self_param);
        }
    }

    // 处理其他参数
    for p in &fn_decl.params {
        let ty = lower_type(&p.ty, type_env);
        params.push(HIRParam::new(p.name.name.clone(), p.name.symbol, ty));
    }

    let return_type = fn_decl
        .return_type
        .as_ref()
        .map_or(HIRType::unit(), |t| lower_type(t, type_env));
    let precondition = fn_decl
        .precondition
        .as_ref()
        .map(|expr| lower_expr(expr, type_env))
        .transpose()?;
    let postcondition = fn_decl
        .postcondition
        .as_ref()
        .map(|expr| lower_expr(expr, type_env))
        .transpose()?;
    let body = lower_body(&fn_decl.body, type_env);

    Ok(HIRFunction {
        name,
        type_params,
        params,
        return_type,
        precondition,
        postcondition,
        body,
        is_async,
        abi: fn_decl.abi.clone(),
        is_unsafe: fn_decl.is_unsafe,
        no_mangle: fn_decl.no_mangle,
        export_name: fn_decl.export_name.clone(),
        is_pub,
    })
}

fn lower_extern_block(
    extern_block: &ast::ExternBlock,
    type_env: &TypeEnv,
) -> Result<HIRExternBlock, String> {
    let mut items = Vec::new();

    for item in &extern_block.items {
        match item {
            ast::ExternItem::Function(fn_decl) => {
                let params = fn_decl
                    .params
                    .iter()
                    .map(|p| {
                        let ty = lower_type(&p.ty, type_env);
                        HIRParam::new(p.name.name.clone(), p.name.symbol, ty)
                    })
                    .collect::<Vec<_>>();
                let return_type = fn_decl
                    .return_type
                    .as_ref()
                    .map_or(HIRType::unit(), |t| lower_type(t, type_env));
                items.push(HIRExternItem::Function(HIRExternFunction {
                    name: fn_decl.name.name.clone(),
                    params,
                    return_type,
                    is_unsafe: fn_decl.is_unsafe,
                    is_pub: matches!(fn_decl.vis, ast::Visibility::Public),
                }));
            }
            ast::ExternItem::Static(static_decl) => {
                items.push(HIRExternItem::Static(HIRExternStatic {
                    name: static_decl.name.name.clone(),
                    ty: lower_type(&static_decl.ty, type_env),
                    is_mut: static_decl.is_mut,
                    is_pub: matches!(static_decl.vis, ast::Visibility::Public),
                }));
            }
        }
    }

    Ok(HIRExternBlock {
        abi: extern_block.abi.clone(),
        link_name: extern_block.link_name.clone(),
        items,
    })
}

/// 降低结构体声明
fn lower_struct(struct_decl: &ast::Struct, type_env: &TypeEnv) -> Result<HIRStruct, String> {
    let name = struct_decl.name.name.clone();
    let is_pub = matches!(struct_decl.vis, ast::Visibility::Public);

    let type_params = struct_decl
        .type_params
        .iter()
        .map(|p| lower_type_param(p, type_env))
        .collect();

    let fields = struct_decl
        .fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let ty = lower_type(&f.ty, type_env);
            let is_pub = matches!(f.vis, ast::Visibility::Public);
            let name = f
                .name
                .as_ref()
                .map(|ident| ident.name.clone())
                .unwrap_or_else(|| format!("_{}", i));
            HIRField { name, ty, is_pub }
        })
        .collect();

    Ok(HIRStruct {
        name,
        type_params,
        fields,
        is_pub,
    })
}

/// 降低枚举声明
fn lower_enum(enum_decl: &ast::Enum, type_env: &TypeEnv) -> Result<HIREnum, String> {
    let name = enum_decl.name.name.clone();
    let is_pub = matches!(enum_decl.vis, ast::Visibility::Public);

    let type_params = enum_decl
        .type_params
        .iter()
        .map(|p| lower_type_param(p, type_env))
        .collect();

    let variants = enum_decl
        .variants
        .iter()
        .map(|v| lower_variant(v, type_env))
        .collect();

    Ok(HIREnum {
        name,
        type_params,
        variants,
        is_pub,
    })
}

/// 降低枚举变体
fn lower_variant(variant: &ast::EnumVariant, type_env: &TypeEnv) -> HIRVariant {
    let name = variant.name.name.clone();

    match &variant.fields[..] {
        [] => HIRVariant::Unit(name),
        [field] => match field {
            VariantField::Unnamed(ty) => HIRVariant::Tuple(name, vec![lower_type(ty, type_env)]),
            VariantField::Named(ident, ty) => HIRVariant::Struct(
                name,
                vec![HIRField {
                    name: ident.name.clone(),
                    ty: lower_type(ty, type_env),
                    is_pub: true,
                }],
            ),
        },
        fields => {
            // 检查是否是元组风格（所有字段都是 Unnamed）
            let is_tuple_style = fields.iter().all(|f| matches!(f, VariantField::Unnamed(_)));

            if is_tuple_style {
                let types = fields
                    .iter()
                    .map(|f| {
                        if let VariantField::Unnamed(ty) = f {
                            lower_type(ty, type_env)
                        } else {
                            HIRType::new(HIRTypeKind::Error)
                        }
                    })
                    .collect();
                HIRVariant::Tuple(name, types)
            } else {
                let struct_fields = fields
                    .iter()
                    .filter_map(|f| {
                        if let VariantField::Named(ident, ty) = f {
                            Some(HIRField {
                                name: ident.name.clone(),
                                ty: lower_type(ty, type_env),
                                is_pub: true,
                            })
                        } else {
                            None
                        }
                    })
                    .collect();
                HIRVariant::Struct(name, struct_fields)
            }
        }
    }
}

/// 降低类声明（作为结构体处理）
fn class_parent_name(
    class_decl: &ast::Class,
    class_index: &HashMap<String, &ast::Class>,
) -> Option<String> {
    class_decl.extends.as_ref().and_then(|path| {
        let parent = path_simple_name(path)?;
        if class_index.contains_key(&parent) {
            Some(parent)
        } else {
            None
        }
    })
}

fn class_header_trait_paths(
    class_decl: &ast::Class,
    class_index: &HashMap<String, &ast::Class>,
) -> Vec<String> {
    let mut paths = Vec::new();
    if let Some(path) = &class_decl.extends {
        if let Some(name) = path_simple_name(path) {
            if !class_index.contains_key(&name) {
                paths.push(name);
            }
        }
    }
    for bound in &class_decl.implements {
        if let Some(name) = path_simple_name(&bound.path) {
            paths.push(name);
        }
    }
    paths
}

struct EffectiveClassField<'a> {
    name: String,
    field: &'a ast::StructField,
}

fn resolve_effective_class_fields<'a>(
    class_decl: &'a ast::Class,
    class_index: &HashMap<String, &'a ast::Class>,
    visiting: &mut HashSet<&'a str>,
) -> Result<Vec<EffectiveClassField<'a>>, String> {
    let class_name: &str = &class_decl.name.name;
    if !visiting.insert(class_name) {
        return Err(format!(
            "cyclic class inheritance detected while lowering `{}`",
            class_name
        ));
    }

    let mut merged_fields = Vec::new();
    let mut seen_names = HashSet::new();

    if let Some(parent_name) = class_parent_name(class_decl, class_index) {
        let parent_decl = class_index.get(&parent_name).ok_or_else(|| {
            format!(
                "class `{}` extends unknown parent `{}`",
                class_name, parent_name
            )
        })?;
        let parent_fields = resolve_effective_class_fields(parent_decl, class_index, visiting)?;
        for parent_field in parent_fields {
            seen_names.insert(parent_field.name.clone());
            merged_fields.push(parent_field);
        }
    }

    for (field_index, member) in class_decl.members.iter().enumerate() {
        let ast::ClassMember::Field(field) = member else {
            continue;
        };

        let field_name = field
            .name
            .as_ref()
            .map(|ident| ident.name.clone())
            .unwrap_or_else(|| format!("_{}", field_index));

        if !seen_names.insert(field_name.clone()) {
            return Err(format!(
                "duplicate inherited field `{}` in class `{}`",
                field_name, class_name
            ));
        }

        merged_fields.push(EffectiveClassField {
            name: field_name,
            field,
        });
    }

    visiting.remove(class_name);
    Ok(merged_fields)
}

fn resolve_effective_class_methods<'a>(
    class_decl: &'a ast::Class,
    class_index: &HashMap<String, &'a ast::Class>,
    trait_index: &HashMap<String, &'a ast::Trait>,
    visiting: &mut HashSet<&'a str>,
) -> Result<Vec<&'a ast::Function>, String> {
    let class_name: &str = &class_decl.name.name;
    if !visiting.insert(class_name) {
        return Err(format!(
            "cyclic class inheritance detected while lowering `{}`",
            class_name
        ));
    }

    let mut resolved_methods = if let Some(parent_name) = class_parent_name(class_decl, class_index)
    {
        let parent_decl = class_index.get(&parent_name).ok_or_else(|| {
            format!(
                "class `{}` extends unknown parent `{}`",
                class_name, parent_name
            )
        })?;
        resolve_effective_class_methods(parent_decl, class_index, trait_index, visiting)?
    } else {
        Vec::new()
    };

    let mut index_by_name: HashMap<&'a str, usize> = resolved_methods
        .iter()
        .enumerate()
        .map(|(index, method)| (method.name.name.as_str(), index))
        .collect();
    let mut local_seen: HashSet<&'a str> = HashSet::new();

    for member in &class_decl.members {
        let ast::ClassMember::Method(method) = member else {
            continue;
        };

        let method_name: &'a str = method.name.name.as_str();
        if !local_seen.insert(method_name) {
            return Err(format!(
                "duplicate method `{}` in class `{}`",
                method_name, class_name
            ));
        }

        if let Some(existing_index) = index_by_name.get(method_name).copied() {
            resolved_methods[existing_index] = method;
        } else {
            index_by_name.insert(method_name, resolved_methods.len());
            resolved_methods.push(method);
        }
    }

    for trait_name in class_header_trait_paths(class_decl, class_index) {
        let Some(trait_decl) = trait_index.get(&trait_name) else {
            continue;
        };
        for item in &trait_decl.items {
            let ast::TraitItem::Function(method) = item else {
                continue;
            };
            if method.body.stmts.is_empty() {
                continue;
            }
            let method_name = method.name.name.as_str();
            if index_by_name.contains_key(method_name) {
                continue;
            }
            index_by_name.insert(method_name, resolved_methods.len());
            resolved_methods.push(method);
        }
    }

    visiting.remove(class_name);
    Ok(resolved_methods)
}

fn lower_class_bundle<'a>(
    class_decl: &'a ast::Class,
    class_index: &HashMap<String, &'a ast::Class>,
    trait_index: &HashMap<String, &'a ast::Trait>,
    type_env: &TypeEnv,
) -> Result<(HIRStruct, Option<HIRImpl>), String> {
    let name = class_decl.name.name.clone();
    let is_pub = matches!(class_decl.vis, ast::Visibility::Public);
    let type_params = class_decl
        .type_params
        .iter()
        .map(|p| lower_type_param(p, type_env))
        .collect();

    let mut field_visiting = HashSet::new();
    let effective_fields =
        resolve_effective_class_fields(class_decl, class_index, &mut field_visiting)?;
    let fields = effective_fields
        .into_iter()
        .map(|effective_field| HIRField {
            name: effective_field.name,
            ty: lower_type(&effective_field.field.ty, type_env),
            is_pub: matches!(effective_field.field.vis, ast::Visibility::Public),
        })
        .collect();

    let class_struct = HIRStruct {
        name: name.clone(),
        type_params,
        fields,
        is_pub,
    };

    let mut method_visiting = HashSet::new();
    let effective_methods = resolve_effective_class_methods(
        class_decl,
        class_index,
        trait_index,
        &mut method_visiting,
    )?;
    let self_ty = HIRType::named(name.clone(), vec![]);
    let impl_items = effective_methods
        .iter()
        .filter_map(|method| {
            lower_function_with_self(method, type_env, Some(self_ty.clone()), Some(name.clone()))
                .ok()
        })
        .collect::<Vec<_>>();

    let class_impl = if impl_items.is_empty() {
        None
    } else {
        Some(HIRImpl {
            target_type: self_ty,
            trait_name: None,
            trait_args: Vec::new(),
            items: impl_items,
        })
    };

    Ok((class_struct, class_impl))
}

fn lower_class(class_decl: &ast::Class, type_env: &TypeEnv) -> Result<HIRStruct, String> {
    let mut class_index = HashMap::new();
    class_index.insert(class_decl.name.name.clone(), class_decl);
    let trait_index = HashMap::new();
    let (class_struct, _) = lower_class_bundle(class_decl, &class_index, &trait_index, type_env)?;
    Ok(class_struct)
}

/// 降低 Trait 声明
fn lower_trait(trait_decl: &ast::Trait, type_env: &TypeEnv) -> Result<HIRTrait, String> {
    let name = trait_decl.name.name.clone();
    let is_pub = matches!(trait_decl.vis, ast::Visibility::Public);

    let type_params = trait_decl
        .type_params
        .iter()
        .map(|p| lower_type_param(p, type_env))
        .collect();

    let items = trait_decl
        .items
        .iter()
        .map(|item| match item {
            ast::TraitItem::Function(fn_decl) => lower_function(fn_decl, type_env)
                .map(HIRTraitItem::Function)
                .unwrap_or(HIRTraitItem::Type("_error".to_string())),
            ast::TraitItem::Const(const_decl) => HIRTraitItem::Const(
                const_decl.name.name.clone(),
                lower_type(&const_decl.ty, type_env),
            ),
            ast::TraitItem::Type(type_alias) => HIRTraitItem::Type(type_alias.name.name.clone()),
        })
        .collect();

    Ok(HIRTrait {
        name,
        type_params,
        items,
        is_pub,
    })
}

/// 降低 Impl 声明
fn lower_impl(impl_decl: &ast::Impl, type_env: &TypeEnv) -> Result<HIRImpl, String> {
    let target_type = lower_type(&impl_decl.target_type, type_env);
    let trait_name = impl_decl
        .trait_path
        .as_ref()
        .and_then(|p| p.as_simple())
        .map(|ident| ident.name.clone());
    let trait_args = impl_decl
        .trait_args
        .iter()
        .map(|arg| lower_type(arg, type_env))
        .collect();

    // 生成类型前缀（用于函数名修饰）
    let type_prefix = Some(hir_type_to_prefix(&target_type));

    let items = impl_decl
        .items
        .iter()
        .filter_map(|fn_decl| {
            lower_function_with_self(
                fn_decl,
                type_env,
                Some(target_type.clone()),
                type_prefix.clone(),
            )
            .ok()
        })
        .collect();

    Ok(HIRImpl {
        target_type,
        trait_name,
        trait_args,
        items,
    })
}

/// 将 HIRType 转换为类型前缀字符串
fn hir_type_to_prefix(ty: &HIRType) -> String {
    crate::type_naming::hir_type_prefix(ty)
}

/// 降低常量声明
fn lower_const(const_decl: &ast::Const, type_env: &TypeEnv) -> Result<HIRConst, String> {
    let name = const_decl.name.name.clone();
    let is_pub = matches!(const_decl.vis, ast::Visibility::Public);
    let ty = lower_type(&const_decl.ty, type_env);
    let value = lower_expr(&const_decl.value, type_env)?;

    Ok(HIRConst {
        name,
        ty,
        value,
        is_pub,
    })
}

/// 降低静态变量声明
fn lower_static(static_decl: &ast::Static, type_env: &TypeEnv) -> Result<HIRStatic, String> {
    let name = static_decl.name.name.clone();
    let is_pub = matches!(static_decl.vis, ast::Visibility::Public);
    let ty = lower_type(&static_decl.ty, type_env);
    let value = lower_expr(&static_decl.value, type_env)?;

    Ok(HIRStatic {
        name,
        ty,
        value,
        is_mut: static_decl.is_mut,
        is_pub,
    })
}

/// 降低类型别名
fn lower_type_alias(
    type_alias: &ast::TypeAlias,
    type_env: &TypeEnv,
) -> Result<HIRTypeAlias, String> {
    let name = type_alias.name.name.clone();
    let is_pub = matches!(type_alias.vis, ast::Visibility::Public);

    let type_params = type_alias
        .type_params
        .iter()
        .map(|p| lower_type_param(p, type_env))
        .collect();

    let alias = lower_type(&type_alias.ty, type_env);

    Ok(HIRTypeAlias {
        name,
        type_params,
        alias,
        is_pub,
    })
}
