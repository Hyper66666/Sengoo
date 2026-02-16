//! Unit tests for string operations
//!
//! Tests that the Sengoo compiler generates correct LLVM IR for string
//! operations (`.len()`, `+` concatenation, `==`/`!=` comparison), and
//! that runtime functions handle null pointers gracefully.
//!
//! _Requirements: 5.1, 5.2, 5.3, 5.6_

use crate::compile_to_ir;
use crate::runtime::{sengoo_str_concat, sengoo_str_eq};

// ============================================================================
// Compilation tests — verify generated LLVM IR contains correct runtime calls
// ============================================================================

/// Test that `"hello".len()` generates a call to `sengoo_str_len`.
///
/// _Requirements: 5.1_
#[test]
fn test_string_len_generates_str_len_call() {
    let source = r#"def main() -> i64 { "hello".len() }"#;
    let ir = compile_to_ir(source).expect("string .len() should compile successfully");
    assert!(
        ir.contains("sengoo_str_len"),
        "Expected IR to contain 'sengoo_str_len', got:\n{}",
        ir
    );
}

/// Test that `"hello" + " world"` generates a call to `sengoo_str_concat`.
///
/// _Requirements: 5.2_
#[test]
fn test_string_concat_generates_str_concat_call() {
    let source = r#"def main() -> i64 { let s = "hello" + " world"; 0 }"#;
    let ir = compile_to_ir(source).expect("string + should compile successfully");
    assert!(
        ir.contains("sengoo_str_concat"),
        "Expected IR to contain 'sengoo_str_concat', got:\n{}",
        ir
    );
}

/// Test that `"hello" == "world"` generates a call to `sengoo_str_eq`.
///
/// _Requirements: 5.3_
#[test]
fn test_string_eq_generates_str_eq_call() {
    let source = r#"def main() -> i64 { if "hello" == "world" { 1 } else { 0 } }"#;
    let ir = compile_to_ir(source).expect("string == should compile successfully");
    assert!(
        ir.contains("sengoo_str_eq"),
        "Expected IR to contain 'sengoo_str_eq', got:\n{}",
        ir
    );
}

/// Test that `"hello" != "world"` also generates a call to `sengoo_str_eq`.
///
/// The `!=` operator should use the same `sengoo_str_eq` runtime function,
/// with the result negated.
///
/// _Requirements: 5.3_
#[test]
fn test_string_ne_generates_str_eq_call() {
    let source = r#"def main() -> i64 { if "hello" != "world" { 1 } else { 0 } }"#;
    let ir = compile_to_ir(source).expect("string != should compile successfully");
    assert!(
        ir.contains("sengoo_str_eq"),
        "Expected IR to contain 'sengoo_str_eq' for != operator, got:\n{}",
        ir
    );
}

// ============================================================================
// Runtime function tests — call functions directly with null pointers
// ============================================================================

/// Test that `sengoo_str_concat` handles both null pointers gracefully.
///
/// _Requirements: 5.6_
#[test]
fn test_runtime_str_concat_both_null() {
    let result = sengoo_str_concat(std::ptr::null(), std::ptr::null());
    // Should not crash; result should be a valid pointer to an empty string
    assert!(
        !result.is_null(),
        "sengoo_str_concat with both nulls should not return null"
    );
    unsafe {
        assert_eq!(
            *result, 0,
            "Result of concatenating two nulls should be an empty string"
        );
    }
}

/// Test that `sengoo_str_concat` handles first argument null gracefully.
///
/// _Requirements: 5.6_
#[test]
fn test_runtime_str_concat_first_null() {
    let s2 = b"hello\0";
    let result = sengoo_str_concat(std::ptr::null(), s2.as_ptr());
    assert!(
        !result.is_null(),
        "sengoo_str_concat with first null should not return null"
    );
    unsafe {
        let result_str = std::ffi::CStr::from_ptr(result as *const i8);
        assert_eq!(result_str.to_str().unwrap(), "hello");
    }
}

/// Test that `sengoo_str_concat` handles second argument null gracefully.
///
/// _Requirements: 5.6_
#[test]
fn test_runtime_str_concat_second_null() {
    let s1 = b"world\0";
    let result = sengoo_str_concat(s1.as_ptr(), std::ptr::null());
    assert!(
        !result.is_null(),
        "sengoo_str_concat with second null should not return null"
    );
    unsafe {
        let result_str = std::ffi::CStr::from_ptr(result as *const i8);
        assert_eq!(result_str.to_str().unwrap(), "world");
    }
}

/// Test that `sengoo_str_eq` handles both null pointers gracefully.
///
/// Two null pointers should be considered equal.
///
/// _Requirements: 5.6_
#[test]
fn test_runtime_str_eq_both_null() {
    let result = sengoo_str_eq(std::ptr::null(), std::ptr::null());
    assert_eq!(result, 1, "Two null pointers should be considered equal");
}

/// Test that `sengoo_str_eq` handles one null pointer gracefully.
///
/// A null pointer compared with a non-null string should not be equal.
///
/// _Requirements: 5.6_
#[test]
fn test_runtime_str_eq_one_null() {
    let s = b"hello\0";
    let result = sengoo_str_eq(std::ptr::null(), s.as_ptr());
    assert_eq!(
        result, 0,
        "Null compared with non-null string should not be equal"
    );

    let result2 = sengoo_str_eq(s.as_ptr(), std::ptr::null());
    assert_eq!(
        result2, 0,
        "Non-null string compared with null should not be equal"
    );
}
