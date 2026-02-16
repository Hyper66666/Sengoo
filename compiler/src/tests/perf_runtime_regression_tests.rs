//! Runtime-oriented MIR optimization regression tests.

use crate::mir::opt::{pipeline_for_level, MirOptLevel};
use crate::mir::{Instruction, LocalKind, MIRType, MirFunction, Terminator};

fn count_redundant_load_store(func: &MirFunction) -> usize {
    let mut count = 0usize;
    for bb in &func.basic_blocks {
        for pair in bb.instructions.windows(2) {
            if let (
                Instruction::Load {
                    destination,
                    source,
                },
                Instruction::Store {
                    destination: dst,
                    value,
                },
            ) = (&pair[0], &pair[1])
            {
                if destination == value && source == dst {
                    count += 1;
                }
            }
        }
    }
    count
}

fn build_hot_loop_like_mir() -> MirFunction {
    let mut func = MirFunction::new("hot_loop_like".to_string(), vec![], MIRType::Unit);
    let user_local = func.add_local(LocalKind::User, MIRType::Int(64));
    let temp_local = func.add_local(LocalKind::Temp, MIRType::Int(64));

    let bb = func
        .block_mut(func.start_block)
        .expect("entry basic block must exist");
    bb.instructions.push(Instruction::Load {
        destination: temp_local,
        source: user_local,
    });
    bb.instructions.push(Instruction::Store {
        destination: user_local,
        value: temp_local,
    });
    bb.set_terminator(Terminator::Return(None));

    func
}

#[test]
fn mir_optimization_removes_redundant_load_store_pairs() {
    let mut func = build_hot_loop_like_mir();
    let before = count_redundant_load_store(&func);
    assert!(
        before > 0,
        "fixture should include redundant load/store pairs"
    );

    let pipeline = pipeline_for_level(MirOptLevel::O2);
    pipeline.run(std::slice::from_mut(&mut func));

    let after = count_redundant_load_store(&func);
    assert!(
        after < before,
        "optimized MIR should contain fewer redundant load/store pairs"
    );
}
