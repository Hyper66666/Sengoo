use std::collections::HashSet;

use crate::{Parser, TypeChecker};

#[test]
fn filtered_body_typecheck_skips_unselected_function_bodies() {
    let source = r#"
def ok() -> i64 {
    1
}

def broken() -> i64 {
    let v: bool = 1
    0
}

def main() -> i64 {
    ok()
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");

    let mut full = TypeChecker::new();
    assert!(
        full.check_program(&program).is_err(),
        "full typecheck should fail because `broken` has a body type error"
    );

    let mut filtered = TypeChecker::new();
    let checked = HashSet::from([String::from("ok"), String::from("main")]);
    assert!(
        filtered
            .check_program_with_filtered_function_bodies(&program, &checked)
            .is_ok(),
        "filtered typecheck should skip non-selected function bodies"
    );
}

#[test]
fn filtered_body_typecheck_reports_selected_function_body_errors() {
    let source = r#"
def broken() -> i64 {
    let v: bool = 1
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    let checked = HashSet::from([String::from("broken")]);

    assert!(
        checker
            .check_program_with_filtered_function_bodies(&program, &checked)
            .is_err(),
        "selected function bodies must still be typechecked"
    );
}
