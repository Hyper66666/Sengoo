//! Unit tests for `match` expression/value Phi generation.

use crate::compile_to_ir;

#[test]
fn test_match_expression_generates_phi() {
    let source = r#"
def main() -> i64 {
    let x = 1;
    let y = match x {
        0 => 10,
        _ => 20,
    };
    y
}
"#;

    let ir = compile_to_ir(source).expect("match expression should compile successfully");
    assert!(
        ir.contains("phi"),
        "Expected IR to contain 'phi' for value-producing match, got:\n{}",
        ir
    );
}

#[test]
fn test_match_statement_does_not_generate_phi_void() {
    let source = r#"
def main() -> i64 {
    let x = 1;
    match x {
        0 => print(0),
        _ => print(1),
    };
    0
}
"#;

    let ir = compile_to_ir(source).expect("statement-style match should compile successfully");
    assert!(
        !ir.contains("phi void"),
        "Expected IR to contain no 'phi void' for statement-style match, got:\n{}",
        ir
    );
}

