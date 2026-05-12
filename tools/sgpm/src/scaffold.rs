use miette::{Context, IntoDiagnostic, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn new_project(name: &str, path: Option<&Path>) -> Result<PathBuf> {
    validate_package_name(name)?;

    let root = path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from(name));
    if root.exists() && root.read_dir().into_diagnostic()?.next().is_some() {
        miette::bail!("destination is not empty: {}", root.display());
    }

    fs::create_dir_all(root.join("src"))
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", root.join("src").display()))?;
    fs::create_dir_all(root.join("tests"))
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", root.join("tests").display()))?;

    write_new_file(&root.join("Sengoo.toml"), manifest_template(name))?;
    write_new_file(&root.join("src").join("main.sg"), main_template())?;
    write_new_file(&root.join(".gitignore"), gitignore_template())?;

    Ok(root)
}

fn validate_package_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        miette::bail!("package name must not be empty");
    }
    let valid = name
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-');
    if !valid {
        miette::bail!(
            "package name '{}' may only contain lowercase ASCII letters, digits, '_' or '-'",
            name
        );
    }
    Ok(())
}

fn write_new_file(path: &Path, content: String) -> Result<()> {
    if path.exists() {
        miette::bail!("refusing to overwrite {}", path.display());
    }
    fs::write(path, content)
        .into_diagnostic()
        .with_context(|| format!("failed to write {}", path.display()))
}

fn manifest_template(name: &str) -> String {
    format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
edition = "2026"

[bin]
path = "src/main.sg"
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
}
