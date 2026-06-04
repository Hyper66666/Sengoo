use crate::hir::{HIRLiteral, HIRMatchArm, HIRPattern};
use crate::mir::pattern_helpers::{
    build_match_switch_plan, classify_switch_arm, pattern_binding_plan, pattern_match_plan,
    MatchSwitchPlan, PatternBindingPlan, PatternMatchPlan,
};
use crate::symbol::SymbolId;

#[test]
fn classify_switch_arm_accepts_non_negative_int_literals() {
    assert_eq!(
        classify_switch_arm(&HIRPattern::Lit(HIRLiteral::Int(7))).discriminants,
        vec![7]
    );
    assert_eq!(
        classify_switch_arm(&HIRPattern::Lit(HIRLiteral::Int(u32::MAX as i64 - 1))).discriminants,
        vec![u32::MAX - 1]
    );
}

#[test]
fn classify_switch_arm_rejects_non_discriminant_patterns() {
    assert!(classify_switch_arm(&HIRPattern::Lit(HIRLiteral::Int(-1)))
        .discriminants
        .is_empty());
    assert!(
        classify_switch_arm(&HIRPattern::Lit(HIRLiteral::Bool(true)))
            .discriminants
            .is_empty()
    );
    assert!(classify_switch_arm(&HIRPattern::Wild)
        .discriminants
        .is_empty());
    assert!(classify_switch_arm(&HIRPattern::Var {
        name: "x".to_string(),
        symbol: SymbolId::new(1),
        mutability: false,
    })
    .discriminants
    .is_empty());
}

#[test]
fn pattern_binding_plan_extracts_simple_var_and_tuple_bindings() {
    assert_eq!(
        pattern_binding_plan(&HIRPattern::Var {
            name: "whole".to_string(),
            symbol: SymbolId::new(2),
            mutability: false,
        }),
        PatternBindingPlan::BindWhole("whole".to_string())
    );

    assert_eq!(
        pattern_binding_plan(&HIRPattern::Tuple(vec![
            HIRPattern::Var {
                name: "left".to_string(),
                symbol: SymbolId::new(3),
                mutability: false,
            },
            HIRPattern::Wild,
            HIRPattern::Var {
                name: "right".to_string(),
                symbol: SymbolId::new(4),
                mutability: false,
            },
        ])),
        PatternBindingPlan::BindTupleFields(vec![
            (0, "left".to_string()),
            (2, "right".to_string())
        ])
    );
}

#[test]
fn pattern_binding_plan_ignores_non_var_tuple_members_and_other_patterns() {
    assert_eq!(
        pattern_binding_plan(&HIRPattern::Tuple(vec![HIRPattern::Wild])),
        PatternBindingPlan::Ignore
    );
    assert_eq!(
        pattern_binding_plan(&HIRPattern::Struct {
            name: "Point".to_string(),
            fields: vec![],
        }),
        PatternBindingPlan::Ignore
    );
}

#[test]
fn classify_switch_arm_flattens_or_patterns() {
    let pat = HIRPattern::Or(
        Box::new(HIRPattern::EnumVariant {
            discriminant: 1,
            fields: Vec::new(),
        }),
        Box::new(HIRPattern::EnumVariant {
            discriminant: 2,
            fields: Vec::new(),
        }),
    );
    assert_eq!(classify_switch_arm(&pat).discriminants, vec![1, 2]);
}

#[test]
fn build_match_switch_plan_routes_literal_arms_and_last_fallback() {
    let arms = vec![
        HIRMatchArm::new(
            HIRPattern::Lit(HIRLiteral::Int(1)),
            crate::hir::HIRExpr::Lit(HIRLiteral::Int(1)),
        ),
        HIRMatchArm::new(
            HIRPattern::Wild,
            crate::hir::HIRExpr::Lit(HIRLiteral::Int(0)),
        ),
        HIRMatchArm::new(
            HIRPattern::Var {
                name: "fallback".to_string(),
                symbol: SymbolId::new(5),
                mutability: false,
            },
            crate::hir::HIRExpr::Lit(HIRLiteral::Int(2)),
        ),
    ];

    let plan = build_match_switch_plan(&arms, &[10, 11, 12], 99);
    assert_eq!(
        plan,
        MatchSwitchPlan {
            targets: vec![(1, 10)],
            otherwise_block: 12,
        }
    );
}

#[test]
fn build_match_switch_plan_enum_or_with_wildcard_is_also_otherwise() {
    let arms = vec![
        HIRMatchArm::new(
            HIRPattern::Or(
                Box::new(HIRPattern::EnumVariant {
                    discriminant: 0,
                    fields: Vec::new(),
                }),
                Box::new(HIRPattern::Wild),
            ),
            crate::hir::HIRExpr::Lit(HIRLiteral::Int(1)),
        ),
        HIRMatchArm::new(
            HIRPattern::EnumVariant {
                discriminant: 1,
                fields: Vec::new(),
            },
            crate::hir::HIRExpr::Lit(HIRLiteral::Int(2)),
        ),
    ];
    let plan = build_match_switch_plan(&arms, &[10, 11], 99);
    assert_eq!(
        plan,
        MatchSwitchPlan {
            targets: vec![(0, 10), (1, 11)],
            otherwise_block: 10,
        }
    );
}

#[test]
fn build_match_switch_plan_maps_or_variant_arms_before_wildcard() {
    let arms = vec![
        HIRMatchArm::new(
            HIRPattern::EnumVariant {
                discriminant: 0,
                fields: Vec::new(),
            },
            crate::hir::HIRExpr::Lit(HIRLiteral::Int(1)),
        ),
        HIRMatchArm::new(
            HIRPattern::Or(
                Box::new(HIRPattern::EnumVariant {
                    discriminant: 1,
                    fields: Vec::new(),
                }),
                Box::new(HIRPattern::EnumVariant {
                    discriminant: 2,
                    fields: Vec::new(),
                }),
            ),
            crate::hir::HIRExpr::Lit(HIRLiteral::Int(2)),
        ),
        HIRMatchArm::new(
            HIRPattern::Wild,
            crate::hir::HIRExpr::Lit(HIRLiteral::Int(0)),
        ),
    ];
    let plan = build_match_switch_plan(&arms, &[10, 11, 12], 99);
    assert_eq!(
        plan,
        MatchSwitchPlan {
            targets: vec![(0, 10), (1, 11), (2, 11)],
            otherwise_block: 12,
        }
    );
}

#[test]
fn pattern_match_plan_distinguishes_literal_and_always_true_patterns() {
    assert_eq!(
        pattern_match_plan(&HIRPattern::Lit(HIRLiteral::Int(9))),
        PatternMatchPlan::EqLiteral(HIRLiteral::Int(9))
    );
    assert_eq!(
        pattern_match_plan(&HIRPattern::Wild),
        PatternMatchPlan::AlwaysTrue
    );
    assert_eq!(
        pattern_match_plan(&HIRPattern::Var {
            name: "bound".to_string(),
            symbol: SymbolId::new(6),
            mutability: false,
        }),
        PatternMatchPlan::AlwaysTrue
    );
    assert_eq!(
        pattern_match_plan(&HIRPattern::Tuple(vec![HIRPattern::Wild])),
        PatternMatchPlan::AlwaysTrue
    );
}
