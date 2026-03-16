use crate::hir::{HIRLiteral, HIRMatchArm, HIRPattern};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PatternBindingPlan {
    Ignore,
    BindWhole(String),
    BindTupleFields(Vec<(u32, String)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MatchSwitchPlan {
    pub targets: Vec<(u32, usize)>,
    pub otherwise_block: usize,
}

pub(crate) fn extract_discriminant_from_pattern(pat: &HIRPattern) -> Option<u32> {
    match pat {
        HIRPattern::Lit(lit) => match lit {
            HIRLiteral::Int(n) if *n >= 0 && *n < u32::MAX as i64 => Some(*n as u32),
            _ => None,
        },
        HIRPattern::Wild | HIRPattern::Var { .. } => None,
        _ => None,
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
        _ => PatternBindingPlan::Ignore,
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
        if let Some(value) = extract_discriminant_from_pattern(&arm.pat) {
            targets.push((value, arm_block));
        } else {
            otherwise_block = arm_block;
        }
    }

    MatchSwitchPlan {
        targets,
        otherwise_block,
    }
}
