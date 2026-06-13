use crate::*;
use miette::{IntoDiagnostic, Result};
use sengoo_compiler::DebugInfoConfig;
use std::fs;
use std::path::{Path, PathBuf};

use super::shared::{
    contract_checks_mode_label, resolve_contract_checks_enabled, ContractChecksOverrideGuard,
    LargeProjectModeOverrideGuard,
};
use super::workset_optimizations::{
    can_reuse_artifacts_for_unreachable_impl_only_changes, can_skip_codegen_via_generic_cache,
};
#[allow(clippy::too_many_arguments)]
pub(crate) async fn cmd_build(
    input: &str,
    output: Option<&str>,
    opt_level: u8,
    contract_checks: ContractChecksMode,
    emit_llvm: bool,
    force_rebuild: bool,
    low_memory: bool,
    frontend_jobs: FrontendJobs,
    frontend_trace: bool,
    reflection: ReflectionCliOptions,
    target: Option<&str>,
    timings_json: Option<&str>,
    debug_info: bool,
) -> Result<()> {
    let build_target = NativeBuildTarget::resolve(target)?;
    if build_target.is_cross() {
        println!("cross-compile target: {}", build_target.triple);
    }
    println!("Building: {}", input);

    let input_path = Path::new(input);
    let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
    let source_dir = input_path.parent().unwrap_or(Path::new("."));
    let build_dir = source_dir.join("build");
    fs::create_dir_all(&build_dir).into_diagnostic()?;

    let root_source = fs::read_to_string(input)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to read source {}: {}", input, e))?;
    let source = expand_imports_for_source(input_path, &root_source)?;
    let native_link_libraries = collect_native_link_libraries_for_graph(input_path, &root_source)?;
    if !native_link_libraries.is_empty() {
        println!(
            "native link libraries: {}",
            native_link_libraries.join(", ")
        );
    }
    if let Some(hint) = maybe_low_memory_mode_hint(source.len(), low_memory) {
        println!("{}", hint);
    }
    let large_project_mode_choice =
        maybe_choose_large_project_optimization_mode(source.len(), low_memory);
    let _large_project_mode_guard = LargeProjectModeOverrideGuard::new(
        set_large_project_mode_override(large_project_mode_choice),
    );
    let contract_checks_enabled = resolve_contract_checks_enabled(contract_checks, opt_level);
    let _contract_checks_override_guard = ContractChecksOverrideGuard::new(
        set_contract_runtime_checks_override(Some(contract_checks_enabled)),
    );
    println!(
        "contract runtime checks: {} (mode={})",
        if contract_checks_enabled {
            "enabled"
        } else {
            "disabled"
        },
        contract_checks_mode_label(contract_checks)
    );
    let collect_symbol_fingerprints =
        should_collect_symbol_fingerprints(source.len(), force_rebuild, low_memory);
    if !collect_symbol_fingerprints && !force_rebuild && !low_memory {
        println!(
            "symbol fingerprint collection: skipped for large source ({} bytes > limit {} bytes)",
            source.len(),
            symbol_fingerprint_collection_limit_bytes()
        );
    }

    let cache_path = build_dir.join(format!("{}.build-cache.json", stem));
    let frontend_session_path = frontend_session_store_path(&build_dir, &stem);
    let generic_cache_path = generic_instance_cache_path(&build_dir, &stem);
    let effective_frontend_jobs = if low_memory {
        FrontendJobs::Fixed(1)
    } else {
        frontend_jobs
    };
    let previous_build_metadata_seed = if low_memory {
        None
    } else {
        load_build_cache(&cache_path)
    };
    let previous_frontend_session = if low_memory {
        None
    } else {
        load_frontend_session_store(&frontend_session_path)
    };
    let previous_generic_cache_seed = if low_memory {
        None
    } else {
        load_generic_instance_cache(&generic_cache_path)
    };
    let probe_mode = if force_rebuild || low_memory {
        FrontendProbeMode::FastNoVerify
    } else {
        FrontendProbeMode::VerifyChangedAndDependents
    };
    if low_memory {
        println!("low-memory mode: enabled (--low-memory)");
    }
    let graph_snapshot = collect_module_graph_snapshot(
        input_path,
        &root_source,
        previous_build_metadata_seed
            .as_ref()
            .and_then(|metadata| metadata.build_graph_v2.as_ref()),
        previous_frontend_session.as_ref(),
        probe_mode,
        effective_frontend_jobs,
        frontend_trace,
        collect_symbol_fingerprints,
    );
    let (root_interface_hash, root_implementation_hash) = resolve_root_hashes_for_request(
        &source,
        previous_build_metadata_seed
            .as_ref()
            .map(|metadata| metadata.root_implementation_hash),
        previous_build_metadata_seed
            .as_ref()
            .map(|metadata| metadata.root_interface_hash),
    );
    let source_hash = root_implementation_hash;
    let reflection = resolve_reflection_options_for_snapshot(reflection, &graph_snapshot);
    println!("{}", reflection_mode_note(&reflection, &graph_snapshot));
    let module_fingerprints = graph_snapshot.module_fingerprints.clone();
    if !graph_snapshot.diagnostics.is_empty() {
        println!("frontend probe diagnostics (stable order):");
        for line in &graph_snapshot.diagnostics {
            println!("  - {}", line);
        }
    }
    println!(
        "frontend session: reused_modules={} rebuilt_modules={}",
        graph_snapshot.reused_modules.len(),
        graph_snapshot.rebuilt_modules.len()
    );
    println!(
        "frontend scheduler: requested={} selected={} serial={} parse_tasks={} body_tasks={} queue_wait_avg_ms={:.3} util={:.2}%",
        graph_snapshot.frontend_scheduler.requested_jobs,
        graph_snapshot.frontend_scheduler.selected_jobs,
        graph_snapshot.frontend_scheduler.serial_mode,
        graph_snapshot.frontend_scheduler.parse_interface_task_count,
        graph_snapshot.frontend_scheduler.body_hir_task_count,
        graph_snapshot.frontend_scheduler.queue_wait_avg_ms,
        graph_snapshot.frontend_scheduler.worker_utilization_pct
    );
    if !graph_snapshot.fallback_events.is_empty() {
        println!("frontend fallback events:");
        for event in &graph_snapshot.fallback_events {
            println!(
                "  - stage={} scope={} reason={}",
                event.stage,
                frontend_fallback_scope_label(event.scope),
                event.reason
            );
        }
    }
    if frontend_trace && !graph_snapshot.planner_trace.is_empty() {
        println!("frontend planner trace (deterministic):");
        for line in &graph_snapshot.planner_trace {
            println!("  - {}", line);
        }
    }
    if !low_memory {
        if let Err(err) = save_frontend_session_store(
            &frontend_session_path,
            &graph_snapshot.frontend_session_store,
        ) {
            println!("frontend session fallback: {}", err);
        }
    }
    let runtime_c = find_runtime_c();
    let runtime_c_fingerprint = optional_runtime_bundle_fingerprint(runtime_c.as_deref())?;

    let output_file = if let Some(out) = output {
        out.to_string()
    } else if emit_llvm {
        build_dir
            .join(format!("{}.ll", stem))
            .to_string_lossy()
            .to_string()
    } else {
        build_dir
            .join(build_target.default_output_basename(&stem))
            .to_string_lossy()
            .to_string()
    };

    let llvm_ir_path = if emit_llvm {
        PathBuf::from(&output_file)
    } else {
        build_dir.join(format!("{}.ll", stem))
    };
    let object_path = if emit_llvm {
        None
    } else {
        Some(build_dir.join(format!("{}.{}", stem, build_target.object_extension())))
    };
    let graph_v2 = if low_memory {
        crate::graph_builder::build_graph_v2_for_source(
            input_path,
            &module_fingerprints,
            &graph_snapshot.dependency_edges,
            object_path.as_deref(),
            root_interface_hash,
            root_implementation_hash,
        )
    } else {
        build_graph_v2_with_function_fingerprints_for_source(
            input_path,
            &module_fingerprints,
            &graph_snapshot.module_function_fingerprints,
            &graph_snapshot.module_generic_items,
            &graph_snapshot.module_generic_instances,
            &graph_snapshot.dependency_edges,
            object_path.as_deref(),
            root_interface_hash,
            root_implementation_hash,
        )
    };
    let generic_feature_flags = vec![
        format!("emit_llvm={}", emit_llvm),
        format!("low_memory={}", low_memory),
        format!("reflection={}", reflection.enabled),
        format!("contract_checks={}", contract_checks_enabled),
        format!("debug_info={}", debug_info),
    ];
    let (generic_plan_stats, next_generic_cache) = derive_generic_instance_plan(
        previous_generic_cache_seed.as_ref(),
        &graph_v2,
        opt_level,
        &generic_feature_flags,
    );
    println!(
        "generic instance cache: total={} hits={} rebuilt={} hit_ratio={:.2} interface_invalidated={} body_invalidated={} dependency_invalidated={} new_instances={}",
        generic_plan_stats.total_instances,
        generic_plan_stats.cache_hits,
        generic_plan_stats.rebuilt_instances,
        generic_instance_hit_ratio(&generic_plan_stats),
        generic_plan_stats.interface_invalidated,
        generic_plan_stats.body_invalidated,
        generic_plan_stats.dependency_invalidated,
        generic_plan_stats.new_instances
    );
    drop(graph_snapshot);
    let key = build_cache_key(
        source_hash,
        module_fingerprints.clone(),
        opt_level,
        contract_checks_enabled,
        debug_info,
        emit_llvm,
        RuntimeSourceIdentity::new(runtime_c.clone(), runtime_c_fingerprint),
        output_file.clone(),
    );
    let mut edit_impact: Option<EditImpact> = None;

    let previous_build_metadata = if low_memory {
        println!("build cache bypassed: --low-memory");
        None
    } else if force_rebuild {
        println!("build cache bypassed: --force-rebuild");
        None
    } else if let Some(metadata) = previous_build_metadata_seed.clone() {
        if build_metadata_matches(&metadata, &key) {
            if build_artifact_exists(&metadata) {
                println!(
                    "build cache hit (opt=O{}, emit_llvm={})",
                    metadata.opt_level, metadata.emit_llvm
                );
                maybe_emit_reflection_sidecar(
                    Path::new(&metadata.output_path),
                    &graph_v2,
                    &reflection,
                    Some(Path::new(&metadata.llvm_ir_path)),
                )?;
                println!("Build output: {}", metadata.output_path);
                return Ok(());
            }
            println!("build cache miss: cached artifacts are missing");
        } else {
            println!("build cache miss: metadata changed");
            for reason in build_cache_mismatch_reasons(&metadata, &key) {
                println!("  - {}", reason);
            }
            let impact = classify_edit_impact(
                metadata.root_interface_hash,
                metadata.root_implementation_hash,
                root_interface_hash,
                root_implementation_hash,
                &metadata.module_fingerprints,
                &module_fingerprints,
                metadata.build_graph_v2.as_ref(),
                &graph_v2,
            );
            for line in format_edit_impact_lines(&impact) {
                println!("  - {}", line);
            }
            edit_impact = Some(impact);
        }
        Some(metadata)
    } else {
        println!(
            "build cache miss: no cache metadata at {}",
            cache_path.to_string_lossy()
        );
        None
    };

    let mut workset_plan = derive_build_workset_plan(
        previous_build_metadata.as_ref(),
        edit_impact.as_ref(),
        &graph_v2.root_module,
        emit_llvm,
        opt_level,
        contract_checks_enabled,
        debug_info,
        &output_file,
        runtime_c.as_deref(),
    );
    if previous_build_metadata.is_some()
        && can_skip_codegen_via_generic_cache(edit_impact.as_ref(), &graph_v2, &generic_plan_stats)
    {
        println!(
            "generic workset optimization: all impacted generic instances are cache hits, skipping MIR/codegen"
        );
        workset_plan = BuildWorksetPlan::ReusePreviousArtifacts;
    } else if previous_build_metadata.is_some()
        && can_reuse_artifacts_for_unreachable_impl_only_changes(
            edit_impact.as_ref(),
            &graph_v2,
            large_project_mode_choice,
        )
    {
        println!(
            "workset optimization: impl-only changes are outside root reachable entry set; reusing previous artifacts"
        );
        workset_plan = BuildWorksetPlan::ReusePreviousArtifacts;
    }
    let workset_manifest = derive_codegen_workset_manifest(
        &graph_v2,
        edit_impact.as_ref(),
        workset_plan,
        Some(&generic_plan_stats),
    );
    let build_workset_manifest_path = codegen_workset_manifest_path(&build_dir, &stem, "build");
    save_codegen_workset_manifest(&build_workset_manifest_path, &workset_manifest)?;
    println!(
        "codegen workset: rebuild_modules={} reuse_modules={} rebuild_symbols={} reuse_symbols={}",
        workset_manifest.rebuild_modules.len(),
        workset_manifest.reuse_modules.len(),
        workset_manifest.rebuild_symbols.len(),
        workset_manifest.reuse_symbols.len(),
    );
    println!(
        "codegen workset manifest: {}",
        build_workset_manifest_path.to_string_lossy()
    );
    match workset_plan {
        BuildWorksetPlan::ReusePreviousArtifacts => {
            if let Some(previous) = previous_build_metadata.as_ref() {
                if build_artifact_exists(previous) {
                    let class_label = edit_impact
                        .as_ref()
                        .map(|impact| edit_class_label(impact.class))
                        .unwrap_or("unknown");
                    println!(
                        "build workset plan: reuse previous artifacts ({})",
                        class_label
                    );
                    maybe_emit_reflection_sidecar(
                        Path::new(&previous.output_path),
                        &graph_v2,
                        &reflection,
                        Some(Path::new(&previous.llvm_ir_path)),
                    )?;
                    println!("Build output: {}", previous.output_path);
                    return Ok(());
                }
                if !emit_llvm {
                    let expected_object_path = object_path.as_deref().ok_or_else(|| {
                        miette::miette!("internal error: missing object path for native build")
                    })?;
                    let previous_object_path = previous.object_path.as_deref();
                    if let Some(previous_object_path) = previous_object_path {
                        if canonical_or_lossy(Path::new(previous_object_path))
                            == canonical_or_lossy(expected_object_path)
                        {
                            if let Some(clang_exe) = find_clang() {
                                let output_path = Path::new(&output_file);
                                match recover_native_output_from_cached_artifacts(
                                    &clang_exe,
                                    Path::new(&previous.llvm_ir_path),
                                    expected_object_path,
                                    output_path,
                                    runtime_c.as_deref(),
                                    opt_level,
                                    Some(&native_link_libraries),
                                ) {
                                    Ok(recovery) => {
                                        let label = match recovery {
                                            CachedNativeRecoveryPlan::RelinkFromObject => {
                                                "relinked cached object"
                                            }
                                            CachedNativeRecoveryPlan::RebuildObjectFromCachedIr => {
                                                "rebuilt object from cached LLVM IR and relinked"
                                            }
                                        };
                                        println!("build workset plan: {}", label);
                                        println!("Build output: {}", output_file);
                                        return Ok(());
                                    }
                                    Err(err) => {
                                        println!("build workset fallback: {}", err);
                                    }
                                }
                            } else {
                                println!(
                                    "build workset fallback: clang unavailable for cached relink"
                                );
                            }
                        } else {
                            println!("build workset fallback: cached object path changed");
                        }
                    } else {
                        println!("build workset fallback: cached object path missing");
                    }
                } else {
                    println!("build workset fallback: previous artifacts are missing");
                }
            }
        }
        BuildWorksetPlan::RebuildImpactedRoot => {
            println!("build workset plan: rebuild impacted root module");
        }
        BuildWorksetPlan::FullRebuild => {
            if edit_impact.is_some() {
                println!("build workset plan: full rebuild");
            }
        }
    }

    let (phases, effective_memory_mode) = compile_source_to_llvm_file_with_phase_timings_with_mode(
        &source,
        opt_level,
        &llvm_ir_path,
        if low_memory {
            Some(FrontendMemoryMode::LowMemory)
        } else {
            None
        },
        None,
        Some(&build_target.triple),
        debug_info.then(|| {
            DebugInfoConfig::for_source(
                input_path.to_string_lossy().replace('\\', "/"),
                root_source.clone(),
            )
        }),
    )
    .map_err(|e| {
        emit_compile_error(Some(input), &e.to_string());
        miette::miette!("compile failed")
    })?;
    crate::maybe_print_phase_timings(&phases);
    if let Some(path) = timings_json {
        write_timings_json_v1(Path::new(path), &phases)?;
    }
    println!(
        "frontend memory mode: {}",
        frontend_memory_mode_label(effective_memory_mode)
    );
    drop(source);
    let llvm_ir_hash = file_fingerprint(&llvm_ir_path)?;

    if emit_llvm {
        maybe_emit_reflection_sidecar(
            Path::new(&output_file),
            &graph_v2,
            &reflection,
            Some(&llvm_ir_path),
        )?;
        let metadata = BuildCacheMetadata {
            cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            source_hash,
            root_interface_hash,
            root_implementation_hash,
            module_fingerprints,
            opt_level,
            contract_checks: contract_checks_enabled,
            debug_info,
            emit_llvm: true,
            runtime_c,
            runtime_c_fingerprint,
            llvm_ir_path: llvm_ir_path.to_string_lossy().to_string(),
            output_path: output_file.clone(),
            llvm_ir_hash,
            object_path: None,
            build_graph_v2: Some(graph_v2),
        };
        save_build_cache(&cache_path, &metadata)?;
        if !low_memory {
            if let Err(err) = save_generic_instance_cache(&generic_cache_path, &next_generic_cache)
            {
                println!("generic instance cache fallback: {}", err);
            }
        }
        println!("LLVM IR written to {}", output_file);
        return Ok(());
    }

    let clang_exe = find_clang().ok_or_else(|| {
        miette::miette!(
            "clang is required to build native binaries. Install LLVM/Clang or use --emit-llvm"
        )
    })?;
    ensure_supported_clang_toolchain(&clang_exe)?;
    let object_path = object_path
        .clone()
        .ok_or_else(|| miette::miette!("internal error: missing object path"))?;
    let output_path = Path::new(&output_file);

    let incremental_mode = incremental_link_mode_from_env();
    if matches!(incremental_mode, IncrementalLinkMode::Off) {
        println!("incremental link disabled: SENGOO_INCREMENTAL_LINK=off");
    }
    let incremental_check = if matches!(incremental_mode, IncrementalLinkMode::Off) {
        None
    } else {
        previous_build_metadata.as_ref().map(|previous| {
            can_use_incremental_link_with_metadata(
                previous,
                llvm_ir_hash,
                &object_path,
                &output_file,
                runtime_c.as_deref(),
                opt_level,
                contract_checks_enabled,
                debug_info,
                &graph_v2,
            )
        })
    };

    match incremental_check {
        Some(Ok(())) => {
            println!(
                "incremental link: reusing object {}",
                object_path.to_string_lossy()
            );
        }
        Some(Err(reason)) => {
            println!("incremental link fallback: {}", reason);
            if let Some(previous) = previous_build_metadata.as_ref() {
                let impact = classify_edit_impact(
                    previous.root_interface_hash,
                    previous.root_implementation_hash,
                    root_interface_hash,
                    root_implementation_hash,
                    &previous.module_fingerprints,
                    &module_fingerprints,
                    previous.build_graph_v2.as_ref(),
                    &graph_v2,
                );
                for line in format_edit_impact_lines(&impact) {
                    println!("  - {}", line);
                }
            }
            compile_ir_to_object(
                &clang_exe,
                &llvm_ir_path,
                &object_path,
                opt_level,
                Some(&build_target),
                debug_info,
            )?;
        }
        None => {
            compile_ir_to_object(
                &clang_exe,
                &llvm_ir_path,
                &object_path,
                opt_level,
                Some(&build_target),
                debug_info,
            )?;
        }
    }

    let mut object_paths = vec![object_path.clone()];
    append_native_runtime_inputs(
        &clang_exe,
        &mut object_paths,
        runtime_c.as_deref(),
        opt_level,
        Some(&build_target),
    )?;
    append_package_native_inputs(
        &clang_exe,
        &mut object_paths,
        input_path,
        &root_source,
        &build_dir,
        opt_level,
        Some(&build_target),
    )?;
    link_native_binary_from_objects(
        &clang_exe,
        &object_paths,
        output_path,
        Some(&build_target),
        Some(&native_link_libraries),
    )?;
    maybe_emit_reflection_sidecar(
        Path::new(&output_file),
        &graph_v2,
        &reflection,
        Some(&llvm_ir_path),
    )?;

    let metadata = BuildCacheMetadata {
        cache_schema_version: BUILD_GRAPH_SCHEMA_VERSION,
        source_hash,
        root_interface_hash,
        root_implementation_hash,
        module_fingerprints,
        opt_level,
        contract_checks: contract_checks_enabled,
        debug_info,
        emit_llvm: false,
        runtime_c,
        runtime_c_fingerprint,
        llvm_ir_path: llvm_ir_path.to_string_lossy().to_string(),
        output_path: output_file.clone(),
        llvm_ir_hash,
        object_path: Some(object_path.to_string_lossy().to_string()),
        build_graph_v2: Some(graph_v2),
    };
    save_build_cache(&cache_path, &metadata)?;
    if !low_memory {
        if let Err(err) = save_generic_instance_cache(&generic_cache_path, &next_generic_cache) {
            println!("generic instance cache fallback: {}", err);
        }
    }

    println!("Build output: {}", output_file);
    Ok(())
}
