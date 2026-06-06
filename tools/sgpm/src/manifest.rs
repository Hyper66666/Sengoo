use miette::{Context, IntoDiagnostic, Result};
use semver::{Version, VersionReq};
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Manifest {
    pub package: PackageMeta,
    pub bin: Option<BinTarget>,
    pub lib: Option<LibTarget>,
    pub test: Vec<TestTarget>,
    pub registries: BTreeMap<String, RegistryConfig>,
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

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestTarget {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RegistryConfig {
    #[serde(default)]
    pub path: Option<PathBuf>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub token_env: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceConfig {
    pub members: Vec<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct WorkspaceManifest {
    pub members: Vec<PathBuf>,
    pub registries: BTreeMap<String, RegistryConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dependency {
    pub name: String,
    pub path: Option<PathBuf>,
    pub git: Option<String>,
    pub rev: Option<String>,
    pub version_req: Option<String>,
    pub registry: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawManifest {
    #[serde(default, rename = "sengoo-schema")]
    sengoo_schema: Option<u32>,
    package: PackageMeta,
    #[serde(default)]
    bin: Option<BinTarget>,
    #[serde(default)]
    lib: Option<LibTarget>,
    #[serde(default)]
    test: Vec<TestTarget>,
    #[serde(default)]
    registries: BTreeMap<String, RegistryConfig>,
    #[serde(default)]
    dependencies: BTreeMap<String, toml::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWorkspaceManifest {
    workspace: WorkspaceConfig,
    #[serde(default)]
    registries: BTreeMap<String, RegistryConfig>,
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
        if let Some(version) = raw.sengoo_schema {
            if version != 1 {
                miette::bail!(
                    "unsupported Sengoo.toml schema version {}; expected 1",
                    version
                );
            }
        }
        validate_package(&raw.package)?;
        if let Some(name) = raw.bin.as_ref().and_then(|bin| bin.name.as_deref()) {
            validate_name("binary name", name)?;
        }
        validate_registries(&raw.registries)?;

        let mut dependencies = BTreeMap::new();
        for (name, value) in raw.dependencies {
            let dep = parse_dependency(&name, value)?;
            dependencies.insert(name, dep);
        }

        Ok(Self {
            package: raw.package,
            bin: raw.bin,
            lib: raw.lib,
            test: raw.test,
            registries: raw.registries,
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

impl WorkspaceManifest {
    pub fn load(path: &Path) -> Result<Self> {
        let source = fs::read_to_string(path)
            .into_diagnostic()
            .with_context(|| format!("failed to read workspace manifest {}", path.display()))?;
        Self::parse(&source)
            .with_context(|| format!("failed to parse workspace manifest {}", path.display()))
    }

    pub fn parse(source: &str) -> Result<Self> {
        let raw: RawWorkspaceManifest = toml::from_str(source).into_diagnostic()?;
        if raw.workspace.members.is_empty() {
            miette::bail!("[workspace].members must not be empty");
        }
        validate_registries(&raw.registries)?;
        for member in &raw.workspace.members {
            if member.as_os_str().is_empty() {
                miette::bail!("[workspace].members must not contain empty paths");
            }
        }
        Ok(Self {
            members: raw.workspace.members,
            registries: raw.registries,
        })
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

    validate_package_name(&package.name)
}

pub(crate) fn validate_package_name(name: &str) -> Result<()> {
    validate_name("package name", name)
}

fn validate_name(kind: &str, name: &str) -> Result<()> {
    if name.trim().is_empty() {
        miette::bail!("{} must not be empty", kind);
    }
    let valid = name
        .chars()
        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-');
    if !valid {
        miette::bail!(
            "{} '{}' may only contain lowercase ASCII letters, digits, '_' or '-'",
            kind,
            name
        );
    }
    Ok(())
}

fn validate_registry_name(name: &str) -> Result<()> {
    if name.trim().is_empty() {
        miette::bail!("registry name must not be empty");
    }
    let boundary_is_valid = |ch: char| ch.is_ascii_lowercase() || ch.is_ascii_digit();
    let valid = name
        .chars()
        .all(|ch| boundary_is_valid(ch) || ch == '_' || ch == '-' || ch == '.')
        && name.chars().next().is_some_and(boundary_is_valid)
        && name.chars().last().is_some_and(boundary_is_valid);
    if !valid {
        miette::bail!(
            "registry name '{}' must start and end with a lowercase ASCII letter or digit and may contain lowercase ASCII letters, digits, '_', '-' or '.'",
            name
        );
    }
    Ok(())
}

fn validate_registries(registries: &BTreeMap<String, RegistryConfig>) -> Result<()> {
    for (name, config) in registries {
        validate_registry_name(name)?;
        let has_path = config.path.is_some();
        let has_url = config
            .url
            .as_deref()
            .map(str::trim)
            .is_some_and(|url| !url.is_empty());
        match (has_path, has_url) {
            (true, false) | (false, true) => {}
            (false, false) => {
                miette::bail!(
                    "[registries.{}] must specify exactly one of path or url",
                    name
                );
            }
            (true, true) => {
                miette::bail!("[registries.{}] must not specify both path and url", name);
            }
        }
        if config
            .token_env
            .as_deref()
            .map(str::trim)
            .is_some_and(str::is_empty)
        {
            miette::bail!("[registries.{}].token_env must not be empty", name);
        }
        if config.token_env.is_some() && !has_url {
            miette::bail!("[registries.{}].token_env requires url", name);
        }
    }
    Ok(())
}

fn parse_dependency(name: &str, value: toml::Value) -> Result<Dependency> {
    validate_name("dependency name", name)?;
    match value {
        toml::Value::Table(table) => parse_table_dependency(name, table),
        toml::Value::String(version) => {
            let version = version.trim();
            let _ = VersionReq::parse(version)
                .into_diagnostic()
                .with_context(|| {
                    format!("invalid version requirement for dependency '{}'", name)
                })?;
            Ok(Dependency {
                name: name.to_string(),
                path: None,
                git: None,
                rev: None,
                version_req: Some(version.to_string()),
                registry: None,
            })
        }
        other => miette::bail!(
            "dependency '{}' must be a version string or a table like {{ path = \"../dep\" }}; got {}",
            name,
            other.type_str()
        ),
    }
}

fn parse_table_dependency(
    name: &str,
    table: toml::map::Map<String, toml::Value>,
) -> Result<Dependency> {
    let unsupported = table
        .keys()
        .filter(|key| {
            !matches!(
                key.as_str(),
                "path" | "git" | "rev" | "version" | "registry"
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        miette::bail!(
            "dependency '{}' uses unsupported key(s) {:?}",
            name,
            unsupported
        );
    }

    let path = table.get("path").and_then(toml::Value::as_str);
    let git = table.get("git").and_then(toml::Value::as_str);
    let rev = table.get("rev").and_then(toml::Value::as_str);
    let version = table.get("version").and_then(toml::Value::as_str);
    let registry = table.get("registry").and_then(toml::Value::as_str);

    let source_count = [path.is_some(), git.is_some(), version.is_some()]
        .into_iter()
        .filter(|present| *present)
        .count();
    if source_count > 1 {
        miette::bail!(
            "dependency '{}' must specify only one source: path, git, or version",
            name
        );
    }
    if source_count == 0 {
        miette::bail!(
            "dependency '{}' must specify path = \"...\", git = \"...\", or version = \"...\"",
            name
        );
    }
    if rev.is_some() && git.is_none() {
        miette::bail!("dependency '{}' uses rev without git", name);
    }
    if registry.is_some() && version.is_none() {
        miette::bail!("dependency '{}' uses registry without version", name);
    }

    let path = path.map(str::trim).filter(|value| !value.is_empty());
    let git = git.map(str::trim).filter(|value| !value.is_empty());
    let version = version.map(str::trim).filter(|value| !value.is_empty());
    let registry = registry.map(str::trim).filter(|value| !value.is_empty());
    if table.contains_key("path") && path.is_none() {
        miette::bail!("dependency '{}' path must not be empty", name);
    }
    if table.contains_key("git") && git.is_none() {
        miette::bail!("dependency '{}' git URL must not be empty", name);
    }
    if table.contains_key("version") && version.is_none() {
        miette::bail!(
            "dependency '{}' version requirement must not be empty",
            name
        );
    }
    if table.contains_key("registry") && registry.is_none() {
        miette::bail!("dependency '{}' registry must not be empty", name);
    }
    if let Some(registry) = registry {
        validate_registry_name(registry)?;
    }
    let rev = rev.map(str::trim).filter(|value| !value.is_empty());
    if table.contains_key("rev") && rev.is_none() {
        miette::bail!("dependency '{}' rev must not be empty", name);
    }
    if let Some(version) = version {
        let _ = VersionReq::parse(version)
            .into_diagnostic()
            .with_context(|| format!("invalid version requirement for dependency '{}'", name))?;
    }

    Ok(Dependency {
        name: name.to_string(),
        path: path.map(PathBuf::from),
        git: git.map(str::to_string),
        rev: rev.map(str::to_string),
        version_req: version.map(str::to_string),
        registry: registry.map(str::to_string),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unsupported_sengoo_schema_version() {
        let source = "sengoo-schema = 99\n[package]\nname = \"hello\"\nversion = \"0.1.0\"\nedition = \"2026\"\n";
        let err = Manifest::parse(source).expect_err("unsupported schema should fail");
        assert!(err.to_string().contains("unsupported Sengoo.toml schema"));
    }

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
    fn parses_manifest_test_targets() {
        let manifest = Manifest::parse(
            r#"
sengoo-schema = 1
[package]
name = "hello"
version = "0.1.0"
edition = "2026"
[[test]]
path = "tests/custom.sg"
"#,
        )
        .expect("manifest with test targets should parse");
        assert_eq!(manifest.test.len(), 1);
        assert_eq!(manifest.test[0].path, PathBuf::from("tests/custom.sg"));
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
    fn rejects_invalid_package_name() {
        let err =
            Manifest::parse("[package]\nname = '../escape'\nversion = '0.1.0'\n").unwrap_err();
        assert!(err.to_string().contains("package name '../escape'"));
    }

    #[test]
    fn rejects_invalid_dependency_name() {
        let err = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[dependencies]\n'../escape' = '1.0.0'\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("dependency name '../escape'"));
    }

    #[test]
    fn rejects_invalid_binary_name() {
        let err = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[bin]\nname = '../escape'\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("binary name '../escape'"));
    }

    #[test]
    fn rejects_unknown_top_level_key() {
        let err =
            Manifest::parse("[package]\nname = 'x'\nversion = '0.1.0'\n[workspace]\n").unwrap_err();
        assert!(err.to_string().contains("unknown field"));
    }

    #[test]
    fn parses_bare_string_dep_as_default_registry_version_req() {
        let manifest = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[dependencies]\nfoo = '1.0.0'\n",
        )
        .expect("manifest should parse");
        let dep = manifest.dependencies.get("foo").expect("foo dep");
        assert_eq!(dep.version_req.as_deref(), Some("1.0.0"));
        assert_eq!(dep.registry.as_deref(), None);
    }

    #[test]
    fn parses_registry_version_dep() {
        let manifest = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[registries.local]\npath = '../registry'\n[dependencies]\nfoo = { version = '>=1.0.0, <2.0.0', registry = 'local' }\n",
        )
        .expect("manifest should parse");
        assert_eq!(
            manifest
                .registries
                .get("local")
                .expect("local registry")
                .path
                .as_deref(),
            Some(Path::new("../registry"))
        );
        let dep = manifest.dependencies.get("foo").expect("foo dep");
        assert_eq!(dep.version_req.as_deref(), Some(">=1.0.0, <2.0.0"));
        assert_eq!(dep.registry.as_deref(), Some("local"));
    }

    #[test]
    fn parses_remote_registry_config() {
        let manifest = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[registries.default]\nurl = 'https://registry.example.invalid'\ntoken_env = 'SENGOO_TOKEN'\n",
        )
        .expect("manifest should parse");
        let registry = manifest
            .registries
            .get("default")
            .expect("default registry");
        assert_eq!(
            registry.url.as_deref(),
            Some("https://registry.example.invalid")
        );
        assert_eq!(registry.token_env.as_deref(), Some("SENGOO_TOKEN"));
        assert!(registry.path.is_none());
    }

    #[test]
    fn rejects_registry_with_path_and_url() {
        let err = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[registries.default]\npath = '../registry'\nurl = 'https://registry.example.invalid'\n",
        )
        .unwrap_err();
        assert!(err
            .to_string()
            .contains("must not specify both path and url"));
    }

    #[test]
    fn rejects_invalid_registry_name() {
        let err = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[registries.'../escape']\npath = '../registry'\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("registry name '../escape'"));
    }

    #[test]
    fn rejects_registry_name_with_boundary_dot() {
        for name in [".", "..", ".hidden", "hidden."] {
            let source = format!(
                "[package]\nname = 'x'\nversion = '0.1.0'\n[registries.'{name}']\npath = '../registry'\n"
            );
            let err = Manifest::parse(&source).unwrap_err();
            assert!(
                err.to_string().contains(&format!("registry name '{name}'")),
                "unexpected diagnostic for {name}: {err}"
            );
        }
    }

    #[test]
    fn rejects_invalid_dependency_registry_name() {
        let err = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[dependencies]\nfoo = { version = '1.0.0', registry = '../escape' }\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("registry name '../escape'"));
    }

    #[test]
    fn rejects_invalid_workspace_registry_name() {
        let err = WorkspaceManifest::parse(
            "[workspace]\nmembers = ['packages/*']\n[registries.'../escape']\npath = '../registry'\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("registry name '../escape'"));
    }

    #[test]
    fn rejects_git_dep_with_empty_url() {
        let err = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[dependencies]\nfoo = { git = '' }\n",
        )
        .unwrap_err();
        assert!(err.to_string().contains("git URL must not be empty"));
    }

    #[test]
    fn parses_git_dep() {
        let manifest = Manifest::parse(
            "[package]\nname = 'x'\nversion = '0.1.0'\n[dependencies]\nfoo = { git = 'file:///tmp/foo', rev = 'abc123' }\n",
        )
        .expect("manifest should parse");
        let dep = manifest.dependencies.get("foo").expect("foo dep");
        assert_eq!(dep.name, "foo");
        assert_eq!(dep.git.as_deref(), Some("file:///tmp/foo"));
        assert_eq!(dep.rev.as_deref(), Some("abc123"));
    }
}
