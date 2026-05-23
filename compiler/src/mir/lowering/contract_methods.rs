use super::*;

impl<'a> LoweringContext<'a> {
    pub(super) fn inject_precondition_check(
        &mut self,
        precondition: &HIRExpr,
        entry_block: usize,
    ) -> usize {
        self.set_current_block(entry_block);
        let cond_local = self.lower_contract_condition(precondition, None);
        let pass_block = self.new_block();
        let fail_block = self.new_block();
        self.set_terminator(Terminator::If {
            cond: cond_local,
            then_block: pass_block,
            else_block: fail_block,
        });
        self.set_current_block(fail_block);
        self.set_terminator(Terminator::Unreachable);
        pass_block
    }

    pub(super) fn inject_postcondition_checks(&mut self, postcondition: &HIRExpr) {
        let return_sites = self
            .mir_fn
            .basic_blocks
            .iter()
            .enumerate()
            .filter_map(|(block_id, block)| match &block.terminator {
                Some(Terminator::Return(value)) => Some((block_id, *value)),
                _ => None,
            })
            .collect::<Vec<_>>();

        for (return_block, return_value) in return_sites {
            let Some(return_local) = return_value else {
                continue;
            };

            let check_block = self.new_block();
            let success_block = self.new_block();
            let fail_block = self.new_block();

            if let Some(block) = self.mir_fn.block_mut(return_block) {
                block.set_terminator(Terminator::Goto(check_block));
            }

            self.set_current_block(check_block);
            let cond_local = self.lower_contract_condition(postcondition, Some(return_local));
            self.set_terminator(Terminator::If {
                cond: cond_local,
                then_block: success_block,
                else_block: fail_block,
            });

            self.set_current_block(success_block);
            self.set_terminator(Terminator::Return(Some(return_local)));

            self.set_current_block(fail_block);
            self.set_terminator(Terminator::Unreachable);
        }
    }

    fn lower_contract_condition(
        &mut self,
        condition: &HIRExpr,
        result_local: Option<Local>,
    ) -> Local {
        let mut saved_name_bindings = Vec::<(String, Option<Local>)>::new();
        let mut saved_symbol_bindings = Vec::<(SymbolId, Option<Local>)>::new();

        for (name, symbol, local) in &self.contract_param_bindings {
            let previous_name = self.local_names.insert(name.clone(), *local);
            saved_name_bindings.push((name.clone(), previous_name));
            if symbol.is_valid() {
                let previous_symbol = self.local_symbols.insert(*symbol, *local);
                saved_symbol_bindings.push((*symbol, previous_symbol));
            }
        }

        if let Some(result_local) = result_local {
            let result_name = "result".to_string();
            let previous_result_name = self.local_names.insert(result_name.clone(), result_local);
            saved_name_bindings.push((result_name, previous_result_name));

            let mut result_symbols = Vec::new();
            collect_named_symbols(condition, "result", &mut result_symbols);
            for symbol in result_symbols {
                if symbol.is_valid() {
                    let previous_symbol = self.local_symbols.insert(symbol, result_local);
                    saved_symbol_bindings.push((symbol, previous_symbol));
                }
            }
        }

        let cond_local = self.lower_expr(condition);

        for (symbol, previous) in saved_symbol_bindings.into_iter().rev() {
            if let Some(local) = previous {
                self.local_symbols.insert(symbol, local);
            } else {
                self.local_symbols.remove(&symbol);
            }
        }
        for (name, previous) in saved_name_bindings.into_iter().rev() {
            if let Some(local) = previous {
                self.local_names.insert(name, local);
            } else {
                self.local_names.remove(&name);
            }
        }

        cond_local
    }
}
