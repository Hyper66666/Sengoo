//! Stable diagnostic codes for `?` and match.

use crate::error::CompileError;
use crate::typeck::{typeck, TypeckError};
use crate::Parser;

fn typeck_error(source: &str) -> TypeckError {
    let mut parser = Parser::new(source);
    let program = parser.parse_program().expect("parse");
    match typeck(&program) {
        Err(CompileError::TypeckError(err)) => err,
        other => panic!("expected typeck failure, got {other:?}"),
    }
}

#[test]
fn non_exhaustive_match_reports_stable_code() {
    let err = typeck_error(
        r#"
enum Color { Red, Blue }
def paint(c: Color) -> i64 {
    match c {
        Color::Red => 1,
    }
}
def main() -> i64 { 0 }
"#,
    );
    assert_eq!(err.stable_code(), Some("non-exhaustive-match"));
    assert!(err.span().is_some());
}

#[test]
fn invalid_question_mark_reports_stable_code() {
    let err = typeck_error(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok() -> Result<i64, i64> {
    Result { is_ok: true, value: 1, error: 0 }
}
def main() -> i64 {
    ok()?
}
"#,
    );
    assert_eq!(err.stable_code(), Some("invalid-question-mark"));
    assert!(err.span().is_some());
}
