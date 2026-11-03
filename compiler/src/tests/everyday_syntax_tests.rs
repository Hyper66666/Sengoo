use crate::compile_to_ir;
use crate::parser::Parser;

fn load_stdlib(modules: &[&str]) -> String {
    let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let stdlib_root = manifest_dir
        .parent()
        .unwrap_or(manifest_dir)
        .join("tools")
        .join("stdlib");
    modules
        .iter()
        .map(|module| {
            std::fs::read_to_string(stdlib_root.join(module))
                .unwrap_or_else(|err| panic!("failed to read {module}: {err}"))
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn compile_with(modules: &[&str], program: &str) -> Result<String, String> {
    compile_to_ir(&format!("{}\n\n{}", load_stdlib(modules), program))
        .map_err(|err| err.to_string())
}

const COLLECTIONS: &[&str] = &[
    "option.sg",
    "result.sg",
    "ffi.sg",
    "string.sg",
    "collections.sg",
];

const STRINGS: &[&str] = &["option.sg", "result.sg", "ffi.sg", "string.sg"];

#[test]
fn vec_macro_builds_from_elements() {
    let ir = compile_with(
        COLLECTIONS,
        r#"
def main() -> i64 {
    let values = vec![1, 2, 3];
    values.len()
}
"#,
    )
    .expect("vec![1, 2, 3] should compile");
    assert!(
        ir.contains("vec_new") || ir.contains("push"),
        "expected vec! to lower through vec_new/push, got:\n{ir}"
    );
}

#[test]
fn vec_macro_repeat_form_compiles() {
    compile_with(
        COLLECTIONS,
        r#"
def main() -> i64 {
    let values = vec![7; 3];
    values.len()
}
"#,
    )
    .expect("vec![value; count] should compile");
}

#[test]
fn for_loop_over_vec_macro_compiles() {
    compile_with(
        COLLECTIONS,
        r#"
def main() -> i64 {
    let mut total = 0;
    for value in vec![1, 2, 3] {
        total = total + value;
    }
    total
}
"#,
    )
    .expect("for over vec! should compile");
}

#[test]
fn unknown_bang_form_is_rejected() {
    let err = Parser::parse("def main() -> i64 { foo![1]; 0 }")
        .expect_err("unknown bang form should be rejected");
    let msg = err.to_string();
    assert!(
        msg.contains("vec!") && msg.contains("foo"),
        "expected pinned vec! diagnostic, got: {msg}"
    );
}

#[test]
fn println_accepts_format_string_and_arguments() {
    let ir = compile_with(
        STRINGS,
        r#"
def main() -> i64 {
    println("x={}", 7);
    eprintln("y={}", 8);
    print("z={}", 9);
    0
}
"#,
    )
    .expect("print family format forms should compile");
    assert!(
        ir.contains("@sengoo_string_new")
            && (ir.contains("@sengoo_print_string") || ir.contains("@sengoo_eprint_string")),
        "expected format-aware print to build a string then print it, got:\n{ir}"
    );
}

#[test]
fn debug_placeholder_with_derive_compiles() {
    compile_with(
        STRINGS,
        r#"
#[derive(Debug)]
struct Point {
    x: i64,
}

def main() -> i64 {
    let point = Point { x: 1 };
    let rendered = format("{:?}", point);
    println("{:?}", point);
    let interpolated = f"{point:?}";
    rendered.len() + interpolated.len()
}
"#,
    )
    .expect("{:?} with Debug should compile through format, println, and f-strings");
}

#[test]
fn debug_placeholder_without_derive_is_rejected() {
    let err = compile_with(
        STRINGS,
        r#"
struct Point {
    x: i64,
}

def main() -> i64 {
    let point = Point { x: 1 };
    let rendered = format("{:?}", point);
    rendered.len()
}
"#,
    )
    .expect_err("{:?} without Debug should fail");
    assert!(
        err.contains("missing-debug-derive") || err.contains("#[derive(Debug)]"),
        "expected missing Debug derive diagnostic, got: {err}"
    );
}

#[test]
fn if_let_binds_option_payload() {
    let ir = compile_to_ir(
        r#"
enum Option<T> { None, Some(T) }

def main() -> i64 {
    let x = Some(3);
    if let Some(v) = x {
        v
    } else {
        0
    }
}
"#,
    )
    .expect("if let Some(v) should compile");
    assert!(
        ir.contains("icmp") || ir.contains("Some"),
        "expected if-let to lower through match, got:\n{ir}"
    );
}

#[test]
fn if_let_irrefutable_pattern_is_rejected() {
    let err = compile_to_ir(
        r#"
def main() -> i64 {
    if let v = 1 {
        v
    } else {
        0
    }
}
"#,
    )
    .expect_err("irrefutable if-let should fail");
    let msg = err.to_string();
    assert!(
        msg.contains("irrefutable") || msg.contains("irrefutable-if-let"),
        "expected irrefutable if-let diagnostic, got: {msg}"
    );
}
