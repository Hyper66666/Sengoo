use crate::hir::{HIRBody, HIRExpr, HIRLiteral, HIRMatchArm, HIRPattern, HIRStmt};
use crate::mir::lowering_helpers::{
    collect_free_vars, collect_named_symbols, collect_named_symbols_in_body,
};
use crate::mir::{Local, LocalKind};
use crate::symbol::SymbolId;
use std::collections::HashMap;

fn var(name: &str, symbol: u32) -> HIRExpr {
    HIRExpr::Var {
        name: name.to_string(),
        symbol: SymbolId::new(symbol),
    }
}

#[test]
fn collect_named_symbols_finds_result_in_nested_control_flow() {
    let expr = HIRExpr::If {
        cond: Box::new(var("cond", 1)),
        then_branch: Box::new(HIRBody::with_expr(HIRExpr::Match {
            scrutinee: Box::new(var("result", 2)),
            arms: vec![
                HIRMatchArm::new(HIRPattern::Wild, var("result", 3)),
                HIRMatchArm::new(HIRPattern::Wild, var("other", 4)),
            ],
        })),
        else_branch: Some(Box::new(HIRBody::with_expr(HIRExpr::Tuple(vec![
            var("other", 5),
            var("result", 6),
        ])))),
    };

    let mut out = Vec::new();
    collect_named_symbols(&expr, "result", &mut out);

    assert_eq!(
        out,
        vec![SymbolId::new(2), SymbolId::new(3), SymbolId::new(6)]
    );
}

#[test]
fn collect_named_symbols_in_body_visits_stmt_values_and_tail_expr() {
    let mut body = HIRBody::new();
    body.add_stmt(HIRStmt::Let {
        name: "x".to_string(),
        symbol: SymbolId::new(10),
        ty: crate::hir::HIRType::unit(),
        value: Some(HIRExpr::Call {
            func: Box::new(var("callee", 11)),
            args: vec![var("result", 12)],
        }),
        is_mut: false,
    });
    body.add_stmt(HIRStmt::Expr(HIRExpr::Await(Box::new(var("result", 13)))));
    body.set_expr(HIRExpr::Block(Box::new(HIRBody::with_expr(HIRExpr::Lit(
        HIRLiteral::Int(0),
    )))));

    let mut out = Vec::new();
    collect_named_symbols_in_body(&body, "result", &mut out);

    assert_eq!(out, vec![SymbolId::new(12), SymbolId::new(13)]);
}

#[test]
fn collect_free_vars_excludes_params_and_deduplicates_outer_locals() {
    let expr = HIRExpr::Binary(
        crate::hir::HIRBinaryOp::Add,
        Box::new(var("param", 1)),
        Box::new(HIRExpr::Binary(
            crate::hir::HIRBinaryOp::Add,
            Box::new(var("outer", 2)),
            Box::new(var("outer", 3)),
        )),
    );

    let mut local_names = HashMap::new();
    local_names.insert("param".to_string(), Local::new(1, LocalKind::User));
    local_names.insert("outer".to_string(), Local::new(2, LocalKind::User));

    let free_vars = collect_free_vars(&expr, &["param".to_string()], &local_names);
    assert_eq!(
        free_vars,
        vec![("outer".to_string(), Local::new(2, LocalKind::User))]
    );
}

#[test]
fn collect_free_vars_respects_let_scope_and_for_binding() {
    let body = HIRBody {
        stmts: vec![
            HIRStmt::Let {
                name: "local".to_string(),
                symbol: SymbolId::new(20),
                ty: crate::hir::HIRType::int(crate::hir::IntKind::I64),
                value: Some(var("outer", 21)),
                is_mut: false,
            },
            HIRStmt::Expr(HIRExpr::For {
                var_name: "item".to_string(),
                var_symbol: SymbolId::new(22),
                iter: Box::new(var("iterable", 23)),
                body: Box::new(HIRBody::with_expr(HIRExpr::Binary(
                    crate::hir::HIRBinaryOp::Add,
                    Box::new(var("item", 24)),
                    Box::new(var("local", 25)),
                ))),
            }),
        ],
        expr: Some(Box::new(var("outer", 26))),
    };

    let expr = HIRExpr::Block(Box::new(body));
    let mut local_names = HashMap::new();
    local_names.insert("outer".to_string(), Local::new(10, LocalKind::User));
    local_names.insert("iterable".to_string(), Local::new(11, LocalKind::User));
    local_names.insert("local".to_string(), Local::new(12, LocalKind::User));
    local_names.insert("item".to_string(), Local::new(13, LocalKind::User));

    let free_vars = collect_free_vars(&expr, &[], &local_names);
    assert_eq!(
        free_vars,
        vec![
            ("outer".to_string(), Local::new(10, LocalKind::User)),
            ("iterable".to_string(), Local::new(11, LocalKind::User)),
        ]
    );
}
