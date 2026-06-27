use super::*;
use crate::typeck::BorrowChecker;

impl TypeChecker {
    /// 检查块表达式，按顺序检查所有语句并返回最终类型。
    pub(super) fn check_block(&mut self, block: &Block) -> TyResult<Ty> {
        self.env.push_scope();

        let mut result_ty = self.env.unit_ty();
        for stmt in &block.stmts {
            if let Some(ty) = self.check_stmt(stmt)? {
                result_ty = ty;
            }
        }

        let mut borrow_checker = BorrowChecker::new(self.env.clone());
        borrow_checker.check_block(block);
        if let Err(errs) = borrow_checker.finish() {
            return Err(crate::typeck::format_borrow_errors(&errs));
        }

        self.env.pop_scope();
        Ok(result_ty)
    }

    /// 检查单条语句，返回可选的类型（仅表达式语句有类型）。
    pub(super) fn check_stmt(&mut self, stmt: &Stmt) -> TyResult<Option<Ty>> {
        match &stmt.kind {
            StmtKind::Let {
                name,
                ty,
                value,
                is_mut,
            } => {
                let var_ty = if let Some(ty) = ty {
                    self.check_type(ty)?
                } else {
                    self.infer.fresh_ty_var()
                };

                let value_ty = match value {
                    Some(v) => self.check_expr(v)?,
                    None => self.env.unit_ty(),
                };
                if Self::is_async_context_ty(&value_ty) {
                    return Err(TypeckError::Other(
                        "AsyncContext is poll-scoped and cannot be stored".to_string(),
                    ));
                }
                self.infer.unify(&var_ty, &value_ty)?;
                let resolved_var_ty = self.infer.apply_subst(&var_ty);

                self.env
                    .insert_var_with_mutability(name.name.clone(), resolved_var_ty, *is_mut);
                Ok(None)
            }
            StmtKind::Const { name, ty, value } => {
                let var_ty = self.check_type(ty)?;
                let value_ty = self.check_expr(value)?;
                if Self::is_async_context_ty(&value_ty) {
                    return Err(TypeckError::Other(
                        "AsyncContext is poll-scoped and cannot be stored".to_string(),
                    ));
                }
                self.infer.unify(&var_ty, &value_ty)?;
                self.env.insert_var(name.name.clone(), var_ty);
                Ok(None)
            }
            StmtKind::Expr(expr) => {
                let ty = self.check_expr(expr)?;
                Ok(Some(ty))
            }
            StmtKind::Item(item) => {
                self.check_decl(item)
                    .map_err(|e| TypeckError::Other(e.to_string()))?;
                Ok(None)
            }
        }
    }

    /// 检查if条件表达式，验证条件为bool型且分支类型兼容。
    pub(super) fn check_if(
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

    /// 检查while循环，验证条件为bool型。
    pub(super) fn check_while(&mut self, cond: &Expr, body: &Block) -> TyResult<Ty> {
        let cond_ty = self.check_expr(cond)?;
        let bool_ty = self.env.bool_ty();
        self.infer.unify(&cond_ty, &bool_ty)?;

        self.check_block(body)?;
        Ok(self.env.unit_ty())
    }

    /// 检查for循环，验证迭代器类型和模式匹配。
    pub(super) fn check_for(
        &mut self,
        pattern: &Pattern,
        iter: &Expr,
        body: &Block,
    ) -> TyResult<Ty> {
        let elem_ty = match &iter.kind {
            ExprKind::Range { start, end, .. } => {
                let range_ty = self.env.int_ty(IntKind::I64);
                if let Some(start) = start.as_deref() {
                    let start_ty = self.check_expr(start)?;
                    self.infer.unify(&start_ty, &range_ty)?;
                }
                if let Some(end) = end.as_deref() {
                    let end_ty = self.check_expr(end)?;
                    self.infer.unify(&end_ty, &range_ty)?;
                }
                range_ty
            }
            _ => {
                let iter_ty = self.check_expr(iter)?;
                match &iter_ty.kind {
                    TyKind::Array(elem, _) | TyKind::Slice(elem) => (**elem).clone(),
                    _ => {
                        return Err(TypeckError::Other(
                            "for loop expects an array, slice, or range iterable".to_string(),
                        ));
                    }
                }
            }
        };

        self.env.push_scope();

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

    /// 检查loop循环体类型。
    pub(super) fn check_loop(&mut self, body: &Block) -> TyResult<Ty> {
        self.check_block(body)?;
        Ok(self.env.unit_ty())
    }

    /// 检查match表达式，验证所有分支类型一致。
    pub(super) fn check_match(
        &mut self,
        scrutinee: &Expr,
        arms: &[MatchArm],
        match_span: crate::lexer::Span,
    ) -> TyResult<Ty> {
        self.check_match_expr(scrutinee, arms, match_span)
    }

    /// 检查return语句，验证返回值类型与函数返回类型匹配。
    pub(super) fn check_return(&mut self, value: &Option<Box<Expr>>) -> TyResult<Ty> {
        if let Some(v) = value {
            let ty = self.check_expr(v)?;
            if Self::is_async_context_ty(&ty) {
                return Err(TypeckError::Other(
                    "AsyncContext is poll-scoped and cannot be returned".to_string(),
                ));
            }
            if self.contains_future_escape_ty(&ty) {
                return Err(Self::future_escape_error());
            }
        }
        Ok(self.env.never_ty())
    }

    /// 检查break语句，验证可选值类型与循环类型匹配。
    pub(super) fn check_break(&mut self, value: &Option<Box<Expr>>) -> TyResult<Ty> {
        if let Some(v) = value {
            let ty = self.check_expr(v)?;
            if self.contains_future_escape_ty(&ty) {
                return Err(Self::future_escape_error());
            }
        }
        Ok(self.env.never_ty())
    }

    /// 检查continue语句，返回never类型。
    pub(super) fn check_continue(&mut self) -> TyResult<Ty> {
        Ok(self.env.never_ty())
    }
}
