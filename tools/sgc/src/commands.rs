use crate::*;
use miette::{IntoDiagnostic, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub(crate) async fn cmd_build(
    input: &str,
    output: Option<&str>,
    opt_level: u8,
    emit_llvm: bool,
    force_rebuild: bool,
    low_memory: bool,
    frontend_jobs: FrontendJobs,
    frontend_trace: bool,
    reflection: ReflectionCliOptions,
) -> Result<()> {
    println!("Building: {}", input);

    let input_path = Path::new(input);
    let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
    let source_dir = input_path.parent().unwrap_or(Path::new("."));
    let build_dir = source_dir.join("build");
    fs::create_dir_all(&build_dir).into_diagnostic()?;

    let source = fs::read_to_string(input)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to read source {}: {}", input, e))?;
    if let Some(hint) = maybe_low_memory_mode_hint(source.len(), low_memory) {
        println!("{}", hint);
    }

    let cache_path = build_dir.join(format!("{}.build-cache.json", stem));
    let frontend_session_path = frontend_session_store_path(&build_dir, &stem);
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
        &source,
        previous_build_metadata_seed
            .as_ref()
            .and_then(|metadata| metadata.build_graph_v2.as_ref()),
        previous_frontend_session.as_ref(),
        probe_mode,
        effective_frontend_jobs,
        frontend_trace,
        !force_rebuild && !low_memory,
    );
    let (root_interface_hash, root_implementation_hash) = resolve_root_hashes_for_request(
        input_path,
        &source,
        &graph_snapshot,
        force_rebuild,
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

    let output_file = if let Some(out) = output {
        out.to_string()
    } else if emit_llvm {
        build_dir
            .join(format!("{}.ll", stem))
            .to_string_lossy()
            .to_string()
    } else {
        let ext = if cfg!(windows) { ".exe" } else { "" };
        build_dir
            .join(format!("{}{}", stem, ext))
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
        Some(build_dir.join(format!("{}.{}", stem, object_file_extension())))
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
            &graph_snapshot.dependency_edges,
            object_path.as_deref(),
            root_interface_hash,
            root_implementation_hash,
        )
    };
    drop(graph_snapshot);
    let key = build_cache_key(
        source_hash,
        module_fingerprints.clone(),
        opt_level,
        emit_llvm,
        runtime_c.clone(),
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

    let workset_plan = derive_build_workset_plan(
        previous_build_metadata.as_ref(),
        edit_impact.as_ref(),
        &graph_v2.root_module,
        emit_llvm,
        opt_level,
        &output_file,
        runtime_c.as_deref(),
    );
    let workset_manifest =
        derive_codegen_workset_manifest(&graph_v2, edit_impact.as_ref(), workset_plan);
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

    let (_phases, effective_memory_mode) =
        compile_source_to_llvm_file_with_phase_timings_with_mode(
            &source,
            opt_level,
            &llvm_ir_path,
            if low_memory {
                Some(FrontendMemoryMode::LowMemory)
            } else {
                None
            },
        )
        .map_err(|e| {
            eprintln!("Compilation error:");
            eprintln!("{}", e);
            miette::miette!("compile failed")
        })?;
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
            emit_llvm: true,
            runtime_c,
            llvm_ir_path: llvm_ir_path.to_string_lossy().to_string(),
            output_path: output_file.clone(),
            llvm_ir_hash,
            object_path: None,
            build_graph_v2: Some(graph_v2),
        };
        save_build_cache(&cache_path, &metadata)?;
        println!("LLVM IR written to {}", output_file);
        return Ok(());
    }

    let clang_exe = find_clang().ok_or_else(|| {
        miette::miette!(
            "clang is required to build native binaries. Install LLVM/Clang or use --emit-llvm"
        )
    })?;
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
            compile_ir_to_object(&clang_exe, &llvm_ir_path, &object_path, opt_level)?;
        }
        None => {
            compile_ir_to_object(&clang_exe, &llvm_ir_path, &object_path, opt_level)?;
        }
    }

    let mut object_paths = vec![object_path.clone()];
    if let Some(runtime_c) = runtime_c.as_deref() {
        object_paths.push(ensure_runtime_object(&clang_exe, runtime_c, opt_level)?);
    }
    link_native_binary_from_objects(&clang_exe, &object_paths, output_path)?;
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
        emit_llvm: false,
        runtime_c,
        llvm_ir_path: llvm_ir_path.to_string_lossy().to_string(),
        output_path: output_file.clone(),
        llvm_ir_hash,
        object_path: Some(object_path.to_string_lossy().to_string()),
        build_graph_v2: Some(graph_v2),
    };
    save_build_cache(&cache_path, &metadata)?;

    println!("Build output: {}", output_file);
    Ok(())
}

pub(crate) async fn cmd_run(
    input: &str,
    opt_level: u8,
    requested_engine: RunEngine,
    force_rebuild: bool,
    _args: &[String],
    low_memory: bool,
    frontend_jobs: FrontendJobs,
    frontend_trace: bool,
    reflection: ReflectionCliOptions,
) -> Result<()> {
    println!("Running: {}", input);

    let input_path = Path::new(input);
    let stem = input_path.file_stem().unwrap_or_default().to_string_lossy();
    let source_dir = input_path.parent().unwrap_or(Path::new("."));
    let build_dir = source_dir.join("build");
    fs::create_dir_all(&build_dir).into_diagnostic()?;

    let llvm_ir_path = build_dir.join(format!("{}.ll", stem));
    let executable_path = if cfg!(windows) {
        build_dir.join(format!("{}.exe", stem))
    } else {
        build_dir.join(stem.to_string())
    };
    let cache_path = build_dir.join(format!("{}.run-cache.json", stem));
    let frontend_session_path = frontend_session_store_path(&build_dir, &stem);
    let effective_frontend_jobs = if low_memory {
        FrontendJobs::Fixed(1)
    } else {
        frontend_jobs
    };
    let previous_run_metadata_seed = if low_memory {
        None
    } else {
        load_run_cache(&cache_path)
    };
    let previous_frontend_session = if low_memory {
        None
    } else {
        load_frontend_session_store(&frontend_session_path)
    };

    let source = fs::read_to_string(input)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to read source {}: {}", input, e))?;
    if let Some(hint) = maybe_low_memory_mode_hint(source.len(), low_memory) {
        println!("{}", hint);
    }
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
        &source,
        previous_run_metadata_seed
            .as_ref()
            .and_then(|metadata| metadata.build_graph_v2.as_ref()),
        previous_frontend_session.as_ref(),
        probe_mode,
        effective_frontend_jobs,
        frontend_trace,
        !force_rebuild && !low_memory,
    );
    let (root_interface_hash, root_implementation_hash) = resolve_root_hashes_for_request(
        input_path,
        &source,
        &graph_snapshot,
        force_rebuild,
        previous_run_metadata_seed
            .as_ref()
            .map(|metadata| metadata.root_implementation_hash),
        previous_run_metadata_seed
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
    let object_path = build_dir.join(format!("{}.{}", stem, object_file_extension()));
    let graph_v2 = if low_memory {
        crate::graph_builder::build_graph_v2_for_source(
            input_path,
            &module_fingerprints,
            &graph_snapshot.dependency_edges,
            Some(&object_path),
            root_interface_hash,
            root_implementation_hash,
        )
    } else {
        build_graph_v2_with_function_fingerprints_for_source(
            input_path,
            &module_fingerprints,
            &graph_snapshot.module_function_fingerprints,
            &graph_snapshot.dependency_edges,
            Some(&object_path),
            root_interface_hash,
            root_implementation_hash,
        )
    };
    drop(graph_snapshot);

    let runtime_c = find_runtime_c();
    let clang_exe = find_clang();
    let lli_exe = find_lli();

    let resolved_engine = resolve_engine(requested_engine, clang_exe.is_some(), lli_exe.is_some())?;

    let key = cache_key(
        source_hash,
        module_fingerprints.clone(),
        opt_level,
        requested_engine,
        resolved_engine,
        runtime_c.clone(),
    );
    let mut edit_impact: Option<EditImpact> = None;

    let previous_run_metadata = if low_memory {
        println!("cache bypassed: --low-memory");
        None
    } else if force_rebuild {
        println!("cache bypassed: --force-rebuild");
        None
    } else if let Some(metadata) = previous_run_metadata_seed.clone() {
        if metadata_matches(&metadata, &key) {
            if artifact_exists(&metadata) {
                println!(
                    "cache hit (engine={:?}, modules={})",
                    metadata.resolved_engine,
                    metadata.module_fingerprints.len()
                );
                return match metadata.resolved_engine {
                    RunEngine::Native => {
                        let exe = metadata.executable_path.as_deref().ok_or_else(|| {
                            miette::miette!("cache corrupted: missing native executable path")
                        })?;
                        maybe_emit_reflection_sidecar(
                            Path::new(exe),
                            &graph_v2,
                            &reflection,
                            Some(Path::new(&metadata.llvm_ir_path)),
                        )?;
                        run_native_binary(Path::new(exe))
                    }
                    RunEngine::Lli => {
                        let lli = lli_exe.as_deref().ok_or_else(|| {
                            miette::miette!("cache hit but lli is unavailable; try --force-rebuild")
                        })?;
                        maybe_emit_reflection_sidecar(
                            Path::new(&metadata.llvm_ir_path),
                            &graph_v2,
                            &reflection,
                            Some(Path::new(&metadata.llvm_ir_path)),
                        )?;
                        run_with_lli(lli, Path::new(&metadata.llvm_ir_path))
                    }
                    RunEngine::Auto => Err(miette::miette!("compile failed")),
                };
            } else {
                println!("cache miss: cached artifacts are missing");
            }
        } else {
            println!("cache miss: metadata changed");
            for reason in cache_mismatch_reasons(&metadata, &key) {
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
            "cache miss: no cache metadata at {}",
            cache_path.to_string_lossy()
        );
        None
    };

    let workset_plan = derive_run_workset_plan(
        previous_run_metadata.as_ref(),
        edit_impact.as_ref(),
        &graph_v2.root_module,
        opt_level,
        requested_engine,
        resolved_engine,
        runtime_c.as_deref(),
    );
    let workset_manifest =
        derive_codegen_workset_manifest(&graph_v2, edit_impact.as_ref(), workset_plan);
    let run_workset_manifest_path = codegen_workset_manifest_path(&build_dir, &stem, "run");
    save_codegen_workset_manifest(&run_workset_manifest_path, &workset_manifest)?;
    println!(
        "codegen workset: rebuild_modules={} reuse_modules={} rebuild_symbols={} reuse_symbols={}",
        workset_manifest.rebuild_modules.len(),
        workset_manifest.reuse_modules.len(),
        workset_manifest.rebuild_symbols.len(),
        workset_manifest.reuse_symbols.len(),
    );
    println!(
        "codegen workset manifest: {}",
        run_workset_manifest_path.to_string_lossy()
    );
    if let BuildWorksetPlan::ReusePreviousArtifacts = workset_plan {
        if let Some(previous) = previous_run_metadata.as_ref() {
            if artifact_exists(previous) {
                let class_label = edit_impact
                    .as_ref()
                    .map(|impact| edit_class_label(impact.class))
                    .unwrap_or("unknown");
                println!(
                    "run workset plan: reuse previous artifacts ({})",
                    class_label
                );
                return match previous.resolved_engine {
                    RunEngine::Native => {
                        let exe = previous.executable_path.as_deref().ok_or_else(|| {
                            miette::miette!("cache corrupted: missing native executable path")
                        })?;
                        run_native_binary(Path::new(exe))
                    }
                    RunEngine::Lli => {
                        let lli = lli_exe.as_deref().ok_or_else(|| {
                            miette::miette!("cache hit but lli is unavailable; try --force-rebuild")
                        })?;
                        run_with_lli(lli, Path::new(&previous.llvm_ir_path))
                    }
                    RunEngine::Auto => Err(miette::miette!("compile failed")),
                };
            }
            if matches!(resolved_engine, RunEngine::Native)
                && matches!(previous.resolved_engine, RunEngine::Native)
            {
                if let Some(previous_object_path) = previous.object_path.as_deref() {
                    if canonical_or_lossy(Path::new(previous_object_path))
                        == canonical_or_lossy(&object_path)
                    {
                        if let Some(clang) = clang_exe.as_deref() {
                            match recover_native_output_from_cached_artifacts(
                                clang,
                                Path::new(&previous.llvm_ir_path),
                                &object_path,
                                &executable_path,
                                runtime_c.as_deref(),
                                opt_level,
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
                                    println!("run workset plan: {}", label);
                                    return run_native_binary(&executable_path);
                                }
                                Err(err) => {
                                    println!("run workset fallback: {}", err);
                                }
                            }
                        } else {
                            println!("run workset fallback: clang unavailable for cached relink");
                        }
                    } else {
                        println!("run workset fallback: cached object path changed");
                    }
                } else {
                    println!("run workset fallback: cached object path missing");
                }
            } else {
                println!("run workset fallback: previous artifacts are missing");
            }
        }
    } else if matches!(workset_plan, BuildWorksetPlan::RebuildImpactedRoot) {
        println!("run workset plan: rebuild impacted root module");
    } else if edit_impact.is_some() {
        println!("run workset plan: full rebuild");
    }

    let (_phases, effective_memory_mode) =
        compile_source_to_llvm_file_with_phase_timings_with_mode(
            &source,
            opt_level,
            &llvm_ir_path,
            if low_memory {
                Some(FrontendMemoryMode::LowMemory)
            } else {
                None
            },
        )
        .map_err(|e| {
            eprintln!("Compilation error:");
            eprintln!("{}", e);
            miette::miette!("compile failed")
        })?;
    println!(
        "frontend memory mode: {}",
        frontend_memory_mode_label(effective_memory_mode)
    );
    drop(source);
    let llvm_ir_hash = file_fingerprint(&llvm_ir_path)?;

    match resolved_engine {
        RunEngine::Native => {
            let clang = clang_exe
                .as_deref()
                .ok_or_else(|| miette::miette!("clang is required for --engine native"))?;

            let incremental_mode = incremental_link_mode_from_env();
            if matches!(incremental_mode, IncrementalLinkMode::Off) {
                println!("incremental link disabled: SENGOO_INCREMENTAL_LINK=off");
            }
            let incremental_check = if matches!(incremental_mode, IncrementalLinkMode::Off) {
                None
            } else {
                previous_run_metadata.as_ref().map(|previous| {
                    can_use_incremental_link_with_run_metadata(
                        previous,
                        llvm_ir_hash,
                        &object_path,
                        runtime_c.as_deref(),
                        opt_level,
                        requested_engine,
                        resolved_engine,
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
                    if let Some(previous) = previous_run_metadata.as_ref() {
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
                    compile_ir_to_object(clang, &llvm_ir_path, &object_path, opt_level)?;
                }
                None => {
                    compile_ir_to_object(clang, &llvm_ir_path, &object_path, opt_level)?;
                }
            }

            let mut object_paths = vec![object_path.clone()];
            if let Some(runtime_c) = runtime_c.as_deref() {
                object_paths.push(ensure_runtime_object(clang, runtime_c, opt_level)?);
            }
            link_native_binary_from_objects(clang, &object_paths, &executable_path)?;
            run_native_binary(&executable_path)?;
        }
        RunEngine::Lli => {
            let lli = lli_exe
                .as_deref()
                .ok_or_else(|| miette::miette!("lli is required for --engine lli"))?;
            run_with_lli(lli, &llvm_ir_path)?;
        }
        RunEngine::Auto => {
            return Err(miette::miette!(
                "internal error: resolved_engine should not be auto"
            ))
        }
    }
    let reflection_artifact_path = match resolved_engine {
        RunEngine::Native => executable_path.as_path(),
        RunEngine::Lli => llvm_ir_path.as_path(),
        RunEngine::Auto => {
            return Err(miette::miette!(
                "internal error: resolved_engine should not be auto"
            ))
        }
    };
    maybe_emit_reflection_sidecar(
        reflection_artifact_path,
        &graph_v2,
        &reflection,
        Some(&llvm_ir_path),
    )?;
    let metadata = RunCacheMetadata {
        source_hash,
        root_interface_hash,
        root_implementation_hash,
        module_fingerprints,
        opt_level,
        requested_engine,
        resolved_engine,
        runtime_c,
        llvm_ir_path: llvm_ir_path.to_string_lossy().to_string(),
        executable_path: if matches!(resolved_engine, RunEngine::Native) {
            Some(executable_path.to_string_lossy().to_string())
        } else {
            None
        },
        llvm_ir_hash,
        object_path: if matches!(resolved_engine, RunEngine::Native) {
            Some(object_path.to_string_lossy().to_string())
        } else {
            None
        },
        build_graph_v2: Some(graph_v2),
    };
    save_run_cache(&cache_path, &metadata)?;

    Ok(())
}
