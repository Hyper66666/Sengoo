use crate::hir::{self, HIRParam};

#[derive(Debug)]
pub(crate) struct MethodCandidate<T> {
    pub(crate) label: String,
    pub(crate) param_count: usize,
    pub(crate) value: T,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum MethodCandidateMatch<T> {
    None,
    WrongArity { expected: usize },
    One(T),
    Ambiguous { labels: Vec<String> },
}

pub(crate) fn explicit_hir_method_params(params: &[HIRParam]) -> &[HIRParam] {
    if params
        .first()
        .map(|param| param.name == "self")
        .unwrap_or(false)
    {
        &params[1..]
    } else {
        params
    }
}

pub(crate) fn explicit_hir_method_param_count(function: &hir::HIRFunction) -> usize {
    explicit_hir_method_params(&function.params).len()
}

pub(crate) fn select_method_candidate<T>(
    candidates: Vec<MethodCandidate<T>>,
    arg_count: usize,
) -> MethodCandidateMatch<T> {
    let mut matching = Vec::new();
    let total_candidates = candidates.len();
    let mut single_expected = None;

    for candidate in candidates {
        if total_candidates == 1 {
            single_expected = Some(candidate.param_count);
        }

        if candidate.param_count == arg_count {
            matching.push(candidate);
        }
    }

    match matching.len() {
        0 => {
            if total_candidates == 1 {
                MethodCandidateMatch::WrongArity {
                    expected: single_expected.expect("single candidate should record expected arity"),
                }
            } else {
                MethodCandidateMatch::None
            }
        }
        1 => MethodCandidateMatch::One(
            matching
                .pop()
                .expect("one matching candidate should remain")
                .value,
        ),
        _ => {
            let mut labels = matching
                .into_iter()
                .map(|candidate| candidate.label)
                .collect::<Vec<_>>();
            labels.sort();
            MethodCandidateMatch::Ambiguous { labels }
        }
    }
}

pub(crate) fn ambiguous_method_error(
    method_name: &str,
    type_name: &str,
    candidates: &[String],
) -> String {
    format!(
        "ambiguous method '{}' for type '{}': candidates {}",
        method_name,
        type_name,
        candidates.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use super::{select_method_candidate, MethodCandidate, MethodCandidateMatch};

    #[test]
    fn select_method_candidate_prefers_matching_arity() {
        let result = select_method_candidate(
            vec![
                MethodCandidate {
                    label: "TraitZero".to_string(),
                    param_count: 0,
                    value: 10,
                },
                MethodCandidate {
                    label: "TraitOne".to_string(),
                    param_count: 1,
                    value: 20,
                },
            ],
            0,
        );

        assert_eq!(result, MethodCandidateMatch::One(10));
    }

    #[test]
    fn select_method_candidate_reports_ambiguity_for_matching_arity() {
        let result = select_method_candidate(
            vec![
                MethodCandidate {
                    label: "Alpha".to_string(),
                    param_count: 0,
                    value: 10,
                },
                MethodCandidate {
                    label: "Beta".to_string(),
                    param_count: 0,
                    value: 20,
                },
            ],
            0,
        );

        assert_eq!(
            result,
            MethodCandidateMatch::Ambiguous {
                labels: vec!["Alpha".to_string(), "Beta".to_string()],
            }
        );
    }

    #[test]
    fn select_method_candidate_reports_wrong_arity_for_single_candidate() {
        let result = select_method_candidate(
            vec![MethodCandidate {
                label: "Only".to_string(),
                param_count: 1,
                value: 10,
            }],
            0,
        );

        assert_eq!(result, MethodCandidateMatch::WrongArity { expected: 1 });
    }
}
