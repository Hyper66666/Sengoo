use crate::hir::{HIRLiteral, HIRMatchArm, HIRPattern};
use crate::mir::pattern_helpers::{
    build_match_switch_plan, extract_discriminant_from_pattern, pattern_binding_plan,
    pattern_match_plan, MatchSwitchPlan, PatternBindingPlan, PatternMatchPlan,
};
use crate::symbol::SymbolId;

#[test]
fn extract_discriminant_from_pattern_accepts_non_negative_int_literals() {
    assert_eq!(
        extract_discriminant_from_pattern(&HIRPattern::Lit(HIRLiteral::Int(7))),
        Some(7)
    );
    assert_eq!(
        extract_discriminant_from_pattern(&HIRPattern::Lit(HIRLiteral::Int(u32::MAX as i64 - 1))),
        Some(u32::MAX - 1)
    );
}

#[test]
fn extract_discriminant_from_pattern_rejects_non_discriminant_patterns() {
    assert_eq!(
        extract_discriminant_from_pattern(&HIRPattern::Lit(HIRLiteral::Int(-1))),
        None
    );
    assert_eq!(
        extract_discriminant_from_pattern(&HIRPattern::Lit(HIRLiteral::Bool(true))),
        None
    );
    assert_eq!(extract_discriminant_from_pattern(&HIRPattern::Wild), None);
    assert_eq!(
        extract_discriminant_from_pattern(&HIRPattern::Var {
            name: "x".to_string(),
            symbol: SymbolId::new(1),
            mutability: false,
        }),
        None
    );
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
        PatternBindingPlan::BindTupleFields(vec![(0, "left".to_string()), (2, "right".to_string())])
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
fn build_match_switch_plan_routes_literal_arms_and_last_fallback() {
    let arms = vec![
        HIRMatchArm::new(HIRPattern::Lit(HIRLiteral::Int(1)), crate::hir::HIRExpr::Lit(HIRLiteral::Int(1))),
        HIRMatchArm::new(HIRPattern::Wild, crate::hir::HIRExpr::Lit(HIRLiteral::Int(0))),
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
fn pattern_match_plan_distinguishes_literal_and_always_true_patterns() {
    assert_eq!(
        pattern_match_plan(&HIRPattern::Lit(HIRLiteral::Int(9))),
        PatternMatchPlan::EqLiteral(HIRLiteral::Int(9))
    );
    assert_eq!(pattern_match_plan(&HIRPattern::Wild), PatternMatchPlan::AlwaysTrue);
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
