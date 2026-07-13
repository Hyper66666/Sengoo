use super::*;

impl<'a> LoweringContext<'a> {
    /// 将HIR函数体降级为基本块（不计算返回值）。
    pub(super) fn lower_body_to_block(&mut self, body: &HIRBody, target_block: usize) {
        self.lower_body_to_block_with_return(body, target_block, true);
    }

    /// 将HIR函数体降级为基本块，计算块值（返回最后一个表达式）。
    pub(super) fn lower_body_to_block_val(&mut self, body: &HIRBody, target_block: usize) -> Local {
        self.set_current_block(target_block);

        for stmt in &body.stmts {
            if self.current_block_is_terminated() {
                break;
            }
            self.lower_stmt(stmt);
        }

        if let Some(expr) = &body.expr {
            if self.current_block_is_terminated() {
                return self.add_local(None, LocalKind::Temp, MIR_UNIT);
            }
            self.lower_expr(expr)
        } else {
            self.add_local(None, LocalKind::Temp, MIR_UNIT)
        }
    }

    /// 将HIR函数体降级为基本块，并在末尾插入return指令。
    pub(super) fn lower_body_to_block_with_return(
        &mut self,
        body: &HIRBody,
        target_block: usize,
        add_return: bool,
    ) {
        self.set_current_block(target_block);

        // 降级函数体的所有语句到当前基本块。
        for stmt in &body.stmts {
            if self.current_block_is_terminated() {
                break;
            }
            self.lower_stmt(stmt);
        }

        // 若块尾存在表达式，则先降级该表达式并视情况插入 return。
        if let Some(expr) = &body.expr {
            if self.current_block_is_terminated() {
                return;
            }
            let result_local = self.lower_expr(expr);
            if add_return {
                // Only add return if the current block doesn't already have a
                // terminator (e.g. set by break/continue/return inside the expr).
                let cur = self.current_block();
                let already_terminated = self
                    .mir_fn
                    .block_mut(cur)
                    .is_some_and(|b| b.terminator.is_some());
                if !already_terminated {
                    // 为函数体末尾生成隐式return指令。
                    // 检查是否为main函数的隐式返回情况。
                    let is_main_with_unit_body = self.mir_fn.name == "main"
                        && matches!(self.mir_fn.return_type, MIRType::Int(_))
                        && matches!(*self.get_local_type(result_local), MIRType::Unit);

                    if is_main_with_unit_body {
                        self.set_terminator(Terminator::Return(None));
                    } else {
                        self.mark_drop_expr_moved(expr);
                        self.mark_drop_local_moved(result_local);
                        self.set_terminator(Terminator::Return(Some(result_local)));
                    }
                }
            }
        // 若需要添加return终止符则插入return指令。
        } else if add_return {
            // 当需要添加return且最后一个块未终止时，插入return指令。
            // Only set return if the current block doesn't already have a
            // terminator (e.g. set by break/continue/return in a statement).
            let cur = self.current_block();
            let already_terminated = self
                .mir_fn
                .block_mut(cur)
                .is_some_and(|b| b.terminator.is_some());
            if !already_terminated {
                self.set_terminator(Terminator::Return(None));
            }
        }
    }

    /// 将单条HIR语句降级为MIR指令序列。
    fn lower_stmt(&mut self, stmt: &HIRStmt) {
        match stmt {
            HIRStmt::Source { site_lo } => self.current_source_site = Some(*site_lo),
            HIRStmt::Coverage { site_lo } => self.emit_coverage_hit(*site_lo),
            HIRStmt::Let {
                name,
                symbol,
                ty,
                value,
                is_mut,
            } => lower_let_stmt(self, name, *symbol, ty, value.as_ref(), *is_mut),
            HIRStmt::Expr(expr) => {
                self.lower_expr(expr);
            }
            HIRStmt::Item => {}
        }
    }

    pub(super) fn lower_expr(&mut self, expr: &HIRExpr) -> Local {
        match expr {
            HIRExpr::Lit(lit) => self.lower_literal(lit),
            HIRExpr::Var { name, symbol } => self.resolve_local(name, *symbol),
            HIRExpr::Unary(op, operand) => lower_unary_expr(self, op, operand),
            HIRExpr::Binary(op, left, right) => lower_binary_expr(self, op, left, right),
            HIRExpr::Block(body) => lower_block_expr(self, body),
            HIRExpr::If {
                cond,
                then_branch,
                else_branch,
            } => lower_if_expr(self, cond, then_branch, else_branch.as_deref()),
            HIRExpr::Loop(body) => lower_loop_expr(self, body),
            HIRExpr::While { cond, body } => lower_while_expr(self, cond, body),
            HIRExpr::For {
                var_name,
                iter,
                body,
                ..
            } => lower_for_expr(self, var_name, iter, body),
            HIRExpr::Call {
                func,
                args,
                site_lo,
                expected_return_type,
            } => lower_call_expr(self, func, args, *site_lo, expected_return_type.as_ref()),
            HIRExpr::EnumConstruct {
                enum_name,
                variant_name,
                discriminant,
                args,
            } => lower_enum_construct_expr(self, enum_name, variant_name, *discriminant, args),
            HIRExpr::And(left, right) => lower_logical_and_expr(self, left, right),
            HIRExpr::Or(left, right) => lower_logical_or_expr(self, left, right),
            HIRExpr::Break(value) => lower_break_expr(self, value.as_deref()),
            HIRExpr::Continue => lower_continue_expr(self),
            HIRExpr::Assign { target, value } => lower_assign_expr(self, target, value),
            HIRExpr::AssignOp { target, op, value } => {
                lower_assign_op_expr(self, target, op, value)
            }
            HIRExpr::Array(elems) => lower_array_expr(self, elems),
            HIRExpr::Tuple(elems) => lower_tuple_expr(self, elems),
            HIRExpr::Index { base, index } => lower_index_expr(self, base, index),
            HIRExpr::Struct {
                name,
                fields,
                concrete_type,
            } => lower_struct_expr(self, name, fields, concrete_type.as_ref()),

            HIRExpr::Field { base, field } => {
                let base_local = self.lower_expr(base);
                lower_field_expr(self, base_local, field)
            }
            HIRExpr::Ref(_is_mut, expr) => lower_ref_expr(self, expr),
            HIRExpr::Deref(expr) => lower_deref_expr(self, expr),
            HIRExpr::Lambda { params, body } => lower_lambda_expr(self, params, body),
            HIRExpr::Match { scrutinee, arms } => lower_match_expr(self, scrutinee, arms),
            HIRExpr::MethodCall {
                receiver,
                method,
                args,
                expected_return_type,
            } => {
                lower_method_call_expr(self, receiver, method, args, expected_return_type.as_ref())
            }
            HIRExpr::Await(inner) => lower_await_expr(self, inner),
            HIRExpr::AsyncBlock(body) => lower_async_block_expr(self, body),
            HIRExpr::Try(operand) => lower_try_expr(self, operand),
            HIRExpr::TryBlock(body) => lower_try_block_expr(self, body),
            HIRExpr::Return(value) => lower_return_expr(self, value.as_deref()),
            HIRExpr::Cast(inner, ty) => {
                let value = self.lower_expr(inner);
                let target_ty = self.hir_type_to_mir(ty);
                let destination = self.add_local(None, LocalKind::Temp, target_ty.clone());
                self.push_inst(Instruction::Cast {
                    destination,
                    value,
                    to: target_ty,
                });
                destination
            }
            _ => self.add_local(None, LocalKind::Temp, MIR_UNIT),
        }
    }

    pub(super) fn lower_scoped_body_to_block_val(
        &mut self,
        body: &HIRBody,
        target_block: usize,
    ) -> Local {
        self.push_drop_scope();
        let result = self.lower_body_to_block_val(body, target_block);
        self.pop_drop_scope(Some(result));
        result
    }
}
