//! Unit tests for print functionality.
//!
//! _Requirements: 7.1, 7.2, 7.3, 7.4_

use crate::compile_to_ir;

#[test]
fn test_print_extended_numeric_literals_compile() {
    let source = r#"def main() -> i64 { print(0b101010 + 0o52 + 42i64 + 1_000_000); 0 }"#;
    let ir = compile_to_ir(source).expect("extended numeric literals should compile successfully");
    assert!(
        ir.contains("call void @sengoo_print_i64(i64"),
        "Expected IR to contain i64 print call for extended numeric literals, got:\n{}",
        ir
    );
}

#[test]
fn test_println_aliases_print_lowering() {
    let source = r#"def main() -> i64 { println("hello"); println(42); 0 }"#;
    let ir = compile_to_ir(source).expect("println should compile through the print runtime path");
    assert!(
        ir.contains("@sengoo_print_str("),
        "Expected println(str) to lower to string print runtime call, got:\n{}",
        ir
    );
    assert!(
        ir.contains("@sengoo_print_i64("),
        "Expected println(i64) to lower to i64 print runtime call, got:\n{}",
        ir
    );
}

#[test]
fn test_eprintln_lowers_to_stderr_runtime_calls() {
    let source = r#"def main() -> i64 { eprintln("oops"); eprintln(42); 0 }"#;
    let ir = compile_to_ir(source).expect("eprintln should compile through stderr runtime calls");
    assert!(
        ir.contains("@sengoo_eprint_str("),
        "Expected eprintln(str) to lower to stderr string runtime call, got:\n{}",
        ir
    );
    assert!(
        ir.contains("@sengoo_eprint_i64("),
        "Expected eprintln(i64) to lower to stderr i64 runtime call, got:\n{}",
        ir
    );
    assert!(
        ir.contains("declare void @sengoo_eprint_str(i8*)")
            && ir.contains("declare void @sengoo_eprint_i64(i64)"),
        "Expected stderr runtime declarations to be emitted on demand, got:\n{}",
        ir
    );
}

#[test]
fn test_print_i64_generates_correct_runtime_call() {
    let source = r#"def main() -> i64 { print(42); 0 }"#;
    let ir = compile_to_ir(source).expect("print(42) should compile successfully");
    assert!(
        ir.contains("call void @sengoo_print_i64(i64"),
        "Expected IR to contain 'call void @sengoo_print_i64(i64', got:\n{}",
        ir
    );
}

#[test]
fn test_print_bool_generates_correct_runtime_call() {
    let source = r#"def main() -> i64 { print(true); 0 }"#;
    let ir = compile_to_ir(source).expect("print(true) should compile successfully");
    assert!(
        ir.contains("@sengoo_print_bool("),
        "Expected IR to contain '@sengoo_print_bool(', got:\n{}",
        ir
    );
}

#[test]
fn test_print_str_generates_correct_runtime_call() {
    let source = r#"def main() -> i64 { print("hello"); 0 }"#;
    let ir = compile_to_ir(source).expect(r#"print("hello") should compile successfully"#);
    assert!(
        ir.contains("@sengoo_print_str("),
        "Expected IR to contain '@sengoo_print_str(', got:\n{}",
        ir
    );
}

#[test]
fn test_print_struct_generates_field_level_runtime_calls() {
    let source = r#"
struct Point { x: i64, y: i64 }
def main() -> i64 {
    let p = Point { x: 1, y: 2 };
    print(p);
    0
}
"#;
    let ir = compile_to_ir(source).expect("print(struct) should compile successfully");

    // `Point { x: 1, y: 2 }` should become string + per-field print calls.
    assert!(
        ir.contains("@sengoo_print_str("),
        "Expected IR to contain string print calls for struct formatting, got:\n{}",
        ir
    );
    assert!(
        ir.matches("@sengoo_print_i64(").count() >= 2,
        "Expected at least two i64 print calls for struct fields, got:\n{}",
        ir
    );
}

#[test]
fn test_print_nested_struct_generates_recursive_runtime_calls() {
    let source = r#"
struct Point { x: i64, y: i64 }
struct Wrapper { p: Point, id: i64 }
def main() -> i64 {
    let w = Wrapper { p: Point { x: 1, y: 2 }, id: 7 };
    print(w);
    0
}
"#;
    let ir = compile_to_ir(source).expect("print(nested struct) should compile successfully");

    let str_calls = ir.matches("@sengoo_print_str(").count();
    let int_calls = ir.matches("@sengoo_print_i64(").count();
    assert!(
        str_calls >= 4,
        "Expected multiple string print calls for nested struct formatting, got {}, IR:\n{}",
        str_calls,
        ir
    );
    assert!(
        int_calls >= 3,
        "Expected nested struct fields to produce recursive i64 print calls, got {}, IR:\n{}",
        int_calls,
        ir
    );
}

#[test]
fn test_print_struct_with_unsupported_field_type_produces_descriptive_error() {
    let source = r#"
struct Weird { values: [i64; 2] }
def main() -> i64 {
    let w = Weird { values: [1, 2] };
    print(w);
    0
}
"#;

    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "print(struct with unsupported field) should fail type check"
    );
    let err_msg = format!("{}", result.unwrap_err());
    assert!(
        err_msg.contains("print does not support field") && err_msg.contains("values"),
        "Expected descriptive field-level print error, got: {}",
        err_msg
    );
}
