use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use crate::{
    canonical_or_lossy, collect_impl_only_impacted_symbols_with_fallback,
    collect_module_sources_with_edges, dependency_graph_digest, frontend_cache_entry_for_module,
    frontend_fallback_scope_label, frontend_jobs_label, frontend_probe_module_body_only,
    frontend_probe_module_full, function_fingerprints_for_module, generic_fingerprints_for_module,
    hir_fragment_fingerprint, merge_frontend_phase_stats, module_dependency_levels,
    resolve_frontend_job_count, run_frontend_tasks_deterministic, source_fingerprint, BuildGraphV2,
    FrontendFallbackEvent, FrontendFallbackScope, FrontendJobs, FrontendModuleCacheEntryV4,
    FrontendProbeMode, FrontendSchedulerPhaseStats, FrontendSchedulerTelemetry,
    FrontendSessionStoreV4, ModuleFingerprint, ModuleGraphSnapshot, BUILD_GRAPH_SCHEMA_VERSION,
    FRONTEND_SCHEDULER_SCHEMA_VERSION,
};

pub(crate) fn collect_module_graph_snapshot(
    input_path: &Path,
    source: &str,
    previous_graph: Option<&BuildGraphV2>,
    previous_frontend_session: Option<&FrontendSessionStoreV4>,
    probe_mode: FrontendProbeMode,
    frontend_jobs: FrontendJobs,
    trace_mode: bool,
    collect_symbol_fingerprints: bool,
) -> ModuleGraphSnapshot {
    let root_module = canonical_or_lossy(input_path);
    let module_sources = collect_module_sources_with_edges(input_path, source);
    let mut reflection_import_modules = module_sources
        .iter()
        .filter_map(|(path, info)| {
            if info.requests_reflection {
                Some(path.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    reflection_import_modules.sort();
    reflection_import_modules.dedup();

    let mut dependency_edges = module_sources
        .iter()
        .map(|(path, info)| (path.clone(), info.depends_on.clone()))
        .collect::<BTreeMap<_, _>>();
    dependency_edges.entry(root_module.clone()).or_default();
    let dependency_digest = dependency_graph_digest(&dependency_edges);

    let mut diagnostics = Vec::new();
    let mut planner_trace = Vec::new();
    let mut fallback_events = Vec::new();
    let mut previous_entry_by_module = HashMap::<String, FrontendModuleCacheEntryV4>::new();

    if trace_mode {
        planner_trace.push(format!(
            "frontend planner: root={} modules={} probe={:?} requested_jobs={} collect_symbols={}",
            root_module,
            module_sources.len(),
            probe_mode,
            frontend_jobs_label(frontend_jobs),
            collect_symbol_fingerprints
        ));
    }

    if let Some(previous) = previous_frontend_session {
        if previous.schema_version != BUILD_GRAPH_SCHEMA_VERSION {
            let reason = format!(
                "schema mismatch ({} -> {})",
                previous.schema_version, BUILD_GRAPH_SCHEMA_VERSION
            );
            diagnostics.push(format!("frontend session fallback: {}", reason));
            fallback_events.push(FrontendFallbackEvent {
                stage: "session_load".to_string(),
                scope: FrontendFallbackScope::FullFrontend,
                reason,
            });
        } else if previous.scheduler_schema_version != FRONTEND_SCHEDULER_SCHEMA_VERSION {
            let reason = format!(
                "scheduler schema mismatch ({} -> {})",
                previous.scheduler_schema_version, FRONTEND_SCHEDULER_SCHEMA_VERSION
            );
            diagnostics.push(format!("frontend session fallback: {}", reason));
            fallback_events.push(FrontendFallbackEvent {
                stage: "session_load".to_string(),
                scope: FrontendFallbackScope::FullFrontend,
                reason,
            });
        } else if previous.dependency_graph_digest != dependency_digest {
            let reason = "dependency digest mismatch".to_string();
            diagnostics.push(format!("frontend session fallback: {}", reason));
            fallback_events.push(FrontendFallbackEvent {
                stage: "session_load".to_string(),
                scope: FrontendFallbackScope::FullFrontend,
                reason,
            });
        } else if previous.compiler_version != env!("CARGO_PKG_VERSION") {
            let reason = format!(
                "compiler version mismatch ({} -> {})",
                previous.compiler_version,
                env!("CARGO_PKG_VERSION")
            );
            diagnostics.push(format!("frontend session fallback: {}", reason));
            fallback_events.push(FrontendFallbackEvent {
                stage: "session_load".to_string(),
                scope: FrontendFallbackScope::FullFrontend,
                reason,
            });
        } else if previous.root_module != root_module {
            let reason = "root module changed".to_string();
            diagnostics.push(format!("frontend session fallback: {}", reason));
            fallback_events.push(FrontendFallbackEvent {
                stage: "session_load".to_string(),
                scope: FrontendFallbackScope::FullFrontend,
                reason,
            });
        } else {
            for module in &previous.modules {
                previous_entry_by_module.insert(module.module_id.clone(), module.clone());
            }
        }
    }

    let mut module_entries = BTreeMap::<String, FrontendModuleCacheEntryV4>::new();
    let mut reused_modules = Vec::new();
    let mut rebuilt_modules = Vec::new();
    let mut parse_phase_stats = FrontendSchedulerPhaseStats::default();
    let mut body_hir_phase_stats = FrontendSchedulerPhaseStats::default();
    let mut utilization_denominator_ms = 0.0;

    let selected_jobs = resolve_frontend_job_count(frontend_jobs, module_sources.len());
    if trace_mode {
        planner_trace.push(format!(
            "frontend scheduler: selected_jobs={} serial_mode={}",
            selected_jobs,
            selected_jobs == 1
        ));
    }

    let module_levels = module_dependency_levels(&dependency_edges);
    if trace_mode {
        planner_trace.push(format!(
            "frontend scheduler: module_levels={}",
            module_levels.len()
        ));
    }

    for level in module_levels {
        let mut level_rebuild = Vec::<String>::new();
        for module in level {
            let Some(info) = module_sources.get(&module) else {
                continue;
            };
            let mut expected_depends_on = info.depends_on.clone();
            expected_depends_on.sort();
            expected_depends_on.dedup();
            let source_hash = source_fingerprint(info.source.as_ref());
            let reused = previous_entry_by_module.get(&module).filter(|previous| {
                previous.source_hash == source_hash
                    && previous.depends_on == expected_depends_on
                    && previous.dependency_digest == dependency_digest
                    && previous.scheduler_schema_version == FRONTEND_SCHEDULER_SCHEMA_VERSION
            });

            if let Some(previous) = reused {
                module_entries.insert(module.clone(), previous.clone());
                reused_modules.push(module.clone());
            } else {
                level_rebuild.push(module.clone());
            }
        }

        if level_rebuild.is_empty() {
            continue;
        }

        let (rebuilt, level_stats) = run_frontend_tasks_deterministic(
            "parse_interface",
            level_rebuild,
            selected_jobs,
            trace_mode,
            &mut planner_trace,
            |module_id| {
                let info = module_sources
                    .get(module_id)
                    .expect("module must exist during parse/interface scheduling");
                frontend_cache_entry_for_module(
                    module_id,
                    info,
                    dependency_digest,
                    collect_symbol_fingerprints,
                )
            },
        );
        merge_frontend_phase_stats(&mut parse_phase_stats, level_stats);
        utilization_denominator_ms += level_stats.wall_ms * level_stats.worker_count as f64;

        for (module_id, entry) in rebuilt {
            module_entries.insert(module_id.clone(), entry);
            rebuilt_modules.push(module_id);
        }
    }

    reused_modules.sort();
    reused_modules.dedup();
    rebuilt_modules.sort();
    rebuilt_modules.dedup();

    let mut symbol_backfilled_modules = 0usize;
    if collect_symbol_fingerprints {
        for (module_id, entry) in &mut module_entries {
            let needs_symbols = entry.symbols.is_empty();
            let needs_generic =
                entry.generic_items.is_empty() && entry.generic_instances.is_empty();
            if !needs_symbols && !needs_generic {
                continue;
            }
            let Some(info) = module_sources.get(module_id) else {
                continue;
            };
            if needs_symbols {
                entry.symbols = function_fingerprints_for_module(module_id, info.source.as_ref());
                for symbol in &mut entry.symbols {
                    if symbol.module_imports.is_empty() {
                        symbol.module_imports = entry.depends_on.clone();
                    }
                }
                entry.symbols.sort_by(|a, b| a.symbol.cmp(&b.symbol));
                entry.hir_hash = hir_fragment_fingerprint(&entry.symbols);
            }
            if needs_generic {
                let (mut generic_items, mut generic_instances) =
                    generic_fingerprints_for_module(module_id, info.source.as_ref());
                generic_items.sort_by(|a, b| a.stable_item_id.cmp(&b.stable_item_id));
                generic_items.dedup_by(|a, b| a.stable_item_id == b.stable_item_id);
                generic_instances.sort_by(|a, b| a.instance_key.cmp(&b.instance_key));
                generic_instances.dedup_by(|a, b| a.instance_key == b.instance_key);
                entry.generic_items = generic_items;
                entry.generic_instances = generic_instances;
            }
            symbol_backfilled_modules += 1;
        }
    }

    if trace_mode {
        planner_trace.push(format!(
            "frontend planner: reused_modules={} rebuilt_modules={}",
            reused_modules.len(),
            rebuilt_modules.len()
        ));
        planner_trace.push(format!(
            "frontend planner: symbol_backfilled_modules={}",
            symbol_backfilled_modules
        ));
    }

    let (verify_full_modules, mut verify_body_symbols) = match probe_mode {
        FrontendProbeMode::FastNoVerify => (HashSet::new(), BTreeMap::<String, Vec<String>>::new()),
        FrontendProbeMode::VerifyAll => (
            module_sources.keys().cloned().collect::<HashSet<_>>(),
            BTreeMap::<String, Vec<String>>::new(),
        ),
        FrontendProbeMode::VerifyChangedAndDependents => {
            let mut full_modules = HashSet::<String>::new();
            let mut body_symbols = BTreeMap::<String, Vec<String>>::new();
            let mut queue = Vec::<String>::new();

            for module in &rebuilt_modules {
                let Some(current) = module_entries.get(module) else {
                    continue;
                };
                let previous = previous_entry_by_module.get(module);
                let interface_changed = previous
                    .map(|entry| entry.interface_hash != current.interface_hash)
                    .unwrap_or(true);

                if interface_changed {
                    if full_modules.insert(module.clone()) {
                        queue.push(module.clone());
                    }
                    continue;
                }

                if let Some(previous) = previous {
                    let (impacted, fallback_reason) =
                        collect_impl_only_impacted_symbols_with_fallback(
                            &previous.symbols,
                            &current.symbols,
                        );
                    if !impacted.is_empty() {
                        body_symbols.insert(module.clone(), impacted);
                    }
                    if let Some(reason) = fallback_reason {
                        fallback_events.push(FrontendFallbackEvent {
                            stage: "impl_only_invalidation".to_string(),
                            scope: FrontendFallbackScope::Module,
                            reason: format!("{}: {}", module, reason),
                        });
                    }
                }
            }

            if !queue.is_empty() {
                let mut reverse_edges = HashMap::<String, HashSet<String>>::new();
                for (node, deps) in &dependency_edges {
                    for dep in deps {
                        reverse_edges
                            .entry(dep.clone())
                            .or_default()
                            .insert(node.clone());
                    }
                }
                if let Some(previous_graph) = previous_graph {
                    for node in &previous_graph.nodes {
                        for dep in &node.depends_on {
                            reverse_edges
                                .entry(dep.clone())
                                .or_default()
                                .insert(node.module_path.clone());
                        }
                    }
                }

                while let Some(module) = queue.pop() {
                    if let Some(parents) = reverse_edges.get(&module) {
                        let mut sorted = parents.iter().cloned().collect::<Vec<_>>();
                        sorted.sort();
                        for parent in sorted {
                            if full_modules.insert(parent.clone()) {
                                queue.push(parent);
                            }
                        }
                    }
                }
            }

            for module in &full_modules {
                body_symbols.remove(module);
            }

            (full_modules, body_symbols)
        }
    };

    if !verify_full_modules.is_empty() {
        let mut sorted = verify_full_modules.into_iter().collect::<Vec<_>>();
        sorted.sort();
        let (verify_results, phase_stats) = run_frontend_tasks_deterministic(
            "verify_full_module",
            sorted,
            selected_jobs,
            trace_mode,
            &mut planner_trace,
            |module| {
                let Some(info) = module_sources.get(module) else {
                    return Some("module source missing".to_string());
                };
                frontend_probe_module_full(module, info.source.as_ref()).err()
            },
        );
        merge_frontend_phase_stats(&mut body_hir_phase_stats, phase_stats);
        utilization_denominator_ms += phase_stats.wall_ms * phase_stats.worker_count as f64;

        for (module, maybe_err) in verify_results {
            if let Some(message) = maybe_err {
                diagnostics.push(format!("{}: {}", module, message));
                fallback_events.push(FrontendFallbackEvent {
                    stage: "verify_full_module".to_string(),
                    scope: FrontendFallbackScope::FullFrontend,
                    reason: format!("{}: {}", module, message),
                });
            }
        }
    }

    if !verify_body_symbols.is_empty() {
        let entries = std::mem::take(&mut verify_body_symbols);
        let mut modules = entries.keys().cloned().collect::<Vec<_>>();
        modules.sort();
        let (verify_results, phase_stats) = run_frontend_tasks_deterministic(
            "verify_body_hir_symbol",
            modules,
            selected_jobs,
            trace_mode,
            &mut planner_trace,
            |module| {
                let impacted_symbols = entries.get(module).cloned().unwrap_or_default();
                if impacted_symbols.is_empty() {
                    return (None::<String>, None::<String>);
                }
                let Some(info) = module_sources.get(module) else {
                    return (
                        Some("module source missing".to_string()),
                        Some("module source missing".to_string()),
                    );
                };

                match frontend_probe_module_body_only(
                    module,
                    info.source.as_ref(),
                    &impacted_symbols,
                ) {
                    Ok(_) => (None, None),
                    Err(body_message) => {
                        let full_message =
                            frontend_probe_module_full(module, info.source.as_ref()).err();
                        (Some(body_message), full_message)
                    }
                }
            },
        );
        merge_frontend_phase_stats(&mut body_hir_phase_stats, phase_stats);
        utilization_denominator_ms += phase_stats.wall_ms * phase_stats.worker_count as f64;

        for (module, (body_error, full_error)) in verify_results {
            if let Some(body_error) = body_error {
                fallback_events.push(FrontendFallbackEvent {
                    stage: "verify_body_hir_symbol".to_string(),
                    scope: FrontendFallbackScope::Symbol,
                    reason: format!("{}: {}", module, body_error),
                });

                if let Some(full_error) = full_error {
                    fallback_events.push(FrontendFallbackEvent {
                        stage: "verify_body_hir_symbol".to_string(),
                        scope: FrontendFallbackScope::FullFrontend,
                        reason: format!("{}: {}", module, full_error),
                    });
                    diagnostics.push(format!("{}: {}", module, full_error));
                } else {
                    fallback_events.push(FrontendFallbackEvent {
                        stage: "verify_body_hir_symbol".to_string(),
                        scope: FrontendFallbackScope::Module,
                        reason: format!(
                            "{}: module-scope verification succeeded after symbol fallback",
                            module
                        ),
                    });
                    diagnostics.push(format!(
                        "{}: symbol verification fallback to module scope ({})",
                        module, body_error
                    ));
                }
            }
        }
    }

    diagnostics.sort();
    diagnostics.dedup();
    fallback_events.sort_by(|a, b| {
        a.stage
            .cmp(&b.stage)
            .then_with(|| {
                frontend_fallback_scope_label(a.scope).cmp(frontend_fallback_scope_label(b.scope))
            })
            .then_with(|| a.reason.cmp(&b.reason))
    });
    fallback_events.dedup();

    let total_tasks = parse_phase_stats.task_count + body_hir_phase_stats.task_count;
    let total_queue_wait_ms =
        parse_phase_stats.queue_wait_total_ms + body_hir_phase_stats.queue_wait_total_ms;
    let queue_wait_avg_ms = if total_tasks == 0 {
        0.0
    } else {
        total_queue_wait_ms / total_tasks as f64
    };
    let queue_wait_max_ms = parse_phase_stats
        .queue_wait_max_ms
        .max(body_hir_phase_stats.queue_wait_max_ms);
    let total_busy_ms = parse_phase_stats.worker_busy_ms + body_hir_phase_stats.worker_busy_ms;
    let worker_utilization_pct = if utilization_denominator_ms > 0.0 {
        (total_busy_ms / utilization_denominator_ms) * 100.0
    } else {
        0.0
    };

    let frontend_scheduler = FrontendSchedulerTelemetry {
        requested_jobs: frontend_jobs_label(frontend_jobs),
        selected_jobs: selected_jobs as u32,
        serial_mode: selected_jobs == 1,
        parse_interface_task_count: parse_phase_stats.task_count,
        body_hir_task_count: body_hir_phase_stats.task_count,
        queue_wait_avg_ms,
        queue_wait_max_ms,
        worker_utilization_pct,
    };

    let mut module_fingerprints = module_entries
        .iter()
        .filter_map(|(path, entry)| {
            if *path == root_module {
                None
            } else {
                Some(ModuleFingerprint {
                    path: path.clone(),
                    interface_hash: entry.interface_hash,
                    hash: entry.body_hash,
                })
            }
        })
        .collect::<Vec<_>>();
    module_fingerprints.sort_by(|a, b| a.path.cmp(&b.path));

    let mut module_function_fingerprints = module_entries
        .iter()
        .map(|(path, entry)| (path.clone(), entry.symbols.clone()))
        .collect::<BTreeMap<_, _>>();
    module_function_fingerprints
        .entry(root_module.clone())
        .or_default();

    let mut module_generic_items = module_entries
        .iter()
        .map(|(path, entry)| (path.clone(), entry.generic_items.clone()))
        .collect::<BTreeMap<_, _>>();
    module_generic_items.entry(root_module.clone()).or_default();

    let mut module_generic_instances = module_entries
        .iter()
        .map(|(path, entry)| (path.clone(), entry.generic_instances.clone()))
        .collect::<BTreeMap<_, _>>();
    module_generic_instances
        .entry(root_module.clone())
        .or_default();

    let mut frontend_modules = module_entries.into_values().collect::<Vec<_>>();
    frontend_modules.sort_by(|a, b| a.module_id.cmp(&b.module_id));

    if trace_mode {
        planner_trace.push(format!(
            "frontend planner summary: parse_tasks={} body_tasks={} fallback_events={}",
            frontend_scheduler.parse_interface_task_count,
            frontend_scheduler.body_hir_task_count,
            fallback_events.len()
        ));
    }

    ModuleGraphSnapshot {
        module_fingerprints,
        module_function_fingerprints,
        module_generic_items,
        module_generic_instances,
        dependency_edges,
        reflection_import_modules,
        diagnostics,
        planner_trace,
        fallback_events,
        frontend_scheduler,
        frontend_session_store: FrontendSessionStoreV4 {
            schema_version: BUILD_GRAPH_SCHEMA_VERSION,
            scheduler_schema_version: FRONTEND_SCHEDULER_SCHEMA_VERSION,
            dependency_graph_digest: dependency_digest,
            compiler_version: env!("CARGO_PKG_VERSION").to_string(),
            root_module,
            modules: frontend_modules,
        },
        reused_modules,
        rebuilt_modules,
    }
}

pub(crate) fn module_fingerprints_for_source(
    input_path: &Path,
    source: &str,
) -> Vec<ModuleFingerprint> {
    collect_module_graph_snapshot(
        input_path,
        source,
        None,
        None,
        FrontendProbeMode::VerifyAll,
        FrontendJobs::Auto,
        false,
        true,
    )
    .module_fingerprints
}
