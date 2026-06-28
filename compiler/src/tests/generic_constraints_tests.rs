use crate::{compile_to_ir, CompileError, Parser, TypeChecker};

#[test]
fn generic_trait_bound_accepts_types_with_impl() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

impl Showable for i64 {
    def show(self) -> i64 {
        self
    }
}

def consume<T: Showable>(x: T) -> i64 {
    0
}

def main() -> i64 {
    consume(42)
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("generic trait bound should accept implemented type");
}

#[test]
fn generic_trait_bound_allows_method_calls_inside_generic_body() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

impl Showable for i64 {
    def show(self) -> i64 {
        self
    }
}

def consume<T: Showable>(x: T) -> i64 {
    x.show()
}

def main() -> i64 {
    consume(42)
}
"#;

    let ir = compile_to_ir(source)
        .expect("a trait bound should make its methods available inside the generic body");
    assert!(
        ir.contains("Showable") || ir.contains("show"),
        "IR should retain the selected trait method call:\n{ir}"
    );
}

#[test]
fn generic_trait_bound_rejects_types_without_impl() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

def consume<T: Showable>(x: T) -> i64 {
    0
}

def main() -> i64 {
    consume(42)
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    let result = checker.check_program(&program);
    assert!(result.is_err(), "missing trait impl should be rejected");
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("Showable") || msg.contains("constraint"),
        "error should mention trait bound context, got: {}",
        msg
    );
}

#[test]
fn generic_trait_bound_rejects_types_without_impl_with_stable_diagnostic() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

def consume<T: Showable>(x: T) -> i64 {
    0
}

def main() -> i64 {
    consume(42)
}
"#;

    let err = compile_to_ir(source).expect_err("missing trait impl should fail");
    let CompileError::TypeckError(typeck) = err else {
        panic!("expected TypeckError, got {err}");
    };
    let target_lo = source.rfind("consume(").expect("call target should exist") as u32;

    assert_eq!(typeck.stable_code(), Some("unsatisfied-trait-bound"));
    assert_eq!(
        typeck.span(),
        Some((target_lo, target_lo + "consume(".len() as u32))
    );
    let message = typeck.to_string();
    assert!(message.contains("Showable"), "message: {message}");
    assert!(message.contains("i64"), "message: {message}");
    assert!(message.contains("T"), "message: {message}");
}

#[test]
fn generic_where_clause_accepts_types_with_impl() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

impl Showable for i64 {
    def show(self) -> i64 {
        self
    }
}

def consume<T>(x: T) -> i64 where T: Showable {
    0
}

def main() -> i64 {
    consume(42)
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("where-clause generic bound should accept implemented type");
}

#[test]
fn generic_where_clause_rejects_types_without_impl() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

def consume<T>(x: T) -> i64 where T: Showable {
    0
}

def main() -> i64 {
    consume(42)
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    let result = checker.check_program(&program);
    assert!(result.is_err(), "where-clause bound should be enforced");
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("Showable") || msg.contains("constraint"),
        "error should mention bound context, got: {}",
        msg
    );
}

#[test]
fn generic_where_clause_unknown_type_param_is_rejected_during_parse() {
    let source = r#"
trait Showable {}

def consume<T>(x: T) -> i64 where U: Showable {
    0
}
"#;

    let result = Parser::parse(source);
    assert!(
        result.is_err(),
        "unknown type param in where should fail parse"
    );
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("unknown type parameter") || msg.contains("where"),
        "error should mention invalid where clause, got: {}",
        msg
    );
}
