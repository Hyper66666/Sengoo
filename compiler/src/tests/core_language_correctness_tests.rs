//! Regression coverage for the core-language-correctness conformance wave.

use crate::ast::{DeclKind, StmtKind};
use crate::codegen::{Codegen, JITCodegen};
use crate::error::CompileError;
use crate::mir::{Instruction, Terminator};
use crate::{compile_to_mir, Parser, TypeChecker};

fn parse(source: &str) -> crate::ast::Program {
    Parser::parse(source).expect("source should parse")
}

fn typeck_error(source: &str) -> crate::typeck::TypeckError {
    let program = parse(source);
    let mut checker = TypeChecker::new();
    match checker.check_program(&program) {
        Err(CompileError::TypeckError(error)) => error,
        Err(other) => panic!("expected type-check error, got {other}"),
        Ok(()) => panic!("expected type-check failure"),
    }
}

#[test]
fn let_mut_parses_and_marks_the_local_binding_mutable() {
    let program = parse(
        r#"
def main() -> i64 {
    let mut value = 1;
    value
}
"#,
    );
    let main = program
        .decls
        .iter()
        .find_map(|decl| match &decl.kind {
            DeclKind::Function(function) if function.name.name == "main" => Some(function),
            _ => None,
        })
        .expect("main function");

    assert!(matches!(
        main.body.stmts[0].kind,
        StmtKind::Let { is_mut: true, .. }
    ));
}

#[test]
fn mutable_local_can_be_reassigned() {
    let program = parse(
        r#"
def main() -> i64 {
    let mut value = 1;
    value = value + 1;
    value
}
"#,
    );
    TypeChecker::new()
        .check_program(&program)
        .expect("mutable assignment should type-check");
}

#[test]
fn mutable_function_parameter_can_be_reassigned() {
    let program = parse(
        r#"
def increment(mut value: i64) -> i64 {
    value = value + 1;
    value
}
def main() -> i64 { increment(1) }
"#,
    );
    TypeChecker::new()
        .check_program(&program)
        .expect("mutable parameter assignment should type-check");
}

#[test]
fn immutable_function_parameter_assignment_is_rejected() {
    let error = typeck_error(
        r#"
def increment(value: i64) -> i64 {
    value = value + 1;
    value
}
def main() -> i64 { increment(1) }
"#,
    );
    assert_eq!(error.stable_code(), Some("immutable-assignment"));
}

#[test]
fn let_mut_requires_a_binding_name() {
    let error = Parser::parse(
        r#"
def main() -> i64 {
    let mut = 1;
    0
}
"#,
    )
    .expect_err("missing binding name should be rejected");

    assert!(error.to_string().contains("identifier"));
}

#[test]
fn immutable_local_assignment_has_stable_diagnostic_and_span() {
    let error = typeck_error(
        r#"
def main() -> i64 {
    let value = 1;
    value = value + 1;
    value
}
"#,
    );

    assert_eq!(error.stable_code(), Some("immutable-assignment"));
    assert!(
        error.span().is_some(),
        "diagnostic should identify the target"
    );
    assert!(error.to_string().contains("let mut"));
}

#[test]
fn fieldless_enum_variant_is_a_value_and_lowers_to_enum_construct() {
    let functions = compile_to_mir(
        r#"
enum Color { Red, Green }

def label(color: Color) -> i64 {
    match color {
        Color::Red => 1,
        Color::Green => 2,
    }
}

def main() -> i64 {
    label(Color::Green)
}
"#,
    )
    .expect("fieldless enum construction should compile");

    let main = functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main MIR");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::EnumConstruct {
            discriminant: 1,
            payload: None,
            ..
        }
    )));
}

#[test]
fn payload_enum_variant_is_constructed_with_checked_payload() {
    let functions = compile_to_mir(
        r#"
enum Maybe { Vacant, Value(i64) }

def unwrap(value: Maybe) -> i64 {
    match value {
        Maybe::Vacant => 0,
        Maybe::Value(inner) => inner,
    }
}

def main() -> i64 {
    unwrap(Maybe::Value(42))
}
"#,
    )
    .expect("payload enum construction should compile");

    let main = functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main MIR");
    assert!(main.instructions.iter().any(|instruction| matches!(
        instruction,
        Instruction::EnumConstruct {
            discriminant: 1,
            payload: Some(_),
            ..
        }
    )));
}

#[test]
fn jit_codegen_lowers_enum_construction_and_match_without_fallback_comments() {
    let functions = compile_to_mir(
        r#"
enum Maybe { Empty, Value(i64) }

def main() -> i64 {
    let value = Maybe::Value(42);
    match value {
        Maybe::Empty => 0,
        Maybe::Value(inner) => inner,
    }
}
"#,
    )
    .expect("enum match should compile to MIR");

    let mut jit = JITCodegen::new();
    let ir = jit
        .generate(&functions)
        .expect("JIT codegen should lower enum construction and match");

    assert!(
        !ir.contains("unhandled"),
        "JIT enum lowering must not emit fallback comments:\n{ir}"
    );
    for unsupported in [
        "unhandled instruction: EnumConstruct",
        "unhandled instruction: Discriminant",
        "unhandled instruction: ExtractPayload",
        "unhandled instruction: Phi",
        "unhandled terminator: Switch",
    ] {
        assert!(
            !ir.contains(unsupported),
            "JIT enum lowering must not fall back to comments ({unsupported}):\n{ir}"
        );
    }
    assert!(ir.contains("switch i64"), "expected enum switch:\n{ir}");
    assert!(ir.contains("phi i64"), "expected match result phi:\n{ir}");

    let mut native = Codegen::new();
    native
        .codegen(&functions)
        .expect("native codegen should still accept the same MIR");
}

#[test]
fn exhaustive_enum_match_routes_unknown_discriminants_to_unreachable() {
    let functions = compile_to_mir(
        r#"
enum Maybe { Empty, Value(i64) }

def main() -> i64 {
    let value = Maybe::Value(42);
    match value {
        Maybe::Empty => 0,
        Maybe::Value(inner) => inner,
    }
}
"#,
    )
    .expect("exhaustive enum match should compile to MIR");

    let main = functions
        .iter()
        .find(|function| function.name == "main")
        .expect("main MIR");
    let otherwise = main
        .basic_blocks
        .iter()
        .find_map(|block| match block.terminator.as_ref() {
            Some(Terminator::Switch { otherwise, .. }) => Some(*otherwise),
            _ => None,
        })
        .expect("enum match switch");

    assert!(
        matches!(
            main.basic_blocks[otherwise].terminator,
            Some(Terminator::Unreachable)
        ),
        "exhaustive enum match default edge must not enter the phi join"
    );
}

#[test]
fn unknown_enum_variant_has_stable_diagnostic() {
    let error = typeck_error(
        r#"
enum Color { Red }
def main() -> Color { Color::Blue }
"#,
    );
    assert_eq!(error.stable_code(), Some("unknown-enum-variant"));
}

#[test]
fn enum_variant_arity_mismatch_has_stable_diagnostic() {
    let error = typeck_error(
        r#"
enum Maybe { Value(i64) }
def main() -> Maybe { Maybe::Value() }
"#,
    );
    assert_eq!(error.stable_code(), Some("enum-variant-arity"));
}

#[test]
fn enum_variant_payload_type_mismatch_has_stable_diagnostic() {
    let error = typeck_error(
        r#"
enum Maybe { Value(i64) }
def main() -> Maybe { Maybe::Value(true) }
"#,
    );
    assert_eq!(error.stable_code(), Some("enum-variant-type"));
}

#[test]
fn fixed_array_constant_out_of_bounds_has_stable_diagnostic() {
    let error = typeck_error(
        r#"
def main() -> i64 {
    let values = [1, 2, 3];
    values[3]
}
"#,
    );
    assert_eq!(error.stable_code(), Some("array-index-out-of-bounds"));
    assert!(error.span().is_some());
}

#[test]
fn fixed_array_non_integer_index_has_stable_diagnostic() {
    let error = typeck_error(
        r#"
def main() -> i64 {
    let values = [1, 2, 3];
    values[true]
}
"#,
    );
    assert_eq!(error.stable_code(), Some("invalid-array-index"));
    assert!(error.span().is_some());
}

#[test]
fn duplicate_closure_parameter_has_stable_diagnostic() {
    let error = typeck_error(
        r#"
def main() -> i64 {
    let invalid = |value, value| value;
    invalid(1, 2)
}
"#,
    );
    assert_eq!(error.stable_code(), Some("duplicate-closure-parameter"));
    assert!(error.span().is_some());
}
