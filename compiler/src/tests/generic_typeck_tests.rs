use crate::{Parser, TypeChecker};
use std::collections::HashSet;

#[test]
fn generic_function_can_be_instantiated_with_different_argument_types() {
    let source = r#"
def id<T>(x: T) -> T {
    x
}

def main() -> i64 {
    let a = id(1)
    let b = id("hello")
    a
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("generic calls with different concrete types should typecheck");
}

#[test]
fn filtered_typecheck_keeps_generic_signature_valid() {
    let source = r#"
def helper<T>(x: T) -> T {
    x
}

def main() -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    let checked = HashSet::from([String::from("main")]);
    checker
        .check_program_with_filtered_function_bodies(&program, &checked)
        .expect("signature-only typecheck should support generic params");
}

#[test]
fn generic_struct_type_annotation_with_explicit_args_typechecks() {
    let source = r#"
struct Box<T> {
    value: T,
}

def accept(x: Box<i64>) -> i64 {
    0
}

def main() -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("generic struct type argument should be checked");
}

#[test]
fn generic_struct_missing_required_args_is_rejected() {
    let source = r#"
struct Pair<T, U> {
    first: T,
    second: U,
}

def bad(x: Pair<i64>) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    let result = checker.check_program(&program);
    assert!(result.is_err(), "missing generic args should be rejected");
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("missing generic argument") || msg.contains("generic"),
        "error should mention generic argument issue, got: {}",
        msg
    );
}

#[test]
fn generic_struct_default_type_argument_is_applied() {
    let source = r#"
struct Pair<T, U = i64> {
    first: T,
    second: U,
}

def ok(x: Pair<bool>) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("default generic type argument should be applied");
}

#[test]
fn nested_generic_type_arguments_with_right_shift_tokens_typecheck() {
    let source = r#"
struct Box<T> {
    value: T,
}

struct Wrap<T> {
    value: T,
}

def f(x: Wrap<Box<i64>>) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("nested generic args should parse/typecheck even with >>");
}

#[test]
fn generic_struct_where_clause_is_supported() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

struct Box<T> where T: Showable {
    value: T,
}

impl Showable for i64 {
    def show(self) -> i64 {
        self
    }
}

def consume(x: Box<i64>) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("struct where-clause bounds should typecheck");
}

#[test]
fn generic_type_alias_where_clause_is_supported() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

type Alias<T> where T: Showable = T;

impl Showable for i64 {
    def show(self) -> i64 {
        self
    }
}

def consume(x: Alias<i64>) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("type alias where-clause bounds should typecheck");
}
