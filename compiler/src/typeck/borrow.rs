//! Borrow checking utilities.
//!
//! This is a lightweight borrow-rule checker over AST, intended to provide
//! early diagnostics for obvious mutable/immutable borrow conflicts.

use crate::ast::{Block, DeclKind, Expr, ExprKind, Program, Stmt, StmtKind, UnOp};
use crate::lexer::Span;
use crate::typeck::ty::Ty;
use crate::typeck::TypeEnv;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct MovePath {
    root: String,
    fields: Vec<String>,
}

impl MovePath {
    fn root(name: String) -> Self {
        Self {
            root: name,
            fields: Vec::new(),
        }
    }

    fn child(&self, field: String) -> Self {
        let mut fields = self.fields.clone();
        fields.push(field);
        Self {
            root: self.root.clone(),
            fields,
        }
    }

    fn is_prefix_of(&self, other: &Self) -> bool {
        self.root == other.root
            && self.fields.len() <= other.fields.len()
            && self
                .fields
                .iter()
                .zip(other.fields.iter())
                .all(|(left, right)| left == right)
    }

    fn display(&self) -> String {
        if self.fields.is_empty() {
            self.root.clone()
        } else {
            format!("{}.{}", self.root, self.fields.join("."))
        }
    }
}

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
    /// A borrowed view would outlive the owner scope that produced it.
    BorrowEscapesScope {
        var: String,
        borrow_span: (usize, usize),
        escape_span: (usize, usize),
    },
    /// Use of a `String` value after it was moved.
    UseAfterMove {
        var: String,
        use_span: (usize, usize),
        move_span: (usize, usize),
    },
    /// Use of a parent value after one of its owning fields was moved.
    UseAfterPartialMove {
        var: String,
        use_span: (usize, usize),
        move_span: (usize, usize),
    },
}

/// Borrow checker.
pub struct BorrowChecker {
    /// Type environment snapshot used by this pass.
    _env: TypeEnv,
    /// Active borrows for current scope.
    borrows: HashMap<MovePath, Vec<Borrow>>,
    /// Nested scope borrow snapshots.
    borrow_stack: Vec<HashMap<MovePath, Vec<Borrow>>>,
    /// Synthetic lifetime id counter.
    lifetime_counter: usize,
    /// Collected errors.
    errors: Vec<BorrowError>,
    /// Variables whose owned value was moved in the current scope.
    moved: HashSet<MovePath>,
    /// Nested scope snapshots for moved-variable tracking.
    moved_stack: Vec<HashSet<MovePath>>,
    /// Last move site per variable (for diagnostics).
    move_spans: HashMap<MovePath, (usize, usize)>,
    /// Remaining statements after the one currently being checked (same block).
    /// Used for last-use borrow termination (D1).
    remaining_stmts: Vec<Stmt>,
    /// Binding name -> owner path for borrow aliases (`let view = owner.as_str()`).
    borrow_aliases: HashMap<String, MovePath>,
    /// Nested scope snapshots for alias tracking.
    borrow_alias_stack: Vec<HashMap<String, MovePath>>,
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
            moved: HashSet::new(),
            moved_stack: Vec::new(),
            move_spans: HashMap::new(),
            remaining_stmts: Vec::new(),
            borrow_aliases: HashMap::new(),
            borrow_alias_stack: Vec::new(),
        }
    }

    /// Check a whole program.
    pub fn check_program(
        &mut self,
        program: &Program,
    ) -> std::result::Result<(), Vec<BorrowError>> {
        for decl in &program.decls {
            if let DeclKind::Function(func) = &decl.kind {
                self.check_block(&func.body);
            }
        }
        self.finish()
    }

    pub(crate) fn check_function_block(&mut self, block: &Block) {
        self.check_block_inner(block, true);
    }

    /// Check one statement.
    pub fn check_stmt(&mut self, stmt: &Stmt) -> std::result::Result<(), Vec<BorrowError>> {
        match &stmt.kind {
            StmtKind::Let { name, value, .. } => {
                if let Some(value) = value {
                    self.check_expr(value);
                    self.track_borrows_in_expr(&name.name, value);
                    if let Some(source) = Self::expr_move_path(value) {
                        if self.move_path_is_movable_owning_value(&source) {
                            self.mark_moved(&source, value.span);
                        }
                    }
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

    pub(crate) fn finish(&mut self) -> std::result::Result<(), Vec<BorrowError>> {
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
        let moved = std::mem::take(&mut self.moved);
        self.moved_stack.push(moved);
        self.moved = HashSet::new();
        let aliases = std::mem::take(&mut self.borrow_aliases);
        self.borrow_alias_stack.push(aliases);
        self.borrow_aliases = HashMap::new();
    }

    fn pop_scope(&mut self) {
        if let Some(prev) = self.borrow_stack.pop() {
            self.borrows = prev;
        }
        if let Some(mut prev_moved) = self.moved_stack.pop() {
            for path in std::mem::take(&mut self.moved) {
                if let Some(span) = self.move_spans.remove(&path) {
                    prev_moved.insert(path.clone());
                    self.move_spans.insert(path, span);
                }
            }
            self.moved = prev_moved;
        }
        if let Some(prev_aliases) = self.borrow_alias_stack.pop() {
            self.borrow_aliases = prev_aliases;
        }
    }

    pub(crate) fn check_block(&mut self, block: &Block) {
        self.check_block_inner(block, false);
    }

    fn check_block_inner(&mut self, block: &Block, reject_tail_escape: bool) {
        self.push_scope();
        for (index, stmt) in block.stmts.iter().enumerate() {
            self.remaining_stmts = block.stmts[index + 1..].to_vec();
            let _ = self.check_stmt(stmt);
            // Escape checks must run before last-use pruning clears live tail borrows.
            if reject_tail_escape && index + 1 == block.stmts.len() {
                if let StmtKind::Expr(expr) = &stmt.kind {
                    if !matches!(expr.kind, ExprKind::Return(_)) {
                        self.check_borrow_escape_expr(expr);
                    }
                }
            }
            // D1: after each statement, end borrows whose aliases have no later use.
            self.end_borrows_with_no_remaining_uses();
        }
        self.remaining_stmts.clear();
        self.pop_scope();
    }

    fn end_borrows_with_no_remaining_uses(&mut self) {
        let aliases: Vec<(String, MovePath)> = self
            .borrow_aliases
            .iter()
            .map(|(name, owner)| (name.clone(), owner.clone()))
            .collect();
        for (name, owner) in aliases {
            if self.remaining_uses_ident(&name) {
                continue;
            }
            self.borrows.remove(&MovePath::root(name.clone()));
            self.borrow_aliases.remove(&name);
            let still_aliased = self
                .borrow_aliases
                .values()
                .any(|path| path.is_prefix_of(&owner) || owner.is_prefix_of(path));
            if !still_aliased {
                let owner_keys: Vec<MovePath> = self
                    .borrows
                    .keys()
                    .filter(|borrowed| {
                        borrowed.is_prefix_of(&owner) || owner.is_prefix_of(borrowed)
                    })
                    .cloned()
                    .collect();
                for key in owner_keys {
                    self.borrows.remove(&key);
                }
            }
        }
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
                    self.maybe_move_string_arg(arg);
                }
            }
            ExprKind::MethodCall {
                receiver,
                method,
                args,
                ..
            } => {
                if matches!(method.name.as_str(), "borrow" | "as_str")
                    || (method.name == "get"
                        && self.receiver_is_vec(receiver)
                        && self.method_call_returns_ref(expr))
                    || (matches!(method.name.as_str(), "iter" | "iter_keys")
                        && self.method_call_returns_borrowing_iter(expr))
                    || (matches!(method.name.as_str(), "front" | "back")
                        && self.receiver_is_vec(receiver)
                        && self.method_call_returns_ref(expr))
                {
                    self.add_borrow(receiver, BorrowKind::Immutable);
                }
                self.check_expr(receiver);
                self.check_owned_string_invalidation(receiver, &method.name);
                self.check_vec_borrow_invalidation(receiver, &method.name);
                for arg in args {
                    self.check_expr(arg);
                    self.maybe_move_string_arg(arg);
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
            ExprKind::Field { base, .. } => {
                if let Some(path) = Self::expr_move_path(expr) {
                    self.check_move_path_use(&path, base.span);
                }
            }
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
            ExprKind::Assign { target, value } => {
                self.check_assignment_place(target);
                self.check_expr(value);
                self.maybe_move_assignment_value(target, value);
                if let Some(path) = Self::expr_move_path(target) {
                    self.reinitialize_move_path(&path);
                    self.track_borrow_alias_for_path(&path, value);
                }
            }
            ExprKind::AssignOp { target, value, .. } => {
                self.check_expr(target);
                self.check_expr(value);
                self.maybe_move_assignment_value(target, value);
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
            ExprKind::TryBlock(block) => self.check_block(block),
            ExprKind::Cast { expr, .. } | ExprKind::Is { expr, .. } => self.check_expr(expr),
            ExprKind::Return(value) => {
                if let Some(value) = value {
                    self.check_expr(value);
                    self.check_borrow_escape_expr(value);
                    if let Some(path) = Self::expr_move_path(value) {
                        if self.move_path_is_movable_owning_value(&path) {
                            self.mark_moved(&path, value.span);
                        }
                    }
                }
            }
            ExprKind::Break(value) | ExprKind::Yield(value) => {
                if let Some(value) = value {
                    self.check_expr(value);
                }
            }
            ExprKind::AsyncBlock(block) | ExprKind::ParallelBlock(block) => self.check_block(block),
            ExprKind::Continue | ExprKind::Literal(_) => {}
            ExprKind::Ident(ident) => {
                self.check_move_path_use(&MovePath::root(ident.name.clone()), expr.span);
            }
            ExprKind::Path(path) => {
                if let Some(ident) = path.as_simple() {
                    self.check_move_path_use(&MovePath::root(ident.name.clone()), expr.span);
                }
            }
        }
    }

    fn check_borrow_escape_expr(&mut self, expr: &Expr) {
        match &expr.kind {
            ExprKind::Paren(inner) => self.check_borrow_escape_expr(inner),
            ExprKind::Tuple(elems) | ExprKind::Array(elems) => {
                for elem in elems {
                    self.check_borrow_escape_expr(elem);
                }
            }
            ExprKind::Struct { fields, .. } => {
                for field in fields {
                    self.check_borrow_escape_expr(&field.value);
                }
            }
            ExprKind::If {
                then_branch,
                else_branch,
                ..
            } => {
                self.check_borrow_escape_block_tail(then_branch);
                if let Some(else_expr) = else_branch {
                    self.check_borrow_escape_expr(else_expr);
                }
            }
            ExprKind::Block(block) => self.check_borrow_escape_block_tail(block),
            ExprKind::Match { arms, .. } => {
                for arm in arms {
                    self.check_borrow_escape_expr(&arm.body);
                }
            }
            ExprKind::MethodCall {
                receiver, method, ..
            } if matches!(method.name.as_str(), "borrow" | "as_str")
                || (method.name == "get"
                    && self.receiver_is_vec(receiver)
                    && self.method_call_returns_ref(expr))
                || (matches!(method.name.as_str(), "iter" | "iter_keys")
                    && self.method_call_returns_borrowing_iter(expr)) =>
            {
                let var = Self::expr_move_path(receiver)
                    .map(|path| path.display())
                    .unwrap_or_else(|| "<temporary>".to_string());
                self.errors.push(BorrowError::BorrowEscapesScope {
                    var,
                    borrow_span: (receiver.span.lo as usize, receiver.span.hi as usize),
                    escape_span: (expr.span.lo as usize, expr.span.hi as usize),
                });
            }
            _ => {
                let Some(path) = Self::expr_move_path(expr) else {
                    return;
                };
                let path = MovePath::root(path.root.clone());
                if let Some(active_borrow) = self.borrows.get(&path).and_then(|borrows| {
                    borrows
                        .iter()
                        .find(|borrow| matches!(borrow.kind, BorrowKind::Immutable))
                }) {
                    self.errors.push(BorrowError::BorrowEscapesScope {
                        var: path.display(),
                        borrow_span: active_borrow.span,
                        escape_span: (expr.span.lo as usize, expr.span.hi as usize),
                    });
                }
            }
        }
    }

    fn check_borrow_escape_block_tail(&mut self, block: &Block) {
        if let Some(Stmt {
            kind: StmtKind::Expr(expr),
            ..
        }) = block.stmts.last()
        {
            if !matches!(expr.kind, ExprKind::Return(_)) {
                self.check_borrow_escape_expr(expr);
            }
        }
    }

    fn ty_is_movable_owning_value(&self, ty: &Ty) -> bool {
        if ty.is_copy_value() {
            return false;
        }
        if matches!(&ty.kind, crate::typeck::ty::TyKind::Adt { name, .. } if matches!(name.as_str(), "Mutex" | "RwLock"))
        {
            return true;
        }
        if matches!(&ty.kind, crate::typeck::ty::TyKind::Adt { name, .. } if name == "Rc") {
            return true;
        }
        if self._env.is_legacy_idempotent_handle_type(ty) {
            return false;
        }
        self._env.type_contains_drop_owned_value(ty)
            || self
                ._env
                .owned_string_ty
                .as_ref()
                .is_some_and(|canonical| canonical.kind == ty.kind)
    }

    fn var_ty(&self, name: &str) -> Option<Ty> {
        self._env
            .lookup(name)
            .and_then(|symbol| match &symbol.kind {
                crate::typeck::env::SymbolKind::Var { ty, .. } => Some(ty.clone()),
                _ => None,
            })
    }

    fn move_path_ty(&self, path: &MovePath) -> Option<Ty> {
        let mut ty = self.var_ty(&path.root)?;
        for field in &path.fields {
            ty = self._env.struct_field_type(&ty, field)?;
        }
        Some(ty)
    }

    fn move_path_is_movable_owning_value(&self, path: &MovePath) -> bool {
        self.move_path_ty(path)
            .is_some_and(|ty| self.ty_is_movable_owning_value(&ty))
    }

    fn check_owned_string_invalidation(&mut self, receiver: &Expr, method_name: &str) {
        if !matches!(method_name, "push_str" | "clear" | "drop") {
            return;
        }
        if let Some(path) = Self::expr_move_path(receiver) {
            self.check_move_path_use(&path, receiver.span);
        }
    }

    fn mark_moved(&mut self, path: &MovePath, span: Span) {
        let move_span = (span.lo as usize, span.hi as usize);
        // D1: end borrows whose last reachable use is already past (no remaining use).
        self.end_last_use_borrows_for_path(path);
        if let Some(active_borrow) = self
            .borrows
            .iter()
            .find(|(borrowed, _)| borrowed.is_prefix_of(path) || path.is_prefix_of(borrowed))
            .and_then(|(_, borrows)| borrows.first())
        {
            self.errors.push(BorrowError::CannotMoveBorrowed {
                var: path.display(),
                borrow_span: active_borrow.span,
                move_span,
            });
            return;
        }
        self.moved.insert(path.clone());
        self.move_spans.insert(path.clone(), move_span);
    }

    /// End owner/alias borrows that have no remaining use in the current block.
    fn end_last_use_borrows_for_path(&mut self, owner: &MovePath) {
        let has_owner_borrow = self
            .borrows
            .keys()
            .any(|borrowed| borrowed.is_prefix_of(owner) || owner.is_prefix_of(borrowed));
        if !has_owner_borrow {
            return;
        }
        let alias_names: Vec<String> = self
            .borrow_aliases
            .iter()
            .filter(|(_, source)| source.is_prefix_of(owner) || owner.is_prefix_of(source))
            .map(|(name, _)| name.clone())
            .collect();
        if alias_names
            .iter()
            .any(|name| self.remaining_uses_ident(name))
        {
            return;
        }
        // Drop owner path borrows and every dead alias binding for that owner.
        let owner_keys: Vec<MovePath> = self
            .borrows
            .keys()
            .filter(|borrowed| borrowed.is_prefix_of(owner) || owner.is_prefix_of(borrowed))
            .cloned()
            .collect();
        for key in owner_keys {
            self.borrows.remove(&key);
        }
        for name in alias_names {
            self.borrows.remove(&MovePath::root(name.clone()));
            self.borrow_aliases.remove(&name);
        }
    }

    fn record_borrow_alias(&mut self, alias: &str, owner: &MovePath) {
        self.borrow_aliases.insert(alias.to_string(), owner.clone());
    }

    fn remaining_uses_ident(&self, name: &str) -> bool {
        self.remaining_stmts
            .iter()
            .any(|stmt| Self::stmt_uses_ident(stmt, name))
    }

    fn stmt_uses_ident(stmt: &Stmt, name: &str) -> bool {
        match &stmt.kind {
            StmtKind::Let { value, .. } => value
                .as_ref()
                .is_some_and(|expr| Self::expr_uses_ident(expr, name)),
            StmtKind::Const { value, .. } => Self::expr_uses_ident(value, name),
            StmtKind::Expr(expr) => Self::expr_uses_ident(expr, name),
            StmtKind::Item(_) => false,
        }
    }

    fn expr_uses_ident(expr: &Expr, name: &str) -> bool {
        match &expr.kind {
            ExprKind::Ident(ident) => ident.name == name,
            ExprKind::Path(path) => path.as_simple().is_some_and(|ident| ident.name == name),
            ExprKind::Unary { operand, .. }
            | ExprKind::Cast { expr: operand, .. }
            | ExprKind::Is { expr: operand, .. }
            | ExprKind::Try(operand)
            | ExprKind::Await(operand)
            | ExprKind::Paren(operand)
            | ExprKind::Lambda { body: operand, .. } => Self::expr_uses_ident(operand, name),
            ExprKind::Binary { left, right, .. }
            | ExprKind::Assign {
                target: left,
                value: right,
            }
            | ExprKind::AssignOp {
                target: left,
                value: right,
                ..
            }
            | ExprKind::Index {
                base: left,
                index: right,
            } => Self::expr_uses_ident(left, name) || Self::expr_uses_ident(right, name),
            ExprKind::Call { func, args } => {
                Self::expr_uses_ident(func, name)
                    || args.iter().any(|arg| Self::expr_uses_ident(arg, name))
            }
            ExprKind::MethodCall { receiver, args, .. } => {
                Self::expr_uses_ident(receiver, name)
                    || args.iter().any(|arg| Self::expr_uses_ident(arg, name))
            }
            ExprKind::Field { base, .. } => Self::expr_uses_ident(base, name),
            ExprKind::Array(elems) | ExprKind::Tuple(elems) => {
                elems.iter().any(|elem| Self::expr_uses_ident(elem, name))
            }
            ExprKind::Struct { fields, base, .. } => {
                fields
                    .iter()
                    .any(|field| Self::expr_uses_ident(&field.value, name))
                    || base
                        .as_ref()
                        .is_some_and(|base| Self::expr_uses_ident(base, name))
            }
            ExprKind::If {
                cond,
                then_branch,
                else_branch,
            } => {
                Self::expr_uses_ident(cond, name)
                    || then_branch
                        .stmts
                        .iter()
                        .any(|stmt| Self::stmt_uses_ident(stmt, name))
                    || else_branch
                        .as_ref()
                        .is_some_and(|else_expr| Self::expr_uses_ident(else_expr, name))
            }
            ExprKind::Block(block)
            | ExprKind::TryBlock(block)
            | ExprKind::AsyncBlock(block)
            | ExprKind::ParallelBlock(block) => block
                .stmts
                .iter()
                .any(|stmt| Self::stmt_uses_ident(stmt, name)),
            ExprKind::While { cond, body }
            | ExprKind::For {
                iter: cond, body, ..
            } => {
                Self::expr_uses_ident(cond, name)
                    || body
                        .stmts
                        .iter()
                        .any(|stmt| Self::stmt_uses_ident(stmt, name))
            }
            ExprKind::Loop(body) => body
                .stmts
                .iter()
                .any(|stmt| Self::stmt_uses_ident(stmt, name)),
            ExprKind::Match { scrutinee, arms } => {
                Self::expr_uses_ident(scrutinee, name)
                    || arms.iter().any(|arm| {
                        arm.guard
                            .as_ref()
                            .is_some_and(|guard| Self::expr_uses_ident(guard, name))
                            || Self::expr_uses_ident(&arm.body, name)
                    })
            }
            ExprKind::Range { start, end, .. } => {
                start
                    .as_ref()
                    .is_some_and(|expr| Self::expr_uses_ident(expr, name))
                    || end
                        .as_ref()
                        .is_some_and(|expr| Self::expr_uses_ident(expr, name))
            }
            ExprKind::Return(value) | ExprKind::Break(value) | ExprKind::Yield(value) => value
                .as_ref()
                .is_some_and(|expr| Self::expr_uses_ident(expr, name)),
            ExprKind::Continue | ExprKind::Literal(_) => false,
        }
    }

    fn check_move_path_use(&mut self, path: &MovePath, span: Span) {
        let use_span = (span.lo as usize, span.hi as usize);
        if let Some(moved) = self.moved.iter().find(|moved| moved.is_prefix_of(path)) {
            let move_span = self.move_spans.get(moved).copied().unwrap_or(use_span);
            self.errors.push(BorrowError::UseAfterMove {
                var: moved.display(),
                use_span,
                move_span,
            });
            return;
        }
        if let Some(moved) = self.moved.iter().find(|moved| path.is_prefix_of(moved)) {
            let move_span = self.move_spans.get(moved).copied().unwrap_or(use_span);
            self.errors.push(BorrowError::UseAfterPartialMove {
                var: path.display(),
                use_span,
                move_span,
            });
        }
    }

    fn maybe_move_string_arg(&mut self, arg: &Expr) {
        if let Some(path) = Self::expr_move_path(arg) {
            if self.move_path_is_movable_owning_value(&path) {
                self.mark_moved(&path, arg.span);
            }
        }
    }

    fn maybe_move_assignment_value(&mut self, target: &Expr, value: &Expr) {
        let target_path = Self::expr_move_path(target);
        let value_path = Self::expr_move_path(value);
        if target_path.is_some() && target_path == value_path {
            return;
        }
        self.maybe_move_string_arg(value);
    }

    fn check_assignment_place(&mut self, target: &Expr) {
        let Some(path) = Self::expr_move_path(target) else {
            self.check_expr(target);
            return;
        };
        let use_span = (target.span.lo as usize, target.span.hi as usize);
        if let Some(moved) = self
            .moved
            .iter()
            .find(|moved| moved.fields.len() < path.fields.len() && moved.is_prefix_of(&path))
        {
            let move_span = self.move_spans.get(moved).copied().unwrap_or(use_span);
            self.errors.push(BorrowError::UseAfterMove {
                var: moved.display(),
                use_span,
                move_span,
            });
        }
    }

    fn reinitialize_move_path(&mut self, path: &MovePath) {
        let reinitialized = self
            .moved
            .iter()
            .filter(|moved| path.is_prefix_of(moved))
            .cloned()
            .collect::<Vec<_>>();
        for moved in reinitialized {
            self.moved.remove(&moved);
            self.move_spans.remove(&moved);
        }
    }

    fn add_borrow(&mut self, expr: &Expr, kind: BorrowKind) {
        if let Some(path) = Self::expr_move_path(expr) {
            let span = (expr.span.lo as usize, expr.span.hi as usize);
            if self.borrows.get(&path).is_some_and(|borrows| {
                borrows
                    .iter()
                    .any(|borrow| borrow.kind == kind && borrow.span == span)
            }) {
                return;
            }
            let lifetime = self.lifetime_counter;
            self.lifetime_counter += 1;

            for (borrowed, existing) in &self.borrows {
                if !borrowed.is_prefix_of(&path) && !path.is_prefix_of(borrowed) {
                    continue;
                }
                for borrow in existing {
                    match (&kind, &borrow.kind) {
                        (BorrowKind::Mutable, BorrowKind::Mutable) => {
                            self.errors.push(BorrowError::MultipleMutableBorrows {
                                var: path.display(),
                                first_span: borrow.span,
                                second_span: span,
                            });
                        }
                        (BorrowKind::Mutable, BorrowKind::Immutable)
                        | (BorrowKind::Immutable, BorrowKind::Mutable) => {
                            self.errors.push(BorrowError::MutableWithOtherBorrows {
                                var: path.display(),
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
            self.borrows.entry(path).or_default().push(borrow);
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
                    if let Some(source) = Self::expr_move_path(operand) {
                        if let Some(existing) = self.borrows.get(&source).cloned() {
                            self.borrows
                                .insert(MovePath::root(name.to_string()), existing);
                            self.record_borrow_alias(name, &source);
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
            ExprKind::MethodCall {
                receiver,
                args,
                method,
                ..
            } => {
                let borrows_receiver = matches!(method.name.as_str(), "borrow" | "as_str")
                    || (method.name == "get"
                        && self.receiver_is_vec(receiver)
                        && self.method_call_returns_ref(expr))
                    || (matches!(method.name.as_str(), "iter" | "iter_keys")
                        && self.method_call_returns_borrowing_iter(expr));
                if borrows_receiver {
                    self.add_borrow(receiver, BorrowKind::Immutable);
                    if let Some(source) = Self::expr_move_path(receiver) {
                        if let Some(existing) = self.borrows.get(&source).cloned() {
                            self.borrows
                                .insert(MovePath::root(name.to_string()), existing);
                            self.record_borrow_alias(name, &source);
                        }
                    }
                }
                // Do not treat non-borrowing method results (e.g. `view.len()` -> i64)
                // as borrow aliases of the receiver; that would keep the owner live forever.
                if !borrows_receiver {
                    for arg in args {
                        self.track_borrows_in_expr(name, arg);
                    }
                } else {
                    for arg in args {
                        self.track_borrows_in_expr(name, arg);
                    }
                }
            }
            ExprKind::Assign { target, value } | ExprKind::AssignOp { target, value, .. } => {
                self.track_borrows_in_expr(name, target);
                self.track_borrows_in_expr(name, value);
            }
            ExprKind::Tuple(elems) | ExprKind::Array(elems) => {
                for elem in elems {
                    self.track_borrows_in_expr(name, elem);
                }
            }
            ExprKind::Struct { fields, .. } => {
                for field in fields {
                    self.track_borrows_in_expr(name, &field.value);
                }
            }
            ExprKind::Paren(inner) => self.track_borrows_in_expr(name, inner),
            ExprKind::Ident(_) | ExprKind::Path(_) | ExprKind::Field { .. } => {
                self.track_borrow_alias(name, expr);
            }
            _ => {}
        }
    }

    fn receiver_is_vec(&self, receiver: &Expr) -> bool {
        let Some(path) = Self::expr_move_path(receiver) else {
            return false;
        };
        self.move_path_ty(&path).is_some_and(
            |ty| matches!(&ty.kind, crate::typeck::ty::TyKind::Adt { name, .. } if matches!(name.as_str(), "Vec" | "VecDeque" | "HashMap" | "BTreeMap" | "BTreeSet")),
        )
    }

    fn method_call_returns_ref(&self, call: &Expr) -> bool {
        self._env
            .resolved_method_return_type(call.span)
            .is_some_and(|ty| matches!(ty.kind, crate::typeck::ty::TyKind::Ref { .. }))
    }

    fn method_call_returns_borrowing_iter(&self, call: &Expr) -> bool {
        self._env
            .resolved_method_return_type(call.span)
            .is_some_and(|ty| {
                matches!(&ty.kind, crate::typeck::ty::TyKind::Adt { name, .. } if matches!(name.as_str(), "RawVecIter" | "RawMapKeyIter"))
            })
    }

    fn check_vec_borrow_invalidation(&mut self, receiver: &Expr, method_name: &str) {
        if !self.receiver_is_vec(receiver)
            || !matches!(
                method_name,
                "push"
                    | "set"
                    | "insert"
                    | "pop"
                    | "remove"
                    | "clear"
                    | "free"
                    | "drop"
                    | "push_front"
                    | "push_back"
                    | "pop_front"
                    | "pop_back"
            )
        {
            return;
        }
        let Some(path) = Self::expr_move_path(receiver) else {
            return;
        };
        let Some(active_borrow) = self
            .borrows
            .iter()
            .find(|(borrowed, _)| borrowed.is_prefix_of(&path) || path.is_prefix_of(borrowed))
            .and_then(|(_, borrows)| borrows.first())
        else {
            return;
        };
        self.errors.push(BorrowError::CannotMoveBorrowed {
            var: path.display(),
            borrow_span: active_borrow.span,
            move_span: (receiver.span.lo as usize, receiver.span.hi as usize),
        });
    }

    fn track_borrow_alias(&mut self, name: &str, expr: &Expr) {
        let target = MovePath::root(name.to_string());
        self.track_borrow_alias_for_path(&target, expr);
    }

    fn track_borrow_alias_for_path(&mut self, target: &MovePath, expr: &Expr) {
        match &expr.kind {
            ExprKind::Unary { op, operand } => {
                let kind = match op {
                    UnOp::Ref => Some(BorrowKind::Immutable),
                    UnOp::RefMut => Some(BorrowKind::Mutable),
                    _ => None,
                };
                if let Some(kind) = kind {
                    self.add_borrow(operand, kind);
                    if let Some(source) = Self::expr_move_path(operand) {
                        if let Some(existing) = self.borrows.get(&source).cloned() {
                            self.borrows.insert(target.clone(), existing);
                            self.record_borrow_alias(&target.root, &source);
                            return;
                        }
                    }
                }
            }
            ExprKind::MethodCall {
                receiver, method, ..
            } if matches!(method.name.as_str(), "borrow" | "as_str") => {
                self.add_borrow(receiver, BorrowKind::Immutable);
                if let Some(source) = Self::expr_move_path(receiver) {
                    if let Some(existing) = self.borrows.get(&source).cloned() {
                        self.borrows.insert(target.clone(), existing);
                        self.record_borrow_alias(&target.root, &source);
                        return;
                    }
                }
            }
            ExprKind::Paren(inner) => {
                self.track_borrow_alias_for_path(target, inner);
                return;
            }
            _ => {}
        }
        let Some(source) = Self::expr_move_path(expr) else {
            self.borrows.remove(target);
            return;
        };
        if let Some(existing) = self.borrows.get(&source).cloned() {
            self.borrows.insert(target.clone(), existing);
            // Rebind of an existing borrow alias (`let rebound = view`).
            if let Some(owner) = self.borrow_aliases.get(&source.root).cloned() {
                self.record_borrow_alias(&target.root, &owner);
            } else if !source.fields.is_empty() || self.borrows.contains_key(&source) {
                // Direct alias of a borrowed owner path.
                self.record_borrow_alias(&target.root, &source);
            }
        } else {
            self.borrows.remove(target);
        }
    }

    fn expr_move_path(expr: &Expr) -> Option<MovePath> {
        match &expr.kind {
            ExprKind::Ident(ident) => Some(MovePath::root(ident.name.clone())),
            ExprKind::Path(path) => path
                .as_simple()
                .map(|ident| MovePath::root(ident.name.clone())),
            ExprKind::Field { base, field } => {
                Self::expr_move_path(base).map(|path| path.child(field.name.clone()))
            }
            ExprKind::Paren(inner) => Self::expr_move_path(inner),
            _ => None,
        }
    }

    /// End a borrow lifetime explicitly.
    pub fn end_borrow(&mut self, var: &str, lifetime: usize) {
        if let Some(borrows) = self.borrows.get_mut(&MovePath::root(var.to_string())) {
            borrows.retain(|b| b.lifetime != lifetime);
        }
    }
}
