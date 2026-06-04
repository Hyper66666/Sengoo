//! Type checking for match expressions.

use crate::typeck::typeck;
use crate::Parser;

fn typeck_ok(source: &str) {
    let mut parser = Parser::new(source);
    let program = parser.parse_program().expect("parse");
    typeck(&program).expect("typeck");
}

fn typeck_err(source: &str) {
    let mut parser = Parser::new(source);
    let program = parser.parse_program().expect("parse");
    assert!(typeck(&program).is_err());
}

#[test]
fn enum_match_requires_exhaustive_arms() {
    typeck_err(
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
}

#[test]
fn enum_tuple_variant_binding_uses_payload_field_type() {
    typeck_ok(
        r#"
enum Opt { Vacant, Val(i64) }
def main(m: Opt) -> i64 {
    match m {
        Opt::Val(x) => x + 1,
        Opt::Vacant => 0,
    }
}
"#,
    );
}

#[test]
fn enum_match_with_wildcard_is_ok() {
    typeck_ok(
        r#"
enum Color { Red, Blue }
def paint(c: Color) -> i64 {
    match c {
        Color::Red => 1,
        _ => 0,
    }
}
def main() -> i64 { 0 }
"#,
    );
}

#[test]
fn unreachable_arm_after_wildcard_is_rejected() {
    typeck_err(
        r#"
def main() -> i64 {
    let x = 1;
    match x {
        _ => 1,
        0 => 2,
    }
}
"#,
    );
}

#[test]
fn match_guard_must_be_bool() {
    typeck_err(
        r#"
def main() -> i64 {
    let x = 1;
    match x {
        y if y => 1,
        _ => 0,
    }
}
"#,
    );
}

#[test]
fn or_pattern_binding_mismatch_is_rejected() {
    typeck_err(
        r#"
def main() -> i64 {
    let x = 1;
    match x {
        a | b => a,
        _ => 0,
    }
}
"#,
    );
}

#[test]
fn match_arm_types_must_unify() {
    typeck_err(
        r#"
def main() -> i64 {
    let x = 1;
    match x {
        0 => 1,
        _ => true,
    }
}
"#,
    );
}
