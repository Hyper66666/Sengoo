use miette::{IntoDiagnostic, Result};
use sengoo_runtime::{ReflectionRuntime, ReflectionSymbolMetadata};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::Ordering;
use std::time::Instant;

use crate::{
    compile_ir_to_object, ensure_runtime_objects, linker_mode_from_env, object_file_extension,
    validate_reflection_metadata, LinkerMode, ReflectionMetadata, LINKER_AVAILABLE,
    LINKER_UNAVAILABLE, LLD_AVAILABILITY,
};

fn reflection_shared_library_extension() -> &'static str {
    if cfg!(windows) {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

fn reflection_shared_library_path_for_artifact(artifact_path: &Path) -> PathBuf {
    PathBuf::from(format!(
        "{}.sgreflect.{}",
        artifact_path.to_string_lossy(),
        reflection_shared_library_extension()
    ))
}

fn reflection_native_export_symbols_from_sidecar(sidecar_path: &Path) -> Result<Vec<String>> {
    let bytes = fs::read(sidecar_path).into_diagnostic().map_err(|e| {
        miette::miette!(
            "failed to read reflection sidecar for native export symbols {}: {}",
            sidecar_path.to_string_lossy(),
            e
        )
    })?;
    let metadata: ReflectionMetadata =
        serde_json::from_slice(&bytes)
            .into_diagnostic()
            .map_err(|e| {
                miette::miette!(
                    "failed to parse reflection sidecar for native export symbols {}: {}",
                    sidecar_path.to_string_lossy(),
                    e
                )
            })?;
    validate_reflection_metadata(&metadata)?;

    let mut symbols = HashSet::<String>::new();
    for module in metadata.modules {
        for symbol in module.symbols {
            let exported = symbol.native_symbol.unwrap_or_else(|| {
                symbol
                    .symbol
                    .rsplit("::")
                    .next()
                    .unwrap_or_default()
                    .to_string()
            });
            if !exported.trim().is_empty() {
                symbols.insert(exported);
            }
        }
    }
    let mut exported = symbols.into_iter().collect::<Vec<_>>();
    exported.sort();
    Ok(exported)
}

fn run_shared_link_command(
    clang_exe: &str,
    object_paths: &[PathBuf],
    shared_library_path: &Path,
    use_lld: bool,
    extra_linker_flags: &[String],
) -> Result<std::process::ExitStatus> {
    let mut clang_cmd = Command::new(clang_exe);
    clang_cmd.arg("-Wno-override-module");
    if use_lld {
        clang_cmd.arg("-fuse-ld=lld");
    }
    clang_cmd.arg("-shared");
    for object in object_paths {
        clang_cmd.arg(object);
    }
    for flag in extra_linker_flags {
        clang_cmd.arg(flag);
    }
    clang_cmd.arg("-o").arg(shared_library_path);
    clang_cmd
        .status()
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to invoke clang shared linker: {}", e))
}

fn link_shared_library_from_objects(
    clang_exe: &str,
    object_paths: &[PathBuf],
    shared_library_path: &Path,
    export_symbols: &[String],
) -> Result<()> {
    let mode = linker_mode_from_env();
    let lld_state = LLD_AVAILABILITY.load(Ordering::Relaxed);
    let try_lld_first = match mode {
        LinkerMode::Lld => true,
        LinkerMode::System => false,
        LinkerMode::Auto => lld_state != LINKER_UNAVAILABLE,
    };

    let export_linker_flags = if cfg!(windows) {
        export_symbols
            .iter()
            .map(|symbol| format!("-Wl,/EXPORT:{}", symbol))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let export_all_linker_flags = if cfg!(windows) {
        vec!["-Wl,--export-all-symbols".to_string()]
    } else {
        Vec::new()
    };

    if try_lld_first {
        let lld_status = run_shared_link_command(
            clang_exe,
            object_paths,
            shared_library_path,
            true,
            &export_linker_flags,
        )?;
        if lld_status.success() {
            if matches!(mode, LinkerMode::Auto) {
                LLD_AVAILABILITY.store(LINKER_AVAILABLE, Ordering::Relaxed);
            }
            return Ok(());
        }
        if cfg!(windows) {
            let lld_export_all_status = run_shared_link_command(
                clang_exe,
                object_paths,
                shared_library_path,
                true,
                &export_all_linker_flags,
            )?;
            if lld_export_all_status.success() {
                if matches!(mode, LinkerMode::Auto) {
                    LLD_AVAILABILITY.store(LINKER_AVAILABLE, Ordering::Relaxed);
                }
                return Ok(());
            }
        }
        if matches!(mode, LinkerMode::Lld) {
            return Err(miette::miette!("compile failed (lld linker mode)"));
        }
        LLD_AVAILABILITY.store(LINKER_UNAVAILABLE, Ordering::Relaxed);
        println!("link fallback: lld unavailable, retrying with system linker");
    }

    let status = run_shared_link_command(
        clang_exe,
        object_paths,
        shared_library_path,
        false,
        &export_linker_flags,
    )?;
    if status.success() {
        return Ok(());
    }
    if cfg!(windows) {
        let status_export_all = run_shared_link_command(
            clang_exe,
            object_paths,
            shared_library_path,
            false,
            &export_all_linker_flags,
        )?;
        if status_export_all.success() {
            return Ok(());
        }
    }
    Err(miette::miette!("compile failed"))
}

fn compile_reflection_shared_library(
    clang_exe: &str,
    llvm_ir_path: &Path,
    shared_library_path: &Path,
    runtime_c: Option<&str>,
    opt_level: u8,
    export_symbols: &[String],
) -> Result<()> {
    let object_path = shared_library_path.with_extension(object_file_extension());
    compile_ir_to_object(clang_exe, llvm_ir_path, &object_path, opt_level, None)?;
    let mut object_paths = vec![object_path];
    if let Some(runtime_c) = runtime_c {
        object_paths.extend(ensure_runtime_objects(
            clang_exe, runtime_c, opt_level, None,
        )?);
    }
    link_shared_library_from_objects(
        clang_exe,
        &object_paths,
        shared_library_path,
        export_symbols,
    )?;
    Ok(())
}

pub(crate) fn maybe_prepare_reflection_native_library(
    clang_exe: Option<&str>,
    runtime_c: Option<&str>,
    llvm_ir_path: &Path,
    artifact_path: &Path,
    sidecar_path: &Path,
    opt_level: u8,
) -> Result<Option<PathBuf>> {
    let Some(clang_exe) = clang_exe else {
        return Ok(None);
    };
    if !llvm_ir_path.exists() || !sidecar_path.exists() {
        return Ok(None);
    }

    let export_symbols = reflection_native_export_symbols_from_sidecar(sidecar_path)?;
    if export_symbols.is_empty() {
        return Ok(None);
    }

    let shared_library_path = reflection_shared_library_path_for_artifact(artifact_path);
    compile_reflection_shared_library(
        clang_exe,
        llvm_ir_path,
        &shared_library_path,
        runtime_c,
        opt_level,
        &export_symbols,
    )?;
    Ok(Some(shared_library_path))
}

pub(crate) fn signature_is_zero_arity_i64(signature: &str) -> bool {
    let mut params = None::<&str>;
    let mut ret = None::<&str>;

    for part in signature.split('|') {
        if let Some(value) = part.strip_prefix("params=[") {
            params = value.strip_suffix(']');
        } else if let Some(value) = part.strip_prefix("ret=") {
            ret = Some(value.trim());
        }
    }

    matches!(ret, Some("i64")) && matches!(params, Some(raw) if raw.trim().is_empty())
}

pub(crate) fn select_reflection_i64_zero_arity_symbol(
    symbols: &[ReflectionSymbolMetadata],
) -> Option<String> {
    for preferred in ["reflect_probe", "main"] {
        for symbol in symbols {
            let short = symbol.symbol.rsplit("::").next().unwrap_or_default();
            if short == preferred && signature_is_zero_arity_i64(&symbol.signature) {
                return Some(short.to_string());
            }
        }
    }

    for symbol in symbols {
        let short = symbol.symbol.rsplit("::").next().unwrap_or_default();
        if signature_is_zero_arity_i64(&symbol.signature) {
            return Some(short.to_string());
        }
    }

    None
}

pub(crate) fn measure_reflection_used_ms(
    sidecar_path: &Path,
    module_id: &str,
    native_library_path: Option<&Path>,
) -> Result<(f64, bool)> {
    let runtime = ReflectionRuntime::new(sidecar_path);
    let start = Instant::now();
    let symbols = runtime
        .list_symbols(module_id)
        .map_err(|e| miette::miette!("reflection API list failed: {}", e))?;
    let symbol = select_reflection_i64_zero_arity_symbol(&symbols).ok_or_else(|| {
        miette::miette!(
            "no zero-arity i64 symbol found for reflection invoke in module {}",
            module_id
        )
    })?;

    let mut native_bound = false;
    if let Some(native_library_path) = native_library_path {
        native_bound = runtime
            .register_i64_native_bindings_from_library(native_library_path)
            .is_ok();
    }
    if !native_bound {
        runtime
            .register_fn(module_id, &symbol, |_args| {
                Ok(sengoo_runtime::ReflectValue::I64(0))
            })
            .map_err(|e| miette::miette!("reflection API register failed: {}", e))?;
    }

    runtime
        .call_i64(module_id, &symbol, &[])
        .map_err(|e| miette::miette!("reflection API typed invoke failed: {}", e))?;
    Ok((start.elapsed().as_secs_f64() * 1000.0, native_bound))
}
