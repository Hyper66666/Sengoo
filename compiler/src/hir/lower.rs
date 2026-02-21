//! AST 到 HIR 的转换

use super::item::*;
use super::*;
use crate::ast::{self, Decl, Program, VariantField};
use crate::hir::ty::{FloatKind, IntKind};
use crate::symbol::SymbolId;
use crate::typeck::TypeEnv;
use std::collections::{HashMap, HashSet};

/// 将 AST 程序转换为 HIR 模块
pub fn lower_ast(program: &Program, type_env: &TypeEnv) -> Module {
    let mut module = Module::new("main".to_string());
    let class_index = build_class_index(program);

    for decl in &program.decls {
        match &decl.kind {
            ast::DeclKind::Class(class_decl) => {
                if let Ok((class_struct, class_impl)) =
                    lower_class_bundle(class_decl, &class_index, type_env)
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

fn build_class_index<'a>(program: &'a Program) -> HashMap<String, &'a ast::Class> {
    let mut index = HashMap::new();
    for decl in &program.decls {
        if let ast::DeclKind::Class(class_decl) = &decl.kind {
            index.insert(class_decl.name.name.clone(), class_decl);
        }
    }
    index
}

/// 降低 AST 声明到 HIR 项
fn lower_decl(decl: &Decl, type_env: &TypeEnv) -> Result<HIRItem, String> {
    match &decl.kind {
        ast::DeclKind::Function(fn_decl) => {
            lower_function(fn_decl, type_env).map(HIRItem::Function)
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
            params.push(HIRParam::new(
                "self".to_string(),
                SymbolId::INVALID,
                ty.clone(),
            ));
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
    let body = lower_body(&fn_decl.body, type_env);

    Ok(HIRFunction {
        name,
        type_params,
        params,
        return_type,
        body,
        is_async,
        is_pub,
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
        .filter_map(|(i, f)| {
            let ty = lower_type(&f.ty, type_env);
            let is_pub = matches!(f.vis, ast::Visibility::Public);
            let name = f
                .name
                .as_ref()
                .map(|ident| ident.name.clone())
                .unwrap_or_else(|| format!("_{}", i));
            Some(HIRField { name, ty, is_pub })
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
fn class_parent_name(class_decl: &ast::Class) -> Option<String> {
    class_decl.extends.as_ref().and_then(|path| {
        path.as_simple()
            .map(|ident| ident.name.clone())
            .or_else(|| path.segments.last().map(|ident| ident.name.clone()))
    })
}

fn resolve_effective_class_fields(
    class_decl: &ast::Class,
    class_index: &HashMap<String, &ast::Class>,
    visiting: &mut HashSet<String>,
) -> Result<Vec<ast::StructField>, String> {
    let class_name = class_decl.name.name.clone();
    if !visiting.insert(class_name.clone()) {
        return Err(format!(
            "cyclic class inheritance detected while lowering `{}`",
            class_name
        ));
    }

    let mut merged_fields = Vec::new();
    let mut seen_names = HashSet::new();

    if let Some(parent_name) = class_parent_name(class_decl) {
        let parent_decl = class_index.get(&parent_name).ok_or_else(|| {
            format!(
                "class `{}` extends unknown parent `{}`",
                class_name, parent_name
            )
        })?;
        let parent_fields = resolve_effective_class_fields(parent_decl, class_index, visiting)?;
        for parent_field in parent_fields {
            if let Some(parent_field_name) = parent_field.name.as_ref() {
                seen_names.insert(parent_field_name.name.clone());
            }
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

        let mut normalized_field = field.clone();
        if normalized_field.name.is_none() {
            normalized_field.name = Some(ast::Ident::with_symbol(
                field_name,
                SymbolId::INVALID,
                field.span,
            ));
        }
        merged_fields.push(normalized_field);
    }

    visiting.remove(&class_name);
    Ok(merged_fields)
}

fn resolve_effective_class_methods(
    class_decl: &ast::Class,
    class_index: &HashMap<String, &ast::Class>,
    visiting: &mut HashSet<String>,
) -> Result<Vec<ast::Function>, String> {
    let class_name = class_decl.name.name.clone();
    if !visiting.insert(class_name.clone()) {
        return Err(format!(
            "cyclic class inheritance detected while lowering `{}`",
            class_name
        ));
    }

    let mut resolved_methods = if let Some(parent_name) = class_parent_name(class_decl) {
        let parent_decl = class_index.get(&parent_name).ok_or_else(|| {
            format!(
                "class `{}` extends unknown parent `{}`",
                class_name, parent_name
            )
        })?;
        resolve_effective_class_methods(parent_decl, class_index, visiting)?
    } else {
        Vec::new()
    };

    let mut index_by_name: HashMap<String, usize> = resolved_methods
        .iter()
        .enumerate()
        .map(|(index, method)| (method.name.name.clone(), index))
        .collect();
    let mut local_seen = HashSet::new();

    for member in &class_decl.members {
        let ast::ClassMember::Method(method) = member else {
            continue;
        };

        let method_name = method.name.name.clone();
        if !local_seen.insert(method_name.clone()) {
            return Err(format!(
                "duplicate method `{}` in class `{}`",
                method_name, class_name
            ));
        }

        if let Some(existing_index) = index_by_name.get(&method_name).copied() {
            resolved_methods[existing_index] = method.clone();
        } else {
            index_by_name.insert(method_name, resolved_methods.len());
            resolved_methods.push(method.clone());
        }
    }

    visiting.remove(&class_name);
    Ok(resolved_methods)
}

fn lower_class_bundle(
    class_decl: &ast::Class,
    class_index: &HashMap<String, &ast::Class>,
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
        .iter()
        .enumerate()
        .map(|(field_index, field)| HIRField {
            name: field
                .name
                .as_ref()
                .map(|ident| ident.name.clone())
                .unwrap_or_else(|| format!("_{}", field_index)),
            ty: lower_type(&field.ty, type_env),
            is_pub: matches!(field.vis, ast::Visibility::Public),
        })
        .collect();

    let class_struct = HIRStruct {
        name: name.clone(),
        type_params,
        fields,
        is_pub,
    };

    let mut method_visiting = HashSet::new();
    let effective_methods =
        resolve_effective_class_methods(class_decl, class_index, &mut method_visiting)?;
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
            items: impl_items,
        })
    };

    Ok((class_struct, class_impl))
}

fn lower_class(class_decl: &ast::Class, type_env: &TypeEnv) -> Result<HIRStruct, String> {
    let mut class_index = HashMap::new();
    class_index.insert(class_decl.name.name.clone(), class_decl);
    let (class_struct, _) = lower_class_bundle(class_decl, &class_index, type_env)?;
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
        items,
    })
}

/// 将 HIRType 转换为类型前缀字符串
fn hir_type_to_prefix(ty: &HIRType) -> String {
    use crate::hir::HIRTypeKind;
    match &ty.kind {
        HIRTypeKind::Int(ik) => format!("i{}", ik.bits()),
        HIRTypeKind::Float(fk) => format!("f{}", fk.bits()),
        HIRTypeKind::Bool => "bool".to_string(),
        HIRTypeKind::Unit => "unit".to_string(),
        HIRTypeKind::Named { name, .. } => name.clone(),
        _ => "unknown".to_string(),
    }
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

/// 降低 AST 类型到 HIR 类型
fn lower_type(ast_type: &ast::Type, type_env: &TypeEnv) -> HIRType {
    match &ast_type.kind {
        ast::TypeKind::Infer => HIRType::new(HIRTypeKind::Error),
        ast::TypeKind::Path(path) => {
            let name = path
                .as_simple()
                .map(|ident| ident.name.as_str())
                .unwrap_or("");

            match name {
                "bool" => HIRType::bool(),
                "char" => HIRType::char(),
                "str" => HIRType::str(),
                "i8" => HIRType::int(IntKind::I8),
                "i16" => HIRType::int(IntKind::I16),
                "i32" => HIRType::int(IntKind::I32),
                "i64" => HIRType::int(IntKind::I64),
                "i128" => HIRType::int(IntKind::I128),
                "isize" => HIRType::int(IntKind::ISize),
                "u8" => HIRType::int(IntKind::U8),
                "u16" => HIRType::int(IntKind::U16),
                "u32" => HIRType::int(IntKind::U32),
                "u64" => HIRType::int(IntKind::U64),
                "u128" => HIRType::int(IntKind::U128),
                "usize" => HIRType::int(IntKind::USize),
                "f32" => HIRType::float(FloatKind::F32),
                "f64" => HIRType::float(FloatKind::F64),
                "()" | "unit" => HIRType::unit(),
                _ => HIRType::named(name.to_string(), vec![]),
            }
        }
        ast::TypeKind::PathWithArgs { path, args } => {
            let name = path
                .as_simple()
                .map(|ident| ident.name.as_str())
                .unwrap_or("");
            let lowered_args = args.iter().map(|arg| lower_type(arg, type_env)).collect();
            HIRType::named(name.to_string(), lowered_args)
        }
        ast::TypeKind::Tuple(types) => {
            if types.is_empty() {
                HIRType::unit()
            } else {
                let hir_types = types.iter().map(|t| lower_type(t, type_env)).collect();
                HIRType::tuple(hir_types)
            }
        }
        ast::TypeKind::Array(elem, len) => {
            let elem_ty = lower_type(elem, type_env);
            HIRType::array(elem_ty, *len as usize)
        }
        ast::TypeKind::Slice(elem) => HIRType::slice(lower_type(elem, type_env)),
        ast::TypeKind::Ptr { base, .. } => HIRType::pointer(lower_type(base, type_env)),
        ast::TypeKind::Ref { base, is_mut } => {
            HIRType::reference(*is_mut, lower_type(base, type_env))
        }
        ast::TypeKind::Fn { params, ret } => {
            let param_types = params.iter().map(|p| lower_type(p, type_env)).collect();
            let ret_type = Box::new(
                ret.as_ref()
                    .map_or(HIRType::unit(), |r| lower_type(r, type_env)),
            );
            HIRType::function(param_types, ret_type)
        }
        ast::TypeKind::Never => HIRType::never(),
        _ => HIRType::new(HIRTypeKind::Error),
    }
}

/// 从 AST 表达式推断类型（简化版，用于 let 语句类型推断）
fn infer_expr_type(expr: &ast::Expr) -> HIRType {
    match &expr.kind {
        ast::ExprKind::Literal(lit) => match lit {
            ast::Literal::Int(_) => HIRType::int(IntKind::I64),
            ast::Literal::Float(_) => HIRType::float(FloatKind::F64),
            ast::Literal::Bool(_) => HIRType::bool(),
            ast::Literal::String(_) => HIRType::pointer(HIRType::int(IntKind::I8)),
            ast::Literal::Char(_) => HIRType::int(IntKind::I32),
            ast::Literal::Bytes(_) => HIRType::pointer(HIRType::int(IntKind::U8)),
            ast::Literal::Null => HIRType::pointer(HIRType::unit()),
            ast::Literal::Unit => HIRType::unit(),
        },
        ast::ExprKind::Ident(_) | ast::ExprKind::Path(_) => {
            // 变量引用 - 默认为 i64，实际类型由类型检查器确定
            HIRType::int(IntKind::I64)
        }
        ast::ExprKind::Binary { op, .. } => {
            // 比较运算符返回 bool 类型
            use crate::ast::BinOp;
            match op {
                BinOp::Eq | BinOp::NotEq | BinOp::Lt | BinOp::Le | BinOp::Gt | BinOp::Ge => {
                    HIRType::bool()
                }
                // 逻辑运算符也返回 bool
                BinOp::And | BinOp::Or => HIRType::bool(),
                // 其他运算符返回 int
                _ => HIRType::int(IntKind::I64),
            }
        }
        ast::ExprKind::Unary { op, operand } => {
            use crate::ast::UnOp;
            use crate::hir::HIRTypeKind;
            match op {
                UnOp::Not => HIRType::bool(),
                UnOp::Ref | UnOp::RefMut => {
                    // 引用类型：指向 operand 类型的指针
                    let inner_ty = infer_expr_type(operand);
                    HIRType::pointer(inner_ty)
                }
                UnOp::Deref | UnOp::DerefMut => {
                    // 解引用：尝试获取指针指向的类型
                    let inner_ty = infer_expr_type(operand);
                    match inner_ty.kind {
                        HIRTypeKind::Ptr(inner) => *inner,
                        HIRTypeKind::Ref(_, inner) => *inner,
                        _ => HIRType::int(IntKind::I64), // 默认为 i64
                    }
                }
                _ => HIRType::int(IntKind::I64),
            }
        }
        ast::ExprKind::Array(elems) => {
            // 数组字面量 - 推断元素类型和数组长度
            if elems.is_empty() {
                // 空数组，默认为 i64 数组
                HIRType::array(HIRType::int(IntKind::I64), 0)
            } else {
                // 推断第一个元素的类型
                let elem_ty = infer_expr_type(&elems[0]);
                HIRType::array(elem_ty, elems.len())
            }
        }
        ast::ExprKind::Struct { fields, .. } => {
            // 结构体字面量 - 推断字段类型
            let field_types: Vec<HIRType> =
                fields.iter().map(|fv| infer_expr_type(&fv.value)).collect();
            HIRType::tuple(field_types)
        }
        _ => HIRType::int(IntKind::I64), // 默认推断为 int
    }
}

/// 解析整数字面量
/// 降低 AST 块到 HIR 块
fn lower_body(block: &ast::Block, type_env: &TypeEnv) -> HIRBody {
    let mut hir_body = HIRBody::new();

    let stmts = &block.stmts;
    let (last_idx, last_is_expr) = if stmts.is_empty() {
        (0, false)
    } else {
        let idx = stmts.len() - 1;
        let is_expr = stmts
            .get(idx)
            .map(|s| matches!(&s.kind, ast::StmtKind::Expr(_)))
            .unwrap_or(false);
        (idx, is_expr)
    };

    let stmts_to_process = if last_is_expr {
        &stmts[..last_idx]
    } else {
        stmts
    };

    for stmt in stmts_to_process {
        match &stmt.kind {
            ast::StmtKind::Let {
                name, ty, value, ..
            } => {
                // 如果有显式类型注解，使用它；否则从值表达式推断
                let hir_ty = if let Some(type_annotation) = ty {
                    lower_type(type_annotation, type_env)
                } else if let Some(value_expr) = value {
                    // 从值表达式推断类型
                    infer_expr_type(value_expr)
                } else {
                    // 没有类型注解也没有值，使用默认类型
                    HIRType::unit()
                };
                let hir_value = value.as_ref().and_then(|v| lower_expr(v, type_env).ok());
                hir_body.add_stmt(HIRStmt::Let {
                    name: name.name.clone(),
                    symbol: name.symbol,
                    ty: hir_ty,
                    value: hir_value,
                    is_mut: false,
                });
            }
            ast::StmtKind::Const { name, ty, value } => {
                let hir_ty = lower_type(&ty, type_env);
                let hir_value =
                    lower_expr(&value, type_env).unwrap_or_else(|_| HIRExpr::Lit(HIRLiteral::Null));
                hir_body.add_stmt(HIRStmt::Let {
                    name: name.name.clone(),
                    symbol: name.symbol,
                    ty: hir_ty,
                    value: Some(hir_value),
                    is_mut: false,
                });
            }
            ast::StmtKind::Expr(expr) => {
                if let Ok(hir_expr) = lower_expr(expr, type_env) {
                    hir_body.add_stmt(HIRStmt::Expr(hir_expr));
                }
            }
            ast::StmtKind::Item(_) => {}
        }
    }

    if last_is_expr {
        if let Some(stmt) = stmts.get(last_idx) {
            if let ast::StmtKind::Expr(expr) = &stmt.kind {
                if let Ok(hir_expr) = lower_expr(expr, type_env) {
                    hir_body.set_expr(hir_expr);
                }
            }
        }
    }

    hir_body
}

/// 降低 AST 表达式到 HIR 表达式
fn lower_expr(expr: &ast::Expr, type_env: &TypeEnv) -> Result<HIRExpr, String> {
    Ok(match &expr.kind {
        ast::ExprKind::Literal(lit) => HIRExpr::Lit(lower_literal(lit)),
        ast::ExprKind::Ident(name) => HIRExpr::Var {
            name: name.name.clone(),
            symbol: name.symbol,
        },
        ast::ExprKind::Path(path) => {
            if let Some(ident) = path.as_simple() {
                HIRExpr::Var {
                    name: ident.name.clone(),
                    symbol: ident.symbol,
                }
            } else {
                HIRExpr::Var {
                    name: String::new(),
                    symbol: SymbolId::INVALID,
                }
            }
        }
        ast::ExprKind::Unary { op, operand } => {
            HIRExpr::Unary(lower_un_op(op), Box::new(lower_expr(operand, type_env)?))
        }
        ast::ExprKind::Binary { op, left, right } => HIRExpr::Binary(
            lower_bin_op(op),
            Box::new(lower_expr(left, type_env)?),
            Box::new(lower_expr(right, type_env)?),
        ),
        ast::ExprKind::If {
            cond,
            then_branch,
            else_branch,
        } => {
            HIRExpr::If {
                cond: Box::new(lower_expr(cond, type_env)?),
                then_branch: Box::new(lower_body(then_branch, type_env)),
                else_branch: else_branch.as_ref().and_then(|e| {
                    // 尝试将表达式转换为块
                    match &e.kind {
                        ast::ExprKind::Literal(ast::Literal::Unit) => None,
                        ast::ExprKind::Block(block) => Some(Box::new(lower_body(block, type_env))),
                        _ => {
                            // 将表达式包装在块中
                            let mut body = HIRBody::new();
                            if let Ok(expr) = lower_expr(e, type_env) {
                                body.set_expr(expr);
                            }
                            Some(Box::new(body))
                        }
                    }
                }),
            }
        }
        ast::ExprKind::Match { scrutinee, arms } => {
            let scrutinee = Box::new(lower_expr(scrutinee, type_env)?);
            let hir_arms = arms
                .iter()
                .filter_map(|arm| {
                    if let Some(pat) = arm.patterns.first() {
                        let hir_pat = lower_pattern(pat).ok()?;
                        let hir_guard = arm
                            .guard
                            .as_ref()
                            .and_then(|g| lower_expr(g, type_env).ok())
                            .map(Box::new);
                        let hir_body = Box::new(lower_expr(&arm.body, type_env).ok()?);
                        Some(HIRMatchArm {
                            pat: hir_pat,
                            guard: hir_guard,
                            body: hir_body,
                        })
                    } else {
                        None
                    }
                })
                .collect();
            HIRExpr::Match {
                scrutinee,
                arms: hir_arms,
            }
        }
        ast::ExprKind::Loop(body) => HIRExpr::Loop(Box::new(lower_body(body, type_env))),
        ast::ExprKind::While { cond, body } => HIRExpr::While {
            cond: Box::new(lower_expr(cond, type_env)?),
            body: Box::new(lower_body(body, type_env)),
        },
        ast::ExprKind::For {
            pattern,
            iter,
            body,
        } => {
            let (var_name, var_symbol) = extract_pattern_var_name(pattern);
            HIRExpr::For {
                var_name,
                var_symbol,
                iter: Box::new(lower_expr(iter, type_env)?),
                body: Box::new(lower_body(body, type_env)),
            }
        }
        ast::ExprKind::Call { func, args } => HIRExpr::Call {
            func: Box::new(lower_expr(func, type_env)?),
            args: args
                .iter()
                .filter_map(|a| lower_expr(a, type_env).ok())
                .collect(),
        },
        ast::ExprKind::MethodCall {
            receiver,
            method,
            args,
        } => HIRExpr::MethodCall {
            receiver: Box::new(lower_expr(receiver, type_env)?),
            method: method.name.clone(),
            args: args
                .iter()
                .filter_map(|a| lower_expr(a, type_env).ok())
                .collect(),
        },
        ast::ExprKind::Struct { path, fields, base } => {
            let _ = base; // 暂时忽略 base
            HIRExpr::Struct {
                name: path.as_simple().map(|i| i.name.clone()).unwrap_or_default(),
                fields: fields
                    .iter()
                    .filter_map(|fv| {
                        let name = match &fv.name {
                            ast::FieldName::Ident(ident) => ident.name.clone(),
                            ast::FieldName::String(s) => s.clone(),
                        };
                        Some((name, lower_expr(&fv.value, type_env).ok()?))
                    })
                    .collect(),
            }
        }
        ast::ExprKind::Array(elems) => HIRExpr::Array(
            elems
                .iter()
                .filter_map(|e| lower_expr(e, type_env).ok())
                .collect(),
        ),
        ast::ExprKind::Index { base, index } => HIRExpr::Index {
            base: Box::new(lower_expr(base, type_env)?),
            index: Box::new(lower_expr(index, type_env)?),
        },
        ast::ExprKind::Field { base, field } => HIRExpr::Field {
            base: Box::new(lower_expr(base, type_env)?),
            field: field.name.clone(),
        },
        ast::ExprKind::Assign { target, value } => HIRExpr::Assign {
            target: Box::new(lower_expr(target, type_env)?),
            value: Box::new(lower_expr(value, type_env)?),
        },
        ast::ExprKind::AssignOp { op, target, value } => HIRExpr::AssignOp {
            target: Box::new(lower_expr(target, type_env)?),
            op: lower_assign_op(op),
            value: Box::new(lower_expr(value, type_env)?),
        },
        ast::ExprKind::Return(value) => HIRExpr::Return(
            value
                .as_ref()
                .and_then(|v| lower_expr(v, type_env).ok())
                .map(Box::new),
        ),
        ast::ExprKind::Break(value) => HIRExpr::Break(
            value
                .as_ref()
                .and_then(|v| lower_expr(v, type_env).ok())
                .map(Box::new),
        ),
        ast::ExprKind::Continue => HIRExpr::Continue,
        ast::ExprKind::Block(block) => HIRExpr::Block(Box::new(lower_body(block, type_env))),
        ast::ExprKind::Cast { expr, ty } => HIRExpr::Cast(
            Box::new(lower_expr(expr, type_env)?),
            lower_type(ty, type_env),
        ),
        ast::ExprKind::Tuple(elems) => HIRExpr::Tuple(
            elems
                .iter()
                .filter_map(|e| lower_expr(e, type_env).ok())
                .collect(),
        ),
        ast::ExprKind::Range {
            start,
            end,
            inclusive,
        } => HIRExpr::Range {
            start: start
                .as_ref()
                .and_then(|s| lower_expr(s, type_env).ok())
                .map(Box::new),
            end: end
                .as_ref()
                .and_then(|e| lower_expr(e, type_env).ok())
                .map(Box::new),
            inclusive: *inclusive,
        },
        ast::ExprKind::Is { expr, ty: _ } => {
            // 暂时跳过类型断言
            lower_expr(expr, type_env)?
        }
        ast::ExprKind::Paren(expr) => lower_expr(expr, type_env)?,
        ast::ExprKind::Try(expr) => {
            // 暂时跳过 Try
            lower_expr(expr, type_env)?
        }
        ast::ExprKind::Yield(value) => {
            // 暂时跳过 Yield
            value
                .as_ref()
                .and_then(|v| lower_expr(v, type_env).ok())
                .unwrap_or_else(|| HIRExpr::Lit(HIRLiteral::Null))
        }
        ast::ExprKind::Await(expr) => lower_expr(expr, type_env)?,
        ast::ExprKind::AsyncBlock(block) => HIRExpr::Block(Box::new(lower_body(block, type_env))),
        ast::ExprKind::ParallelBlock(block) => {
            HIRExpr::Block(Box::new(lower_body(block, type_env)))
        }
        ast::ExprKind::Lambda { params, body } => HIRExpr::Lambda {
            params: params.iter().map(|p| p.name.clone()).collect(),
            body: Box::new(lower_expr(body, type_env)?),
        },
    })
}

/// 降低字面量
fn lower_literal(lit: &ast::Literal) -> HIRLiteral {
    match lit {
        ast::Literal::Int(n) => HIRLiteral::Int(*n),
        ast::Literal::Float(f) => HIRLiteral::Float(*f),
        ast::Literal::String(s) => HIRLiteral::String(s.clone()),
        ast::Literal::Bytes(b) => HIRLiteral::Bytes(b.clone()),
        ast::Literal::Char(c) => HIRLiteral::Char(*c),
        ast::Literal::Bool(b) => HIRLiteral::Bool(*b),
        ast::Literal::Null => HIRLiteral::Null,
        ast::Literal::Unit => HIRLiteral::Null,
    }
}

/// 降低一元运算符
fn lower_un_op(op: &ast::UnOp) -> HIRUnaryOp {
    match op {
        ast::UnOp::Plus => HIRUnaryOp::Neg, // 正号通常转换为无操作
        ast::UnOp::Neg => HIRUnaryOp::Neg,
        ast::UnOp::Not => HIRUnaryOp::Not,
        ast::UnOp::BitNot => HIRUnaryOp::BitNot,
        ast::UnOp::Ref => HIRUnaryOp::Ref,
        ast::UnOp::RefMut => HIRUnaryOp::RefMut,
        ast::UnOp::Deref => HIRUnaryOp::Deref,
        ast::UnOp::DerefMut => HIRUnaryOp::Deref,
    }
}

/// 降低二元运算符
fn lower_bin_op(op: &ast::BinOp) -> HIRBinaryOp {
    match op {
        ast::BinOp::Add => HIRBinaryOp::Add,
        ast::BinOp::Sub => HIRBinaryOp::Sub,
        ast::BinOp::Mul => HIRBinaryOp::Mul,
        ast::BinOp::Div => HIRBinaryOp::Div,
        ast::BinOp::Mod => HIRBinaryOp::Mod,
        ast::BinOp::BitAnd => HIRBinaryOp::BitAnd,
        ast::BinOp::BitOr => HIRBinaryOp::BitOr,
        ast::BinOp::BitXor => HIRBinaryOp::BitXor,
        ast::BinOp::Shl => HIRBinaryOp::Shl,
        ast::BinOp::Shr => HIRBinaryOp::Shr,
        ast::BinOp::And => HIRBinaryOp::LogAnd,
        ast::BinOp::Or => HIRBinaryOp::LogOr,
        ast::BinOp::Eq => HIRBinaryOp::Eq,
        ast::BinOp::NotEq => HIRBinaryOp::NotEq,
        ast::BinOp::Lt => HIRBinaryOp::Lt,
        ast::BinOp::Le => HIRBinaryOp::Le,
        ast::BinOp::Gt => HIRBinaryOp::Gt,
        ast::BinOp::Ge => HIRBinaryOp::Ge,
        _ => HIRBinaryOp::Add,
    }
}

/// 降低赋值运算符
fn lower_assign_op(op: &ast::AssignOp) -> HIRBinaryOp {
    match op {
        ast::AssignOp::AddAssign => HIRBinaryOp::Add,
        ast::AssignOp::SubAssign => HIRBinaryOp::Sub,
        ast::AssignOp::MulAssign => HIRBinaryOp::Mul,
        ast::AssignOp::DivAssign => HIRBinaryOp::Div,
        ast::AssignOp::ModAssign => HIRBinaryOp::Mod,
        ast::AssignOp::BitAndAssign => HIRBinaryOp::BitAnd,
        ast::AssignOp::BitOrAssign => HIRBinaryOp::BitOr,
        ast::AssignOp::BitXorAssign => HIRBinaryOp::BitXor,
        ast::AssignOp::ShlAssign => HIRBinaryOp::Shl,
        ast::AssignOp::ShrAssign => HIRBinaryOp::Shr,
        _ => HIRBinaryOp::Add,
    }
}

/// 降低模式
fn lower_pattern(pat: &ast::pattern::Pattern) -> Result<HIRPattern, String> {
    Ok(match &pat.kind {
        ast::pattern::PatternKind::Wildcard => HIRPattern::Wild,
        ast::pattern::PatternKind::Literal(lit) => HIRPattern::Lit(lower_literal(lit)),
        ast::pattern::PatternKind::Ident(name) => HIRPattern::Var {
            name: name.name.clone(),
            symbol: name.symbol,
            mutability: false,
        },
        ast::pattern::PatternKind::Tuple(pats) => {
            HIRPattern::Tuple(pats.iter().filter_map(|p| lower_pattern(p).ok()).collect())
        }
        _ => HIRPattern::Wild,
    })
}

/// 提取模式中的变量名
fn extract_pattern_var_name(pat: &ast::pattern::Pattern) -> (String, SymbolId) {
    match &pat.kind {
        ast::pattern::PatternKind::Ident(name) => (name.name.clone(), name.symbol),
        ast::pattern::PatternKind::Wildcard => ("_loop".to_string(), SymbolId::INVALID),
        _ => ("_loop".to_string(), SymbolId::INVALID),
    }
}
