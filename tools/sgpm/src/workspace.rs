use crate::manifest::{Manifest, RegistryConfig, WorkspaceManifest};
use crate::resolver::resolve_manifest_path;
use miette::{Context, IntoDiagnostic, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SelectedManifest {
    pub manifest_path: PathBuf,
    pub inherited_registries: BTreeMap<String, RegistryConfig>,
}

#[derive(Debug, Clone)]
struct WorkspaceMember {
    name: String,
    manifest_path: PathBuf,
}

pub fn select_manifest(manifest_path: &Path, package: Option<&str>) -> Result<SelectedManifest> {
    let mut selected = select_manifests(manifest_path, package, false)?;
    selected
        .pop()
        .ok_or_else(|| miette::miette!("no workspace member manifests selected"))
}

pub fn select_manifests(
    manifest_path: &Path,
    package: Option<&str>,
    workspace_all: bool,
) -> Result<Vec<SelectedManifest>> {
    let requested = normalized_package(package)?;
    if workspace_all && requested.is_some() {
        miette::bail!("--workspace cannot be combined with --package");
    }

    let root_manifest = resolve_manifest_path(manifest_path)?;
    if !has_workspace_table(&root_manifest)? {
        if workspace_all {
            miette::bail!("--workspace requires a workspace manifest");
        }
        validate_package_request(&root_manifest, requested)?;
        return Ok(vec![SelectedManifest {
            manifest_path: root_manifest,
            inherited_registries: BTreeMap::new(),
        }]);
    }

    let workspace = WorkspaceManifest::load(&root_manifest)?;
    let workspace_dir = root_manifest.parent().ok_or_else(|| {
        miette::miette!(
            "workspace manifest has no parent directory: {}",
            root_manifest.display()
        )
    })?;
    let members = load_members(workspace_dir, &workspace.members)?;
    let inherited_registries = absolutize_registries(workspace_dir, workspace.registries);

    if workspace_all {
        return Ok(members
            .into_iter()
            .map(|member| SelectedManifest {
                manifest_path: member.manifest_path,
                inherited_registries: inherited_registries.clone(),
            })
            .collect());
    }

    let manifest_path = select_member_manifest(&root_manifest, &members, requested)?;
    Ok(vec![SelectedManifest {
        manifest_path,
        inherited_registries,
    }])
}

fn normalized_package(package: Option<&str>) -> Result<Option<&str>> {
    match package {
        Some(value) if value.trim().is_empty() => miette::bail!("--package must not be empty"),
        Some(value) => Ok(Some(value.trim())),
        None => Ok(None),
    }
}

fn has_workspace_table(manifest_path: &Path) -> Result<bool> {
    let source = fs::read_to_string(manifest_path)
        .into_diagnostic()
        .with_context(|| format!("failed to read manifest {}", manifest_path.display()))?;
    let value: toml::Value = toml::from_str(&source)
        .into_diagnostic()
        .with_context(|| format!("failed to inspect manifest {}", manifest_path.display()))?;
    Ok(value.get("workspace").is_some())
}

fn validate_package_request(manifest_path: &Path, requested: Option<&str>) -> Result<()> {
    if let Some(name) = requested {
        let manifest = Manifest::load(manifest_path)?;
        if manifest.package.name != name {
            miette::bail!(
                "package manifest declares '{}', but --package requested '{}'",
                manifest.package.name,
                name
            );
        }
    }
    Ok(())
}

fn load_members(root_dir: &Path, member_patterns: &[PathBuf]) -> Result<Vec<WorkspaceMember>> {
    let mut paths = BTreeSet::new();
    for pattern in member_patterns {
        for path in expand_member(root_dir, pattern)? {
            paths.insert(path);
        }
    }
    if paths.is_empty() {
        miette::bail!("workspace has no members matching [workspace].members");
    }

    let mut members = Vec::new();
    let mut names = BTreeMap::<String, PathBuf>::new();
    for path in paths {
        let manifest = Manifest::load(&path)
            .with_context(|| format!("failed to load workspace member {}", path.display()))?;
        if let Some(existing) = names.get(&manifest.package.name) {
            miette::bail!(
                "workspace has multiple member packages named '{}': {} and {}",
                manifest.package.name,
                existing.display(),
                path.display()
            );
        }
        names.insert(manifest.package.name.clone(), path.clone());
        members.push(WorkspaceMember {
            name: manifest.package.name,
            manifest_path: path,
        });
    }
    members.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.manifest_path.cmp(&right.manifest_path))
    });
    Ok(members)
}

fn expand_member(root_dir: &Path, member: &Path) -> Result<Vec<PathBuf>> {
    let rendered = member.to_string_lossy().replace('\\', "/");
    if let Some(prefix) = rendered.strip_suffix("/*") {
        let dir = if prefix.is_empty() {
            root_dir.to_path_buf()
        } else {
            root_dir.join(prefix)
        };
        return expand_direct_children(&dir);
    }
    if rendered.contains('*') {
        miette::bail!(
            "workspace member pattern '{}' is unsupported; only trailing /* is supported",
            rendered
        );
    }

    let path = if member.is_absolute() {
        member.to_path_buf()
    } else {
        root_dir.join(member)
    };
    resolve_manifest_path(&path).map(|path| vec![path])
}

fn expand_direct_children(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut manifests = Vec::new();
    for entry in fs::read_dir(dir).into_diagnostic().with_context(|| {
        format!(
            "failed to read workspace member directory {}",
            dir.display()
        )
    })? {
        let entry = entry.into_diagnostic().with_context(|| {
            format!(
                "failed to read workspace member directory {}",
                dir.display()
            )
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest = path.join("Sengoo.toml");
        if manifest.exists() {
            manifests.push(resolve_manifest_path(&manifest)?);
        }
    }
    manifests.sort();
    Ok(manifests)
}

fn select_member_manifest(
    root_manifest: &Path,
    members: &[WorkspaceMember],
    requested: Option<&str>,
) -> Result<PathBuf> {
    if let Some(name) = requested {
        let matches = members
            .iter()
            .filter(|member| member.name == name)
            .collect::<Vec<_>>();
        return match matches.as_slice() {
            [member] => Ok(member.manifest_path.clone()),
            [] => miette::bail!("workspace has no member package named '{}'", name),
            _ => miette::bail!(
                "workspace has multiple member packages named '{}'; use unique package names",
                name
            ),
        };
    }

    if let [member] = members {
        return Ok(member.manifest_path.clone());
    }

    let available = members
        .iter()
        .map(|member| member.name.as_str())
        .collect::<Vec<_>>()
        .join(", ");
    miette::bail!(
        "workspace manifest {} has multiple members; pass --package <name> (available: {})",
        root_manifest.display(),
        available
    )
}

fn absolutize_registries(
    root_dir: &Path,
    registries: BTreeMap<String, RegistryConfig>,
) -> BTreeMap<String, RegistryConfig> {
    registries
        .into_iter()
        .map(|(name, config)| {
            let path = config.path.map(|path| {
                if path.is_absolute() {
                    path
                } else {
                    root_dir.join(path)
                }
            });
            (
                name,
                RegistryConfig {
                    path,
                    url: config.url,
                    token_env: config.token_env,
                },
            )
        })
        .collect()
}
