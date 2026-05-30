use sengoo_compiler::{DeclKind, Parser, Program};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::source_imports::{module_map_from_env, resolve_import_path};
use crate::{canonical_or_lossy, decl_requests_reflection, ModuleSourceInfo};

fn resolve_direct_import_dependencies_from_program(
    source_dir: &Path,
    program: &Program,
    module_map: &BTreeMap<String, PathBuf>,
) -> Vec<PathBuf> {
    let mut deps = program
        .decls
        .iter()
        .filter_map(|decl| match &decl.kind {
            DeclKind::Import(import_decl) => Some(import_decl),
            _ => None,
        })
        .filter_map(|import_decl| resolve_import_path(source_dir, &import_decl.path, module_map))
        .map(|path| fs::canonicalize(&path).unwrap_or(path))
        .collect::<Vec<_>>();
    deps.sort();
    deps.dedup();
    deps
}

fn resolve_direct_import_metadata(
    source_dir: &Path,
    source: &str,
    module_map: &BTreeMap<String, PathBuf>,
) -> (Vec<PathBuf>, bool) {
    if !source.contains("import") {
        return (Vec::new(), false);
    }

    let program = match Parser::parse(source) {
        Ok(program) => program,
        Err(_) => return (Vec::new(), false),
    };

    let deps = resolve_direct_import_dependencies_from_program(source_dir, &program, module_map);
    let requests_reflection = program.decls.iter().any(decl_requests_reflection);
    (deps, requests_reflection)
}

pub(crate) fn collect_module_sources_with_edges(
    input_path: &Path,
    root_source: &str,
) -> BTreeMap<String, ModuleSourceInfo> {
    let module_map = module_map_from_env().unwrap_or_default();
    collect_module_sources_with_edges_from_map(input_path, root_source, &module_map)
}

fn collect_module_sources_with_edges_from_map(
    input_path: &Path,
    root_source: &str,
    module_map: &BTreeMap<String, PathBuf>,
) -> BTreeMap<String, ModuleSourceInfo> {
    let root_path = fs::canonicalize(input_path).unwrap_or_else(|_| input_path.to_path_buf());
    let mut queue = vec![(root_path, Arc::<str>::from(root_source))];
    let mut sources = BTreeMap::new();

    while let Some((module_path, source)) = queue.pop() {
        let module_key = canonical_or_lossy(&module_path);
        if sources.contains_key(&module_key) {
            continue;
        }

        let source_dir = module_path.parent().unwrap_or(Path::new("."));
        let (deps, requests_reflection) =
            resolve_direct_import_metadata(source_dir, source.as_ref(), module_map);
        let mut dep_keys = deps
            .iter()
            .map(|dep| canonical_or_lossy(dep))
            .collect::<Vec<_>>();
        dep_keys.sort();
        dep_keys.dedup();

        sources.insert(
            module_key.clone(),
            ModuleSourceInfo {
                source: Arc::clone(&source),
                depends_on: dep_keys,
                requests_reflection,
            },
        );

        for dep in deps.into_iter().rev() {
            if let Ok(dep_source) = fs::read_to_string(&dep) {
                queue.push((dep, Arc::<str>::from(dep_source)));
            }
        }
    }

    sources
}

#[allow(dead_code)]
pub(crate) fn module_dependency_levels(
    dependency_edges: &BTreeMap<String, Vec<String>>,
) -> Vec<Vec<String>> {
    let mut indegree = HashMap::<String, usize>::new();
    let mut reverse = HashMap::<String, Vec<String>>::new();

    for node in dependency_edges.keys() {
        indegree.entry(node.clone()).or_insert(0);
    }

    for (node, deps) in dependency_edges {
        let mut unique = deps.clone();
        unique.sort();
        unique.dedup();

        let dep_count = unique
            .iter()
            .filter(|dep| dependency_edges.contains_key(dep.as_str()))
            .count();
        indegree.insert(node.clone(), dep_count);

        for dep in unique {
            indegree.entry(dep.clone()).or_insert(0);
            reverse.entry(dep).or_default().push(node.clone());
        }
    }

    for dependents in reverse.values_mut() {
        dependents.sort();
        dependents.dedup();
    }

    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(node, _)| node.clone())
        .collect::<Vec<_>>();
    ready.sort();
    ready.dedup();

    let mut levels = Vec::new();
    let mut processed = HashSet::<String>::new();

    while !ready.is_empty() {
        let batch = ready.clone();
        ready.clear();
        levels.push(batch.clone());

        for node in batch {
            processed.insert(node.clone());
            if let Some(dependents) = reverse.get(&node) {
                for dependent in dependents {
                    if let Some(degree) = indegree.get_mut(dependent) {
                        if *degree > 0 {
                            *degree -= 1;
                        }
                        if *degree == 0 && !processed.contains(dependent) {
                            ready.push(dependent.clone());
                        }
                    }
                }
            }
        }

        ready.sort();
        ready.dedup();
    }

    let mut unresolved = indegree
        .iter()
        .filter_map(|(node, degree)| {
            if *degree > 0 && !processed.contains(node) {
                Some(node.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if !unresolved.is_empty() {
        unresolved.sort();
        levels.push(unresolved);
    }

    levels
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("sgc_module_graph_{}_{}", name, stamp));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn mapped_package_participates_in_graph_and_reflection_detection() {
        let root = temp_dir("mapped");
        let dep = root.join("dep/src/lib.sg");
        let main = root.join("app/src/main.sg");
        fs::create_dir_all(dep.parent().unwrap()).unwrap();
        fs::create_dir_all(main.parent().unwrap()).unwrap();
        fs::write(
            &dep,
            "import std::reflect;\ndef imported_value() -> i64 { 42 }\n",
        )
        .unwrap();
        let source = "import dep;\ndef main() -> i64 { imported_value() }\n";
        fs::write(&main, source).unwrap();
        let module_map = BTreeMap::from([("dep".to_string(), dep.clone())]);

        let sources = collect_module_sources_with_edges_from_map(&main, source, &module_map);
        let main_id = canonical_or_lossy(&main);
        let dep_id = canonical_or_lossy(&dep);

        assert_eq!(sources[&main_id].depends_on, vec![dep_id.clone()]);
        assert!(sources[&dep_id].requests_reflection);
        let _ = fs::remove_dir_all(root);
    }
}
