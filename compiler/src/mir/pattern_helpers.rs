use crate::hir::{HIRLiteral, HIRMatchArm, HIRPattern};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PatternBindingPlan {
    Ignore,
    BindWhole(String),
    BindTupleFields(Vec<(u32, String)>),
    BindStructFields(Vec<(String, String)>),
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PatternMatchPlan {
    AlwaysTrue,
    EqLiteral(HIRLiteral),
    EqDiscriminant(u32),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchSwitchPlan {
    pub targets: Vec<(u32, usize)>,
    pub otherwise_block: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SwitchArmPlan {
    pub discriminants: Vec<u32>,
    pub includes_fallback: bool,
}

/// Classify how an arm participates in enum/int switch lowering.
pub(crate) fn classify_switch_arm(pat: &HIRPattern) -> SwitchArmPlan {
    match pat {
        HIRPattern::Wild | HIRPattern::Var { .. } => SwitchArmPlan {
            discriminants: Vec::new(),
            includes_fallback: true,
        },
        HIRPattern::Lit(lit) => SwitchArmPlan {
            discriminants: match lit {
                HIRLiteral::Int(n) if *n >= 0 && *n < u32::MAX as i64 => vec![*n as u32],
                _ => Vec::new(),
            },
            includes_fallback: false,
        },
        HIRPattern::EnumVariant { discriminant, .. } => SwitchArmPlan {
            discriminants: vec![*discriminant],
            includes_fallback: false,
        },
        HIRPattern::Or(lhs, rhs) => {
            let left = classify_switch_arm(lhs);
            let right = classify_switch_arm(rhs);
            let mut discriminants = left.discriminants;
            discriminants.extend(right.discriminants);
            discriminants.sort_unstable();
            discriminants.dedup();
            SwitchArmPlan {
                discriminants,
                includes_fallback: left.includes_fallback || right.includes_fallback,
            }
        }
        _ => SwitchArmPlan {
            discriminants: Vec::new(),
            includes_fallback: false,
        },
    }
}

pub(crate) fn pattern_binding_plan(pat: &HIRPattern) -> PatternBindingPlan {
    match pat {
        HIRPattern::Var { name, .. } => PatternBindingPlan::BindWhole(name.clone()),
        HIRPattern::Tuple(patterns) => {
            let fields = patterns
                .iter()
                .enumerate()
                .filter_map(|(index, sub_pat)| match sub_pat {
                    HIRPattern::Var { name, .. } => Some((index as u32, name.clone())),
                    _ => None,
                })
                .collect::<Vec<_>>();
            if fields.is_empty() {
                PatternBindingPlan::Ignore
            } else {
                PatternBindingPlan::BindTupleFields(fields)
            }
        }
        HIRPattern::EnumVariant { fields, .. } => enum_variant_binding_plan(fields),
        HIRPattern::Struct { fields, .. } => struct_binding_plan(fields),
        HIRPattern::Or(lhs, _) => pattern_binding_plan(lhs),
        _ => PatternBindingPlan::Ignore,
    }
}

fn enum_variant_binding_plan(fields: &[(String, Option<HIRPattern>)]) -> PatternBindingPlan {
    let tuple_like = fields.iter().all(|(name, _)| name.starts_with('_'));
    if tuple_like {
        let bindings = fields
            .iter()
            .enumerate()
            .filter_map(|(index, (_, sub_pat))| match sub_pat {
                Some(HIRPattern::Var { name, .. }) => Some((index as u32, name.clone())),
                _ => None,
            })
            .collect::<Vec<_>>();
        if bindings.is_empty() {
            PatternBindingPlan::Ignore
        } else {
            PatternBindingPlan::BindTupleFields(bindings)
        }
    } else {
        struct_binding_plan(fields)
    }
}

fn struct_binding_plan(fields: &[(String, Option<HIRPattern>)]) -> PatternBindingPlan {
    let bindings: Vec<(String, String)> = fields
        .iter()
        .map(|(field_name, sub_pat)| {
            let bind_name = match sub_pat {
                Some(HIRPattern::Var { name, .. }) => name.clone(),
                Some(_) | None => field_name.clone(),
            };
            (field_name.clone(), bind_name)
        })
        .collect();
    if bindings.is_empty() {
        PatternBindingPlan::Ignore
    } else {
        PatternBindingPlan::BindStructFields(bindings)
    }
}

pub(crate) fn pattern_match_plan(pat: &HIRPattern) -> PatternMatchPlan {
    match pat {
        HIRPattern::Lit(lit) => PatternMatchPlan::EqLiteral(lit.clone()),
        HIRPattern::EnumVariant { discriminant, .. } => {
            PatternMatchPlan::EqDiscriminant(*discriminant)
        }
        HIRPattern::Wild | HIRPattern::Var { .. } => PatternMatchPlan::AlwaysTrue,
        HIRPattern::Struct { fields, .. } if fields.is_empty() => PatternMatchPlan::AlwaysTrue,
        HIRPattern::Or(lhs, rhs) => {
            let lhs_plan = pattern_match_plan(lhs);
            let rhs_plan = pattern_match_plan(rhs);
            match (lhs_plan, rhs_plan) {
                (PatternMatchPlan::EqDiscriminant(a), PatternMatchPlan::EqDiscriminant(b))
                    if a == b =>
                {
                    PatternMatchPlan::EqDiscriminant(a)
                }
                _ => PatternMatchPlan::AlwaysTrue,
            }
        }
        _ => PatternMatchPlan::AlwaysTrue,
    }
}

pub(crate) fn build_match_switch_plan(
    arms: &[HIRMatchArm],
    arm_blocks: &[usize],
    default_otherwise: usize,
) -> MatchSwitchPlan {
    let mut targets = Vec::new();
    let mut otherwise_block = default_otherwise;

    for (arm, arm_block) in arms.iter().zip(arm_blocks.iter().copied()) {
        let plan = classify_switch_arm(&arm.pat);
        for value in plan.discriminants {
            targets.push((value, arm_block));
        }
        if plan.includes_fallback {
            otherwise_block = arm_block;
        }
    }

    MatchSwitchPlan {
        targets,
        otherwise_block,
    }
}
