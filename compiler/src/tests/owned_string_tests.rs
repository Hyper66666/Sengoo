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
fn copy_scalars_remain_usable_after_by_value_copy_sites() {
    let source = r#"
def take_i64(value: i64) -> i64 {
    value
}

def take_bool(value: bool) -> i64 {
    if value { 1 } else { 0 }
}

def take_f64(value: f64) -> f64 {
    value
}

def main() -> i64 {
    let n: i64 = 7;
    let copied_n = n;
    let echoed_n = take_i64(n);
    let after_arg_n = n;

    let flag: bool = true;
    let copied_flag = flag;
    let flag_score = take_bool(flag);
    let flag_after_arg = if flag { 1 } else { 0 };

    let real: f64 = 2.5;
    let copied_real = real;
    let echoed_real = take_f64(real);
    let echoed_real_again = take_f64(real);

    if copied_flag {
        copied_n + echoed_n + after_arg_n + flag_score + flag_after_arg
    } else {
        0
    }
}
"#;
    let parsed = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&parsed)
        .expect("Copy scalar bindings and arguments should remain usable");
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
