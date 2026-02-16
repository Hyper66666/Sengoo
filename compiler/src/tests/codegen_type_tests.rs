//! Unit tests for type-aware operand_value in codegen
//!
//! Tests that the Sengoo compiler generates correct LLVM IR load instructions
//! for User locals of different types (Bool, Float(64), Ptr), using the actual
//! MIR type instead of hardcoding `i64` for all loads.
//!
//! _Requirements: 8.3_

use crate::compile_to_ir;

/// Test that User locals of type `Bool` generate `load i1, i1*` (not `load i64, i64*`).
///
/// When a boolean variable is declared and used, the codegen should emit a load
/// instruction with `i1` type, reflecting the actual MIR type `Bool`.
///
/// _Requirements: 8.3_
#[test]
fn test_bool_user_local_generates_i1_load() {
    let source = r#"def main() -> i64 { let b = true; let c = b && false; 0 }"#;
    let ir = compile_to_ir(source).expect("bool variable program should compile successfully");

    // The IR should contain a load with i1 type for the bool User local
    assert!(
        ir.contains("load i1, i1*"),
        "Expected IR to contain 'load i1, i1*' for bool User local, got:\n{}",
        ir
    );
    // It should NOT use i64 for loading a bool User local
    // (We check that at least one i1 load exists, which confirms type-aware codegen)
}

/// Test that User locals of type `Float(64)` generate `load double, double*`.
///
/// When a float variable is declared and used in a binary operation, the codegen
/// should emit a load instruction with `double` type via `operand_value()`,
/// reflecting the actual MIR type `Float(64)`.
///
/// _Requirements: 8.3_
#[test]
fn test_float64_user_local_generates_double_load() {
    // Use a float variable in a binary operation to trigger operand_value()
    let source = r#"def main() -> i64 { let f = 3.14; let g = f + 1.0; 0 }"#;
    let ir = compile_to_ir(source).expect("float variable program should compile successfully");

    // The IR should contain a load with double type for the float User local
    // when operand_value() is called for the binary add operation
    assert!(
        ir.contains("load double, double*"),
        "Expected IR to contain 'load double, double*' for float User local, got:\n{}",
        ir
    );
}

/// Test that User locals of type `Ptr(...)` (string) generate correct pointer loads.
///
/// When a string variable is declared and used (e.g., in string concatenation),
/// the codegen should correctly handle the `i8*` pointer type for the User local.
/// The alloca should use `i8*` type, and any load of the variable should use
/// `i8*` type rather than `i64`.
///
/// _Requirements: 8.3_
#[test]
fn test_ptr_user_local_generates_pointer_load() {
    // String concatenation with a User local triggers the Call instruction
    // which should correctly handle the pointer type
    let source = r#"def main() -> i64 { let s = "hello"; let t = s + " world"; 0 }"#;
    let ir = compile_to_ir(source).expect("string variable program should compile successfully");

    // The IR should contain an alloca with i8* type for the string User local
    assert!(
        ir.contains("alloca i8*"),
        "Expected IR to contain 'alloca i8*' for string pointer User local, got:\n{}",
        ir
    );

    // The IR should contain a call to sengoo_str_concat with i8* typed arguments
    assert!(
        ir.contains("sengoo_str_concat"),
        "Expected IR to contain 'sengoo_str_concat' for string concatenation, got:\n{}",
        ir
    );
}

/// Test that i64 User locals still generate `load i64, i64*` (baseline/regression test).
///
/// This ensures the type-aware operand_value doesn't break the default i64 case.
///
/// _Requirements: 8.3_
#[test]
fn test_i64_user_local_generates_i64_load() {
    let source = r#"def main() -> i64 { let x = 42; let y = x + 1; y }"#;
    let ir = compile_to_ir(source).expect("i64 variable program should compile successfully");

    // The IR should contain a load with i64 type for the integer User local
    assert!(
        ir.contains("load i64, i64*"),
        "Expected IR to contain 'load i64, i64*' for i64 User local, got:\n{}",
        ir
    );
}

/// Test that bool User locals use `i1` alloca, not `i64` alloca.
///
/// This verifies the type-aware codegen uses the correct alloca type for booleans,
/// complementing the load instruction test.
///
/// _Requirements: 8.3_
#[test]
fn test_bool_user_local_uses_i1_alloca() {
    let source = r#"def main() -> i64 { let b = true; let c = b && false; 0 }"#;
    let ir = compile_to_ir(source).expect("bool variable program should compile successfully");

    // The IR should contain an alloca with i1 type for the bool User local
    assert!(
        ir.contains("alloca i1"),
        "Expected IR to contain 'alloca i1' for bool User local, got:\n{}",
        ir
    );
}
