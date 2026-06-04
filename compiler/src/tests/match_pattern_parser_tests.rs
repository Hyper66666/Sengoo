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
