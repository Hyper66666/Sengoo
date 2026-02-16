//! 借用规则检查
//!
//! 实现基于非词域生命周期（NLL）的借用检查器

use crate::typeck::TypeEnv;
use crate::ast::{Expr, ExprKind, Stmt, StmtKind};
use std::collections::HashMap;

/// 借用类型
#[derive(Debug, Clone, PartialEq)]
pub enum BorrowKind {
    /// 不可变借用 (&T)
    Immutable,
    /// 可变借用 (&mut T)
    Mutable,
}

/// 借用信息
#[derive(Debug, Clone)]
pub struct Borrow {
    /// 借用类型
    pub kind: BorrowKind,
    /// 借用的生命周期标识（用于 NLL）
    pub lifetime: usize,
    /// 借用发生的位置
    pub span: (usize, usize),
}

/// 借用错误
#[derive(Debug, Clone)]
pub enum BorrowError {
    /// 不能同时有多个可变借用
    MultipleMutableBorrows {
        var: String,
        first_span: (usize, usize),
        second_span: (usize, usize),
    },
    /// 有可变借用时不能有其他借用
    MutableWithOtherBorrows {
        var: String,
        mutable_span: (usize, usize),
        other_span: (usize, usize),
    },
    /// 已借用的值不能被移动
    CannotMoveBorrowed {
        var: String,
        borrow_span: (usize, usize),
        move_span: (usize, usize),
    },
}

/// 借用检查器
pub struct BorrowChecker {
    /// 类型环境
    pub env: TypeEnv,
    /// 当前作用域中的借用
    /// 键是变量名，值是借用的列表
    borrows: HashMap<String, Vec<Borrow>>,
    /// 借用栈（用于跟踪嵌套作用域）
    borrow_stack: Vec<HashMap<String, Vec<Borrow>>>,
    /// 生命周期计数器，用于标识不同的借用
    lifetime_counter: usize,
    /// 收集的错误
    errors: Vec<BorrowError>,
}

impl BorrowChecker {
    /// 创建新的借用检查器
    pub fn new(env: TypeEnv) -> Self {
        Self {
            env,
            borrows: HashMap::new(),
            borrow_stack: Vec::new(),
            lifetime_counter: 0,
            errors: Vec::new(),
        }
    }

    /// 进入新的作用域
    fn push_scope(&mut self) {
        let current = std::mem::take(&mut self.borrows);
        self.borrow_stack.push(current);
        self.borrows = HashMap::new();
    }

    /// 退出作用域
    fn pop_scope(&mut self) {
        if let Some(prev) = self.borrow_stack.pop() {
            self.borrows = prev;
        }
    }

    /// 检查语句
    pub fn check_stmt(&mut self, stmt: &Stmt) -> Result<(), Vec<BorrowError>> {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                // 检查初始化表达式
                self.check_expr(value)?;

                // 如果值是一个借用，记录借用信息
                self.track_borrows_in_expr(name, value);
            }
            StmtKind::Expr(expr) => {
                self.check_expr(expr)?;
            }
            StmtKind::Block(stmts) => {
                self.push_scope();
                for s in stmts {
                    self.check_stmt(s)?;
                }
                self.pop_scope();
            }
            _ => {}
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    /// 检查表达式
    fn check_expr(&mut self, expr: &Expr) -> Result<(), Vec<BorrowError>> {
        match &expr.kind {
            ExprKind::Unary { op, operand } => {
                // 检查引用运算符
                match op {
                    crate::ast::UnOp::Ref => {
                        // 不可变借用
                        self.add_borrow(operand, BorrowKind::Immutable);
                    }
                    crate::ast::UnOp::RefMut => {
                        // 可变借用
                        self.add_borrow(operand, BorrowKind::Mutable);
                    }
                    _ => {}
                }
                self.check_expr(operand)?;
            }
            ExprKind::Binary { left, right, .. } => {
                self.check_expr(left)?;
                self.check_expr(right)?;
            }
            ExprKind::Call { func, args } => {
                self.check_expr(func)?;
                for arg in args {
                    self.check_expr(arg)?;
                }
            }
            ExprKind::Block(stmts) => {
                self.push_scope();
                for s in stmts {
                    self.check_stmt(s)?;
                }
                self.pop_scope();
            }
            ExprKind::If { cond, then_branch, else_branch } => {
                self.check_expr(cond)?;
                self.check_block(then_branch)?;
                if let Some(eb) = else_branch {
                    self.check_block(eb)?;
                }
            }
            ExprKind::While { cond, body } => {
                self.check_expr(cond)?;
                self.check_block(body)?;
            }
            ExprKind::For { var, iter, body } => {
                self.check_expr(iter)?;
                self.check_block(body)?;
            }
            ExprKind::Index { base, index } => {
                self.check_expr(base)?;
                self.check_expr(index)?;
            }
            ExprKind::Field { base, .. } => {
                self.check_expr(base)?;
            }
            _ => {}
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    /// 检查块
    fn check_block(&mut self, stmts: &[Stmt]) -> Result<(), Vec<BorrowError>> {
        self.push_scope();
        for stmt in stmts {
            self.check_stmt(stmt)?;
        }
        self.pop_scope();
        Ok(())
    }

    /// 添加借用
    fn add_borrow(&mut self, expr: &Expr, kind: BorrowKind) {
        // 获取被借用的变量名
        if let ExprKind::Var(name) = &expr.kind {
            let lifetime = self.lifetime_counter;
            self.lifetime_counter += 1;

            // 检查借用规则
            if let Some(existing) = self.borrows.get(name) {
                for borrow in existing {
                    match (&kind, &borrow.kind) {
                        (BorrowKind::Mutable, BorrowKind::Mutable) => {
                            // 不能有多个可变借用
                            self.errors.push(BorrowError::MultipleMutableBorrows {
                                var: name.clone(),
                                first_span: borrow.span,
                                second_span: (0, 0),
                            });
                        }
                        (BorrowKind::Mutable, BorrowKind::Immutable) |
                        (BorrowKind::Immutable, BorrowKind::Mutable) => {
                            // 可变借用与其他借用不能共存
                            self.errors.push(BorrowError::MutableWithOtherBorrows {
                                var: name.clone(),
                                mutable_span: (0, 0),
                                other_span: borrow.span,
                            });
                        }
                        _ => {}
                    }
                }
            }

            // 记录借用
            let borrow = Borrow {
                kind,
                lifetime,
                span: (0, 0),
            };
            self.borrows.entry(name.clone()).or_default().push(borrow);
        }
    }

    /// 跟踪表达式中的借用（用于 let 绑定）
    fn track_borrows_in_expr(&mut self, _name: &str, _expr: &Expr) {
        // TODO: 实现更精确的借用跟踪
    }

    /// 结束借用（用于 NLL）
    pub fn end_borrow(&mut self, var: &str, lifetime: usize) {
        if let Some(borrows) = self.borrows.get_mut(var) {
            borrows.retain(|b| b.lifetime != lifetime);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_multiple_immutable_borrows_ok() {
        // 多个不可变借用应该被允许
        let env = TypeEnv::new();
        let mut checker = BorrowChecker::new(env);

        // &x, &y 应该没有问题
        // (这里只是示例，实际需要构造完整的 AST)
    }

    #[test]
    fn test_mutable_borrow_conflict() {
        // 可变借用冲突检测
        let env = TypeEnv::new();
        let mut checker = BorrowChecker::new(env);

        // &mut x, &mut x 应该报错
        // (这里只是示例，实际需要构造完整的 AST)
    }
}
