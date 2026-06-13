use crate::ast;
use crate::hir::ty::{FloatKind, IntKind};
use crate::typeck::TypeEnv;

use super::super::{HIRType, HIRTypeKind};

/// 降低 AST 类型到 HIR 类型
#[allow(clippy::only_used_in_recursion)]
pub(super) fn lower_type(ast_type: &ast::Type, _type_env: &TypeEnv) -> HIRType {
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
            let lowered_args = args.iter().map(|arg| lower_type(arg, _type_env)).collect();
            HIRType::named(name.to_string(), lowered_args)
        }
        ast::TypeKind::Tuple(types) => {
            if types.is_empty() {
                HIRType::unit()
            } else {
                let hir_types = types.iter().map(|t| lower_type(t, _type_env)).collect();
                HIRType::tuple(hir_types)
            }
        }
        ast::TypeKind::Array(elem, len) => {
            let elem_ty = lower_type(elem, _type_env);
            HIRType::array(elem_ty, *len as usize)
        }
        ast::TypeKind::Slice(elem) => HIRType::slice(lower_type(elem, _type_env)),
        ast::TypeKind::Ptr { base, .. } => HIRType::pointer(lower_type(base, _type_env)),
        ast::TypeKind::Ref { base, is_mut } => {
            HIRType::reference(*is_mut, lower_type(base, _type_env))
        }
        ast::TypeKind::Fn { params, ret } => {
            let param_types = params.iter().map(|p| lower_type(p, _type_env)).collect();
            let ret_type = Box::new(
                ret.as_ref()
                    .map_or(HIRType::unit(), |r| lower_type(r, _type_env)),
            );
            HIRType::function(param_types, ret_type)
        }
        ast::TypeKind::Dyn(bounds) => HIRType::new(HIRTypeKind::TraitObject(
            bounds
                .iter()
                .map(|bound| path_to_string(&bound.path))
                .collect(),
        )),
        ast::TypeKind::Never => HIRType::never(),
        _ => HIRType::new(HIRTypeKind::Error),
    }
}

/// 从 AST 表达式推断类型（简化版，用于 let 语句类型推断）
fn path_to_string(path: &ast::Path) -> String {
    path.segments
        .iter()
        .map(|segment| segment.name.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

pub(super) fn infer_expr_type(expr: &ast::Expr) -> HIRType {
    match &expr.kind {
        ast::ExprKind::Literal(lit) => match lit {
            ast::Literal::Int(_) => HIRType::int(IntKind::I64),
            ast::Literal::Float(_) => HIRType::float(FloatKind::F64),
            ast::Literal::Bool(_) => HIRType::bool(),
            ast::Literal::String(_) => HIRType::reference(false, HIRType::str()),
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
        ast::ExprKind::Lambda { params, body } => {
            let ret_type = infer_expr_type(body);
            let param_types = params
                .iter()
                .map(|param| infer_lambda_param_type(&param.name, body))
                .collect();
            HIRType::function(param_types, Box::new(ret_type))
        }
        _ => HIRType::int(IntKind::I64), // 默认推断为 int
    }
}

fn infer_lambda_param_type(param_name: &str, body: &ast::Expr) -> HIRType {
    if lambda_body_uses_param_as_bool(param_name, body) {
        HIRType::bool()
    } else {
        HIRType::int(IntKind::I64)
    }
}

fn lambda_body_uses_param_as_bool(param_name: &str, expr: &ast::Expr) -> bool {
    match &expr.kind {
        ast::ExprKind::Unary { op, operand } => {
            matches!(op, crate::ast::UnOp::Not) && expr_mentions_ident(operand, param_name)
        }
        ast::ExprKind::Binary { op, left, right } => match op {
            crate::ast::BinOp::And | crate::ast::BinOp::Or => {
                expr_mentions_ident(left, param_name) || expr_mentions_ident(right, param_name)
            }
            crate::ast::BinOp::Eq | crate::ast::BinOp::NotEq => {
                (expr_is_ident(left, param_name) && expr_is_bool_literal(right))
                    || (expr_is_ident(right, param_name) && expr_is_bool_literal(left))
            }
            _ => false,
        },
        ast::ExprKind::Paren(inner) => lambda_body_uses_param_as_bool(param_name, inner),
        _ => false,
    }
}

fn expr_mentions_ident(expr: &ast::Expr, name: &str) -> bool {
    match &expr.kind {
        ast::ExprKind::Ident(ident) => ident.name == name,
        ast::ExprKind::Path(path) => path.segments.len() == 1 && path.segments[0].name == name,
        ast::ExprKind::Unary { operand, .. } => expr_mentions_ident(operand, name),
        ast::ExprKind::Binary { left, right, .. } => {
            expr_mentions_ident(left, name) || expr_mentions_ident(right, name)
        }
        ast::ExprKind::Paren(inner) => expr_mentions_ident(inner, name),
        _ => false,
    }
}

fn expr_is_ident(expr: &ast::Expr, name: &str) -> bool {
    match &expr.kind {
        ast::ExprKind::Ident(ident) => ident.name == name,
        ast::ExprKind::Path(path) => path.segments.len() == 1 && path.segments[0].name == name,
        _ => false,
    }
}

fn expr_is_bool_literal(expr: &ast::Expr) -> bool {
    matches!(&expr.kind, ast::ExprKind::Literal(ast::Literal::Bool(_)))
}
