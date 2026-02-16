//! Snapshot tests for the Sengoo compiler
//!
//! Uses `insta` to snapshot the LLVM IR output for each example `.sg` file.
//! When the compiler output changes, run `cargo insta review` to inspect and
//! accept/reject the new snapshots.
//!
//! _Requirements: 6.4, 6.5_

use crate::compile_to_ir;

/// Helper: compile source and return a snapshot-friendly string.
/// On success returns the LLVM IR; on error returns the error description.
fn compile_snapshot(source: &str) -> String {
    match compile_to_ir(source) {
        Ok(ir) => ir,
        Err(e) => format!("COMPILE ERROR: {}", e),
    }
}

#[test]
fn snapshot_01_hello() {
    let source = include_str!("../../../examples/01_hello.sg");
    let output = compile_snapshot(source);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_02_arithmetic() {
    let source = include_str!("../../../examples/02_arithmetic.sg");
    let output = compile_snapshot(source);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_03_variables() {
    let source = include_str!("../../../examples/03_variables.sg");
    let output = compile_snapshot(source);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_04_array() {
    let source = include_str!("../../../examples/04_array.sg");
    let output = compile_snapshot(source);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_05_loop() {
    let source = include_str!("../../../examples/05_loop.sg");
    let output = compile_snapshot(source);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_06_lambda() {
    let source = include_str!("../../../examples/06_lambda.sg");
    let output = compile_snapshot(source);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_07_if() {
    let source = include_str!("../../../examples/07_if.sg");
    let output = compile_snapshot(source);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_08_struct() {
    let source = include_str!("../../../examples/08_struct.sg");
    let output = compile_snapshot(source);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_09_method_call() {
    let source = include_str!("../../../examples/09_method_call.sg");
    let output = compile_snapshot(source);
    insta::assert_snapshot!(output);
}

#[test]
fn snapshot_09_simple() {
    let source = include_str!("../../../examples/09_simple.sg");
    let output = compile_snapshot(source);
    insta::assert_snapshot!(output);
}
