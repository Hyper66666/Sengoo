use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::{
    canonical_or_lossy, BuildGraphNodeV2, BuildGraphV2, FunctionFingerprint,
    GenericInstanceFingerprint, GenericItemFingerprint, ModuleFingerprint,
    BUILD_GRAPH_SCHEMA_VERSION,
};

pub(crate) fn build_graph_v2_for_source(
    input_path: &Path,
    module_fingerprints: &[ModuleFingerprint],
    dependency_edges: &BTreeMap<String, Vec<String>>,
    root_object_path: Option<&Path>,
    root_interface_hash: u64,
    root_implementation_hash: u64,
) -> BuildGraphV2 {
    let root_module = canonical_or_lossy(input_path);
    let mut fingerprint_map = module_fingerprints
        .iter()
        .map(|fp| (fp.path.clone(), (fp.interface_hash, fp.hash)))
        .collect::<HashMap<_, _>>();
    fingerprint_map.insert(
        root_module.clone(),
        (root_interface_hash, root_implementation_hash),
    );

    let mut all_paths = HashSet::<String>::new();
    all_paths.insert(root_module.clone());
    all_paths.extend(module_fingerprints.iter().map(|fp| fp.path.clone()));
    for (path, deps) in dependency_edges {
        all_paths.insert(path.clone());
        all_paths.extend(deps.iter().cloned());
    }

    let mut node_paths = all_paths.into_iter().collect::<Vec<_>>();
    node_paths.sort();

    let mut nodes = Vec::with_capacity(node_paths.len());
    for path in node_paths {
        let (interface_hash, implementation_hash) =
            fingerprint_map.get(&path).copied().unwrap_or_default();
        let mut depends_on = dependency_edges.get(&path).cloned().unwrap_or_default();
        depends_on.sort();
        depends_on.dedup();
        nodes.push(BuildGraphNodeV2 {
            module_path: path.clone(),
            interface_hash,
            implementation_hash,
            depends_on,
            object_path: if path == root_module {
                root_object_path.map(canonical_or_lossy)
            } else {
                None
            },
            functions: Vec::new(),
            generic_items: Vec::new(),
            generic_instances: Vec::new(),
        });
    }

    BuildGraphV2 {
        schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        root_module,
        nodes,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_graph_v2_with_function_fingerprints_for_source(
    input_path: &Path,
    module_fingerprints: &[ModuleFingerprint],
    module_function_fingerprints: &BTreeMap<String, Vec<FunctionFingerprint>>,
    module_generic_items: &BTreeMap<String, Vec<GenericItemFingerprint>>,
    module_generic_instances: &BTreeMap<String, Vec<GenericInstanceFingerprint>>,
    dependency_edges: &BTreeMap<String, Vec<String>>,
    root_object_path: Option<&Path>,
    root_interface_hash: u64,
    root_implementation_hash: u64,
) -> BuildGraphV2 {
    let mut graph = build_graph_v2_for_source(
        input_path,
        module_fingerprints,
        dependency_edges,
        root_object_path,
        root_interface_hash,
        root_implementation_hash,
    );

    for node in &mut graph.nodes {
        let mut functions = module_function_fingerprints
            .get(&node.module_path)
            .cloned()
            .unwrap_or_default();
        for function in &mut functions {
            if function.module_imports.is_empty() {
                function.module_imports = node.depends_on.clone();
            }
        }
        functions.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        node.functions = functions;

        let mut generic_items = module_generic_items
            .get(&node.module_path)
            .cloned()
            .unwrap_or_default();
        generic_items.sort_by(|a, b| a.stable_item_id.cmp(&b.stable_item_id));
        generic_items.dedup_by(|a, b| a.stable_item_id == b.stable_item_id);
        node.generic_items = generic_items;

        let mut generic_instances = module_generic_instances
            .get(&node.module_path)
            .cloned()
            .unwrap_or_default();
        generic_instances.sort_by(|a, b| a.instance_key.cmp(&b.instance_key));
        generic_instances.dedup_by(|a, b| a.instance_key == b.instance_key);
        node.generic_instances = generic_instances;
    }

    graph
}
