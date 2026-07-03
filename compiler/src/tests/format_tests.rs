//! Tests for the `format(template, args...)` mini-language (G2b core).
//!
//! `format` parses a string-literal template at compile time, validates that
//! its `{}` / `{:?}` placeholders match the argument count, and lowers to
//! owned-`String` runtime building.

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
fn format_builds_string_from_scalar_arguments() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let rendered = format("x={} ok={}", 7, true);
    rendered.len()
}
"#,
    )
    .expect("format with scalar args should compile");

    assert!(
        ir.contains("@sengoo_string_new"),
        "expected a fresh owned String to be created, got:\n{ir}"
    );
    assert!(
        ir.contains("@sengoo_string_push_i64_status"),
        "expected the i64 argument to be rendered, got:\n{ir}"
    );
    assert!(
        ir.contains("@sengoo_string_push_bool_status"),
        "expected the bool argument to be rendered, got:\n{ir}"
    );
    assert!(
        ir.contains("@sengoo_string_push_str_status"),
        "expected literal chunks to be appended, got:\n{ir}"
    );
}

#[test]
fn format_renders_str_arguments() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let rendered = format("hi {}", "there");
    rendered.len()
}
"#,
    )
    .expect("format with a &str arg should compile");

    assert!(
        ir.contains("@sengoo_stdlib_str_ptr"),
        "expected the &str argument to be converted to a pointer, got:\n{ir}"
    );
    assert!(
        ir.contains("@sengoo_string_push_str_status"),
        "expected the &str argument to be appended, got:\n{ir}"
    );
}

#[test]
fn format_dispatches_display_arguments_through_to_string() {
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
    let t = Tag { id: 1 };
    let rendered = format("tag={}", t);
    rendered.len()
}
"#,
    )
    .expect("format with a Display arg should compile");

    assert!(
        ir.contains("@Tag_Display_to_string"),
        "expected the Display arg to render through its impl, got:\n{ir}"
    );
    assert!(
        ir.contains("@sengoo_string_as_str_ptr"),
        "expected the rendered String text to be appended, got:\n{ir}"
    );
}

#[test]
fn format_brace_escapes_emit_literal_braces() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let rendered = format("{{{}}}", 9);
    rendered.len()
}
"#,
    )
    .expect("format with brace escapes should compile");

    assert!(
        ir.contains("@sengoo_string_push_i64_status"),
        "expected the single placeholder to render its arg, got:\n{ir}"
    );
}

#[test]
fn format_rejects_argument_count_mismatch() {
    let err = compile_failure(
        r#"
def main() -> i64 {
    let rendered = format("{} and {}", 1);
    rendered.len()
}
"#,
    );
    assert!(
        err.contains("format-argument-count"),
        "expected a stable arity diagnostic, got: {err}"
    );
}

#[test]
fn format_rejects_bad_template_with_stable_code() {
    let err = compile_failure(
        r#"
def main() -> i64 {
    let rendered = format("{", 1);
    rendered.len()
}
"#,
    );
    assert!(
        err.contains("invalid-format-template"),
        "expected a stable invalid-template diagnostic, got: {err}"
    );
}

#[test]
fn format_debug_placeholder_renders_scalar_arguments() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let rendered = format("value={:?}", 1);
    rendered.len()
}
"#,
    )
    .expect("format with a Debug scalar placeholder should compile");

    assert!(
        ir.contains("@sengoo_string_push_i64_status"),
        "expected the debug placeholder to render its i64 arg, got:\n{ir}"
    );
}

#[test]
fn format_debug_placeholder_renders_struct_fields() {
    let ir = compile_with_stdlib(
        r#"
struct Point {
    x: i64,
    ok: bool,
}

def main() -> i64 {
    let point = Point { x: 7, ok: true };
    let rendered = format("{:?}", point);
    rendered.len()
}
"#,
    )
    .expect("format with a Debug struct placeholder should compile");

    assert!(
        ir.contains("@sengoo_string_push_i64_status")
            && ir.contains("@sengoo_string_push_bool_status"),
        "expected Debug struct formatting to render scalar fields, got:\n{ir}"
    );
}

#[test]
fn format_debug_placeholder_dispatches_custom_debug_impl() {
    let ir = compile_with_stdlib(
        r#"
struct Tag {
    id: i64,
}

impl Debug for Tag {
    def to_string(&self) -> String {
        string_from_str("Tag(debug)").value
    }
}

def main() -> i64 {
    let tag = Tag { id: 7 };
    let rendered = format("{:?}", tag);
    rendered.len()
}
"#,
    )
    .expect("format with a custom Debug impl should compile");

    assert!(
        ir.contains("@Tag_Debug_to_string"),
        "expected format({{:?}}) to call the user Debug impl, got:\n{ir}"
    );
    assert!(
        !ir.contains("Tag { "),
        "custom Debug must take precedence over structural fallback, got:\n{ir}"
    );
}

#[test]
fn debug_impl_without_to_string_is_rejected() {
    let err = compile_failure(
        r#"
struct Bare {
    id: i64,
}

impl Debug for Bare {
}

def main() -> i64 {
    0
}
"#,
    );

    assert!(
        err.contains("debug-contract"),
        "expected the Debug contract diagnostic, got: {err}"
    );
}

#[test]
fn debug_impl_with_wrong_to_string_return_is_rejected() {
    let err = compile_failure(
        r#"
struct Bad {
    id: i64,
}

impl Debug for Bad {
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
        err.contains("debug-contract"),
        "expected the Debug contract diagnostic, got: {err}"
    );
}

#[test]
fn derive_debug_enum_renders_variant_name() {
    let ir = compile_with_stdlib(
        r#"
#[derive(Debug)]
enum Color {
    Red,
    Blue,
}

def main() -> i64 {
    let color = Color::Blue;
    let rendered = format("{:?}", color);
    rendered.len()
}
"#,
    )
    .expect("derived Debug enum should compile");

    assert!(
        ir.contains("Color::Blue"),
        "expected derived Debug enum formatting to append the variant name, got:\n{ir}"
    );
}

#[test]
fn derive_debug_enum_renders_tuple_payload() {
    let ir = compile_with_stdlib(
        r#"
#[derive(Debug)]
enum Maybe {
    Empty,
    Value(i64),
}

def main() -> i64 {
    let value = Maybe::Value(42);
    let rendered = format("{:?}", value);
    rendered.len()
}
"#,
    )
    .expect("derived Debug payload enum should compile");

    assert!(
        ir.contains("Maybe::Value")
            && ir.contains("c\"(\\00\"")
            && ir.contains("@sengoo_string_push_i64_status"),
        "expected derived Debug enum formatting to append payload fields, got:\n{ir}"
    );
}

#[test]
fn format_debug_placeholder_dispatches_custom_enum_debug_impl() {
    let ir = compile_with_stdlib(
        r#"
enum Status {
    Ready,
    Done(i64),
}

impl Debug for Status {
    def to_string(&self) -> String {
        string_from_str("Status(debug)").value
    }
}

def main() -> i64 {
    let status = Status::Done(5);
    let rendered = format("{:?}", status);
    rendered.len()
}
"#,
    )
    .expect("format with a custom enum Debug impl should compile");

    assert!(
        ir.contains("@Status_Debug_to_string"),
        "expected format({{:?}}) on an enum to call the user Debug impl, got:\n{ir}"
    );
    assert!(
        !ir.contains("Status::Done"),
        "custom enum Debug must take precedence over structural fallback, got:\n{ir}"
    );
}

#[test]
fn format_positional_placeholders_select_arguments_by_index() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let rendered = format("{1}:{0}", 7, 42);
    rendered.len()
}
"#,
    )
    .expect("format with positional placeholders should compile");

    assert!(
        ir.contains("@sengoo_string_push_i64_status"),
        "expected positional placeholders to render i64 args, got:\n{ir}"
    );
}

#[test]
fn format_width_placeholder_renders_right_aligned_padding() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let rendered = format("{:>8}", 1);
    rendered.len()
}
"#,
    )
    .expect("format with right-aligned width should compile");

    assert!(
        ir.contains("@sengoo_string_push_padded_string_status"),
        "expected width formatting to route through the padded append helper, got:\n{ir}"
    );
}

#[test]
fn format_precision_placeholder_renders_f64_argument() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let rendered = format("{:.2}", 3.14159);
    rendered.len()
}
"#,
    )
    .expect("format with f64 precision should compile");

    assert!(
        ir.contains("@sengoo_string_push_f64_precision_status"),
        "expected precision formatting to route through the f64 precision helper, got:\n{ir}"
    );
}

#[test]
fn fstring_lowers_to_format_runtime_building() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let n = 3;
    let rendered = f"count={n}";
    rendered.len()
}
"#,
    )
    .expect("f-string should compile through the format lowering");

    assert!(
        ir.contains("@sengoo_string_new"),
        "expected the f-string to build an owned String, got:\n{ir}"
    );
    assert!(
        ir.contains("@sengoo_string_push_i64_status"),
        "expected the interpolated i64 to be rendered, got:\n{ir}"
    );
    assert!(
        ir.contains("@sengoo_string_push_str_status"),
        "expected the literal prefix to be appended, got:\n{ir}"
    );
}

#[test]
fn fstring_supports_compound_expressions() {
    let ir = compile_with_stdlib(
        r#"
def main() -> i64 {
    let a = 2;
    let b = 5;
    let rendered = f"sum={a + b}";
    rendered.len()
}
"#,
    )
    .expect("f-string with a compound expression should compile");

    assert!(
        ir.contains("@sengoo_string_push_i64_status"),
        "expected the interpolated sum to be rendered, got:\n{ir}"
    );
}

#[test]
fn format_rejects_non_literal_template() {
    let err = compile_failure(
        r#"
def main() -> i64 {
    let template = "x={}";
    let rendered = format(template, 1);
    rendered.len()
}
"#,
    );
    assert!(
        err.contains("invalid-format-template"),
        "expected a stable literal-template diagnostic, got: {err}"
    );
}
