use crate::codegen::{Codegen, JITCodegen};
use crate::mir::{Instruction, LocalKind, MIRType, MirConstant, MirFunction, Terminator};
use crate::{compile_to_ir, compile_to_mir};

/// Verify that the compiler accepts same-width integer operations.
/// This is a baseline test to ensure the type system works correctly.
#[test]
fn same_width_operations_compile() {
    let source = r#"
def main() -> i64 {
    let x: i64 = 10;
    let y: i64 = 20;
    x + y
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "same-width i64+i64 should compile, got: {:?}",
        result.err()
    );
}

#[test]
fn mixed_width_signed_operations_are_supported() {
    let source = r#"
extern "C" {
    fn get_i32() -> i32;
}
def main() -> i64 {
    let x = get_i32();
    let y: i64 = 100;
    x + y
}
"#;
    // The key improvement is that this now compiles successfully
    // Previously, the type checker would reject i32 + i64
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "mixed-width i32+i64 should now compile, got: {:?}",
        result.err()
    );
}

#[test]
fn mixed_width_unsigned_operations_are_rejected_for_now() {
    let source = r#"
extern "C" {
    fn get_u32() -> u32;
    fn get_u64() -> u64;
}
def main() -> i64 {
    let left = get_u32();
    let right = get_u64();
    let _mixed = left + right;
    0
}
"#;
    let err = compile_to_ir(source).expect_err("mixed-width unsigned arithmetic should be rejected for now");
    let msg = err.to_string();
    assert!(
        msg.contains("type check error"),
        "unsigned mixed-width rejection should surface as a type-check failure, got: {}",
        msg
    );
}

#[test]
fn mixed_width_signed_operations_insert_casts_in_mir() {
    let source = r#"
extern "C" {
    fn get_i32() -> i32;
}
def main() -> i64 {
    let x = get_i32();
    let y: i64 = 100;
    x + y
}
"#;
    let mir_fns = compile_to_mir(source).expect("should compile to MIR");
    let main_fn = mir_fns
        .iter()
        .find(|f| f.name == "main")
        .expect("should have main function");

    let has_cast = main_fn
        .instructions
        .iter()
        .any(|inst| matches!(inst, Instruction::Cast { .. }));

    assert!(has_cast, "MIR should contain Cast instructions for signed mixed-width arithmetic");
}

fn build_cast_fixture(source_ty: MIRType, target_ty: MIRType, value: MirConstant) -> MirFunction {
    let mut func = MirFunction::new("main".to_string(), vec![], target_ty.clone());
    let source = func.add_local(LocalKind::Temp, source_ty);
    let casted = func.add_local(LocalKind::Temp, target_ty);
    let entry = func.start_block;

    func.push_inst_to_block(
        entry,
        Instruction::Assign {
            destination: source,
            value,
        },
    );
    func.push_inst_to_block(
        entry,
        Instruction::Cast {
            destination: casted,
            value: source,
            to: func.return_type.clone(),
        },
    );
    func.block_mut(entry)
        .expect("entry block should exist")
        .set_terminator(Terminator::Return(Some(casted)));

    func
}

fn build_bitcast_fixture(source_ty: MIRType, target_ty: MIRType, value: MirConstant) -> MirFunction {
    let mut func = MirFunction::new("main".to_string(), vec![], target_ty.clone());
    let source = func.add_local(LocalKind::Temp, source_ty);
    let casted = func.add_local(LocalKind::Temp, target_ty);
    let entry = func.start_block;

    func.push_inst_to_block(
        entry,
        Instruction::Assign {
            destination: source,
            value,
        },
    );
    func.push_inst_to_block(
        entry,
        Instruction::Bitcast {
            destination: casted,
            value: source,
            to: func.return_type.clone(),
        },
    );
    func.block_mut(entry)
        .expect("entry block should exist")
        .set_terminator(Terminator::Return(Some(casted)));

    func
}

#[test]
fn llvm_codegen_lowers_cast_with_real_instructions() {
    let mut codegen = Codegen::new();
    let signed_widen_ir = codegen
        .codegen(&[build_cast_fixture(
            MIRType::Int(32),
            MIRType::Int(64),
            MirConstant::Int(-7),
        )])
        .expect("LLVM codegen should succeed");
    assert!(
        signed_widen_ir.contains("sext i32"),
        "LLVM IR should sign-extend i32 -> i64 casts, got:\n{}",
        signed_widen_ir
    );

    let mut codegen = Codegen::new();
    let bool_widen_ir = codegen
        .codegen(&[build_cast_fixture(
            MIRType::Bool,
            MIRType::Int(64),
            MirConstant::Bool(true),
        )])
        .expect("LLVM codegen should succeed");
    assert!(
        bool_widen_ir.contains("zext i1"),
        "LLVM IR should zero-extend bool -> i64 casts, got:\n{}",
        bool_widen_ir
    );
}

#[test]
fn jit_codegen_handles_cast_instructions_without_falling_back_to_comments() {
    let mut jit = JITCodegen::new();
    let ir = jit
        .generate(&[build_cast_fixture(
            MIRType::Int(32),
            MIRType::Int(64),
            MirConstant::Int(-7),
        )])
        .expect("JIT codegen should succeed");

    assert!(
        !ir.contains("unhandled instruction: Cast"),
        "JIT should lower Cast instructions instead of dropping them, got:\n{}",
        ir
    );
    assert!(
        ir.contains("sext i32"),
        "JIT should sign-extend i32 -> i64 casts, got:\n{}",
        ir
    );
}

#[test]
fn llvm_codegen_lowers_bitcast_with_real_instructions() {
    let mut codegen = Codegen::new();
    let ir = codegen
        .codegen(&[build_bitcast_fixture(
            MIRType::Float(64),
            MIRType::Int(64),
            MirConstant::Float(3.25),
        )])
        .expect("LLVM codegen should succeed");

    assert!(
        ir.contains("bitcast double"),
        "LLVM IR should emit bitcast for f64 -> i64 reinterpretation, got:\n{}",
        ir
    );

    let mut codegen = Codegen::new();
    let f32_ir = codegen
        .codegen(&[build_bitcast_fixture(
            MIRType::Float(32),
            MIRType::Int(32),
            MirConstant::Float(1.5),
        )])
        .expect("LLVM codegen should succeed");

    assert!(
        f32_ir.contains("bitcast float"),
        "LLVM IR should emit bitcast for f32 -> i32 reinterpretation, got:\n{}",
        f32_ir
    );
}

#[test]
fn jit_codegen_lowers_bitcast_with_real_instructions() {
    let mut jit = JITCodegen::new();
    let ir = jit
        .generate(&[build_bitcast_fixture(
            MIRType::Float(64),
            MIRType::Int(64),
            MirConstant::Float(3.25),
        )])
        .expect("JIT codegen should succeed");

    assert!(
        !ir.contains("unhandled instruction: Bitcast"),
        "JIT should lower Bitcast instructions instead of dropping them, got:\n{}",
        ir
    );
    assert!(
        ir.contains("bitcast double"),
        "JIT IR should emit bitcast for f64 -> i64 reinterpretation, got:\n{}",
        ir
    );

    let mut jit = JITCodegen::new();
    let f32_ir = jit
        .generate(&[build_bitcast_fixture(
            MIRType::Float(32),
            MIRType::Int(32),
            MirConstant::Float(1.5),
        )])
        .expect("JIT codegen should succeed");

    assert!(
        f32_ir.contains("bitcast float"),
        "JIT IR should emit bitcast for f32 -> i32 reinterpretation, got:\n{}",
        f32_ir
    );
}
