use crate::compile_to_ir;
use crate::{Parser, TypeChecker};
use std::fs;
use std::path::Path;

fn load_stdlib(modules: &[&str]) -> String {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_root = manifest_dir
        .parent()
        .unwrap_or(manifest_dir)
        .join("tools")
        .join("stdlib");
    modules
        .iter()
        .map(|module| {
            fs::read_to_string(stdlib_root.join(module))
                .unwrap_or_else(|err| panic!("failed to read {module}: {err}"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn typecheck_with_stdlib(program: &str) -> Result<(), String> {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        program
    );
    let parsed = Parser::parse(&source).expect("source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&parsed)
        .map_err(|err| format!("{err:?}"))
}

fn typecheck_fails_with_stdlib(program: &str) -> String {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        program
    );
    let parsed = Parser::parse(&source).expect("source should parse");
    let mut checker = TypeChecker::new();
    match checker.check_program(&parsed) {
        Ok(()) => panic!("expected typecheck/borrow failure"),
        Err(err) => format!("{err:?}"),
    }
}

#[test]
fn stdlib_owned_string_typechecks_from_str() {
    typecheck_with_stdlib(
        r#"
def main() -> i64 {
    let built = string_from_str("hello");
    if built.is_ok {
        built.value.len()
    } else {
        0
    }
}
"#,
    )
    .expect("stdlib owned string should typecheck");
}

#[test]
fn stdlib_owned_string_add_assign_str_lowers_to_push_str() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def main() -> i64 {
    let mut text = string_from_str("hi").value;
    text += "!";
    text.len()
}
"#
    );

    let ir = compile_to_ir(&source).expect("String += &str should compile");
    assert!(
        ir.contains("@sengoo_string_push_str_status")
            && ir.contains("@sengoo_panic_result_unwrap_i64"),
        "expected String += &str to lower to checked in-place push_str, got:\n{ir}"
    );
}

#[test]
fn stdlib_str_plus_owned_string_builds_owned_string() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def main() -> i64 {
    let tail = string_from_str("tail").value;
    let text = "head-" + tail;
    text.len()
}
"#
    );

    let ir = compile_to_ir(&source).expect("&str + String should compile");
    assert!(
        ir.contains("@sengoo_string_from_str_copy")
            && ir.contains("@sengoo_string_as_str_ptr")
            && ir.contains("@sengoo_string_push_str_status")
            && ir.contains("@sengoo_panic_result_unwrap_i64"),
        "expected &str + String to build and append an owned String, got:\n{ir}"
    );
}

#[test]
fn stdlib_owned_string_as_str_borrows_owner_and_lowers_to_runtime_pointer() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def main() -> i64 {
    let text = string_from_str("borrowed").value;
    let view = text.as_str();
    view.len()
}
"#
    );

    let ir = compile_to_ir(&source).expect("String.as_str should compile");
    assert!(
        ir.contains("@sengoo_string_as_str_ptr") && ir.contains("@sengoo_str_len"),
        "expected String.as_str to lower to a borrowed &str view, got:\n{ir}"
    );
}

#[test]
fn stdlib_owned_string_as_str_view_prevents_owner_move() {
    let err = typecheck_fails_with_stdlib(
        r#"
def main() -> i64 {
    let owner: String = string_from_str("borrowed").value;
    let view = owner.as_str();
    let moved = owner;
    view.len()
}
"#,
    );
    assert!(
        err.contains("cannot move borrowed value `owner`"),
        "unexpected error: {err}"
    );
}

#[test]
fn stdlib_owned_string_as_str_view_cannot_escape_return() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def leak() -> &str {
    let owner: String = string_from_str("borrowed").value;
    let view = owner.as_str();
    return view;
}
"#,
    );
    let parsed = Parser::parse(&source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&parsed)
        .expect_err("returning a borrowed view should fail type checking");
    let crate::error::CompileError::TypeckError(typeck) = &err else {
        panic!("expected typeck error, got {err:?}");
    };
    assert_eq!(typeck.stable_code(), Some("borrow-escapes-owner"));
    let err = format!("{err:?}");
    assert!(
        err.contains("borrowed view `view` escapes its owner scope"),
        "unexpected error: {err}"
    );
}

#[test]
fn stdlib_owned_string_rebound_as_str_view_cannot_escape_return() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def leak() -> &str {
    let owner: String = string_from_str("borrowed").value;
    let view = owner.as_str();
    let rebound = view;
    return rebound;
}
"#,
    );
    let parsed = Parser::parse(&source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&parsed)
        .expect_err("returning a rebound borrowed view should fail type checking");
    let crate::error::CompileError::TypeckError(typeck) = &err else {
        panic!("expected typeck error, got {err:?}");
    };
    assert_eq!(typeck.stable_code(), Some("borrow-escapes-owner"));
}

#[test]
fn stdlib_owned_string_rebound_as_str_view_prevents_owner_move() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def main() -> i64 {
    let owner: String = string_from_str("borrowed").value;
    let view = owner.as_str();
    let rebound = view;
    let moved = owner;
    rebound.len()
}
"#,
    );
    let parsed = Parser::parse(&source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&parsed)
        .expect_err("moving an owner with a rebound view should fail");
    let crate::error::CompileError::TypeckError(typeck) = err else {
        panic!("expected TypeckError, got {err:?}");
    };
    assert_eq!(typeck.stable_code(), Some("cannot-move-borrowed"));
}

#[test]
fn stdlib_owned_string_reassigned_as_str_view_cannot_escape_return() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def leak() -> &str {
    let owner: String = string_from_str("borrowed").value;
    let mut alias = "fallback";
    alias = owner.as_str();
    return alias;
}
"#,
    );
    let parsed = Parser::parse(&source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&parsed)
        .expect_err("returning a reassigned borrowed view should fail type checking");
    let crate::error::CompileError::TypeckError(typeck) = &err else {
        panic!("expected typeck error, got {err:?}");
    };
    assert_eq!(typeck.stable_code(), Some("borrow-escapes-owner"));
}

#[test]
fn stdlib_owned_string_reassigned_as_str_view_prevents_owner_move() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def main() -> i64 {
    let owner: String = string_from_str("borrowed").value;
    let mut alias = "fallback";
    alias = owner.as_str();
    let moved = owner;
    alias.len()
}
"#,
    );
    let parsed = Parser::parse(&source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&parsed)
        .expect_err("moving an owner with a reassigned view should fail");
    let crate::error::CompileError::TypeckError(typeck) = err else {
        panic!("expected TypeckError, got {err:?}");
    };
    assert_eq!(typeck.stable_code(), Some("cannot-move-borrowed"));
}

#[test]
fn stdlib_owned_string_as_str_view_cannot_escape_tail_expression() {
    let err = typecheck_fails_with_stdlib(
        r#"
def leak() -> &str {
    let owner: String = string_from_str("borrowed").value;
    let view = owner.as_str();
    view
}
"#,
    );
    assert!(
        err.contains("borrowed view `view` escapes its owner scope"),
        "unexpected error: {err}"
    );
}

#[test]
fn stdlib_owned_string_as_str_view_cannot_escape_in_tuple() {
    let err = typecheck_fails_with_stdlib(
        r#"
def leak() -> (&str, i64) {
    let owner: String = string_from_str("borrowed").value;
    let view = owner.as_str();
    (view, 1)
}
"#,
    );
    assert!(
        err.contains("borrowed view `view` escapes its owner scope"),
        "unexpected error: {err}"
    );
}

#[test]
fn stdlib_owned_string_as_str_view_cannot_escape_in_struct_literal() {
    let err = typecheck_fails_with_stdlib(
        r#"
struct Holder {
    view: &str,
}

def leak() -> Holder {
    let owner: String = string_from_str("borrowed").value;
    return Holder { view: owner.as_str() };
}
"#,
    );
    assert!(
        err.contains("escapes its owner scope"),
        "unexpected error: {err}"
    );
}

#[test]
fn stdlib_owned_string_as_str_view_cannot_escape_if_else_tail() {
    let err = typecheck_fails_with_stdlib(
        r#"
def leak(flag: bool) -> &str {
    let owner: String = string_from_str("borrowed").value;
    let view = owner.as_str();
    if flag {
        view
    } else {
        "fallback"
    }
}
"#,
    );
    assert!(
        err.contains("borrowed view `view` escapes its owner scope"),
        "unexpected error: {err}"
    );
}

#[test]
fn stdlib_owned_string_move_rejects_use_after_move() {
    let err = typecheck_fails_with_stdlib(
        r#"
def main() -> i64 {
    let a: String = string_from_str("a").value;
    let b = a;
    a.len()
}
"#,
    );
    assert!(
        err.contains("use of moved value `a`"),
        "unexpected error: {err}"
    );
}

#[test]
fn stdlib_owned_string_use_after_move_reports_stable_code() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def main() -> i64 {
    let a: String = string_from_str("a").value;
    let b = a;
    a.len()
}
"#
    );
    let parsed = Parser::parse(&source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&parsed)
        .expect_err("use after move should fail type checking");

    let crate::error::CompileError::TypeckError(typeck_err) = err else {
        panic!("expected typeck error, got {err:?}");
    };
    assert_eq!(typeck_err.stable_code(), Some("use-after-move"));
}

#[test]
fn stdlib_owned_string_move_rejects_use_after_inner_block_move() {
    let err = typecheck_fails_with_stdlib(
        r#"
def main() -> i64 {
    let a: String = string_from_str("a").value;
    {
        let b = a;
        b.len()
    }
    a.len()
}
"#,
    );
    assert!(
        err.contains("use of moved value `a`"),
        "unexpected error: {err}"
    );
}

#[test]
fn stdlib_owned_string_assignment_move_rejects_use_after_move() {
    let err = typecheck_fails_with_stdlib(
        r#"
def main() -> i64 {
    let a: String = string_from_str("a").value;
    let mut b: String = string_from_str("b").value;
    b = a;
    a.len()
}
"#,
    );
    assert!(
        err.contains("use of moved value `a`"),
        "unexpected error: {err}"
    );
}

#[test]
fn stdlib_owned_string_user_struct_does_not_get_move_rules() {
    let source = r#"
struct MyString {
    handle: i64,
}

def main() -> i64 {
    let a: MyString = MyString { handle: 1 };
    let b: MyString = a;
    a.handle
}
"#;
    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("non-canonical string struct should not move-check");
}

#[test]
fn user_drop_type_move_rejects_use_after_move() {
    let source = r#"
struct Resource {
    handle: i64,
}

impl Drop for Resource {
    def drop(&mut self) {
    }
}

def main() -> i64 {
    let a: Resource = Resource { handle: 1 };
    let b = a;
    a.handle
}
"#;
    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("a moved user Drop type should reject later use");
    let err = format!("{err:?}");
    assert!(
        err.contains("use of moved value `a`"),
        "unexpected error: {err}"
    );
}

#[test]
fn stdlib_owned_string_return_marks_value_moved() {
    let err = typecheck_fails_with_stdlib(
        r#"
def return_then_reuse(value: String) -> String {
    return value;
    value
}
"#,
    );
    assert!(
        err.contains("use of moved value `value`"),
        "unexpected error: {err}"
    );
}

#[test]
fn stdlib_owned_string_cannot_move_while_borrowed() {
    // Live use of the borrow after the move site keeps the owner borrowed (D1 last-use).
    let err = typecheck_fails_with_stdlib(
        r#"
def main() -> i64 {
    let owner: String = string_from_str("borrowed").value;
    let view = &owner;
    let moved = owner;
    view.len()
}
"#,
    );
    assert!(
        err.contains("cannot move borrowed value `owner`"),
        "unexpected error: {err}"
    );
}

#[test]
fn moving_a_borrowed_owner_reports_stable_diagnostic() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def main() -> i64 {
    let owner: String = string_from_str("borrowed").value;
    let view = &owner;
    let moved = owner;
    view.len()
}
"#
    );
    let program = Parser::parse(&source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("moving a borrowed owner should fail");
    let crate::error::CompileError::TypeckError(typeck) = err else {
        panic!("expected TypeckError, got {err:?}");
    };
    assert_eq!(typeck.stable_code(), Some("cannot-move-borrowed"));
    let owner_move = source.rfind("owner;").expect("move site should exist") as u32;
    assert_eq!(
        typeck.span(),
        Some((owner_move, owner_move + "owner;".len() as u32))
    );
}

#[test]
fn borrowed_owning_field_cannot_be_moved() {
    let err = typecheck_fails_with_stdlib(
        r#"
struct Pair {
    left: String,
    right: String,
}

def main() -> i64 {
    let pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    let view = &pair.left;
    let moved = pair.left;
    0
}
"#,
    );
    assert!(
        err.contains("cannot move borrowed value `pair.left`"),
        "unexpected error: {err}"
    );
}

#[test]
fn parent_with_borrowed_owning_field_cannot_be_moved() {
    let err = typecheck_fails_with_stdlib(
        r#"
struct Pair {
    left: String,
    right: String,
}

def main() -> i64 {
    let pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    let view = &pair.left;
    let moved = pair;
    0
}
"#,
    );
    assert!(
        err.contains("cannot move borrowed value `pair`"),
        "unexpected error: {err}"
    );
}

#[test]
fn rc_owner_cannot_move_while_borrow_is_live() {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&[
            "option.sg",
            "result.sg",
            "ffi.sg",
            "string.sg",
            "collections.sg"
        ]),
        r#"
def main() -> i64 {
    let first = rc_new(21);
    let view = first.borrow();
    let moved = first;
    *view
}
"#
    );
    let program = Parser::parse(&source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("moving an Rc owner while a borrow is live should fail");
    let crate::error::CompileError::TypeckError(typeck) = err else {
        panic!("expected TypeckError, got {err:?}");
    };
    assert_eq!(typeck.stable_code(), Some("cannot-move-borrowed"));
}

#[test]
fn borrowing_one_owning_field_allows_moving_its_sibling() {
    typecheck_with_stdlib(
        r#"
struct Pair {
    left: String,
    right: String,
}

def main() -> i64 {
    let pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    let view = &pair.left;
    let moved = pair.right;
    0
}
"#,
    )
    .expect("disjoint sibling fields should not conflict");
}

#[test]
fn user_drop_field_return_marks_only_that_field_moved() {
    let source = r#"
struct Token {
    value: i64,
}

impl Drop for Token {
    def drop(&mut self) {
    }
}

struct Pair {
    left: Token,
    right: Token,
}

def take_left(pair: Pair) -> Token {
    return pair.left;
    pair.left
}
"#;
    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("returning an owning field should move it");
    let err = format!("{err:?}");
    assert!(
        err.contains("use of moved value `pair.left`"),
        "unexpected error: {err}"
    );
}

#[test]
fn owned_field_move_rejects_reusing_the_same_field() {
    let err = typecheck_fails_with_stdlib(
        r#"
struct Pair {
    left: String,
    right: String,
}

def main() -> i64 {
    let pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    let moved = pair.left;
    pair.left.len()
}
"#,
    );
    assert!(
        err.contains("use of moved value `pair.left`"),
        "unexpected error: {err}"
    );
}

#[test]
fn owned_field_move_keeps_sibling_field_available() {
    typecheck_with_stdlib(
        r#"
struct Pair {
    left: String,
    right: String,
}

def main() -> i64 {
    let pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    let moved = pair.left;
    pair.right.len()
}
"#,
    )
    .expect("moving one owning field should not move its sibling");
}

#[test]
fn owned_field_move_rejects_using_the_whole_parent_value() {
    let err = typecheck_fails_with_stdlib(
        r#"
struct Pair {
    left: String,
    right: String,
}

def consume(value: Pair) -> i64 {
    value.right.len()
}

def main() -> i64 {
    let pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    let moved = pair.left;
    consume(pair)
}
"#,
    );
    assert!(
        err.contains("use of partially moved value `pair`"),
        "unexpected error: {err}"
    );
}

#[test]
fn owned_field_assignment_reinitializes_a_moved_field() {
    typecheck_with_stdlib(
        r#"
struct Pair {
    left: String,
    right: String,
}

def main() -> i64 {
    let mut pair = Pair {
        left: string_from_str("left").value,
        right: string_from_str("right").value,
    };
    let moved = pair.left;
    pair.left = string_from_str("replacement").value;
    pair.left.len()
}
"#,
    )
    .expect("assigning a moved field should reinitialize that field");
}

#[test]
fn stdlib_owned_string_exact_capacity_emits_runtime_calls() {
    let ir = compile_to_ir(&format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        r#"
def main() -> i64 {
    let built = string_from_str("12345678");
    if built.is_ok {
        built.value.len()
    } else {
        0
    }
}
"#
    ))
    .expect("program should compile");
    assert!(ir.contains("sengoo_string_from_str_copy"));
    assert!(ir.contains("sengoo_string_len"));
}
