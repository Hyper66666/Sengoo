use crate::ast::{DeclKind, TraitItem, TypeKind};
use crate::{
    compile_to_ir, lower_ast, lower_hir_with_options, MirLowerOptions, Parser, TypeChecker,
};
use std::collections::HashSet;

#[test]
fn dyn_trait_type_syntax_parses_trait_bounds() {
    let source = r#"
trait Show {}

def takes(x: dyn Show) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should accept dyn Trait syntax");
    let function = program
        .decls
        .iter()
        .find_map(|decl| match &decl.kind {
            DeclKind::Function(function) if function.name.name == "takes" => Some(function),
            _ => None,
        })
        .expect("expected takes function");

    let TypeKind::Dyn(bounds) = &function.params[0].ty.kind else {
        panic!(
            "expected dyn trait parameter, got {:?}",
            function.params[0].ty.kind
        );
    };

    let names = bounds
        .iter()
        .map(|bound| {
            bound
                .path
                .as_simple()
                .expect("dyn bound should be a simple trait path")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["Show"]);
}

#[test]
fn trait_associated_type_declaration_parses_without_a_rhs() {
    let source = r#"
trait Iterator {
    type Item;
}
"#;

    let program = Parser::parse(source).expect("trait associated type declaration should parse");
    let trait_decl = program
        .decls
        .iter()
        .find_map(|decl| match &decl.kind {
            DeclKind::Trait(trait_decl) => Some(trait_decl),
            _ => None,
        })
        .expect("expected trait declaration");

    assert!(matches!(
        trait_decl.items.as_slice(),
        [TraitItem::Type(item)] if item.name.name == "Item"
    ));
}

#[test]
fn trait_method_self_associated_projection_parses() {
    let source = r#"
trait Iterator {
    type Item;
    def next(&self) -> Option<Self::Item> {}
}
"#;

    Parser::parse(source).expect("Self::Item should parse in a trait method signature");
}

#[test]
fn impl_associated_type_definition_typechecks() {
    let source = r#"
trait Iterator {
    type Item;
}

struct Counter {
    value: i64,
}

impl Iterator for Counter {
    type Item = i64;
}
"#;

    let program = Parser::parse(source).expect("impl associated type definition should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("impl associated type definition should be registered");
}

#[test]
fn impl_missing_required_associated_type_is_rejected() {
    let source = r#"
trait Iterator {
    type Item;
}

struct Counter {
    value: i64,
}

impl Iterator for Counter {}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("missing associated type should be rejected");
    assert!(
        err.to_string()
            .contains("missing required associated types: Item"),
        "expected missing associated type diagnostic, got: {err}"
    );
}

#[test]
fn impl_unknown_associated_type_is_rejected() {
    let source = r#"
trait Iterator {
    type Item;
}

struct Counter {
    value: i64,
}

impl Iterator for Counter {
    type Item = i64;
    type Output = i64;
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("unknown associated type should be rejected");
    assert!(
        err.to_string()
            .contains("defines unknown associated types: Output"),
        "expected unknown associated type diagnostic, got: {err}"
    );
}

#[test]
fn generic_associated_type_projection_resolves_at_call_site() {
    let source = r#"
trait Iterator {
    type Item;
}

struct Counter {
    value: i64,
}

impl Iterator for Counter {
    type Item = i64;
}

def select_item<T: Iterator>(owner: T, value: T::Item) -> T::Item {
    value
}

def main() -> i64 {
    select_item(Counter { value: 0 }, 7)
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("T::Item should resolve through the concrete trait impl");
    compile_to_ir(source).expect("resolved associated type projection should lower to LLVM IR");
}

#[test]
fn unbounded_associated_type_projection_is_rejected() {
    let source = r#"
def bad<T>(value: T::Item) -> T::Item {
    value
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("unbounded associated projection should be rejected");
    assert!(
        err.to_string()
            .contains("associated type `Item` is not declared by a bound on `T`"),
        "expected bounded projection diagnostic, got: {err}"
    );
}

#[test]
fn dyn_trait_type_syntax_parses_multiple_bounds() {
    let source = r#"
trait Read {}
trait Write {}

def stream(x: dyn Read + Write) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should accept dyn A + B syntax");
    let function = program
        .decls
        .iter()
        .find_map(|decl| match &decl.kind {
            DeclKind::Function(function) if function.name.name == "stream" => Some(function),
            _ => None,
        })
        .expect("expected stream function");

    let TypeKind::Dyn(bounds) = &function.params[0].ty.kind else {
        panic!(
            "expected dyn trait parameter, got {:?}",
            function.params[0].ty.kind
        );
    };

    let names = bounds
        .iter()
        .map(|bound| {
            bound
                .path
                .as_simple()
                .expect("dyn bound should be a simple trait path")
                .name
                .as_str()
        })
        .collect::<Vec<_>>();
    assert_eq!(names, vec!["Read", "Write"]);
}

#[test]
fn dyn_trait_type_requires_declared_trait() {
    let source = r#"
def bad(x: dyn Missing) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("source should parse before type checking");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("undefined dyn trait should be rejected");
    let message = err.to_string();
    assert!(
        message.contains("[undefined-dyn-trait]") && message.contains("Missing"),
        "expected undefined-dyn-trait diagnostic, got: {}",
        message
    );
}

#[test]
fn dyn_trait_typechecks_when_trait_is_declared() {
    let source = r#"
trait Show {}

def takes(x: dyn Show) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("declared dyn trait should typecheck");
}

#[test]
fn dyn_trait_with_associated_type_requires_fixed_binding() {
    let source = r#"
trait Iterator {
    type Item;
}

def takes(x: dyn Iterator) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("source should parse before type checking");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("unfixed dyn associated type should be rejected");
    let message = err.to_string();
    assert!(
        message.contains("[dyn-associated-type]")
            && message.contains("Iterator")
            && message.contains("Item"),
        "expected dyn-associated-type diagnostic for unfixed associated type, got: {message}"
    );
}

#[test]
fn dyn_trait_with_fixed_associated_type_typechecks() {
    let source = r#"
trait Iterator {
    type Item;
}

def takes(x: dyn Iterator<Item = i64>) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("fixed associated type binding should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("fixed dyn associated type binding should typecheck");
}

#[test]
fn dyn_trait_rejects_associated_function_as_not_object_safe() {
    let source = r#"
trait Factory {
    def make() -> i64 {
        0
    }
}

def takes(x: dyn Factory) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("associated function traits should not be dyn-safe yet");
    let message = err.to_string();
    assert!(
        message.contains("[not-object-safe]")
            && message.contains("Factory")
            && message.contains("method `make`"),
        "expected not-object-safe diagnostic for associated function, got: {}",
        message
    );
}

#[test]
fn dyn_trait_rejects_generic_method_as_not_object_safe() {
    let source = r#"
trait Mapper {
    def map<T>(self, value: T) -> T {
        value
    }
}

def takes(x: dyn Mapper) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("generic method traits should not be dyn-safe yet");
    let message = err.to_string();
    assert!(
        message.contains("[not-object-safe]")
            && message.contains("Mapper")
            && message.contains("method `map`"),
        "expected not-object-safe diagnostic for generic method, got: {}",
        message
    );
}

#[test]
fn dyn_trait_allows_self_return_through_reference_indirection() {
    let source = r#"
trait Borrowed {
    def borrowed(&self) -> &Self {
        self
    }
}

def takes(x: dyn Borrowed) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("parse should succeed");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("Self behind a reference should remain object-safe");
}

#[test]
fn orphan_rule_rejects_external_trait_for_external_type() {
    let source = r#"
impl Drop for i64 {
    def drop(&mut self) {}
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("external trait for external type should be rejected");
    let message = err.to_string();
    assert!(
        message.contains("[orphan-rule]") && message.contains("Drop") && message.contains("i64"),
        "expected orphan-rule diagnostic, got: {message}"
    );
}

#[test]
fn duplicate_trait_impl_for_same_type_is_rejected() {
    let source = r#"
trait Greet {
    def greet(self) -> i64 { 0 }
}

impl Greet for i64 {
    def greet(self) -> i64 { 1 }
}

impl Greet for i64 {
    def greet(self) -> i64 { 2 }
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("two impls of the same trait for the same type should be rejected");
    let message = err.to_string();
    assert!(
        message.contains("[conflicting-impl]")
            && message.contains("Greet")
            && message.contains("i64"),
        "expected conflicting-impl diagnostic, got: {message}"
    );
}

#[test]
fn distinct_trait_impls_for_same_type_are_accepted() {
    let source = r#"
trait Greet {
    def greet(self) -> i64 { 0 }
}

trait Wave {
    def wave(self) -> i64 { 0 }
}

impl Greet for i64 {
    def greet(self) -> i64 { 1 }
}

impl Wave for i64 {
    def wave(self) -> i64 { 2 }
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("distinct traits for the same type should not conflict");
}

#[test]
fn same_generic_trait_for_same_type_with_distinct_args_is_accepted() {
    let source = r#"
trait Convert<T> {
    def convert(self) -> T {}
}

impl Convert<i64> for i32 {
    def convert(self) -> i64 { self as i64 }
}

impl Convert<u64> for i32 {
    def convert(self) -> u64 { self as u64 }
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("generic trait impls with distinct trait args should not conflict");
}

#[test]
fn duplicate_generic_trait_impl_for_same_type_and_args_is_rejected() {
    let source = r#"
trait Convert<T> {
    def convert(self) -> T {}
}

impl Convert<i64> for i32 {
    def convert(self) -> i64 { self as i64 }
}

impl Convert<i64> for i32 {
    def convert(self) -> i64 { self as i64 }
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("duplicate generic trait impl should be rejected");
    let message = err.to_string();
    assert!(
        message.contains("[conflicting-impl]")
            && message.contains("Convert<i64>")
            && message.contains("i32"),
        "expected generic conflicting-impl diagnostic, got: {message}"
    );
}

#[test]
fn polymorphic_recursion_reports_monomorphization_overflow() {
    // `deepen<T>` recurses as `deepen<Wrap<T>>`, growing the type argument
    // without bound. Monomorphization must stop with a stable diagnostic rather
    // than recursing until the compiler stack overflows.
    let source = r#"
struct Wrap<T> {
    inner: T,
}

def deepen<T>(value: T, n: i64) -> i64 {
    if n <= 0 {
        0
    } else {
        deepen(Wrap { inner: value }, n - 1)
    }
}

def main() -> i64 {
    deepen(0, 1000)
}
"#;

    let result = compile_to_ir(source);
    let err = result.expect_err("unbounded polymorphic recursion should be rejected");
    let message = err.to_string();
    assert!(
        message.contains("[monomorphization-overflow]"),
        "expected monomorphization-overflow diagnostic, got: {message}"
    );
}

#[test]
fn compiler_known_core_traits_and_support_types_are_available() {
    let source = r#"
def accepts_core_traits<T: Clone + Copy + Debug + Default + Iterator>(value: T) -> i64 {
    0
}

def accepts_support_types(ordering: Ordering, formatter: Formatter, hasher: Hasher) -> i64 {
    0
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("compiler-known core traits and support types should resolve");
}

#[test]
fn builtin_derives_register_core_trait_impls_for_bounds() {
    let source = r#"
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug, Default)]
struct User {
    id: i64,
}

def needs_traits<T: Clone + Copy + PartialEq + Eq + PartialOrd + Ord + Hash + Debug + Default>(value: T) -> i64 {
    0
}

def main() -> i64 {
    needs_traits(User { id: 1 })
}
"#;

    let program = Parser::parse(source).expect("derive source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("builtin derives should satisfy core trait bounds");
}

#[test]
fn primitive_types_satisfy_builtin_collection_trait_bounds() {
    let source = r#"
def needs_hash_eq<T: Hash + Eq>(value: T) -> i64 { 0 }
def needs_order<T: PartialEq + Eq + PartialOrd + Ord>(value: T) -> i64 { 0 }

def main() -> i64 {
    needs_hash_eq(1) + needs_hash_eq(true) + needs_order(1) + needs_order(true)
}
"#;

    let program = Parser::parse(source).expect("primitive trait-bound source should parse");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("primitive collection traits should be compiler-known");
}

#[test]
fn copy_and_drop_impls_are_mutually_exclusive() {
    let source = r#"
#[derive(Copy)]
struct Resource {
    id: i64,
}

impl Drop for Resource {
    def drop(&mut self) {}
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("Copy and Drop should be mutually exclusive");
    let message = err.to_string();
    assert!(
        message.contains("[copy-drop-conflict]") && message.contains("Resource"),
        "expected copy-drop-conflict diagnostic, got: {message}"
    );
}

#[test]
fn copy_derive_rejects_non_copy_fields() {
    let source = r#"
struct Owned {
    id: i64,
}

impl Drop for Owned {
    def drop(&mut self) {}
}

#[derive(Copy)]
struct Wrapper {
    owned: Owned,
}
"#;

    let program = Parser::parse(source).expect("source should parse");
    let mut checker = TypeChecker::new();
    let err = checker
        .check_program(&program)
        .expect_err("Copy should require all fields to be Copy");
    let message = err.to_string();
    assert!(
        message.contains("[copy-field-not-copy]")
            && message.contains("Wrapper")
            && message.contains("owned"),
        "expected copy-field-not-copy diagnostic, got: {message}"
    );
}

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
fn nested_generic_struct_field_with_trailing_comma_typechecks() {
    let source = r#"
struct Box<T> {
    value: T,
}

struct Wrap<T> {
    value: T,
}

struct Shared<T> {
    value: Wrap<Box<T>>,
}
"#;

    let program = Parser::parse(source)
        .expect("nested generic struct fields should close >> before the field comma");
    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .expect("nested generic struct field should typecheck");
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

#[test]
fn derive_default_struct_static_constructor_returns_zeroed_fields() {
    let source = r#"
#[derive(Default)]
struct Defaults {
    count: i64,
    ready: bool,
}

def main() -> i64 {
    let value = Defaults::default();
    if value.ready {
        0
    } else {
        value.count
    }
}
"#;

    let ir = compile_to_ir(source).expect("derived Default constructor should compile");
    assert!(
        ir.contains("; Function: Defaults_default"),
        "expected generated Defaults_default function in IR\n{ir}"
    );
}

#[test]
fn derive_clone_struct_method_copies_scalar_fields() {
    let source = r#"
#[derive(Clone)]
struct Point {
    x: i64,
    y: i64,
}

def main() -> i64 {
    let first = Point { x: 20, y: 1 };
    let second = first.clone();
    second.x + second.y
}
"#;

    let ir = compile_to_ir(source).expect("derived Clone method should compile");
    assert!(
        ir.contains("; Function: Point_clone"),
        "expected generated Point_clone function in IR\n{ir}"
    );
}

#[test]
fn derives_include_public_struct_fields() {
    let source = r#"
#[derive(Clone, Default, PartialEq)]
struct PublicPoint {
    pub x: i64,
    pub ready: bool,
}

def main() -> i64 {
    let first = PublicPoint { x: 41, ready: true };
    let second = first.clone();
    let fallback = PublicPoint::default();
    if second.eq(&first) && second.ready {
        second.x + fallback.x + 1
    } else {
        0
    }
}
"#;

    let ir = compile_to_ir(source).expect("derives should retain public fields");
    assert!(ir.contains("; Function: PublicPoint_clone"));
    assert!(ir.contains("; Function: PublicPoint_default"));
    assert!(ir.contains("; Function: PublicPoint_eq"));
}

#[test]
fn derive_clone_struct_method_clones_nested_fields() {
    let source = r#"
#[derive(Clone)]
struct Inner {
    value: i64,
}

#[derive(Clone)]
struct Outer {
    inner: Inner,
    flag: bool,
}

def main() -> i64 {
    let first = Outer { inner: Inner { value: 41 }, flag: true };
    let second = first.clone();
    if second.flag {
        second.inner.value + 1
    } else {
        0
    }
}
"#;

    let ir = compile_to_ir(source).expect("derived nested Clone method should compile");
    assert!(
        ir.contains("; Function: Outer_clone") && ir.contains("call %Inner @Inner_clone"),
        "expected generated Outer_clone to call Inner_clone\n{ir}"
    );
}

#[test]
fn derive_partial_eq_struct_method_compares_scalar_fields() {
    let source = r#"
#[derive(PartialEq)]
struct Point {
    x: i64,
    y: i64,
}

def main() -> i64 {
    let left = Point { x: 20, y: 1 };
    let right = Point { x: 20, y: 1 };
    if left.eq(&right) {
        42
    } else {
        0
    }
}
"#;

    let ir = compile_to_ir(source).expect("derived PartialEq method should compile");
    assert!(
        ir.contains("; Function: Point_eq"),
        "expected generated Point_eq function in IR\n{ir}"
    );
}

#[test]
fn derive_partial_eq_struct_operator_uses_generated_eq_method() {
    let source = r#"
#[derive(PartialEq)]
struct Point {
    x: i64,
    y: i64,
}

def main() -> i64 {
    let left = Point { x: 20, y: 1 };
    let right = Point { x: 20, y: 1 };
    if left == right {
        42
    } else {
        0
    }
}
"#;

    let ir = compile_to_ir(source).expect("derived PartialEq operator should compile");
    assert!(
        ir.contains("call i1 @Point_eq"),
        "expected == to call generated Point_eq method\n{ir}"
    );
}

#[test]
fn derive_partial_eq_struct_operator_compares_nested_fields() {
    let source = r#"
#[derive(PartialEq)]
struct Inner {
    value: i64,
}

#[derive(PartialEq)]
struct Outer {
    inner: Inner,
    flag: bool,
}

def main() -> i64 {
    let left = Outer { inner: Inner { value: 41 }, flag: true };
    let right = Outer { inner: Inner { value: 41 }, flag: true };
    if left == right {
        42
    } else {
        0
    }
}
"#;

    let ir = compile_to_ir(source).expect("derived nested PartialEq should compile");
    assert!(
        ir.contains("call i1 @Outer_eq") && ir.contains("call i1 @Inner_eq"),
        "expected nested equality to call both generated eq methods\n{ir}"
    );
}

#[test]
fn derive_ord_struct_method_compares_scalar_fields_lexicographically() {
    let source = r#"
#[derive(Ord)]
struct Point {
    x: i64,
    y: i64,
}

def main() -> i64 {
    let left = Point { x: 1, y: 9 };
    let right = Point { x: 2, y: 0 };
    if left.compare(&right) < 0 {
        42
    } else {
        0
    }
}
"#;

    let ir = compile_to_ir(source).expect("derived Ord compare method should compile");
    assert!(
        ir.contains("; Function: Point_compare"),
        "expected generated Point_compare function in IR\n{ir}"
    );
}

#[test]
fn derive_ord_struct_operator_uses_generated_compare_method() {
    let source = r#"
#[derive(Ord)]
struct Point {
    x: i64,
    y: i64,
}

def main() -> i64 {
    let left = Point { x: 1, y: 9 };
    let right = Point { x: 2, y: 0 };
    if left < right {
        42
    } else {
        0
    }
}
"#;

    let ir = compile_to_ir(source).expect("derived Ord comparison operator should compile");
    assert!(
        ir.contains("call i64 @Point_compare"),
        "expected < to call generated Point_compare method\n{ir}"
    );
}

#[test]
fn derive_ord_struct_operator_compares_nested_fields() {
    let source = r#"
#[derive(Ord)]
struct Inner {
    value: i64,
}

#[derive(Ord)]
struct Outer {
    inner: Inner,
    flag: bool,
}

def main() -> i64 {
    let left = Outer { inner: Inner { value: 1 }, flag: false };
    let right = Outer { inner: Inner { value: 2 }, flag: false };
    if left < right {
        42
    } else {
        0
    }
}
"#;

    let ir = compile_to_ir(source).expect("derived nested Ord comparison should compile");
    assert!(
        ir.contains("call i64 @Outer_compare") && ir.contains("call i64 @Inner_compare"),
        "expected nested ordering to call both generated compare methods\n{ir}"
    );
}

#[test]
fn derive_default_struct_static_constructor_uses_nested_default_fields() {
    let source = r#"
#[derive(Default)]
struct Inner {
    value: i64,
}

#[derive(Default)]
struct Outer {
    inner: Inner,
    ready: bool,
}

def main() -> i64 {
    let value = Outer::default();
    if value.ready {
        0
    } else {
        value.inner.value
    }
}
"#;

    let ir = compile_to_ir(source).expect("derived nested Default constructor should compile");
    assert!(
        ir.contains("; Function: Outer_default") && ir.contains("call %Inner @Inner_default"),
        "expected generated Outer_default to call Inner_default\n{ir}"
    );
}

#[test]
fn derive_hash_struct_method_combines_scalar_fields() {
    let source = r#"
#[derive(Hash)]
struct Key {
    a: i64,
    b: i64,
}

def main() -> i64 {
    let key = Key { a: 7, b: 11 };
    key.hash()
}
"#;

    let ir = compile_to_ir(source).expect("derived Hash method should compile");
    assert!(
        ir.contains("; Function: Key_hash"),
        "expected generated Key_hash function in IR\n{ir}"
    );
}

#[test]
fn derive_hash_struct_method_combines_nested_hash_fields() {
    let source = r#"
#[derive(Hash)]
struct Inner {
    value: i64,
}

#[derive(Hash)]
struct Outer {
    inner: Inner,
    flag: bool,
}

def main() -> i64 {
    let key = Outer { inner: Inner { value: 7 }, flag: true };
    key.hash()
}
"#;

    let ir = compile_to_ir(source).expect("derived nested Hash method should compile");
    assert!(
        ir.contains("; Function: Outer_hash") && ir.contains("call i64 @Inner_hash"),
        "expected generated Outer_hash to call Inner_hash\n{ir}"
    );
}

#[test]
fn custom_hash_into_impl_synthesizes_hash_bridge() {
    let source = r#"
struct Hasher {
    state: i64,
}

def hasher_new() -> Hasher {
    Hasher { state: 0 }
}

impl Hasher {
    def write_i64(&mut self, value: i64) -> bool {
        self.state = self.state + value;
        true
    }

    def finish(self) -> i64 {
        self.state
    }
}

struct Key {
    id: i64,
}

impl Hash for Key {
    def hash_into(&self, h: &mut Hasher) {
        h.write_i64(self.id);
    }
}

def use_hash<T: Hash>(value: T) -> i64 {
    value.hash()
}

def main() -> i64 {
    use_hash(Key { id: 7 })
}
"#;

    let ir = compile_to_ir(source).expect("hash_into protocol should synthesize hash bridge");
    assert!(
        ir.contains("; Function: Key_Hash_hash") && ir.contains("call void @Key_Hash_hash_into"),
        "expected Hash.hash bridge to drive hash_into, got:\n{ir}"
    );
}

#[test]
fn derive_hash_routes_through_hash_into_when_hasher_in_scope() {
    let source = r#"
struct Hasher {
    state: i64,
}

def hasher_new() -> Hasher {
    Hasher { state: 0 }
}

impl Hasher {
    def write_i64(&mut self, value: i64) -> bool {
        self.state = self.state + value;
        true
    }

    def write_bool(&mut self, value: bool) -> bool {
        self.state = self.state + (if value { 1 } else { 0 });
        true
    }

    def finish(self) -> i64 {
        self.state
    }
}

#[derive(Hash)]
struct Inner {
    value: i64,
}

#[derive(Hash)]
struct Key {
    inner: Inner,
    flag: bool,
}

def use_hash<T: Hash>(value: T) -> i64 {
    value.hash()
}

def main() -> i64 {
    use_hash(Key { inner: Inner { value: 7 }, flag: true }) + Key { inner: Inner { value: 7 }, flag: true }.hash()
}
"#;

    let ir = compile_to_ir(source).expect("derived Hash should route through hash_into");
    assert!(
        ir.contains("; Function: Key_Hash_hash_into")
            && ir.contains("call void @Inner_Hash_hash_into")
            && ir.contains("call void @Key_Hash_hash_into"),
        "expected derived hash_into bodies driving nested hash_into, got:\n{ir}"
    );
    assert!(
        ir.contains("; Function: Key_Hash_hash"),
        "expected synthesized hash bridge for derived hash_into, got:\n{ir}"
    );
}

#[test]
fn zero_argument_generic_constructor_infers_type_from_expected_return() {
    let source = r#"
struct Holder<T> {
    handle: i64,
}

def holder_new<T>() -> Holder<T> {
    Holder { handle: 0 }
}

def main() -> i64 {
    let holder: Holder<i64> = holder_new();
    holder.handle
}
"#;

    compile_to_ir(source)
        .expect("expected return type should infer a zero-argument constructor type parameter");
}

#[test]
fn option_none_infers_and_materializes_concrete_payload_defaults() {
    let source = r#"
struct Option<T> {
    is_some: bool,
    value: T,
}

struct Payload {
    count: i64,
    ready: bool,
}

def option_none<T>() -> Option<T> { __sengoo_option_none() }
def __sengoo_option_none<T>() -> Option<T> { __sengoo_option_none() }

def main() -> i64 {
    let number: Option<i64> = option_none();
    let flag: Option<bool> = option_none();
    let payload: Option<Payload> = option_none();
    if number.is_some || flag.is_some || payload.is_some {
        1
    } else {
        payload.value.count
    }
}
"#;

    let ir = compile_to_ir(source)
        .expect("option_none should infer T and synthesize a concrete unused payload");
    assert!(ir.contains("%Option_i64"), "missing Option<i64> in:\n{ir}");
    assert!(
        ir.contains("%Option_bool"),
        "missing Option<bool> in:\n{ir}"
    );
    assert!(
        ir.contains("%Option_Payload"),
        "missing Option<Payload> in:\n{ir}"
    );
    assert!(
        !ir.contains("call %Option_"),
        "option_none should lower directly instead of calling an empty body:\n{ir}"
    );
}

#[test]
fn user_option_none_function_is_not_hijacked_by_stdlib_intrinsic() {
    let source = r#"
def option_none() -> i64 {
    41
}

def main() -> i64 {
    option_none() + 1
}
"#;

    let ir = compile_to_ir(source).expect("ordinary same-named functions must resolve normally");
    assert!(
        ir.contains("call i64 @option_none"),
        "expected a normal user function call in:\n{ir}"
    );
}

#[test]
fn generic_iterator_next_resolves_associated_item_end_to_end() {
    let source = r#"
struct Option<T> {
    is_some: bool,
    value: T,
}

trait Iterator {
    type Item;
    def next(&self) -> Option<Self::Item> {}
}

struct BoolIter {
    value: bool,
}

impl Iterator for BoolIter {
    type Item = bool;
    def next(&self) -> Option<bool> {
        Option { is_some: true, value: self.value }
    }
}

def pull<I: Iterator>(iter: &I) -> Option<I::Item> {
    iter.next()
}

def main() -> i64 {
    let iter = BoolIter { value: true };
    let item: Option<bool> = pull(&iter);
    if item.value { 0 } else { 1 }
}
"#;

    let ir = compile_to_ir(source)
        .expect("generic Iterator::next should specialize I::Item through MIR lowering");
    assert!(
        ir.contains("pull_BoolIter") && ir.contains("BoolIter_Iterator_next"),
        "expected generic trait dispatch specialization in:\n{ir}"
    );
}

#[test]
fn generic_adapter_can_store_a_concrete_next_function() {
    let source = r#"
struct Option<T> { is_some: bool, value: T }
struct Counter { value: i64 }
struct Adapter<I, T> {
    inner: I,
    next_fn: fn(&I) -> Option<T>,
}

trait Iterator {
    type Item;
    def next(&self) -> Option<Self::Item> {}
}

impl Counter {
    def read(&self) -> i64 { self.value }
}

def counter_next(counter: &Counter) -> Option<i64> {
    Option { is_some: true, value: counter.read() }
}

impl<I, T> Iterator for Adapter<I, T> {
    type Item = T;
    def next(&self) -> Option<T> {
        let next = self.next_fn;
        next(&self.inner)
    }
}

impl<I, T> Adapter<I, T> {
    def next(&self) -> Option<T> {
        let next = self.next_fn;
        next(&self.inner)
    }
}

def main() -> i64 {
    let next: fn(&Counter) -> Option<i64> = counter_next;
    let adapter = Adapter {
        inner: Counter { value: 42 },
        next_fn: next,
    };
    adapter.next().value
}
"#;

    let ir = compile_to_ir(source).expect("adapter next function fields should specialize");
    assert!(
        ir.contains("call %Option_i64 %") || ir.contains("call %Option_i64 @counter_next"),
        "expected a concrete adapter next call in:\n{ir}"
    );
}

#[test]
fn generic_zero_arg_constructor_infers_from_variable_and_field_assignment_targets() {
    let source = r#"
struct Holder<T> { handle: i64 }
struct Outer<T> { inner: Holder<T> }

def holder_new<T>() -> Holder<T> {
    Holder { handle: 1 }
}

def main() -> i64 {
    let mut holder: Holder<bool> = holder_new();
    holder = holder_new();
    let initial: Holder<bool> = holder_new();
    let mut outer: Outer<bool> = Outer { inner: initial };
    outer.inner = holder_new();
    holder.handle + outer.inner.handle
}
"#;

    let ir = compile_to_ir(source).expect("assignment targets should infer constructor generics");
    assert!(ir.contains("holder_new_bool"));
}

#[test]
fn local_function_value_shadowing_wins_over_global_function_reference() {
    let source = r#"
def counter_next(value: i64) -> i64 { value + 1 }
def alt_next(value: i64) -> i64 { value + 9 }

def main() -> i64 {
    let counter_next: fn(i64) -> i64 = alt_next;
    let alias: fn(i64) -> i64 = counter_next;
    alias(1)
}
"#;

    let ir = compile_to_ir(source).expect("local function values should shadow globals");
    let main = ir
        .split("; Function: main")
        .nth(1)
        .expect("main should be emitted");
    assert!(main.contains("@alt_next"));
    assert!(!main.contains("@counter_next"));
}

#[test]
fn phantom_generic_struct_literals_keep_distinct_mir_instance_names() {
    let source = r#"
struct Phantom<T> {
    handle: i64,
}

def make_i64() -> Phantom<i64> {
    Phantom { handle: 1 }
}

def make_bool() -> Phantom<bool> {
    Phantom { handle: 2 }
}

def main() -> i64 {
    make_i64().handle + make_bool().handle
}
"#;
    let ir = compile_to_ir(source).expect("phantom generic instances should compile");
    assert!(
        ir.contains("%Phantom_i64 = type"),
        "missing i64 instance:\n{ir}"
    );
    assert!(
        ir.contains("%Phantom_bool = type"),
        "missing bool instance:\n{ir}"
    );
}
