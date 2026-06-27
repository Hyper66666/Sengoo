//! Supertrait enforcement tests (`trait Sub: Super`).
//!
//! Supertraits parse into `Trait.bounds`; these tests cover the type-checker
//! enforcement added on top: a `Sub` impl requires the `Super` impl, declared
//! supertraits must name known traits, cycles are rejected, and a `T: Sub`
//! bound transitively provides the supertrait.

use crate::{Parser, TypeChecker};

fn check(source: &str) -> Result<(), String> {
    let program = Parser::parse(source).map_err(|e| format!("{e}"))?;
    let mut checker = TypeChecker::new();
    checker.check_program(&program).map_err(|e| format!("{e}"))
}

#[test]
fn supertrait_impl_present_is_accepted() {
    let source = r#"
trait Greet {
    def greet(self) -> i64 { 0 }
}

trait Loud: Greet {
    def shout(self) -> i64 { 0 }
}

impl Greet for i64 {
    def greet(self) -> i64 { self }
}

impl Loud for i64 {
    def shout(self) -> i64 { self }
}

def main() -> i64 { 0 }
"#;
    check(source).expect("subtrait impl with present supertrait impl should typecheck");
}

#[test]
fn supertrait_impl_missing_is_rejected() {
    let source = r#"
trait Greet {
    def greet(self) -> i64 { 0 }
}

trait Loud: Greet {
    def shout(self) -> i64 { 0 }
}

impl Loud for i64 {
    def shout(self) -> i64 { self }
}

def main() -> i64 { 0 }
"#;
    let err = check(source).expect_err("missing supertrait impl should be rejected");
    assert!(
        err.contains("missing-supertrait-impl") || err.contains("supertrait `Greet`"),
        "diagnostic should mention the missing supertrait, got: {err}"
    );
}

#[test]
fn unknown_supertrait_is_rejected() {
    let source = r#"
trait Loud: Nonexistent {
    def shout(self) -> i64 { 0 }
}

def main() -> i64 { 0 }
"#;
    let err = check(source).expect_err("unknown supertrait should be rejected");
    assert!(
        err.contains("unknown-supertrait") || err.contains("Nonexistent"),
        "diagnostic should mention the unknown supertrait, got: {err}"
    );
}

#[test]
fn supertrait_cycle_is_rejected() {
    let source = r#"
trait A: B {
    def a(self) -> i64 { 0 }
}

trait B: A {
    def b(self) -> i64 { 0 }
}

def main() -> i64 { 0 }
"#;
    let err = check(source).expect_err("supertrait cycle should be rejected");
    assert!(
        err.contains("supertrait-cycle"),
        "diagnostic should report a supertrait cycle, got: {err}"
    );
}

#[test]
fn transitive_supertrait_impl_is_required() {
    // `Loudest: Loud: Greet` — implementing `Loudest` requires both `Loud` and
    // its supertrait `Greet`.
    let source = r#"
trait Greet {
    def greet(self) -> i64 { 0 }
}

trait Loud: Greet {
    def shout(self) -> i64 { 0 }
}

trait Loudest: Loud {
    def scream(self) -> i64 { 0 }
}

impl Loud for i64 {
    def shout(self) -> i64 { self }
}

impl Loudest for i64 {
    def scream(self) -> i64 { self }
}

def main() -> i64 { 0 }
"#;
    let err = check(source).expect_err("transitive supertrait impl should be required");
    assert!(
        err.contains("Greet"),
        "diagnostic should mention transitive supertrait `Greet`, got: {err}"
    );
}

#[test]
fn subtrait_bound_requires_supertrait_at_call_site() {
    // A `T: Loud` parameter expands to also require `Greet`. Since every `Loud`
    // impl is forced to also implement `Greet`, a concrete `Loud` type always
    // satisfies the expanded bound set.
    let source = r#"
trait Greet {
    def greet(self) -> i64 { 0 }
}

trait Loud: Greet {
    def shout(self) -> i64 { 0 }
}

impl Greet for i64 {
    def greet(self) -> i64 { self }
}

impl Loud for i64 {
    def shout(self) -> i64 { self }
}

def use_loud<T: Loud>(x: T) -> i64 {
    0
}

def main() -> i64 {
    use_loud(7)
}
"#;
    check(source).expect("concrete Loud type satisfies the expanded supertrait bound set");
}
