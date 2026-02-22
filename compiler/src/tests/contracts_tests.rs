use crate::ast::DeclKind;
use crate::{compile_to_ir, Parser};

#[test]
fn parser_supports_requires_and_ensures_clauses() {
    let source = r#"
def divide(a: i64, b: i64) -> i64
requires b != 0
ensures result * b == a
{
    a / b
}
"#;

    let program = Parser::parse(source).expect("contracts should parse");
    let first = program
        .decls
        .first()
        .expect("function declaration expected");
    let DeclKind::Function(function) = &first.kind else {
        panic!("expected function declaration");
    };

    assert!(function.precondition.is_some(), "requires clause missing");
    assert!(function.postcondition.is_some(), "ensures clause missing");
}

#[test]
fn parser_accepts_optional_semicolon_after_contract_clause() {
    let source = r#"
def divide(a: i64, b: i64) -> i64
requires b != 0;
ensures result * b == a;
{
    a / b
}
"#;

    compile_to_ir(source).expect("contracts with optional semicolons should compile");
}

#[test]
fn requires_clause_must_be_bool() {
    let source = r#"
def main() -> i64
requires 123
{
    0
}
"#;

    let err = compile_to_ir(source).expect_err("non-bool requires must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("bool"),
        "expected bool-related diagnostic, got: {}",
        msg
    );
}

#[test]
fn ensures_clause_can_reference_result_placeholder() {
    let source = r#"
def identity(x: i64) -> i64
ensures result == x
{
    x
}

def main() -> i64 {
    identity(3)
}
"#;

    compile_to_ir(source).expect("result placeholder in ensures should typecheck");
}

#[test]
fn detects_obvious_constant_postcondition_conflict() {
    let source = r#"
def answer() -> i64
ensures result == 7
{
    42
}
"#;

    let err = compile_to_ir(source).expect_err("contradicting constant postcondition must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("postcondition contradicts constant return value"),
        "unexpected error message: {}",
        msg
    );
}
