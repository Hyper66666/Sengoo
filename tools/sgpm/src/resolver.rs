use crate::manifest::Manifest;
use miette::{Context, IntoDiagnostic, Result};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PackageNode {
    pub name: String,
    pub manifest_path: PathBuf,
    pub root_dir: PathBuf,
    pub entry_path: PathBuf,
    pub manifest: Manifest,
}

#[derive(Debug, Clone)]
pub struct Graph {
    pub root: PathBuf,
    pub nodes: Vec<PackageNode>,
}

impl Graph {
    pub fn from_root(manifest_path: &Path) -> Result<Self> {
        let root_manifest = resolve_manifest_path(manifest_path)?;
        let mut builder = GraphBuilder::default();
        builder.visit(&root_manifest)?;
        Ok(Self {
            root: root_manifest,
            nodes: builder.nodes,
        })
    }

    pub fn root_package(&self) -> Option<&PackageNode> {
        self.nodes
            .iter()
            .find(|node| node.manifest_path == self.root)
    }
}

#[derive(Default)]
struct GraphBuilder {
    visiting: BTreeSet<PathBuf>,
    visited: BTreeSet<PathBuf>,
    stack: Vec<PathBuf>,
    nodes: Vec<PackageNode>,
}

impl GraphBuilder {
    fn visit(&mut self, manifest_path: &Path) -> Result<()> {
        let key = canonicalize_existing(manifest_path)?;
        if self.visited.contains(&key) {
            return Ok(());
        }
        if self.visiting.contains(&key) {
            let mut cycle = self.stack.clone();
            cycle.push(key);
            let rendered = cycle
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(" -> ");
            miette::bail!("cyclic path dependency detected: {}", rendered);
        }

        self.visiting.insert(key.clone());
        self.stack.push(key.clone());

        let manifest = Manifest::load(&key)?;
        let root_dir = key
            .parent()
            .ok_or_else(|| miette::miette!("manifest has no parent directory: {}", key.display()))?
            .to_path_buf();

        for dep in manifest.dependencies.values() {
            let dep_manifest = resolve_dependency_manifest(&root_dir, &dep.path)
                .with_context(|| format!("failed to resolve dependency '{}'", dep.name))?;
            self.visit(&dep_manifest)?;
        }

        let entry_path = root_dir.join(manifest.entry_path());
        self.nodes.push(PackageNode {
            name: manifest.package.name.clone(),
            manifest_path: key.clone(),
            root_dir,
            entry_path,
            manifest,
        });

        self.stack.pop();
        self.visiting.remove(&key);
        self.visited.insert(key);
        Ok(())
    }
}

pub fn resolve_manifest_path(path: &Path) -> Result<PathBuf> {
    let candidate = if path.is_dir() {
        path.join("Sengoo.toml")
    } else {
        path.to_path_buf()
    };
    canonicalize_existing(&candidate)
}

fn resolve_dependency_manifest(parent_dir: &Path, dep_path: &Path) -> Result<PathBuf> {
    let joined = if dep_path.is_absolute() {
        dep_path.to_path_buf()
    } else {
        parent_dir.join(dep_path)
    };
    let manifest = if joined.is_dir() {
        joined.join("Sengoo.toml")
    } else {
        joined
    };
    canonicalize_existing(&manifest)
}

fn canonicalize_existing(path: &Path) -> Result<PathBuf> {
    fs::canonicalize(path)
        .into_diagnostic()
        .with_context(|| format!("path not found: {}", path.display()))
}

pub fn render_tree(graph: &Graph) -> String {
    graph
        .nodes
        .iter()
        .map(|node| {
            format!(
                "{} v{} {}",
                node.name,
                node.manifest.package.version,
                node.root_dir.display()
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
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
        let dir = std::env::temp_dir().join(format!("sgpm_resolver_{}_{}", name, stamp));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn write_pkg(root: &Path, name: &str, deps: &[(&str, &str)]) {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.sg"), "def main() -> i64 { 0 }\n").unwrap();
        let mut text = format!("[package]\nname = '{}'\nversion = '0.1.0'\n\n", name);
        if !deps.is_empty() {
            text.push_str("[dependencies]\n");
            for (dep_name, dep_path) in deps {
                text.push_str(&format!(
                    "{} = {{ path = '{}' }}\n",
                    dep_name,
                    dep_path.replace('\\', "\\\\")
                ));
            }
        }
        fs::write(root.join("Sengoo.toml"), text).unwrap();
    }

    #[test]
    fn resolves_topological_order_three_packages() {
        let dir = temp_dir("chain");
        let a = dir.join("a");
        let b = dir.join("b");
        let c = dir.join("c");
        write_pkg(&c, "c", &[]);
        write_pkg(&b, "b", &[("c", "../c")]);
        write_pkg(&a, "a", &[("b", "../b")]);

        let graph = Graph::from_root(&a.join("Sengoo.toml")).unwrap();
        let names = graph
            .nodes
            .iter()
            .map(|n| n.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["c", "b", "a"]);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn resolves_diamond_once() {
        let dir = temp_dir("diamond");
        let a = dir.join("a");
        let b = dir.join("b");
        let c = dir.join("c");
        let d = dir.join("d");
        write_pkg(&d, "d", &[]);
        write_pkg(&b, "b", &[("d", "../d")]);
        write_pkg(&c, "c", &[("d", "../d")]);
        write_pkg(&a, "a", &[("b", "../b"), ("c", "../c")]);

        let graph = Graph::from_root(&a.join("Sengoo.toml")).unwrap();
        let names = graph
            .nodes
            .iter()
            .map(|n| n.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["d", "b", "c", "a"]);
        assert_eq!(names.iter().filter(|name| **name == "d").count(), 1);
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_self_loop() {
        let dir = temp_dir("self_loop");
        let a = dir.join("a");
        write_pkg(&a, "a", &[("a", ".")]);

        let err = Graph::from_root(&a.join("Sengoo.toml")).unwrap_err();
        assert!(err.to_string().contains("cyclic path dependency"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_cyclic_path_deps() {
        let dir = temp_dir("cycle");
        let a = dir.join("a");
        let b = dir.join("b");
        write_pkg(&a, "a", &[("b", "../b")]);
        write_pkg(&b, "b", &[("a", "../a")]);

        let err = Graph::from_root(&a.join("Sengoo.toml")).unwrap_err();
        assert!(err.to_string().contains("cyclic path dependency"));
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_three_cycle() {
        let dir = temp_dir("three_cycle");
        let a = dir.join("a");
        let b = dir.join("b");
        let c = dir.join("c");
        write_pkg(&a, "a", &[("b", "../b")]);
        write_pkg(&b, "b", &[("c", "../c")]);
        write_pkg(&c, "c", &[("a", "../a")]);

        let err = Graph::from_root(&a.join("Sengoo.toml")).unwrap_err();
        assert!(err.to_string().contains("cyclic path dependency"));
        let _ = fs::remove_dir_all(dir);
    }
}
