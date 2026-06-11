//! Unit tests for if/else expression Phi node generation
//!
//! Tests that the Sengoo compiler correctly generates LLVM IR `phi`
//! instructions when if/else is used as an expression, omits them when
//! if/else is used as a statement, and reports type errors when branch
//! types mismatch.
//!
//! _Requirements: 5.1, 5.2, 5.3, 5.4_

use crate::compile_to_ir;

/// Test that `let x = if true { 1 } else { 2 }` generates IR containing a
/// `phi` instruction. When if/else is used as an expression assigned to a
/// variable, the MIR lowering should produce a Phi instruction and codegen
/// should emit the corresponding LLVM IR `phi`.
///
/// _Requirements: 5.1, 5.2_
#[test]
fn test_if_else_expression_generates_phi() {
    let source = r#"
def main() -> i64 {
    let x = if true { 1 } else { 2 };
    x
}
"#;
    let ir = compile_to_ir(source).expect("if/else expression should compile successfully");
    assert!(
        ir.contains("phi"),
        "Expected IR to contain 'phi' instruction for if/else expression, got:\n{}",
        ir
    );
}

/// Test that `if true { print(1) } else { print(2) }` used as a statement
/// does not generate any `phi` instruction.
/// `phi void` is illegal LLVM IR, so statement if/else must not emit Phi.
///
/// _Requirements: 5.3_
#[test]
fn test_if_else_statement_does_not_generate_value_phi() {
    let source = r#"
def main() -> i64 {
    if true { print(1) } else { print(2) }
    0
}
"#;
    let ir = compile_to_ir(source).expect("if/else statement should compile successfully");
    // Statement-style if/else should not carry any value across branches.
    assert!(
        !ir.contains(" phi "),
        "Expected IR to contain no 'phi' for if/else statement, got:\n{}",
        ir
    );
}

/// Test that nested statement-style if/else with assignment branches does not
/// generate illegal `phi void`.
#[test]
fn test_nested_statement_if_else_does_not_generate_phi_void() {
    let source = r#"
def main() -> i64 {
    let mut lo = 0;
    let mut hi = 10;
    let mid = 5;

    if mid < hi {
        lo = mid + 1;
    } else {
        if mid > lo {
            hi = mid - 1;
        } else {
            lo = hi + 1;
        }
    }

    lo
}
"#;
    let ir = compile_to_ir(source).expect("nested statement if/else should compile");
    assert!(
        !ir.contains("phi void"),
        "Expected IR to contain no 'phi void', got:\n{}",
        ir
    );
}

/// Test that if/else branches with mismatched types produce a compile error.
/// The then-branch returns i64 while the else-branch returns bool, which
/// should trigger a type mismatch error from the TypeChecker.
///
/// _Requirements: 5.4_
#[test]
fn test_if_else_branch_type_mismatch_produces_error() {
    let source = r#"
def main() -> i64 {
    let x = if true { 1 } else { true };
    0
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "if/else with mismatched branch types should produce a compile error, but got Ok:\n{}",
        result.unwrap_or_default()
    );
}
