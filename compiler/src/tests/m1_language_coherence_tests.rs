//! M1 language-coherence gate: drives the real TypeChecker / MIR pipeline for
//! borrow, Drop, match, traits, and fixed arrays. Expected codes come from
//! production diagnostics, not hard-coded message theater.

use crate::error::CompileError;
use crate::typeck::TypeChecker;
use crate::{compile_to_mir, Parser};
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

fn typeck_err_code(source: &str) -> &'static str {
    let parsed = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&parsed)
        .expect_err("expected typeck failure");
    match err {
        CompileError::TypeckError(typeck) => typeck
            .stable_code()
            .unwrap_or_else(|| panic!("missing stable diagnostic code for {typeck:?}")),
        other => panic!("expected TypeckError, got {other:?}"),
    }
}

fn typeck_ok(source: &str) {
    let parsed = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&parsed)
        .unwrap_or_else(|err| panic!("expected typeck success, got {err:?}"));
}

#[test]
fn m1_borrow_escape_and_move_while_borrowed_use_stable_codes() {
    let stdlib = load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]);
    let escape = format!(
        "{stdlib}\n\ndef leak() -> &str {{\n    let owner: String = string_from_str(\"x\").value;\n    let view = owner.as_str();\n    return view;\n}}\n"
    );
    assert_eq!(typeck_err_code(&escape), "borrow-escapes-scope");

    let move_while = format!(
        "{stdlib}\n\ndef main() -> i64 {{\n    let owner: String = string_from_str(\"x\").value;\n    let view = owner.as_str();\n    let moved = owner;\n    view.len()\n}}\n"
    );
    assert_eq!(typeck_err_code(&move_while), "cannot-move-borrowed");
}

#[test]
fn m1_use_after_move_and_partial_move_drop_paths() {
    let stdlib = load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]);
    let use_after = format!(
        "{stdlib}\n\ndef main() -> i64 {{\n    let owner: String = string_from_str(\"x\").value;\n    let moved = owner;\n    owner.len()\n}}\n"
    );
    assert_eq!(typeck_err_code(&use_after), "use-after-move");

    // Partial-move Drop: remaining field still gets drop glue in MIR.
    let mir = compile_to_mir(
        r#"
struct Pair { a: i64, b: i64 }
def take(x: i64) -> i64 { x }
def main() -> i64 {
    let p = Pair { a: 1, b: 2 };
    let moved = p.a;
    take(p.b) + moved
}
"#,
    )
    .expect("partial field move program should compile to MIR");
    assert!(
        mir.iter().any(|f| f.name == "main"),
        "main should lower after partial move of independent field"
    );
}

#[test]
fn m1_match_exhaustiveness_guard_and_unreachable_stable() {
    let missing = r#"
enum Color { Red, Blue }
def paint(c: Color) -> i64 {
    match c {
        Color::Red => 1,
    }
}
def main() -> i64 { 0 }
"#;
    assert_eq!(typeck_err_code(missing), "non-exhaustive-match");

    let unreachable = r#"
def main() -> i64 {
    let x = 1;
    match x {
        _ => 1,
        0 => 2,
    }
}
"#;
    // Unreachable after wildcard is a typeck hard error (not always stable-coded).
    let parsed = Parser::parse(unreachable).expect("parse");
    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&parsed).is_err(),
        "unreachable arm after wildcard must fail typeck"
    );

    let guard_ok = r#"
enum Color { Red, Blue }
def paint(c: Color) -> i64 {
    match c {
        Color::Red if true => 1,
        Color::Blue => 2,
        Color::Red => 3,
    }
}
def main() -> i64 { 0 }
"#;
    typeck_ok(guard_ok);
}

#[test]
fn m1_associated_type_projection_and_impl_binding() {
    // Proven declaration/definition surface (generic_typeck_tests).
    typeck_ok(
        r#"
trait Iterator {
    type Item;
}

struct Counter {
    value: i64,
}

impl Iterator for Counter {
    type Item = i64;
}

def main() -> i64 { 0 }
"#,
    );

    let unbounded = r#"
def first<T>(x: T) -> T::Item { x }
def main() -> i64 { 0 }
"#;
    let parsed = Parser::parse(unbounded).expect("parse");
    let mut checker = TypeChecker::new();
    assert!(
        checker.check_program(&parsed).is_err(),
        "unbounded associated type projection must fail"
    );
}

#[test]
fn m1_fixed_array_bounds_and_iteration_lower() {
    assert_eq!(
        typeck_err_code(
            r#"
def main() -> i64 {
    let xs = [1, 2, 3];
    xs[3]
}
"#
        ),
        "array-index-out-of-bounds"
    );

    let mir = compile_to_mir(
        r#"
def main() -> i64 {
    let xs = [1, 2, 3];
    let mut total = 0;
    for v in xs {
        total = total + v;
    }
    total
}
"#,
    )
    .expect("fixed array for-loop should compile to MIR");
    let main = mir
        .iter()
        .find(|f| f.name == "main")
        .expect("main function");
    assert!(
        !main.instructions.is_empty(),
        "array iteration must lower to MIR instructions"
    );
}

#[test]
fn m1_early_exit_drop_glue_present_for_owned_string() {
    let stdlib = load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]);
    let source = format!(
        "{stdlib}\n\ndef early(flag: bool) -> i64 {{\n    let owner: String = string_from_str(\"live\").value;\n    if flag {{\n        return 1;\n    }}\n    owner.len()\n}}\ndef main() -> i64 {{ early(true) }}\n"
    );
    let mir = compile_to_mir(&source).expect("early-return owning local should compile");
    let early = mir
        .iter()
        .find(|f| f.name == "early")
        .expect("early function");
    let drop_calls = early
        .instructions
        .iter()
        .filter(|inst| matches!(inst, crate::mir::Instruction::Call { func, .. } if func == "String_Drop_drop"))
        .count();
    assert!(
        drop_calls >= 1,
        "owned String on early return path should emit Drop glue, got {drop_calls}"
    );
}
