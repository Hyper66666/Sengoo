use crate::expand_stdlib_imports_for_source;
use miette::{Context, IntoDiagnostic, Result};
use sengoo_compiler::{DeclKind, Import, ImportKind, Parser, Path as AstPath};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) const MODULE_MAP_ENV: &str = "SENGOO_MODULE_MAP";

pub(crate) fn expand_local_imports_for_source(input_path: &Path, source: &str) -> Result<String> {
    let module_map = module_map_from_env()?;
    expand_local_imports_for_source_with_map(input_path, source, &module_map)
}

pub(crate) fn expand_imports_for_source(input_path: &Path, source: &str) -> Result<String> {
    let source = expand_local_imports_for_source(input_path, source)?;
    expand_stdlib_imports_for_source(&source)
}

fn expand_local_imports_for_source_with_map(
    input_path: &Path,
    source: &str,
    module_map: &BTreeMap<String, PathBuf>,
) -> Result<String> {
    let mut visiting = BTreeSet::new();
    let mut visited = BTreeSet::new();
    let mut sources = Vec::new();
    collect_imported_sources(
        input_path,
        source,
        module_map,
        &mut visiting,
        &mut visited,
        &mut sources,
    )?;
    Ok(sources.join("\n\n"))
}

fn collect_imported_sources(
    input_path: &Path,
    source: &str,
    module_map: &BTreeMap<String, PathBuf>,
    visiting: &mut BTreeSet<PathBuf>,
    visited: &mut BTreeSet<PathBuf>,
    sources: &mut Vec<String>,
) -> Result<()> {
    let key = fs::canonicalize(input_path).unwrap_or_else(|_| input_path.to_path_buf());
    if visited.contains(&key) {
        return Ok(());
    }
    if !visiting.insert(key.clone()) {
        miette::bail!("cyclic source import detected at {}", input_path.display());
    }

    if let Ok(program) = Parser::parse(source) {
        let source_dir = input_path.parent().unwrap_or(Path::new("."));
        for import_decl in program.decls.iter().filter_map(|decl| match &decl.kind {
            DeclKind::Import(import_decl) => Some(import_decl),
            _ => None,
        }) {
            let imported_path = match resolve_import_path(source_dir, &import_decl.path, module_map)
            {
                Some(imported_path) => imported_path,
                None if should_defer_source_import(import_decl) => continue,
                None => {
                    miette::bail!(
                        "unresolved source import '{}' from {}",
                        import_path_text(&import_decl.path),
                        input_path.display()
                    );
                }
            };
            let imported_source = fs::read_to_string(&imported_path)
                .into_diagnostic()
                .with_context(|| {
                    format!("failed to read imported module {}", imported_path.display())
                })?;
            collect_imported_sources(
                &imported_path,
                &imported_source,
                module_map,
                visiting,
                visited,
                sources,
            )?;
        }
    }

    visiting.remove(&key);
    visited.insert(key);
    sources.push(source.to_string());
    Ok(())
}

fn should_defer_source_import(import_decl: &Import) -> bool {
    let segments = import_decl
        .path
        .segments
        .iter()
        .map(|segment| segment.name.as_str())
        .collect::<Vec<_>>();
    if segments.first() == Some(&"std") {
        return true;
    }
    if segments.as_slice() == ["reflect"] {
        return true;
    }
    if segments.first() != Some(&"sengoo") {
        return false;
    }
    if segments.get(1) == Some(&"reflect") {
        return true;
    }
    segments.len() == 1
        && matches!(&import_decl.kind, ImportKind::Selective(names) if names
            .iter()
            .any(|name| name.name.eq_ignore_ascii_case("reflect")))
}

fn import_path_text(import_path: &AstPath) -> String {
    import_path
        .segments
        .iter()
        .map(|segment| segment.name.as_str())
        .collect::<Vec<_>>()
        .join("::")
}

pub(crate) fn resolve_import_path(
    source_dir: &Path,
    import_path: &AstPath,
    module_map: &BTreeMap<String, PathBuf>,
) -> Option<PathBuf> {
    resolve_candidates(
        source_dir,
        import_path
            .segments
            .iter()
            .map(|segment| segment.name.as_str()),
    )
    .into_iter()
    .find(|path| path.exists())
    .or_else(|| resolve_mapped_import(import_path, module_map))
}

fn resolve_mapped_import(
    import_path: &AstPath,
    module_map: &BTreeMap<String, PathBuf>,
) -> Option<PathBuf> {
    let first = import_path.segments.first()?;
    let entry_path = module_map.get(&first.name)?;
    if import_path.segments.len() == 1 {
        return entry_path.exists().then(|| entry_path.clone());
    }

    let source_dir = entry_path.parent()?;
    resolve_candidates(
        source_dir,
        import_path
            .segments
            .iter()
            .skip(1)
            .map(|segment| segment.name.as_str()),
    )
    .into_iter()
    .find(|path| path.exists())
}

fn resolve_candidates<'a>(
    source_dir: &Path,
    segments: impl Iterator<Item = &'a str>,
) -> Vec<PathBuf> {
    let mut joined = PathBuf::new();
    for segment in segments {
        joined.push(segment);
    }
    if joined.as_os_str().is_empty() {
        return Vec::new();
    }

    vec![
        source_dir.join(&joined).with_extension("sg"),
        source_dir.join(&joined).join("mod.sg"),
        source_dir.join(&joined).join("index.sg"),
    ]
}

pub(crate) fn module_map_from_env() -> Result<BTreeMap<String, PathBuf>> {
    let Some(raw) = env::var_os(MODULE_MAP_ENV) else {
        return Ok(BTreeMap::new());
    };

    let mut module_map = BTreeMap::new();
    for entry in env::split_paths(&raw) {
        let text = entry.to_string_lossy();
        let Some((name, path)) = text.split_once('=') else {
            miette::bail!(
                "{} entries must use <module>=<path>: {}",
                MODULE_MAP_ENV,
                text
            );
        };
        if name.trim().is_empty() || path.trim().is_empty() {
            miette::bail!(
                "{} entries must use non-empty <module>=<path>: {}",
                MODULE_MAP_ENV,
                text
            );
        }
        module_map.insert(name.to_string(), PathBuf::from(path));
    }
    Ok(module_map)
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
        let dir = env::temp_dir().join(format!("sgc_source_imports_{}_{}", name, stamp));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn expands_relative_imports_dependency_first() {
        let root = temp_dir("relative");
        let util = root.join("util.sg");
        fs::write(&util, "def imported_value() -> i64 { 42 }\n").unwrap();
        let main = root.join("main.sg");
        let source = "import util;\ndef main() -> i64 { imported_value() }\n";

        let expanded =
            expand_local_imports_for_source_with_map(&main, source, &BTreeMap::new()).unwrap();

        assert!(expanded.find("def imported_value").unwrap() < expanded.find("def main").unwrap());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expands_mapped_package_entry() {
        let root = temp_dir("mapped");
        let dep = root.join("dep/src/lib.sg");
        fs::create_dir_all(dep.parent().unwrap()).unwrap();
        fs::write(&dep, "def imported_value() -> i64 { 42 }\n").unwrap();
        let main = root.join("app/src/main.sg");
        let source = "import dep;\ndef main() -> i64 { imported_value() }\n";
        let module_map = BTreeMap::from([("dep".to_string(), dep)]);

        let expanded =
            expand_local_imports_for_source_with_map(&main, source, &module_map).unwrap();

        assert!(expanded.contains("def imported_value"));
        sengoo_compiler::compile_to_ir(&expanded).expect("mapped package symbol should compile");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_unresolved_source_import() {
        let root = temp_dir("unresolved");
        let main = root.join("main.sg");
        let source = "import missing;\ndef main() -> i64 { 0 }\n";

        let err = expand_local_imports_for_source_with_map(&main, source, &BTreeMap::new())
            .expect_err("missing import should be rejected");

        assert!(err
            .to_string()
            .contains("unresolved source import 'missing'"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_cyclic_source_import() {
        let root = temp_dir("cycle");
        let main = root.join("main.sg");
        let util = root.join("util.sg");
        fs::write(&main, "import util;\ndef main() -> i64 { 0 }\n").unwrap();
        fs::write(&util, "import main;\ndef util_value() -> i64 { 1 }\n").unwrap();
        let source = fs::read_to_string(&main).unwrap();

        let err = expand_local_imports_for_source_with_map(&main, &source, &BTreeMap::new())
            .expect_err("cyclic import should be rejected");

        assert!(err.to_string().contains("cyclic source import detected"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn expands_mapped_package_nested_module() {
        let root = temp_dir("mapped_nested");
        let dep = root.join("dep/src/lib.sg");
        let nested = root.join("dep/src/math.sg");
        fs::create_dir_all(dep.parent().unwrap()).unwrap();
        fs::write(&dep, "def imported_value() -> i64 { 42 }\n").unwrap();
        fs::write(&nested, "def nested_value() -> i64 { 7 }\n").unwrap();
        let main = root.join("app/src/main.sg");
        let source = "import dep::math;\ndef main() -> i64 { nested_value() }\n";
        let module_map = BTreeMap::from([("dep".to_string(), dep)]);

        let expanded =
            expand_local_imports_for_source_with_map(&main, source, &module_map).unwrap();

        assert!(expanded.contains("def nested_value"));
        sengoo_compiler::compile_to_ir(&expanded).expect("mapped nested symbol should compile");
        let _ = fs::remove_dir_all(root);
    }
}
