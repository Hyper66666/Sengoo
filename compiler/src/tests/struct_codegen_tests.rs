//! Unit tests for struct code generation
//!
//! Tests that the Sengoo compiler correctly generates LLVM IR for struct
//! type declarations, struct construction (`insertvalue`), and struct
//! field access (`extractvalue`).
//!
//! _Requirements: 4.1, 4.2, 4.3, 4.5_

use crate::compile_to_ir;
use std::io::Write;
use std::process::{Command, Stdio};

fn assert_clang_accepts_ir(ir: &str) {
    let output_path = if cfg!(windows) { "NUL" } else { "/dev/null" };
    let mut child = ["clang", "clang.exe"]
        .iter()
        .find_map(|candidate| {
            Command::new(candidate)
                .args(["-x", "ir", "-c", "-o", output_path, "-"])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
                .ok()
        })
        .expect("clang is required to validate generated LLVM IR");
    child
        .stdin
        .take()
        .expect("clang stdin should be piped")
        .write_all(ir.as_bytes())
        .expect("generated LLVM IR should be writable to clang");
    let output = child.wait_with_output().expect("clang should finish");
    assert!(
        output.status.success(),
        "clang rejected generated LLVM IR:\n{}\nIR:\n{}",
        String::from_utf8_lossy(&output.stderr),
        ir
    );
}

fn assert_native_main_succeeds(ir: &str, label: &str) {
    let executable = std::env::temp_dir().join(format!(
        "sengoo-receiver-{}-{}{}",
        std::process::id(),
        label,
        std::env::consts::EXE_SUFFIX,
    ));
    let mut child = Command::new("clang")
        .args(["-x", "ir", "-o"])
        .arg(&executable)
        .arg("-")
        .stdin(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("clang is required for receiver ABI regression tests");
    child.stdin.take().unwrap().write_all(ir.as_bytes()).unwrap();
    let result = child.wait_with_output().unwrap();
    assert!(result.status.success(), "{}\n{ir}", String::from_utf8_lossy(&result.stderr));
    let status = Command::new(&executable).status().unwrap();
    std::fs::remove_file(&executable).unwrap();
    assert!(status.success(), "mutable receiver changes did not reach their original storage: {status}\n{ir}");
}

#[test]
fn mutable_receiver_updates_local_nested_field_and_reborrowed_parameter() {
    let source = r#"
struct Counter { value: i64 }
struct Holder { inner: Counter }
impl Counter {
    def add(&mut self, amount: i64) { self.value = self.value + amount; (); }
    def twice(&mut self) { self.add(2); self.add(3); (); }
    def read(&self) -> i64 { return self.value; }
}
impl Holder {
    def update(&mut self) { self.inner.twice(); (); }
}
def update_ref(counter: &mut Counter) { counter.twice(); (); }
def main() -> i64 {
    let mut counter = Counter { value: 1 };
    counter.add(4);
    update_ref(&mut counter);
    if counter.read() != 10 { return 1; };
    let mut holder = Holder { inner: Counter { value: 7 } };
    holder.inner.add(2);
    holder.update();
    if holder.inner.read() != 14 { return 2; };
    0
}
"#;
    let ir = compile_to_ir(source).expect("mutable receiver program should compile");
    assert!(ir.contains("define void @Counter_add(%Counter*"), "{ir}");
    assert_native_main_succeeds(&ir, "nested");
}

#[test]
fn generic_mutable_receiver_retains_reference_abi_after_specialization() {
    let source = r#"
struct Cell<T> { value: T }
impl<T> Cell<T> {
    def replace(&mut self, value: T) { self.value = value; (); }
    def read(&self) -> T { self.value }
}
def main() -> i64 {
    let mut cell = Cell { value: 3 };
    cell.replace(42);
    if cell.read() != 42 { return 1; };
    0
}
"#;
    let ir = compile_to_ir(source).expect("generic mutable receiver should compile");
    assert_native_main_succeeds(&ir, "generic");
}

#[test]
fn empty_struct_values_and_receivers_preserve_aggregate_abi() {
    let source = r#"
struct Empty {}
struct Holder { empty: Empty, value: i64 }
impl Empty {
    def read(&self, value: &mut i64) -> i64 { *value }
    def update(&mut self, value: &mut i64) { *value = *value + 1; (); }
    def identity(self) -> Empty { self }
}
def create() -> Empty { Empty {} }
def consume(value: Empty) -> i64 { 7 }
def inspect(value: &Empty, target: &mut i64) -> i64 { value.read(target) }
def main() -> i64 {
    let mut reader = Empty {};
    let mut target = 3;
    let count = reader.read(&mut target);
    reader.update(&mut target);
    if count != 3 or target != 4 { return 1; };
    let returned = create();
    if consume(returned.identity()) != 7 { return 2; };
    let holder = Holder { empty: create(), value: 11 };
    if inspect(&holder.empty, &mut target) != 4 or holder.value != 11 { return 3; };
    0
}
"#;
    let ir = compile_to_ir(source).expect("zero-field values and receivers should compile");
    assert_native_main_succeeds(&ir, "empty-aggregate");
}

#[test]
fn mutable_destructor_keeps_automatic_drop_call_abi() {
    let source = r#"
struct Token { value: i64 }
extern "C" { fn abort(); }
impl Drop for Token {
    def drop(&mut self) { self.value = self.value + 1; if self.value != 8 { abort(); }; (); }
}
def main() -> i64 { let token = Token { value: 7 }; 0 }
"#;
    let ir = compile_to_ir(source).expect("Drop should retain its generated call ABI");
    assert!(ir.contains("define void @Token_Drop_drop(%Token %"), "{ir}");
    assert_native_main_succeeds(&ir, "drop");
}

#[test]
fn logical_operators_short_circuit_side_effects_and_fallible_rhs() {
    let source = r#"
struct Result<T, E> { is_ok: bool, value: T, error: E }
struct Counter { value: i64 }
impl Counter {
    def bump(&mut self) -> bool { self.value = self.value + 1; true }
}
def failure() -> Result<bool, i64> { Result { is_ok: false, value: false, error: 7 } }
def check() -> Result<i64, i64> {
    let mut counter = Counter { value: 0 };
    let a = false and counter.bump();
    let b = true or counter.bump();
    let c = false and failure()?;
    let d = true or failure()?;
    let e = true and counter.bump();
    let f = false or counter.bump();
    Result { is_ok: true, value: counter.value, error: 0 }
}
def main() -> i64 {
    let result = check();
    if not result.is_ok or result.value != 2 { return 1; };
    0
}
"#;
    let ir = compile_to_ir(source).expect("fallible short circuit expressions should lower");
    assert_native_main_succeeds(&ir, "short-circuit");
}

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

#[test]
fn local_value_reference_generates_valid_ir() {
    let source = r#"
struct Token { value: i64 }

def inspect(value: &Token) -> i64 { 0 }

def main() -> i64 {
    let token = Token { value: 7 };
    inspect(&token)
}
"#;
    let ir = compile_to_ir(source).expect("local value reference should lower to LLVM IR");
    assert_clang_accepts_ir(&ir);
}

#[test]
fn local_struct_field_reference_generates_valid_ir() {
    let source = r#"
struct Token { value: i64 }
struct Container { token: Token }

def inspect(value: &Token) -> i64 { 0 }

def main() -> i64 {
    let container = Container { token: Token { value: 7 } };
    inspect(&container.token)
}
"#;
    let ir = compile_to_ir(source).expect("struct field reference should lower to LLVM IR");
    assert_clang_accepts_ir(&ir);
}

#[test]
fn nested_owned_string_parameter_field_reference_generates_valid_ir() {
    let source = r#"
struct String { handle: i64 }
struct Identifiers { correlation_ref: String }
struct Plan { identifiers: Identifiers }

def inspect(value: &String) -> i64 { 0 }

def encode(plan: Plan) -> i64 {
    inspect(&plan.identifiers.correlation_ref)
}

def main() -> i64 {
    let plan = Plan {
        identifiers: Identifiers {
            correlation_ref: String { handle: 7 },
        },
    };
    encode(plan)
}
"#;
    let ir = compile_to_ir(source)
        .expect("nested owned String parameter field reference should lower to LLVM IR");
    assert_clang_accepts_ir(&ir);
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
