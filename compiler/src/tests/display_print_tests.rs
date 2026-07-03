//! Tests for `Display`-driven `print`/`println`/`eprintln` (G2a-1).
//!
//! A type with a user `impl Display` is printed by calling its `to_string`
//! method and emitting the resulting owned `String`'s text, instead of the
//! built-in structural printer. Owned `String` values print their own text.

use crate::compile_to_ir;
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

fn compile_with_stdlib(program: &str) -> Result<String, String> {
    let source = format!(
        "{}\n\n{}",
        load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]),
        program
    );
    compile_to_ir(&source).map_err(|err| format!("{err:?}"))
}

fn compile_failure(program: &str) -> String {
    match compile_with_stdlib(program) {
        Ok(_) => panic!("expected compilation to fail"),
        Err(err) => err,
    }
}

#[test]
fn print_dispatches_through_user_display_impl() {
    let ir = compile_with_stdlib(
        r#"
struct Point {
    x: i64,
    y: i64,
}

impl Display for Point {
    def to_string(&self) -> String {
        string_from_str("Point").value
    }
}

def main() -> i64 {
    let p = Point { x: 1, y: 2 };
    print(p);
    0
}
"#,
    )
    .expect("print of a Display type should compile");

    assert!(
        ir.contains("@Point_Display_to_string"),
        "expected print to call the user Display impl, got:\n{ir}"
    );
    assert!(
        ir.contains("@sengoo_print_string("),
        "expected the rendered String to be printed via sengoo_print_string, got:\n{ir}"
    );
}

#[test]
fn print_owned_string_emits_string_text() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let s = string_from_str("hello").value;
    print(s);
    0
}
"#,
    )
    .expect("printing an owned String should compile");

    assert!(
        ir.contains("@sengoo_print_string("),
        "expected owned String to print its text via sengoo_print_string, got:\n{ir}"
    );
}

#[test]
fn eprintln_owned_string_uses_stderr_runtime_call() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let s = string_from_str("oops").value;
    eprintln(s);
    0
}
"#,
    )
    .expect("eprintln of an owned String should compile");

    assert!(
        ir.contains("@sengoo_eprint_string("),
        "expected eprintln(String) to use the stderr runtime call, got:\n{ir}"
    );
    assert!(
        ir.contains("declare void @sengoo_eprint_string(i64)"),
        "expected the stderr String declaration to be emitted on demand, got:\n{ir}"
    );
}

#[test]
fn to_string_method_dispatches_to_display_impl() {
    let ir = compile_with_stdlib(
        r#"
struct Tag {
    id: i64,
}

impl Display for Tag {
    def to_string(&self) -> String {
        string_from_str("Tag").value
    }
}

def main() -> i64 {
    let t = Tag { id: 7 };
    let rendered = t.to_string();
    rendered.len()
}
"#,
    )
    .expect("calling to_string on a Display type should compile");

    assert!(
        ir.contains("@Tag_Display_to_string"),
        "expected t.to_string() to resolve to the Display impl, got:\n{ir}"
    );
}

#[test]
fn display_impl_without_to_string_is_rejected() {
    let err = compile_failure(
        r#"
struct Bare {
    id: i64,
}

impl Display for Bare {
}

def main() -> i64 {
    0
}
"#,
    );

    assert!(
        err.contains("display-contract"),
        "expected the Display contract diagnostic, got: {err}"
    );
}

#[test]
fn display_impl_with_wrong_to_string_return_is_rejected() {
    let err = compile_failure(
        r#"
struct Bad {
    id: i64,
}

impl Display for Bad {
    def to_string(&self) -> i64 {
        0
    }
}

def main() -> i64 {
    0
}
"#,
    );

    assert!(
        err.contains("display-contract"),
        "expected the Display contract diagnostic, got: {err}"
    );
}

#[test]
fn print_without_display_still_uses_structural_printer() {
    let ir = compile_with_stdlib(
        r#"
struct Pair {
    a: i64,
    b: i64,
}

def main() -> i64 {
    let p = Pair { a: 1, b: 2 };
    print(p);
    0
}
"#,
    )
    .expect("structural print should still compile");

    assert!(
        ir.contains("@sengoo_print_i64("),
        "expected the structural printer to print fields, got:\n{ir}"
    );
    assert!(
        !ir.contains("@Pair_Display_to_string"),
        "a type without a Display impl must not dispatch through Display, got:\n{ir}"
    );
}
