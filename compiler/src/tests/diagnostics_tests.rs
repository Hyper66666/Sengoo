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

fn assert_english(msg: &str) {
    assert!(
        !msg.chars()
            .any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)),
        "diagnostic must be English, got:\n{msg}"
    );
}

#[test]
fn typeck_diagnostics_are_english_with_stable_wording() {
    let undefined = compile_to_ir("def main() -> i64 { missing }")
        .expect_err("undefined variable should fail")
        .to_string();
    assert!(
        undefined.contains("undefined variable: missing"),
        "{undefined}"
    );
    assert_english(&undefined);

    let arity = compile_to_ir(
        r#"
def add(a: i64, b: i64) -> i64 { a + b }
def main() -> i64 { add(1) }
"#,
    )
    .expect_err("argument count mismatch should fail")
    .to_string();
    assert!(
        arity.contains("argument count mismatch: expected 2, found 1"),
        "{arity}"
    );
    assert_english(&arity);

    let method = compile_to_ir(
        r#"
def main() -> i64 {
    let x: i64 = 1;
    x.nonexistent()
}
"#,
    )
    .expect_err("unknown method should fail")
    .to_string();
    assert!(
        method.contains("has no method nonexistent") || method.contains("undefined method"),
        "{method}"
    );
    assert_english(&method);
}
