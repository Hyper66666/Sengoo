use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default)]
struct LockPackageEntry {
    source_kind: Option<String>,
    source_path: Option<String>,
    manifest: Option<String>,
    legacy_source: Option<String>,
}

fn find_lockfile_for_root(root: &Path) -> Option<PathBuf> {
    let direct = root.join("Sengoo.lock");
    if direct.is_file() {
        return Some(direct);
    }
    let mut current = root.to_path_buf();
    loop {
        let candidate = current.join("Sengoo.lock");
        if candidate.is_file() {
            return Some(candidate);
        }
        if !current.pop() {
            break;
        }
    }
    None
}

fn unescape_toml_string(value: &str) -> String {
    value.replace("\\\"", "\"").replace("\\\\", "\\")
}

fn parse_lockfile_packages(content: &str) -> Vec<LockPackageEntry> {
    let mut packages = Vec::new();
    let mut current = LockPackageEntry::default();
    let mut in_package = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[[package]]" {
            if in_package {
                packages.push(current);
            }
            current = LockPackageEntry::default();
            in_package = true;
            continue;
        }
        if !in_package {
            continue;
        }
        if let Some((key, value)) = trimmed.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            let value = unescape_toml_string(value);
            match key {
                "source" => current.legacy_source = Some(value),
                "source.kind" => current.source_kind = Some(value),
                "source.path" => current.source_path = Some(value),
                "manifest" => current.manifest = Some(value),
                _ => {}
            }
        }
    }
    if in_package {
        packages.push(current);
    }
    packages
}

fn package_root_from_entry(lockfile_dir: &Path, entry: &LockPackageEntry) -> Option<PathBuf> {
    if let (Some(kind), Some(manifest)) = (entry.source_kind.as_deref(), entry.manifest.as_ref()) {
        let manifest_path = lockfile_dir.join(manifest);
        match kind {
            "path" => {
                let source_path = entry.source_path.as_deref().unwrap_or(".");
                let root = lockfile_dir.join(source_path);
                if root.is_dir() {
                    Some(root)
                } else {
                    manifest_path.parent().map(Path::to_path_buf)
                }
            }
            "git" | "registry" => manifest_path.parent().map(Path::to_path_buf),
            _ => None,
        }
    } else if let Some(legacy_source) = entry.legacy_source.as_deref() {
        if let Some(path_suffix) = legacy_source.strip_prefix("path+") {
            let root = lockfile_dir.join(path_suffix);
            if root.is_dir() {
                return Some(root);
            }
        }
        entry
            .manifest
            .as_ref()
            .map(|manifest| lockfile_dir.join(manifest))
            .and_then(|manifest_path| manifest_path.parent().map(Path::to_path_buf))
    } else {
        entry
            .manifest
            .as_ref()
            .map(|manifest| lockfile_dir.join(manifest))
            .and_then(|manifest_path| manifest_path.parent().map(Path::to_path_buf))
    }
}

pub(crate) fn dependency_roots_for_workspace_roots(roots: &[PathBuf]) -> Vec<PathBuf> {
    let mut dependency_roots = BTreeSet::new();
    for root in roots {
        let Some(lockfile_path) = find_lockfile_for_root(root) else {
            continue;
        };
        let Ok(content) = fs::read_to_string(&lockfile_path) else {
            continue;
        };
        let lockfile_dir = lockfile_path.parent().unwrap_or_else(|| Path::new("."));
        let packages = parse_lockfile_packages(&content);
        let root_canonical = fs::canonicalize(root).unwrap_or_else(|_| root.clone());
        for entry in packages {
            let Some(package_root) = package_root_from_entry(lockfile_dir, &entry) else {
                continue;
            };
            let canonical = fs::canonicalize(&package_root).unwrap_or(package_root);
            if canonical == root_canonical {
                continue;
            }
            dependency_roots.insert(canonical);
        }
    }
    dependency_roots.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn dependency_roots_include_path_dependency_from_lockfile_v2() {
        let root = std::env::temp_dir().join(format!(
            "sglsp_dep_roots_v2_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let app = root.join("app");
        let dep = root.join("dep");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(dep.join("src")).unwrap();
        fs::write(
            root.join("Sengoo.lock"),
            r#"# generated
version = 2
root = "app"

[[package]]
id = "app@0.1.0+path+."
name = "app"
version = "0.1.0"
source.kind = "path"
source.path = "app"
manifest = "app/Sengoo.toml"

[[package]]
id = "dep@0.1.0+path+../dep"
name = "dep"
version = "0.1.0"
source.kind = "path"
source.path = "dep"
manifest = "dep/Sengoo.toml"
"#,
        )
        .unwrap();

        let roots = dependency_roots_for_workspace_roots(std::slice::from_ref(&app));
        let dep_canonical = fs::canonicalize(&dep).unwrap();
        assert!(
            roots
                .iter()
                .any(|item| fs::canonicalize(item).ok() == Some(dep_canonical.clone())),
            "expected path dependency root {dep_canonical:?}, got {roots:?}"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn dependency_roots_include_path_dependency_from_lockfile_v1() {
        let root = std::env::temp_dir().join(format!(
            "sglsp_dep_roots_v1_{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let app = root.join("app");
        let dep = root.join("dep");
        fs::create_dir_all(app.join("src")).unwrap();
        fs::create_dir_all(dep.join("src")).unwrap();
        fs::write(
            app.join("Sengoo.lock"),
            r#"version = 1
root = "app"

[[package]]
name = "dep"
version = "0.1.0"
source = "path+../dep"
manifest = "../dep/Sengoo.toml"

[[package]]
name = "app"
version = "0.1.0"
source = "path+."
manifest = "Sengoo.toml"
"#,
        )
        .unwrap();

        let roots = dependency_roots_for_workspace_roots(std::slice::from_ref(&app));
        let dep_canonical = fs::canonicalize(&dep).unwrap();
        assert!(
            roots
                .iter()
                .any(|item| fs::canonicalize(item).ok() == Some(dep_canonical.clone())),
            "expected path dependency root {dep_canonical:?}, got {roots:?}"
        );

        let _ = fs::remove_dir_all(root);
    }
}
