//! Unit tests for recursive function type checking
//!
//! Tests that the Sengoo compiler correctly handles recursive and mutually
//! recursive function definitions, including pre-registering function signatures
//! before checking function bodies.
//!
//! _Requirements: 2.1, 2.3, 2.4_

use crate::compile_to_ir;

/// Test that a simple recursive fibonacci function compiles successfully.
/// The TypeChecker should pre-register the function signature so that
/// the recursive call `fib(n - 1)` resolves correctly.
///
/// _Requirements: 2.1, 2.3_
#[test]
fn test_recursive_fibonacci_compiles_successfully() {
    let source = r#"
def fib(n: i64) -> i64 {
    if n < 2 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}
def main() -> i64 {
    fib(10)
}
"#;
    let ir = compile_to_ir(source).expect("recursive fibonacci should compile successfully");
    // The IR should contain the fib function definition
    assert!(
        ir.contains("@fib("),
        "Expected IR to contain '@fib(' function definition, got:\n{}",
        ir
    );
    // The IR should contain recursive calls to fib
    assert!(
        ir.contains("call i64 @fib("),
        "Expected IR to contain 'call i64 @fib(' for recursive calls, got:\n{}",
        ir
    );
}

/// Test that mutually recursive functions compile successfully.
/// The TypeChecker should pre-register all function signatures in a first pass
/// so that f can call g and g can call f.
///
/// _Requirements: 2.4_
#[test]
fn test_mutually_recursive_functions_compile_successfully() {
    let source = r#"
def is_even(n: i64) -> bool {
    if n == 0 {
        true
    } else {
        is_odd(n - 1)
    }
}
def is_odd(n: i64) -> bool {
    if n == 0 {
        false
    } else {
        is_even(n - 1)
    }
}
def main() -> i64 {
    if is_even(4) { 1 } else { 0 }
}
"#;
    let ir =
        compile_to_ir(source).expect("mutually recursive functions should compile successfully");
    assert!(
        ir.contains("@is_even(") && ir.contains("@is_odd("),
        "Expected IR to contain both '@is_even(' and '@is_odd(' definitions, got:\n{}",
        ir
    );
    // is_even should call is_odd and vice versa
    assert!(
        ir.contains("call i1 @is_odd(") || ir.contains("call i64 @is_odd("),
        "Expected IR to contain a call to is_odd from is_even, got:\n{}",
        ir
    );
    assert!(
        ir.contains("call i1 @is_even(") || ir.contains("call i64 @is_even("),
        "Expected IR to contain a call to is_even from is_odd, got:\n{}",
        ir
    );
}

/// Test that a recursive call with wrong argument type produces a type error.
/// The function expects i64 but the recursive call passes a bool.
///
/// _Requirements: 2.3_
#[test]
fn test_recursive_call_wrong_argument_type_produces_error() {
    let source = r#"
def bad_recurse(n: i64) -> i64 {
    if n < 1 {
        0
    } else {
        bad_recurse(true)
    }
}
def main() -> i64 {
    bad_recurse(5)
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "recursive call with wrong argument type should produce a compile error, but got Ok:\n{}",
        result.unwrap_or_default()
    );
}
