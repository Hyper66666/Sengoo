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
