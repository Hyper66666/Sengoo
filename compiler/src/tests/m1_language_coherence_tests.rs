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
    assert_eq!(typeck_err_code(&escape), "borrow-escapes-owner");

    let move_while = format!(
        "{stdlib}\n\ndef main() -> i64 {{\n    let owner: String = string_from_str(\"x\").value;\n    let view = owner.as_str();\n    let moved = owner;\n    view.len()\n}}\n"
    );
    assert_eq!(typeck_err_code(&move_while), "cannot-move-borrowed");
}

#[test]
fn m1_last_use_borrow_allows_owner_move() {
    // D1: after last reachable use of the borrow alias, owner may move.
    let stdlib = load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]);
    let source = format!(
        "{stdlib}\n\ndef main() -> i64 {{\n    let owner: String = string_from_str(\"hi\").value;\n    let view = owner.as_str();\n    let n = view.len();\n    let moved = owner;\n    n + moved.len()\n}}\n"
    );
    typeck_ok(&source);
}

#[test]
fn m1_last_use_borrow_tracks_nested_control_flow_and_expression_order() {
    let stdlib = load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]);

    let after_if = format!(
        "{stdlib}\n\ndef test(flag: bool) -> i64 {{\n    let owner: String = string_from_str(\"x\").value;\n    let view = owner.as_str();\n    if flag {{ 1; }} else {{ 0; }};\n    let moved = owner;\n    view.len() + moved.len()\n}}\ndef main() -> i64 {{ test(true) }}\n"
    );
    assert_eq!(typeck_err_code(&after_if), "cannot-move-borrowed");

    let after_loop = format!(
        "{stdlib}\n\ndef test(flag: bool) -> i64 {{\n    let owner: String = string_from_str(\"x\").value;\n    let view = owner.as_str();\n    while flag {{ break; }};\n    let moved = owner;\n    view.len() + moved.len()\n}}\ndef main() -> i64 {{ test(false) }}\n"
    );
    assert_eq!(typeck_err_code(&after_loop), "cannot-move-borrowed");

    let same_expr = format!(
        "{stdlib}\n\ndef consume(value: String) -> i64 {{ value.len() }}\ndef main() -> i64 {{\n    let owner: String = string_from_str(\"x\").value;\n    let view = owner.as_str();\n    consume(owner) + view.len()\n}}\n"
    );
    assert_eq!(typeck_err_code(&same_expr), "cannot-move-borrowed");
}

#[test]
fn m1_last_use_borrow_allows_move_after_branch_join() {
    let stdlib = load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]);
    let source = format!(
        "{stdlib}\n\ndef consume(value: String) -> i64 {{ value.len() }}\ndef test(flag: bool) -> i64 {{\n    let owner: String = string_from_str(\"x\").value;\n    let view = owner.as_str();\n    let observed = if flag {{ view.len() }} else {{ 0 }};\n    consume(owner) + observed\n}}\ndef main() -> i64 {{ test(true) }}\n"
    );
    typeck_ok(&source);
}

#[test]
fn m1_call_argument_borrow_ends_after_the_statement() {
    typeck_ok(
        r#"
def observe(value: &mut i64) -> i64 { *value }

def main() -> i64 {
    let mut value = 20;
    let observed = observe(&mut value);
    let shared = &value;
    observed + *shared
}
"#,
    );
}

#[test]
fn m1_use_after_move_and_partial_move_drop_paths() {
    let stdlib = load_stdlib(&["option.sg", "result.sg", "ffi.sg", "string.sg"]);
    let use_after = format!(
        "{stdlib}\n\ndef main() -> i64 {{\n    let owner: String = string_from_str(\"x\").value;\n    let moved = owner;\n    owner.len()\n}}\n"
    );
    assert_eq!(typeck_err_code(&use_after), "use-after-move");

    // Non-Copy partial move: whole-value use after field move is use-after-partial-move.
    let partial = format!(
        "{stdlib}\n\nstruct Pair {{\n    a: String,\n    b: String,\n}}\n\ndef main() -> i64 {{\n    let p = Pair {{\n        a: string_from_str(\"a\").value,\n        b: string_from_str(\"b\").value,\n    }};\n    let moved = p.a;\n    let whole = p;\n    whole.b.len() + moved.len()\n}}\n"
    );
    assert_eq!(typeck_err_code(&partial), "use-after-partial-move");

    // Independent non-Copy field remains usable and Drop still lowers for remaining path.
    let mir = compile_to_mir(&format!(
        "{stdlib}\n\nstruct Pair {{\n    a: String,\n    b: String,\n}}\n\ndef main() -> i64 {{\n    let p = Pair {{\n        a: string_from_str(\"a\").value,\n        b: string_from_str(\"b\").value,\n    }};\n    let moved = p.a;\n    moved.len() + p.b.len()\n}}\n"
    ))
    .expect("partial field move of independent non-Copy field should compile to MIR");
    let main = mir
        .iter()
        .find(|f| f.name == "main")
        .expect("main function");
    let drop_calls = main
        .instructions
        .iter()
        .filter(|inst| matches!(inst, crate::mir::Instruction::Call { func, .. } if func.contains("Drop")))
        .count();
    assert!(
        drop_calls >= 1,
        "remaining owning field should still receive Drop glue after partial move, got {drop_calls}"
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
fn m1_trait_associated_function_trait_and_type_paths() {
    // D5: Trait::method and Type::method both resolve for receiver-less methods.
    typeck_ok(
        r#"
trait Math {
    def add(a: i64, b: i64) -> i64 {}
}

impl Math for i64 {
    def add(a: i64, b: i64) -> i64 { a + b }
}

def main() -> i64 {
    Math::add(1, 2) + i64::add(3, 4)
}
"#,
    );
}

#[test]
fn m1_trait_associated_function_uses_expected_result_type() {
    let source = r#"
trait Factory {
    def make(value: i64) -> Self {}
}

struct ProductA { value: i64 }
struct ProductB { value: i64 }

impl Factory for ProductA {
    def make(value: i64) -> ProductA { ProductA { value: value } }
}

impl Factory for ProductB {
    def make(value: i64) -> ProductB { ProductB { value: value } }
}

def main() -> i64 {
    let product: ProductA = Factory::make(42);
    product.value
}
"#;

    compile_to_mir(source).expect("expected result type should select ProductA::make");
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
