use crate::error::ParseError;
use crate::{compile_to_ir, Parser};

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