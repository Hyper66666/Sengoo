use miette::{Context, IntoDiagnostic, Result};
use semver::Version;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Manifest {
    pub package: PackageMeta,
    pub bin: Option<BinTarget>,
    pub lib: Option<LibTarget>,
    pub dependencies: BTreeMap<String, Dependency>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageMeta {
    pub name: String,
    pub version: String,
    #[allow(dead_code)]
    #[serde(default)]
    pub authors: Vec<String>,
    #[allow(dead_code)]
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub edition: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BinTarget {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default = "default_bin_path")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LibTarget {
    #[serde(default = "default_lib_path")]
    pub path: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub path: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    package: PackageMeta,
    #[serde(default)]
    bin: Option<BinTarget>,
    #[serde(default)]
    lib: Option<LibTarget>,
    #[serde(default)]
    dependencies: BTreeMap<String, toml::Value>,
}

fn default_bin_path() -> PathBuf {
    PathBuf::from("src/main.sg")
}

fn default_lib_path() -> PathBuf {
    PathBuf::from("src/lib.sg")
}

impl Manifest {
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .into_diagnostic()
            .with_context(|| format!("failed to read manifest {}", path.display()))?;
        Self::parse(&source).with_context(|| format!("failed to parse manifest {}", path.display()))
    }

    pub fn parse(source: &str) -> Result<Self> {
        let raw: RawManifest = toml::from_str(source).into_diagnostic()?;
        validate_package(&raw.package)?;

        let mut dependencies = BTreeMap::new();
        for (name, value) in raw.dependencies {
            let dep = parse_dependency(&name, value)?;
            dependencies.insert(name, dep);
        }

        Ok(Self {
            package: raw.package,
            bin: raw.bin,
            lib: raw.lib,
            dependencies,
        })
    }

    pub fn entry_path(&self) -> PathBuf {
        if let Some(bin) = &self.bin {
            return bin.path.clone();
        }
        if let Some(lib) = &self.lib {
            return lib.path.clone();
        }
        default_bin_path()
    }
}

fn validate_package(package: &PackageMeta) -> Result<()> {
    Version::parse(&package.version)
        .into_diagnostic()
        .with_context(|| format!("invalid [package].version for {}", package.name))?;

    if let Some(edition) = &package.edition {
        if edition != "2026" {
            miette::bail!("unsupported Sengoo edition '{}'; expected '2026'", edition);
        }
    }

    if package.name.trim().is_empty() {
        miette::bail!("[package].name must not be empty");
    }
    Ok(())
}

fn parse_dependency(name: &str, value: toml::Value) -> Result<Dependency> {
    match value {
        toml::Value::Table(table) => parse_table_dependency(name, table),
        toml::Value::String(version) => {
            let _ = Version::parse(&version)
                .into_diagnostic()
                .with_context(|| {
                    format!("invalid version requirement for dependency '{}'", name)
                })?;
            miette::bail!(
                "dependency '{}' uses registry version '{}', but sgpm MVP only supports path dependencies",
                name,
                version
            )
        }
        other => miette::bail!(
            "dependency '{}' must be a table like {{ path = \"../dep\" }}; got {}",
            name,
            other.type_str()
        ),
    }
}

fn parse_table_dependency(
    name: &str,
    table: toml::map::Map<String, toml::Value>,
) -> Result<Dependency> {
    if let Some(version) = table.get("version").and_then(toml::Value::as_str) {
        let _ = Version::parse(version)
            .into_diagnostic()
            .with_context(|| format!("invalid version field for dependency '{}'", name))?;
    }

    let unsupported = table
        .keys()
        .filter(|key| key.as_str() != "path")
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        miette::bail!(
            "dependency '{}' uses unsupported key(s) {:?}; registry/git support is not implemented in sgpm MVP",
            name,
            unsupported
        );
    }

    let path = table
        .get("path")
        .and_then(toml::Value::as_str)
        .ok_or_else(|| miette::miette!("dependency '{}' must specify path = \"...\"", name))?;
    if path.trim().is_empty() {
        miette::bail!("dependency '{}' path must not be empty", name);
    }

    Ok(Dependency {
        name: name.to_string(),
        path: PathBuf::from(path),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_minimal_manifest() {
        let manifest = Manifest::parse(
            r#"
[package]
name = "hello"
version = "0.1.0"
edition = "2026"
"#,
        )
        .expect("manifest should parse");
        assert_eq!(manifest.package.name, "hello");
        assert_eq!(manifest.entry_path(), PathBuf::from("src/main.sg"));
    }

    #[test]
    fn rejects_missing_package() {
        let err = Manifest::parse("[bin]\npath = 'src/main.sg'\n").unwrap_err();
        assert!(err.to_string().contains("missing field"));
    }

    #[test]
    fn rejects_invalid_semver() {
        let err = Manifest::parse("[package]\nname = 'x'\nversion = 'not-semver'\n").unwrap_err();
        assert!(err.to_string().contains("invalid [package].version"));
    }

    #[test]
    fn rejects_unknown_top_level_key() {
        let err =
            Manifest::parse("[package]\nname = 'x'\nversion = '0.1.0'\n[workspace]\n").unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn rejects_bare_string_dep() {
        let err = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[dependencies]\nfoo = '1.0.0'\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("only supports path dependencies"));
    }

    #[test]
    fn rejects_version_only_dep() {
        let err = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[dependencies]\nfoo = { version = '1.0.0' }\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("unsupported key"));
    }

    #[test]
    fn rejects_git_only_dep() {
        let err = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[dependencies]\nfoo = { git = 'https://example.invalid/foo' }\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("unsupported key"));
    }
}
