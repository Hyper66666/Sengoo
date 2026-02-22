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
