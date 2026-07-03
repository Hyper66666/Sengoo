//! Unit tests for the compiler-known `Drop` trait contract (task 1.1).
//!
//! Drop glue is compiler-inserted, so the only method of a `Drop` trait or an
//! `impl Drop for T` must be `def drop(&mut self)`. A malformed receiver or an
//! extra parameter is rejected during type checking instead of surfacing as a
//! confusing lowering error later.

use crate::{Parser, TypeChecker};

fn typecheck(source: &str) -> Result<(), String> {
    let parsed = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&parsed)
        .map_err(|err| format!("{err:?}"))
}

#[test]
fn drop_trait_is_compiler_known_by_default() {
    let checker = TypeChecker::new();
    assert!(
        checker.trait_registry().contains("Drop"),
        "`Drop` should be seeded as a compiler-known trait"
    );
}

#[test]
fn drop_impl_with_mut_self_typechecks() {
    typecheck(
        r#"
struct Widget {
    id: i64,
}

impl Drop for Widget {
    def drop(&mut self) {
    }
}
"#,
    )
    .expect("a `Drop` impl with `&mut self` should typecheck");
}

#[test]
fn drop_impl_with_shared_self_is_rejected() {
    let err = typecheck(
        r#"
struct Widget {
    id: i64,
}

impl Drop for Widget {
    def drop(&self) {
    }
}
"#,
    )
    .expect_err("a `Drop` impl with `&self` must be rejected");
    assert!(
        err.contains("drop-trait-contract"),
        "expected the Drop receiver diagnostic, got: {err}"
    );
}

#[test]
fn drop_impl_with_extra_param_is_rejected() {
    let err = typecheck(
        r#"
struct Widget {
    id: i64,
}

impl Drop for Widget {
    def drop(&mut self, extra: i64) {
    }
}
"#,
    )
    .expect_err("a `Drop` impl with an extra parameter must be rejected");
    assert!(
        err.contains("drop-trait-contract"),
        "expected the Drop parameter diagnostic, got: {err}"
    );
}

#[test]
fn direct_drop_trait_call_is_rejected() {
    let err = typecheck(
        r#"
struct Widget {
    id: i64,
}

impl Drop for Widget {
    def drop(&mut self) {
    }
}

def main() -> i64 {
    let w = Widget { id: 1 };
    w.drop();
    0
}
"#,
    )
    .expect_err("user code must not call a `Drop` trait method directly");
    assert!(
        err.contains("drop-direct-call"),
        "expected the direct Drop call diagnostic, got: {err}"
    );
}
