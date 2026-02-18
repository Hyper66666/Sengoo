use std::collections::{HashMap, HashSet};

use crate::{
    BuildGraphV2, EditClass, EditImpact, FunctionChangeKind, FunctionFingerprint,
    IncrementalLinkMode, ModuleChangeKind, ModuleFingerprint, ModuleInvalidationStats,
};
use crate::symbol_intern::SymbolInterner;

pub(crate) fn module_invalidation_stats(
    before: &[ModuleFingerprint],
    after: &[ModuleFingerprint],
) -> ModuleInvalidationStats {
    let before_map: HashMap<&str, &ModuleFingerprint> =
        before.iter().map(|fp| (fp.path.as_str(), fp)).collect();
    let after_map: HashMap<&str, &ModuleFingerprint> =
        after.iter().map(|fp| (fp.path.as_str(), fp)).collect();

    let mut all_paths = HashSet::new();
    all_paths.extend(before_map.keys().copied());
    all_paths.extend(after_map.keys().copied());

    let mut stats = ModuleInvalidationStats {
        total_modules: all_paths.len() as u32,
        ..Default::default()
    };

    for path in all_paths {
        match (before_map.get(path), after_map.get(path)) {
            (Some(old), Some(new)) => {
                if old.hash == new.hash && old.interface_hash == new.interface_hash {
                    stats.reused_modules += 1;
                } else if old.interface_hash != new.interface_hash {
                    stats.rebuilt_modules += 1;
                    stats.interface_changed_modules += 1;
                } else {
                    stats.rebuilt_modules += 1;
                    stats.implementation_only_changed_modules += 1;
                }
            }
            _ => {
                stats.rebuilt_modules += 1;
                stats.interface_changed_modules += 1;
            }
        }
    }

    stats
}

fn collect_module_changes(
    before: &[ModuleFingerprint],
    after: &[ModuleFingerprint],
) -> Vec<(String, ModuleChangeKind)> {
    let before_map: HashMap<&str, &ModuleFingerprint> =
        before.iter().map(|fp| (fp.path.as_str(), fp)).collect();
    let after_map: HashMap<&str, &ModuleFingerprint> =
        after.iter().map(|fp| (fp.path.as_str(), fp)).collect();

    let mut all_paths = HashSet::new();
    all_paths.extend(before_map.keys().copied());
    all_paths.extend(after_map.keys().copied());

    let mut changes = Vec::new();
    for path in all_paths {
        let change = match (before_map.get(path), after_map.get(path)) {
            (Some(old), Some(new))
                if old.hash == new.hash && old.interface_hash == new.interface_hash =>
            {
                None
            }
            (Some(old), Some(new)) if old.interface_hash == new.interface_hash => {
                Some(ModuleChangeKind::ImplOnly)
            }
            (Some(_), Some(_)) => Some(ModuleChangeKind::Interface),
            _ => Some(ModuleChangeKind::Interface),
        };

        if let Some(kind) = change {
            changes.push((path.to_string(), kind));
        }
    }

    changes
}

fn add_reverse_edges(graph: &BuildGraphV2, reverse: &mut HashMap<String, HashSet<String>>) {
    for node in &graph.nodes {
        for dep in &node.depends_on {
            reverse
                .entry(dep.clone())
                .or_default()
                .insert(node.module_path.clone());
        }
    }
}

#[derive(Debug, Clone)]
struct FunctionNodeState {
    module_path: String,
    abi_hash: u64,
    body_hash: u64,
}

fn collect_function_state(graph: &BuildGraphV2) -> HashMap<String, FunctionNodeState> {
    let mut out = HashMap::new();
    for node in &graph.nodes {
        for function in &node.functions {
            out.insert(
                function.symbol.clone(),
                FunctionNodeState {
                    module_path: node.module_path.clone(),
                    abi_hash: function.abi_hash,
                    body_hash: function.body_hash,
                },
            );
        }
    }
    out
}

fn collect_function_changes(
    previous_graph: Option<&BuildGraphV2>,
    current_graph: &BuildGraphV2,
) -> Vec<(String, FunctionChangeKind)> {
    let previous = previous_graph
        .map(collect_function_state)
        .unwrap_or_default();
    let current = collect_function_state(current_graph);

    let mut all_symbols = HashSet::new();
    all_symbols.extend(previous.keys().cloned());
    all_symbols.extend(current.keys().cloned());

    let mut changes = Vec::new();
    for symbol in all_symbols {
        let change = match (previous.get(&symbol), current.get(&symbol)) {
            (Some(prev), Some(curr))
                if prev.abi_hash == curr.abi_hash && prev.body_hash == curr.body_hash =>
            {
                None
            }
            (Some(prev), Some(curr)) if prev.abi_hash == curr.abi_hash => {
                Some(FunctionChangeKind::ImplOnly)
            }
            (Some(_), Some(_)) => Some(FunctionChangeKind::Interface),
            _ => Some(FunctionChangeKind::Interface),
        };
        if let Some(kind) = change {
            changes.push((symbol, kind));
        }
    }

    changes.sort_by(|a, b| a.0.cmp(&b.0));
    changes
}

#[allow(dead_code)]
pub(crate) fn collect_impl_only_impacted_symbols(
    previous_symbols: &[FunctionFingerprint],
    current_symbols: &[FunctionFingerprint],
) -> Vec<String> {
    collect_impl_only_impacted_symbols_with_fallback(previous_symbols, current_symbols).0
}

pub(crate) fn collect_impl_only_impacted_symbols_with_fallback(
    previous_symbols: &[FunctionFingerprint],
    current_symbols: &[FunctionFingerprint],
) -> (Vec<String>, Option<String>) {
    let mut interner = SymbolInterner::default();
    for function in previous_symbols.iter().chain(current_symbols.iter()) {
        interner.intern(&function.symbol);
        for callee in &function.calls {
            interner.intern(callee);
        }
    }

    let previous_map = previous_symbols
        .iter()
        .map(|function| (interner.intern(&function.symbol), function))
        .collect::<HashMap<_, _>>();
    let current_map = current_symbols
        .iter()
        .map(|function| (interner.intern(&function.symbol), function))
        .collect::<HashMap<_, _>>();

    let mut all_symbols = HashSet::<u32>::new();
    all_symbols.extend(previous_map.keys().copied());
    all_symbols.extend(current_map.keys().copied());

    let mut changed_symbols = Vec::<u32>::new();
    let mut fallback_to_full = false;

    for symbol_id in all_symbols {
        match (previous_map.get(&symbol_id), current_map.get(&symbol_id)) {
            (Some(previous), Some(current))
                if previous.abi_hash == current.abi_hash
                    && previous.body_hash == current.body_hash => {}
            (Some(previous), Some(current)) if previous.abi_hash == current.abi_hash => {
                changed_symbols.push(symbol_id);
            }
            _ => {
                fallback_to_full = true;
                break;
            }
        }
    }

    if fallback_to_full {
        let mut all_current = current_map
            .keys()
            .copied()
            .map(|id| interner.resolve(id).to_string())
            .collect::<Vec<_>>();
        all_current.sort();
        all_current.dedup();
        return (
            all_current,
            Some("symbol signature drift detected, escalate to module scope".to_string()),
        );
    }

    changed_symbols.sort();
    changed_symbols.dedup();
    if changed_symbols.is_empty() {
        return (Vec::new(), None);
    }

    let current_symbol_set = current_map.keys().copied().collect::<HashSet<_>>();
    let mut reverse_calls = HashMap::<u32, HashSet<u32>>::new();
    for function in previous_symbols.iter().chain(current_symbols.iter()) {
        let caller_id = interner.intern(&function.symbol);
        for callee in &function.calls {
            let callee_id = interner.intern(callee);
            reverse_calls
                .entry(callee_id)
                .or_default()
                .insert(caller_id);
        }
    }

    let mut impacted = changed_symbols.clone();
    let mut queue = changed_symbols;
    let mut seen = impacted.iter().copied().collect::<HashSet<_>>();

    while let Some(symbol_id) = queue.pop() {
        if let Some(callers) = reverse_calls.get(&symbol_id) {
            let mut sorted = callers.iter().copied().collect::<Vec<_>>();
            sorted.sort();
            for caller_id in sorted {
                if !current_symbol_set.contains(&caller_id) {
                    continue;
                }
                if seen.insert(caller_id) {
                    impacted.push(caller_id);
                    queue.push(caller_id);
                }
            }
        }
    }

    let mut impacted_symbols = impacted
        .into_iter()
        .map(|id| interner.resolve(id).to_string())
        .collect::<Vec<_>>();
    impacted_symbols.sort();
    impacted_symbols.dedup();
    (impacted_symbols, None)
}

fn add_reverse_call_edges(graph: &BuildGraphV2, reverse: &mut HashMap<String, HashSet<String>>) {
    for node in &graph.nodes {
        for function in &node.functions {
            for callee in &function.calls {
                reverse
                    .entry(callee.clone())
                    .or_default()
                    .insert(function.symbol.clone());
            }
        }
    }
}

pub(crate) fn edit_class_label(class: EditClass) -> &'static str {
    match class {
        EditClass::Noop => "noop",
        EditClass::ImplOnly => "impl_only",
        EditClass::InterfaceChange => "interface_change",
    }
}

pub(crate) fn incremental_link_mode_from_env() -> IncrementalLinkMode {
    match std::env::var("SENGOO_INCREMENTAL_LINK") {
        Ok(raw) => match raw.trim().to_ascii_lowercase().as_str() {
            "off" | "false" | "0" => IncrementalLinkMode::Off,
            _ => IncrementalLinkMode::Auto,
        },
        Err(_) => IncrementalLinkMode::Auto,
    }
}

pub(crate) fn classify_edit_impact(
    previous_root_interface_hash: u64,
    previous_root_implementation_hash: u64,
    root_interface_hash: u64,
    root_implementation_hash: u64,
    before_modules: &[ModuleFingerprint],
    after_modules: &[ModuleFingerprint],
    previous_graph: Option<&BuildGraphV2>,
    current_graph: &BuildGraphV2,
) -> EditImpact {
    let mut module_changes: Vec<(String, ModuleChangeKind)> = Vec::new();

    if previous_root_interface_hash == 0
        && previous_root_implementation_hash == 0
        && (root_interface_hash != 0 || root_implementation_hash != 0)
    {
        module_changes.push((
            current_graph.root_module.clone(),
            ModuleChangeKind::Interface,
        ));
    } else if previous_root_interface_hash != root_interface_hash {
        module_changes.push((
            current_graph.root_module.clone(),
            ModuleChangeKind::Interface,
        ));
    } else if previous_root_implementation_hash != root_implementation_hash {
        module_changes.push((
            current_graph.root_module.clone(),
            ModuleChangeKind::ImplOnly,
        ));
    }

    module_changes.extend(collect_module_changes(before_modules, after_modules));
    module_changes.sort_by(|a, b| a.0.cmp(&b.0));
    module_changes.dedup_by(|a, b| a.0 == b.0);

    let function_changes = collect_function_changes(previous_graph, current_graph);

    let has_interface_change = module_changes
        .iter()
        .any(|(_, kind)| matches!(kind, ModuleChangeKind::Interface))
        || function_changes
            .iter()
            .any(|(_, kind)| matches!(kind, FunctionChangeKind::Interface));

    let class = if module_changes.is_empty() && function_changes.is_empty() {
        EditClass::Noop
    } else if has_interface_change {
        EditClass::InterfaceChange
    } else {
        EditClass::ImplOnly
    };

    let mut changed_modules = module_changes
        .iter()
        .map(|(path, _)| path.clone())
        .collect::<Vec<_>>();
    changed_modules.sort();
    changed_modules.dedup();

    let mut changed_functions = function_changes
        .iter()
        .map(|(symbol, _)| symbol.clone())
        .collect::<Vec<_>>();
    changed_functions.sort();
    changed_functions.dedup();

    let mut impacted_modules = changed_modules.clone();
    let mut impacted_functions = changed_functions.clone();

    if matches!(class, EditClass::InterfaceChange) {
        let mut reverse_modules: HashMap<String, HashSet<String>> = HashMap::new();
        add_reverse_edges(current_graph, &mut reverse_modules);
        if let Some(previous_graph) = previous_graph {
            add_reverse_edges(previous_graph, &mut reverse_modules);
        }

        let mut queue = module_changes
            .iter()
            .filter_map(|(path, kind)| {
                if matches!(kind, ModuleChangeKind::Interface) {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut seen: HashSet<String> = impacted_modules.iter().cloned().collect();

        while let Some(node) = queue.pop() {
            if let Some(parents) = reverse_modules.get(&node) {
                let mut sorted = parents.iter().cloned().collect::<Vec<_>>();
                sorted.sort();
                for parent in sorted {
                    if seen.insert(parent.clone()) {
                        impacted_modules.push(parent.clone());
                        queue.push(parent);
                    }
                }
            }
        }

        let mut reverse_calls: HashMap<String, HashSet<String>> = HashMap::new();
        add_reverse_call_edges(current_graph, &mut reverse_calls);
        if let Some(previous_graph) = previous_graph {
            add_reverse_call_edges(previous_graph, &mut reverse_calls);
        }

        let mut function_queue = function_changes
            .iter()
            .filter_map(|(symbol, kind)| {
                if matches!(kind, FunctionChangeKind::Interface) {
                    Some(symbol.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let mut seen_functions: HashSet<String> = impacted_functions.iter().cloned().collect();

        while let Some(symbol) = function_queue.pop() {
            if let Some(callers) = reverse_calls.get(&symbol) {
                let mut sorted = callers.iter().cloned().collect::<Vec<_>>();
                sorted.sort();
                for caller in sorted {
                    if seen_functions.insert(caller.clone()) {
                        impacted_functions.push(caller.clone());
                        function_queue.push(caller);
                    }
                }
            }
        }
    }

    impacted_modules.sort();
    impacted_modules.dedup();
    impacted_functions.sort();
    impacted_functions.dedup();

    let mut function_to_module = HashMap::<String, String>::new();
    if let Some(previous_graph) = previous_graph {
        for (symbol, state) in collect_function_state(previous_graph) {
            function_to_module
                .entry(symbol)
                .or_insert(state.module_path);
        }
    }
    for (symbol, state) in collect_function_state(current_graph) {
        function_to_module.insert(symbol, state.module_path);
    }

    for symbol in &impacted_functions {
        if let Some(module_path) = function_to_module.get(symbol) {
            impacted_modules.push(module_path.clone());
        }
    }
    impacted_modules.sort();
    impacted_modules.dedup();

    EditImpact {
        class,
        changed_modules,
        impacted_modules,
        changed_functions,
        impacted_functions,
    }
}

pub(crate) fn format_edit_impact_lines(impact: &EditImpact) -> Vec<String> {
    let mut lines = Vec::new();
    lines.push(format!(
        "edit classification: {}",
        edit_class_label(impact.class)
    ));
    if !impact.changed_modules.is_empty() {
        lines.push(format!(
            "changed modules: {}",
            impact.changed_modules.join(", ")
        ));
    }
    if !impact.impacted_modules.is_empty() {
        lines.push(format!(
            "impacted modules: {}",
            impact.impacted_modules.join(", ")
        ));
    }
    if !impact.changed_functions.is_empty() {
        lines.push(format!(
            "changed functions: {}",
            impact.changed_functions.join(", ")
        ));
    }
    if !impact.impacted_functions.is_empty() {
        lines.push(format!(
            "impacted functions: {}",
            impact.impacted_functions.join(", ")
        ));
    }
    lines
}
