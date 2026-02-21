use miette::{IntoDiagnostic, Result};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::UNIX_EPOCH;

use crate::{
    object_file_extension, BuildCacheMetadata, CachedNativeRecoveryPlan, LinkerMode,
    RunCacheMetadata, RunEngine, LINKER_AVAILABLE, LINKER_UNAVAILABLE, LLD_AVAILABILITY,
};

fn runtime_object_cache_path(runtime_c_path: &Path, opt_level: u8) -> Result<PathBuf> {
    let canonical =
        fs::canonicalize(runtime_c_path).unwrap_or_else(|_| runtime_c_path.to_path_buf());
    let meta = fs::metadata(&canonical).into_diagnostic().map_err(|e| {
        miette::miette!(
            "failed to stat runtime source {}: {}",
            canonical.display(),
            e
        )
    })?;
    let modified_secs = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);

    let mut hasher = DefaultHasher::new();
    canonical.to_string_lossy().hash(&mut hasher);
    meta.len().hash(&mut hasher);
    modified_secs.hash(&mut hasher);
    opt_level.hash(&mut hasher);
    let key = hasher.finish();

    let ext = if cfg!(windows) { "obj" } else { "o" };
    let cache_dir = std::env::temp_dir()
        .join("sengoo")
        .join("runtime-obj-cache");
    fs::create_dir_all(&cache_dir).into_diagnostic()?;
    Ok(cache_dir.join(format!("runtime-{}-O{}.{}", key, opt_level, ext)))
}

pub(crate) fn ensure_runtime_object(
    clang_exe: &str,
    runtime_c: &str,
    opt_level: u8,
) -> Result<PathBuf> {
    let runtime_c_path = Path::new(runtime_c);
    let object_path = runtime_object_cache_path(runtime_c_path, opt_level)?;
    if object_path.exists() {
        return Ok(object_path);
    }

    let status = Command::new(clang_exe)
        .arg("-Wno-override-module")
        .arg(format!("-O{}", opt_level))
        .arg("-c")
        .arg(runtime_c_path)
        .arg("-o")
        .arg(&object_path)
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke clang for runtime object: {}", e))?;

    if !status.success() {
        return Err(miette::miette!(
            "compile failed while preparing runtime object cache"
        ));
    }

    Ok(object_path)
}

pub(crate) fn compile_ir_to_object(
    clang_exe: &str,
    llvm_ir_path: &Path,
    object_path: &Path,
    opt_level: u8,
) -> Result<()> {
    let status = Command::new(clang_exe)
        .arg("-Wno-override-module")
        .arg(format!("-O{}", opt_level))
        .arg("-c")
        .arg(llvm_ir_path)
        .arg("-o")
        .arg(object_path)
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke clang for object compilation: {}", e))?;

    if !status.success() {
        return Err(miette::miette!("compile failed"));
    }
    Ok(())
}

pub(crate) fn linker_mode_from_env() -> LinkerMode {
    parse_linker_mode(std::env::var("SENGOO_LINKER").ok().as_deref())
}

pub(crate) fn parse_linker_mode(value: Option<&str>) -> LinkerMode {
    let Some(value) = value else {
        return LinkerMode::Auto;
    };
    match value.trim().to_ascii_lowercase().as_str() {
        "lld" => LinkerMode::Lld,
        "system" => LinkerMode::System,
        _ => LinkerMode::Auto,
    }
}

fn run_link_command(
    clang_exe: &str,
    object_paths: &[PathBuf],
    executable_path: &Path,
    use_lld: bool,
) -> Result<std::process::ExitStatus> {
    let mut clang_cmd = Command::new(clang_exe);
    clang_cmd.arg("-Wno-override-module");
    if use_lld {
        clang_cmd.arg("-fuse-ld=lld");
    }
    for object in object_paths {
        clang_cmd.arg(object);
    }
    clang_cmd.arg("-o").arg(executable_path);
    clang_cmd
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke clang linker: {}", e))
}

pub(crate) fn link_native_binary_from_objects(
    clang_exe: &str,
    object_paths: &[PathBuf],
    executable_path: &Path,
) -> Result<()> {
    let mode = linker_mode_from_env();
    let lld_state = LLD_AVAILABILITY.load(Ordering::Relaxed);
    let try_lld_first = match mode {
        LinkerMode::Lld => true,
        LinkerMode::System => false,
        LinkerMode::Auto => lld_state != LINKER_UNAVAILABLE,
    };

    if try_lld_first {
        let lld_status = run_link_command(clang_exe, object_paths, executable_path, true)?;
        if lld_status.success() {
            if matches!(mode, LinkerMode::Auto) {
                LLD_AVAILABILITY.store(LINKER_AVAILABLE, Ordering::Relaxed);
            }
            return Ok(());
        }
        if matches!(mode, LinkerMode::Lld) {
            return Err(miette::miette!("compile failed (lld linker mode)"));
        }
        LLD_AVAILABILITY.store(LINKER_UNAVAILABLE, Ordering::Relaxed);
        println!("link fallback: lld unavailable, retrying with system linker");
    }

    let status = run_link_command(clang_exe, object_paths, executable_path, false)?;
    if !status.success() {
        return Err(miette::miette!("compile failed"));
    }
    Ok(())
}

pub(crate) fn compile_native_binary(
    clang_exe: &str,
    llvm_ir_path: &Path,
    executable_path: &Path,
    runtime_c: Option<&str>,
    opt_level: u8,
) -> Result<()> {
    let object_path = executable_path.with_extension(object_file_extension());
    compile_ir_to_object(clang_exe, llvm_ir_path, &object_path, opt_level)?;
    let mut object_paths = vec![object_path];
    if let Some(runtime_c) = runtime_c {
        let runtime_obj = ensure_runtime_object(clang_exe, runtime_c, opt_level)?;
        object_paths.push(runtime_obj);
    }
    link_native_binary_from_objects(clang_exe, &object_paths, executable_path)?;
    Ok(())
}

pub(crate) fn run_native_binary(executable_path: &Path) -> Result<()> {
    let run_output = Command::new(executable_path)
        .output()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to execute native binary: {}", e))?;

    let stdout = String::from_utf8_lossy(&run_output.stdout);
    if !stdout.is_empty() {
        print!("{}", stdout);
    }

    let stderr = String::from_utf8_lossy(&run_output.stderr);
    if !stderr.is_empty() {
        eprint!("{}", stderr);
    }

    if let Some(code) = run_output.status.code() {
        println!("exit code: {}", code);
    }

    Ok(())
}

pub(crate) fn run_with_lli(lli_exe: &str, llvm_ir_path: &Path) -> Result<()> {
    let output = Command::new(lli_exe)
        .arg(llvm_ir_path)
        .output()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke lli: {}", e))?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    if !stdout.is_empty() {
        print!("{}", stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !stderr.is_empty() {
        eprint!("{}", stderr);
    }

    if !output.status.success() {
        return Err(miette::miette!("compile failed"));
    }

    Ok(())
}

pub(crate) fn artifact_exists(metadata: &RunCacheMetadata) -> bool {
    match metadata.resolved_engine {
        RunEngine::Native => metadata
            .executable_path
            .as_ref()
            .is_some_and(|p| Path::new(p).exists()),
        RunEngine::Lli => Path::new(&metadata.llvm_ir_path).exists(),
        RunEngine::Auto => false,
    }
}

pub(crate) fn build_artifact_exists(metadata: &BuildCacheMetadata) -> bool {
    if metadata.emit_llvm {
        return Path::new(&metadata.output_path).exists();
    }

    Path::new(&metadata.llvm_ir_path).exists() && Path::new(&metadata.output_path).exists()
}

pub(crate) fn derive_cached_native_recovery_plan(
    llvm_ir_exists: bool,
    object_exists: bool,
) -> Option<CachedNativeRecoveryPlan> {
    if object_exists {
        Some(CachedNativeRecoveryPlan::RelinkFromObject)
    } else if llvm_ir_exists {
        Some(CachedNativeRecoveryPlan::RebuildObjectFromCachedIr)
    } else {
        None
    }
}

pub(crate) fn recover_native_output_from_cached_artifacts(
    clang_exe: &str,
    llvm_ir_path: &Path,
    object_path: &Path,
    output_path: &Path,
    runtime_c: Option<&str>,
    opt_level: u8,
) -> Result<CachedNativeRecoveryPlan> {
    let recovery_plan =
        derive_cached_native_recovery_plan(llvm_ir_path.exists(), object_path.exists())
            .ok_or_else(|| miette::miette!("cached object and LLVM IR are both missing"))?;

    if matches!(
        recovery_plan,
        CachedNativeRecoveryPlan::RebuildObjectFromCachedIr
    ) {
        compile_ir_to_object(clang_exe, llvm_ir_path, object_path, opt_level)?;
    }

    let mut object_paths = vec![object_path.to_path_buf()];
    if let Some(runtime_c) = runtime_c {
        object_paths.push(ensure_runtime_object(clang_exe, runtime_c, opt_level)?);
    }
    link_native_binary_from_objects(clang_exe, &object_paths, output_path)?;

    Ok(recovery_plan)
}

pub(crate) fn default_build_output_path_for_case(case: &Path) -> PathBuf {
    let stem = case.file_stem().unwrap_or_default().to_string_lossy();
    let source_dir = case.parent().unwrap_or(Path::new("."));
    let build_dir = source_dir.join("build");
    let ext = if cfg!(windows) { ".exe" } else { "" };
    build_dir.join(format!("{}{}", stem, ext))
}
