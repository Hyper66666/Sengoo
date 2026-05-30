use crate::ast;
use crate::symbol::SymbolId;
use crate::typeck::TypeEnv;

use super::super::{
    HIRBinaryOp, HIRBody, HIRExpr, HIRLiteral, HIRMatchArm, HIRPattern, HIRStmt, HIRType,
    HIRUnaryOp,
};
use super::types::{infer_expr_type, lower_type};

/// 解析整数字面量
/// 降低 AST 块到 HIR 块
pub(super) fn lower_body(block: &ast::Block, type_env: &TypeEnv) -> HIRBody {
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
                let hir_ty = lower_type(ty, type_env);
                let hir_value =
                    lower_expr(value.as_ref(), type_env).unwrap_or(HIRExpr::Lit(HIRLiteral::Null));
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
pub(super) fn lower_expr(expr: &ast::Expr, type_env: &TypeEnv) -> Result<HIRExpr, String> {
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
                .unwrap_or(HIRExpr::Lit(HIRLiteral::Null))
        }
        ast::ExprKind::Await(expr) => HIRExpr::Await(Box::new(lower_expr(expr, type_env)?)),
        ast::ExprKind::AsyncBlock(block) => {
            HIRExpr::AsyncBlock(Box::new(lower_body(block, type_env)))
        }
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
