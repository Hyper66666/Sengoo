use crate::method_resolution::{
    select_method_candidate, MethodCandidate, MethodCandidateMatch,
};

pub(crate) fn select_known_trait_method_candidate<'a, I>(
    functions: I,
    type_prefix: &str,
    method: &str,
    excluded_name: &str,
    expected_param_count: usize,
) -> MethodCandidateMatch<String>
where
    I: IntoIterator<Item = (&'a str, usize)>,
{
    let suffix = format!("_{}", method);
    let prefix = format!("{}_", type_prefix);
    let matches = functions
        .into_iter()
        .filter(|(name, _)| {
            name.starts_with(&prefix) && name.ends_with(&suffix) && *name != excluded_name && {
                let middle = &name[prefix.len()..name.len() - suffix.len()];
                !middle.is_empty()
            }
        })
        .map(|(name, param_count)| MethodCandidate {
            label: name.to_string(),
            param_count,
            value: name.to_string(),
        })
        .collect();
    select_method_candidate(matches, expected_param_count)
}
