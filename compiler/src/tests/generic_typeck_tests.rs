use crate::{
    compile_to_ir, lower_ast, lower_hir_with_options, MirLowerOptions, Parser, TypeChecker,
};
use std::collections::HashSet;

#[test]
fn generic_function_can_be_instantiated_with_different_argument_types() {
    let source = r#"
def id<T>(x: T) -> T {
    x
}

def main() -> i64 {
    let a = id(1)
    let b = id("hello")
    a
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("generic calls with different concrete types should typecheck");
}

#[test]
fn filtered_typecheck_keeps_generic_signature_valid() {
    let source = r#"
def helper<T>(x: T) -> T {
    x
}

def main() -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    let checked = HashSet::from([String::from("main")]);
    checker
        .check_program_with_filtered_function_bodies(&program, &checked)
        .expect("signature-only typecheck should support generic params");
}

#[test]
fn generic_struct_type_annotation_with_explicit_args_typechecks() {
    let source = r#"
struct Box<T> {
    value: T,
}

def accept(x: Box<i64>) -> i64 {
    0
}

def main() -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("generic struct type argument should be checked");
}

#[test]
fn generic_struct_missing_required_args_is_rejected() {
    let source = r#"
struct Pair<T, U> {
    first: T,
    second: U,
}

def bad(x: Pair<i64>) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    let result = checker.check_program(&program);
    assert!(result.is_err(), "missing generic args should be rejected");
    let msg = format!("{}", result.unwrap_err());
    assert!(
        msg.contains("missing generic argument") || msg.contains("generic"),
        "error should mention generic argument issue, got: {}",
        msg
    );
}

#[test]
fn generic_struct_default_type_argument_is_applied() {
    let source = r#"
struct Pair<T, U = i64> {
    first: T,
    second: U,
}

def ok(x: Pair<bool>) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("default generic type argument should be applied");
}

#[test]
fn nested_generic_type_arguments_with_right_shift_tokens_typecheck() {
    let source = r#"
struct Box<T> {
    value: T,
}

struct Wrap<T> {
    value: T,
}

def f(x: Wrap<Box<i64>>) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("nested generic args should parse/typecheck even with >>");
}

#[test]
fn generic_struct_where_clause_is_supported() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

struct Box<T> where T: Showable {
    value: T,
}

impl Showable for i64 {
    def show(self) -> i64 {
        self
    }
}

def consume(x: Box<i64>) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("struct where-clause bounds should typecheck");
}

#[test]
fn generic_type_alias_where_clause_is_supported() {
    let source = r#"
trait Showable {
    def show(self) -> i64 {
        0
    }
}

type Alias<T> where T: Showable = T;

impl Showable for i64 {
    def show(self) -> i64 {
        self
    }
}

def consume(x: Alias<i64>) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("type alias where-clause bounds should typecheck");
}

#[test]
fn lazy_monomorphization_skips_uninstantiated_generic_function() {
    let source = r#"
def id<T>(x: T) -> T {
    x
}

def main() -> i64 {
    42
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("typecheck should succeed");
    let env = checker.into_env();
    let hir = lower_ast(&program, &env);

    let mir = lower_hir_with_options(
        &hir.items,
        MirLowerOptions::new(false, true, std::collections::HashSet::new()),
    )
    .expect("MIR lowering should succeed");

    let names = mir.iter().map(|f| f.name.as_str()).collect::<HashSet<_>>();
    assert!(names.contains("main"));
    assert!(
        !names.contains("id"),
        "unused generic function should be skipped in lazy mode"
    );
}

#[test]
fn lazy_monomorphization_keeps_instantiated_generic_function() {
    let source = r#"
def id<T>(x: T) -> T {
    x
}

def main() -> i64 {
    id(1)
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("typecheck should succeed");
    let env = checker.into_env();
    let hir = lower_ast(&program, &env);

    let mir = lower_hir_with_options(
        &hir.items,
        MirLowerOptions::new(false, true, std::collections::HashSet::new()),
    )
    .expect("MIR lowering should succeed");

    let names = mir.iter().map(|f| f.name.as_str()).collect::<HashSet<_>>();
    assert!(names.contains("main"));
    assert!(
        names.contains("id_i64"),
        "instantiated generic function should be materialized in lazy mode"
    );
}

#[test]
fn generic_impl_method_on_box_typechecks() {
    let source = r#"
struct Box<T> {
    value: T,
}

impl<T> Box<T> {
    def get(self) -> T {
        self.value
    }
}

def unwrap_box_i64(boxed: Box<i64>) -> i64 {
    boxed.get()
}

def main() -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("generic impl method should typecheck");
}

#[test]
fn generic_impl_method_on_box_lowers_to_ir() {
    let source = r#"
struct Box<T> {
    value: T,
}

impl<T> Box<T> {
    def get(self) -> T {
        self.value
    }
}

def unwrap_box_i64(boxed: Box<i64>) -> i64 {
    boxed.get()
}

def main() -> i64 {
    0
}
"#;

    let ir = compile_to_ir(source).expect("generic impl method should compile to IR");
    assert!(
        ir.contains("Box_i64_get"),
        "expected monomorphized method call in IR\n{}",
        ir
    );
}

#[test]
fn generic_function_can_return_struct_literal_parameterized_by_function_type() {
    let source = r#"
struct Box<T> {
    value: T,
}

def make_box<T>(value: T) -> Box<T> {
    Box { value: value }
}

def main() -> i64 {
    let boxed = make_box(true);
    if boxed.value {
        1
    } else {
        0
    }
}
"#;

    let ir = compile_to_ir(source)
        .expect("generic function should construct a generic struct literal from its parameter");
    assert!(
        ir.contains("; Function: make_box_bool"),
        "expected bool-specialized make_box\n{}",
        ir
    );
}

#[test]
fn generic_enum_payload_typechecks_and_lowers_to_ir() {
    let source = r#"
enum Maybe<T> { Empty, Value(T) }

def unwrap(value: Maybe<i64>) -> i64 {
    match value {
        Maybe::Empty => 0,
        Maybe::Value(inner) => inner,
    }
}

def main() -> i64 {
    unwrap(Maybe::Value(42))
}
"#;

    let ir = compile_to_ir(source).expect("generic enum payload should compile to IR");
    assert!(
        ir.contains("; Function: main"),
        "expected generic enum program to lower to IR\n{}",
        ir
    );
}
