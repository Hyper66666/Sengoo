//! Unit tests for struct code generation
//!
//! Tests that the Sengoo compiler correctly generates LLVM IR for struct
//! type declarations, struct construction (`insertvalue`), and struct
//! field access (`extractvalue`).
//!
//! _Requirements: 4.1, 4.2, 4.3, 4.5_

use crate::compile_to_ir;

/// Test that `struct Point { x: i64, y: i64 }` with construction and field access
/// generates valid LLVM IR containing `insertvalue` and `extractvalue`.
///
/// This verifies the full struct codegen pipeline:
/// - Named struct type declaration in IR (Requirement 4.1)
/// - `insertvalue` instructions for struct construction (Requirement 4.2)
/// - `extractvalue` instruction for struct field access (Requirement 4.3)
///
/// _Requirements: 4.1, 4.2, 4.3_
#[test]
fn test_struct_point_construction_and_field_access_generates_valid_ir() {
    let source = r#"
struct Point { x: i64, y: i64 }
def main() -> i64 {
    let p = Point { x: 10, y: 20 };
    p.x
}
"#;
    let ir = compile_to_ir(source)
        .expect("struct Point construction + field access should compile successfully");

    assert!(
        ir.contains("insertvalue"),
        "Expected IR to contain 'insertvalue' for struct construction, got:\n{}",
        ir
    );
    assert!(
        ir.contains("extractvalue"),
        "Expected IR to contain 'extractvalue' for struct field access, got:\n{}",
        ir
    );
}

/// Test that constructing a struct with missing fields produces a compile error.
///
/// Requirement 4.5 states: IF a struct construction is missing required fields,
/// THEN THE TypeChecker SHALL emit an error listing the missing fields.
///
/// _Requirements: 4.5_
#[test]
fn test_struct_construction_missing_field_produces_error() {
    let source = r#"
struct Point { x: i64, y: i64 }
def main() -> i64 {
    let p = Point { x: 10 };
    p.x
}
"#;
    let err = compile_to_ir(source).expect_err("missing struct field should be rejected");
    assert!(
        err.to_string()
            .contains("invalid struct literal `Point`: missing fields: `y`"),
        "Expected missing field diagnostic for `y`, got:\n{}",
        err
    );
}

#[test]
fn test_struct_construction_duplicate_field_produces_error() {
    let source = r#"
struct Point { x: i64, y: i64 }
def main() -> i64 {
    let p = Point { x: 10, x: 20, y: 30 };
    p.x
}
"#;
    let err = compile_to_ir(source).expect_err("duplicate struct field should be rejected");
    assert!(
        err.to_string()
            .contains("invalid struct literal `Point`: duplicate fields: `x`"),
        "Expected duplicate field diagnostic for `x`, got:\n{}",
        err
    );
}

#[test]
fn test_struct_construction_unknown_field_produces_error() {
    let source = r#"
struct Point { x: i64, y: i64 }
def main() -> i64 {
    let p = Point { x: 10, y: 20, z: 30 };
    p.x
}
"#;
    let err = compile_to_ir(source).expect_err("unknown struct field should be rejected");
    assert!(
        err.to_string()
            .contains("invalid struct literal `Point`: unknown fields: `z`"),
        "Expected unknown field diagnostic for `z`, got:\n{}",
        err
    );
}

#[test]
fn test_struct_construction_reports_missing_duplicate_and_unknown_fields_together() {
    let source = r#"
struct Point { x: i64, y: i64, z: i64 }
def main() -> i64 {
    let p = Point { x: 10, x: 20, extra: 30 };
    p.x
}
"#;
    let err =
        compile_to_ir(source).expect_err("struct field completeness issues should be aggregated");
    let message = err.to_string();

    assert!(
        message.contains("invalid struct literal `Point`:"),
        "Expected aggregated struct literal diagnostic header, got:\n{}",
        message
    );
    assert!(
        message.contains("missing fields: `y`, `z`"),
        "Expected missing field list for `y` and `z`, got:\n{}",
        message
    );
    assert!(
        message.contains("duplicate fields: `x`"),
        "Expected duplicate field list for `x`, got:\n{}",
        message
    );
    assert!(
        message.contains("unknown fields: `extra`"),
        "Expected unknown field list for `extra`, got:\n{}",
        message
    );
}

#[test]
fn option_struct_literal_diagnostic_names_variant_constructors() {
    let source = r#"
enum Option<T> { None, Some(T) }
def main() -> i64 {
    let value: Option<i64> = Option { is_some: true, value: 1 };
    0
}
"#;
    let error = compile_to_ir(source).expect_err("Option struct literal should be rejected");
    let message = error.to_string();
    assert!(
        message.contains("Option") && message.contains("Some") && message.contains("None"),
        "Option diagnostic should name Some/None replacements:\n{message}"
    );
}

#[test]
fn result_struct_literal_diagnostic_names_variant_constructors() {
    let source = r#"
enum Result<T, E> { Ok(T), Err(E) }
def main() -> i64 {
    let value: Result<i64, i64> = Result { is_ok: true, value: 1, error: 0 };
    0
}
"#;
    let error = compile_to_ir(source).expect_err("Result struct literal should be rejected");
    let message = error.to_string();
    assert!(
        message.contains("Result") && message.contains("Ok") && message.contains("Err"),
        "Result diagnostic should name Ok/Err replacements:\n{message}"
    );
}
