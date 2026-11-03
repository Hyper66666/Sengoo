use miette::{IntoDiagnostic, Result};
use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use crate::cross_compile::{linux_sysroot_from_env, windows_cross_sdk_include_paths};
use crate::installed_runtime::resolve_installed_native_runtime;
use crate::module_graph::collect_module_sources_with_edges;
use crate::native_link::{
    append_native_library_link_args, format_native_link_failure_message,
    native_library_search_paths_from_env, sgplatform_graphics_skip_enabled, SDL2_INCLUDE_DIR_ENV,
    SDL2_LIB_DIR_ENV,
};
use crate::{
    file_fingerprint, BuildCacheMetadata, CachedNativeRecoveryPlan, LinkerMode, NativeBuildTarget,
    RunCacheMetadata, RunEngine,
};

pub(crate) const MINIMUM_CLANG_MAJOR: u32 = 15;
pub(crate) const SENGOO_RUNTIME_ABI_VERSION: u32 = 1;
static RUNTIME_OBJECT_TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

fn effective_target(target: Option<&NativeBuildTarget>) -> NativeBuildTarget {
    target.cloned().unwrap_or_else(NativeBuildTarget::host)
}

pub(crate) fn parse_clang_major_version(version_text: &str) -> Option<u32> {
    let lower = version_text.to_ascii_lowercase();
    let tail = lower
        .find("clang version")
        .map(|index| &version_text[index + "clang version".len()..])
        .unwrap_or(version_text);
    tail.split(|ch: char| !ch.is_ascii_digit())
        .find(|part| !part.is_empty())
        .and_then(|part| part.parse().ok())
}

pub(crate) fn detected_clang_major_version(clang_exe: &str) -> Result<Option<u32>> {
    let output = Command::new(clang_exe)
        .arg("--version")
        .output()
        .into_diagnostic()
        .map_err(|error| {
            miette::miette!("failed to invoke clang for version detection: {error}")
        })?;
    let mut version_text = String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.stderr.is_empty() {
        version_text.push('\n');
        version_text.push_str(&String::from_utf8_lossy(&output.stderr));
    }
    Ok(parse_clang_major_version(&version_text))
}

fn validate_clang_major_version(major: Option<u32>, clang_exe: &str) -> Result<()> {
    let Some(major) = major else {
        return Err(miette::miette!(
            "unable to determine clang/LLVM version from `{clang_exe} --version`; Sengoo native builds require clang/LLVM {MINIMUM_CLANG_MAJOR}+ with opaque pointer support"
        ));
    };
    if major < MINIMUM_CLANG_MAJOR {
        return Err(miette::miette!(
            "unsupported clang/LLVM {major}; Sengoo native builds require clang/LLVM {MINIMUM_CLANG_MAJOR}+ with opaque pointer support. Install a newer LLVM/Clang or use `sgc build --emit-llvm` to inspect IR without native codegen."
        ));
    }
    Ok(())
}

pub(crate) fn ensure_supported_clang_toolchain(clang_exe: &str) -> Result<()> {
    validate_clang_major_version(detected_clang_major_version(clang_exe)?, clang_exe)
}

/// The `-isystem` include paths added for MSVC targets are only meaningful when
/// clang is preprocessing C. Passing them on an LLVM-IR or link-only invocation
/// makes clang warn about every one of them — an artifact of how the driver is
/// invoked, not a signal about the user's program. Silence them unless the user
/// asked to see the raw toolchain output.
fn suppress_pass_through_driver_warnings(command: &mut Command) {
    if !crate::verbose_output_enabled() {
        command.arg("-Wno-unused-command-line-argument");
    }
}

fn apply_clang_target_args(command: &mut Command, target: &NativeBuildTarget) -> Result<()> {
    command.arg(format!("--target={}", target.triple));

    if target.is_windows_msvc() {
        command.arg("-fms-runtime-lib=dll");
        if target.is_cross() {
            for include in windows_cross_sdk_include_paths()? {
                command.arg("-isystem").arg(include);
            }
        } else {
            #[cfg(windows)]
            {
                for include in windows_compile_include_paths()? {
                    command.arg("-isystem").arg(include);
                }
            }
        }
    }

    if target.is_linux_gnu() && target.is_cross() {
        let sysroot = linux_sysroot_from_env()?;
        command.arg(format!("--sysroot={sysroot}"));
    }
    Ok(())
}
#[cfg(not(windows))]
use crate::{LINKER_AVAILABLE, LINKER_UNAVAILABLE, LLD_AVAILABILITY};

const RUNTIME_SPLIT_C_SOURCES: &[&str] = &[
    "runtime_breadth.c",
    "runtime_collections.c",
    "runtime_json.c",
    "runtime_process.c",
    "runtime_stream.c",
    "runtime_string.c",
];
const RUNTIME_SHARED_HEADER: &str = "runtime_shared.h";

fn runtime_shared_header_path(runtime_c_path: &Path) -> Option<PathBuf> {
    let local = runtime_c_path.parent()?.join(RUNTIME_SHARED_HEADER);
    if local.is_file() {
        return Some(local);
    }
    canonical_stdlib_runtime_dir()
        .map(|directory| directory.join(RUNTIME_SHARED_HEADER))
        .filter(|header| header.is_file())
}

fn parse_runtime_abi_version(header: &str) -> Result<u32> {
    for line in header.lines() {
        let tokens = line.split_whitespace().collect::<Vec<_>>();
        if tokens.len() >= 3 && tokens[0] == "#define" && tokens[1] == "SENGOO_RUNTIME_ABI_VERSION"
        {
            return tokens[2].parse::<u32>().map_err(|error| {
                miette::miette!(
                    "invalid SENGOO_RUNTIME_ABI_VERSION `{}`: {error}",
                    tokens[2]
                )
            });
        }
    }
    Err(miette::miette!(
        "runtime ABI declaration missing SENGOO_RUNTIME_ABI_VERSION"
    ))
}

fn validate_runtime_abi_compatibility(runtime_c_path: &Path) -> Result<()> {
    let header_path = runtime_shared_header_path(runtime_c_path).ok_or_else(|| {
        miette::miette!(
            "runtime ABI declaration missing: no {RUNTIME_SHARED_HEADER} beside {}",
            runtime_c_path.display()
        )
    })?;
    let header = fs::read_to_string(&header_path)
        .into_diagnostic()
        .map_err(|error| {
            miette::miette!(
                "failed to read runtime ABI declaration {}: {error}",
                header_path.display()
            )
        })?;
    let available = parse_runtime_abi_version(&header)?;
    if available != SENGOO_RUNTIME_ABI_VERSION {
        return Err(miette::miette!(
            "runtime ABI mismatch: toolchain requires {SENGOO_RUNTIME_ABI_VERSION}, runtime provides {available} ({})",
            header_path.display()
        ));
    }
    Ok(())
}

fn push_existing_split_sources(sources: &mut Vec<PathBuf>, runtime_dir: &Path) {
    for sibling in RUNTIME_SPLIT_C_SOURCES {
        let candidate = runtime_dir.join(sibling);
        if candidate.exists() && !sources.iter().any(|existing| existing == &candidate) {
            sources.push(candidate);
        }
    }
}

fn canonical_stdlib_runtime_dir() -> Option<PathBuf> {
    let dir = workspace_root().join("tools").join("stdlib");
    if dir.join("runtime.c").is_file() {
        Some(dir)
    } else {
        None
    }
}

pub(crate) fn runtime_source_bundle(runtime_c: &str) -> Result<Vec<PathBuf>> {
    let runtime_c_path = Path::new(runtime_c);
    let mut sources = vec![runtime_c_path.to_path_buf()];
    let mut local_splits = Vec::new();
    if let Some(runtime_dir) = runtime_c_path.parent() {
        push_existing_split_sources(&mut local_splits, runtime_dir);
    }
    sources.extend(local_splits.iter().cloned());

    // Temp copies of runtime.c (async/native tests) live outside tools/stdlib and ship
    // without split siblings; link the canonical split objects in that case only.
    if local_splits.is_empty() {
        if let Some(canonical_dir) = canonical_stdlib_runtime_dir() {
            push_existing_split_sources(&mut sources, &canonical_dir);
        }
    }
    Ok(sources)
}

fn runtime_bundle_fingerprint_inputs(runtime_c: &str) -> Result<Vec<PathBuf>> {
    let mut inputs = runtime_source_bundle(runtime_c)?;
    if let Some(header) = runtime_shared_header_path(Path::new(runtime_c)) {
        if !inputs.iter().any(|input| input == &header) {
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
    target: &NativeBuildTarget,
    defines: &[&str],
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
    target.triple.hash(&mut hasher);
    defines.hash(&mut hasher);
    let key = hasher.finish();

    let ext = target.object_extension();
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
    target: &NativeBuildTarget,
    defines: &[&str],
) -> Result<()> {
    let mut command = Command::new(clang_exe);
    command
        .arg("-Wno-override-module")
        .arg(format!("-O{}", opt_level));
    for define in defines {
        command.arg(format!("-D{define}"));
    }

    if let Some(runtime_dir) = runtime_source_path.parent() {
        command.arg("-I").arg(runtime_dir);
        if !runtime_dir.join(RUNTIME_SHARED_HEADER).exists() {
            let bundled_stdlib_dir = workspace_root().join("tools").join("stdlib");
            if bundled_stdlib_dir.join(RUNTIME_SHARED_HEADER).exists() {
                command.arg("-I").arg(bundled_stdlib_dir);
            }
        }
    }

    apply_clang_target_args(&mut command, target)?;

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

fn runtime_object_temp_path(object_path: &Path) -> PathBuf {
    let sequence = RUNTIME_OBJECT_TEMP_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = object_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("runtime-object");
    object_path.with_file_name(format!(
        ".{file_name}.tmp-{}-{sequence}",
        std::process::id()
    ))
}

fn publish_runtime_object(temp_path: &Path, object_path: &Path) -> Result<()> {
    match fs::rename(temp_path, object_path) {
        Ok(()) => Ok(()),
        Err(_) if object_path.exists() => {
            let _ = fs::remove_file(temp_path);
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(temp_path);
            Err(error).into_diagnostic()
        }
    }
}

pub(crate) fn ensure_runtime_objects(
    clang_exe: &str,
    runtime_c: &str,
    opt_level: u8,
    target: Option<&NativeBuildTarget>,
) -> Result<Vec<PathBuf>> {
    ensure_runtime_objects_with_defines(clang_exe, runtime_c, opt_level, target, &[])
}

/// Marks the C runtime bundle as linked next to the native Rust runtime
/// staticlib, compiling out fallback stubs that would otherwise shadow the
/// real implementations during symbol resolution.
pub(crate) const NATIVE_NET_RUNTIME_DEFINE: &str = "SENGOO_NATIVE_NET_RUNTIME";
pub(crate) const NATIVE_ASYNC_RUNTIME_DEFINE: &str = "SENGOO_NATIVE_ASYNC_RUNTIME";

pub(crate) fn ensure_runtime_objects_with_defines(
    clang_exe: &str,
    runtime_c: &str,
    opt_level: u8,
    target: Option<&NativeBuildTarget>,
    defines: &[&str],
) -> Result<Vec<PathBuf>> {
    let target = effective_target(target);
    let runtime_c_path = Path::new(runtime_c);
    validate_runtime_abi_compatibility(runtime_c_path)?;
    let sources = runtime_source_bundle(runtime_c)?;
    let bundle_fingerprint = runtime_bundle_fingerprint(runtime_c)?;
    let mut object_paths = Vec::with_capacity(sources.len());
    for source_path in sources {
        let object_path = runtime_object_cache_path(
            &source_path,
            runtime_c_path,
            bundle_fingerprint,
            opt_level,
            &target,
            defines,
        )?;
        if !object_path.exists() {
            let temp_path = runtime_object_temp_path(&object_path);
            let compile_result = compile_runtime_source_to_object(
                clang_exe,
                &source_path,
                &temp_path,
                opt_level,
                &target,
                defines,
            );
            if let Err(error) = compile_result {
                let _ = fs::remove_file(&temp_path);
                return Err(error);
            }
            publish_runtime_object(&temp_path, &object_path)?;
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
        // Dedicated profile (release-like, but per-module codegen units and no
        // LTO) so archive members stay independently extractable; see
        // `[profile.staticlib]` in the workspace Cargo.toml.
        "staticlib"
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

pub(crate) fn ensure_async_runtime_staticlib(
    opt_level: u8,
    target: Option<&NativeBuildTarget>,
) -> Result<PathBuf> {
    let target = effective_target(target);
    if let Some(runtime) = resolve_installed_native_runtime(&target)? {
        return Ok(runtime.library);
    }
    eprintln!(
        "[toolchain::source_runtime_development] runtime_mode=source-development artifact_provenance=source-cargo-development release_eligible=false senline_pin_evidence=false"
    );
    let profile = async_runtime_profile(opt_level);
    let workspace_root = workspace_root();
    let mut command = Command::new("cargo");
    command
        .arg("build")
        .arg("-p")
        .arg("sengoo-runtime")
        .arg("--lib")
        .arg("--features")
        .arg("native-bridge")
        .env("RUSTFLAGS", "-C link-dead-code");
    if !crate::verbose_output_enabled() {
        // Hides cargo's `Compiling`/`Finished` banner for the bundled runtime
        // staticlib; real cargo errors still reach the user.
        command.arg("--quiet");
    }
    if profile != "debug" {
        command.arg("--profile").arg(profile);
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
    target: Option<&NativeBuildTarget>,
) -> Result<()> {
    if let Some(runtime_c) = runtime_c {
        object_paths.extend(ensure_runtime_objects_with_defines(
            clang_exe,
            runtime_c,
            opt_level,
            target,
            &[NATIVE_NET_RUNTIME_DEFINE, NATIVE_ASYNC_RUNTIME_DEFINE],
        )?);
    }
    object_paths.push(ensure_async_runtime_staticlib(opt_level, target)?);
    Ok(())
}

fn platform_linker_args(target: &NativeBuildTarget) -> Vec<&'static str> {
    if target.triple.ends_with("-apple-darwin") || target.triple.ends_with("-apple-macosx") {
        vec!["-framework", "Security", "-framework", "CoreFoundation"]
    } else if target.is_linux_gnu() {
        vec!["-lm"]
    } else {
        Vec::new()
    }
}

/// Pin-grade dual-build identity: enabled only when package scripts (or tests)
/// explicitly set `SENGOO_DETERMINISTIC_LINK` to a truthy value. Default off so
/// ordinary developer builds are unaffected.
fn deterministic_link_requested() -> bool {
    match std::env::var("SENGOO_DETERMINISTIC_LINK") {
        Ok(value) => {
            let trimmed = value.trim();
            !(trimmed.is_empty() || trimmed == "0" || trimmed.eq_ignore_ascii_case("false"))
        }
        Err(_) => false,
    }
}

fn append_deterministic_link_args(command: &mut Command, target: &NativeBuildTarget) {
    if !deterministic_link_requested() {
        return;
    }
    if target.is_linux_gnu() || target.triple.contains("linux") {
        // Drop GNU build-id notes that embed non-content identity.
        command.arg("-Wl,--build-id=none");
        command.arg("-Wl,--hash-style=gnu");
    } else if target.is_windows_msvc() {
        // clang driver path (cross or lld): /Brepro zeros PE timestamps.
        command.arg("-Wl,/Brepro");
    }
}

fn sorted_object_paths(object_paths: &[PathBuf]) -> Vec<PathBuf> {
    // Sort only relocatable objects. Static archives (.a/.lib) must stay after
    // the objects that reference them (Unix linkers resolve archive symbols in
    // left-to-right order). Reordering archives to the front caused
    // `undefined reference to sengoo_net_last_error` on Linux core-language CI.
    let mut objects = Vec::new();
    let mut archives = Vec::new();
    for path in object_paths {
        let is_archive = path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| {
                ext.eq_ignore_ascii_case("a")
                    || ext.eq_ignore_ascii_case("lib")
                    || ext.eq_ignore_ascii_case("rlib")
            });
        if is_archive {
            archives.push(path.clone());
        } else {
            objects.push(path.clone());
        }
    }
    objects.sort();
    objects.extend(archives);
    objects
}

fn link_cross_target(
    clang_exe: &str,
    object_paths: &[PathBuf],
    executable_path: &Path,
    target: &NativeBuildTarget,
    native_link_libraries: &[String],
) -> Result<()> {
    let search_paths = native_library_search_paths_from_env();
    let mut command = Command::new(clang_exe);
    command.arg("-Wno-override-module");
    apply_clang_target_args(&mut command, target)?;
    suppress_pass_through_driver_warnings(&mut command);
    if target.is_linux_gnu() {
        command.arg("-fuse-ld=lld");
    }
    let objects = sorted_object_paths(object_paths);
    for object in &objects {
        command.arg(object);
    }
    append_native_library_link_args(&mut command, native_link_libraries, target, &search_paths);
    for arg in platform_linker_args(target) {
        command.arg(arg);
    }
    append_deterministic_link_args(&mut command, target);
    command.arg("-o").arg(executable_path);
    let status = command
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke clang cross linker: {}", e))?;
    if !status.success() {
        if native_link_libraries.is_empty() {
            return Err(miette::miette!(
                "cross-compile link failed for target {}; verify SDK/sysroot env vars documented in docs/cross-compilation.md",
                target.triple
            ));
        }
        return Err(miette::miette!(format_native_link_failure_message(
            native_link_libraries
        )));
    }
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
    native_link_libraries: &[String],
    debug_info: bool,
) -> Result<std::process::ExitStatus> {
    let link_exe = find_msvc_link_exe().ok_or_else(|| {
        miette::miette!("failed to locate MSVC link.exe for native async linking")
    })?;
    let target = NativeBuildTarget::host();
    let search_paths = native_library_search_paths_from_env();
    let mut link_cmd = Command::new(link_exe);
    link_cmd.arg("/NOLOGO");
    if deterministic_link_requested() {
        // Zero PE timestamps / non-deterministic COFF metadata for dual-build
        // pin-grade package identity (MSVC link.exe /Brepro).
        link_cmd.arg("/Brepro");
    }
    let objects = sorted_object_paths(object_paths);
    let links_async_runtime = objects.iter().any(|path| is_async_runtime_staticlib(path));
    if links_async_runtime {
        // Keep compiler-generated async dispatch symbols that are only referenced
        // from the Rust async runtime static library.
        link_cmd.arg("/OPT:NOREF");
    }
    for lib_path in windows_link_lib_paths() {
        link_cmd.arg(format!("/LIBPATH:{}", lib_path.display()));
    }
    append_native_library_link_args(&mut link_cmd, native_link_libraries, &target, &search_paths);
    for object in &objects {
        link_cmd.arg(object);
    }
    for lib in [
        "kernel32.lib",
        "ntdll.lib",
        "userenv.lib",
        "ws2_32.lib",
        "dbghelp.lib",
        // Native net/TLS (schannel) support linked through the runtime staticlib.
        "advapi32.lib",
        "bcrypt.lib",
        "crypt32.lib",
        "ncrypt.lib",
        "secur32.lib",
        "legacy_stdio_definitions.lib",
        "msvcrt.lib",
        "vcruntime.lib",
        "ucrt.lib",
    ] {
        link_cmd.arg(lib);
    }
    link_cmd.arg("/ENTRY:mainCRTStartup");
    link_cmd.arg("/SUBSYSTEM:CONSOLE");
    append_windows_debug_link_args(&mut link_cmd, executable_path, debug_info);
    link_cmd.arg(format!("/OUT:{}", executable_path.display()));
    link_cmd
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke MSVC linker: {}", e))
}

#[cfg(any(windows, test))]
fn append_windows_debug_link_args(command: &mut Command, executable_path: &Path, debug_info: bool) {
    if !debug_info {
        return;
    }
    command.arg("/DEBUG:FULL");
    command.arg(format!(
        "/PDB:{}",
        executable_path.with_extension("pdb").display()
    ));
}

fn append_debug_compile_arg(command: &mut Command, target: &NativeBuildTarget, debug_info: bool) {
    if !debug_info {
        return;
    }
    command.arg(if target.is_windows_msvc() {
        "-gcodeview"
    } else {
        "-g"
    });
}

fn find_package_root(path: &Path) -> Option<PathBuf> {
    let mut dir = if path.is_file() {
        path.parent()?.to_path_buf()
    } else {
        path.to_path_buf()
    };
    loop {
        if dir.join("Sengoo.toml").is_file() {
            return Some(dir);
        }
        dir = dir.parent()?.to_path_buf();
    }
}

pub(crate) fn collect_package_native_c_sources(
    input_path: &Path,
    root_source: &str,
) -> Vec<PathBuf> {
    use std::collections::BTreeSet;

    let module_sources = collect_module_sources_with_edges(input_path, root_source);
    let mut package_roots = BTreeSet::new();
    for module_key in module_sources.keys() {
        if let Some(root) = find_package_root(Path::new(module_key)) {
            package_roots.insert(fs::canonicalize(&root).unwrap_or(root));
        }
    }
    if let Some(root) = find_package_root(input_path) {
        package_roots.insert(fs::canonicalize(&root).unwrap_or(root));
    }

    let mut sources = Vec::new();
    for root in package_roots {
        let native_dir = root.join("native");
        if !native_dir.is_dir() {
            continue;
        }
        let Ok(entries) = fs::read_dir(&native_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("c") {
                sources.push(fs::canonicalize(&path).unwrap_or(path));
            }
        }
    }
    sources.sort();
    sources.dedup();
    sources
}

fn native_c_compile_include_args() -> Vec<String> {
    let mut args = Vec::new();
    if let Ok(dir) = std::env::var(SDL2_INCLUDE_DIR_ENV) {
        if !dir.trim().is_empty() {
            args.push("-I".to_string());
            args.push(dir);
        }
    }
    args
}

fn is_sgplatform_shim_source(source_path: &Path) -> bool {
    source_path
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name == "sgplatform_shim.c")
}

fn native_c_compile_defines(source_path: &Path) -> Vec<&'static str> {
    if !sgplatform_graphics_skip_enabled() {
        return Vec::new();
    }
    if is_sgplatform_shim_source(source_path) {
        vec!["-DSGPLATFORM_STUB=1"]
    } else {
        Vec::new()
    }
}

fn package_native_object_fingerprint(source_path: &Path) -> Result<u64> {
    let mut hasher = DefaultHasher::new();
    file_fingerprint(source_path)?.hash(&mut hasher);
    if is_sgplatform_shim_source(source_path) {
        sgplatform_graphics_skip_enabled().hash(&mut hasher);
    }
    Ok(hasher.finish())
}

pub(crate) fn compile_c_to_object(
    clang_exe: &str,
    source_path: &Path,
    object_path: &Path,
    opt_level: u8,
    target: Option<&NativeBuildTarget>,
) -> Result<()> {
    let target = effective_target(target);
    let mut command = Command::new(clang_exe);
    command
        .arg("-Wno-override-module")
        .arg(format!("-O{}", opt_level));
    apply_clang_target_args(&mut command, &target)?;
    for arg in native_c_compile_include_args() {
        command.arg(arg);
    }
    for arg in native_c_compile_defines(source_path) {
        command.arg(arg);
    }
    let status = command
        .arg("-c")
        .arg(source_path)
        .arg("-o")
        .arg(object_path)
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke clang for C compilation: {}", e))?;
    if !status.success() {
        let mut message = format!(
            "failed to compile native package source {}",
            source_path.display()
        );
        if source_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains("sgplatform") || name.contains("sdl"))
        {
            message.push_str(&format!(
                "\nHint: install SDL2 development headers and set {SDL2_INCLUDE_DIR_ENV} (and {SDL2_LIB_DIR_ENV} for linking). See docs/sgplatform.md."
            ));
        }
        return Err(miette::miette!(message));
    }
    Ok(())
}

pub(crate) fn append_package_native_inputs(
    clang_exe: &str,
    object_paths: &mut Vec<PathBuf>,
    input_path: &Path,
    root_source: &str,
    build_dir: &Path,
    opt_level: u8,
    target: Option<&NativeBuildTarget>,
) -> Result<()> {
    let target = effective_target(target);
    for source in collect_package_native_c_sources(input_path, root_source) {
        let stem = source
            .file_stem()
            .and_then(|name| name.to_str())
            .unwrap_or("native");
        let hash = package_native_object_fingerprint(&source)?;
        let object_path =
            build_dir.join(format!("{}-{:x}.{}", stem, hash, target.object_extension()));
        if !object_path.is_file() {
            if let Some(parent) = object_path.parent() {
                fs::create_dir_all(parent).into_diagnostic()?;
            }
            compile_c_to_object(clang_exe, &source, &object_path, opt_level, Some(&target))?;
        }
        object_paths.push(object_path);
    }
    Ok(())
}

pub(crate) fn compile_ir_to_object(
    clang_exe: &str,
    llvm_ir_path: &Path,
    object_path: &Path,
    opt_level: u8,
    target: Option<&NativeBuildTarget>,
    debug_info: bool,
) -> Result<()> {
    let target = effective_target(target);
    let mut command = Command::new(clang_exe);
    command
        .arg("-Wno-override-module")
        .arg(format!("-O{}", opt_level));
    append_debug_compile_arg(&mut command, &target, debug_info);
    apply_clang_target_args(&mut command, &target)?;
    suppress_pass_through_driver_warnings(&mut command);

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
    native_link_libraries: &[String],
    target: &NativeBuildTarget,
) -> Result<std::process::ExitStatus> {
    let search_paths = native_library_search_paths_from_env();
    let mut clang_cmd = Command::new(clang_exe);
    clang_cmd.arg("-Wno-override-module");
    if use_lld {
        clang_cmd.arg("-fuse-ld=lld");
    }
    let objects = sorted_object_paths(object_paths);
    for object in &objects {
        clang_cmd.arg(object);
    }
    append_native_library_link_args(&mut clang_cmd, native_link_libraries, target, &search_paths);
    for arg in platform_linker_args(target) {
        clang_cmd.arg(arg);
    }
    append_deterministic_link_args(&mut clang_cmd, target);
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
    target: Option<&NativeBuildTarget>,
    native_link_libraries: Option<&[String]>,
) -> Result<()> {
    link_native_binary_from_objects_with_debug(
        clang_exe,
        object_paths,
        executable_path,
        target,
        native_link_libraries,
        false,
    )
}

#[cfg(windows)]
pub(crate) fn link_native_binary_from_objects_with_debug(
    clang_exe: &str,
    object_paths: &[PathBuf],
    executable_path: &Path,
    target: Option<&NativeBuildTarget>,
    native_link_libraries: Option<&[String]>,
    debug_info: bool,
) -> Result<()> {
    let target = effective_target(target);
    let libraries = native_link_libraries.unwrap_or(&[]);
    if target.is_cross() {
        return link_cross_target(clang_exe, object_paths, executable_path, &target, libraries);
    }
    let _ = clang_exe;
    let status = run_windows_link_command(object_paths, executable_path, libraries, debug_info)?;
    if !status.success() {
        return Err(miette::miette!(format_native_link_failure_message(
            libraries
        )));
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn link_native_binary_from_objects(
    clang_exe: &str,
    object_paths: &[PathBuf],
    executable_path: &Path,
    target: Option<&NativeBuildTarget>,
    native_link_libraries: Option<&[String]>,
) -> Result<()> {
    link_native_binary_from_objects_with_debug(
        clang_exe,
        object_paths,
        executable_path,
        target,
        native_link_libraries,
        false,
    )
}

#[cfg(not(windows))]
pub(crate) fn link_native_binary_from_objects_with_debug(
    clang_exe: &str,
    object_paths: &[PathBuf],
    executable_path: &Path,
    target: Option<&NativeBuildTarget>,
    native_link_libraries: Option<&[String]>,
    _debug_info: bool,
) -> Result<()> {
    let target = effective_target(target);
    let libraries = native_link_libraries.unwrap_or(&[]);
    if target.is_cross() {
        return link_cross_target(clang_exe, object_paths, executable_path, &target, libraries);
    }
    let mode = linker_mode_from_env();
    let lld_state = LLD_AVAILABILITY.load(Ordering::Relaxed);
    let try_lld_first = match mode {
        LinkerMode::Lld => true,
        LinkerMode::System => false,
        LinkerMode::Auto => lld_state != LINKER_UNAVAILABLE,
    };

    if try_lld_first {
        let lld_status = run_link_command(
            clang_exe,
            object_paths,
            executable_path,
            true,
            libraries,
            &target,
        )?;
        if lld_status.success() {
            if matches!(mode, LinkerMode::Auto) {
                LLD_AVAILABILITY.store(LINKER_AVAILABLE, Ordering::Relaxed);
            }
            return Ok(());
        }
        if matches!(mode, LinkerMode::Lld) {
            return Err(miette::miette!(format_native_link_failure_message(
                libraries
            )));
        }
        LLD_AVAILABILITY.store(LINKER_UNAVAILABLE, Ordering::Relaxed);
        vprintln!("link fallback: lld unavailable, retrying with system linker");
    }

    let status = run_link_command(
        clang_exe,
        object_paths,
        executable_path,
        false,
        libraries,
        &target,
    )?;
    if !status.success() {
        return Err(miette::miette!(format_native_link_failure_message(
            libraries
        )));
    }
    Ok(())
}

pub(crate) fn compile_native_binary(
    clang_exe: &str,
    llvm_ir_path: &Path,
    executable_path: &Path,
    runtime_c: Option<&str>,
    opt_level: u8,
    target: Option<&NativeBuildTarget>,
    native_link_libraries: Option<&[String]>,
) -> Result<()> {
    let target = effective_target(target);
    let object_path = executable_path.with_extension(target.object_extension());
    compile_ir_to_object(
        clang_exe,
        llvm_ir_path,
        &object_path,
        opt_level,
        Some(&target),
        false,
    )?;
    let mut object_paths = vec![object_path];
    append_native_runtime_inputs(
        clang_exe,
        &mut object_paths,
        runtime_c,
        opt_level,
        Some(&target),
    )?;
    link_native_binary_from_objects(
        clang_exe,
        &object_paths,
        executable_path,
        Some(&target),
        native_link_libraries,
    )?;
    Ok(())
}

pub(crate) fn run_native_binary_with_args(executable_path: &Path, args: &[String]) -> Result<i32> {
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

    let exit_code = run_output.status.code().unwrap_or(1);
    println!("exit code: {}", exit_code);
    Ok(exit_code)
}

pub(crate) fn propagate_run_exit_code(exit_code: i32) -> Result<()> {
    if exit_code != 0 {
        std::process::exit(exit_code);
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn recover_native_output_from_cached_artifacts(
    clang_exe: &str,
    llvm_ir_path: &Path,
    object_path: &Path,
    output_path: &Path,
    runtime_c: Option<&str>,
    opt_level: u8,
    debug_info: bool,
    native_link_libraries: Option<&[String]>,
) -> Result<CachedNativeRecoveryPlan> {
    let recovery_plan =
        derive_cached_native_recovery_plan(llvm_ir_path.exists(), object_path.exists())
            .ok_or_else(|| miette::miette!("cached object and LLVM IR are both missing"))?;

    if matches!(
        recovery_plan,
        CachedNativeRecoveryPlan::RebuildObjectFromCachedIr
    ) {
        compile_ir_to_object(
            clang_exe,
            llvm_ir_path,
            object_path,
            opt_level,
            None,
            debug_info,
        )?;
    }

    let mut object_paths = vec![object_path.to_path_buf()];
    append_native_runtime_inputs(clang_exe, &mut object_paths, runtime_c, opt_level, None)?;
    link_native_binary_from_objects_with_debug(
        clang_exe,
        &object_paths,
        output_path,
        None,
        native_link_libraries,
        debug_info,
    )?;

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
        let before = runtime_object_cache_path(
            &runtime_c,
            &runtime_c,
            before_fingerprint,
            1,
            &NativeBuildTarget::host(),
            &[],
        )
        .unwrap();

        fs::write(&runtime_c, "bbbb").unwrap();
        let file = fs::OpenOptions::new().write(true).open(&runtime_c).unwrap();
        file.set_times(fs::FileTimes::new().set_modified(modified))
            .unwrap();
        let after_fingerprint = runtime_bundle_fingerprint(&runtime_c.to_string_lossy()).unwrap();
        let after = runtime_object_cache_path(
            &runtime_c,
            &runtime_c,
            after_fingerprint,
            1,
            &NativeBuildTarget::host(),
            &[],
        )
        .unwrap();

        assert_ne!(before, after);
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn runtime_object_cache_path_changes_with_defines() {
        let root = temp_test_dir("runtime-object-defines");
        let runtime_c = root.join("runtime.c");
        fs::create_dir_all(&root).unwrap();
        fs::write(&runtime_c, "aaaa").unwrap();
        let fingerprint = runtime_bundle_fingerprint(&runtime_c.to_string_lossy()).unwrap();

        let plain = runtime_object_cache_path(
            &runtime_c,
            &runtime_c,
            fingerprint,
            1,
            &NativeBuildTarget::host(),
            &[],
        )
        .unwrap();
        let native_net = runtime_object_cache_path(
            &runtime_c,
            &runtime_c,
            fingerprint,
            1,
            &NativeBuildTarget::host(),
            &[NATIVE_NET_RUNTIME_DEFINE],
        )
        .unwrap();

        assert_ne!(
            plain, native_net,
            "C-only and native-net runtime objects must not share a cache slot"
        );
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn runtime_abi_header_parser_accepts_the_frozen_define() {
        assert_eq!(
            parse_runtime_abi_version(
                "#define SENGOO_COLLECTIONS_ABI_VERSION 1\n#define SENGOO_RUNTIME_ABI_VERSION 7\n"
            )
            .unwrap(),
            7
        );
        let error = parse_runtime_abi_version("#define SENGOO_OTHER_VERSION 1\n").unwrap_err();
        assert!(error
            .to_string()
            .contains("missing SENGOO_RUNTIME_ABI_VERSION"));
    }

    #[test]
    fn runtime_abi_validation_reports_required_and_available_versions() {
        let root = temp_test_dir("runtime-abi-mismatch");
        let runtime_c = root.join("runtime.c");
        let header = root.join(RUNTIME_SHARED_HEADER);
        fs::create_dir_all(&root).unwrap();
        fs::write(&runtime_c, "#include \"runtime_shared.h\"\n").unwrap();
        fs::write(&header, "#define SENGOO_RUNTIME_ABI_VERSION 99\n").unwrap();

        let error = validate_runtime_abi_compatibility(&runtime_c).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("runtime ABI mismatch"), "{message}");
        assert!(
            message.contains(&format!("requires {SENGOO_RUNTIME_ABI_VERSION}")),
            "{message}"
        );
        assert!(message.contains("provides 99"), "{message}");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn runtime_object_cache_publish_is_atomic_under_competing_writers() {
        let root = temp_test_dir("runtime-object-publish");
        fs::create_dir_all(&root).unwrap();
        let object_path = root.join("runtime.obj");
        let first_temp = root.join("first.tmp");
        let second_temp = root.join("second.tmp");
        fs::write(&first_temp, b"complete-first").unwrap();
        fs::write(&second_temp, b"complete-second").unwrap();

        let barrier = std::sync::Arc::new(std::sync::Barrier::new(2));
        std::thread::scope(|scope| {
            for temp_path in [&first_temp, &second_temp] {
                let barrier = barrier.clone();
                let object_path = &object_path;
                scope.spawn(move || {
                    barrier.wait();
                    publish_runtime_object(temp_path, object_path).unwrap();
                });
            }
        });

        let published = fs::read(&object_path).unwrap();
        assert!(published == b"complete-first" || published == b"complete-second");
        assert!(!first_temp.exists());
        assert!(!second_temp.exists());
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn parse_clang_major_version_handles_common_formats() {
        assert_eq!(parse_clang_major_version("clang version 19.1.7"), Some(19));
        assert_eq!(
            parse_clang_major_version("Ubuntu clang version 15.0.7 (tags/RELEASE_1507/final)"),
            Some(15)
        );
        assert_eq!(
            parse_clang_major_version("Apple clang version 16.0.0 (clang-1600.0.26.3)"),
            Some(16)
        );
    }

    #[test]
    fn parse_clang_major_version_rejects_unparseable_output() {
        assert_eq!(
            parse_clang_major_version("not a clang version banner"),
            None
        );
    }

    #[test]
    fn validate_clang_major_version_reports_contract_floor() {
        let error = validate_clang_major_version(Some(14), "clang").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("unsupported clang/LLVM 14"));
        assert!(message.contains("clang/LLVM 15+"));
        assert!(message.contains("--emit-llvm"));
    }

    #[test]
    fn validate_clang_major_version_reports_unparseable_banner() {
        let error = validate_clang_major_version(None, "clang").unwrap_err();
        let message = error.to_string();
        assert!(message.contains("unable to determine clang/LLVM version"));
        assert!(message.contains("clang/LLVM 15+"));
    }

    #[test]
    #[cfg(not(windows))]
    fn windows_cross_target_uses_env_sdk_include_paths_on_non_windows_hosts() {
        let root = temp_test_dir("windows-cross-sdk");
        for leaf in ["ucrt", "um", "shared"] {
            fs::create_dir_all(root.join(leaf)).unwrap();
        }

        std::env::set_var("SENGOO_WINDOWS_SDK_ROOT", &root);
        let mut command = Command::new("clang");
        let target =
            NativeBuildTarget::resolve(Some(crate::cross_compile::REFERENCE_TARGET_WINDOWS_MSVC))
                .unwrap();

        apply_clang_target_args(&mut command, &target).unwrap();

        let args = command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect::<Vec<_>>();
        assert!(args.contains(&"--target=x86_64-pc-windows-msvc".to_string()));
        assert!(args.contains(&"-fms-runtime-lib=dll".to_string()));
        assert!(args.iter().any(|arg| arg.ends_with("/ucrt")));
        assert!(args.iter().any(|arg| arg.ends_with("/um")));
        assert!(args.iter().any(|arg| arg.ends_with("/shared")));

        std::env::remove_var("SENGOO_WINDOWS_SDK_ROOT");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn native_link_adds_macos_security_and_corefoundation_frameworks() {
        for triple in ["aarch64-apple-darwin", "aarch64-apple-macosx"] {
            let target = NativeBuildTarget {
                triple: triple.to_string(),
            };
            assert_eq!(
                platform_linker_args(&target),
                vec!["-framework", "Security", "-framework", "CoreFoundation"]
            );
        }
    }

    #[test]
    fn native_link_adds_libm_for_linux_targets() {
        let target = NativeBuildTarget {
            triple: crate::cross_compile::REFERENCE_TARGET_LINUX_GNU.to_string(),
        };
        assert_eq!(platform_linker_args(&target), vec!["-lm"]);
    }

    #[test]
    fn debug_compile_uses_codeview_for_windows_msvc_and_dwarf_elsewhere() {
        let windows = NativeBuildTarget {
            triple: crate::cross_compile::REFERENCE_TARGET_WINDOWS_MSVC.to_string(),
        };
        let linux = NativeBuildTarget {
            triple: crate::cross_compile::REFERENCE_TARGET_LINUX_GNU.to_string(),
        };
        let mut windows_command = Command::new("clang");
        let mut linux_command = Command::new("clang");

        append_debug_compile_arg(&mut windows_command, &windows, true);
        append_debug_compile_arg(&mut linux_command, &linux, true);

        assert_eq!(
            windows_command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["-gcodeview"]
        );
        assert_eq!(
            linux_command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec!["-g"]
        );
    }

    #[test]
    fn debug_windows_link_uses_full_symbols_and_deterministic_pdb_path() {
        let executable = Path::new(r"C:\build\debugger_probe.exe");
        let mut command = Command::new("link.exe");

        append_windows_debug_link_args(&mut command, executable, true);

        assert_eq!(
            command
                .get_args()
                .map(|arg| arg.to_string_lossy().into_owned())
                .collect::<Vec<_>>(),
            vec![
                "/DEBUG:FULL".to_string(),
                format!("/PDB:{}", executable.with_extension("pdb").display()),
            ]
        );
    }

    #[test]
    fn non_debug_windows_link_does_not_request_pdb_output() {
        let mut command = Command::new("link.exe");

        append_windows_debug_link_args(
            &mut command,
            Path::new(r"C:\build\debugger_probe.exe"),
            false,
        );

        assert_eq!(command.get_args().count(), 0);
    }
}
