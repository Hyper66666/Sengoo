//! Unit tests for impl block method call error handling
//!
//! Tests that the Sengoo compiler produces descriptive errors when a method
//! call references a method that does not exist in any impl block.
//!
//! _Requirements: 3.6_

use crate::compile_to_ir;

/// Test that calling a non-existent method on i64 produces a descriptive error.
///
/// _Requirements: 3.6_
#[test]
fn test_missing_method_on_i64_produces_error() {
    let source = r#"
def main() -> i64 {
    let x: i64 = 42;
    x.nonexistent()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "Calling a non-existent method should produce a compile error, but got Ok:\n{}",
        result.unwrap_or_default()
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("has no method nonexistent") || err_msg.contains("undefined method"),
        "Error message should name the missing method in English, got: {}",
        err_msg
    );
}

/// Test that calling a non-existent method on bool produces a descriptive error.
///
/// _Requirements: 3.6_
#[test]
fn test_missing_method_on_bool_produces_error() {
    let source = r#"
def main() -> i64 {
    let b = true;
    b.missing_method();
    0
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "Calling a non-existent method on bool should produce a compile error, but got Ok:\n{}",
        result.unwrap_or_default()
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("has no method missing_method") || err_msg.contains("undefined method"),
        "Error message should name the missing method in English, got: {}",
        err_msg
    );
}

/// Test that calling an existing method still works correctly (no false positives).
///
/// _Requirements: 3.2, 3.5_
#[test]
fn test_existing_method_call_still_works() {
    let source = r#"
impl i64 {
    def double(self) -> i64 {
        self + self
    }
}

def main() -> i64 {
    let x: i64 = 21;
    x.double()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "Calling an existing method should compile successfully, but got error: {}",
        result.unwrap_err()
    );
    let ir = result.unwrap();
    assert!(
        ir.contains("i64_double"),
        "IR should contain the mangled method name 'i64_double', got:\n{}",
        ir
    );
}

/// Test that calling a method that exists in one impl block but not another
/// still works correctly.
///
/// _Requirements: 3.2, 3.6_
#[test]
fn test_method_from_different_impl_block_works() {
    let source = r#"
impl i64 {
    def abs(self) -> i64 {
        if self < 0 {
            -self
        } else {
            self
        }
    }
}

impl bool {
    def to_int(self) -> i64 {
        if self {
            1
        } else {
            0
        }
    }
}

def main() -> i64 {
    let x = -5;
    x.abs()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "Calling abs on i64 should compile successfully, but got error: {}",
        result.unwrap_err()
    );
}

/// Test i64 abs method end-to-end: impl block + method call produces correct IR.
///
/// _Requirements: 3.1, 3.2, 3.5_
#[test]
fn test_i64_abs_method_end_to_end() {
    let source = r#"
impl i64 {
    def abs(self) -> i64 {
        if self < 0 {
            -self
        } else {
            self
        }
    }
}

def main() -> i64 {
    let x: i64 = -42;
    x.abs()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "i64 abs method should compile successfully, but got error: {}",
        result.unwrap_err()
    );
    let ir = result.unwrap();
    assert!(
        ir.contains("i64_abs"),
        "IR should contain the mangled method name 'i64_abs', got:\n{}",
        ir
    );
}

/// Test struct method call: impl Point with sum method, called via p.sum().
///
/// _Requirements: 3.1, 3.2, 3.5_
#[test]
fn test_struct_method_call_point_sum() {
    let source = r#"
struct Point { x: i64, y: i64 }

impl Point {
    def sum(self) -> i64 {
        self.x + self.y
    }
}

def main() -> i64 {
    let p = Point { x: 3, y: 4 };
    p.sum()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "Struct method call should compile successfully, but got error: {}",
        result.unwrap_err()
    );
    let ir = result.unwrap();
    assert!(
        ir.contains("Point_sum"),
        "IR should contain the mangled method name 'Point_sum', got:\n{}",
        ir
    );
}
