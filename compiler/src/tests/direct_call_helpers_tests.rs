use crate::hir::{
    HIRBody, HIRExpr, HIRFunction, HIRImpl, HIRItem, HIRLiteral, HIRMatchArm, HIRParam, HIRPattern,
    HIRStmt, HIRType, IntKind,
};
use crate::mir::direct_call_helpers::{
    collect_direct_call_names, collect_direct_calls_in_body, collect_direct_calls_in_expr,
};
use crate::symbol::SymbolId;
use std::collections::HashSet;

fn var(name: &str, symbol: u32) -> HIRExpr {
    HIRExpr::Var {
        name: name.to_string(),
        symbol: SymbolId::new(symbol),
    }
}

fn empty_function(name: &str, body: HIRBody) -> HIRFunction {
    HIRFunction {
        name: name.to_string(),
        type_params: vec![],
        params: vec![HIRParam::new(
            "x".to_string(),
            SymbolId::new(100),
            HIRType::int(IntKind::I64),
        )],
        return_type: HIRType::int(IntKind::I64),
        precondition: None,
        postcondition: None,
        body,
        is_async: false,
        abi: None,
        is_unsafe: false,
        no_mangle: false,
        export_name: None,
        is_pub: false,
    }
}

#[test]
fn collect_direct_calls_in_expr_finds_nested_direct_calls_only() {
    let expr = HIRExpr::Match {
        scrutinee: Box::new(HIRExpr::Call {
            func: Box::new(var("scrutinee_fn", 1)),
            args: vec![],
        }),
        arms: vec![HIRMatchArm {
            pat: HIRPattern::Wild,
            guard: Some(Box::new(HIRExpr::Call {
                func: Box::new(var("guard_fn", 2)),
                args: vec![],
            })),
            body: Box::new(HIRExpr::AsyncBlock(Box::new(HIRBody::with_expr(HIRExpr::Call {
                func: Box::new(var("body_fn", 3)),
                args: vec![
                    HIRExpr::Call {
                        func: Box::new(HIRExpr::Field {
                            base: Box::new(var("obj", 4)),
                            field: "method".to_string(),
                        }),
                        args: vec![],
                    },
                    var("plain_arg", 5),
                ],
            })))),
        }],
    };

    let mut calls = HashSet::new();
    collect_direct_calls_in_expr(&expr, &mut calls);

    assert_eq!(
        calls,
        HashSet::from([
            "scrutinee_fn".to_string(),
            "guard_fn".to_string(),
            "body_fn".to_string(),
        ])
    );
}

#[test]
fn collect_direct_call_names_accumulates_function_and_impl_bodies() {
    let function = HIRItem::Function(empty_function(
        "main",
        HIRBody {
            stmts: vec![HIRStmt::Expr(HIRExpr::Call {
                func: Box::new(var("free_fn", 10)),
                args: vec![],
            })],
            expr: Some(Box::new(HIRExpr::Lit(HIRLiteral::Int(0)))),
        },
    ));

    let impl_item = HIRItem::Impl(HIRImpl {
        target_type: HIRType::named("Point".to_string(), vec![]),
        trait_name: None,
        items: vec![empty_function(
            "sum",
            HIRBody::with_expr(HIRExpr::Call {
                func: Box::new(var("helper_fn", 11)),
                args: vec![],
            }),
        )],
    });

    let calls = collect_direct_call_names(&[function, impl_item]);
    assert_eq!(
        calls,
        HashSet::from(["free_fn".to_string(), "helper_fn".to_string()])
    );
}

#[test]
fn collect_direct_calls_in_body_visits_stmt_values_and_tail_expression() {
    let body = HIRBody {
        stmts: vec![HIRStmt::Let {
            name: "tmp".to_string(),
            symbol: SymbolId::new(12),
            ty: HIRType::int(IntKind::I64),
            value: Some(HIRExpr::Call {
                func: Box::new(var("stmt_fn", 13)),
                args: vec![],
            }),
            is_mut: false,
        }],
        expr: Some(Box::new(HIRExpr::Call {
            func: Box::new(var("tail_fn", 14)),
            args: vec![],
        })),
    };

    let mut calls = HashSet::new();
    collect_direct_calls_in_body(&body, &mut calls);

    assert_eq!(
        calls,
        HashSet::from(["stmt_fn".to_string(), "tail_fn".to_string()])
    );
}
