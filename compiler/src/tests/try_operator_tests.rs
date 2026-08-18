//! Type checking and MIR lowering for `?` and `try {}`.

use crate::compile_to_ir;
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
    assert!(typeck(&program).is_err(), "expected typeck failure");
}

#[test]
fn result_question_in_result_function_typechecks() {
    typeck_ok(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok_i64(v: i64) -> Result<i64, i64> {
    Result { is_ok: true, value: v, error: 0 }
}
def step() -> Result<i64, i64> {
    ok_i64(1)
}
def main() -> Result<i64, i64> {
    let x = step()?;
    ok_i64(x + 1)
}
"#,
    );
}

#[test]
fn option_question_in_option_function_typechecks() {
    typeck_ok(
        r#"
struct Option<T> { is_some: bool, value: T }
def some_i64(v: i64) -> Option<i64> {
    Option { is_some: true, value: v }
}
def main() -> Option<i64> {
    let x = some_i64(3)?;
    some_i64(x + 1)
}
"#,
    );
}

#[test]
fn question_rejects_result_in_option_context() {
    typeck_err(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
struct Option<T> { is_some: bool, value: T }
def bad() -> Result<i64, i64> {
    Result { is_ok: true, value: 0, error: 0 }
}
def main() -> Option<i64> {
    let sink = bad()?;
    sink
}
"#,
    );
}

#[test]
fn question_rejects_plain_i64_main() {
    typeck_err(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok() -> Result<i64, i64> {
    Result { is_ok: true, value: 1, error: 0 }
}
def main() -> i64 {
    let sink = ok()?;
    sink
}
"#,
    );
}

#[test]
fn question_rejects_mismatched_result_error_type() {
    typeck_err(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def err_other() -> Result<i64, bool> {
    Result { is_ok: false, value: 0, error: true }
}
def main() -> Result<i64, i64> {
    let sink = err_other()?;
    Result { is_ok: true, value: sink, error: 0 }
}
"#,
    );
}

#[test]
fn try_block_allows_question_in_plain_main() {
    typeck_ok(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok_i64(v: i64) -> Result<i64, i64> {
    Result { is_ok: true, value: v, error: 0 }
}
def main() -> i64 {
    let r = try {
        let x = ok_i64(5)?;
        ok_i64(x + 1)
    };
    if r.is_ok { r.value } else { 0 }
}
"#,
    );
}

#[test]
fn nested_result_question_typechecks() {
    typeck_ok(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok_i64(v: i64) -> Result<i64, i64> {
    Result { is_ok: true, value: v, error: 0 }
}
def outer() -> Result<i64, i64> {
    let a = ok_i64(1)?;
    let b = ok_i64(a)?;
    ok_i64(b + 1)
}
def main() -> i64 { 0 }
"#,
    );
}

#[test]
fn try_block_scalar_success_wraps_inferred_result_container() {
    compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def fail() -> Result<i64, i64> {
    Result { is_ok: false, value: 0, error: 9 }
}
def main() -> i64 {
    let r = try {
        let x = fail()?;
        x + 1
    };
    if r.is_ok { r.value } else { 0 }
}
"#,
    )
    .expect("try block should wrap scalar success in Result");
}

#[test]
fn try_block_option_propagation_wraps_scalar_success() {
    compile_to_ir(
        r#"
struct Option<T> { is_some: bool, value: T }
def opt_none() -> Option<i64> {
    Option { is_some: false, value: 0 }
}
def main() -> i64 {
    let r = try {
        let x = opt_none()?;
        x + 1
    };
    if r.is_some { r.value } else { 0 }
}
"#,
    )
    .expect("try block should wrap scalar success in Option");
}

#[test]
fn try_block_lowers_without_double_wrapping_result() {
    compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok_i64(v: i64) -> Result<i64, i64> {
    Result { is_ok: true, value: v, error: 0 }
}
def main() -> i64 {
    let r = try {
        let x = ok_i64(5)?;
        ok_i64(x + 1)
    };
    if r.is_ok { r.value } else { 0 }
}
"#,
    )
    .expect("try block should lower to valid IR");
}

#[test]
fn result_question_lowers_with_branch_and_return() {
    let ir = compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
def ok_i64(v: i64) -> Result<i64, i64> {
    Result { is_ok: true, value: v, error: 0 }
}
def fail() -> Result<i64, i64> {
    Result { is_ok: false, value: 0, error: 7 }
}
def use_it() -> Result<i64, i64> {
    let x = fail()?;
    ok_i64(x)
}
def main() -> i64 { 0 }
"#,
    )
    .expect("compile");
    assert!(
        ir.contains("br i1") && ir.contains("extractvalue"),
        "expected branch/unpack for ?, got:\n{ir}"
    );
}

#[test]
fn result_question_rebuilds_failure_for_a_different_success_type() {
    let ir = compile_to_ir(
        r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
struct Token { value: i64 }

def fail_token() -> Result<Token, i64> {
    Result {
        is_ok: false,
        value: Token { value: 0 },
        error: 9,
    }
}

def bridge() -> Result<i64, i64> {
    let token = fail_token()?;
    Result { is_ok: true, value: token.value, error: 0 }
}

def main() -> i64 { 0 }
"#,
    )
    .expect("? should rebuild the error container for the caller's success type");

    let bridge = ir
        .split("; Function: bridge")
        .nth(1)
        .expect("bridge should be emitted");
    assert!(
        bridge.contains("ret %Result_i64_i64"),
        "bridge must return its declared Result<i64, i64> on every path:\n{bridge}"
    );
    assert!(
        !bridge.contains("ret %Result_Token_i64"),
        "the failure path must not return the operand container unchanged:\n{bridge}"
    );
}

#[test]
fn enum_result_question_propagates_result_to_result() {
    compile_to_ir(
        r#"
enum Result<T, E> { Ok(T), Err(E) }
def divide(a: i64, b: i64) -> Result<i64, i64> {
    if b == 0 { Err(1) } else { Ok(a / b) }
}
def chain(a: i64, b: i64) -> Result<i64, i64> {
    let value = divide(a, b)?;
    Ok(value + 100)
}
def main() -> i64 { 0 }
"#,
    )
    .expect("enum Result `?` should propagate Result to Result");
}

#[test]
fn enum_option_question_propagates_option_to_option() {
    compile_to_ir(
        r#"
enum Option<T> { None, Some(T) }
def find(n: i64) -> Option<i64> {
    if n > 0 { Some(n) } else { None }
}
def bump(n: i64) -> Option<i64> {
    let value = find(n)?;
    Some(value + 1)
}
def main() -> i64 { 0 }
"#,
    )
    .expect("enum Option `?` should propagate Option to Option");
}

#[test]
fn enum_question_rejects_result_in_option_context() {
    typeck_err(
        r#"
enum Result<T, E> { Ok(T), Err(E) }
enum Option<T> { None, Some(T) }
def ok() -> Result<i64, i64> { Ok(1) }
def main() -> Option<i64> {
    let sink = ok()?;
    Some(sink)
}
"#,
    );
}

#[test]
fn enum_question_rejects_plain_i64_main() {
    typeck_err(
        r#"
enum Result<T, E> { Ok(T), Err(E) }
def ok() -> Result<i64, i64> { Ok(1) }
def main() -> i64 {
    let sink = ok()?;
    sink
}
"#,
    );
}
