//! Unit tests for array element assignment
//!
//! Tests that the Sengoo compiler correctly generates LLVM IR for array
//! element assignment operations, including `getelementptr` and `store`
//! instructions.
//!
//! _Requirements: 3.1, 3.2, 3.3_

use crate::compile_to_ir;

/// Test that `arr[0] = 42` generates IR containing `getelementptr` and `store`.
/// The MIR lowering should produce IndexAddr + Store, and codegen should emit
/// the corresponding LLVM IR instructions.
///
/// _Requirements: 3.1, 3.2, 3.3_
#[test]
fn test_array_constant_index_assign_generates_getelementptr_and_store() {
    let source = r#"
def main() -> i64 {
    let mut arr = [0, 0, 0];
    arr[0] = 42;
    arr[0]
}
"#;
    let ir = compile_to_ir(source).expect("arr[0] = 42 should compile successfully");
    assert!(
        ir.contains("getelementptr"),
        "Expected IR to contain 'getelementptr' for array index address computation, got:\n{}",
        ir
    );
    assert!(
        ir.contains("store"),
        "Expected IR to contain 'store' for array element assignment, got:\n{}",
        ir
    );
}

/// Test that `arr[i] = arr[j]` with variable indices compiles successfully.
/// This verifies that the compiler handles variable-index array assignment
/// where both the target and source use dynamic indices.
///
/// _Requirements: 3.1, 3.2, 3.3_
#[test]
fn test_array_variable_index_assign_compiles_successfully() {
    let source = r#"
def main() -> i64 {
    let mut arr = [10, 20, 30];
    let i = 0;
    let j = 2;
    arr[i] = arr[j];
    arr[i]
}
"#;
    let ir = compile_to_ir(source).expect("arr[i] = arr[j] should compile successfully");
    assert!(
        ir.contains("getelementptr"),
        "Expected IR to contain 'getelementptr' for variable index assignment, got:\n{}",
        ir
    );
    assert!(
        ir.contains("store"),
        "Expected IR to contain 'store' for variable index assignment, got:\n{}",
        ir
    );
}
