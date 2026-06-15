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
fn drop_impl_with_mut_self_typechecks() {
    typecheck(
        r#"
trait Drop {
    def drop(&mut self) {
    }
}

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
trait Drop {
    def drop(&mut self) {
    }
}

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
        err.contains("Drop::drop must use `&mut self`"),
        "expected the Drop receiver diagnostic, got: {err}"
    );
}

#[test]
fn drop_impl_with_extra_param_is_rejected() {
    let err = typecheck(
        r#"
trait Drop {
    def drop(&mut self) {
    }
}

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
        err.contains("Drop::drop must take no parameters"),
        "expected the Drop parameter diagnostic, got: {err}"
    );
}
