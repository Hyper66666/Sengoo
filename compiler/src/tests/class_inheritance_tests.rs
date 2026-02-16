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
fn class_header_trait_list_is_rejected_with_migration_hint() {
    let source = r#"
class Animal {}
trait Runner {
    def run(self) -> i64 {
        1
    }
}
class Dog: Animal, Runner {}
def main() -> i64 { 0 }
"#;
    let result = compile_to_ir(source);
    assert!(
        result.is_err(),
        "trait list in class header should be rejected"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("impl Trait for Type") || err.contains("class header"),
        "error should include migration hint, got: {}",
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
