//! Borrow checking utilities.
//!
//! This is a lightweight borrow-rule checker over AST, intended to provide
//! early diagnostics for obvious mutable/immutable borrow conflicts.

use crate::ast::{Block, DeclKind, Expr, ExprKind, Program, Stmt, StmtKind, UnOp};
use crate::typeck::TypeEnv;
use std::collections::HashMap;

/// Borrow category.
#[derive(Debug, Clone, PartialEq)]
pub enum BorrowKind {
    /// Immutable borrow (`&T`)
    Immutable,
    /// Mutable borrow (`&mut T`)
    Mutable,
}

/// Borrow record.
#[derive(Debug, Clone)]
pub struct Borrow {
    /// Borrow category.
    pub kind: BorrowKind,
    /// Synthetic lifetime id (NLL placeholder).
    pub lifetime: usize,
    /// Source span `(lo, hi)`.
    pub span: (usize, usize),
}

/// Borrow checking errors.
#[derive(Debug, Clone)]
pub enum BorrowError {
    /// Two mutable borrows of same variable overlap.
    MultipleMutableBorrows {
        var: String,
        first_span: (usize, usize),
        second_span: (usize, usize),
    },
    /// Mutable borrow overlaps with immutable borrow.
    MutableWithOtherBorrows {
        var: String,
        mutable_span: (usize, usize),
        other_span: (usize, usize),
    },
    /// Moving a borrowed value (reserved for future precise tracking).
    CannotMoveBorrowed {
        var: String,
        borrow_span: (usize, usize),
        move_span: (usize, usize),
    },
}

/// Borrow checker.
pub struct BorrowChecker {
    /// Type environment snapshot used by this pass.
    _env: TypeEnv,
    /// Active borrows for current scope.
    borrows: HashMap<String, Vec<Borrow>>,
    /// Nested scope borrow snapshots.
    borrow_stack: Vec<HashMap<String, Vec<Borrow>>>,
    /// Synthetic lifetime id counter.
    lifetime_counter: usize,
    /// Collected errors.
    errors: Vec<BorrowError>,
}

impl BorrowChecker {
    /// Create a new checker.
    pub fn new(env: TypeEnv) -> Self {
        Self {
            _env: env,
            borrows: HashMap::new(),
            borrow_stack: Vec::new(),
            lifetime_counter: 0,
            errors: Vec::new(),
        }
    }

    /// Check a whole program.
    pub fn check_program(&mut self, program: &Program) -> std::result::Result<(), Vec<BorrowError>> {
        for decl in &program.decls {
            if let DeclKind::Function(func) = &decl.kind {
                self.check_block(&func.body);
            }
        }
        self.finish()
    }

    /// Check one statement.
    pub fn check_stmt(&mut self, stmt: &Stmt) -> std::result::Result<(), Vec<BorrowError>> {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                if let Some(value) = value {
                    self.check_expr(value);
                    self.track_borrows_in_expr(&name.name, value);
                }
            }
            StmtKind::Const { value, .. } => self.check_expr(value),
            StmtKind::Expr(expr) => self.check_expr(expr),
            StmtKind::Item(_) => {}
        }
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.clone())
        }
    }

    fn finish(&mut self) -> std::result::Result<(), Vec<BorrowError>> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(std::mem::take(&mut self.errors))
        }
    }

    fn push_scope(&mut self) {
        let current = std::mem::take(&mut self.borrows);
        self.borrow_stack.push(current);
        self.borrows = HashMap::new();
    }

    fn pop_scope(&mut self) {
        if let Some(prev) = self.borrow_stack.pop() {
            self.borrows = prev;
        }
    }

    fn check_block(&mut self, block: &Block) {
        self.push_scope();
        for stmt in &block.stmts {
            let _ = self.check_stmt(stmt);
        }
        self.pop_scope();
    }

    fn check_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Unary { op, operand } => {
                match op {
                    UnOp::Ref => self.add_borrow(operand, BorrowKind::Immutable),
                    UnOp::RefMut => self.add_borrow(operand, BorrowKind::Mutable),
                    _ => {}
                }
                self.check_expr(operand);
            }
            ExprKind::Binary { left, right, .. } => {
                self.check_expr(left);
                self.check_expr(right);
            }
            ExprKind::Call { func, args } => {
                self.check_expr(func);
                for arg in args {
                    self.check_expr(arg);
                }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.check_expr(receiver);
                for arg in args {
                    self.check_expr(arg);
                }
            }
            ExprKind::Block(block) => self.check_block(block),
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                self.check_expr(cond);
                self.check_block(then_branch);
                if let Some(else_expr) = else_branch {
                    self.check_expr(else_expr);
                }
            }
            ExprKind::While { cond, body } => {
                self.check_expr(cond);
                self.check_block(body);
            }
            ExprKind::For { iter, body, .. } => {
                self.check_expr(iter);
                self.check_block(body);
            }
            ExprKind::Loop(body) => self.check_block(body),
            ExprKind::Match { scrutinee, arms } => {
                self.check_expr(scrutinee);
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        self.check_expr(guard);
                    }
                    self.check_expr(&arm.body);
                }
            }
            ExprKind::Index { base, index } => {
                self.check_expr(base);
                self.check_expr(index);
            }
            ExprKind::Field { base, .. } => self.check_expr(base),
            ExprKind::Array(elems) | ExprKind::Tuple(elems) => {
                for elem in elems {
                    self.check_expr(elem);
                }
            }
            ExprKind::Struct { fields, base, .. } => {
                for field in fields {
                    self.check_expr(&field.value);
                }
                if let Some(base) = base {
                    self.check_expr(base);
                }
            }
            ExprKind::Assign { target, value } | ExprKind::AssignOp { target, value, .. } => {
                self.check_expr(target);
                self.check_expr(value);
            }
            ExprKind::Range { start, end, .. } => {
                if let Some(start) = start {
                    self.check_expr(start);
                }
                if let Some(end) = end {
                    self.check_expr(end);
                }
            }
            ExprKind::Lambda { body, .. }
            | ExprKind::Try(body)
            | ExprKind::Await(body)
            | ExprKind::Paren(body) => self.check_expr(body),
            ExprKind::Cast { expr, .. } | ExprKind::Is { expr, .. } => self.check_expr(expr),
            ExprKind::Return(value) | ExprKind::Break(value) | ExprKind::Yield(value) => {
                if let Some(value) = value {
                    self.check_expr(value);
                }
            }
            ExprKind::AsyncBlock(block) | ExprKind::ParallelBlock(block) => self.check_block(block),
            ExprKind::Continue
            | ExprKind::Literal(_)
            | ExprKind::Ident(_)
            | ExprKind::Path(_) => {}
        }
    }

    fn add_borrow(&mut self, expr: &Expr, kind: BorrowKind) {
        if let Some(name) = Self::expr_var_name(expr) {
            let lifetime = self.lifetime_counter;
            self.lifetime_counter += 1;
            let span = (expr.span.lo as usize, expr.span.hi as usize);

            if let Some(existing) = self.borrows.get(&name) {
                for borrow in existing {
                    match (&kind, &borrow.kind) {
                        (BorrowKind::Mutable, BorrowKind::Mutable) => {
                            self.errors.push(BorrowError::MultipleMutableBorrows {
                                var: name.clone(),
                                first_span: borrow.span,
                                second_span: span,
                            });
                        }
                        (BorrowKind::Mutable, BorrowKind::Immutable)
                        | (BorrowKind::Immutable, BorrowKind::Mutable) => {
                            self.errors.push(BorrowError::MutableWithOtherBorrows {
                                var: name.clone(),
                                mutable_span: span,
                                other_span: borrow.span,
                            });
                        }
                        _ => {}
                    }
                }
            }

            let borrow = Borrow {
                kind,
                lifetime,
                span,
            };
            self.borrows.entry(name).or_default().push(borrow);
        }
    }

    fn track_borrows_in_expr(&mut self, name: &str, expr: &Expr) {
        match &expr.kind {
            ExprKind::Unary { op, operand } => {
                let kind = match op {
                    UnOp::Ref => Some(BorrowKind::Immutable),
                    UnOp::RefMut => Some(BorrowKind::Mutable),
                    _ => None,
                };
                if let Some(kind) = kind {
                    self.add_borrow(operand, kind);
                    if let Some(source) = Self::expr_var_name(operand) {
                        if let Some(existing) = self.borrows.get(&source).cloned() {
                            self.borrows.insert(name.to_string(), existing);
                        }
                    }
                }
            }
            ExprKind::Binary { left, right, .. } => {
                self.track_borrows_in_expr(name, left);
                self.track_borrows_in_expr(name, right);
            }
            ExprKind::Call { func, args } => {
                self.track_borrows_in_expr(name, func);
                for arg in args {
                    self.track_borrows_in_expr(name, arg);
                }
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                self.track_borrows_in_expr(name, receiver);
                for arg in args {
                    self.track_borrows_in_expr(name, arg);
                }
            }
            ExprKind::Assign { target, value } | ExprKind::AssignOp { target, value, .. } => {
                self.track_borrows_in_expr(name, target);
                self.track_borrows_in_expr(name, value);
            }
            _ => {}
        }
    }

    fn expr_var_name(expr: &Expr) -> Option<String> {
        match &expr.kind {
            ExprKind::Ident(ident) => Some(ident.name.clone()),
            ExprKind::Path(path) => path.as_simple().map(|ident| ident.name.clone()),
            _ => None,
        }
    }

    /// End a borrow lifetime explicitly.
    pub fn end_borrow(&mut self, var: &str, lifetime: usize) {
        if let Some(borrows) = self.borrows.get_mut(var) {
            borrows.retain(|b| b.lifetime != lifetime);
        }
    }
}
