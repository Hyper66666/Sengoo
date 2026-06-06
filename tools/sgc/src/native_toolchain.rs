use miette::{IntoDiagnostic, Result};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
#[cfg(not(windows))]
use std::sync::atomic::Ordering;

use crate::{
    file_fingerprint, object_file_extension, BuildCacheMetadata, CachedNativeRecoveryPlan,
    LinkerMode, RunCacheMetadata, RunEngine,
};
#[cfg(not(windows))]
use crate::{LINKER_AVAILABLE, LINKER_UNAVAILABLE, LLD_AVAILABILITY};

const RUNTIME_SPLIT_C_SOURCES: &[&str] = &[
    "runtime_breadth.c",
    "runtime_collections.c",
    "runtime_json.c",
    "runtime_process.c",
    "runtime_string.c",
];
const RUNTIME_SHARED_HEADER: &str = "runtime_shared.h";

pub(crate) fn runtime_source_bundle(runtime_c: &str) -> Result<Vec<PathBuf>> {
    let runtime_c_path = Path::new(runtime_c);
    let mut sources = Vec::new();
    sources.push(runtime_c_path.to_path_buf());
    let Some(runtime_dir) = runtime_c_path.parent() else {
        return Ok(sources);
    };
    for sibling in RUNTIME_SPLIT_C_SOURCES {
        let candidate = runtime_dir.join(sibling);
        if candidate.exists() {
            sources.push(candidate);
        }
    }
    Ok(sources)
}

fn runtime_bundle_fingerprint_inputs(runtime_c: &str) -> Result<Vec<PathBuf>> {
    let mut inputs = runtime_source_bundle(runtime_c)?;
    if let Some(runtime_dir) = Path::new(runtime_c).parent() {
        let header = runtime_dir.join(RUNTIME_SHARED_HEADER);
        if header.exists() {
            inputs.push(header);
        }
    }
    Ok(inputs)
}

pub(crate) fn runtime_bundle_fingerprint(runtime_c: &str) -> Result<u64> {
    let mut hasher = DefaultHasher::new();
    for input in runtime_bundle_fingerprint_inputs(runtime_c)? {
        let canonical = fs::canonicalize(&input).unwrap_or_else(|_| input.clone());
        canonical.to_string_lossy().hash(&mut hasher);
        file_fingerprint(&canonical)?.hash(&mut hasher);
    }
    Ok(hasher.finish())
}

pub(crate) fn optional_runtime_bundle_fingerprint(runtime_c: Option<&str>) -> Result<Option<u64>> {
    runtime_c.map(runtime_bundle_fingerprint).transpose()
}

fn runtime_object_cache_path(
    runtime_source_path: &Path,
    runtime_c_path: &Path,
    runtime_bundle_fingerprint: u64,
    opt_level: u8,
) -> Result<PathBuf> {
    let runtime_c_canonical =
        fs::canonicalize(runtime_c_path).unwrap_or_else(|_| runtime_c_path.to_path_buf());
    let canonical =
        fs::canonicalize(runtime_source_path).unwrap_or_else(|_| runtime_source_path.to_path_buf());
    let runtime_source_fingerprint = file_fingerprint(&canonical)?;

    let mut hasher = DefaultHasher::new();
    runtime_c_canonical.to_string_lossy().hash(&mut hasher);
    canonical.to_string_lossy().hash(&mut hasher);
    runtime_bundle_fingerprint.hash(&mut hasher);
    runtime_source_fingerprint.hash(&mut hasher);
    opt_level.hash(&mut hasher);
    if cfg!(windows) {
        "x86_64-pc-windows-msvc".hash(&mut hasher);
    }
    let key = hasher.finish();

    let ext = if cfg!(windows) { "obj" } else { "o" };
    let cache_dir = std::env::temp_dir()
        .join("sengoo")
        .join("runtime-obj-cache");
    fs::create_dir_all(&cache_dir).into_diagnostic()?;
    Ok(cache_dir.join(format!("runtime-{}-O{}.{}", key, opt_level, ext)))
}

fn compile_runtime_source_to_object(
    clang_exe: &str,
    runtime_source_path: &Path,
    object_path: &Path,
    opt_level: u8,
) -> Result<()> {
    let mut command = Command::new(clang_exe);
    command
        .arg("-Wno-override-module")
        .arg(format!("-O{}", opt_level));

    if let Some(runtime_dir) = runtime_source_path.parent() {
        command.arg("-I").arg(runtime_dir);
        if !runtime_dir.join(RUNTIME_SHARED_HEADER).exists() {
            let bundled_stdlib_dir = workspace_root().join("tools").join("stdlib");
            if bundled_stdlib_dir.join(RUNTIME_SHARED_HEADER).exists() {
                command.arg("-I").arg(bundled_stdlib_dir);
            }
        }
    }

    #[cfg(windows)]
    {
        command
            .arg("--target=x86_64-pc-windows-msvc")
            .arg("-fms-runtime-lib=dll");
        for include in windows_compile_include_paths()? {
            command.arg("-isystem").arg(include);
        }
    }

    let status = command
        .arg("-c")
        .arg(runtime_source_path)
        .arg("-o")
        .arg(object_path)
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke clang for runtime object: {}", e))?;

    if !status.success() {
        return Err(miette::miette!(
            "compile failed while preparing runtime object cache"
        ));
    }

    Ok(())
}

pub(crate) fn ensure_runtime_objects(
    clang_exe: &str,
    runtime_c: &str,
    opt_level: u8,
) -> Result<Vec<PathBuf>> {
    let runtime_c_path = Path::new(runtime_c);
    let sources = runtime_source_bundle(runtime_c)?;
    let bundle_fingerprint = runtime_bundle_fingerprint(runtime_c)?;
    let mut object_paths = Vec::with_capacity(sources.len());
    for source_path in sources {
        let object_path =
            runtime_object_cache_path(&source_path, runtime_c_path, bundle_fingerprint, opt_level)?;
        if !object_path.exists() {
            compile_runtime_source_to_object(clang_exe, &source_path, &object_path, opt_level)?;
        }
        object_paths.push(object_path);
    }
    Ok(object_paths)
}

fn compiled_workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .unwrap_or_else(|| Path::new(env!("CARGO_MANIFEST_DIR")))
        .to_path_buf()
}

fn looks_like_workspace_root(candidate: &Path) -> bool {
    candidate.join("Cargo.toml").is_file()
        && candidate.join("runtime").join("Cargo.toml").is_file()
        && candidate
            .join("tools")
            .join("sgc")
            .join("Cargo.toml")
            .is_file()
}

fn discover_workspace_root_from(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_file() {
        start.parent()?
    } else {
        start
    };

    loop {
        if looks_like_workspace_root(current) {
            return Some(current.to_path_buf());
        }
        let Some(parent) = current.parent() else {
            break;
        };
        current = parent;
    }

    None
}

fn resolve_workspace_root(
    exe_path: Option<&Path>,
    cwd: Option<&Path>,
    compiled_root: &Path,
) -> PathBuf {
    exe_path
        .and_then(discover_workspace_root_from)
        .or_else(|| cwd.and_then(discover_workspace_root_from))
        .unwrap_or_else(|| compiled_root.to_path_buf())
}

fn workspace_root() -> PathBuf {
    let current_exe = std::env::current_exe().ok();
    let current_dir = std::env::current_dir().ok();
    let compiled_root = compiled_workspace_root();
    resolve_workspace_root(
        current_exe.as_deref(),
        current_dir.as_deref(),
        &compiled_root,
    )
}

fn async_runtime_profile(opt_level: u8) -> &'static str {
    if opt_level >= 2 {
        "release"
    } else {
        "debug"
    }
}

fn is_async_runtime_staticlib(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };

    if cfg!(windows) {
        name.starts_with("sengoo_runtime") && name.ends_with(".lib")
    } else {
        name.starts_with("libsengoo_runtime") && name.ends_with(".a")
    }
}

fn find_async_runtime_staticlib_in_dir(dir: &Path) -> Option<PathBuf> {
    let entries = fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() && is_async_runtime_staticlib(&path) {
            return Some(path);
        }
    }
    None
}

fn find_async_runtime_staticlib(profile: &str) -> Option<PathBuf> {
    let profile_dir = workspace_root().join("target").join(profile);
    find_async_runtime_staticlib_in_dir(&profile_dir)
        .or_else(|| find_async_runtime_staticlib_in_dir(&profile_dir.join("deps")))
}

pub(crate) fn ensure_async_runtime_staticlib(opt_level: u8) -> Result<PathBuf> {
    let profile = async_runtime_profile(opt_level);
    let workspace_root = workspace_root();
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("-p")
        .arg("sengoo-runtime")
        .arg("--lib")
        .arg("--features")
        .arg("native-bridge");
    if profile == "release" {
        command.arg("--release");
    }
    let status = command
        .current_dir(&workspace_root)
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to build async runtime static library: {}", e))?;
    if !status.success() {
        return Err(miette::miette!(
            "compile failed while building async runtime static library"
        ));
    }

    find_async_runtime_staticlib(profile).ok_or_else(|| {
        miette::miette!(
            "async runtime static library missing after build in {} profile",
            profile
        )
    })
}

pub(crate) fn append_native_runtime_inputs(
    clang_exe: &str,
    object_paths: &mut Vec<PathBuf>,
    runtime_c: Option<&str>,
    opt_level: u8,
) -> Result<()> {
    if let Some(runtime_c) = runtime_c {
        object_paths.extend(ensure_runtime_objects(clang_exe, runtime_c, opt_level)?);
    }
    object_paths.push(ensure_async_runtime_staticlib(opt_level)?);
    Ok(())
}

#[cfg(windows)]
fn newest_child_dir(dir: &Path) -> Option<PathBuf> {
    let mut candidates = fs::read_dir(dir)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.pop()
}

#[cfg(windows)]
fn find_windows_sdk_lib_root() -> Option<PathBuf> {
    newest_child_dir(Path::new(r"C:\Program Files (x86)\Windows Kits\10\Lib"))
}

#[cfg(windows)]
fn find_windows_sdk_include_root() -> Option<PathBuf> {
    newest_child_dir(Path::new(r"C:\Program Files (x86)\Windows Kits\10\Include"))
}

#[cfg(windows)]
fn find_msvc_tool_root() -> Option<PathBuf> {
    for base in [
        Path::new(r"C:\Program Files\Microsoft Visual Studio"),
        Path::new(r"C:\Program Files (x86)\Microsoft Visual Studio"),
    ] {
        let Some(year_dir) = newest_child_dir(base) else {
            continue;
        };
        let Some(edition_dir) = newest_child_dir(&year_dir) else {
            continue;
        };
        let tool_root = edition_dir.join("VC").join("Tools").join("MSVC");
        if !tool_root.exists() {
            continue;
        }
        if let Some(version_dir) = newest_child_dir(&tool_root) {
            return Some(version_dir);
        }
    }
    None
}

#[cfg(windows)]
fn find_msvc_link_exe() -> Option<PathBuf> {
    let tool_root = find_msvc_tool_root()?;
    let candidate = tool_root
        .join("bin")
        .join("Hostx64")
        .join("x64")
        .join("link.exe");
    candidate.exists().then_some(candidate)
}

#[cfg(windows)]
fn windows_link_lib_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(tool_root) = find_msvc_tool_root() {
        let vc_lib = tool_root.join("lib").join("x64");
        if vc_lib.exists() {
            paths.push(vc_lib);
        }
    }
    if let Some(sdk_root) = find_windows_sdk_lib_root() {
        let ucrt = sdk_root.join("ucrt").join("x64");
        if ucrt.exists() {
            paths.push(ucrt);
        }
        let um = sdk_root.join("um").join("x64");
        if um.exists() {
            paths.push(um);
        }
    }
    paths
}

#[cfg(windows)]
fn windows_compile_include_paths() -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let tool_root = find_msvc_tool_root()
        .ok_or_else(|| miette::miette!("failed to locate MSVC toolchain headers"))?;
    let msvc_include = tool_root.join("include");
    if msvc_include.exists() {
        paths.push(msvc_include);
    }

    let sdk_root = find_windows_sdk_include_root()
        .ok_or_else(|| miette::miette!("failed to locate Windows SDK headers"))?;
    for leaf in ["ucrt", "um", "shared"] {
        let include_dir = sdk_root.join(leaf);
        if include_dir.exists() {
            paths.push(include_dir);
        }
    }

    Ok(paths)
}

#[cfg(windows)]
fn run_windows_link_command(
    object_paths: &[PathBuf],
    executable_path: &Path,
) -> Result<std::process::ExitStatus> {
    let link_exe = find_msvc_link_exe().ok_or_else(|| {
        miette::miette!("failed to locate MSVC link.exe for native async linking")
    })?;
    let mut link_cmd = Command::new(link_exe);
    link_cmd.arg("/NOLOGO");
    let links_async_runtime = object_paths
        .iter()
        .any(|path| is_async_runtime_staticlib(path));
    if links_async_runtime {
        // Keep compiler-generated async dispatch symbols that are only referenced
        // from the Rust async runtime static library.
        link_cmd.arg("/OPT:NOREF");
    }
    for lib_path in windows_link_lib_paths() {
        link_cmd.arg(format!("/LIBPATH:{}", lib_path.display()));
    }
    for object in object_paths {
        link_cmd.arg(object);
    }
    for lib in [
        "kernel32.lib",
        "ntdll.lib",
        "userenv.lib",
        "ws2_32.lib",
        "dbghelp.lib",
        "legacy_stdio_definitions.lib",
        "msvcrt.lib",
        "vcruntime.lib",
        "ucrt.lib",
    ] {
        link_cmd.arg(lib);
    }
    link_cmd.arg("/ENTRY:mainCRTStartup");
    link_cmd.arg("/SUBSYSTEM:CONSOLE");
    link_cmd.arg(format!("/OUT:{}", executable_path.display()));
    link_cmd
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke MSVC linker: {}", e))
}

pub(crate) fn compile_ir_to_object(
    clang_exe: &str,
    llvm_ir_path: &Path,
    object_path: &Path,
    opt_level: u8,
) -> Result<()> {
    let mut command = Command::new(clang_exe);
    command
        .arg("-Wno-override-module")
        .arg(format!("-O{}", opt_level));

    #[cfg(windows)]
    {
        command.arg("--target=x86_64-pc-windows-msvc");
    }

    let status = command
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
#[cfg(not(windows))]

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

#[cfg(windows)]
pub(crate) fn link_native_binary_from_objects(
    clang_exe: &str,
    object_paths: &[PathBuf],
    executable_path: &Path,
) -> Result<()> {
    let _ = clang_exe;
    let status = run_windows_link_command(object_paths, executable_path)?;
    if !status.success() {
        return Err(miette::miette!("compile failed"));
    }
    Ok(())
}

#[cfg(not(windows))]
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
    append_native_runtime_inputs(clang_exe, &mut object_paths, runtime_c, opt_level)?;
    link_native_binary_from_objects(clang_exe, &object_paths, executable_path)?;
    Ok(())
}

pub(crate) fn run_native_binary_with_args(executable_path: &Path, args: &[String]) -> Result<()> {
    let run_output = Command::new(executable_path)
        .args(args)
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

pub(crate) fn run_with_lli_args(
    lli_exe: &str,
    llvm_ir_path: &Path,
    args: &[String],
    extra_objects: &[PathBuf],
) -> Result<()> {
    let mut command = Command::new(lli_exe);
    for object in extra_objects {
        command.arg(format!("--extra-object={}", object.display()));
    }
    let output = command
        .arg(llvm_ir_path)
        .args(args)
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
    append_native_runtime_inputs(clang_exe, &mut object_paths, runtime_c, opt_level)?;
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
            "sengoo-sgc-{label}-{}-{unique}",
            std::process::id()
        ))
    }

    fn write_marker(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, "").unwrap();
    }

    #[test]
    fn resolve_workspace_root_prefers_runtime_binary_ancestor() {
        let runtime_root = temp_test_dir("runtime-root");
        let stale_compiled_root = temp_test_dir("stale-compiled");
        let exe_name = if cfg!(windows) { "sgc.exe" } else { "sgc" };
        let exe_path = runtime_root.join("target").join("debug").join(exe_name);

        write_marker(&runtime_root.join("Cargo.toml"));
        write_marker(&runtime_root.join("runtime").join("Cargo.toml"));
        write_marker(&runtime_root.join("tools").join("sgc").join("Cargo.toml"));
        write_marker(&exe_path);

        let resolved = resolve_workspace_root(Some(&exe_path), None, &stale_compiled_root);
        assert_eq!(resolved, runtime_root);

        let _ = fs::remove_dir_all(&resolved);
    }

    #[test]
    fn resolve_workspace_root_falls_back_to_current_dir_ancestor() {
        let runtime_root = temp_test_dir("cwd-root");
        let stale_compiled_root = temp_test_dir("stale-cwd");
        let cwd = runtime_root.join("bench").join("tests");

        write_marker(&runtime_root.join("Cargo.toml"));
        write_marker(&runtime_root.join("runtime").join("Cargo.toml"));
        write_marker(&runtime_root.join("tools").join("sgc").join("Cargo.toml"));
        fs::create_dir_all(&cwd).unwrap();

        let resolved = resolve_workspace_root(None, Some(&cwd), &stale_compiled_root);
        assert_eq!(resolved, runtime_root);

        let _ = fs::remove_dir_all(&resolved);
    }

    #[test]
    fn runtime_object_cache_path_changes_when_equal_length_source_bytes_change() {
        let root = temp_test_dir("runtime-object-content");
        let runtime_c = root.join("runtime.c");
        fs::create_dir_all(&root).unwrap();
        fs::write(&runtime_c, "aaaa").unwrap();
        let modified = fs::metadata(&runtime_c).unwrap().modified().unwrap();
        let before_fingerprint = runtime_bundle_fingerprint(&runtime_c.to_string_lossy()).unwrap();
        let before =
            runtime_object_cache_path(&runtime_c, &runtime_c, before_fingerprint, 1).unwrap();

        fs::write(&runtime_c, "bbbb").unwrap();
        let file = fs::OpenOptions::new().write(true).open(&runtime_c).unwrap();
        file.set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let after_fingerprint = runtime_bundle_fingerprint(&runtime_c.to_string_lossy()).unwrap();
        let after =
            runtime_object_cache_path(&runtime_c, &runtime_c, after_fingerprint, 1).unwrap();

        assert_ne!(before, after);
        let _ = fs::remove_dir_all(&root);
    }
}
