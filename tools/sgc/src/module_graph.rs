use sengoo_compiler::{DeclKind, Parser, Path as AstPath, Program};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::{canonical_or_lossy, decl_requests_reflection, ModuleSourceInfo};

fn resolve_import_candidates(source_dir: &Path, import_path: &AstPath) -> Vec<PathBuf> {
    if import_path.segments.is_empty() {
        return Vec::new();
    }

    let mut joined = PathBuf::new();
    for seg in &import_path.segments {
        joined.push(&seg.name);
    }

    let mut candidates = vec![
        source_dir.join(&joined).with_extension("sg"),
        source_dir.join(&joined).join("mod.sg"),
        source_dir.join(&joined).join("index.sg"),
    ];
    candidates.dedup();
    candidates
}

fn resolve_direct_import_dependencies_from_program(source_dir: &Path, program: &Program) -> Vec<PathBuf> {
    let mut deps = program
        .decls
        .iter()
        .filter_map(|decl| match &decl.kind {
            DeclKind::Import(import_decl) => Some(import_decl),
            _ => None,
        })
        .filter_map(|import_decl| {
            resolve_import_candidates(source_dir, &import_decl.path)
                .into_iter()
                .find(|p| p.exists())
        })
        .map(|path| fs::canonicalize(&path).unwrap_or(path))
        .collect::<Vec<_>>();
    deps.sort();
    deps.dedup();
    deps
}

fn resolve_direct_import_metadata(source_dir: &Path, source: &str) -> (Vec<PathBuf>, bool) {
    if !source.contains("import") {
        return (Vec::new(), false);
    }

    let program = match Parser::parse(source) {
        Ok(program) => program,
        Err(_) => return (Vec::new(), false),
    };

    let deps = resolve_direct_import_dependencies_from_program(source_dir, &program);
    let requests_reflection = program.decls.iter().any(decl_requests_reflection);
    (deps, requests_reflection)
}

pub(crate) fn collect_module_sources_with_edges(
    input_path: &Path,
    root_source: &str,
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
        let (deps, requests_reflection) = resolve_direct_import_metadata(source_dir, source.as_ref());
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
pub(crate) fn module_dependency_levels(dependency_edges: &BTreeMap<String, Vec<String>>) -> Vec<Vec<String>> {
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
