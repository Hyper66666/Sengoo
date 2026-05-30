use crate::resolver::resolve_manifest_path;
use miette::{Context, IntoDiagnostic, Result};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct CacheEntry {
    pub kind: &'static str,
    pub name: String,
    pub path: PathBuf,
}

pub fn list(manifest_path: &Path) -> Result<Vec<CacheEntry>> {
    let mut entries = list_git(manifest_path)?;
    entries.extend(list_registry(manifest_path)?);
    entries.sort_by(|left, right| {
        left.kind
            .cmp(right.kind)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(entries)
}

fn list_git(manifest_path: &Path) -> Result<Vec<CacheEntry>> {
    let git_dir = git_cache_dir(manifest_path)?;
    if !git_dir.exists() {
        return Ok(Vec::new());
    }

    let mut entries = fs::read_dir(&git_dir)
        .into_diagnostic()
        .with_context(|| format!("failed to read {}", git_dir.display()))?
        .map(|entry| {
            let entry = entry
                .into_diagnostic()
                .with_context(|| format!("failed to read {}", git_dir.display()))?;
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            Ok(CacheEntry {
                kind: "git",
                name,
                path,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    entries.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(entries)
}

fn list_registry(manifest_path: &Path) -> Result<Vec<CacheEntry>> {
    let registry_dir = registry_cache_dir(manifest_path)?;
    let mut entries = Vec::new();
    for registry in child_dirs(&registry_dir)? {
        for package in child_dirs(&registry)? {
            for version in child_dirs(&package)? {
                let name = version
                    .strip_prefix(&registry_dir)
                    .into_diagnostic()?
                    .components()
                    .map(|component| component.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                entries.push(CacheEntry {
                    kind: "registry",
                    name,
                    path: version,
                });
            }
        }
    }
    Ok(entries)
}

fn child_dirs(path: &Path) -> Result<Vec<PathBuf>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let mut dirs = fs::read_dir(path)
        .into_diagnostic()
        .with_context(|| format!("failed to read {}", path.display()))?
        .map(|entry| {
            entry
                .into_diagnostic()
                .with_context(|| format!("failed to read {}", path.display()))
                .map(|entry| entry.path())
        })
        .collect::<Result<Vec<_>>>()?;
    dirs.retain(|entry| entry.is_dir());
    dirs.sort();
    Ok(dirs)
}

pub fn clean_git(manifest_path: &Path) -> Result<Option<PathBuf>> {
    let git_dir = git_cache_dir(manifest_path)?;
    clean_dir(git_dir)
}

pub fn clean_registry(manifest_path: &Path) -> Result<Option<PathBuf>> {
    let registry_dir = registry_cache_dir(manifest_path)?;
    clean_dir(registry_dir)
}

fn clean_dir(path: PathBuf) -> Result<Option<PathBuf>> {
    if !path.exists() {
        return Ok(None);
    }
    fs::remove_dir_all(&path)
        .into_diagnostic()
        .with_context(|| format!("failed to remove {}", path.display()))?;
    Ok(Some(path))
}

fn git_cache_dir(manifest_path: &Path) -> Result<PathBuf> {
    Ok(cache_root_dir(manifest_path)?.join("git"))
}

fn registry_cache_dir(manifest_path: &Path) -> Result<PathBuf> {
    Ok(cache_root_dir(manifest_path)?.join("registry"))
}

fn cache_root_dir(manifest_path: &Path) -> Result<PathBuf> {
    let manifest = resolve_manifest_path(manifest_path)?;
    let root_dir = manifest.parent().ok_or_else(|| {
        miette::miette!("manifest has no parent directory: {}", manifest.display())
    })?;
    Ok(root_dir.join("target").join("sgpm"))
}
