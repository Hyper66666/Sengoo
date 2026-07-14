use crate::{NativeBuildTarget, NativeRuntimeProvenance};
use clap::ValueEnum;
use miette::{IntoDiagnostic, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::sync::OnceLock;

pub(crate) const NATIVE_RUNTIME_ABI_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum NativeRuntimeMode {
    Installed,
    SourceDevelopment,
}

static NATIVE_RUNTIME_MODE: OnceLock<NativeRuntimeMode> = OnceLock::new();

pub(crate) fn initialize_native_runtime_mode(mode: NativeRuntimeMode) -> Result<()> {
    NATIVE_RUNTIME_MODE
        .set(mode)
        .map_err(|_| miette::miette!("native runtime mode is already initialized"))
}

pub(crate) fn native_runtime_mode() -> NativeRuntimeMode {
    NATIVE_RUNTIME_MODE.get().copied().unwrap_or({
        if cfg!(test) {
            NativeRuntimeMode::SourceDevelopment
        } else {
            NativeRuntimeMode::Installed
        }
    })
}

#[derive(Debug, Deserialize)]
struct InstalledToolchainManifest {
    schema_version: u32,
    target: String,
    build_manifest_id: String,
    #[serde(default)]
    artifact_provenance: String,
    #[serde(default)]
    release_eligible: bool,
    payloads: Vec<InstalledPayloadManifest>,
    native_runtime: Option<InstalledNativeRuntimeManifest>,
}

#[derive(Debug, Deserialize)]
struct InstalledPayloadManifest {
    path: String,
    sha256: String,
}

#[derive(Debug, Deserialize)]
struct InstalledNativeRuntimeManifest {
    abi_version: u32,
    target: String,
    library: String,
    sha256: String,
    link_args: Vec<String>,
    dynamic_dependencies: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct InstalledNativeRuntime {
    pub(crate) library: PathBuf,
    pub(crate) cache_fingerprint: u64,
    pub(crate) provenance: NativeRuntimeProvenance,
}

fn install_root_from_current_exe() -> Option<PathBuf> {
    let executable = std::env::current_exe().ok()?;
    executable.parent()?.parent().map(Path::to_path_buf)
}

fn manifest_path_from_current_exe() -> Option<PathBuf> {
    let path = install_root_from_current_exe()?.join("manifest.json");
    path.is_file().then_some(path)
}

fn current_exe_is_source_local() -> bool {
    let Some(workspace_root) = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2) else {
        return false;
    };
    let Ok(workspace_root) = workspace_root.canonicalize() else {
        return false;
    };
    let Ok(executable) = std::env::current_exe().and_then(|path| path.canonicalize()) else {
        return false;
    };
    executable.starts_with(workspace_root)
}

pub(crate) fn validate_native_runtime_mode_environment() -> Result<()> {
    if native_runtime_mode() == NativeRuntimeMode::SourceDevelopment {
        return Ok(());
    }
    for variable in ["SENGOO_ROOT", "SENGOO_STDLIB", "SENGOO_RUNTIME"] {
        if std::env::var_os(variable).is_some() {
            return Err(miette::miette!(
                "installed runtime mode rejects {variable}; use --runtime-mode source-development inside the Sengoo source workspace for local runtime overrides"
            ));
        }
    }
    Ok(())
}

fn validate_relative_payload_path(path: &str) -> Result<PathBuf> {
    let relative = PathBuf::from(path);
    if relative.as_os_str().is_empty()
        || relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(miette::miette!(
            "installed native runtime library path must be a normalized relative payload path: {path}"
        ));
    }
    Ok(relative)
}

fn expected_link_args(target: &NativeBuildTarget) -> Vec<String> {
    if target.is_windows_msvc() {
        [
            "kernel32.lib",
            "ntdll.lib",
            "userenv.lib",
            "ws2_32.lib",
            "dbghelp.lib",
            "advapi32.lib",
            "bcrypt.lib",
            "crypt32.lib",
            "ncrypt.lib",
            "secur32.lib",
            "legacy_stdio_definitions.lib",
            "msvcrt.lib",
            "vcruntime.lib",
            "ucrt.lib",
        ]
        .into_iter()
        .map(str::to_string)
        .collect()
    } else if target.triple.ends_with("-apple-darwin") {
        ["-framework", "Security", "-framework", "CoreFoundation"]
            .into_iter()
            .map(str::to_string)
            .collect()
    } else {
        vec!["-lm".to_string()]
    }
}

fn validate_installed_runtime_bridge(
    install_root: &Path,
    payloads: &[InstalledPayloadManifest],
) -> Result<Vec<String>> {
    let stdlib = install_root.join("share").join("sengoo").join("stdlib");
    let files = [
        "runtime.c",
        "runtime_breadth.c",
        "runtime_collections.c",
        "runtime_json.c",
        "runtime_process.c",
        "runtime_string.c",
        "runtime_shared.h",
    ];
    for file in files {
        let path = stdlib.join(file);
        if !path.is_file() {
            return Err(miette::miette!(
                "installed native runtime bridge file is missing: {}",
                path.display()
            ));
        }
    }

    let mut verified_hashes = Vec::with_capacity(files.len());
    for file in files {
        let relative = format!("share/sengoo/stdlib/{file}");
        let matches = payloads
            .iter()
            .filter(|payload| payload.path == relative)
            .collect::<Vec<_>>();
        if matches.len() != 1 {
            return Err(miette::miette!(
                "installed toolchain manifest must contain exactly one payload checksum for {relative}"
            ));
        }
        let payload = matches[0];
        validate_relative_payload_path(&payload.path)?;
        if payload.sha256.len() != 64
            || !payload.sha256.bytes().all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(miette::miette!(
                "installed runtime payload SHA-256 is not 64 hexadecimal characters for {relative}: {}",
                payload.sha256
            ));
        }
        let path = install_root.join(&payload.path);
        let actual_sha256 = sha256_file(&path)?;
        if !actual_sha256.eq_ignore_ascii_case(&payload.sha256) {
            return Err(miette::miette!(
                "installed runtime payload SHA-256 mismatch for {}: expected={}, actual={}",
                path.display(),
                payload.sha256,
                actual_sha256
            ));
        }
        verified_hashes.push(payload.sha256.to_ascii_lowercase());
    }
    Ok(verified_hashes)
}

fn sha256_file(path: &Path) -> Result<String> {
    let mut input = fs::File::open(path)
        .into_diagnostic()
        .map_err(|error| miette::miette!("failed to open installed native runtime: {error}"))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .into_diagnostic()
            .map_err(|error| miette::miette!("failed to hash installed native runtime: {error}"))?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    let digest = digest.finalize();
    Ok(format!("{digest:x}"))
}

fn collect_source_runtime_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let entries = fs::read_dir(directory)
        .into_diagnostic()
        .map_err(|error| miette::miette!("failed to inspect source runtime inputs: {error}"))?;
    for entry in entries {
        let entry = entry
            .into_diagnostic()
            .map_err(|error| miette::miette!("failed to inspect source runtime inputs: {error}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .into_diagnostic()
            .map_err(|error| miette::miette!("failed to inspect source runtime inputs: {error}"))?;
        if file_type.is_dir() {
            collect_source_runtime_files(&path, files)?;
        } else if file_type.is_file() {
            files.push(path);
        }
    }
    Ok(())
}

fn source_runtime_input_fingerprint(workspace_root: &Path) -> Result<u64> {
    let runtime_root = workspace_root.join("runtime");
    let mut files = vec![
        workspace_root.join("Cargo.toml"),
        workspace_root.join("Cargo.lock"),
        runtime_root.join("Cargo.toml"),
    ];
    for optional in [
        workspace_root.join("rust-toolchain.toml"),
        workspace_root.join(".cargo").join("config.toml"),
        runtime_root.join("build.rs"),
    ] {
        if optional.is_file() {
            files.push(optional);
        }
    }
    collect_source_runtime_files(&runtime_root.join("src"), &mut files)?;
    files.sort_by_key(|path| {
        path.strip_prefix(workspace_root)
            .unwrap_or(path)
            .to_string_lossy()
            .replace('\\', "/")
    });

    let mut digest = Sha256::new();
    for path in files {
        let relative = path
            .strip_prefix(workspace_root)
            .unwrap_or(&path)
            .to_string_lossy()
            .replace('\\', "/");
        let bytes = fs::read(&path)
            .into_diagnostic()
            .map_err(|error| miette::miette!("failed to read source runtime input: {error}"))?;
        digest.update((relative.len() as u64).to_be_bytes());
        digest.update(relative.as_bytes());
        digest.update((bytes.len() as u64).to_be_bytes());
        digest.update(&bytes);
    }
    let digest = digest.finalize();
    Ok(u64::from_be_bytes(digest[..8].try_into().unwrap()))
}

pub(crate) fn resolve_installed_native_runtime(
    target: &NativeBuildTarget,
) -> Result<Option<InstalledNativeRuntime>> {
    let manifest_path = manifest_path_from_current_exe();
    if native_runtime_mode() == NativeRuntimeMode::SourceDevelopment {
        if !current_exe_is_source_local() {
            return Err(miette::miette!(
                "source runtime development mode requires an sgc executable inside its compiled Sengoo source workspace"
            ));
        }
        return Ok(None);
    }
    validate_native_runtime_mode_environment()?;
    let Some(manifest_path) = manifest_path else {
        if current_exe_is_source_local() {
            return Err(miette::miette!(
                "source runtime development mode is not selected; pass --runtime-mode source-development to authorize non-release Cargo runtime construction"
            ));
        }
        return Err(miette::miette!(
            "installed toolchain manifest is missing; Cargo fallback is disabled outside a Sengoo source checkout"
        ));
    };
    let bytes = fs::read(&manifest_path)
        .into_diagnostic()
        .map_err(|error| miette::miette!("failed to read installed toolchain manifest: {error}"))?;
    let manifest: InstalledToolchainManifest = serde_json::from_slice(&bytes)
        .into_diagnostic()
        .map_err(|error| miette::miette!("invalid installed toolchain manifest: {error}"))?;
    if manifest.schema_version != 2 {
        return Err(miette::miette!(
            "installed toolchain manifest schema {} does not provide native runtime metadata",
            manifest.schema_version
        ));
    }
    if manifest.target != target.triple {
        return Err(miette::miette!(
            "installed toolchain target mismatch: manifest={}, requested={}",
            manifest.target,
            target.triple
        ));
    }
    if manifest.build_manifest_id.len() != 64
        || !manifest
            .build_manifest_id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(miette::miette!(
            "installed build_manifest_id is not 64 hexadecimal characters"
        ));
    }
    let runtime = manifest.native_runtime.ok_or_else(|| {
        miette::miette!("installed toolchain manifest is missing native_runtime metadata")
    })?;
    if runtime.target != target.triple {
        return Err(miette::miette!(
            "installed native runtime target mismatch: manifest={}, requested={}",
            runtime.target,
            target.triple
        ));
    }
    if runtime.abi_version != NATIVE_RUNTIME_ABI_VERSION {
        return Err(miette::miette!(
            "installed native runtime ABI mismatch: manifest={}, supported={}",
            runtime.abi_version,
            NATIVE_RUNTIME_ABI_VERSION
        ));
    }
    let relative_library = validate_relative_payload_path(&runtime.library)?;
    let install_root = manifest_path
        .parent()
        .ok_or_else(|| miette::miette!("installed toolchain manifest has no parent directory"))?;
    let library = install_root.join(relative_library);
    if !library.is_file() {
        return Err(miette::miette!(
            "installed native runtime library is missing: {}",
            library.display()
        ));
    }
    if runtime.sha256.len() != 64 || !runtime.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(miette::miette!(
            "installed native runtime SHA-256 is not 64 hexadecimal characters: {}",
            runtime.sha256
        ));
    }
    let actual_sha256 = sha256_file(&library)?;
    if !actual_sha256.eq_ignore_ascii_case(&runtime.sha256) {
        return Err(miette::miette!(
            "installed native runtime SHA-256 mismatch for {}: expected={}, actual={}",
            library.display(),
            runtime.sha256,
            actual_sha256
        ));
    }
    let expected_link_args = expected_link_args(target);
    if runtime.link_args != expected_link_args {
        return Err(miette::miette!(
            "installed native runtime link arguments mismatch: manifest={:?}, supported={:?}",
            runtime.link_args,
            expected_link_args
        ));
    }
    if !runtime.dynamic_dependencies.is_empty() {
        return Err(miette::miette!(
            "installed native runtime declares unsupported dynamic dependencies: {:?}",
            runtime.dynamic_dependencies
        ));
    }
    let bridge_hashes = validate_installed_runtime_bridge(install_root, &manifest.payloads)?;
    let mut cache_identity = DefaultHasher::new();
    manifest.build_manifest_id.hash(&mut cache_identity);
    manifest.artifact_provenance.hash(&mut cache_identity);
    manifest.release_eligible.hash(&mut cache_identity);
    runtime.abi_version.hash(&mut cache_identity);
    runtime.target.hash(&mut cache_identity);
    runtime
        .sha256
        .to_ascii_lowercase()
        .hash(&mut cache_identity);
    runtime.link_args.hash(&mut cache_identity);
    runtime.dynamic_dependencies.hash(&mut cache_identity);
    bridge_hashes.hash(&mut cache_identity);
    let artifact_provenance = if manifest.artifact_provenance.is_empty() {
        "installed-unknown".to_string()
    } else {
        manifest.artifact_provenance
    };
    Ok(Some(InstalledNativeRuntime {
        library,
        cache_fingerprint: cache_identity.finish(),
        provenance: NativeRuntimeProvenance {
            runtime_mode: "installed".to_string(),
            artifact_provenance,
            release_eligible: manifest.release_eligible,
            senline_pin_evidence: false,
            build_manifest_id: Some(manifest.build_manifest_id),
        },
    }))
}

pub(crate) fn native_runtime_cache_context(
    target: &NativeBuildTarget,
) -> Result<(Option<u64>, NativeRuntimeProvenance)> {
    match resolve_installed_native_runtime(target)? {
        Some(runtime) => Ok((Some(runtime.cache_fingerprint), runtime.provenance)),
        None => {
            let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"))
                .ancestors()
                .nth(2)
                .ok_or_else(|| miette::miette!("Sengoo source workspace root is unavailable"))?;
            Ok((
                Some(source_runtime_input_fingerprint(workspace_root)?),
                NativeRuntimeProvenance::source_development(),
            ))
        }
    }
}

pub(crate) fn runtime_source_cache_identity(
    runtime_c: Option<&str>,
    provenance: &NativeRuntimeProvenance,
) -> Option<String> {
    runtime_c.map(|path| {
        if provenance.runtime_mode == "installed" {
            "installed:share/sengoo/stdlib/runtime.c".to_string()
        } else {
            path.to_string()
        }
    })
}

pub(crate) fn validate_installed_native_runtime_for_host() -> Result<()> {
    resolve_installed_native_runtime(&NativeBuildTarget::host()).map(|_| ())
}

pub(crate) fn combine_runtime_cache_fingerprints(
    source_fingerprint: Option<u64>,
    installed_fingerprint: Option<u64>,
) -> Option<u64> {
    if source_fingerprint.is_none() && installed_fingerprint.is_none() {
        return None;
    }
    let mut combined = DefaultHasher::new();
    source_fingerprint.hash(&mut combined);
    installed_fingerprint.hash(&mut combined);
    Some(combined.finish())
}

#[cfg(test)]
mod tests {
    use super::source_runtime_input_fingerprint;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn source_runtime_fingerprint_tracks_rust_sources_and_lockfile() {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("sgc_source_runtime_fingerprint_{stamp}"));
        let runtime_src = root.join("runtime").join("src");
        fs::create_dir_all(&runtime_src).unwrap();
        fs::write(root.join("Cargo.toml"), "[workspace]\n").unwrap();
        fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
        fs::write(
            root.join("runtime").join("Cargo.toml"),
            "[package]\nname = \"sengoo-runtime\"\n",
        )
        .unwrap();
        fs::write(runtime_src.join("lib.rs"), "pub fn value() -> u32 { 1 }\n").unwrap();

        let baseline = source_runtime_input_fingerprint(&root).unwrap();
        fs::write(runtime_src.join("lib.rs"), "pub fn value() -> u32 { 2 }\n").unwrap();
        let source_changed = source_runtime_input_fingerprint(&root).unwrap();
        assert_ne!(baseline, source_changed);

        fs::write(
            root.join("Cargo.lock"),
            "version = 4\n# dependency changed\n",
        )
        .unwrap();
        let lock_changed = source_runtime_input_fingerprint(&root).unwrap();
        assert_ne!(source_changed, lock_changed);

        let _ = fs::remove_dir_all(root);
    }
}
