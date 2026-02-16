//! Unit tests for constant folding MIR optimization pass
//!
//! Tests that the ConstantFolding pass correctly replaces Binary instructions
//! with Assign instructions when both operands are known constants, and
//! correctly preserves Binary instructions when operands are not constant
//! or when folding would be unsafe (e.g., division by zero).
//!
//! _Requirements: 8.6, 8.7_

use crate::compile_to_mir;
use crate::mir::opt::{ConstantFolding, MirPass};
use crate::mir::Instruction;
use crate::mir::MirConstant;

/// Helper: check if any instruction in the main function is an Assign with the given i64 value.
fn has_assign_with_int(mir_fns: &[crate::mir::MirFunction], expected: i64) -> bool {
    let main_fn = mir_fns.iter().find(|f| f.name == "main").unwrap();
    main_fn.basic_blocks.iter().any(|bb| {
        bb.instructions.iter().any(|inst| {
            matches!(
                inst,
                Instruction::Assign { value: MirConstant::Int(v), .. } if *v == expected
            )
        })
    })
}

/// Helper: check if any instruction in the main function is a Binary instruction.
fn has_binary_instruction(mir_fns: &[crate::mir::MirFunction]) -> bool {
    let main_fn = mir_fns.iter().find(|f| f.name == "main").unwrap();
    main_fn.basic_blocks.iter().any(|bb| {
        bb.instructions
            .iter()
            .any(|inst| matches!(inst, Instruction::Binary { .. }))
    })
}

/// Test `3 + 4` → folded to `Assign(7)`
///
/// When the ConstantFolding pass encounters a Binary Add instruction where both
/// operands are known integer constants (3 and 4), it should replace the Binary
/// instruction with an Assign instruction containing the computed result (7).
///
/// _Requirements: 8.6, 8.7_
#[test]
fn test_constant_folding_add() {
    let source = "def main() -> i64 { 3 + 4 }";
    let mut mir_fns = compile_to_mir(source).expect("should compile to MIR");

    // Before optimization: should have a Binary instruction
    assert!(
        has_binary_instruction(&mir_fns),
        "MIR should contain a Binary instruction before constant folding"
    );

    // Run constant folding
    let pass = ConstantFolding;
    let main_fn = mir_fns.iter_mut().find(|f| f.name == "main").unwrap();
    let changed = pass.run(main_fn);

    assert!(
        changed,
        "ConstantFolding should report that it modified the MIR"
    );

    // After optimization: Binary should be replaced with Assign(7)
    assert!(
        !has_binary_instruction(&mir_fns),
        "MIR should NOT contain a Binary instruction after constant folding"
    );
    assert!(
        has_assign_with_int(&mir_fns, 7),
        "MIR should contain an Assign with value 7 after folding 3 + 4"
    );
}

/// Test `10 - 3` → folded to `Assign(7)`
///
/// When the ConstantFolding pass encounters a Binary Sub instruction where both
/// operands are known integer constants (10 and 3), it should replace the Binary
/// instruction with an Assign instruction containing the computed result (7).
///
/// _Requirements: 8.6, 8.7_
#[test]
fn test_constant_folding_sub() {
    let source = "def main() -> i64 { 10 - 3 }";
    let mut mir_fns = compile_to_mir(source).expect("should compile to MIR");

    // Before optimization: should have a Binary instruction
    assert!(
        has_binary_instruction(&mir_fns),
        "MIR should contain a Binary instruction before constant folding"
    );

    // Run constant folding
    let pass = ConstantFolding;
    let main_fn = mir_fns.iter_mut().find(|f| f.name == "main").unwrap();
    let changed = pass.run(main_fn);

    assert!(
        changed,
        "ConstantFolding should report that it modified the MIR"
    );

    // After optimization: Binary should be replaced with Assign(7)
    assert!(
        !has_binary_instruction(&mir_fns),
        "MIR should NOT contain a Binary instruction after constant folding"
    );
    assert!(
        has_assign_with_int(&mir_fns, 7),
        "MIR should contain an Assign with value 7 after folding 10 - 3"
    );
}

/// Test `6 * 7` → folded to `Assign(42)`
///
/// When the ConstantFolding pass encounters a Binary Mul instruction where both
/// operands are known integer constants (6 and 7), it should replace the Binary
/// instruction with an Assign instruction containing the computed result (42).
///
/// _Requirements: 8.6, 8.7_
#[test]
fn test_constant_folding_mul() {
    let source = "def main() -> i64 { 6 * 7 }";
    let mut mir_fns = compile_to_mir(source).expect("should compile to MIR");

    // Before optimization: should have a Binary instruction
    assert!(
        has_binary_instruction(&mir_fns),
        "MIR should contain a Binary instruction before constant folding"
    );

    // Run constant folding
    let pass = ConstantFolding;
    let main_fn = mir_fns.iter_mut().find(|f| f.name == "main").unwrap();
    let changed = pass.run(main_fn);

    assert!(
        changed,
        "ConstantFolding should report that it modified the MIR"
    );

    // After optimization: Binary should be replaced with Assign(42)
    assert!(
        !has_binary_instruction(&mir_fns),
        "MIR should NOT contain a Binary instruction after constant folding"
    );
    assert!(
        has_assign_with_int(&mir_fns, 42),
        "MIR should contain an Assign with value 42 after folding 6 * 7"
    );
}

/// Test `10 / 0` → NOT folded (div-by-zero guard)
///
/// When the ConstantFolding pass encounters a Binary Div instruction where the
/// right operand is zero, it should NOT fold the instruction to avoid division
/// by zero errors. The Binary instruction should remain unchanged.
///
/// _Requirements: 8.6, 8.7_
#[test]
fn test_constant_folding_div_by_zero_not_folded() {
    let source = "def main() -> i64 { 10 / 0 }";
    let mut mir_fns = compile_to_mir(source).expect("should compile to MIR");

    // Before optimization: should have a Binary instruction
    assert!(
        has_binary_instruction(&mir_fns),
        "MIR should contain a Binary instruction before constant folding"
    );

    // Run constant folding
    let pass = ConstantFolding;
    let main_fn = mir_fns.iter_mut().find(|f| f.name == "main").unwrap();
    let changed = pass.run(main_fn);

    // The pass should NOT fold division by zero
    assert!(
        !changed,
        "ConstantFolding should NOT modify the MIR for division by zero"
    );

    // The Binary instruction should still be present
    assert!(
        has_binary_instruction(&mir_fns),
        "MIR should still contain a Binary instruction after attempting to fold 10 / 0"
    );
}

/// Test non-constant operands → NOT folded
///
/// When the ConstantFolding pass encounters a Binary instruction where at least
/// one operand is not a known constant (e.g., a function parameter), it should
/// NOT fold the instruction. The Binary instruction should remain unchanged.
///
/// _Requirements: 8.6, 8.7_
#[test]
fn test_constant_folding_non_constant_operands_not_folded() {
    // Use a function parameter so the operand is not a known constant
    let source = "def add_one(x: i64) -> i64 { x + 1 }\ndef main() -> i64 { add_one(5) }";
    let mut mir_fns = compile_to_mir(source).expect("should compile to MIR");

    // Find the add_one function which has a non-constant operand (parameter x)
    let add_one_fn = mir_fns
        .iter()
        .find(|f| f.name == "add_one")
        .expect("should have add_one function");

    // Before optimization: should have a Binary instruction in add_one
    let has_binary_before = add_one_fn.basic_blocks.iter().any(|bb| {
        bb.instructions
            .iter()
            .any(|inst| matches!(inst, Instruction::Binary { .. }))
    });
    assert!(
        has_binary_before,
        "add_one MIR should contain a Binary instruction before constant folding"
    );

    // Run constant folding on add_one
    let pass = ConstantFolding;
    let add_one_fn_mut = mir_fns.iter_mut().find(|f| f.name == "add_one").unwrap();
    let changed = pass.run(add_one_fn_mut);

    // The pass should NOT fold because x is a parameter (not a known constant)
    assert!(
        !changed,
        "ConstantFolding should NOT modify the MIR when an operand is a function parameter"
    );

    // The Binary instruction should still be present
    let add_one_fn = mir_fns.iter().find(|f| f.name == "add_one").unwrap();
    let has_binary_after = add_one_fn.basic_blocks.iter().any(|bb| {
        bb.instructions
            .iter()
            .any(|inst| matches!(inst, Instruction::Binary { .. }))
    });
    assert!(
        has_binary_after,
        "add_one MIR should still contain a Binary instruction after attempting to fold non-constant operands"
    );
}
