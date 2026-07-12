//! Tests for shared compiler options.

use crate::{compile_to_ir, compile_to_ir_with_options, CompileOptions, MirOptLevel};

#[test]
fn default_compile_options_use_o2() {
    let options = CompileOptions::default();
    assert_eq!(options.mir_opt_level, MirOptLevel::O2);
}

#[test]
fn compile_with_o0_succeeds() {
    let source = "def main() -> i64 { let x = 1 + 2; x }";
    let options = CompileOptions {
        mir_opt_level: MirOptLevel::O0,
        runtime_contract_checks: false,
    };

    let ir = compile_to_ir_with_options(source, options)
        .expect("compile_to_ir_with_options(O0) should succeed");

    assert!(ir.contains("define i64 @main()"));
}

#[test]
fn compile_to_ir_matches_default_options_wrapper() {
    let source = "def main() -> i64 { 42 }";
    let from_wrapper = compile_to_ir(source).expect("compile_to_ir should succeed");
    let from_explicit = compile_to_ir_with_options(source, CompileOptions::default())
        .expect("compile_to_ir_with_options(default) should succeed");

    assert_eq!(from_wrapper, from_explicit);
}

#[test]
fn runtime_contract_checks_option_changes_ir_shape() {
    let source = r#"
def bump(x: i64) -> i64
requires x > 0
ensures result > x
{
    x + 1
}

def main() -> i64 {
    bump(1)
}
"#;

    let without_checks = compile_to_ir_with_options(
        source,
        CompileOptions {
            mir_opt_level: MirOptLevel::O0,
            runtime_contract_checks: false,
        },
    )
    .expect("compile without runtime contract checks should succeed");

    let with_checks = compile_to_ir_with_options(
        source,
        CompileOptions {
            mir_opt_level: MirOptLevel::O0,
            runtime_contract_checks: true,
        },
    )
    .expect("compile with runtime contract checks should succeed");

    assert_ne!(
        without_checks, with_checks,
        "runtime contract checks should change generated IR"
    );
    assert!(
        with_checks.contains("unreachable"),
        "runtime-contract-checked IR should contain trap/unreachable guard"
    );
}

#[test]
fn o0_integer_add_uses_checked_overflow_intrinsic() {
    let source = r#"
extern "C" {
    fn get_i64() -> i64;
}

def main() -> i64 {
    get_i64() + 1
}
"#;

    let ir = compile_to_ir_with_options(
        source,
        CompileOptions {
            mir_opt_level: MirOptLevel::O0,
            runtime_contract_checks: false,
        },
    )
    .expect("O0 integer add should compile");

    assert!(
        ir.contains("call { i64, i1 } @llvm.sadd.with.overflow.i64"),
        "O0 integer add should materialize an overflow check, got:\n{ir}"
    );
    assert!(
        ir.contains("extractvalue { i64, i1 }"),
        "O0 integer add should extract both checked result and overflow flag, got:\n{ir}"
    );
    assert!(
        ir.contains("call void @sengoo_panic_integer_overflow"),
        "O0 integer add should route the overflow flag to the runtime trap helper, got:\n{ir}"
    );
}

#[test]
fn o0_unsigned_integer_add_uses_unsigned_checked_overflow_intrinsic() {
    let source = r#"
extern "C" {
    fn get_u32() -> u32;
}

def main() -> u32 {
    get_u32() + 1u32
}
"#;

    let ir = compile_to_ir_with_options(
        source,
        CompileOptions {
            mir_opt_level: MirOptLevel::O0,
            runtime_contract_checks: false,
        },
    )
    .expect("O0 unsigned integer add should compile");

    assert!(
        ir.contains("call { i32, i1 } @llvm.uadd.with.overflow.i32"),
        "O0 unsigned integer add should materialize an unsigned overflow check, got:\n{ir}"
    );
    assert!(
        ir.contains("call void @sengoo_panic_integer_overflow"),
        "O0 unsigned integer add should route the overflow flag to the runtime trap helper, got:\n{ir}"
    );
}

#[test]
fn o2_integer_add_keeps_plain_wrapping_ir() {
    let source = r#"
extern "C" {
    fn get_i64() -> i64;
}

def main() -> i64 {
    get_i64() + 1
}
"#;

    let ir = compile_to_ir_with_options(
        source,
        CompileOptions {
            mir_opt_level: MirOptLevel::O2,
            runtime_contract_checks: false,
        },
    )
    .expect("O2 integer add should compile");

    assert!(
        !ir.contains("call { i64, i1 } @llvm.sadd.with.overflow.i64"),
        "O2 integer add should keep release wrapping codegen, got:\n{ir}"
    );
    assert!(
        !ir.contains("llvm.sadd.with.overflow")
            && !ir.contains("sengoo_panic_integer_overflow")
            && !ir.contains("sengoo_panic_division_by_zero"),
        "O2 integer add should not declare debug-only overflow helpers, got:\n{ir}"
    );
    assert!(
        ir.contains(" = add i64 "),
        "O2 integer add should still emit a plain integer add, got:\n{ir}"
    );
}

#[test]
fn o0_integer_division_checks_zero_divisor() {
    let source = r#"
extern "C" {
    fn get_i64() -> i64;
}

def main() -> i64 {
    84 / get_i64()
}
"#;

    let debug_ir = compile_to_ir_with_options(
        source,
        CompileOptions {
            mir_opt_level: MirOptLevel::O0,
            runtime_contract_checks: false,
        },
    )
    .expect("O0 integer division should compile");
    assert!(
        debug_ir.contains("call void @sengoo_panic_division_by_zero"),
        "O0 integer division should check the divisor before division, got:\n{debug_ir}"
    );

    let release_ir = compile_to_ir_with_options(
        source,
        CompileOptions {
            mir_opt_level: MirOptLevel::O2,
            runtime_contract_checks: false,
        },
    )
    .expect("O2 integer division should compile");
    assert!(
        !release_ir.contains("call void @sengoo_panic_division_by_zero"),
        "O2 integer division should keep release plain division, got:\n{release_ir}"
    );
}

#[test]
fn ordinary_compilation_emits_no_coverage_probe_calls() {
    let ir = compile_to_ir_with_options(
        "def main() -> i64 { let value = 41; value + 1 }",
        CompileOptions::default(),
    )
    .expect("ordinary source should compile");

    assert!(
        !ir.contains("call void @sengoo_coverage_register")
            && !ir.contains("call void @sengoo_coverage_hit"),
        "coverage instrumentation must remain absent outside sgc test --coverage:\n{ir}"
    );
}
