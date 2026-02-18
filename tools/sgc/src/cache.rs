use miette::{IntoDiagnostic, Result};
use std::fs;
use std::path::{Path, PathBuf};

use crate::{BuildCacheMetadata, FrontendSessionStoreV4, RunCacheMetadata};

pub(crate) fn load_run_cache(path: &Path) -> Option<RunCacheMetadata> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn load_build_cache(path: &Path) -> Option<BuildCacheMetadata> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn frontend_session_store_path(build_dir: &Path, stem: &str) -> PathBuf {
    build_dir
        .join("workset")
        .join(format!("{}.frontend-session-v4.json", stem))
}

pub(crate) fn load_frontend_session_store(path: &Path) -> Option<FrontendSessionStoreV4> {
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub(crate) fn save_run_cache(path: &Path, metadata: &RunCacheMetadata) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|e| miette::miette!("failed to serialize run cache metadata: {}", e))?;
    fs::write(path, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write run cache metadata: {}", e))
}

pub(crate) fn save_build_cache(path: &Path, metadata: &BuildCacheMetadata) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|e| miette::miette!("failed to serialize build cache metadata: {}", e))?;
    fs::write(path, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write build cache metadata: {}", e))
}

pub(crate) fn save_frontend_session_store(
    path: &Path,
    metadata: &FrontendSessionStoreV4,
) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).into_diagnostic()?;
    }
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|e| miette::miette!("failed to serialize frontend session metadata: {}", e))?;
    fs::write(path, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write frontend session metadata: {}", e))
}
