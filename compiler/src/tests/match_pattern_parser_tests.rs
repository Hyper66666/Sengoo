//! Parser coverage for match pattern forms.

use crate::ast::pattern::PatternKind;
use crate::ast::{DeclKind, Expr, ExprKind, StmtKind};
use crate::Parser;

fn parse_program(source: &str) -> crate::ast::Program {
    let mut parser = Parser::new(source);
    parser.parse_program().expect("parse should succeed")
}

fn find_match_arms(program: &crate::ast::Program) -> Vec<crate::ast::MatchArm> {
    let func = program
        .decls
        .iter()
        .find_map(|decl| match &decl.kind {
            DeclKind::Function(func) if func.name.name == "main" => Some(func),
            _ => None,
        })
        .expect("main function");
    let mut matches = Vec::new();
    for stmt in &func.body.stmts {
        match &stmt.kind {
            StmtKind::Expr(expr) => walk_expr(expr, &mut matches),
            StmtKind::Let {
                value: Some(value), ..
            } => walk_expr(value, &mut matches),
            _ => {}
        }
    }
    matches
}

fn walk_expr(expr: &Expr, out: &mut Vec<crate::ast::MatchArm>) {
    if let ExprKind::Match { arms, .. } = &expr.kind {
        out.extend(arms.clone());
        return;
    }
    if let ExprKind::Block(block) = &expr.kind {
        for stmt in &block.stmts {
            if let StmtKind::Expr(inner) = &stmt.kind {
                walk_expr(inner, out);
            } else if let StmtKind::Let {
                value: Some(value), ..
            } = &stmt.kind
            {
                walk_expr(value, out);
            }
        }
    }
}

#[test]
fn match_literal_and_wildcard_patterns_parse() {
    let program = parse_program(
        r#"
def main() -> i64 {
    let x = 1;
    match x {
        0 => 10,
        _ => 20,
    }
}
"#,
    );
    let arms = find_match_arms(&program);
    assert!(matches!(arms[0].patterns[0].kind, PatternKind::Literal(_)));
}

#[test]
fn match_enum_tuple_struct_pattern_parses() {
    let program = parse_program(
        r#"
enum Color { Red, Blue }
def main() -> i64 {
    let c = Color::Red;
    match c {
        Color::Red => 1,
        Color::Blue => 2,
    }
}
"#,
    );
    let arms = find_match_arms(&program);
    assert!(matches!(arms[0].patterns[0].kind, PatternKind::Path(_)));
}

#[test]
fn match_struct_shorthand_and_guard_parse() {
    let program = parse_program(
        r#"
struct Point { x: i64, y: i64 }
def main() -> i64 {
    let p = Point { x: 1, y: 2 };
    match p {
        Point { x, y } if y > 0 => x,
        _ => 0,
    }
}
"#,
    );
    let arms = find_match_arms(&program);
    assert!(matches!(
        arms[0].patterns[0].kind,
        PatternKind::Struct { .. }
    ));
    assert!(arms[0].guard.is_some());
}

#[test]
fn match_or_pattern_in_arm_parses() {
    let program = parse_program(
        r#"
def main() -> i64 {
    let x = 1;
    match x {
        0 | 1 => 10,
        _ => 20,
    }
}
"#,
    );
    let arms = find_match_arms(&program);
    assert!(
        arms[0].patterns.len() == 2 || matches!(arms[0].patterns[0].kind, PatternKind::Or(_)),
        "expected arm-level or nested or-pattern"
    );
}

fn assert_tuple_struct_arm(arms: &[crate::ast::MatchArm], index: usize, arity: usize) {
    match &arms[index].patterns[0].kind {
        PatternKind::TupleStruct { patterns, .. } => {
            assert_eq!(
                patterns.len(),
                arity,
                "payload arm {index} should bind {arity} values"
            );
        }
        other => panic!("expected tuple-struct payload pattern, got {other:?}"),
    }
}

#[test]
fn match_payload_arm_parses_in_first_middle_and_last_positions() {
    let program = parse_program(
        r#"
enum State { One(i64), Two, Three(i64) }
def main() -> i64 {
    let state = State::One(1);
    match state {
        State::One(first) => first,
        State::Two => 2,
        State::Three(last) => last,
    }
}
"#,
    );

    let arms = find_match_arms(&program);
    assert_eq!(arms.len(), 3);
    assert_tuple_struct_arm(&arms, 0, 1);
    assert!(matches!(arms[1].patterns[0].kind, PatternKind::Path(_)));
    assert_tuple_struct_arm(&arms, 2, 1);

    let program = parse_program(
        r#"
enum State { One, Two(i64), Three }
def main() -> i64 {
    let state = State::Two(2);
    match state {
        State::One => 1,
        State::Two(middle) => middle,
        State::Three => 3,
    }
}
"#,
    );

    let arms = find_match_arms(&program);
    assert_eq!(arms.len(), 3);
    assert_tuple_struct_arm(&arms, 1, 1);
}

#[test]
fn match_multiple_payload_arms_parse_in_one_match() {
    let program = parse_program(
        r#"
enum Event { Number(i64), Pair(i64, bool), Empty }
def main() -> i64 {
    let event = Event::Pair(2, true);
    match event {
        Event::Number(value) => value,
        Event::Pair(number, enabled) => if enabled { number } else { 0 },
        Event::Empty => 0,
    }
}
"#,
    );

    let arms = find_match_arms(&program);
    assert_eq!(arms.len(), 3);
    assert_tuple_struct_arm(&arms, 0, 1);
    assert_tuple_struct_arm(&arms, 1, 2);
}

#[test]
fn malformed_payload_pattern_is_rejected() {
    let mut parser = Parser::new(
        r#"
enum Maybe { Empty, Value(i64) }
def main() -> i64 {
    let value = Maybe::Value(1);
    match value {
        Maybe::Value(,) => 1,
        Maybe::Empty => 0,
    }
}
"#,
    );

    let error = parser
        .parse_program()
        .expect_err("malformed payload pattern should be rejected");
    let message = error.to_string();
    assert!(
        message.contains("pattern") || message.contains("identifier"),
        "diagnostic should be pattern-related: {message}"
    );
}
