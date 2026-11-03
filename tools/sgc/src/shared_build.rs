//! Shared libraries use the same checked frontend and native runtime inputs as executables.
use miette::{IntoDiagnostic, Result};
use sengoo_compiler::{Codegen, CompileOptions, DebugInfoConfig, IntegerOverflowMode, MirOptLevel};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::{ContractChecksMode, NativeBuildTarget};

pub(crate) fn shared_extension() -> &'static str {
    if cfg!(windows) {
        "dll"
    } else if cfg!(target_os = "macos") {
        "dylib"
    } else {
        "so"
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_shared(
    input: &str,
    output: Option<&str>,
    opt_level: u8,
    checks: ContractChecksMode,
    target: Option<&str>,
    python_exports: &[String],
    debug_info: bool,
) -> Result<()> {
    crate::installed_runtime::validate_native_runtime_mode_environment()?;
    let target = NativeBuildTarget::resolve(target)?;
    if target.is_cross() {
        miette::bail!("shared libraries currently require the native host target; build platform wheels on each target host");
    }
    let input = Path::new(input);
    let root_source = fs::read_to_string(input).into_diagnostic()?;
    let source = crate::expand_imports_for_source(input, &root_source)?;
    let options = CompileOptions {
        mir_opt_level: match opt_level {
            0 => MirOptLevel::O0,
            1 => MirOptLevel::O1,
            3 => MirOptLevel::O3,
            _ => MirOptLevel::O2,
        },
        runtime_contract_checks: match checks {
            ContractChecksMode::On => true,
            ContractChecksMode::Off => false,
            ContractChecksMode::Auto => opt_level < 2,
        },
    };
    // There is deliberately no main-based pruning: exported functions are roots even
    // in a library-only program. The native linker can discard unreachable internals.
    let mut bundle =
        sengoo_compiler::compile_to_mir_bundle_for_target(&source, options, &target.triple)
            .map_err(|error| miette::miette!("shared library compilation failed: {error}"))?;
    let bridge = if python_exports.is_empty() {
        None
    } else {
        Some(crate::python_bindings::generate(
            &source,
            python_exports,
            &bundle.functions,
        )?)
    };
    let mut exports = bundle
        .ffi_codegen
        .export_symbols
        .iter()
        .map(|symbol| symbol.export_name.clone())
        .collect::<BTreeSet<_>>();
    if let Some(bridge) = &bridge {
        exports.extend(bridge.exports.iter().cloned());
    }
    if exports.is_empty() {
        miette::bail!("shared library has no exports; use existing extern C exports or --python-export FUNCTION");
    }
    sengoo_compiler::mir::opt::pipeline_for_level(options.mir_opt_level).run(&mut bundle.functions);
    let mut codegen = Codegen::with_ffi_target_debug_and_overflow(
        bundle.ffi_codegen,
        Some(target.triple.clone()),
        DebugInfoConfig::disabled(),
        if opt_level < 2 {
            IntegerOverflowMode::DebugChecked
        } else {
            IntegerOverflowMode::ReleaseWrapping
        },
    );
    let mut ir = codegen
        .codegen(&bundle.functions)
        .map_err(|error| miette::miette!("{error}"))?;
    if let Some(bridge) = &bridge {
        ir.push_str(&bridge.llvm);
    }
    let stem = input.file_stem().unwrap_or_default().to_string_lossy();
    let output = output
        .map(PathBuf::from)
        .unwrap_or_else(|| input.with_file_name(format!("{stem}.{}", shared_extension())));
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).into_diagnostic()?;
    }
    let build_dir = output
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!(".sengoo-shared-{stem}"));
    fs::create_dir_all(&build_dir).into_diagnostic()?;
    let ir_path = build_dir.join("module.ll");
    fs::write(&ir_path, ir).into_diagnostic()?;
    let clang = crate::find_clang()
        .ok_or_else(|| miette::miette!("clang is required for shared builds; run sgc doctor"))?;
    crate::ensure_supported_clang_toolchain(&clang)?;
    let object = build_dir.join(format!("module.{}", target.object_extension()));
    crate::native_toolchain::compile_ir_to_object_for_shared(
        &clang,
        &ir_path,
        &object,
        opt_level,
        Some(&target),
        debug_info,
    )?;
    let mut objects = vec![object];
    let runtime_c = crate::find_runtime_c()
        .ok_or_else(|| miette::miette!("native runtime is missing; run sgc doctor"))?;
    crate::native_toolchain::append_native_runtime_inputs_for_shared(
        &clang,
        &mut objects,
        &runtime_c,
        opt_level,
        Some(&target),
    )?;
    crate::native_toolchain::append_package_native_inputs_for_shared(
        &clang,
        &mut objects,
        input,
        &root_source,
        &build_dir,
        opt_level,
        Some(&target),
    )?;
    if let Some(bridge) = &bridge {
        let c_path = build_dir.join("python_bridge.c");
        let object = build_dir.join(format!("python_bridge.{}", target.object_extension()));
        fs::write(&c_path, &bridge.c_source).into_diagnostic()?;
        crate::native_toolchain::compile_c_to_object_for_shared(
            &clang,
            &c_path,
            &object,
            opt_level,
            Some(&target),
        )?;
        objects.push(object);
    }
    let libraries = crate::collect_native_link_libraries_for_graph(input, &root_source)?;
    crate::native_toolchain::link_native_shared_from_objects(
        &clang, &objects, &output, &build_dir, &exports, &libraries, &target,
    )?;
    if let Some(bridge) = bridge {
        let descriptor = PathBuf::from(format!("{}.sgpy.json", output.display()));
        fs::write(
            &descriptor,
            serde_json::to_vec_pretty(&bridge.descriptor).into_diagnostic()?,
        )
        .into_diagnostic()?;
        println!("Python binding descriptor: {}", descriptor.display());
    }
    println!("Built shared library: {}", output.display());
    Ok(())
}
