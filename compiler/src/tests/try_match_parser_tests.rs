//! Parser coverage for `?` and `try {}` (typeck/lowering follow in OpenSpec §2–3).

use crate::ast::{DeclKind, ExprKind, StmtKind};
use crate::Parser;

fn parse_ok(source: &str) -> crate::ast::Program {
    let mut parser = Parser::new(source);
    parser.parse_program().expect("expected parse success")
}

fn parse_err(source: &str) {
    let mut parser = Parser::new(source);
    assert!(
        parser.parse_program().is_err(),
        "expected parse failure for:\n{source}"
    );
}

fn first_function_body(program: &crate::ast::Program) -> &crate::ast::Block {
    program
        .decls
        .iter()
        .find_map(|decl| match &decl.kind {
            DeclKind::Function(func) if func.name.name == "main" || func.name.name == "f" => {
                Some(&func.body)
            }
            _ => None,
        })
        .expect("function body")
}

#[test]
fn postfix_question_parses_as_try_expr() {
    let program = parse_ok(
        r#"
def f() -> i64 {
    let x = g()?;
    x
}
def g() -> i64 { 1 }
"#,
    );
    let body = first_function_body(&program);
    let StmtKind::Let {
        value: Some(value), ..
    } = &body.stmts[0].kind
    else {
        panic!("expected let");
    };
    assert!(matches!(value.kind, ExprKind::Try(_)));
}

#[test]
fn nested_postfix_question_parses() {
    parse_ok(
        r#"
def main() -> i64 {
    let x = outer(inner()?)?;
    x
}
def outer(v: i64) -> i64 { v }
def inner() -> i64 { 1 }
"#,
    );
}

#[test]
fn try_block_expr_parses() {
    let program = parse_ok(
        r#"
def main() -> i64 {
    try {
        let x = 1;
        x
    }
}
"#,
    );
    let body = first_function_body(&program);
    let StmtKind::Expr(expr) = &body.stmts[0].kind else {
        panic!("expected expr stmt");
    };
    assert!(matches!(expr.kind, ExprKind::TryBlock(_)));
}

#[test]
fn postfix_question_binds_tighter_than_add() {
    let program = parse_ok(
        r#"
def main() -> i64 {
    let x = f()? + 1;
    x
}
def f() -> i64 { 1 }
"#,
    );
    let body = first_function_body(&program);
    let StmtKind::Let {
        value: Some(value), ..
    } = &body.stmts[0].kind
    else {
        panic!("expected let");
    };
    let ExprKind::Binary { left, .. } = &value.kind else {
        panic!("expected binary +");
    };
    assert!(matches!(left.kind, ExprKind::Try(_)));
}

#[test]
fn match_or_pattern_and_guard_still_parse() {
    parse_ok(
        r#"
def main() -> i64 {
    let x = 2;
    match x {
        0 | 1 => 10,
        y if y > 1 => 20,
        _ => 30,
    }
}
"#,
    );
}

#[test]
fn leading_question_is_invalid() {
    parse_err(
        r#"
def main() -> i64 {
    ?1
}
"#,
    );
}
