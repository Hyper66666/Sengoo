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
