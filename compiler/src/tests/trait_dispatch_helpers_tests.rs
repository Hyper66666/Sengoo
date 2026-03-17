use crate::method_resolution::MethodCandidateMatch;
use crate::mir::trait_dispatch_helpers::select_known_trait_method_candidate;

#[test]
fn select_known_trait_method_candidate_filters_prefix_suffix_and_excluded_name() {
    let candidates = vec![
        ("Vec_map", 1usize),
        ("Vec_Into_map", 1usize),
        ("Vec_Iter_map", 1usize),
        ("Other_Iter_map", 1usize),
        ("Vec_Iter_len", 0usize),
    ];

    let result =
        select_known_trait_method_candidate(candidates, "Vec", "map", "Vec_map", 1);

    assert_eq!(result, MethodCandidateMatch::Ambiguous {
        labels: vec!["Vec_Into_map".to_string(), "Vec_Iter_map".to_string()],
    });
}

#[test]
fn select_known_trait_method_candidate_reports_wrong_arity_for_single_match() {
    let candidates = vec![("Vec_Iter_map", 2usize)];

    let result =
        select_known_trait_method_candidate(candidates, "Vec", "map", "Vec_map", 1);

    assert_eq!(result, MethodCandidateMatch::WrongArity { expected: 2 });
}
