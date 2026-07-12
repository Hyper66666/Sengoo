//! Unit tests for trait system MIR lowering and codegen
//!
//! Tests that the Sengoo compiler correctly resolves trait method calls
//! to the specific implementation function using three-part mangled names.
//!
//! _Requirements: 4.2, 4.3_

use crate::compile_to_ir;

/// Test that a trait impl method call on i64 resolves to the three-part mangled name.
///
/// When `show` is defined via `impl Printable for i64`, calling `x.show()` should
/// generate a call to `i64_Printable_show`.
///
/// _Requirements: 4.2, 4.3_
#[test]
fn test_trait_method_call_on_i64_resolves_to_three_part_name() {
    let source = r#"
trait Printable {
    def show(self) -> i64 {
        0
    }
}

impl Printable for i64 {
    def show(self) -> i64 {
        self
    }
}

def main() -> i64 {
    let x: i64 = 42;
    x.show()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "Trait method call should compile successfully, but got error: {}",
        result.unwrap_err()
    );
    let ir = result.unwrap();
    // The generated IR should contain the three-part mangled function name
    assert!(
        ir.contains("i64_Printable_show"),
        "IR should contain the three-part mangled name 'i64_Printable_show', got:\n{}",
        ir
    );
}

/// Test that a trait impl method produces a function definition with the three-part name.
///
/// _Requirements: 4.2, 4.4_
#[test]
fn test_trait_impl_produces_three_part_function_definition() {
    let source = r#"
trait Describable {
    def describe(self) -> i64 {
        0
    }
}

impl Describable for i64 {
    def describe(self) -> i64 {
        self + 1
    }
}

def main() -> i64 {
    let x: i64 = 10;
    x.describe()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "Trait impl should compile successfully, but got error: {}",
        result.unwrap_err()
    );
    let ir = result.unwrap();
    // Should have both a function definition and a call with the three-part name
    assert!(
        ir.contains("@i64_Describable_describe"),
        "IR should contain function definition '@i64_Describable_describe', got:\n{}",
        ir
    );
}

/// Test that inherent impl methods still work when trait impls are also present.
///
/// _Requirements: 3.2, 4.3_
#[test]
fn test_inherent_impl_preferred_over_trait_search() {
    let source = r#"
trait Showable {
    def display(self) -> i64 {
        0
    }
}

impl i64 {
    def double(self) -> i64 {
        self + self
    }
}

impl Showable for i64 {
    def display(self) -> i64 {
        self
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
        "Inherent method call should still work with trait impls present, but got error: {}",
        result.unwrap_err()
    );
    let ir = result.unwrap();
    // The inherent impl method should use two-part name, not three-part
    assert!(
        ir.contains("i64_double"),
        "IR should contain the two-part mangled name 'i64_double' for inherent impl, got:\n{}",
        ir
    );
}

/// Test that calling a method that doesn't exist in either inherent or trait impls
/// still produces an error.
///
/// _Requirements: 3.6_
#[test]
fn test_nonexistent_method_with_trait_impls_still_errors() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

impl Showable for i64 {
    def show(self) -> i64 {
        self
    }
}

def main() -> i64 {
    let x: i64 = 42;
    x.nonexistent()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "Calling a non-existent method should produce an error even with trait impls present, but got Ok:\n{}",
        result.unwrap_or_default()
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("nonexistent"),
        "Error message should mention the method name 'nonexistent', got: {}",
        err_msg
    );
}

#[test]
fn test_ambiguous_trait_method_call_reports_error() {
    let source = r#"
trait Printable {
    def show(self) -> i64 {
        1
    }
}

trait Debuggable {
    def show(self) -> i64 {
        2
    }
}

impl Printable for i64 {
}

impl Debuggable for i64 {
}

def main() -> i64 {
    let x: i64 = 42;
    x.show()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "ambiguous trait method call should error, but got Ok:\n{}",
        result.unwrap_or_default()
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("ambiguous"),
        "error should mention ambiguity, got: {}",
        err_msg
    );
    assert!(
        err_msg.contains("show"),
        "error should mention method name ''show'', got: {}",
        err_msg
    );
}

#[test]
fn test_trait_method_resolution_ignores_different_arity_candidates() {
    let source = r#"
trait ZeroArgShow {
    def show(self) -> i64 {
        1
    }
}

trait OneArgShow {
    def show(self, extra: i64) -> i64 {
        extra
    }
}

impl ZeroArgShow for i64 {
}

impl OneArgShow for i64 {
}

def main() -> i64 {
    let x: i64 = 42;
    x.show()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "trait method resolution should ignore different-arity candidates, but got error: {}",
        result.unwrap_err()
    );
    let ir = result.unwrap();
    assert!(
        ir.contains("i64_ZeroArgShow_show"),
        "IR should resolve to the zero-arg trait method, got:\n{}",
        ir
    );
}

#[test]
fn test_trait_method_wrong_arity_reports_argument_count_mismatch() {
    let source = r#"
trait OneArgShow {
    def show(self, extra: i64) -> i64 {
        extra
    }
}

impl OneArgShow for i64 {
}

def main() -> i64 {
    let x: i64 = 42;
    x.show()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "wrong-arity trait method call should error, but got Ok:\n{}",
        result.unwrap_or_default()
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("参数数量错误")
            || err.contains("ArgumentCountMismatch")
            || err.contains("期望 1 个, 找到 0 个"),
        "error should report argument count mismatch, got: {}",
        err
    );
}

/// Test that a default trait method is used when the impl doesn''t override it.
///
/// When a trait defines `def default_method(self) -> i64 { 42 }` and the impl
/// does not provide an override, the compiler should generate a function with
/// the three-part mangled name using the default body.
///
/// _Requirements: 4.5_
#[test]
fn test_default_trait_method_generates_function() {
    let source = r#"
trait HasDefault {
    def default_val(self) -> i64 {
        42
    }
}

impl HasDefault for i64 {
}

def main() -> i64 {
    let x: i64 = 10;
    x.default_val()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "Default trait method should compile successfully, but got error: {}",
        result.unwrap_err()
    );
    let ir = result.unwrap();
    // The generated IR should contain the three-part mangled function name
    // for the default method
    assert!(
        ir.contains("i64_HasDefault_default_val"),
        "IR should contain the three-part mangled name 'i64_HasDefault_default_val' for the default method, got:\n{}",
        ir
    );
}

#[test]
fn test_default_trait_method_with_method_generic_emits_specialized_function() {
    let source = r#"
struct Wrap<T> {
    value: T,
}

trait WrapValue {
    def wrap<T>(self, value: T) -> Wrap<T> {
        Wrap { value: value }
    }
}

impl WrapValue for i64 {
}

def main() -> i64 {
    let wrapped = 1.wrap(true);
    if wrapped.value {
        1
    } else {
        0
    }
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "generic default trait method should compile successfully, but got error: {}",
        result.unwrap_err()
    );
    let ir = result.unwrap();
    assert!(
        ir.contains("; Function: i64_WrapValue_wrap_bool"),
        "IR should contain specialized generic default trait method 'i64_WrapValue_wrap_bool', got:
{}",
        ir
    );
    assert!(
        !ir.contains("define %Wrap_i64 @i64_WrapValue_wrap("),
        "unspecialized trait generic method should not leak into IR, got:
{}",
        ir
    );
}

#[test]
fn test_generic_trait_args_are_part_of_impl_function_names() {
    let source = r#"
trait Convert<T> {
    def convert(self) -> T {}
}

impl Convert<i64> for i32 {
    def convert(self) -> i64 {
        self as i64
    }
}

impl Convert<u64> for i32 {
    def convert(self) -> u64 {
        self as u64
    }
}

def main() -> i64 {
    0
}
"#;

    let ir = compile_to_ir(source)
        .expect("generic trait impls with distinct args should lower to distinct functions");
    assert!(
        ir.contains("i32_Convert_i64_convert"),
        "IR should contain target-specific Convert<i64> impl, got:\n{}",
        ir
    );
    assert!(
        ir.contains("i32_Convert_u64_convert"),
        "IR should contain target-specific Convert<u64> impl, got:\n{}",
        ir
    );
    assert!(
        !ir.contains("i32_Convert_convert"),
        "IR should not emit the old trait-arg-erasing impl name, got:\n{}",
        ir
    );
}

/// Test that a default trait method is NOT generated when the impl overrides it.
///
/// When the impl provides its own implementation, the default should not be used.
///
/// _Requirements: 4.5_
#[test]
fn test_overridden_trait_method_uses_impl_not_default() {
    let source = r#"
trait HasDefault {
    def get_val(self) -> i64 {
        0
    }
}

impl HasDefault for i64 {
    def get_val(self) -> i64 {
        self + 100
    }
}

def main() -> i64 {
    let x: i64 = 5;
    x.get_val()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "Overridden trait method should compile successfully, but got error: {}",
        result.unwrap_err()
    );
    let ir = result.unwrap();
    // Should have the three-part mangled function name
    assert!(
        ir.contains("i64_HasDefault_get_val"),
        "IR should contain 'i64_HasDefault_get_val', got:\n{}",
        ir
    );
}

/// Test that a trait with multiple methods uses defaults for unimplemented ones
/// while using the impl for overridden ones.
///
/// _Requirements: 4.5_
#[test]
fn test_partial_impl_uses_defaults_for_missing_methods() {
    let source = r#"
trait MultiMethod {
    def method_a(self) -> i64 {
        1
    }
    def method_b(self) -> i64 {
        2
    }
}

impl MultiMethod for i64 {
    def method_a(self) -> i64 {
        self
    }
}

def main() -> i64 {
    let x: i64 = 10;
    x.method_b()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "Partial trait impl with default methods should compile successfully, but got error: {}",
        result.unwrap_err()
    );
    let ir = result.unwrap();
    // method_a should be from the impl (overridden)
    assert!(
        ir.contains("i64_MultiMethod_method_a"),
        "IR should contain 'i64_MultiMethod_method_a' from the impl, got:\n{}",
        ir
    );
    // method_b should be generated from the default
    assert!(
        ir.contains("i64_MultiMethod_method_b"),
        "IR should contain 'i64_MultiMethod_method_b' from the default, got:\n{}",
        ir
    );
}

/// Test that a trait with a required method (no default body) that is not
/// implemented produces an error listing the missing methods.
///
/// A required method is one with an empty body `{ }` in the trait definition.
/// When a type claims to implement the trait but doesn't provide the required
/// method, the TypeChecker should emit an error.
///
/// _Requirements: 4.6_
#[test]
fn test_missing_required_trait_method_produces_error() {
    let source = r#"
trait Describable {
    def describe(self) -> i64 {
    }
}

impl Describable for i64 {
}

def main() -> i64 {
    let x: i64 = 42;
    x.describe()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "Impl missing a required trait method should produce an error, but got Ok:\n{}",
        result.unwrap_or_default()
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("missing required trait methods"),
        "Error message should mention 'missing required trait methods', got: {}",
        err_msg
    );
    assert!(
        err_msg.contains("describe"),
        "Error message should list the missing method 'describe', got: {}",
        err_msg
    );
}

/// Test that a trait with multiple methods where some have defaults and some don't
/// only errors on the truly required (non-default) methods.
///
/// _Requirements: 4.6_
#[test]
fn test_missing_required_method_with_default_methods_present() {
    let source = r#"
trait MixedTrait {
    def required_method(self) -> i64 {
    }
    def default_method(self) -> i64 {
        42
    }
}

impl MixedTrait for i64 {
}

def main() -> i64 {
    let x: i64 = 10;
    x.default_method()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "Impl missing a required method should error even if default methods exist, but got Ok:\n{}",
        result.unwrap_or_default()
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("required_method"),
        "Error should list 'required_method' as missing, got: {}",
        err_msg
    );
    // The default_method should NOT be listed as missing
    assert!(
        !err_msg.contains("default_method"),
        "Error should NOT list 'default_method' as missing (it has a default), got: {}",
        err_msg
    );
}

/// Test that implementing all required methods succeeds even when some default
/// methods are not overridden.
///
/// _Requirements: 4.5, 4.6_
#[test]
fn test_implementing_required_methods_with_defaults_succeeds() {
    let source = r#"
trait MixedTrait {
    def required_method(self) -> i64 {
    }
    def default_method(self) -> i64 {
        42
    }
}

impl MixedTrait for i64 {
    def required_method(self) -> i64 {
        self + 1
    }
}

def main() -> i64 {
    let x: i64 = 10;
    x.required_method()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "Impl providing all required methods should compile successfully, but got error: {}",
        result.unwrap_err()
    );
}
