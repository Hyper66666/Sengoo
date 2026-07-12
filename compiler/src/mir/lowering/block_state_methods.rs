use super::*;

impl<'a> LoweringContext<'a> {
    /// 将循环上下文压入循环嵌套栈。
    pub(super) fn push_loop(&mut self, break_block: usize, continue_block: usize) {
        self.loop_stack.push(LoopContext {
            break_block,
            continue_block,
            drop_scope_depth: self.drop_scope_markers.len(),
        });
    }

    /// 弹出当前循环的break/continue目标。
    pub(super) fn pop_loop(&mut self) {
        self.loop_stack.pop();
    }

    /// 获取当前循环的break目标块索引。
    pub(super) fn get_break_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.break_block)
    }

    /// 获取当前循环的continue目标块索引。
    pub(super) fn get_continue_target(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.continue_block)
    }

    pub(super) fn get_loop_drop_scope_depth(&self) -> Option<usize> {
        self.loop_stack.last().map(|ctx| ctx.drop_scope_depth)
    }

    /// 添加一个新的局部变量并返回其Local句柄。
    pub(super) fn add_local(
        &mut self,
        name: Option<String>,
        kind: LocalKind,
        ty: MIRType,
    ) -> Local {
        let local = self.mir_fn.add_local(kind, ty);
        if let Some(name) = name {
            self.mir_fn.set_local_debug_name(local, name.clone());
            self.local_names.insert(name, local);
        }
        local
    }

    pub(super) fn bind_local_symbol(&mut self, symbol: SymbolId, local: Local) {
        if symbol.is_valid() {
            self.local_symbols.insert(symbol, local);
        }
    }

    /// 获取局部变量的MIR类型。
    pub(super) fn hir_type_to_mir(&self, ty: &crate::hir::HIRType) -> MIRType {
        crate::mir::type_mapping_helpers::hir_type_to_mir_with_structs_and_enums(
            ty,
            self.struct_defs,
            &self.options.enum_defs,
            &std::collections::HashMap::new(),
        )
    }

    pub(super) fn get_local_type(&self, local: Local) -> &MIRType {
        if let Some((_, ty)) = self.mir_fn.locals.get(local.index()) {
            ty
        } else {
            &MIR_UNIT
        }
    }

    /// 获取局部变量的类型信息。
    /// 解析名称和符号ID对应的局部变量，或创建新的局部变量。
    pub(super) fn resolve_local(&mut self, name: &str, symbol: SymbolId) -> Local {
        if symbol.is_valid() {
            if let Some(&local) = self.local_symbols.get(&symbol) {
                return local;
            }
        }
        match self.local_names.get(name) {
            Some(&local) => local,
            None => {
                // 变量未定义时报告错误并返回临时变量。
                self.errors.push(format!("undefined variable: '{}'", name));
                // 错误处理：返回一个unit类型的临时local。
                self.mir_fn.add_local(LocalKind::Temp, MIR_UNIT)
            }
        }
    }

    /// 创建一个新的基本块并返回其索引。
    pub(super) fn new_block(&mut self) -> usize {
        self.mir_fn.add_block()
    }

    /// 设置当前基本块为指定块。
    pub(super) fn set_current_block(&mut self, block: usize) {
        self.current_block = Some(block);
    }

    fn current_block_or_error(&mut self, context: &str) -> usize {
        match self.current_block {
            Some(block) => block,
            None => {
                self.errors.push(format!(
                    "internal MIR lowering error: no current block set while {context}"
                ));
                self.mir_fn.start_block
            }
        }
    }

    /// 返回当前正在生成的基本块索引。
    pub(super) fn current_block(&self) -> usize {
        debug_assert!(self.current_block.is_some(), "no current block set");
        self.current_block.unwrap_or(self.mir_fn.start_block)
    }

    pub(super) fn current_block_is_terminated(&self) -> bool {
        self.current_block
            .and_then(|block| self.mir_fn.basic_blocks.get(block))
            .is_some_and(|block| block.terminator.is_some())
    }

    pub(super) fn propagate_future_origin_through_phi(
        &mut self,
        destination: Local,
        incoming: &[(Local, usize)],
    ) {
        if !matches!(self.get_local_type(destination), MIRType::Future(_)) {
            return;
        }

        let mut resolved = Vec::with_capacity(incoming.len());
        for (local, _) in incoming {
            let Some(origin) = self.future_origins.get(local).cloned() else {
                return;
            };
            resolved.push(origin);
        }

        let Some(first) = resolved.first().cloned() else {
            return;
        };
        if resolved.iter().all(|origin| origin == &first) {
            self.future_origins.insert(destination, first);
        }
    }

    /// Check if two types are compatible for binary operations and, if not,
    /// try to insert Cast instructions to reconcile them.  Returns the
    /// (possibly cast) left and right locals whose types now match, or pushes
    /// an error and returns the originals unchanged.
    pub(super) fn reconcile_binary_operand_types(
        &mut self,
        left: Local,
        right: Local,
    ) -> (Local, Local) {
        let left_ty = self.get_local_type(left).clone();
        let right_ty = self.get_local_type(right).clone();

        // 若两侧类型已经相同，无需调和。
        if left_ty == right_ty {
            return (left, right);
        }

        // Determine if a cast between two types is valid and, if so,
        // which direction to cast (returns the common target type).
        match (&left_ty, &right_ty) {
            // 对整数和浮点数类型进行隐式类型提升（宽化）。
            (MIRType::Int(a), MIRType::Int(b)) => {
                let target_bits = std::cmp::max(*a, *b);
                let target_ty = MIRType::Int(target_bits);
                let new_left = if left_ty != target_ty {
                    self.insert_cast(left, target_ty.clone())
                } else {
                    left
                };
                let new_right = if right_ty != target_ty {
                    self.insert_cast(right, target_ty)
                } else {
                    right
                };
                (new_left, new_right)
            }
            (MIRType::UInt(a), MIRType::UInt(b)) => {
                let target_bits = std::cmp::max(*a, *b);
                let target_ty = MIRType::UInt(target_bits);
                let new_left = if left_ty != target_ty {
                    self.insert_cast(left, target_ty.clone())
                } else {
                    left
                };
                let new_right = if right_ty != target_ty {
                    self.insert_cast(right, target_ty)
                } else {
                    right
                };
                (new_left, new_right)
            }

            // 两个浮点数操作数：选择较大位宽的类型。
            (MIRType::Float(a), MIRType::Float(b)) => {
                let target_bits = std::cmp::max(*a, *b);
                let target_ty = MIRType::Float(target_bits);
                let new_left = if left_ty != target_ty {
                    self.insert_cast(left, target_ty.clone())
                } else {
                    left
                };
                let new_right = if right_ty != target_ty {
                    self.insert_cast(right, target_ty)
                } else {
                    right
                };
                (new_left, new_right)
            }

            // 整数与浮点数混合：将整数转为浮点数。
            (MIRType::Int(_) | MIRType::UInt(_), MIRType::Float(b)) => {
                let target_ty = MIRType::Float(*b);
                let new_left = self.insert_cast(left, target_ty);
                (new_left, right)
            }
            (MIRType::Float(a), MIRType::Int(_) | MIRType::UInt(_)) => {
                let target_ty = MIRType::Float(*a);
                let new_right = self.insert_cast(right, target_ty);
                (left, new_right)
            }

            // 布尔与整数混合：将bool转为对应位宽的整数。
            (MIRType::Bool, MIRType::Int(b)) => {
                let target_ty = MIRType::Int(*b);
                let new_left = self.insert_cast(left, target_ty);
                (new_left, right)
            }
            (MIRType::Bool, MIRType::UInt(b)) => {
                let target_ty = MIRType::UInt(*b);
                let new_left = self.insert_cast(left, target_ty);
                (new_left, right)
            }
            (MIRType::Int(a), MIRType::Bool) => {
                let target_ty = MIRType::Int(*a);
                let new_right = self.insert_cast(right, target_ty);
                (left, new_right)
            }
            (MIRType::UInt(a), MIRType::Bool) => {
                let target_ty = MIRType::UInt(*a);
                let new_right = self.insert_cast(right, target_ty);
                (left, new_right)
            }

            // 其他类型组合：无需自动转换，直接返回左侧类型。
            _ => {
                self.errors.push(format!(
                    "type mismatch in binary operation: left operand has type {:?}, right operand has type {:?}",
                    left_ty, right_ty
                ));
                (left, right)
            }
        }
    }

    /// Insert a Cast instruction that converts `source` to `target_ty`,
    /// returning the new local that holds the cast result.
    fn insert_cast(&mut self, source: Local, target_ty: MIRType) -> Local {
        let dest = self.add_local(None, LocalKind::Temp, target_ty.clone());
        self.push_inst(Instruction::Cast {
            destination: dest,
            value: source,
            to: target_ty,
        });
        dest
    }

    /// 向当前基本块追加一条MIR指令。
    pub(super) fn push_inst(&mut self, inst: Instruction) {
        let block_id = self.current_block_or_error("emitting MIR instruction");
        self.mir_fn.push_inst_to_block(block_id, inst);
    }

    /// 向当前基本块追加terminator终止指令。
    pub(super) fn set_terminator(&mut self, term: Terminator) {
        let block_id = self.current_block_or_error("emitting MIR terminator");
        if let Some(block) = self.mir_fn.block_mut(block_id) {
            block.set_terminator(term);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::panic::{catch_unwind, AssertUnwindSafe};

    #[test]
    fn set_terminator_without_current_block_records_error_instead_of_panicking() {
        let mut mir_fn = MirFunction::new("test".to_string(), vec![], MIR_UNIT);
        let mut lambda_counter = 0usize;
        let known_functions = HashSet::new();
        let function_sigs = HashMap::new();
        let struct_defs = HashMap::new();
        let inherent_templates = Vec::new();
        let trait_templates = Vec::new();

        let mut ctx = LoweringContext::new(
            &mut mir_fn,
            &mut lambda_counter,
            &known_functions,
            &function_sigs,
            &struct_defs,
            ConcreteTypeRegistry::default(),
            MirLowerOptions::default(),
            &inherent_templates,
            &trait_templates,
        );

        let result = catch_unwind(AssertUnwindSafe(|| {
            ctx.set_terminator(Terminator::Return(None));
        }));

        assert!(
            result.is_ok(),
            "set_terminator should not panic without a current block"
        );
        assert!(
            ctx.errors
                .iter()
                .any(|err| err.contains("no current block set while emitting MIR terminator")),
            "expected lowering error to be recorded, got {:?}",
            ctx.errors
        );
    }
}
