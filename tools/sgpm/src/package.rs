use crate::resolver::Graph;
use flate2::write::GzEncoder;
use flate2::Compression;
use miette::{Context, IntoDiagnostic, Result};
use reqwest::header::{HeaderMap, HeaderValue, AUTHORIZATION, CONTENT_TYPE};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use tar::Builder;
use tokio::runtime::Builder as TokioRuntimeBuilder;
use walkdir::{DirEntry, WalkDir};

static PUBLISH_STAGING_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone)]
pub struct PackageArtifact {
    pub archive_path: PathBuf,
    pub checksum_path: PathBuf,
    pub checksum: String,
}

#[derive(Debug, Clone)]
pub struct RegistryPublish {
    pub name: String,
    pub version: String,
    pub target_dir: PathBuf,
}

#[derive(Debug, Clone)]
pub struct RemoteRegistryPublish {
    pub name: String,
    pub version: String,
    pub endpoint: String,
    pub checksum: String,
}

#[derive(Debug, Clone)]
pub enum RegistryPublishResult {
    Local(RegistryPublish),
    Remote(RemoteRegistryPublish),
}

pub fn publish_dry_run(graph: &Graph, output_dir: Option<&Path>) -> Result<PackageArtifact> {
    let root = graph
        .root_package()
        .ok_or_else(|| miette::miette!("dependency graph has no root package"))?;
    let version = &root.manifest.package.version;
    let package_name = format!("{}-{}.tar.gz", root.name, version);
    let output_dir = resolve_output_dir(&root.root_dir, output_dir);
    fs::create_dir_all(&output_dir)
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", output_dir.display()))?;
    let files = package_files(&root.root_dir, std::slice::from_ref(&output_dir))?;

    let archive_path = output_dir.join(&package_name);
    let archive_file = File::create(&archive_path)
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", archive_path.display()))?;
    let encoder = GzEncoder::new(archive_file, Compression::default());
    let mut builder = Builder::new(encoder);

    for file in files {
        let rel = file.strip_prefix(&root.root_dir).into_diagnostic()?;
        let package_path = package_path(rel);
        builder
            .append_path_with_name(&file, &package_path)
            .into_diagnostic()
            .with_context(|| {
                format!(
                    "failed to add {} as {}",
                    file.display(),
                    package_path.display()
                )
            })?;
    }

    let encoder = builder
        .into_inner()
        .into_diagnostic()
        .with_context(|| format!("failed to finish {}", archive_path.display()))?;
    encoder
        .finish()
        .into_diagnostic()
        .with_context(|| format!("failed to write {}", archive_path.display()))?;

    let archive_bytes = fs::read(&archive_path)
        .into_diagnostic()
        .with_context(|| format!("failed to read {}", archive_path.display()))?;
    let checksum = format!("{:x}", Sha256::digest(&archive_bytes));
    let checksum_path = archive_path.with_file_name(format!("{}.sha256", package_name));
    let checksum_body = format!("{}  {}\n", checksum, package_name);
    fs::write(&checksum_path, checksum_body)
        .into_diagnostic()
        .with_context(|| format!("failed to write {}", checksum_path.display()))?;

    Ok(PackageArtifact {
        archive_path,
        checksum_path,
        checksum,
    })
}

fn resolve_output_dir(root_dir: &Path, output_dir: Option<&Path>) -> PathBuf {
    output_dir
        .map(|path| {
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                root_dir.join(path)
            }
        })
        .unwrap_or_else(|| root_dir.join("target").join("package"))
}

pub fn publish_to_local_registry(graph: &Graph, registry_name: &str) -> Result<RegistryPublish> {
    let root = graph
        .root_package()
        .ok_or_else(|| miette::miette!("dependency graph has no root package"))?;
    let registry = graph.registries.get(registry_name).ok_or_else(|| {
        miette::miette!(
            "registry '{}' is not configured; add [registries.{}] to Sengoo.toml or the selected workspace manifest",
            registry_name,
            registry_name
        )
    })?;
    let registry_path = registry.path.as_ref().ok_or_else(|| {
        miette::miette!(
            "registry '{}' is configured with url; use remote publish handling instead",
            registry_name
        )
    })?;
    let registry_root = resolve_registry_dir(&root.root_dir, registry_path);
    fs::create_dir_all(&registry_root)
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", registry_root.display()))?;
    let registry_root = fs::canonicalize(&registry_root)
        .into_diagnostic()
        .with_context(|| format!("failed to resolve {}", registry_root.display()))?;
    let target_dir = registry_root
        .join(&root.name)
        .join(&root.manifest.package.version);
    if target_dir.exists() {
        miette::bail!(
            "package '{}' version '{}' already exists in registry '{}': {}",
            root.name,
            root.manifest.package.version,
            registry_name,
            target_dir.display()
        );
    }

    let exclusions = if registry_root.starts_with(&root.root_dir) {
        vec![registry_root.clone()]
    } else {
        Vec::new()
    };
    let files = package_files(&root.root_dir, &exclusions)?;
    publish_local_files(&root.root_dir, &target_dir, &files)?;

    Ok(RegistryPublish {
        name: root.name.clone(),
        version: root.manifest.package.version.clone(),
        target_dir,
    })
}

fn publish_local_files(root_dir: &Path, target_dir: &Path, files: &[PathBuf]) -> Result<()> {
    let parent = target_dir
        .parent()
        .ok_or_else(|| miette::miette!("registry publish target has no parent directory"))?;
    fs::create_dir_all(parent)
        .into_diagnostic()
        .with_context(|| format!("failed to create {}", parent.display()))?;
    let staging_dir = create_publish_staging_dir(target_dir)?;
    let publish_result = (|| {
        copy_package_files(root_dir, &staging_dir, files)?;
        fs::rename(&staging_dir, target_dir)
            .into_diagnostic()
            .with_context(|| {
                format!(
                    "failed to finalize registry publish {} to {}",
                    staging_dir.display(),
                    target_dir.display()
                )
            })
    })();
    if publish_result.is_err() {
        let _ = fs::remove_dir_all(&staging_dir);
    }
    publish_result
}

fn create_publish_staging_dir(target_dir: &Path) -> Result<PathBuf> {
    let parent = target_dir
        .parent()
        .ok_or_else(|| miette::miette!("registry publish target has no parent directory"))?;
    let version = target_dir
        .file_name()
        .ok_or_else(|| miette::miette!("registry publish target has no version component"))?
        .to_string_lossy();
    for _ in 0..128 {
        let counter = PUBLISH_STAGING_COUNTER.fetch_add(1, Ordering::Relaxed);
        let staging_dir = parent.join(format!(
            ".{version}.sgpm-publish-{}-{counter}",
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
        "failed to create a unique registry publish staging directory under {}",
        parent.display()
    )
}

fn copy_package_files(root_dir: &Path, target_dir: &Path, files: &[PathBuf]) -> Result<()> {
    for file in files {
        let rel = file.strip_prefix(root_dir).into_diagnostic()?;
        let dest = target_dir.join(rel);
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .into_diagnostic()
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        fs::copy(file, &dest).into_diagnostic().with_context(|| {
            format!("failed to publish {} to {}", file.display(), dest.display())
        })?;
    }
    Ok(())
}

pub fn publish_to_registry(graph: &Graph, registry_name: &str) -> Result<RegistryPublishResult> {
    let registry = graph.registries.get(registry_name).ok_or_else(|| {
        miette::miette!(
            "registry '{}' is not configured; add [registries.{}] to Sengoo.toml or the selected workspace manifest",
            registry_name,
            registry_name
        )
    })?;
    if registry.path.is_some() {
        return publish_to_local_registry(graph, registry_name).map(RegistryPublishResult::Local);
    }
    publish_to_remote_registry(graph, registry_name).map(RegistryPublishResult::Remote)
}

pub fn publish_to_remote_registry(
    graph: &Graph,
    registry_name: &str,
) -> Result<RemoteRegistryPublish> {
    let root = graph
        .root_package()
        .ok_or_else(|| miette::miette!("dependency graph has no root package"))?;
    let registry = graph.registries.get(registry_name).ok_or_else(|| {
        miette::miette!(
            "registry '{}' is not configured; add [registries.{}] to Sengoo.toml or the selected workspace manifest",
            registry_name,
            registry_name
        )
    })?;
    let url = registry
        .url
        .as_deref()
        .map(str::trim)
        .filter(|url| !url.is_empty())
        .ok_or_else(|| {
            miette::miette!(
                "registry '{}' is configured with path; use local publish handling instead",
                registry_name
            )
        })?;
    let artifact = publish_dry_run(graph, None)?;
    let archive_bytes = fs::read(&artifact.archive_path)
        .into_diagnostic()
        .with_context(|| format!("failed to read {}", artifact.archive_path.display()))?;
    let endpoint = format!(
        "{}/api/v1/packages/{}/{}",
        url.trim_end_matches('/'),
        root.name,
        root.manifest.package.version
    );
    let mut headers = HeaderMap::new();
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/gzip"));
    headers.insert(
        "x-sengoo-package",
        HeaderValue::from_str(&root.name).into_diagnostic()?,
    );
    headers.insert(
        "x-sengoo-version",
        HeaderValue::from_str(&root.manifest.package.version).into_diagnostic()?,
    );
    headers.insert(
        "x-sengoo-checksum",
        HeaderValue::from_str(&artifact.checksum).into_diagnostic()?,
    );
    if let Some(token_env) = registry.token_env.as_deref() {
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
    }

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
            .post(&endpoint)
            .headers(headers)
            .body(archive_bytes)
            .send()
            .await
            .into_diagnostic()
            .with_context(|| format!("failed to upload package to {}", endpoint))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            miette::bail!(
                "remote registry '{}' rejected package {} v{} with status {}: {}",
                registry_name,
                root.name,
                root.manifest.package.version,
                status,
                body
            );
        }
        Ok(())
    })?;

    Ok(RemoteRegistryPublish {
        name: root.name.clone(),
        version: root.manifest.package.version.clone(),
        endpoint,
        checksum: artifact.checksum,
    })
}

fn resolve_registry_dir(root_dir: &Path, registry_path: &Path) -> PathBuf {
    if registry_path.is_absolute() {
        registry_path.to_path_buf()
    } else {
        root_dir.join(registry_path)
    }
}

fn package_files(root_dir: &Path, excluded_dirs: &[PathBuf]) -> Result<Vec<PathBuf>> {
    let excluded_dirs = excluded_dirs
        .iter()
        .filter_map(|path| fs::canonicalize(path).ok())
        .filter(|path| path.starts_with(root_dir))
        .collect::<Vec<_>>();
    let mut files = WalkDir::new(root_dir)
        .into_iter()
        .filter_entry(|entry| keep_package_entry(entry, &excluded_dirs))
        .map(|entry| entry.into_diagnostic())
        .collect::<Result<Vec<_>>>()
        .with_context(|| format!("failed to enumerate package {}", root_dir.display()))?
        .into_iter()
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| entry.into_path())
        .collect::<Vec<_>>();
    files.sort();
    Ok(files)
}

fn keep_package_entry(entry: &DirEntry, excluded_dirs: &[PathBuf]) -> bool {
    if excluded_dirs
        .iter()
        .any(|path| entry.path() == path || entry.path().starts_with(path))
    {
        return false;
    }
    if entry.depth() == 0 || !entry.file_type().is_dir() {
        return true;
    }
    let name = entry.file_name().to_string_lossy();
    name != ".git" && name != "target"
}

fn package_path(path: &Path) -> PathBuf {
    path.components()
        .map(|component| component.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
        .into()
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
        std::env::temp_dir().join(format!("sgpm_package_{name}_{stamp}"))
    }

    #[test]
    fn local_registry_staging_is_removed_when_copy_fails() {
        let dir = temp_dir("publish_cleanup");
        let root = dir.join("app");
        let source_dir = root.join("src");
        fs::create_dir_all(&source_dir).unwrap();
        let manifest = root.join("Sengoo.toml");
        let source = source_dir.join("main.sg");
        fs::write(&manifest, "[package]\nname = 'app'\nversion = '0.1.0'\n").unwrap();
        fs::write(&source, "def main() -> i64 { 0 }\n").unwrap();

        let target_dir = dir.join("registry/app/0.1.0");
        let files = vec![manifest, source.clone()];
        fs::remove_file(source).unwrap();

        let err = publish_local_files(&root, &target_dir, &files).unwrap_err();
        assert!(err.to_string().contains("failed to publish"));
        assert!(!target_dir.exists());
        assert!(
            fs::read_dir(target_dir.parent().unwrap())
                .unwrap()
                .next()
                .is_none(),
            "failed publish should remove staging directory"
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn package_files_rejects_missing_root() {
        let root = temp_dir("missing_root");
        let err = package_files(&root, &[]).unwrap_err();
        assert!(err.to_string().contains("failed to enumerate package"));
    }
}
