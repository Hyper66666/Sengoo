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
fn match_guard_cannot_move_owned_payload_binding() {
    let source = r#"
struct Owned { value: i64 }
impl Drop for Owned {
    def drop(&mut self) { self.value = 0; }
}
enum MaybeOwned { Empty, Value(Owned) }
def consume_and_reject(value: Owned) -> bool { value.value == 99 }
def main() -> i64 {
    let value: MaybeOwned = MaybeOwned::Value(Owned { value: 7 });
    match value {
        MaybeOwned::Value(inner) if consume_and_reject(inner) => 1,
        MaybeOwned::Value(inner) => inner.value,
        MaybeOwned::Empty => 0,
    }
}
"#;
    let mut parser = Parser::new(source);
    let program = parser.parse_program().expect("parse");
    let err = typeck(&program).expect_err("a match guard must not consume its payload binding");
    assert!(
        err.to_string().contains("[match-guard-move]"),
        "expected stable match-guard-move diagnostic, got: {err}"
    );
}

#[test]
fn match_guard_cannot_move_owned_payload_through_by_value_method_receiver() {
    let source = r#"
struct Owned { value: i64 }
impl Drop for Owned {
    def drop(&mut self) { self.value = 0; }
}
impl Owned {
    def consume(self) -> bool { self.value == 99 }
}
enum MaybeOwned { Empty, Value(Owned) }
def main() -> i64 {
    let value: MaybeOwned = MaybeOwned::Value(Owned { value: 7 });
    match value {
        MaybeOwned::Value(inner) if inner.consume() => 1,
        MaybeOwned::Value(inner) => inner.value,
        MaybeOwned::Empty => 0,
    }
}
"#;
    let mut parser = Parser::new(source);
    let program = parser.parse_program().expect("parse");
    let err = typeck(&program).expect_err("a match guard must not consume its receiver binding");
    assert!(
        err.to_string().contains("[match-guard-move]"),
        "expected stable match-guard-move diagnostic, got: {err}"
    );
}

#[test]
fn match_guard_cannot_move_owned_payload_through_method_argument() {
    let source = r#"
struct Owned { value: i64 }
impl Drop for Owned {
    def drop(&mut self) { self.value = 0; }
}
struct Inspector {}
impl Inspector {
    def consume_and_reject(&self, value: Owned) -> bool { value.value == 99 }
}
enum MaybeOwned { Empty, Value(Owned) }
def main() -> i64 {
    let inspector = Inspector {};
    let value: MaybeOwned = MaybeOwned::Value(Owned { value: 7 });
    match value {
        MaybeOwned::Value(inner) if inspector.consume_and_reject(inner) => 1,
        MaybeOwned::Value(inner) => inner.value,
        MaybeOwned::Empty => 0,
    }
}
"#;
    let mut parser = Parser::new(source);
    let program = parser.parse_program().expect("parse");
    let err =
        typeck(&program).expect_err("a match guard must not consume a method argument binding");
    assert!(
        err.to_string().contains("[match-guard-move]"),
        "expected stable match-guard-move diagnostic, got: {err}"
    );
}

#[test]
fn match_guard_cannot_move_owned_payload_through_associated_function() {
    let source = r#"
struct Owned { value: i64 }
impl Drop for Owned {
    def drop(&mut self) { self.value = 0; }
}
impl Owned {
    def consume_and_reject(value: Owned) -> bool { value.value == 99 }
}
enum MaybeOwned { Empty, Value(Owned) }
def main() -> i64 {
    let value: MaybeOwned = MaybeOwned::Value(Owned { value: 7 });
    match value {
        MaybeOwned::Value(inner) if Owned::consume_and_reject(inner) => 1,
        MaybeOwned::Value(inner) => inner.value,
        MaybeOwned::Empty => 0,
    }
}
"#;
    let mut parser = Parser::new(source);
    let program = parser.parse_program().expect("parse");
    let err = typeck(&program)
        .expect_err("a match guard must not consume an associated argument binding");
    assert!(
        err.to_string().contains("[match-guard-move]"),
        "expected stable match-guard-move diagnostic, got: {err}"
    );
}

#[test]
fn match_guard_cannot_move_owned_payload_through_trait_associated_function() {
    let source = r#"
struct Owned { value: i64 }
impl Drop for Owned {
    def drop(&mut self) { self.value = 0; }
}
trait Reject {
    def consume_and_reject(value: Owned) -> bool {}
}
impl Reject for Owned {
    def consume_and_reject(value: Owned) -> bool { value.value == 99 }
}
enum MaybeOwned { Empty, Value(Owned) }
def main() -> i64 {
    let value: MaybeOwned = MaybeOwned::Value(Owned { value: 7 });
    match value {
        MaybeOwned::Value(inner) if Reject::consume_and_reject(inner) => 1,
        MaybeOwned::Value(inner) => inner.value,
        MaybeOwned::Empty => 0,
    }
}
"#;
    let mut parser = Parser::new(source);
    let program = parser.parse_program().expect("parse");
    let err = typeck(&program)
        .expect_err("a match guard must not consume a trait associated argument binding");
    assert!(
        err.to_string().contains("[match-guard-move]"),
        "expected stable match-guard-move diagnostic, got: {err}"
    );
}

#[test]
fn match_guard_can_borrow_owned_payload_through_method_receiver() {
    typeck_ok(
        r#"
struct Owned { value: i64 }
impl Drop for Owned {
    def drop(&mut self) { self.value = 0; }
}
impl Owned {
    def inspect(&self) -> bool { self.value == 99 }
}
enum MaybeOwned { Empty, Value(Owned) }
def main() -> i64 {
    let value: MaybeOwned = MaybeOwned::Value(Owned { value: 7 });
    match value {
        MaybeOwned::Value(inner) if inner.inspect() => 1,
        MaybeOwned::Value(inner) => inner.value,
        MaybeOwned::Empty => 0,
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
