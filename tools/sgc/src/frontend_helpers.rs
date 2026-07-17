use sengoo_compiler::{lower_ast, lower_hir_with_options, MirLowerOptions, Parser, TypeChecker};
use std::collections::{hash_map::DefaultHasher, BTreeMap, HashSet, VecDeque};
use std::hash::{Hash, Hasher};
use std::path::Path;
use std::sync::{mpsc, Arc, Mutex};
use std::time::Instant;

use crate::{
    expand_imports_for_source, function_fingerprints_for_program, generic_fingerprints_for_program,
    implementation_fingerprint, implementation_fingerprint_from_normalized, interface_fingerprint,
    interface_fingerprint_fast, interface_fingerprint_fast_from_normalized,
    interface_fingerprint_from_program, normalize_source_for_hash, source_fingerprint,
    FrontendJobs, FrontendModuleCacheEntryV4, FrontendSchedulerPhaseStats, FunctionFingerprint,
    ModuleSourceInfo, FRONTEND_SCHEDULER_SCHEMA_VERSION,
};

pub(crate) fn frontend_probe_module_full(
    path: &str,
    source: &str,
) -> std::result::Result<(u64, u64), String> {
    let semantic_source = semantic_source_for_frontend_probe(path, source)?;
    let parsed = Parser::parse(&semantic_source).map_err(|e| format!("parse failed: {}", e))?;
    let mut checker = TypeChecker::new();
    checker
        .check_program(&parsed)
        .map_err(|e| format!("typecheck failed: {}", e))?;
    let async_functions = checker.async_function_names().clone();
    let type_env = checker.into_env();
    let hir = lower_ast(&parsed, &type_env);
    drop(type_env);
    let _ = lower_hir_with_options(
        &hir.items,
        MirLowerOptions::new(false, true, async_functions),
    )
    .map_err(|e| format!("lower failed: {}", e))?;

    Ok((
        interface_fingerprint(source),
        implementation_fingerprint(source),
    ))
}

pub(crate) fn frontend_probe_module_body_only(
    path: &str,
    source: &str,
    impacted_symbols: &[String],
) -> std::result::Result<(u64, u64), String> {
    let semantic_source = semantic_source_for_frontend_probe(path, source)?;
    let parsed = Parser::parse(&semantic_source).map_err(|e| format!("parse failed: {}", e))?;

    let checked_function_names = impacted_symbols
        .iter()
        .filter_map(|symbol| symbol.rsplit("::").next())
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string())
        .collect::<HashSet<_>>();

    if checked_function_names.is_empty() {
        return Ok((
            interface_fingerprint(source),
            implementation_fingerprint(source),
        ));
    }

    let mut checker = TypeChecker::new();
    checker
        .check_program_with_filtered_function_bodies(&parsed, &checked_function_names)
        .map_err(|e| format!("typecheck failed: {}", e))?;

    Ok((
        interface_fingerprint(source),
        implementation_fingerprint(source),
    ))
}

fn semantic_source_for_frontend_probe(path: &str, source: &str) -> Result<String, String> {
    expand_imports_for_source(Path::new(path), source)
        .map_err(|err| format!("import expansion failed: {}", err))
}

pub(crate) fn hir_fragment_fingerprint(functions: &[FunctionFingerprint]) -> u64 {
    let mut hasher = DefaultHasher::new();
    for function in functions {
        function.symbol.hash(&mut hasher);
        function.abi_hash.hash(&mut hasher);
        function.body_hash.hash(&mut hasher);
        for call in &function.calls {
            call.hash(&mut hasher);
        }
        for import in &function.module_imports {
            import.hash(&mut hasher);
        }
    }
    hasher.finish()
}

pub(crate) fn dependency_graph_digest(dependency_edges: &BTreeMap<String, Vec<String>>) -> u64 {
    let mut hasher = DefaultHasher::new();
    for (module, deps) in dependency_edges {
        module.hash(&mut hasher);
        let mut unique = deps.clone();
        unique.sort();
        unique.dedup();
        for dep in unique {
            dep.hash(&mut hasher);
        }
        "|".hash(&mut hasher);
    }
    hasher.finish()
}

pub(crate) fn resolve_frontend_job_count(requested: FrontendJobs, task_count: usize) -> usize {
    // Pin-grade dual package builds set SENGOO_DETERMINISTIC_LINK=1; force a
    // serial frontend so independent rebuilds emit bit-identical IR/object order.
    let force_serial = match std::env::var("SENGOO_DETERMINISTIC_LINK") {
        Ok(value) => {
            let trimmed = value.trim();
            !(trimmed.is_empty() || trimmed == "0" || trimmed.eq_ignore_ascii_case("false"))
        }
        Err(_) => false,
    };
    if force_serial {
        return 1.min(task_count.max(1));
    }
    let requested = match requested {
        FrontendJobs::Auto => std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .saturating_sub(1)
            .max(1),
        FrontendJobs::Fixed(value) => value.max(1),
    };
    requested.min(task_count.max(1))
}

#[derive(Debug, Clone)]
struct FrontendScheduledTask {
    id: String,
    enqueued_at: Instant,
}

#[derive(Debug)]
struct FrontendTaskResult<R> {
    id: String,
    value: R,
    wait_ms: f64,
    busy_ms: f64,
}

pub(crate) fn merge_frontend_phase_stats(
    total: &mut FrontendSchedulerPhaseStats,
    current: FrontendSchedulerPhaseStats,
) {
    total.task_count += current.task_count;
    total.queue_wait_total_ms += current.queue_wait_total_ms;
    if current.queue_wait_max_ms > total.queue_wait_max_ms {
        total.queue_wait_max_ms = current.queue_wait_max_ms;
    }
    total.worker_busy_ms += current.worker_busy_ms;
    total.wall_ms += current.wall_ms;
    total.worker_count += current.worker_count;
}

pub(crate) fn run_frontend_tasks_deterministic<R, F>(
    phase_name: &str,
    task_ids: Vec<String>,
    jobs: usize,
    trace_mode: bool,
    planner_trace: &mut Vec<String>,
    execute: F,
) -> (BTreeMap<String, R>, FrontendSchedulerPhaseStats)
where
    R: Send,
    F: Fn(&str) -> R + Sync,
{
    let mut results = BTreeMap::<String, R>::new();
    if task_ids.is_empty() {
        return (results, FrontendSchedulerPhaseStats::default());
    }

    let worker_count = jobs.max(1).min(task_ids.len());
    if trace_mode {
        planner_trace.push(format!(
            "frontend scheduler: phase={} tasks={} workers={}",
            phase_name,
            task_ids.len(),
            worker_count
        ));
    }

    let mut stats = FrontendSchedulerPhaseStats {
        task_count: task_ids.len() as u32,
        worker_count: worker_count as u32,
        ..Default::default()
    };

    if worker_count == 1 {
        let phase_start = Instant::now();
        for id in task_ids {
            let work_start = Instant::now();
            let value = execute(&id);
            let busy_ms = work_start.elapsed().as_secs_f64() * 1000.0;
            stats.worker_busy_ms += busy_ms;
            results.insert(id, value);
        }
        stats.wall_ms = phase_start.elapsed().as_secs_f64() * 1000.0;
        return (results, stats);
    }

    let now = Instant::now();
    let queue = task_ids
        .into_iter()
        .map(|id| FrontendScheduledTask {
            id,
            enqueued_at: now,
        })
        .collect::<VecDeque<_>>();
    let queue = Arc::new(Mutex::new(queue));

    let phase_start = Instant::now();
    let mut completed = Vec::<FrontendTaskResult<R>>::with_capacity(stats.task_count as usize);

    std::thread::scope(|scope| {
        let (tx, rx) = mpsc::channel::<FrontendTaskResult<R>>();

        for _ in 0..worker_count {
            let queue = Arc::clone(&queue);
            let tx = tx.clone();
            let execute = &execute;
            scope.spawn(move || loop {
                let task = {
                    let mut queue = queue.lock().expect("frontend scheduler queue poisoned");
                    queue.pop_front()
                };
                let Some(task) = task else {
                    break;
                };

                let wait_ms = task.enqueued_at.elapsed().as_secs_f64() * 1000.0;
                let work_start = Instant::now();
                let value = execute(&task.id);
                let busy_ms = work_start.elapsed().as_secs_f64() * 1000.0;
                if tx
                    .send(FrontendTaskResult {
                        id: task.id,
                        value,
                        wait_ms,
                        busy_ms,
                    })
                    .is_err()
                {
                    break;
                }
            });
        }

        drop(tx);
        for _ in 0..stats.task_count {
            match rx.recv() {
                Ok(done) => completed.push(done),
                Err(_) => break,
            }
        }
    });

    stats.wall_ms = phase_start.elapsed().as_secs_f64() * 1000.0;
    for done in completed {
        stats.worker_busy_ms += done.busy_ms;
        stats.queue_wait_total_ms += done.wait_ms;
        if done.wait_ms > stats.queue_wait_max_ms {
            stats.queue_wait_max_ms = done.wait_ms;
        }
        results.insert(done.id, done.value);
    }

    (results, stats)
}

pub(crate) fn frontend_cache_entry_for_module(
    module_path: &str,
    info: &ModuleSourceInfo,
    dependency_digest: u64,
    collect_symbol_fingerprints: bool,
) -> FrontendModuleCacheEntryV4 {
    let mut depends_on = info.depends_on.clone();
    depends_on.sort();
    depends_on.dedup();

    let source_hash = source_fingerprint(info.source.as_ref());
    let (interface_hash, body_hash, mut symbols, mut generic_items, mut generic_instances) =
        if collect_symbol_fingerprints {
            let body_hash = implementation_fingerprint(info.source.as_ref());
            match Parser::parse(info.source.as_ref()) {
                Ok(program) => {
                    let (generic_items, generic_instances) = generic_fingerprints_for_program(
                        module_path,
                        info.source.as_ref(),
                        &program,
                    );
                    (
                        interface_fingerprint_from_program(&program),
                        body_hash,
                        function_fingerprints_for_program(
                            module_path,
                            info.source.as_ref(),
                            &program,
                        ),
                        generic_items,
                        generic_instances,
                    )
                }
                Err(_) => (
                    interface_fingerprint_fast(info.source.as_ref()),
                    body_hash,
                    Vec::new(),
                    Vec::new(),
                    Vec::new(),
                ),
            }
        } else {
            let normalized = normalize_source_for_hash(info.source.as_ref());
            (
                interface_fingerprint_fast_from_normalized(&normalized),
                implementation_fingerprint_from_normalized(&normalized),
                Vec::new(),
                Vec::new(),
                Vec::new(),
            )
        };
    if collect_symbol_fingerprints {
        for symbol in &mut symbols {
            if symbol.module_imports.is_empty() {
                symbol.module_imports = depends_on.clone();
            }
        }
        symbols.sort_by(|a, b| a.symbol.cmp(&b.symbol));
        generic_items.sort_by(|a, b| a.stable_item_id.cmp(&b.stable_item_id));
        generic_items.dedup_by(|a, b| a.stable_item_id == b.stable_item_id);
        generic_instances.sort_by(|a, b| a.instance_key.cmp(&b.instance_key));
        generic_instances.dedup_by(|a, b| a.instance_key == b.instance_key);
    }

    FrontendModuleCacheEntryV4 {
        module_id: module_path.to_string(),
        source_hash,
        parse_hash: source_hash,
        interface_hash,
        body_hash,
        hir_hash: hir_fragment_fingerprint(&symbols),
        dependency_digest,
        scheduler_schema_version: FRONTEND_SCHEDULER_SCHEMA_VERSION,
        depends_on,
        symbols,
        generic_items,
        generic_instances,
    }
}
