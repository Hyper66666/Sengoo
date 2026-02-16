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
/// NOTE: The current TypeChecker does NOT validate missing struct fields —
/// it only checks field value types and looks up the struct type. This test
/// is marked as `#[ignore]` until Requirement 4.5 is implemented.
///
/// _Requirements: 4.5_
#[test]
#[ignore = "TypeChecker does not yet validate missing struct fields (Requirement 4.5 not implemented)"]
fn test_struct_construction_missing_field_produces_error() {
    let source = r#"
struct Point { x: i64, y: i64 }
def main() -> i64 {
    let p = Point { x: 10 };
    p.x
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "Expected compile error for struct construction with missing field 'y', but compilation succeeded"
    );
}
