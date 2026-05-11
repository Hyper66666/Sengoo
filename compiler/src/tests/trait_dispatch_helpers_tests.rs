use crate::method_resolution::MethodCandidateMatch;
use crate::mir::trait_dispatch_helpers::{
    resolve_known_trait_method_name, select_known_trait_method_candidate,
};

#[test]
fn select_known_trait_method_candidate_filters_prefix_suffix_and_excluded_name() {
    let candidates = vec![
        ("Vec_map", 1usize),
        ("Vec_Into_map", 1usize),
        ("Vec_Iter_map", 1usize),
        ("Other_Iter_map", 1usize),
        ("Vec_Iter_len", 0usize),
    ];

    let result = select_known_trait_method_candidate(candidates, "Vec", "map", "Vec_map", 1);

    assert_eq!(
        result,
        MethodCandidateMatch::Ambiguous {
            labels: vec!["Vec_Into_map".to_string(), "Vec_Iter_map".to_string()],
        }
    );
}

#[test]
fn select_known_trait_method_candidate_reports_wrong_arity_for_single_match() {
    let candidates = vec![("Vec_Iter_map", 2usize)];

    let result = select_known_trait_method_candidate(candidates, "Vec", "map", "Vec_map", 1);

    assert_eq!(result, MethodCandidateMatch::WrongArity { expected: 2 });
}

#[test]
fn resolve_known_trait_method_name_returns_selected_name() {
    let resolved = resolve_known_trait_method_name(
        vec![("Vec_Iter_map", 1usize)],
        "Vec",
        "map",
        "Vec_map",
        1,
        "Vec",
    )
    .expect("single matching candidate should resolve");

    assert_eq!(resolved, "Vec_Iter_map");
}

#[test]
fn resolve_known_trait_method_name_builds_ambiguous_error() {
    let err = resolve_known_trait_method_name(
        vec![("Vec_Into_map", 1usize), ("Vec_Iter_map", 1usize)],
        "Vec",
        "map",
        "Vec_map",
        1,
        "Vec",
    )
    .expect_err("multiple matching candidates should be ambiguous");

    assert_eq!(
        err,
        "ambiguous method 'map' for type 'Vec': candidates Vec_Into_map, Vec_Iter_map"
    );
}
