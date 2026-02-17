//! MIR optimization pass infrastructure.

use std::collections::HashMap;

use super::inst::{Instruction, Local};
use super::op::{MirBinOp, MirConstant};
use super::MirFunction;

/// MIR optimization level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MirOptLevel {
    O0,
    O1,
    O2,
    O3,
}

impl MirOptLevel {
    /// Convert a numeric level (0..=3) into a [`MirOptLevel`].
    pub fn from_u8(level: u8) -> Option<Self> {
        match level {
            0 => Some(Self::O0),
            1 => Some(Self::O1),
            2 => Some(Self::O2),
            3 => Some(Self::O3),
            _ => None,
        }
    }
}

/// MIR optimization pass trait.
pub trait MirPass {
    /// Pass name for diagnostics/logging.
    fn name(&self) -> &str;

    /// Run optimization on one MIR function.
    /// Returns `true` when the function was modified.
    fn run(&self, func: &mut MirFunction) -> bool;
}

/// Constant folding pass.
pub struct ConstantFolding;

impl MirPass for ConstantFolding {
    fn name(&self) -> &str {
        "constant_folding"
    }

    fn run(&self, func: &mut MirFunction) -> bool {
        let mut changed = false;

        for bb_idx in 0..func.basic_blocks.len() {
            // Track known constants per basic block.
            let mut known_constants: HashMap<Local, MirConstant> = HashMap::new();
            let inst_len = func.basic_blocks[bb_idx].instructions.len();
            for inst_pos in 0..inst_len {
                let inst_id = func.basic_blocks[bb_idx].instructions[inst_pos];
                let inst = func.instruction_mut(inst_id);
                match inst {
                    Instruction::Assign { destination, value } => {
                        known_constants.insert(*destination, value.clone());
                    }
                    Instruction::Binary {
                        destination,
                        op,
                        left,
                        right,
                    } => {
                        if let (Some(lc), Some(rc)) =
                            (known_constants.get(left), known_constants.get(right))
                        {
                            if let Some(result) = fold_binary(*op, lc, rc) {
                                known_constants.insert(*destination, result.clone());
                                *inst = Instruction::Assign {
                                    destination: *destination,
                                    value: result,
                                };
                                changed = true;
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        changed
    }
}

/// Remove redundant `load X; store X` writebacks on the same address.
pub struct RedundantLoadStoreElimination;

impl MirPass for RedundantLoadStoreElimination {
    fn name(&self) -> &str {
        "redundant_load_store_elimination"
    }

    fn run(&self, func: &mut MirFunction) -> bool {
        let mut changed = false;

        for bb_idx in 0..func.basic_blocks.len() {
            let original_ids = {
                let block = &mut func.basic_blocks[bb_idx];
                std::mem::take(&mut block.instructions)
            };

            if original_ids.len() < 2 {
                func.basic_blocks[bb_idx].instructions = original_ids;
                continue;
            }

            let mut optimized = Vec::with_capacity(original_ids.len());
            let mut idx = 0usize;
            let mut block_changed = false;

            while idx < original_ids.len() {
                if idx + 1 < original_ids.len() {
                    let load_id = original_ids[idx];
                    let store_id = original_ids[idx + 1];
                    let load = func.instruction(load_id);
                    let store = func.instruction(store_id);
                    if let (
                        Instruction::Load {
                            destination,
                            source,
                        },
                        Instruction::Store {
                            destination: store_dst,
                            value,
                        },
                    ) = (load, store)
                    {
                        if destination == value && source == store_dst {
                            // Keep the load result available for later users, but drop the
                            // writeback because it stores the just-loaded value back unchanged.
                            optimized.push(load_id);
                            idx += 2;
                            block_changed = true;
                            continue;
                        }
                    }
                }

                optimized.push(original_ids[idx]);
                idx += 1;
            }

            if block_changed {
                func.basic_blocks[bb_idx].instructions = optimized;
                changed = true;
            } else {
                func.basic_blocks[bb_idx].instructions = original_ids;
            }
        }

        changed
    }
}

fn fold_binary(op: MirBinOp, left: &MirConstant, right: &MirConstant) -> Option<MirConstant> {
    match (op, left, right) {
        (MirBinOp::Add, MirConstant::Int(a), MirConstant::Int(b)) => {
            Some(MirConstant::Int(a.wrapping_add(*b)))
        }
        (MirBinOp::Sub, MirConstant::Int(a), MirConstant::Int(b)) => {
            Some(MirConstant::Int(a.wrapping_sub(*b)))
        }
        (MirBinOp::Mul, MirConstant::Int(a), MirConstant::Int(b)) => {
            Some(MirConstant::Int(a.wrapping_mul(*b)))
        }
        (MirBinOp::Div, MirConstant::Int(a), MirConstant::Int(b)) if *b != 0 => {
            Some(MirConstant::Int(a.wrapping_div(*b)))
        }
        (MirBinOp::Rem, MirConstant::Int(a), MirConstant::Int(b)) if *b != 0 => {
            Some(MirConstant::Int(a.wrapping_rem(*b)))
        }
        _ => None,
    }
}

/// Optimization pipeline that runs all registered passes in order.
pub struct OptPipeline {
    passes: Vec<Box<dyn MirPass>>,
}

impl OptPipeline {
    /// Create an empty pipeline.
    pub fn new() -> Self {
        Self { passes: Vec::new() }
    }

    /// Add one pass to the pipeline.
    pub fn add_pass(&mut self, pass: Box<dyn MirPass>) {
        self.passes.push(pass);
    }

    /// Number of registered passes.
    pub fn pass_count(&self) -> usize {
        self.passes.len()
    }

    /// Run all passes for all MIR functions.
    pub fn run(&self, funcs: &mut [MirFunction]) {
        for func in funcs.iter_mut() {
            for pass in &self.passes {
                pass.run(func);
            }
        }
    }
}

/// Build a MIR optimization pipeline for the selected level.
pub fn pipeline_for_level(level: MirOptLevel) -> OptPipeline {
    let mut pipeline = OptPipeline::new();

    // Keep O0 disabled. Prioritize hot-path cleanups on higher levels.
    match level {
        MirOptLevel::O0 => {}
        MirOptLevel::O1 => {
            pipeline.add_pass(Box::new(ConstantFolding));
        }
        MirOptLevel::O2 | MirOptLevel::O3 => {
            pipeline.add_pass(Box::new(RedundantLoadStoreElimination));
            pipeline.add_pass(Box::new(ConstantFolding));
            // A second sweep catches patterns exposed by folding and simplification.
            pipeline.add_pass(Box::new(RedundantLoadStoreElimination));
        }
    }

    pipeline
}

#[cfg(test)]
mod tests {
    use super::{pipeline_for_level, MirOptLevel};

    #[test]
    fn from_u8_parses_valid_levels() {
        assert_eq!(MirOptLevel::from_u8(0), Some(MirOptLevel::O0));
        assert_eq!(MirOptLevel::from_u8(1), Some(MirOptLevel::O1));
        assert_eq!(MirOptLevel::from_u8(2), Some(MirOptLevel::O2));
        assert_eq!(MirOptLevel::from_u8(3), Some(MirOptLevel::O3));
    }

    #[test]
    fn from_u8_rejects_invalid_levels() {
        assert_eq!(MirOptLevel::from_u8(4), None);
        assert_eq!(MirOptLevel::from_u8(255), None);
    }

    #[test]
    fn o0_pipeline_has_no_optional_passes() {
        let pipeline = pipeline_for_level(MirOptLevel::O0);
        assert_eq!(pipeline.pass_count(), 0);
    }

    #[test]
    fn o1_plus_pipeline_has_optimizations() {
        let o1 = pipeline_for_level(MirOptLevel::O1);
        let o2 = pipeline_for_level(MirOptLevel::O2);
        let o3 = pipeline_for_level(MirOptLevel::O3);

        assert!(o1.pass_count() > 0);
        assert!(o2.pass_count() > 0);
        assert!(o3.pass_count() > 0);
    }
}
