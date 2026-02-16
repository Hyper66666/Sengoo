use crate::typeck::TypeckError;
use crate::{compile_to_ir, CompileError, Parser};

#[derive(Debug, Clone, Copy)]
enum TypeckExpectation {
    Ok,
    UndefinedVariable,
    UndefinedFunction,
    ArgumentCountMismatch,
    TypeMismatch,
    MethodNotFound,
}

struct ParseCase {
    name: &'static str,
    source: &'static str,
    should_parse: bool,
}

struct TypeckCase {
    name: &'static str,
    source: &'static str,
    expectation: TypeckExpectation,
}

fn parser_cases() -> Vec<ParseCase> {
    vec![
        ParseCase {
            name: "simple_main_function",
            source: r#"
def main() -> i64 {
    42
}
"#,
            should_parse: true,
        },
        ParseCase {
            name: "if_comparison_condition",
            source: r#"
def main() -> i64 {
    let a = 1;
    let b = 2;
    if a < b { 1 } else { 0 }
}
"#,
            should_parse: true,
        },
        ParseCase {
            name: "struct_literal_with_shorthand",
            source: r#"
struct Point { x: i64, y: i64 }
def main() -> i64 {
    let x = 1;
    let y = 2;
    let p = Point { x, y };
    p.x
}
"#,
            should_parse: true,
        },
        ParseCase {
            name: "match_with_or_pattern",
            source: r#"
def main() -> i64 {
    let x = 2;
    match x {
        0 | 1 => 10,
        _ => 20,
    }
}
"#,
            should_parse: true,
        },
        ParseCase {
            name: "lambda_expression",
            source: r#"
def main() -> i64 {
    let add1 = |x| x + 1;
    add1(41)
}
"#,
            should_parse: true,
        },
        ParseCase {
            name: "impl_trait_declaration",
            source: r#"
trait AddOne {
    def add_one(self) -> i64 {
        1
    }
}
impl AddOne for i64 {
    def add_one(self) -> i64 {
        self + 1
    }
}
def main() -> i64 {
    0
}
"#,
            should_parse: true,
        },
        ParseCase {
            name: "invalid_struct_numeric_field_name",
            source: r#"
def main() -> i64 {
    let p = Point { 123: 1 };
    0
}
"#,
            should_parse: false,
        },
        ParseCase {
            name: "invalid_struct_string_shorthand",
            source: r#"
def main() -> i64 {
    let p = Point { "x" };
    0
}
"#,
            should_parse: false,
        },
        ParseCase {
            name: "top_level_let_is_not_declaration",
            source: r#"
let x = 1;
"#,
            should_parse: false,
        },
        ParseCase {
            name: "trait_item_must_be_def_or_const",
            source: r#"
trait T {
    let x = 1;
}
"#,
            should_parse: false,
        },
        ParseCase {
            name: "missing_type_after_annotation_colon",
            source: r#"
def main() -> i64 {
    let x: = 1;
    x
}
"#,
            should_parse: false,
        },
        ParseCase {
            name: "array_type_requires_integer_length",
            source: r#"
def main() -> i64 {
    let x: [i64; foo] = [1];
    x[0]
}
"#,
            should_parse: false,
        },
    ]
}

fn typeck_cases() -> Vec<TypeckCase> {
    vec![
        TypeckCase {
            name: "ok_simple_literal",
            source: r#"def main() -> i64 { 1 }"#,
            expectation: TypeckExpectation::Ok,
        },
        TypeckCase {
            name: "ok_builtin_print",
            source: r#"def main() -> i64 { print("ok"); 0 }"#,
            expectation: TypeckExpectation::Ok,
        },
        TypeckCase {
            name: "ok_function_call",
            source: r#"
def add(a: i64, b: i64) -> i64 { a + b }
def main() -> i64 { add(1, 2) }
"#,
            expectation: TypeckExpectation::Ok,
        },
        TypeckCase {
            name: "undefined_variable",
            source: r#"
def main() -> i64 {
    missing_name
}
"#,
            expectation: TypeckExpectation::UndefinedVariable,
        },
        TypeckCase {
            name: "undefined_function_from_non_callable",
            source: r#"
def main() -> i64 {
    let x = 1;
    x()
}
"#,
            expectation: TypeckExpectation::UndefinedFunction,
        },
        TypeckCase {
            name: "argument_count_mismatch_builtin_print",
            source: r#"
def main() -> i64 {
    print(1, 2);
    0
}
"#,
            expectation: TypeckExpectation::ArgumentCountMismatch,
        },
        TypeckCase {
            name: "argument_count_mismatch_user_function",
            source: r#"
def add(a: i64, b: i64) -> i64 { a + b }
def main() -> i64 { add(1) }
"#,
            expectation: TypeckExpectation::ArgumentCountMismatch,
        },
        TypeckCase {
            name: "type_mismatch_in_let_annotation",
            source: r#"
def main() -> i64 {
    let x: bool = 1;
    0
}
"#,
            expectation: TypeckExpectation::TypeMismatch,
        },
        TypeckCase {
            name: "type_mismatch_in_if_condition",
            source: r#"
def main() -> i64 {
    if 1 { 1 } else { 0 }
}
"#,
            expectation: TypeckExpectation::TypeMismatch,
        },
        TypeckCase {
            name: "type_mismatch_in_if_branches",
            source: r#"
def main() -> i64 {
    if true { 1 } else { false }
}
"#,
            expectation: TypeckExpectation::TypeMismatch,
        },
        TypeckCase {
            name: "method_not_found_on_i64",
            source: r#"
def main() -> i64 {
    let x = 1;
    x.len()
}
"#,
            expectation: TypeckExpectation::MethodNotFound,
        },
        TypeckCase {
            name: "method_argument_count_mismatch",
            source: r#"
def main() -> i64 {
    "abc".len(1)
}
"#,
            expectation: TypeckExpectation::ArgumentCountMismatch,
        },
    ]
}

fn assert_typeck_error(case_name: &str, err: CompileError, expectation: TypeckExpectation) {
    let compile_err = match err {
        CompileError::TypeckError(typeck_err) => typeck_err,
        other => panic!(
            "[{}] expected type-checking error, got different compiler stage: {:?}",
            case_name, other
        ),
    };

    match (expectation, compile_err) {
        (TypeckExpectation::UndefinedVariable, TypeckError::UndefinedVariable { .. }) => {}
        (TypeckExpectation::UndefinedFunction, TypeckError::UndefinedFunction { .. }) => {}
        (TypeckExpectation::ArgumentCountMismatch, TypeckError::ArgumentCountMismatch { .. }) => {}
        (TypeckExpectation::TypeMismatch, TypeckError::TypeMismatch { .. }) => {}
        (TypeckExpectation::MethodNotFound, TypeckError::MethodNotFound { .. }) => {}
        (expected, actual) => {
            panic!("[{}] expected {:?}, got {:?}", case_name, expected, actual);
        }
    }
}

#[test]
fn regression_suite_has_at_least_twenty_cases() {
    let total = parser_cases().len() + typeck_cases().len();
    assert!(
        total >= 20,
        "regression suite should include at least 20 cases, got {}",
        total
    );
}

#[test]
fn parser_regression_suite() {
    for case in parser_cases() {
        let parsed = Parser::parse(case.source);
        if case.should_parse {
            assert!(
                parsed.is_ok(),
                "[{}] expected parse success, got {:?}",
                case.name,
                parsed.err()
            );
        } else {
            assert!(parsed.is_err(), "[{}] expected parse failure", case.name);
        }
    }
}

#[test]
fn typeck_regression_suite() {
    for case in typeck_cases() {
        match case.expectation {
            TypeckExpectation::Ok => {
                let ir = compile_to_ir(case.source);
                assert!(
                    ir.is_ok(),
                    "[{}] expected compile success, got {:?}",
                    case.name,
                    ir.err()
                );
            }
            expectation => {
                let err = compile_to_ir(case.source)
                    .expect_err(&format!("[{}] expected compile failure", case.name));
                assert_typeck_error(case.name, err, expectation);
            }
        }
    }
}
