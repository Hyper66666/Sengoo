use crate::codegen::{Codegen, IntegerOverflowMode, JITCodegen};
use crate::mir::{Instruction, LocalKind, MIRType, MirBinOp, MirConstant, MirFunction, Terminator};
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
fn mixed_width_unsigned_operations_insert_unsigned_casts_in_mir() {
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
    let mir_fns = compile_to_mir(source).expect("mixed-width unsigned arithmetic should compile");
    let main_fn = mir_fns
        .iter()
        .find(|f| f.name == "main")
        .expect("should have main function");
    let has_u32_to_u64_cast = main_fn.instructions.iter().any(|inst| {
        matches!(
            inst,
            Instruction::Cast {
                value,
                to: MIRType::UInt(64),
                ..
            } if matches!(main_fn.locals.get(value.index()), Some((_, MIRType::UInt(32))))
        )
    });
    assert!(
        has_u32_to_u64_cast,
        "MIR should widen u32 to u64 for mixed unsigned arithmetic, got:\n{:#?}",
        main_fn.instructions
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

    assert!(
        has_cast,
        "MIR should contain Cast instructions for signed mixed-width arithmetic"
    );
}

#[test]
fn explicit_as_cast_inserts_cast_in_mir() {
    let source = r#"
extern "C" {
    fn get_i32() -> i32;
}
def main() -> i64 {
    let x = get_i32();
    x as i64
}
"#;
    let mir_fns = compile_to_mir(source).expect("explicit as cast should compile to MIR");
    let main_fn = mir_fns
        .iter()
        .find(|f| f.name == "main")
        .expect("should have main function");

    let has_i32_to_i64_cast = main_fn.instructions.iter().any(|inst| {
        matches!(
            inst,
            Instruction::Cast {
                value,
                to: MIRType::Int(64),
                ..
            } if matches!(main_fn.locals.get(value.index()), Some((_, MIRType::Int(32))))
        )
    });

    assert!(
        has_i32_to_i64_cast,
        "explicit `as` should lower to a Cast from i32 to i64, got:\n{:#?}",
        main_fn.instructions
    );
}

#[test]
fn explicit_as_cast_reaches_llvm_codegen() {
    let source = r#"
extern "C" {
    fn get_i32() -> i32;
}
def main() -> i64 {
    let x = get_i32();
    x as i64
}
"#;
    let ir = compile_to_ir(source).expect("explicit as cast should compile to LLVM IR");
    assert!(
        ir.contains("sext i32"),
        "explicit i32 as i64 should reach LLVM as a sign-extension, got:\n{}",
        ir
    );
}

#[test]
fn explicit_as_cast_supported_matrix_reaches_llvm_codegen() {
    let int_to_bool = compile_to_ir(
        r#"
def main() -> bool {
    1 as bool
}
"#,
    )
    .expect("int-to-bool cast should compile");
    assert!(
        int_to_bool.contains("trunc i64"),
        "int-to-bool should use integer truncation, got:\n{}",
        int_to_bool
    );

    let bool_to_int = compile_to_ir(
        r#"
def main() -> i64 {
    true as i64
}
"#,
    )
    .expect("bool-to-int cast should compile");
    assert!(
        bool_to_int.contains("zext i1"),
        "bool-to-int should use zero extension, got:\n{}",
        bool_to_int
    );

    let int_to_float = compile_to_ir(
        r#"
def main() -> f64 {
    7 as f64
}
"#,
    )
    .expect("int-to-float cast should compile");
    assert!(
        int_to_float.contains("sitofp i64"),
        "int-to-float should use signed integer conversion, got:\n{}",
        int_to_float
    );

    let float_to_int = compile_to_ir(
        r#"
def main() -> i64 {
    7.0 as i64
}
"#,
    )
    .expect("float-to-int cast should compile");
    assert!(
        float_to_int.contains("call i64 @llvm.fptosi.sat.i64.f64(double"),
        "float-to-int should use a defined saturating conversion, got:\n{}",
        float_to_int
    );
}

#[test]
fn float_to_integer_casts_use_saturating_intrinsics() {
    let unrelated = compile_to_ir("def main() -> i64 { 7 }").expect("integer program compiles");
    assert!(
        !unrelated.contains("llvm.fptosi.sat") && !unrelated.contains("llvm.fptoui.sat"),
        "modules without float-to-integer casts must not declare conversion intrinsics"
    );

    let signed = compile_to_ir(
        r#"
extern "C" { fn input() -> f64; }
def main() -> i8 { input() as i8 }
"#,
    )
    .expect("f64 to i8 should compile");
    assert!(
        signed.contains("call i8 @llvm.fptosi.sat.i8.f64(double"),
        "signed float casts must avoid poison on NaN or overflow:\n{signed}"
    );
    assert_eq!(
        signed
            .matches("declare i8 @llvm.fptosi.sat.i8.f64(double)")
            .count(),
        1,
        "the used intrinsic must be declared exactly once"
    );
    assert!(
        !signed.contains("declare i16 @llvm.fptosi.sat")
            && !signed.contains("declare i8 @llvm.fptoui.sat"),
        "unused saturating intrinsics must not be declared"
    );

    let unsigned = compile_to_ir(
        r#"
extern "C" { fn input() -> f32; }
def main() -> u16 { input() as u16 }
"#,
    )
    .expect("f32 to u16 should compile");
    assert!(
        unsigned.contains("call i16 @llvm.fptoui.sat.i16.f32(float"),
        "unsigned float casts must clamp negatives and overflow:\n{unsigned}"
    );
}

#[test]
fn explicit_as_cast_precedence_matches_binary_expressions() {
    let right_cast = r#"
extern "C" {
    fn get_i32() -> i32;
}
def main() -> i64 {
    1 + get_i32() as i64
}
"#;
    compile_to_ir(right_cast).expect("right-hand as cast should bind inside binary RHS");

    let left_cast = r#"
extern "C" {
    fn get_i32() -> i32;
}
def main() -> i64 {
    get_i32() as i64 + 1
}
"#;
    compile_to_ir(left_cast).expect("left-hand as cast should compose with following binary op");
}

#[test]
fn suffixed_signed_integer_literals_desugar_to_casts() {
    let source = r#"
def main() -> i64 {
    42i32 as i64
}
"#;
    let ir = compile_to_ir(source).expect("signed integer suffix should compile");
    assert!(
        ir.contains("sext i32"),
        "i32 suffix followed by as i64 should codegen from i32, got:\n{}",
        ir
    );

    let negative = r#"
def main() -> i64 {
    -1i32 as i64
}
"#;
    let negative_ir =
        compile_to_ir(negative).expect("negative signed integer suffix should compile");
    assert!(
        negative_ir.contains("sext i32"),
        "negative i32 suffix should preserve cast before widening, got:\n{}",
        negative_ir
    );
}

#[test]
fn suffixed_float_literals_desugar_to_casts() {
    let widen = r#"
def main() -> f64 {
    1.5f32 as f64
}
"#;
    let widen_ir = compile_to_ir(widen).expect("f32 suffix should compile");
    assert!(
        widen_ir.contains("fpext float"),
        "f32 suffix followed by as f64 should codegen from float, got:\n{}",
        widen_ir
    );

    let narrow = r#"
def main() -> f32 {
    1.5f64 as f32
}
"#;
    let narrow_ir = compile_to_ir(narrow).expect("f64 suffix should compile");
    assert!(
        narrow_ir.contains("fptrunc double"),
        "f64 suffix followed by as f32 should codegen from double, got:\n{}",
        narrow_ir
    );
}

#[test]
fn explicit_as_cast_rejects_non_scalar_targets() {
    let source = r#"
struct Bag {
    value: i64,
}
def main() -> i64 {
    let x: i64 = 7;
    let _bad = x as Bag;
    0
}
"#;
    let err = compile_to_ir(source).expect_err("casting an integer to a struct should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("invalid cast"),
        "invalid cast should produce a stable diagnostic, got: {}",
        msg
    );
}

#[test]
fn explicit_as_cast_rejects_bool_float_boundary_until_codegen_supports_it() {
    let source = r#"
def main() -> f64 {
    true as f64
}
"#;
    let err = compile_to_ir(source)
        .expect_err("bool-to-float cast should not type-check until codegen supports it");
    let msg = err.to_string();
    assert!(
        msg.contains("invalid-cast"),
        "unsupported bool-to-float cast should report invalid-cast, got: {}",
        msg
    );
}

#[test]
fn fixed_width_unsigned_integer_suffix_literals_desugar_to_unsigned_casts() {
    for (source, expected) in [
        ("def main() -> u64 { 255u8 as u64 }", "zext i8"),
        ("def main() -> u64 { 255u16 as u64 }", "zext i16"),
        ("def main() -> u64 { 255u32 as u64 }", "zext i32"),
        ("def main() -> u32 { 255u64 as u32 }", "trunc i64"),
    ] {
        let ir = compile_to_ir(source).expect("fixed-width unsigned suffix should compile");
        assert!(
            ir.contains(expected),
            "unsigned suffix should preserve source width via `{expected}`, got:\n{}",
            ir
        );
    }
}

#[test]
fn explicit_unsigned_as_casts_reach_llvm_codegen() {
    let widen = compile_to_ir("def main() -> u64 { 255u32 as u64 }")
        .expect("unsigned widening should compile");
    assert!(
        widen.contains("zext i32") && !widen.contains("sext i32"),
        "u32 as u64 should zero-extend, got:\n{}",
        widen
    );

    let narrow = compile_to_ir("def main() -> u32 { 255u64 as u32 }")
        .expect("unsigned narrowing should compile");
    assert!(
        narrow.contains("trunc i64"),
        "u64 as u32 should truncate, got:\n{}",
        narrow
    );

    let to_float =
        compile_to_ir("def main() -> f64 { 7u32 as f64 }").expect("u32 as f64 should compile");
    assert!(
        to_float.contains("uitofp i32") && !to_float.contains("sitofp i32"),
        "u32 as f64 should use unsigned int-to-float conversion, got:\n{}",
        to_float
    );

    let from_float =
        compile_to_ir("def main() -> u32 { 7.0 as u32 }").expect("f64 as u32 should compile");
    assert!(
        from_float.contains("@llvm.fptoui.sat.i32.f64(double")
            && !from_float.contains("@llvm.fptosi.sat.i32.f64(double"),
        "f64 as u32 should use unsigned float-to-int conversion, got:\n{}",
        from_float
    );
}

#[test]
fn unsigned_ops_use_unsigned_llvm_opcodes() {
    let comparison =
        compile_to_ir("def main() -> bool { 1u32 < 2u32 }").expect("unsigned comparison");
    assert!(
        comparison.contains("icmp ult i32") && !comparison.contains("icmp slt i32"),
        "unsigned less-than should use icmp ult, got:\n{}",
        comparison
    );

    let division = compile_to_ir("def main() -> u32 { 8u32 / 2u32 }").expect("unsigned division");
    assert!(
        division.contains("udiv i32") && !division.contains("sdiv i32"),
        "unsigned division should use udiv, got:\n{}",
        division
    );

    let remainder = compile_to_ir("def main() -> u32 { 9u32 % 4u32 }").expect("unsigned rem");
    assert!(
        remainder.contains("urem i32") && !remainder.contains("srem i32"),
        "unsigned remainder should use urem, got:\n{}",
        remainder
    );

    let shift = compile_to_ir("def main() -> u32 { 8u32 >> 1u32 }").expect("unsigned shift");
    assert!(
        shift.contains("lshr i32") && !shift.contains("ashr i32"),
        "unsigned right shift should use logical shift, got:\n{}",
        shift
    );
}

#[test]
fn pointer_sized_numeric_suffixes_lower_as_64_bit_on_the_native_target() {
    let usize_ir =
        compile_to_ir("def main() -> usize { 1usize }").expect("usize suffix should compile");
    assert!(
        usize_ir.contains("define i64 @main()"),
        "usize is currently a 64-bit native-target integer, got:\n{}",
        usize_ir
    );

    let isize_ir =
        compile_to_ir("def main() -> isize { -1isize }").expect("isize suffix should compile");
    assert!(
        isize_ir.contains("define i64 @main()"),
        "isize is currently a 64-bit native-target integer, got:\n{}",
        isize_ir
    );
}

#[test]
fn u64_literal_above_i64_max_compiles_with_unsigned_suffix() {
    let ir = compile_to_ir("def main() -> u64 { 18446744073709551615u64 }")
        .expect("u64::MAX literal should compile when suffixed as u64");
    assert!(
        ir.contains("define i64 @main()"),
        "u64 is currently lowered as an i64-sized LLVM integer, got:\n{}",
        ir
    );
    assert!(
        ir.contains("18446744073709551615"),
        "u64::MAX literal should preserve the unsigned payload, got:\n{}",
        ir
    );
}

#[test]
fn integer_literal_above_i64_max_requires_unsigned_suffix() {
    let err = compile_to_ir("def main() -> i64 { 9223372036854775808 }")
        .expect_err("unsuffixed integer above i64::MAX should be rejected");
    let message = err.to_string();
    assert!(
        message.contains("exceeds range of `i64`"),
        "expected oversized integer diagnostic, got: {message}"
    );
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

fn build_binary_fixture(
    op: MirBinOp,
    ty: MIRType,
    left: MirConstant,
    right: MirConstant,
) -> MirFunction {
    let mut func = MirFunction::new("main".to_string(), vec![], ty.clone());
    let lhs = func.add_local(LocalKind::Temp, ty.clone());
    let rhs = func.add_local(LocalKind::Temp, ty.clone());
    let result = func.add_local(LocalKind::Temp, ty.clone());
    let entry = func.start_block;

    func.push_inst_to_block(
        entry,
        Instruction::Assign {
            destination: lhs,
            value: left,
        },
    );
    func.push_inst_to_block(
        entry,
        Instruction::Assign {
            destination: rhs,
            value: right,
        },
    );
    func.push_inst_to_block(
        entry,
        Instruction::Binary {
            destination: result,
            op,
            left: lhs,
            right: rhs,
        },
    );
    func.block_mut(entry)
        .expect("entry block should exist")
        .set_terminator(Terminator::Return(Some(result)));
    func
}

fn build_bitcast_fixture(
    source_ty: MIRType,
    target_ty: MIRType,
    value: MirConstant,
) -> MirFunction {
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
fn jit_debug_integer_add_uses_overflow_trap_helper() {
    let mut jit = JITCodegen::with_integer_overflow_mode(IntegerOverflowMode::DebugChecked);
    let ir = jit
        .generate(&[build_binary_fixture(
            MirBinOp::Add,
            MIRType::Int(64),
            MirConstant::Int(i64::MAX),
            MirConstant::Int(1),
        )])
        .expect("JIT debug integer add should codegen");

    assert!(
        ir.contains("call { i64, i1 } @llvm.sadd.with.overflow.i64"),
        "JIT debug integer add should use overflow intrinsic, got:\n{}",
        ir
    );
    assert!(
        ir.contains("call void @sengoo_panic_integer_overflow"),
        "JIT debug integer add should call overflow trap helper, got:\n{}",
        ir
    );
}

#[test]
fn jit_debug_unsigned_integer_add_uses_unsigned_overflow_intrinsic() {
    let mut jit = JITCodegen::with_integer_overflow_mode(IntegerOverflowMode::DebugChecked);
    let ir = jit
        .generate(&[build_binary_fixture(
            MirBinOp::Add,
            MIRType::UInt(32),
            MirConstant::Uint(u32::MAX as u64),
            MirConstant::Uint(1),
        )])
        .expect("JIT debug unsigned integer add should codegen");

    assert!(
        ir.contains("call { i32, i1 } @llvm.uadd.with.overflow.i32"),
        "JIT debug unsigned add should use unsigned overflow intrinsic, got:\n{}",
        ir
    );
    assert!(
        ir.contains("call void @sengoo_panic_integer_overflow"),
        "JIT debug unsigned add should call overflow trap helper, got:\n{}",
        ir
    );
}

#[test]
fn jit_debug_integer_division_checks_zero_divisor() {
    let mut jit = JITCodegen::with_integer_overflow_mode(IntegerOverflowMode::DebugChecked);
    let ir = jit
        .generate(&[build_binary_fixture(
            MirBinOp::Div,
            MIRType::Int(64),
            MirConstant::Int(84),
            MirConstant::Int(0),
        )])
        .expect("JIT debug integer division should codegen");

    assert!(
        ir.contains("call void @sengoo_panic_division_by_zero"),
        "JIT debug integer division should call zero-divisor trap helper, got:\n{}",
        ir
    );
}

#[test]
fn jit_debug_unsigned_integer_division_zero_check_zero_extends_divisor() {
    let mut jit = JITCodegen::with_integer_overflow_mode(IntegerOverflowMode::DebugChecked);
    let ir = jit
        .generate(&[build_binary_fixture(
            MirBinOp::Div,
            MIRType::UInt(32),
            MirConstant::Uint(84),
            MirConstant::Uint(0),
        )])
        .expect("JIT debug unsigned integer division should codegen");

    assert!(
        ir.contains("zext i32") && !ir.contains("sext i32"),
        "JIT debug unsigned division should zero-extend the divisor before trap helper, got:\n{}",
        ir
    );
    assert!(
        ir.contains("call void @sengoo_panic_division_by_zero"),
        "JIT debug unsigned division should call zero-divisor trap helper, got:\n{}",
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
