use super::*;
use crate::mir::InstId;

impl<'a> LoweringContext<'a> {
    pub(super) fn push_drop_scope(&mut self) {
        self.drop_scope_markers.push(self.drop_bindings.len());
    }

    pub(super) fn drop_scope_depth(&self) -> usize {
        self.drop_scope_markers.len()
    }

    pub(super) fn pop_drop_scope(&mut self, moved_result: Option<Local>) {
        if let Some(result) = moved_result {
            self.mark_drop_local_moved(result);
        }
        let Some(marker) = self.drop_scope_markers.pop() else {
            return;
        };
        let bindings = self.drop_bindings.split_off(marker);
        if self.current_block_is_terminated() {
            return;
        }
        for binding in bindings.iter().rev() {
            if self.drop_binding_is_moved(binding) {
                continue;
            }
            self.push_drop_call(self.current_block(), binding);
        }
    }

    pub(super) fn emit_active_drop_scopes_before_exit(&mut self) {
        self.emit_drop_scopes_from_depth(0);
    }

    pub(super) fn emit_drop_scopes_from_depth(&mut self, depth: usize) {
        let Some(marker) = self.drop_scope_markers.get(depth).copied() else {
            return;
        };
        let bindings = self.drop_bindings[marker..]
            .iter()
            .filter(|binding| !self.drop_binding_is_moved(binding))
            .cloned()
            .collect::<Vec<_>>();
        for binding in bindings.iter().rev() {
            self.push_drop_call(self.current_block(), binding);
        }
    }

    pub(super) fn record_drop_binding_if_needed(&mut self, local: Local) {
        if local.kind == LocalKind::Param
            && Self::is_legacy_idempotent_handle_mir_type(self.get_local_type(local))
        {
            return;
        }
        let mut bindings = Vec::new();
        if let Some(drop_func) = self.drop_func_for_local(local) {
            bindings.push(DropBinding {
                local,
                field_path: Vec::new(),
                drop_func,
            });
        } else {
            let ty = self.get_local_type(local).clone();
            self.collect_field_drop_bindings(local, &ty, &mut Vec::new(), &mut bindings);
        };
        for binding in bindings {
            if !self.drop_bindings.iter().any(|existing| {
                existing.local == binding.local && existing.field_path == binding.field_path
            }) {
                self.drop_bindings.push(binding);
            }
        }
    }

    pub(super) fn mark_drop_local_moved(&mut self, local: Local) {
        if self.drop_func_for_local(local).is_some() {
            self.moved_drop_locals.insert(local);
        }
    }

    pub(super) fn mark_drop_locals_moved(&mut self, locals: &[Local]) {
        for local in locals {
            self.mark_drop_local_moved(*local);
        }
    }

    pub(super) fn mark_drop_expr_moved(&mut self, expr: &HIRExpr) {
        match expr {
            HIRExpr::If {
                then_branch,
                else_branch,
                ..
            } => {
                if let Some(expr) = then_branch.expr.as_deref() {
                    self.mark_drop_expr_moved(expr);
                }
                if let Some(expr) = else_branch
                    .as_deref()
                    .and_then(|branch| branch.expr.as_deref())
                {
                    self.mark_drop_expr_moved(expr);
                }
            }
            HIRExpr::Block(body) | HIRExpr::TryBlock(body) | HIRExpr::AsyncBlock(body) => {
                if let Some(expr) = body.expr.as_deref() {
                    self.mark_drop_expr_moved(expr);
                }
            }
            _ => {
                let Some((local, field_path)) = self.resolve_drop_place(expr) else {
                    return;
                };
                if field_path.is_empty() {
                    self.mark_drop_local_moved(local);
                } else {
                    self.mark_drop_field_moved(local, field_path);
                }
            }
        }
    }

    pub(super) fn resolve_drop_place(&mut self, expr: &HIRExpr) -> Option<(Local, Vec<u32>)> {
        match expr {
            HIRExpr::Var { name, symbol } => Some((self.resolve_local(name, *symbol), Vec::new())),
            HIRExpr::Field { base, field } => {
                let (local, mut path) = self.resolve_drop_place(base)?;
                let owner_ty = self.drop_place_type(local, &path)?;
                let index = match owner_ty {
                    MIRType::Struct { fields, .. } => {
                        fields.iter().position(|(name, _)| name == field)?
                    }
                    MIRType::Tuple(fields) => {
                        let index = field.parse::<usize>().ok()?;
                        (index < fields.len()).then_some(index)?
                    }
                    _ => return None,
                };
                path.push(index as u32);
                Some((local, path))
            }
            _ => None,
        }
    }

    fn drop_place_type(&self, local: Local, path: &[u32]) -> Option<MIRType> {
        let mut ty = self.get_local_type(local).clone();
        for index in path {
            ty = match ty {
                MIRType::Struct { fields, .. } => fields.get(*index as usize)?.1.clone(),
                MIRType::Tuple(fields) => fields.get(*index as usize)?.clone(),
                MIRType::Array(elem, len) if (*index as u64) < len => *elem,
                _ => return None,
            };
        }
        Some(ty)
    }

    pub(super) fn mark_drop_local_reinitialized(&mut self, local: Local) {
        self.moved_drop_locals.remove(&local);
        self.moved_drop_fields
            .retain(|(moved_local, _)| *moved_local != local);
        self.record_drop_binding_if_needed(local);
    }

    pub(super) fn drop_local_now_if_initialized(&mut self, local: Local) {
        if self.moved_drop_locals.contains(&local) {
            return;
        }
        let bindings = self
            .drop_bindings
            .iter()
            .filter(|binding| binding.local == local && !self.drop_binding_is_moved(binding))
            .cloned()
            .collect::<Vec<_>>();
        for binding in bindings.iter().rev() {
            self.push_drop_call(self.current_block(), binding);
        }
    }

    pub(super) fn drop_field_now_if_initialized(&mut self, local: Local, field_path: &[u32]) {
        if self.moved_drop_locals.contains(&local) {
            return;
        }
        let bindings = self
            .drop_bindings
            .iter()
            .filter(|binding| {
                binding.local == local
                    && Self::field_path_is_prefix(field_path, &binding.field_path)
                    && !self.drop_binding_is_moved(binding)
            })
            .cloned()
            .collect::<Vec<_>>();
        for binding in bindings.iter().rev() {
            self.push_drop_call(self.current_block(), binding);
        }
    }

    pub(super) fn mark_drop_field_reinitialized(&mut self, local: Local, field_path: &[u32]) {
        self.moved_drop_fields.retain(|(moved_local, moved_path)| {
            *moved_local != local || !Self::field_path_is_prefix(field_path, moved_path)
        });
        self.record_drop_binding_if_needed(local);
    }

    pub(super) fn rebuild_drop_place_with_value(
        &mut self,
        root: Local,
        field_path: &[u32],
        new_value: Local,
    ) -> Option<Local> {
        let root_ty = self.get_local_type(root).clone();
        self.rebuild_drop_place_inner(root, root_ty, field_path, new_value)
    }

    fn rebuild_drop_place_inner(
        &mut self,
        aggregate: Local,
        aggregate_ty: MIRType,
        field_path: &[u32],
        new_value: Local,
    ) -> Option<Local> {
        let (&field, rest) = field_path.split_first()?;
        let field_ty = Self::field_type_at(&aggregate_ty, field)?;
        let replacement = if rest.is_empty() {
            new_value
        } else {
            let extracted = self.add_local(None, LocalKind::Temp, field_ty.clone());
            self.push_inst(Instruction::Extract {
                destination: extracted,
                value: aggregate,
                index: field,
            });
            self.rebuild_drop_place_inner(extracted, field_ty, rest, new_value)?
        };

        let rebuilt = self.add_local(None, LocalKind::Temp, aggregate_ty);
        self.push_inst(Instruction::Insert {
            destination: rebuilt,
            value: aggregate,
            field,
            new_value: replacement,
        });
        Some(rebuilt)
    }

    fn field_type_at(ty: &MIRType, field: u32) -> Option<MIRType> {
        match ty {
            MIRType::Struct { fields, .. } => fields.get(field as usize).map(|(_, ty)| ty.clone()),
            MIRType::Tuple(fields) => fields.get(field as usize).cloned(),
            MIRType::Array(elem, len) if (field as u64) < *len => Some((**elem).clone()),
            _ => None,
        }
    }

    fn is_legacy_idempotent_handle_mir_type(ty: &MIRType) -> bool {
        match ty {
            MIRType::Struct { name, .. } => {
                matches!(
                    name.as_str(),
                    "String"
                        | "Buffer"
                        | "JsonDoc"
                        | "ProcessCommand"
                        | "ProcessOutput"
                        | "ProcessHandle"
                        | "TcpStream"
                        | "UdpSocket"
                        | "HttpClient"
                        | "HttpServer"
                        | "HttpServerRequest"
                        | "WsClient"
                ) || name.starts_with("Vec_")
            }
            _ => false,
        }
    }

    fn drop_func_for_local(&self, local: Local) -> Option<String> {
        let type_name = match self.get_local_type(local) {
            MIRType::Struct { name, .. } => Some(name.as_str()),
            _ => self.type_names.get(&local).map(String::as_str),
        }?;
        let drop_func = format!("{type_name}_Drop_drop");
        self.is_known_function(&drop_func).then_some(drop_func)
    }

    fn drop_func_for_type(&self, ty: &MIRType) -> Option<String> {
        let MIRType::Struct { name, .. } = ty else {
            return None;
        };
        let drop_func = format!("{name}_Drop_drop");
        self.is_known_function(&drop_func).then_some(drop_func)
    }

    fn collect_field_drop_bindings(
        &self,
        local: Local,
        ty: &MIRType,
        path: &mut Vec<u32>,
        bindings: &mut Vec<DropBinding>,
    ) {
        if let Some(drop_func) = self.drop_func_for_type(ty) {
            bindings.push(DropBinding {
                local,
                field_path: path.clone(),
                drop_func,
            });
            return;
        }
        let fields = match ty {
            MIRType::Struct { fields, .. } => fields.iter().map(|(_, ty)| ty).collect::<Vec<_>>(),
            MIRType::Tuple(fields) => fields.iter().collect::<Vec<_>>(),
            MIRType::Array(elem, len) => (0..*len).map(|_| elem.as_ref()).collect::<Vec<_>>(),
            _ => return,
        };
        for (index, field_ty) in fields.into_iter().enumerate() {
            path.push(index as u32);
            self.collect_field_drop_bindings(local, field_ty, path, bindings);
            path.pop();
        }
    }

    fn drop_binding_is_moved(&self, binding: &DropBinding) -> bool {
        self.moved_drop_locals.contains(&binding.local)
            || self.moved_drop_fields.iter().any(|(local, moved_path)| {
                *local == binding.local
                    && (Self::field_path_is_prefix(moved_path, &binding.field_path)
                        || Self::field_path_is_prefix(&binding.field_path, moved_path))
            })
    }

    fn field_path_is_prefix(left: &[u32], right: &[u32]) -> bool {
        left.len() <= right.len() && left.iter().zip(right).all(|(left, right)| left == right)
    }

    pub(super) fn mark_drop_field_moved(&mut self, local: Local, field_path: Vec<u32>) {
        if self
            .drop_bindings
            .iter()
            .any(|binding| binding.local == local && !binding.field_path.is_empty())
        {
            self.moved_drop_fields.insert((local, field_path));
        }
    }

    pub(super) fn insert_drop_glue(&mut self) {
        let bindings = self
            .drop_bindings
            .iter()
            .filter(|binding| !self.drop_binding_is_moved(binding))
            .cloned()
            .collect::<Vec<_>>();
        if bindings.is_empty() {
            return;
        }

        let return_blocks = self
            .mir_fn
            .basic_blocks
            .iter()
            .filter_map(|block| match block.terminator {
                Some(Terminator::Return(value)) => Some((block.id, value)),
                _ => None,
            })
            .collect::<Vec<_>>();

        if return_blocks.is_empty() {
            return;
        }

        if return_blocks.len() == 1 && self.all_bindings_initialized_in_entry(&bindings) {
            self.insert_straight_line_drops(return_blocks[0].0, &bindings);
        } else {
            self.insert_flagged_drops(&return_blocks, &bindings);
        }
    }

    fn insert_straight_line_drops(&mut self, block: usize, bindings: &[DropBinding]) {
        for binding in bindings.iter().rev() {
            self.push_drop_call(block, binding);
        }
    }

    fn insert_flagged_drops(&mut self, exits: &[(usize, Option<Local>)], bindings: &[DropBinding]) {
        let flagged = bindings
            .iter()
            .filter_map(|binding| {
                let flag = self.mir_fn.add_local(LocalKind::User, MIR_BOOL);
                if binding.local.kind == LocalKind::Param {
                    self.insert_bool_store_at_block_start(self.mir_fn.start_block, flag, true);
                    return Some((binding.clone(), flag));
                }
                self.insert_bool_store_at_block_start(self.mir_fn.start_block, flag, false);
                if let Some((store_id, _)) = self.find_first_store_to(binding.local) {
                    self.insert_bool_store_after(store_id, flag, true);
                    Some((binding.clone(), flag))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        if flagged.is_empty() {
            return;
        }

        for (exit_block, return_value) in exits.iter().copied() {
            self.rewrite_return_with_flagged_drops(exit_block, return_value, &flagged);
        }
    }

    fn rewrite_return_with_flagged_drops(
        &mut self,
        exit_block: usize,
        return_value: Option<Local>,
        flagged: &[(DropBinding, Local)],
    ) {
        let final_return = self.mir_fn.add_block();
        self.mir_fn.basic_blocks[final_return].set_terminator(Terminator::Return(return_value));

        let mut next = final_return;
        for (binding, flag) in flagged {
            let drop_block = self.mir_fn.add_block();
            self.push_drop_call(drop_block, binding);
            self.mir_fn.basic_blocks[drop_block].set_terminator(Terminator::Goto(next));

            let guard_block = self.mir_fn.add_block();
            let guard_value = self.mir_fn.add_local(LocalKind::Temp, MIR_BOOL);
            self.mir_fn.push_inst_to_block(
                guard_block,
                Instruction::Load {
                    destination: guard_value,
                    source: *flag,
                },
            );
            self.mir_fn.basic_blocks[guard_block].set_terminator(Terminator::If {
                cond: guard_value,
                then_block: drop_block,
                else_block: next,
            });
            next = guard_block;
        }

        self.mir_fn.basic_blocks[exit_block].set_terminator(Terminator::Goto(next));
    }

    fn push_drop_call(&mut self, block: usize, binding: &DropBinding) {
        let mut argument = binding.local;
        for index in &binding.field_path {
            let field_ty = match self.get_local_type(argument) {
                MIRType::Struct { fields, .. } => fields
                    .get(*index as usize)
                    .map(|(_, ty)| ty.clone())
                    .unwrap_or(MIR_UNIT),
                MIRType::Tuple(fields) => fields.get(*index as usize).cloned().unwrap_or(MIR_UNIT),
                MIRType::Array(elem, _) => (**elem).clone(),
                _ => MIR_UNIT,
            };
            let extracted = self.mir_fn.add_local(LocalKind::Temp, field_ty);
            self.mir_fn.push_inst_to_block(
                block,
                Instruction::Extract {
                    destination: extracted,
                    value: argument,
                    index: *index,
                },
            );
            argument = extracted;
        }
        let destination = self.mir_fn.add_local(LocalKind::Temp, MIR_UNIT);
        self.mir_fn.push_inst_to_block(
            block,
            Instruction::Call {
                destination,
                func: binding.drop_func.clone(),
                args: vec![argument],
            },
        );
    }

    fn alloc_bool_store(&mut self, destination: Local, value: bool) -> [InstId; 2] {
        let value_local = self.mir_fn.add_local(LocalKind::Temp, MIR_BOOL);
        let assign = self.mir_fn.alloc_inst(Instruction::Assign {
            destination: value_local,
            value: MirConstant::Bool(value),
        });
        let store = self.mir_fn.alloc_inst(Instruction::Store {
            destination,
            value: value_local,
        });
        [assign, store]
    }

    fn insert_bool_store_at_block_start(&mut self, block: usize, destination: Local, value: bool) {
        let insts = self.alloc_bool_store(destination, value);
        self.mir_fn.basic_blocks[block]
            .instructions
            .splice(0..0, insts);
    }

    fn insert_bool_store_after(&mut self, after: InstId, destination: Local, value: bool) {
        let Some((block_id, index)) = self.find_inst_position(after) else {
            return;
        };
        let insts = self.alloc_bool_store(destination, value);
        self.mir_fn.basic_blocks[block_id]
            .instructions
            .splice(index + 1..index + 1, insts);
    }

    fn all_bindings_initialized_in_entry(&self, bindings: &[DropBinding]) -> bool {
        bindings.iter().all(|binding| {
            binding.local.kind == LocalKind::Param
                || self
                    .find_first_store_to(binding.local)
                    .is_some_and(|(_, block)| block == self.mir_fn.start_block)
        })
    }

    fn find_first_store_to(&self, destination: Local) -> Option<(InstId, usize)> {
        self.mir_fn.basic_blocks.iter().find_map(|block| {
            block.instructions.iter().copied().find_map(|id| {
                matches!(
                    self.mir_fn.instruction(id),
                    Instruction::Store {
                        destination: store_destination,
                        ..
                    } if *store_destination == destination
                )
                .then_some((id, block.id))
            })
        })
    }

    fn find_inst_position(&self, target: InstId) -> Option<(usize, usize)> {
        self.mir_fn.basic_blocks.iter().find_map(|block| {
            block
                .instructions
                .iter()
                .position(|id| *id == target)
                .map(|index| (block.id, index))
        })
    }
}
