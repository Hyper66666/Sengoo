use super::*;
use crate::mir::InstId;

impl<'a> LoweringContext<'a> {
    pub(super) fn record_drop_binding_if_needed(&mut self, local: Local) {
        let Some(drop_func) = self.drop_func_for_local(local) else {
            return;
        };
        if !self.is_known_function(drop_func) {
            return;
        }
        if self
            .drop_bindings
            .iter()
            .any(|binding| binding.local == local)
        {
            return;
        }
        self.drop_bindings.push(DropBinding { local, drop_func });
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

    fn drop_func_for_local(&self, local: Local) -> Option<&'static str> {
        match self.get_local_type(local) {
            MIRType::Struct { name, .. } if name == "String" => Some("String_drop"),
            _ => match self.type_names.get(&local).map(String::as_str) {
                Some("String") => Some("String_drop"),
                _ => None,
            },
        }
    }

    pub(super) fn insert_drop_glue(&mut self) {
        let bindings = self
            .drop_bindings
            .iter()
            .filter(|binding| !self.moved_drop_locals.contains(&binding.local))
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
                let flag = self.mir_fn.add_local(LocalKind::Temp, MIR_BOOL);
                self.insert_bool_assign_at_block_start(self.mir_fn.start_block, flag, false);
                if let Some((store_id, _)) = self.find_first_store_to(binding.local) {
                    self.insert_bool_assign_after(store_id, flag, true);
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
            self.mir_fn.basic_blocks[guard_block].set_terminator(Terminator::If {
                cond: *flag,
                then_block: drop_block,
                else_block: next,
            });
            next = guard_block;
        }

        self.mir_fn.basic_blocks[exit_block].set_terminator(Terminator::Goto(next));
    }

    fn push_drop_call(&mut self, block: usize, binding: &DropBinding) {
        let destination = self.mir_fn.add_local(LocalKind::Temp, MIR_BOOL);
        self.mir_fn.push_inst_to_block(
            block,
            Instruction::Call {
                destination,
                func: binding.drop_func.to_string(),
                args: vec![binding.local],
            },
        );
    }

    fn insert_bool_assign_at_block_start(&mut self, block: usize, destination: Local, value: bool) {
        let inst = self.mir_fn.alloc_inst(Instruction::Assign {
            destination,
            value: MirConstant::Bool(value),
        });
        self.mir_fn.basic_blocks[block].instructions.insert(0, inst);
    }

    fn insert_bool_assign_after(&mut self, after: InstId, destination: Local, value: bool) {
        let Some((block_id, index)) = self.find_inst_position(after) else {
            return;
        };
        let inst = self.mir_fn.alloc_inst(Instruction::Assign {
            destination,
            value: MirConstant::Bool(value),
        });
        self.mir_fn.basic_blocks[block_id]
            .instructions
            .insert(index + 1, inst);
    }

    fn all_bindings_initialized_in_entry(&self, bindings: &[DropBinding]) -> bool {
        bindings.iter().all(|binding| {
            self.find_first_store_to(binding.local)
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
