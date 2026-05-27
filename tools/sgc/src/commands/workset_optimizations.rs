use crate::{BuildGraphV2, EditClass, EditImpact, GenericInstancePlanStats};
use std::collections::{HashMap, HashSet, VecDeque};

use super::shared::{configured_hir_prune_min_functions, large_project_mode_effectively_enabled};

fn generic_symbol_set(graph: &BuildGraphV2) -> HashSet<String> {
    graph
        .nodes
        .iter()
        .flat_map(|node| node.generic_items.iter().map(|item| item.symbol.clone()))
        .collect()
}

fn root_function_count(graph: &BuildGraphV2) -> usize {
    graph
        .nodes
        .iter()
        .find(|node| node.module_path == graph.root_module)
        .map(|node| node.functions.len())
        .unwrap_or(0)
}

fn likely_prunes_unreachable_functions(graph: &BuildGraphV2, choice: Option<bool>) -> bool {
    if !large_project_mode_effectively_enabled(choice) {
        return false;
    }
    let min = configured_hir_prune_min_functions();
    if min == usize::MAX {
        return false;
    }
    root_function_count(graph) >= min
}

fn reachable_symbols_from_main(graph: &BuildGraphV2) -> Option<HashSet<String>> {
    let mut calls_by_symbol = HashMap::<String, Vec<String>>::new();
    for node in &graph.nodes {
        for function in &node.functions {
            calls_by_symbol.insert(function.symbol.clone(), function.calls.clone());
        }
    }

    let root_main = format!("{}::main", graph.root_module);
    if !calls_by_symbol.contains_key(&root_main) {
        return None;
    }

    let mut reachable = HashSet::<String>::new();
    let mut queue = VecDeque::from([root_main]);
    while let Some(symbol) = queue.pop_front() {
        if !reachable.insert(symbol.clone()) {
            continue;
        }
        let Some(calls) = calls_by_symbol.get(&symbol) else {
            continue;
        };
        for callee in calls {
            if calls_by_symbol.contains_key(callee) {
                queue.push_back(callee.clone());
            }
        }
    }

    Some(reachable)
}

pub(crate) fn can_reuse_artifacts_for_unreachable_impl_only_changes(
    impact: Option<&EditImpact>,
    graph: &BuildGraphV2,
    large_project_mode_choice: Option<bool>,
) -> bool {
    let Some(impact) = impact else {
        return false;
    };
    if !matches!(impact.class, EditClass::ImplOnly) {
        return false;
    }
    if impact.changed_functions.is_empty() {
        return false;
    }
    if !likely_prunes_unreachable_functions(graph, large_project_mode_choice) {
        return false;
    }

    let root_prefix = format!("{}::", graph.root_module);
    let changed_root_functions = impact
        .changed_functions
        .iter()
        .filter(|symbol| symbol.starts_with(&root_prefix))
        .cloned()
        .collect::<Vec<_>>();
    if changed_root_functions.is_empty() {
        return false;
    }

    let Some(reachable) = reachable_symbols_from_main(graph) else {
        return false;
    };

    changed_root_functions
        .iter()
        .all(|symbol| !reachable.contains(symbol))
}

pub(crate) fn can_skip_codegen_via_generic_cache(
    impact: Option<&EditImpact>,
    graph: &BuildGraphV2,
    generic_stats: &GenericInstancePlanStats,
) -> bool {
    if generic_stats.total_instances == 0
        || generic_stats.rebuilt_instances != 0
        || generic_stats.new_instances != 0
        || generic_stats.interface_invalidated != 0
        || generic_stats.body_invalidated != 0
        || generic_stats.dependency_invalidated != 0
    {
        return false;
    }
    let Some(impact) = impact else {
        return false;
    };
    if matches!(impact.class, EditClass::InterfaceChange) {
        return false;
    }

    let generic_symbols = generic_symbol_set(graph);
    if generic_symbols.is_empty() {
        return false;
    }
    if impact.changed_functions.is_empty() || impact.impacted_functions.is_empty() {
        return false;
    }
    impact
        .changed_functions
        .iter()
        .all(|symbol| generic_symbols.contains(symbol))
        && impact
            .impacted_functions
            .iter()
            .all(|symbol| generic_symbols.contains(symbol))
}
