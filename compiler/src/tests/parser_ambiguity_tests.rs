//! Unit tests for Parser ambiguity resolution between struct literals
//! and comparison operators in conditional contexts.
//!
//! These tests verify that the Parser correctly distinguishes between
//! `{` as a block body start (in if/while conditions) vs struct literal
//! construction (in non-ambiguous contexts like variable assignment).
//!
//! _Requirements: 1.1, 1.2, 1.3_

use crate::compile_to_ir;

/// Test that `if a > c { ... }` parses the comparison correctly.
/// The Parser should treat `a > c` as a binary Gt comparison, not
/// attempt to parse `c { ... }` as a struct literal.
///
/// _Requirements: 1.1, 1.2_
#[test]
fn test_if_greater_than_comparison_parses_as_binary_gt() {
    let source = r#"
def main() -> i64 {
    let a = 10;
    let c = 5;
    if a > c { 1 } else { 0 }
}
"#;
    let ir = compile_to_ir(source).expect("if a > c { ... } should compile successfully");
    assert!(
        ir.contains("icmp sgt") || ir.contains("icmp ugt"),
        "Expected IR to contain a greater-than comparison instruction, got:\n{}",
        ir
    );
}

/// Test that `let p = Point { x: 1, y: 2 }` still parses as a struct
/// construction in a non-ambiguous context (variable assignment).
///
/// _Requirements: 1.3_
#[test]
fn test_struct_literal_in_let_binding_parses_correctly() {
    let source = r#"
struct Point { x: i64, y: i64 }
def main() -> i64 {
    let p = Point { x: 1, y: 2 };
    p.x
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "let p = Point {{ x: 1, y: 2 }} should compile successfully, but got error: {:?}",
        result.err()
    );
}

/// Test that `while x < 10 { ... }` parses the comparison correctly.
/// The Parser should treat `x < 10` as a binary Lt comparison, not
/// attempt to parse `10 { ... }` as something else.
///
/// _Requirements: 1.1, 1.2_
#[test]
fn test_while_less_than_comparison_parses_as_binary_lt() {
    let source = r#"
def main() -> i64 {
    let x = 0;
    while x < 10 {
        x = x + 1;
    }
    x
}
"#;
    let ir = compile_to_ir(source).expect("while x < 10 { ... } should compile successfully");
    assert!(
        ir.contains("icmp slt") || ir.contains("icmp ult"),
        "Expected IR to contain a less-than comparison instruction, got:\n{}",
        ir
    );
}
