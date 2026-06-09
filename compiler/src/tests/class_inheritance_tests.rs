//! Tests for class inheritance syntax and V1 semantics.

use crate::compile_to_ir;

#[test]
fn class_single_parent_header_compiles() {
    let source = r#"
class Animal {
    age: i64;
}

class Dog: Animal {
    weight: i64;
}

def main() -> i64 {
    let d = Dog { age: 2, weight: 8 };
    d.age + d.weight
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "single-parent class header should compile, got error: {:?}",
        result.err()
    );
}

#[test]
fn class_header_base_and_traits_compile() {
    let source = r#"
class Animal {
    age: i64;
}

trait Runner {
    def run(self) -> i64 {
        1
    }
}

class Dog: Animal, Runner {}

def main() -> i64 {
    let d = Dog { age: 3 };
    d.age + d.run()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "class header with base and traits should compile, got {:?}",
        result.err()
    );
}

#[test]
fn trait_only_class_header_compiles() {
    let source = r#"
trait Service {
    def ping(self) -> i64 {
        42
    }
}

class Worker: Service {}

def main() -> i64 {
    let w = Worker {};
    w.ping()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "trait-only class header should compile, got {:?}",
        result.err()
    );
}

#[test]
fn class_after_trait_in_header_is_rejected() {
    let source = r#"
class Animal {}
trait Runner {
    def run(self) -> i64 { 1 }
}
class Dog: Runner, Animal {}
def main() -> i64 { 0 }
"#;
    let result = compile_to_ir(source);
    assert!(result.is_err(), "class path after trait should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("class header") || err.contains("class base"),
        "error should mention invalid header ordering, got: {}",
        err
    );
}

#[test]
fn second_class_base_in_header_is_rejected() {
    let source = r#"
class A {}
class B {}
class C: A, B {}
def main() -> i64 { 0 }
"#;
    let result = compile_to_ir(source);
    assert!(result.is_err(), "second class base should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("class header") || err.contains("one class base"),
        "error should mention duplicate class base, got: {}",
        err
    );
}

#[test]
fn missing_parent_class_is_rejected() {
    let source = r#"
class Dog: MissingParent {}
def main() -> i64 { 0 }
"#;
    let result = compile_to_ir(source);
    assert!(result.is_err(), "missing parent class should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("MissingParent") && (err.contains("parent") || err.contains("class")),
        "error should mention missing parent class, got: {}",
        err
    );
}

#[test]
fn inheritance_cycle_is_rejected() {
    let source = r#"
class A: B {}
class B: A {}
def main() -> i64 { 0 }
"#;
    let result = compile_to_ir(source);
    assert!(result.is_err(), "inheritance cycle should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("cycle") || err.contains("cyclic"),
        "error should mention cycle, got: {}",
        err
    );
}

#[test]
fn duplicate_inherited_field_name_is_rejected() {
    let source = r#"
class Parent {
    x: i64;
}

class Child: Parent {
    x: i64;
}

def main() -> i64 { 0 }
"#;
    let result = compile_to_ir(source);
    assert!(result.is_err(), "duplicate inherited field should fail");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("duplicate") && err.contains("x"),
        "error should mention duplicate field x, got: {}",
        err
    );
}

#[test]
fn child_method_overrides_parent_method() {
    let source = r#"
class Parent {
    def score(self) -> i64 {
        1
    }
}

class Child: Parent {
    def score(self) -> i64 {
        2
    }
}

def main() -> i64 {
    let c = Child {};
    c.score()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "child override method should compile, got error: {:?}",
        result.err()
    );
    let ir = result.unwrap();
    assert!(
        ir.contains("Child_score"),
        "IR should resolve call to child method symbol, got:\n{}",
        ir
    );
}

#[test]
fn child_inherits_parent_method_when_not_overridden() {
    let source = r#"
class Parent {
    def score(self) -> i64 {
        7
    }
}

class Child: Parent {}

def main() -> i64 {
    let c = Child {};
    c.score()
}
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_ok(),
        "child should inherit parent method, got error: {:?}",
        result.err()
    );
    let ir = result.unwrap();
    assert!(
        ir.contains("Child_score") || ir.contains("Parent_score"),
        "IR should contain inherited method symbol, got:\n{}",
        ir
    );
}
