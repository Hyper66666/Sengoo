use miette::{IntoDiagnostic, Result};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;

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

fn frontend_session_store_fallback_path(path: &Path) -> PathBuf {
    let mut hasher = DefaultHasher::new();
    path.to_string_lossy().hash(&mut hasher);
    let key = hasher.finish();
    let file_stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("frontend-session");
    std::env::temp_dir()
        .join("sengoo")
        .join("frontend-session-cache")
        .join(format!("{file_stem}-{key:016x}.json"))
}

fn file_mtime(path: &Path) -> Option<std::time::SystemTime> {
    fs::metadata(path).ok()?.modified().ok()
}

pub(crate) fn load_frontend_session_store(path: &Path) -> Option<FrontendSessionStoreV4> {
    let fallback = frontend_session_store_fallback_path(path);
    let candidate = match (file_mtime(path), file_mtime(&fallback)) {
        (Some(primary), Some(secondary)) if secondary >= primary => &fallback,
        (Some(_), _) => path,
        (None, Some(_)) => &fallback,
        (None, None) => path,
    };
    let bytes = fs::read(candidate).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn metadata_temp_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("metadata.json");
    path.with_file_name(format!("{file_name}.tmp-{}", std::process::id()))
}

fn replace_file_with_retry(path: &Path, temp_path: &Path) -> io::Result<()> {
    const WINDOWS_PERMISSION_RETRIES: usize = 4;

    for attempt in 0..=WINDOWS_PERMISSION_RETRIES {
        if path.exists() {
            match fs::remove_file(path) {
                Ok(()) => {}
                Err(err) if err.kind() == io::ErrorKind::NotFound => {}
                Err(err)
                    if cfg!(windows)
                        && err.kind() == io::ErrorKind::PermissionDenied
                        && attempt < WINDOWS_PERMISSION_RETRIES =>
                {
                    thread::sleep(Duration::from_millis(25));
                    continue;
                }
                Err(err) => return Err(err),
            }
        }

        match fs::rename(temp_path, path) {
            Ok(()) => return Ok(()),
            Err(err)
                if cfg!(windows)
                    && err.kind() == io::ErrorKind::PermissionDenied
                    && attempt < WINDOWS_PERMISSION_RETRIES =>
            {
                thread::sleep(Duration::from_millis(25));
            }
            Err(err) => return Err(err),
        }
    }

    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        format!("failed to replace metadata file {}", path.display()),
    ))
}

fn write_metadata_file(path: &Path, bytes: &[u8], label: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).into_diagnostic()?;
    }

    let temp_path = metadata_temp_path(path);
    fs::write(&temp_path, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to stage {}: {}", label, e))?;

    if let Err(err) = replace_file_with_retry(path, &temp_path) {
        let _ = fs::remove_file(&temp_path);
        return Err(miette::miette!("failed to write {}: {}", label, err));
    }

    Ok(())
}

pub(crate) fn save_run_cache(path: &Path, metadata: &RunCacheMetadata) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|e| miette::miette!("failed to serialize run cache metadata: {}", e))?;
    write_metadata_file(path, &bytes, "run cache metadata")
}

pub(crate) fn save_build_cache(path: &Path, metadata: &BuildCacheMetadata) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|e| miette::miette!("failed to serialize build cache metadata: {}", e))?;
    write_metadata_file(path, &bytes, "build cache metadata")
}

pub(crate) fn save_frontend_session_store(
    path: &Path,
    metadata: &FrontendSessionStoreV4,
) -> Result<()> {
    let bytes = serde_json::to_vec_pretty(metadata)
        .map_err(|e| miette::miette!("failed to serialize frontend session metadata: {}", e))?;
    if let Err(primary_err) = write_metadata_file(path, &bytes, "frontend session metadata") {
        let fallback = frontend_session_store_fallback_path(path);
        write_metadata_file(&fallback, &bytes, "frontend session metadata fallback").map_err(
            |fallback_err| {
                miette::miette!(
                    "failed to write frontend session metadata: {}; fallback {} also failed: {}",
                    primary_err,
                    fallback.display(),
                    fallback_err
                )
            },
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_test_dir(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "sengoo-cache-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    #[test]
    fn write_metadata_file_creates_parent_directories() {
        let root = temp_test_dir("parent");
        let target = root.join("nested").join("data.json");

        write_metadata_file(&target, br#"{"ok":true}"#, "test metadata").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "{\"ok\":true}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn write_metadata_file_replaces_existing_contents() {
        let root = temp_test_dir("replace");
        let target = root.join("data.json");
        fs::create_dir_all(&root).unwrap();
        fs::write(&target, b"old").unwrap();

        write_metadata_file(&target, b"new", "test metadata").unwrap();

        assert_eq!(fs::read_to_string(&target).unwrap(), "new");
        assert!(!metadata_temp_path(&target).exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn load_frontend_session_store_prefers_newer_fallback_file() {
        let root = temp_test_dir("frontend-fallback-load");
        let primary = root
            .join("build")
            .join("workset")
            .join("sample.frontend-session-v4.json");
        let fallback = frontend_session_store_fallback_path(&primary);

        fs::create_dir_all(primary.parent().unwrap()).unwrap();
        fs::create_dir_all(fallback.parent().unwrap()).unwrap();
        fs::write(
            &primary,
            br#"{"compiler_version":"primary","root_module":"m"}"#,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(20));
        fs::write(
            &fallback,
            br#"{"compiler_version":"fallback","root_module":"m"}"#,
        )
        .unwrap();

        let loaded = load_frontend_session_store(&primary).unwrap();
        assert_eq!(loaded.compiler_version, "fallback");

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_file(fallback);
    }

    #[test]
    fn save_frontend_session_store_uses_temp_fallback_when_primary_path_is_unwritable() {
        let root = temp_test_dir("frontend-fallback-save");
        let blocked_parent = root.join("blocked");
        let primary = blocked_parent
            .join("child")
            .join("sample.frontend-session-v4.json");
        let fallback = frontend_session_store_fallback_path(&primary);
        let metadata = FrontendSessionStoreV4 {
            schema_version: 4,
            scheduler_schema_version: 1,
            dependency_graph_digest: 7,
            compiler_version: "test".to_string(),
            root_module: "main".to_string(),
            modules: Vec::new(),
        };

        fs::create_dir_all(&root).unwrap();
        fs::write(&blocked_parent, b"occupied").unwrap();

        save_frontend_session_store(&primary, &metadata).unwrap();

        let loaded = load_frontend_session_store(&primary).unwrap();
        assert_eq!(loaded.compiler_version, "test");
        assert!(fallback.exists());

        let _ = fs::remove_dir_all(&root);
        let _ = fs::remove_file(fallback);
    }
}
