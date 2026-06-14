use crate::manifest::{Dependency, Manifest, RegistryConfig};
use flate2::read::GzDecoder;
use miette::{Context, IntoDiagnostic, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION};
use semver::{Version, VersionReq};
use serde::{Deserialize, Deserializer};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use tar::Archive;
use tokio::runtime::Builder as TokioRuntimeBuilder;
use walkdir::WalkDir;

static REMOTE_REGISTRY_STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);
static GIT_STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);
const REMOTE_ARCHIVE_CHECKSUM_FILE: &str = ".sgpm-archive.sha256";
const REMOTE_CONTENT_CHECKSUM_FILE: &str = ".sgpm-content.sha256";

#[derive(Debug, Clone)]
pub struct DependencyEdge {
    pub from: String,
    pub alias: String,
    pub to: String,
}

#[derive(Debug, Clone)]
pub struct PackageNode {
    pub id: String,
    pub name: String,
    pub manifest_path: PathBuf,
    pub root_dir: PathBuf,
    pub entry_path: PathBuf,
    pub manifest: Manifest,
    pub source: PackageSource,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackageSource {
    Path,
    Git {
        url: String,
        rev: String,
    },
    Registry {
        registry: String,
        version: String,
        metadata: RegistryPackageMetadata,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RegistryPackageMetadata {
    pub checksum: Option<String>,
    pub yanked: bool,
    pub yank_reason: Option<String>,
    pub features: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Graph {
    pub root: PathBuf,
    pub nodes: Vec<PackageNode>,
    pub edges: Vec<DependencyEdge>,
    pub registries: BTreeMap<String, RegistryConfig>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ResolveOptions {
    pub refresh_git: bool,
    pub allow_yanked: bool,
}

impl Graph {
    #[cfg(test)]
    pub fn from_root(manifest_path: &Path) -> Result<Self> {
        Self::from_root_with_options(manifest_path, ResolveOptions::default())
    }

    pub fn from_root_with_options(manifest_path: &Path, options: ResolveOptions) -> Result<Self> {
        Self::from_root_with_registries(manifest_path, options, BTreeMap::new())
    }

    pub fn from_root_with_registries(
        manifest_path: &Path,
        options: ResolveOptions,
        inherited_registries: BTreeMap<String, RegistryConfig>,
    ) -> Result<Self> {
        let root_manifest = resolve_manifest_path(manifest_path)?;
        let root_dir = root_manifest
            .parent()
            .ok_or_else(|| {
                miette::miette!(
                    "manifest has no parent directory: {}",
                    root_manifest.display()
                )
            })?
            .to_path_buf();
        let root_manifest_data = Manifest::load(&root_manifest)?;
        let mut registries = inherited_registries;
        registries.extend(root_manifest_data.registries);
        let effective_registries = registries.clone();
        let mut builder = GraphBuilder::new(
            root_dir.join("target").join("sgpm").join("git"),
            root_dir,
            registries,
            options,
        );
        builder.visit(&root_manifest, PackageSource::Path, None)?;
        Ok(Self {
            root: root_manifest,
            nodes: builder.nodes,
            edges: builder.edges,
            registries: effective_registries,
        })
    }

    pub fn root_package(&self) -> Option<&PackageNode> {
        self.nodes
            .iter()
            .find(|node| node.manifest_path == self.root)
    }

    #[cfg(test)]
    pub fn requires_lockfile_v2(&self) -> bool {
        graph_requires_lockfile_v2(self)
    }

    pub fn node_by_id(&self, id: &str) -> Option<&PackageNode> {
        self.nodes.iter().find(|node| node.id == id)
    }
}

struct GraphBuilder {
    visiting: BTreeSet<PathBuf>,
    visited: BTreeSet<PathBuf>,
    stack: Vec<PathBuf>,
    nodes: Vec<PackageNode>,
    edges: Vec<DependencyEdge>,
    git_cache_dir: PathBuf,
    registry_cache_dir: PathBuf,
    root_dir: PathBuf,
    registries: BTreeMap<String, RegistryConfig>,
    package_ids: BTreeMap<String, PathBuf>,
    options: ResolveOptions,
}

impl GraphBuilder {
    fn new(
        git_cache_dir: PathBuf,
        root_dir: PathBuf,
        registries: BTreeMap<String, RegistryConfig>,
        options: ResolveOptions,
    ) -> Self {
        let registry_cache_dir = git_cache_dir
            .parent()
            .map(|path| path.join("registry"))
            .unwrap_or_else(|| git_cache_dir.join("registry"));
        Self {
            visiting: BTreeSet::new(),
            visited: BTreeSet::new(),
            stack: Vec::new(),
            nodes: Vec::new(),
            edges: Vec::new(),
            git_cache_dir,
            registry_cache_dir,
            root_dir,
            registries,
            package_ids: BTreeMap::new(),
            options,
        }
    }
}

impl GraphBuilder {
    fn visit(
        &mut self,
        manifest_path: &Path,
        source: PackageSource,
        parent_id: Option<&str>,
    ) -> Result<()> {
        let _ = parent_id;
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
        let package_id = package_identity(
            &manifest.package.name,
            &manifest.package.version,
            &canonical_source_key(&self.root_dir, &root_dir, &source, &manifest.package.name),
        );
        if let Some(existing) = self.package_ids.get(&package_id) {
            if existing != &key {
                miette::bail!(
                    "package '{}' version '{}' resolves to conflicting sources: {} and {}",
                    manifest.package.name,
                    manifest.package.version,
                    existing.display(),
                    key.display()
                );
            }
        } else {
            self.package_ids.insert(package_id.clone(), key.clone());
        }
        validate_package_entries(&root_dir, &manifest)?;

        for (alias, dep) in &manifest.dependencies {
            let (dep_manifest, dep_source) = self
                .resolve_dependency_manifest(&root_dir, dep)
                .with_context(|| format!("failed to resolve dependency '{}'", alias))?;
            validate_dependency_package_name(dep, &dep_manifest)?;
            let dep_manifest_data = Manifest::load(&dep_manifest)?;
            let dep_root_dir = dep_manifest
                .parent()
                .ok_or_else(|| {
                    miette::miette!(
                        "manifest has no parent directory: {}",
                        dep_manifest.display()
                    )
                })?
                .to_path_buf();
            let dep_id = package_identity(
                &dep_manifest_data.package.name,
                &dep_manifest_data.package.version,
                &canonical_source_key(
                    &self.root_dir,
                    &dep_root_dir,
                    &dep_source,
                    &dep_manifest_data.package.name,
                ),
            );
            self.edges.push(DependencyEdge {
                from: package_id.clone(),
                alias: alias.clone(),
                to: dep_id.clone(),
            });
            self.visit(&dep_manifest, dep_source, Some(&package_id))?;
        }

        let entry_path = root_dir.join(manifest.entry_path());
        self.nodes.push(PackageNode {
            id: package_id,
            name: manifest.package.name.clone(),
            manifest_path: key.clone(),
            root_dir,
            entry_path,
            manifest,
            source,
        });

        self.stack.pop();
        self.visiting.remove(&key);
        self.visited.insert(key);
        Ok(())
    }

    fn resolve_dependency_manifest(
        &mut self,
        parent_dir: &Path,
        dep: &Dependency,
    ) -> Result<(PathBuf, PackageSource)> {
        if let Some(dep_path) = &dep.path {
            return resolve_path_dependency_manifest(parent_dir, dep_path)
                .map(|path| (path, PackageSource::Path));
        }

        if let Some(git_url) = dep.git.as_deref() {
            return resolve_git_dependency_manifest(
                parent_dir,
                &self.git_cache_dir,
                dep,
                git_url,
                self.options.refresh_git,
            );
        }

        self.resolve_registry_dependency_manifest(dep)
    }

    fn resolve_registry_dependency_manifest(
        &mut self,
        dep: &Dependency,
    ) -> Result<(PathBuf, PackageSource)> {
        let requirement = dep
            .version_req
            .as_deref()
            .ok_or_else(|| miette::miette!("dependency '{}' has no supported source", dep.name))?;
        let req = VersionReq::parse(requirement)
            .into_diagnostic()
            .with_context(|| {
                format!("invalid version requirement for dependency '{}'", dep.name)
            })?;
        let registry_name = dep.registry.as_deref().unwrap_or("default");
        let package_name = dep.target_name();

        let config = self.registries.get(registry_name).ok_or_else(|| {
            miette::miette!(
                "dependency '{}' requires registry '{}', but no [registries.{}] is configured",
                dep.name,
                registry_name,
                registry_name
            )
        })?;
        let (version, manifest_path, metadata) = if let Some(registry_path) = config.path.as_ref() {
            let registry_root = resolve_registry_path(&self.root_dir, registry_path)?;
            let package_root = canonicalize_existing(&registry_root.join(package_name))
                .with_context(|| {
                    format!(
                        "registry '{}' has no package '{}'",
                        registry_name, package_name
                    )
                })?;
            select_registry_package_manifest(&package_root, package_name, &req, requirement)?
        } else {
            fetch_remote_registry_package_manifest(
                &self.registry_cache_dir,
                registry_name,
                config,
                package_name,
                &req,
                requirement,
                self.options.allow_yanked,
            )?
        };
        if !self.options.allow_yanked {
            reject_yanked_package(package_name, &version, &metadata)?;
        }

        Ok((
            manifest_path,
            PackageSource::Registry {
                registry: registry_name.to_string(),
                version: version.to_string(),
                metadata,
            },
        ))
    }
}

fn validate_dependency_package_name(dep: &Dependency, manifest_path: &Path) -> Result<()> {
    let manifest = Manifest::load(manifest_path)?;
    if manifest.package.name != dep.target_name() {
        miette::bail!(
            "dependency '{}' resolves to package '{}'; expected package '{}'",
            dep.name,
            manifest.package.name,
            dep.target_name()
        );
    }
    Ok(())
}

pub fn package_identity(name: &str, version: &str, source_key: &str) -> String {
    format!("{}@{}+{}", name, version, source_key)
}

pub fn canonical_source_key(
    base_dir: &Path,
    package_root: &Path,
    source: &PackageSource,
    package_name: &str,
) -> String {
    match source {
        PackageSource::Path => {
            format!("path:{}", canonical_path_source(base_dir, package_root))
        }
        PackageSource::Git { url, rev } => format!("git+{}#{}", url, rev),
        PackageSource::Registry {
            registry, version, ..
        } => {
            format!("registry+{}/{}@{}", registry, package_name, version)
        }
    }
}

pub fn canonical_path_source(base_dir: &Path, package_root: &Path) -> String {
    let path = relative_path(base_dir, package_root);
    slash_path(&path)
}

pub fn graph_requires_lockfile_v2(graph: &Graph) -> bool {
    for edge in &graph.edges {
        if let Some(node) = graph.node_by_id(&edge.to) {
            if edge.alias != node.name {
                return true;
            }
        }
    }
    let mut seen = BTreeMap::<&str, BTreeSet<&str>>::new();
    for node in &graph.nodes {
        seen.entry(node.name.as_str())
            .or_default()
            .insert(node.id.as_str());
    }
    seen.values().any(|ids| ids.len() > 1)
}

fn relative_path(from_dir: &Path, to: &Path) -> PathBuf {
    if from_dir == to {
        return PathBuf::from(".");
    }

    let from = path_parts(from_dir);
    let target = path_parts(to);
    if from.first() != target.first() {
        return to.to_path_buf();
    }

    let common = from
        .iter()
        .zip(target.iter())
        .take_while(|(left, right)| left == right)
        .count();

    let mut result = PathBuf::new();
    for _ in common..from.len() {
        result.push("..");
    }
    for part in &target[common..] {
        result.push(part);
    }
    if result.as_os_str().is_empty() {
        PathBuf::from(".")
    } else {
        result
    }
}

fn path_parts(path: &Path) -> Vec<String> {
    use std::path::Component;
    path.components()
        .filter_map(|component| match component {
            Component::Prefix(prefix) => Some(prefix.as_os_str().to_string_lossy().to_string()),
            Component::RootDir => Some("/".to_string()),
            Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            Component::ParentDir => Some("..".to_string()),
            Component::CurDir => None,
        })
        .collect()
}

fn slash_path(path: &Path) -> String {
    use std::path::Component;
    path.components()
        .map(|component| match component {
            Component::Prefix(prefix) => prefix.as_os_str().to_string_lossy().to_string(),
            Component::RootDir => "/".to_string(),
            Component::Normal(part) => part.to_string_lossy().to_string(),
            Component::ParentDir => "..".to_string(),
            Component::CurDir => ".".to_string(),
        })
        .collect::<Vec<_>>()
        .join("/")
}

fn validate_package_entries(root_dir: &Path, manifest: &Manifest) -> Result<()> {
    if manifest.bin.is_none() && manifest.lib.is_none() {
        return validate_package_entry(root_dir, manifest, "[bin]", &manifest.entry_path());
    }
    if let Some(bin) = &manifest.bin {
        validate_package_entry(root_dir, manifest, "[bin]", &bin.path)?;
    }
    if let Some(lib) = &manifest.lib {
        validate_package_entry(root_dir, manifest, "[lib]", &lib.path)?;
    }
    Ok(())
}

fn validate_package_entry(
    root_dir: &Path,
    manifest: &Manifest,
    target: &str,
    relative_path: &Path,
) -> Result<()> {
    if relative_path.is_absolute() {
        miette::bail!(
            "package '{}' {} entry must be relative to the package root: {}",
            manifest.package.name,
            target,
            relative_path.display()
        );
    }
    let entry_path = root_dir.join(relative_path);
    if !entry_path.exists() {
        miette::bail!(
            "package '{}' {} entry does not exist: {}",
            manifest.package.name,
            target,
            entry_path.display()
        );
    }
    let entry_path = fs::canonicalize(&entry_path)
        .into_diagnostic()
        .with_context(|| format!("failed to resolve package entry {}", entry_path.display()))?;
    if !entry_path.starts_with(root_dir) {
        miette::bail!(
            "package '{}' {} entry must stay within the package root: {}",
            manifest.package.name,
            target,
            relative_path.display()
        );
    }
    if !entry_path.is_file() {
        miette::bail!(
            "package '{}' {} entry is not a file: {}",
            manifest.package.name,
            target,
            entry_path.display()
        );
    }
    Ok(())
}

pub fn resolve_manifest_path(path: &Path) -> Result<PathBuf> {
    let candidate = if path.is_dir() {
        path.join("Sengoo.toml")
    } else {
        path.to_path_buf()
    };
    canonicalize_existing(&candidate)
}

fn resolve_path_dependency_manifest(parent_dir: &Path, dep_path: &Path) -> Result<PathBuf> {
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

fn resolve_registry_path(root_dir: &Path, registry_path: &Path) -> Result<PathBuf> {
    let joined = if registry_path.is_absolute() {
        registry_path.to_path_buf()
    } else {
        root_dir.join(registry_path)
    };
    canonicalize_existing(&joined)
}

fn select_registry_package_manifest(
    package_root: &Path,
    package_name: &str,
    req: &VersionReq,
    requirement: &str,
) -> Result<(Version, PathBuf, RegistryPackageMetadata)> {
    let mut candidates = Vec::new();
    for entry in fs::read_dir(package_root)
        .into_diagnostic()
        .with_context(|| format!("failed to read registry package {}", package_root.display()))?
    {
        let entry = entry.into_diagnostic().with_context(|| {
            format!("failed to read registry package {}", package_root.display())
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let Ok(version) = Version::parse(&name) else {
            continue;
        };
        if !req.matches(&version) {
            continue;
        }
        let manifest_path = canonicalize_existing(&path.join("Sengoo.toml"))?;
        let manifest = Manifest::load(&manifest_path)?;
        if manifest.package.name != package_name {
            miette::bail!(
                "registry package '{}' version {} manifest declares package '{}'",
                package_name,
                version,
                manifest.package.name
            );
        }
        if manifest.package.version != version.to_string() {
            miette::bail!(
                "registry package '{}' version directory {} contains manifest version {}",
                package_name,
                version,
                manifest.package.version
            );
        }
        let metadata = load_local_registry_metadata(&path)?;
        candidates.push((version, manifest_path, metadata));
    }
    candidates.sort_by(|left, right| left.0.cmp(&right.0));
    candidates.pop().ok_or_else(|| {
        miette::miette!(
            "no versions of registry package '{}' satisfy constraint '{}'",
            package_name,
            requirement
        )
    })
}

#[derive(Debug, Deserialize)]
struct RemoteRegistryIndex {
    versions: Vec<RemoteRegistryVersion>,
}

#[derive(Debug, Deserialize)]
struct RemoteRegistryVersion {
    version: String,
    #[serde(default)]
    checksum: Option<String>,
    #[serde(default)]
    yanked: bool,
    #[serde(default, alias = "reason")]
    yank_reason: Option<String>,
    #[serde(default, deserialize_with = "deserialize_feature_names")]
    features: Vec<String>,
}

fn fetch_remote_registry_package_manifest(
    registry_cache_dir: &Path,
    registry_name: &str,
    config: &RegistryConfig,
    package_name: &str,
    req: &VersionReq,
    requirement: &str,
    allow_yanked: bool,
) -> Result<(Version, PathBuf, RegistryPackageMetadata)> {
    let url = config
        .url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| {
            miette::miette!(
                "registry '{}' must configure path or url before resolving package '{}'",
                registry_name,
                package_name
            )
        })?;
    let base_url = url.trim_end_matches('/');
    let index_url = format!("{}/api/v1/packages/{}", base_url, package_name);
    let index = fetch_remote_registry_index(registry_name, config, &index_url)?;
    let (version, checksum, metadata) =
        select_remote_registry_version(&index, package_name, req, requirement)?;
    if !allow_yanked {
        reject_yanked_package(package_name, &version, &metadata)?;
    }
    let manifest_path = cached_remote_manifest_path(
        registry_cache_dir,
        registry_name,
        package_name,
        &version.to_string(),
    );
    let package_dir = manifest_path
        .parent()
        .ok_or_else(|| miette::miette!("invalid registry cache path"))?;
    let cache_is_valid =
        validate_cached_remote_package(package_dir, package_name, &version, checksum.as_deref())
            .is_ok();
    if !cache_is_valid {
        let download_url = format!(
            "{}/api/v1/packages/{}/{}/download",
            base_url, package_name, version
        );
        let archive = fetch_remote_registry_archive(registry_name, config, &download_url)?;
        if let Some(expected) = checksum.as_deref() {
            let actual = format!("{:x}", Sha256::digest(&archive));
            if actual != expected {
                miette::bail!(
                    "registry '{}' package '{}' version {} checksum mismatch: expected {}, got {}",
                    registry_name,
                    package_name,
                    version,
                    expected,
                    actual
                );
            }
        }
        unpack_remote_registry_archive(
            registry_cache_dir,
            registry_name,
            package_name,
            &version,
            &archive,
            checksum.as_deref(),
        )?;
    }
    validate_registry_package_manifest(&manifest_path, package_name, &version)?;
    Ok((version, manifest_path, metadata))
}

fn fetch_remote_registry_index(
    registry_name: &str,
    config: &RegistryConfig,
    index_url: &str,
) -> Result<RemoteRegistryIndex> {
    remote_registry_request(registry_name, config, index_url, |response| async move {
        response
            .json::<RemoteRegistryIndex>()
            .await
            .into_diagnostic()
    })
}

fn fetch_remote_registry_archive(
    registry_name: &str,
    config: &RegistryConfig,
    download_url: &str,
) -> Result<Vec<u8>> {
    remote_registry_request(registry_name, config, download_url, |response| async move {
        response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .into_diagnostic()
    })
}

fn remote_registry_request<T, Fut, F>(
    registry_name: &str,
    config: &RegistryConfig,
    url: &str,
    decode: F,
) -> Result<T>
where
    Fut: std::future::Future<Output = Result<T>>,
    F: FnOnce(reqwest::Response) -> Fut,
{
    let (headers, token_for_redaction) = remote_registry_headers(registry_name, config)?;
    let runtime = TokioRuntimeBuilder::new_current_thread()
        .enable_all()
        .build()
        .into_diagnostic()?;
    runtime.block_on(async {
        let client = reqwest::Client::builder()
            .no_proxy()
            .build()
            .into_diagnostic()?;
        let response = client
            .get(url)
            .headers(headers)
            .send()
            .await
            .into_diagnostic()
            .with_context(|| format!("failed to fetch remote registry URL {}", url))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            let body = redact_token(&body, token_for_redaction.as_deref());
            miette::bail!(
                "remote registry '{}' returned status {} for {}: {}",
                registry_name,
                status,
                url,
                body
            );
        }
        decode(response).await
    })
}

fn remote_registry_headers(
    registry_name: &str,
    config: &RegistryConfig,
) -> Result<(HeaderMap, Option<String>)> {
    let mut headers = HeaderMap::new();
    let mut token_for_redaction = None;
    if let Some(token_env) = config.token_env.as_deref() {
        let token = env::var(token_env).into_diagnostic().with_context(|| {
            format!(
                "registry '{}' token env {} is not set",
                registry_name, token_env
            )
        })?;
        let auth = format!("Bearer {}", token);
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&auth).into_diagnostic()?,
        );
        token_for_redaction = Some(token);
    }
    Ok((headers, token_for_redaction))
}

fn redact_token(text: &str, token: Option<&str>) -> String {
    match token.filter(|token| !token.is_empty()) {
        Some(token) => text.replace(token, "<redacted>"),
        None => text.to_string(),
    }
}

fn select_remote_registry_version(
    index: &RemoteRegistryIndex,
    package_name: &str,
    req: &VersionReq,
    requirement: &str,
) -> Result<(Version, Option<String>, RegistryPackageMetadata)> {
    let mut candidates = Vec::new();
    for entry in &index.versions {
        let version = Version::parse(&entry.version)
            .into_diagnostic()
            .with_context(|| {
                format!(
                    "remote registry package '{}' has invalid version '{}'",
                    package_name, entry.version
                )
            })?;
        if req.matches(&version) {
            candidates.push((
                version,
                entry.checksum.clone(),
                RegistryPackageMetadata {
                    checksum: entry.checksum.clone(),
                    yanked: entry.yanked,
                    yank_reason: entry.yank_reason.clone(),
                    features: normalized_features(entry.features.clone()),
                },
            ));
        }
    }
    candidates.sort_by(|(left, _, _), (right, _, _)| left.cmp(right));
    candidates.pop().ok_or_else(|| {
        miette::miette!(
            "no versions of remote registry package '{}' satisfy constraint '{}'",
            package_name,
            requirement
        )
    })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct LocalRegistryVersionMetadata {
    #[serde(default)]
    yanked: bool,
    #[serde(default, alias = "reason")]
    yank_reason: Option<String>,
    #[serde(default, deserialize_with = "deserialize_feature_names")]
    features: Vec<String>,
}

fn load_local_registry_metadata(package_version_dir: &Path) -> Result<RegistryPackageMetadata> {
    let metadata_path = package_version_dir.join("sgpm-index.toml");
    if !metadata_path.exists() {
        return Ok(RegistryPackageMetadata::default());
    }
    let source = fs::read_to_string(&metadata_path)
        .into_diagnostic()
        .with_context(|| format!("failed to read {}", metadata_path.display()))?;
    let parsed: LocalRegistryVersionMetadata = toml::from_str(&source)
        .into_diagnostic()
        .with_context(|| format!("failed to parse {}", metadata_path.display()))?;
    Ok(RegistryPackageMetadata {
        checksum: None,
        yanked: parsed.yanked,
        yank_reason: parsed
            .yank_reason
            .filter(|reason| !reason.trim().is_empty()),
        features: normalized_features(parsed.features),
    })
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum FeatureNames {
    List(Vec<String>),
    Map(BTreeMap<String, serde::de::IgnoredAny>),
}

fn deserialize_feature_names<'de, D>(deserializer: D) -> std::result::Result<Vec<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let Some(features) = Option::<FeatureNames>::deserialize(deserializer)? else {
        return Ok(Vec::new());
    };
    Ok(match features {
        FeatureNames::List(features) => features,
        FeatureNames::Map(features) => features.into_keys().collect(),
    })
}

fn normalized_features(features: Vec<String>) -> Vec<String> {
    let mut features = features
        .into_iter()
        .map(|feature| feature.trim().to_string())
        .filter(|feature| !feature.is_empty())
        .collect::<Vec<_>>();
    features.sort();
    features.dedup();
    features
}

fn reject_yanked_package(
    package_name: &str,
    version: &Version,
    metadata: &RegistryPackageMetadata,
) -> Result<()> {
    if !metadata.yanked {
        return Ok(());
    }
    let reason = metadata
        .yank_reason
        .as_deref()
        .filter(|reason| !reason.trim().is_empty())
        .map(|reason| format!(": {}", reason))
        .unwrap_or_default();
    miette::bail!(
        "registry package '{}' version {} is yanked{}; choose a non-yanked version or update the dependency constraint",
        package_name,
        version,
        reason
    )
}

fn cached_remote_manifest_path(
    registry_cache_dir: &Path,
    registry_name: &str,
    package_name: &str,
    version: &str,
) -> PathBuf {
    registry_cache_dir
        .join(sanitize_path_component(registry_name))
        .join(sanitize_path_component(package_name))
        .join(version)
        .join("Sengoo.toml")
}

fn unpack_remote_registry_archive(
    registry_cache_dir: &Path,
    registry_name: &str,
    package_name: &str,
    version: &Version,
    archive: &[u8],
    archive_checksum: Option<&str>,
) -> Result<()> {
    let package_dir = cached_remote_manifest_path(
        registry_cache_dir,
        registry_name,
        package_name,
        &version.to_string(),
    )
    .parent()
    .ok_or_else(|| miette::miette!("invalid registry cache path"))?
    .to_path_buf();
    let package_parent = package_dir
        .parent()
        .ok_or_else(|| miette::miette!("invalid registry cache package path"))?;
    fs::create_dir_all(package_parent)
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", package_parent.display()))?;
    let staging_dir = create_remote_registry_staging_dir(&package_dir)?;
    let unpack_result = (|| {
        let decoder = GzDecoder::new(Cursor::new(archive));
        let mut tar = Archive::new(decoder);
        tar.unpack(&staging_dir)
            .into_diagnostic()
            .with_context(|| {
                format!(
                    "failed to unpack remote package into {}",
                    staging_dir.display()
                )
            })?;
        validate_registry_package_manifest(
            &staging_dir.join("Sengoo.toml"),
            package_name,
            version,
        )?;
        if let Some(checksum) = archive_checksum {
            fs::write(
                staging_dir.join(REMOTE_ARCHIVE_CHECKSUM_FILE),
                format!("{checksum}\n"),
            )
            .into_diagnostic()
            .context("failed to write remote archive checksum marker")?;
        }
        let content_checksum = remote_package_content_checksum(&staging_dir)?;
        fs::write(
            staging_dir.join(REMOTE_CONTENT_CHECKSUM_FILE),
            format!("{content_checksum}\n"),
        )
        .into_diagnostic()
        .context("failed to write remote content checksum marker")?;
        if package_dir.exists() {
            if validate_cached_remote_package(&package_dir, package_name, version, archive_checksum)
                .is_ok()
            {
                return Ok(());
            }
            fs::remove_dir_all(&package_dir)
                .into_diagnostic()
                .with_context(|| format!("failed to clean {}", package_dir.display()))?;
        }
        fs::rename(&staging_dir, &package_dir)
            .into_diagnostic()
            .with_context(|| {
                format!(
                    "failed to finalize remote package cache {} to {}",
                    staging_dir.display(),
                    package_dir.display()
                )
            })
    })();
    if unpack_result.is_err() || staging_dir.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
    unpack_result
}

fn validate_cached_remote_package(
    package_dir: &Path,
    package_name: &str,
    version: &Version,
    archive_checksum: Option<&str>,
) -> Result<()> {
    validate_registry_package_manifest(&package_dir.join("Sengoo.toml"), package_name, version)?;
    if let Some(expected) = archive_checksum {
        let marker = fs::read_to_string(package_dir.join(REMOTE_ARCHIVE_CHECKSUM_FILE))
            .into_diagnostic()
            .context("missing remote archive checksum marker")?;
        if marker.trim() != expected {
            miette::bail!(
                "remote registry cache archive checksum mismatch: expected {}, got {}",
                expected,
                marker.trim()
            );
        }
    }
    let expected_content = fs::read_to_string(package_dir.join(REMOTE_CONTENT_CHECKSUM_FILE))
        .into_diagnostic()
        .context("missing remote content checksum marker")?;
    let actual_content = remote_package_content_checksum(package_dir)?;
    if expected_content.trim() != actual_content {
        miette::bail!(
            "remote registry cache content checksum mismatch: expected {}, got {}",
            expected_content.trim(),
            actual_content
        );
    }
    Ok(())
}

fn remote_package_content_checksum(package_dir: &Path) -> Result<String> {
    let mut files = WalkDir::new(package_dir)
        .into_iter()
        .filter_map(|entry| match entry {
            Ok(entry) if entry.file_type().is_file() => Some(Ok(entry.into_path())),
            Ok(_) => None,
            Err(err) => Some(Err(err)),
        })
        .collect::<std::result::Result<Vec<_>, _>>()
        .into_diagnostic()
        .with_context(|| format!("failed to enumerate {}", package_dir.display()))?;
    files.retain(|path| {
        !matches!(
            path.file_name().and_then(|name| name.to_str()),
            Some(REMOTE_ARCHIVE_CHECKSUM_FILE | REMOTE_CONTENT_CHECKSUM_FILE)
        )
    });
    files.sort_by_key(|path| {
        path.strip_prefix(package_dir)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    });

    let mut digest = Sha256::new();
    for path in files {
        let relative = path
            .strip_prefix(package_dir)
            .into_diagnostic()?
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&path)
            .into_diagnostic()
            .with_context(|| format!("failed to read {}", path.display()))?;
        digest.update(relative.len().to_le_bytes());
        digest.update(relative.as_bytes());
        digest.update(bytes.len().to_le_bytes());
        digest.update(bytes);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn create_remote_registry_staging_dir(package_dir: &Path) -> Result<PathBuf> {
    let parent = package_dir
        .parent()
        .ok_or_else(|| miette::miette!("registry cache package path has no parent directory"))?;
    let version = package_dir
        .file_name()
        .ok_or_else(|| miette::miette!("registry cache package path has no version component"))?
        .to_string_lossy();
    for _ in 0..128 {
        let counter = REMOTE_REGISTRY_STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        let staging_dir = parent.join(format!(
            ".{version}.sgpm-fetch-{}-{counter}",
            std::process::id()
        ));
        match fs::create_dir(&staging_dir) {
            Ok(()) => return Ok(staging_dir),
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(err)
                    .into_diagnostic()
                    .with_context(|| format!("failed to create {}", staging_dir.display()))
            }
        }
    }
    miette::bail!(
        "failed to create a unique registry fetch staging directory under {}",
        parent.display()
    )
}

fn validate_registry_package_manifest(
    manifest_path: &Path,
    package_name: &str,
    version: &Version,
) -> Result<()> {
    let manifest = Manifest::load(manifest_path)?;
    if manifest.package.name != package_name {
        miette::bail!(
            "registry package '{}' version {} manifest declares package '{}'",
            package_name,
            version,
            manifest.package.name
        );
    }
    if manifest.package.version != version.to_string() {
        miette::bail!(
            "registry package '{}' version cache contains manifest version {}",
            package_name,
            manifest.package.version
        );
    }
    let root_dir = manifest_path.parent().ok_or_else(|| {
        miette::miette!(
            "registry package manifest has no parent directory: {}",
            manifest_path.display()
        )
    })?;
    validate_package_entries(root_dir, &manifest)?;
    Ok(())
}

fn resolve_git_dependency_manifest(
    parent_dir: &Path,
    git_cache_dir: &Path,
    dep: &Dependency,
    git_url: &str,
    refresh: bool,
) -> Result<(PathBuf, PackageSource)> {
    fs::create_dir_all(git_cache_dir)
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", git_cache_dir.display()))?;
    let clone_url = clone_url_for_git_dependency(parent_dir, git_url)?;
    let checkout_dir = git_cache_dir.join(git_cache_name(dep, &clone_url));
    if refresh || !checkout_dir.join(".git").exists() {
        clone_git_dependency_checkout(&checkout_dir, &clone_url, dep, refresh)?;
    }

    if let Some(rev) = dep.rev.as_deref() {
        run_git(
            &["checkout", "--quiet", rev],
            Some(&checkout_dir),
            &format!(
                "failed to checkout git dependency '{}' rev {}",
                dep.name, rev
            ),
        )?;
    }

    let resolved_rev = git_output(&["rev-parse", "HEAD"], &checkout_dir)
        .with_context(|| format!("failed to resolve git dependency '{}' revision", dep.name))?;
    let manifest = canonicalize_existing(&checkout_dir.join("Sengoo.toml"))?;
    Ok((
        manifest,
        PackageSource::Git {
            url: git_url.to_string(),
            rev: resolved_rev,
        },
    ))
}

fn clone_git_dependency_checkout(
    checkout_dir: &Path,
    clone_url: &str,
    dep: &Dependency,
    replace_existing: bool,
) -> Result<()> {
    let staging_dir = git_staging_dir(checkout_dir)?;
    let staging_arg = git_path_arg(&staging_dir);
    let clone_result = (|| {
        run_git(
            &["clone", "--quiet", clone_url, &staging_arg],
            None,
            &format!("failed to clone git dependency '{}'", dep.name),
        )?;
        if let Some(rev) = dep.rev.as_deref() {
            run_git(
                &["checkout", "--quiet", rev],
                Some(&staging_dir),
                &format!(
                    "failed to checkout git dependency '{}' rev {}",
                    dep.name, rev
                ),
            )?;
        }
        git_output(&["rev-parse", "HEAD"], &staging_dir)
            .with_context(|| format!("failed to resolve git dependency '{}' revision", dep.name))?;
        let manifest_path = canonicalize_existing(&staging_dir.join("Sengoo.toml"))
            .with_context(|| format!("git dependency '{}' has no Sengoo.toml", dep.name))?;
        let manifest = Manifest::load(&manifest_path)?;
        validate_dependency_package_name(dep, &manifest_path)?;
        validate_package_entries(&staging_dir, &manifest)?;

        if checkout_dir.exists() {
            if !replace_existing && checkout_dir.join(".git").exists() {
                return Ok(());
            }
            fs::remove_dir_all(checkout_dir)
                .into_diagnostic()
                .with_context(|| format!("failed to clean {}", checkout_dir.display()))?;
        }
        fs::rename(&staging_dir, checkout_dir)
            .into_diagnostic()
            .with_context(|| {
                format!(
                    "failed to finalize git dependency cache {} to {}",
                    staging_dir.display(),
                    checkout_dir.display()
                )
            })
    })();
    if clone_result.is_err() || staging_dir.exists() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
    clone_result
}

fn git_staging_dir(checkout_dir: &Path) -> Result<PathBuf> {
    let parent = checkout_dir
        .parent()
        .ok_or_else(|| miette::miette!("git dependency cache path has no parent directory"))?;
    let checkout_name = checkout_dir
        .file_name()
        .ok_or_else(|| miette::miette!("git dependency cache path has no checkout component"))?
        .to_string_lossy();
    for _ in 0..128 {
        let counter = GIT_STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        let staging_dir = parent.join(format!(
            ".{checkout_name}.sgpm-clone-{}-{counter}",
            std::process::id()
        ));
        if !staging_dir.exists() {
            return Ok(staging_dir);
        }
    }
    miette::bail!(
        "failed to create a unique git dependency staging path under {}",
        parent.display()
    )
}

fn clone_url_for_git_dependency(parent_dir: &Path, git_url: &str) -> Result<String> {
    if is_remote_git_url(git_url) {
        return Ok(git_url.to_string());
    }

    let git_path = Path::new(git_url);
    let source_path = if git_path.is_absolute() {
        git_path.to_path_buf()
    } else {
        parent_dir.join(git_path)
    };
    canonicalize_existing(&source_path).map(|path| git_path_arg(&path))
}

fn is_remote_git_url(git_url: &str) -> bool {
    git_url.contains("://") || git_url.starts_with("git@")
}

fn git_cache_name(dep: &Dependency, git_url: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(git_url.as_bytes());
    if let Some(rev) = &dep.rev {
        hasher.update(b"\0");
        hasher.update(rev.as_bytes());
    }
    let digest = format!("{:x}", hasher.finalize());
    format!("{}-{}", dep.name, &digest[..16])
}

fn sanitize_path_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn git_path_arg(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(r"\\?\")
        .replace('\\', "/")
}

fn run_git(args: &[&str], cwd: Option<&Path>, context: &str) -> Result<()> {
    let mut command = Command::new("git");
    command.args(args);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let status = command
        .status()
        .into_diagnostic()
        .with_context(|| format!("failed to start git {}", args.join(" ")))?;
    if !status.success() {
        miette::bail!("{} (git exit status: {})", context, status);
    }
    Ok(())
}

fn git_output(args: &[&str], cwd: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .into_diagnostic()
        .with_context(|| format!("failed to start git {}", args.join(" ")))?;
    if !output.status.success() {
        miette::bail!(
            "git {} failed (exit status: {})",
            args.join(" "),
            output.status
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
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

    #[test]
    fn alias_dependency_resolves_with_package_field() {
        let dir = temp_dir("alias_dep");
        let app = dir.join("app");
        let dep = dir.join("dep");
        write_pkg(&dep, "actual_name", &[]);
        fs::create_dir_all(app.join("src")).unwrap();
        fs::write(app.join("src/main.sg"), "def main() -> i64 { 0 }\n").unwrap();
        fs::write(
            app.join("Sengoo.toml"),
            "[package]\nname = 'app'\nversion = '0.1.0'\nedition = '2026'\n\n[bin]\npath = 'src/main.sg'\n\n[dependencies]\nmy_alias = { package = 'actual_name', path = '../dep' }\n",
        )
        .unwrap();

        let graph = Graph::from_root(&app.join("Sengoo.toml")).unwrap();
        let names = graph
            .nodes
            .iter()
            .map(|n| n.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["actual_name", "app"]);
        assert_eq!(graph.edges.len(), 1);
        assert_eq!(graph.edges[0].alias, "my_alias");
        assert!(graph.requires_lockfile_v2());
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn multiversion_graph_keeps_distinct_package_nodes() {
        let dir = temp_dir("multiversion");
        let app = dir.join("app");
        let b = dir.join("b");
        let c = dir.join("c");
        let foo_one = dir.join("foo_one");
        let foo_two = dir.join("foo_two");
        write_pkg_version(&foo_one, "foo", "1.0.0", &[]);
        write_pkg_version(&foo_two, "foo", "2.0.0", &[]);
        write_pkg(&b, "b", &[("foo", "../foo_one")]);
        write_pkg(&c, "c", &[("foo", "../foo_two")]);
        write_pkg(&app, "app", &[("b", "../b"), ("c", "../c")]);

        let graph = Graph::from_root(&app.join("Sengoo.toml")).unwrap();
        let foo_versions = graph
            .nodes
            .iter()
            .filter(|node| node.name == "foo")
            .map(|node| node.manifest.package.version.as_str())
            .collect::<BTreeSet<_>>();
        assert_eq!(foo_versions, BTreeSet::from(["1.0.0", "2.0.0"]));
        assert!(graph.requires_lockfile_v2());
        let _ = fs::remove_dir_all(dir);
    }

    fn write_pkg_version(root: &Path, name: &str, version: &str, deps: &[(&str, &str)]) {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(root.join("src/main.sg"), "def main() -> i64 { 0 }\n").unwrap();
        let mut text = format!("[package]\nname = '{}'\nversion = '{}'\n\n", name, version);
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
    fn rejects_path_dependency_key_that_differs_without_package_field() {
        let dir = temp_dir("path_name_mismatch");
        let app = dir.join("app");
        let dep = dir.join("dep");
        write_pkg(&dep, "actual_name", &[]);
        write_pkg(&app, "app", &[("alias", "../dep")]);

        let err = Graph::from_root(&app.join("Sengoo.toml")).unwrap_err();

        assert!(
            err.to_string()
                .contains("dependency 'alias' resolves to package 'actual_name'"),
            "unexpected diagnostic: {err}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_missing_default_binary_entry() {
        let dir = temp_dir("missing_default_entry");
        let app = dir.join("app");
        write_pkg(&app, "app", &[]);
        fs::remove_file(app.join("src/main.sg")).unwrap();

        let err = Graph::from_root(&app.join("Sengoo.toml")).unwrap_err();

        assert!(
            err.to_string()
                .contains("package 'app' [bin] entry does not exist"),
            "unexpected diagnostic: {err}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_missing_declared_library_entry() {
        let dir = temp_dir("missing_library_entry");
        let app = dir.join("app");
        fs::create_dir_all(&app).unwrap();
        fs::write(
            app.join("Sengoo.toml"),
            "[package]\nname = 'app'\nversion = '0.1.0'\n\n[lib]\npath = 'src/lib.sg'\n",
        )
        .unwrap();

        let err = Graph::from_root(&app.join("Sengoo.toml")).unwrap_err();

        assert!(
            err.to_string()
                .contains("package 'app' [lib] entry does not exist"),
            "unexpected diagnostic: {err}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn rejects_entry_outside_package_root() {
        let dir = temp_dir("outside_entry");
        let app = dir.join("app");
        fs::create_dir_all(&app).unwrap();
        fs::write(dir.join("outside.sg"), "def main() -> i64 { 0 }\n").unwrap();
        fs::write(
            app.join("Sengoo.toml"),
            "[package]\nname = 'app'\nversion = '0.1.0'\n\n[bin]\npath = '../outside.sg'\n",
        )
        .unwrap();

        let err = Graph::from_root(&app.join("Sengoo.toml")).unwrap_err();

        assert!(
            err.to_string()
                .contains("entry must stay within the package root"),
            "unexpected diagnostic: {err}"
        );
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn failed_remote_registry_unpack_does_not_leave_partial_cache() {
        let dir = temp_dir("remote_unpack_cleanup");
        let version = Version::parse("1.0.0").unwrap();
        let err = unpack_remote_registry_archive(
            &dir,
            "default",
            "foo",
            &version,
            b"not a gzip archive",
            None,
        )
        .unwrap_err();

        assert!(err.to_string().contains("failed to unpack remote package"));
        assert!(!dir.join("default/foo/1.0.0").exists());
        let _ = fs::remove_dir_all(dir);
    }
}
