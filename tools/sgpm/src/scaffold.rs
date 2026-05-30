use crate::manifest::validate_package_name;
use miette::{Context, IntoDiagnostic, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProjectKind {
    Binary,
    Library,
}

#[cfg(test)]
pub fn new_project(name: &str, path: Option<&Path>) -> Result<PathBuf> {
    new_project_with_kind(name, path, ProjectKind::Binary)
}

pub fn new_project_with_kind(
    name: &str,
    path: Option<&Path>,
    kind: ProjectKind,
) -> Result<PathBuf> {
    validate_package_name(name)?;

    let root = path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(name));
    if root.exists() && root.read_dir().into_diagnostic()?.next().is_some() {
        miette::bail!("destination is not empty: {}", root.display());
    }

    initialize_project(&root, name, kind)?;

    Ok(root)
}

#[cfg(test)]
pub fn init_project(name: Option<&str>, path: Option<&Path>) -> Result<(String, PathBuf)> {
    init_project_with_kind(name, path, ProjectKind::Binary)
}

pub fn init_project_with_kind(
    name: Option<&str>,
    path: Option<&Path>,
    kind: ProjectKind,
) -> Result<(String, PathBuf)> {
    let root = path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let name = match name {
        Some(name) => name.to_string(),
        None => default_package_name(&root)?,
    };
    validate_package_name(&name)?;
    initialize_project(&root, &name, kind)?;

    Ok((name, root))
}

fn initialize_project(root: &Path, name: &str, kind: ProjectKind) -> Result<()> {
    for path in [
        root.join("Sengoo.toml"),
        root.join(kind.entry_path()),
        root.join(".gitignore"),
    ] {
        if path.exists() {
            miette::bail!("refusing to overwrite {}", path.display());
        }
    }

    fs::create_dir_all(root.join("src"))
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", root.join("src").display()))?;
    fs::create_dir_all(root.join("tests"))
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", root.join("tests").display()))?;

    write_new_file(&root.join("Sengoo.toml"), manifest_template(name, kind))?;
    write_new_file(&root.join(kind.entry_path()), kind.source_template())?;
    write_new_file(&root.join(".gitignore"), gitignore_template())?;

    Ok(())
}

impl ProjectKind {
    fn entry_path(self) -> &'static Path {
        match self {
            Self::Binary => Path::new("src/main.sg"),
            Self::Library => Path::new("src/lib.sg"),
        }
    }

    fn source_template(self) -> String {
        match self {
            Self::Binary => main_template(),
            Self::Library => lib_template(),
        }
    }
}

fn default_package_name(root: &Path) -> Result<String> {
    let root = if root.file_name().is_some() {
        root.to_path_buf()
    } else {
        fs::canonicalize(root)
            .into_diagnostic()
            .with_context(|| format!("failed to resolve {}", root.display()))?
    };
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .ok_or_else(|| miette::miette!("could not derive package name from {}", root.display()))
}

fn write_new_file(path: &Path, content: String) -> Result<()> {
    if path.exists() {
        miette::bail!("refusing to overwrite {}", path.display());
    }
    fs::write(path, content)
        .into_diagnostic()
        .with_context(|| format!("failed to write {}", path.display()))
}

fn manifest_template(name: &str, kind: ProjectKind) -> String {
    let target = match kind {
        ProjectKind::Binary => "[bin]\npath = \"src/main.sg\"",
        ProjectKind::Library => "[lib]\npath = \"src/lib.sg\"",
    };
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2026"

{target}
"#
    )
}

fn main_template() -> String {
    r#"def main() -> i64 {
    print("Hello from sgpm!")
    0
}
"#
    .to_string()
}

fn lib_template() -> String {
    r#"def answer() -> i64 {
    42
}
"#
    .to_string()
}

fn gitignore_template() -> String {
    r#"# Sengoo build output
target/
build/
*.ll

# Editor / OS noise
.vscode/
.idea/
*.swp
Thumbs.db
.DS_Store
"#
    .to_string()
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
        std::env::temp_dir().join(format!("sgpm_scaffold_{}_{}", name, stamp))
    }

    #[test]
    fn creates_project_layout() {
        let root = temp_dir("layout");
        let created = new_project("demo_pkg", Some(&root)).unwrap();
        assert_eq!(created, root);
        assert!(created.join("Sengoo.toml").exists());
        assert!(created.join("src/main.sg").exists());
        assert!(created.join("tests").exists());
        assert!(created.join(".gitignore").exists());

        let manifest = fs::read_to_string(created.join("Sengoo.toml")).unwrap();
        assert!(manifest.contains("name = \"demo_pkg\""));
        assert!(manifest.contains("edition = \"2026\""));
        let _ = fs::remove_dir_all(created);
    }

    #[test]
    fn creates_library_project_layout() {
        let root = temp_dir("library_layout");
        let created = new_project_with_kind("demo_lib", Some(&root), ProjectKind::Library).unwrap();
        assert!(created.join("Sengoo.toml").exists());
        assert!(created.join("src/lib.sg").exists());
        assert!(!created.join("src/main.sg").exists());

        let manifest = fs::read_to_string(created.join("Sengoo.toml")).unwrap();
        assert!(manifest.contains("[lib]"));
        assert!(manifest.contains("path = \"src/lib.sg\""));
        let _ = fs::remove_dir_all(created);
    }

    #[test]
    fn rejects_invalid_package_name() {
        let err = new_project("DemoPkg", Some(&temp_dir("invalid"))).unwrap_err();
        assert!(err.to_string().contains("may only contain"));
    }

    #[test]
    fn refuses_non_empty_destination() {
        let root = temp_dir("non_empty");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("existing.txt"), "").unwrap();
        let err = new_project("demo", Some(&root)).unwrap_err();
        assert!(err.to_string().contains("destination is not empty"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn initializes_existing_directory_without_overwriting_files() {
        let root = temp_dir("init");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("README.md"), "# existing\n").unwrap();

        let (name, created) = init_project(Some("demo"), Some(&root)).unwrap();

        assert_eq!(name, "demo");
        assert_eq!(created, root);
        assert!(created.join("Sengoo.toml").exists());
        assert!(created.join("src/main.sg").exists());
        assert_eq!(
            fs::read_to_string(created.join("README.md")).unwrap(),
            "# existing\n"
        );
        let _ = fs::remove_dir_all(created);
    }

    #[test]
    fn init_refuses_to_overwrite_existing_scaffold_files() {
        let root = temp_dir("init_conflict");
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Sengoo.toml"), "existing").unwrap();

        let err = init_project(Some("demo"), Some(&root)).unwrap_err();

        assert!(err.to_string().contains("refusing to overwrite"));
        assert_eq!(
            fs::read_to_string(root.join("Sengoo.toml")).unwrap(),
            "existing"
        );
        assert!(!root.join("src").exists());
        let _ = fs::remove_dir_all(root);
    }
}
