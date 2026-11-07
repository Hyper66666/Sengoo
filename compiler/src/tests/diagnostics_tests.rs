use crate::error::ParseError;
use crate::{compile_to_ir, Parser};

#[test]
fn value_in_if_without_else_reports_branch_span_and_function() {
    let source = "def action() -> bool { true }\ndef update(flag: bool) { if flag { action(); }; }";
    let parsed = Parser::parse(source).unwrap();
    let error = crate::TypeChecker::new()
        .check_program(&parsed)
        .unwrap_err();
    match error {
        crate::CompileError::TypeckError(crate::typeck::TypeckError::Diagnostic {
            code,
            message,
            span_lo,
            span_hi,
        }) => {
            assert_eq!(code, "if-without-else-value");
            assert!(message.contains("function `update`"), "{message}");
            assert!(message.contains("end the branch with ()"), "{message}");
            assert!(source[span_lo as usize..span_hi as usize].contains("action()"));
        }
        other => panic!("expected a positioned branch diagnostic, got {other:?}"),
    }
}

#[test]
fn explicit_unit_and_conditional_returns_keep_their_existing_semantics() {
    let source = "def action() -> bool { true }\n\
        def update(flag: bool) { if flag { action(); (); }; }\n\
        def early(flag: bool) -> i64 { if flag { return 7; }; 0 }\n\
        def main() -> i64 { update(true); early(false) }";
    compile_to_ir(source).expect("explicit unit and early returns remain supported");
}

#[test]
fn invalid_struct_field_name_reports_actionable_error() {
    let source = r#"
def main() -> i64 {
    let p = Point { 123: 1 }
    0
}
"#;

    let err = compile_to_ir(source).expect_err("invalid struct field should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("invalid struct field"),
        "expected actionable struct-field diagnostic, got:\n{}",
        msg
    );

    match err {
        crate::CompileError::ParseError(ParseError::InvalidStructField { found, .. }) => {
            assert!(
                found.contains("Int"),
                "expected token kind details in error, got: {}",
                found
            );
        }
        other => panic!("expected ParseError::InvalidStructField, got: {other}"),
    }
}

#[test]
fn struct_expr_with_identifier_field_still_parses() {
    let source = r#"
def main() -> i64 {
    let p = Point { x: 1 }
    0
}
"#;

    let parsed = Parser::parse(source);
    assert!(
        parsed.is_ok(),
        "identifier field in struct literal should parse, got: {:?}",
        parsed.err()
    );
}

#[test]
fn string_field_shorthand_reports_actionable_error() {
    let source = r#"
def main() -> i64 {
    let p = Point { "x" }
    0
}
"#;

    let err = compile_to_ir(source).expect_err("string field shorthand should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("struct field shorthand supports identifiers only"),
        "expected shorthand diagnostic, got:\n{}",
        msg
    );
}
