use crate::hir::{
    HIRBody, HIRExpr, HIRFunction, HIRLiteral, HIRMatchArm, HIRParam, HIRPattern, HIRStmt, HIRType,
    HIRTypeKind, IntKind,
};
use crate::mir::hir_specialization_helpers::{
    hir_type_is_concrete, hir_type_is_placeholder_name, substitute_hir_function,
    substitute_hir_type,
};
use crate::symbol::SymbolId;
use std::collections::{HashMap, HashSet};

#[test]
fn hir_type_placeholder_and_concrete_checks_respect_known_named_types() {
    let known_named_types = HashSet::from(["Point".to_string(), "Vec".to_string()]);

    assert_eq!(
        hir_type_is_placeholder_name(&HIRType::named("T".to_string(), vec![]), &known_named_types),
        Some("T".to_string())
    );
    assert_eq!(
        hir_type_is_placeholder_name(
            &HIRType::named("Point".to_string(), vec![]),
            &known_named_types
        ),
        None
    );

    assert!(hir_type_is_concrete(
        &HIRType::named("Point".to_string(), vec![]),
        &known_named_types
    ));
    assert!(hir_type_is_concrete(
        &HIRType::named("Vec".to_string(), vec![HIRType::int(IntKind::I64)]),
        &known_named_types
    ));
    assert!(!hir_type_is_concrete(
        &HIRType::named(
            "Vec".to_string(),
            vec![HIRType::named("T".to_string(), vec![])]
        ),
        &known_named_types
    ));
}

#[test]
fn substitute_hir_type_preserves_associated_projection_identity() {
    let projection = HIRType::new(HIRTypeKind::AssocProjection {
        base: Box::new(HIRType::named("T".to_string(), Vec::new())),
        trait_name: "Iterator".to_string(),
        name: "Item".to_string(),
    });
    let substituted = substitute_hir_type(
        &projection,
        &HashMap::from([("T".to_string(), HIRType::int(IntKind::I64))]),
    );

    assert_eq!(
        substituted,
        HIRType::new(HIRTypeKind::AssocProjection {
            base: Box::new(HIRType::int(IntKind::I64)),
            trait_name: "Iterator".to_string(),
            name: "Item".to_string(),
        })
    );

    let resolved = substitute_hir_type(
        &projection,
        &HashMap::from([("<T as Iterator>::Item".to_string(), HIRType::bool())]),
    );
    assert_eq!(resolved, HIRType::bool());
}

#[test]
fn substitute_hir_function_rewrites_signature_contracts_and_async_block_body() {
    let mut subst = HashMap::new();
    subst.insert("T".to_string(), HIRType::int(IntKind::I64));

    let function = HIRFunction {
        name: "demo".to_string(),
        type_params: vec![],
        params: vec![HIRParam::new(
            "value".to_string(),
            SymbolId::new(1),
            HIRType::named("T".to_string(), vec![]),
        )],
        return_type: HIRType::named("T".to_string(), vec![]),
        precondition: Some(HIRExpr::Ascribe(
            Box::new(HIRExpr::Lit(HIRLiteral::Int(1))),
            HIRType::named("T".to_string(), vec![]),
        )),
        postcondition: Some(HIRExpr::Match {
            scrutinee: Box::new(HIRExpr::Var {
                name: "value".to_string(),
                symbol: SymbolId::new(1),
            }),
            arms: vec![HIRMatchArm {
                pat: HIRPattern::Wild,
                guard: Some(Box::new(HIRExpr::Ascribe(
                    Box::new(HIRExpr::Lit(HIRLiteral::Int(2))),
                    HIRType::named("T".to_string(), vec![]),
                ))),
                body: Box::new(HIRExpr::Lit(HIRLiteral::Bool(true))),
            }],
        }),
        body: HIRBody {
            stmts: vec![HIRStmt::Let {
                name: "captured".to_string(),
                symbol: SymbolId::new(2),
                ty: HIRType::named("T".to_string(), vec![]),
                value: Some(HIRExpr::Cast(
                    Box::new(HIRExpr::Lit(HIRLiteral::Int(3))),
                    HIRType::named("T".to_string(), vec![]),
                )),
                is_mut: false,
            }],
            expr: Some(Box::new(HIRExpr::AsyncBlock(Box::new(HIRBody {
                stmts: vec![],
                expr: Some(Box::new(HIRExpr::Ascribe(
                    Box::new(HIRExpr::Var {
                        name: "captured".to_string(),
                        symbol: SymbolId::new(2),
                    }),
                    HIRType::named("T".to_string(), vec![]),
                ))),
            })))),
        },
        is_async: true,
        abi: None,
        is_unsafe: false,
        no_mangle: false,
        export_name: None,
        is_pub: false,
    };

    let substituted = substitute_hir_function(&function, &subst);

    assert_eq!(substituted.params[0].ty, HIRType::int(IntKind::I64));
    assert_eq!(substituted.return_type, HIRType::int(IntKind::I64));

    let Some(HIRExpr::Ascribe(_, pre_ty)) = substituted.precondition else {
        panic!("expected substituted precondition ascription");
    };
    assert_eq!(pre_ty, HIRType::int(IntKind::I64));

    let Some(HIRExpr::Match { arms, .. }) = substituted.postcondition else {
        panic!("expected substituted match postcondition");
    };
    let Some(guard) = &arms[0].guard else {
        panic!("expected substituted match guard");
    };
    let HIRExpr::Ascribe(_, guard_ty) = guard.as_ref() else {
        panic!("expected substituted match guard ascription");
    };
    assert_eq!(*guard_ty, HIRType::int(IntKind::I64));

    let HIRStmt::Let { ty, value, .. } = &substituted.body.stmts[0] else {
        panic!("expected substituted let statement");
    };
    assert_eq!(ty, &HIRType::int(IntKind::I64));
    let Some(HIRExpr::Cast(_, cast_ty)) = value else {
        panic!("expected substituted cast in let initializer");
    };
    assert_eq!(cast_ty, &HIRType::int(IntKind::I64));

    let Some(expr) = substituted.body.expr.as_deref() else {
        panic!("expected async block body");
    };
    let HIRExpr::AsyncBlock(async_body) = expr else {
        panic!("expected async block");
    };
    let Some(inner_expr) = &async_body.expr else {
        panic!("expected async block tail expression");
    };
    let HIRExpr::Ascribe(_, async_ty) = inner_expr.as_ref() else {
        panic!("expected substituted async block ascription");
    };
    assert_eq!(*async_ty, HIRType::int(IntKind::I64));
}
