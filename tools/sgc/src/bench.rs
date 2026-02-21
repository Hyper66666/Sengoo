use crate::{
    canonical_or_lossy, collect_module_graph_snapshot, compile_native_binary,
    compile_source_to_llvm_file_with_phase_timings, default_build_output_path_for_case, find_clang,
    find_runtime_c, maybe_prepare_reflection_native_library, measure_reflection_used_ms,
    module_fingerprints_for_source, module_invalidation_stats,
    reflection_sidecar_path_for_artifact, FrontendFallbackEvent, FrontendJobs, FrontendProbeMode,
    FrontendSchedulerTelemetry,
};
use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct BenchCaseResult {
    pub(crate) name: String,
    pub(crate) iterations: u32,
    pub(crate) warmup: u32,
    pub(crate) sample_ms: Vec<f64>,
    pub(crate) p50_ms: Option<f64>,
    pub(crate) p95_ms: Option<f64>,
    pub(crate) phases: Option<BTreeMap<String, f64>>,
    pub(crate) total_ms: Option<f64>,
    pub(crate) before_ms: Option<f64>,
    pub(crate) after_ms: Option<f64>,
    pub(crate) cache_reused_modules: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) generic_cache_hit_ratio: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) generic_rebuilt_instances: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) frontend_scheduler: Option<FrontendSchedulerTelemetry>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) frontend_fallback_events: Option<Vec<FrontendFallbackEvent>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) frontend_planner_trace: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub(crate) struct BenchReport {
    pub(crate) schema_version: u32,
    pub(crate) kind: String,
    pub(crate) suite: String,
    pub(crate) generated_at_unix_ms: u128,
    pub(crate) cases: Vec<BenchCaseResult>,
}

#[derive(Debug, Deserialize, Default)]
struct BenchBaselineTargets {
    runtime_median_improvement_pct: Option<f64>,
    full_compile_reduction_pct: Option<f64>,
    incremental_compile_reduction_pct: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct BenchBaselineCase {
    p50_ms: Option<f64>,
    total_ms: Option<f64>,
    before_ms: Option<f64>,
    after_ms: Option<f64>,
}

#[derive(Debug, Deserialize, Default)]
struct BenchBaseline {
    #[allow(dead_code)]
    schema_version: Option<u32>,
    #[allow(dead_code)]
    updated_at: Option<String>,
    #[serde(default)]
    targets: BenchBaselineTargets,
    #[serde(default)]
    cases: BTreeMap<String, BenchBaselineCase>,
}

#[derive(Debug, Deserialize, Default)]
struct WorksetGenericMetrics {
    #[serde(default)]
    generic_total_instances: u32,
    #[serde(default)]
    generic_cache_hits: u32,
    #[serde(default)]
    generic_rebuilt_instances: u32,
}

fn read_workset_generic_metrics(case: &Path, command_kind: &str) -> Option<WorksetGenericMetrics> {
    let stem = case.file_stem()?.to_string_lossy().to_string();
    let build_dir = case.parent()?.join("build").join("workset");
    let path = build_dir.join(format!("{}.{}.workset.json", stem, command_kind));
    let bytes = fs::read(path).ok()?;
    serde_json::from_slice::<WorksetGenericMetrics>(&bytes).ok()
}

fn generic_cache_metadata_path(case: &Path) -> Option<PathBuf> {
    let stem = case.file_stem()?.to_string_lossy().to_string();
    let build_dir = case.parent()?.join("build").join("workset");
    Some(build_dir.join(format!("{}.generic-instance-cache.json", stem)))
}

fn build_cache_metadata_path(case: &Path) -> Option<PathBuf> {
    let stem = case.file_stem()?.to_string_lossy().to_string();
    let build_dir = case.parent()?.join("build");
    Some(build_dir.join(format!("{}.build-cache.json", stem)))
}

fn frontend_session_metadata_path(case: &Path) -> Option<PathBuf> {
    let stem = case.file_stem()?.to_string_lossy().to_string();
    let build_dir = case.parent()?.join("build").join("workset");
    Some(build_dir.join(format!("{}.frontend-session-v4.json", stem)))
}

pub(crate) fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

pub(crate) fn bench_root_dir() -> PathBuf {
    if let Ok(cwd) = std::env::current_dir() {
        for dir in cwd.ancestors() {
            let candidate = dir.join("bench");
            if candidate.join("suites").exists() {
                return candidate;
            }
        }
    }
    PathBuf::from("bench")
}

pub(crate) fn resolve_bench_suite_path(kind: &str, suite: &str) -> Result<PathBuf> {
    let default_dir = bench_root_dir().join("suites").join(kind);
    if suite == kind || suite == "default" {
        return Ok(default_dir);
    }

    let suite_path = Path::new(suite);
    if suite_path.exists() {
        return Ok(suite_path.to_path_buf());
    }

    let candidate = default_dir.join(suite);
    if candidate.exists() {
        return Ok(candidate);
    }

    let candidate_sg = default_dir.join(format!("{}.sg", suite));
    if candidate_sg.exists() {
        return Ok(candidate_sg);
    }

    Err(miette::miette!(
        "benchmark suite not found: '{}' (kind={})",
        suite,
        kind
    ))
}

pub(crate) fn collect_bench_cases(path: &Path) -> Result<Vec<PathBuf>> {
    if path.is_file() {
        return Ok(vec![path.to_path_buf()]);
    }

    let mut cases = fs::read_dir(path)
        .into_diagnostic()?
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|p| p.extension().is_some_and(|ext| ext == "sg"))
        .collect::<Vec<_>>();

    cases.sort();
    if cases.is_empty() {
        return Err(miette::miette!(
            "no benchmark cases found under {}",
            path.to_string_lossy()
        ));
    }
    Ok(cases)
}

pub(crate) fn percentile(values: &[f64], p: f64) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted.get(idx).copied()
}

pub(crate) fn run_sgc_command(args: &[String]) -> Result<()> {
    let exe = std::env::current_exe().into_diagnostic()?;
    let output = Command::new(exe).args(args).output().into_diagnostic()?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(miette::miette!(
            "benchmark command failed: sgc {}\n{}",
            args.join(" "),
            stderr
        ));
    }
    Ok(())
}

pub(crate) fn measure_sgc_command_ms(args: &[String]) -> Result<f64> {
    let start = Instant::now();
    run_sgc_command(args)?;
    Ok(start.elapsed().as_secs_f64() * 1000.0)
}

pub(crate) fn sanitize_for_filename(input: &str) -> String {
    input
        .chars()
        .map(|c| match c {
            '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|' | ' ' => '_',
            _ => c,
        })
        .collect()
}

fn link_ir_file_with_clang_ms(
    llvm_ir_path: &Path,
    case_name: &str,
    clang_exe: &str,
    runtime_c: Option<&str>,
    clang_opt_level: u8,
) -> Result<f64> {
    let tmp_dir = bench_root_dir().join("results").join(".tmp");
    fs::create_dir_all(&tmp_dir).into_diagnostic()?;

    let stamp = now_unix_ms();
    let base = sanitize_for_filename(case_name);
    let exe_path = if cfg!(windows) {
        tmp_dir.join(format!("{}-{}.exe", base, stamp))
    } else {
        tmp_dir.join(format!("{}-{}", base, stamp))
    };

    let link_start = Instant::now();
    compile_native_binary(
        clang_exe,
        llvm_ir_path,
        &exe_path,
        runtime_c,
        clang_opt_level,
    )?;
    let link_ms = link_start.elapsed().as_secs_f64() * 1000.0;

    let _ = fs::remove_file(&exe_path);
    Ok(link_ms)
}

pub(crate) fn write_bench_report(report: &BenchReport) -> Result<PathBuf> {
    let out_dir = bench_root_dir().join("results");
    fs::create_dir_all(&out_dir).into_diagnostic()?;

    let file_name = format!(
        "{}-{}-{}.json",
        now_unix_ms(),
        sanitize_for_filename(&report.kind),
        sanitize_for_filename(&report.suite)
    );
    let output = out_dir.join(file_name);

    let bytes = serde_json::to_vec_pretty(report)
        .map_err(|e| miette::miette!("failed to serialize benchmark report: {}", e))?;
    fs::write(&output, bytes)
        .into_diagnostic()
        .map_err(|e| miette::miette!("failed to write benchmark report: {}", e))?;
    Ok(output)
}

fn baseline_case_key(kind: &str, suite: &str, case_name: &str) -> String {
    format!("{}/{}/{}", kind, suite, case_name)
}

fn read_bench_baseline() -> Result<BenchBaseline, String> {
    let path = bench_root_dir().join("baseline.json");
    let bytes =
        fs::read(&path).map_err(|_| format!("baseline missing: {}", path.to_string_lossy()))?;

    // Windows editors often write UTF-8 BOM. Accept both BOM and non-BOM JSON.
    let bytes = bytes
        .strip_prefix(&[0xEF, 0xBB, 0xBF])
        .unwrap_or(bytes.as_slice());
    serde_json::from_slice::<BenchBaseline>(bytes)
        .map_err(|err| format!("baseline parse error: {} ({})", path.to_string_lossy(), err))
}

pub(crate) fn diff_report_against_baseline(report: &BenchReport) -> Vec<String> {
    let mut lines = Vec::new();
    let baseline = match read_bench_baseline() {
        Ok(baseline) => baseline,
        Err(err) => {
            lines.push(err);
            return lines;
        }
    };

    for case in &report.cases {
        let key = baseline_case_key(&report.kind, &report.suite, &case.name);
        let Some(base_case) = baseline.cases.get(&key) else {
            continue;
        };

        match report.kind.as_str() {
            "runtime" => {
                if let (Some(curr), Some(base)) = (case.p50_ms, base_case.p50_ms) {
                    let delta_pct = ((curr - base) / base) * 100.0;
                    lines.push(format!(
                        "{} p50: {:.2}ms vs baseline {:.2}ms ({:+.2}%)",
                        case.name, curr, base, delta_pct
                    ));
                    if let Some(target) = baseline.targets.runtime_median_improvement_pct {
                        let improvement = ((base - curr) / base) * 100.0;
                        lines.push(format!(
                            "{} runtime improvement: {:.2}% (target {:.2}%)",
                            case.name, improvement, target
                        ));
                    }
                }
            }
            "compile" => {
                if let (Some(curr), Some(base)) = (case.total_ms, base_case.total_ms) {
                    let delta_pct = ((curr - base) / base) * 100.0;
                    lines.push(format!(
                        "{} total: {:.2}ms vs baseline {:.2}ms ({:+.2}%)",
                        case.name, curr, base, delta_pct
                    ));
                    if let Some(target) = baseline.targets.full_compile_reduction_pct {
                        let reduction = ((base - curr) / base) * 100.0;
                        lines.push(format!(
                            "{} full compile reduction: {:.2}% (target {:.2}%)",
                            case.name, reduction, target
                        ));
                    }
                }
            }
            "incremental" => {
                if let (Some(curr), Some(base)) = (case.before_ms, base_case.before_ms) {
                    let delta_pct = ((curr - base) / base) * 100.0;
                    lines.push(format!(
                        "{} before: {:.2}ms vs baseline {:.2}ms ({:+.2}%)",
                        case.name, curr, base, delta_pct
                    ));
                }

                if let (Some(curr), Some(base)) = (case.after_ms, base_case.after_ms) {
                    let delta_pct = ((curr - base) / base) * 100.0;
                    lines.push(format!(
                        "{} after: {:.2}ms vs baseline {:.2}ms ({:+.2}%)",
                        case.name, curr, base, delta_pct
                    ));
                }

                if let (Some(before), Some(after), Some(target)) = (
                    case.before_ms,
                    case.after_ms,
                    baseline.targets.incremental_compile_reduction_pct,
                ) {
                    let gain = ((before - after) / before) * 100.0;
                    lines.push(format!(
                        "{} reduction vs same-run before: {:.2}% (target {:.2}%)",
                        case.name, gain, target
                    ));
                }
            }
            "reflection" => {
                if let (Some(curr), Some(base)) = (case.p50_ms, base_case.p50_ms) {
                    let delta_pct = ((curr - base) / base) * 100.0;
                    lines.push(format!(
                        "{} p50: {:.2}ms vs baseline {:.2}ms ({:+.2}%)",
                        case.name, curr, base, delta_pct
                    ));
                }
            }
            _ => {}
        }
    }

    if lines.is_empty() {
        lines.push(
            "baseline loaded, but no matching case metrics were found for this report".to_string(),
        );
    }
    lines
}

pub(crate) async fn cmd_bench_run(
    suite: &str,
    opt_level: u8,
    warmup: u32,
    iterations: u32,
) -> Result<()> {
    let suite_path = resolve_bench_suite_path("runtime", suite)?;
    let cases = collect_bench_cases(&suite_path)?;

    println!(
        "Benchmark runtime suite: {} ({} case(s))",
        suite_path.to_string_lossy(),
        cases.len()
    );

    let mut results = Vec::new();
    for case in cases {
        let case_name = case
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| case.to_string_lossy().to_string());

        let mut sample_ms = Vec::new();
        for i in 0..warmup {
            let mut args = vec![
                "run".to_string(),
                case.to_string_lossy().to_string(),
                "-O".to_string(),
                opt_level.to_string(),
            ];
            if i == 0 {
                args.push("--force-rebuild".to_string());
            }
            run_sgc_command(&args)?;
        }

        for _ in 0..iterations {
            let args = vec![
                "run".to_string(),
                case.to_string_lossy().to_string(),
                "-O".to_string(),
                opt_level.to_string(),
            ];
            sample_ms.push(measure_sgc_command_ms(&args)?);
        }

        let p50 = percentile(&sample_ms, 0.50);
        let p95 = percentile(&sample_ms, 0.95);
        println!(
            "  - {}: p50={:.2}ms p95={:.2}ms",
            case_name,
            p50.unwrap_or_default(),
            p95.unwrap_or_default()
        );

        results.push(BenchCaseResult {
            name: case_name,
            iterations,
            warmup,
            sample_ms,
            p50_ms: p50,
            p95_ms: p95,
            phases: None,
            total_ms: None,
            before_ms: None,
            after_ms: None,
            cache_reused_modules: None,
            generic_cache_hit_ratio: None,
            generic_rebuilt_instances: None,
            frontend_scheduler: None,
            frontend_fallback_events: None,
            frontend_planner_trace: None,
        });
    }

    let report = BenchReport {
        schema_version: 1,
        kind: "runtime".to_string(),
        suite: suite.to_string(),
        generated_at_unix_ms: now_unix_ms(),
        cases: results,
    };
    let out = write_bench_report(&report)?;
    println!("Runtime benchmark report: {}", out.to_string_lossy());
    for line in diff_report_against_baseline(&report) {
        println!("  baseline: {}", line);
    }
    Ok(())
}

pub(crate) async fn cmd_bench_compile(suite: &str, opt_level: u8, iterations: u32) -> Result<()> {
    let suite_path = resolve_bench_suite_path("compile", suite)?;
    let cases = collect_bench_cases(&suite_path)?;
    let clang = find_clang();
    let runtime_c = find_runtime_c();
    if clang.is_none() {
        println!("  ! clang not found, compile benchmark link phase will be 0ms");
    }

    println!(
        "Benchmark compile suite: {} ({} case(s))",
        suite_path.to_string_lossy(),
        cases.len()
    );

    let mut results = Vec::new();
    for case in cases {
        let case_name = case
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| case.to_string_lossy().to_string());
        let source = fs::read_to_string(&case)
            .into_diagnostic()
            .map_err(|e| miette::miette!("failed to read benchmark case {}: {}", case_name, e))?;
        let frontend_snapshot = collect_module_graph_snapshot(
            &case,
            &source,
            None,
            None,
            FrontendProbeMode::VerifyAll,
            FrontendJobs::Auto,
            false,
            true,
        );

        let mut sample_ms = Vec::new();
        let mut phase_totals: BTreeMap<String, f64> = BTreeMap::new();
        // Use O0 for the external clang link step to reduce backend noise in compile KPI.
        let bench_link_opt_level = 0;
        let tmp_dir = bench_root_dir().join("results").join(".tmp");
        fs::create_dir_all(&tmp_dir).into_diagnostic()?;
        for _ in 0..iterations {
            let ll_path = tmp_dir.join(format!(
                "{}-{}-{}.ll",
                sanitize_for_filename(&case_name),
                now_unix_ms(),
                sample_ms.len()
            ));
            let (mut phases, _effective_mode) =
                compile_source_to_llvm_file_with_phase_timings(&source, opt_level, &ll_path)?;
            if let Some(clang_exe) = clang.as_deref() {
                let link_ms = link_ir_file_with_clang_ms(
                    &ll_path,
                    &case_name,
                    clang_exe,
                    runtime_c.as_deref(),
                    bench_link_opt_level,
                )?;
                phases.insert("link".to_string(), link_ms);
            }
            let _ = fs::remove_file(&ll_path);

            let total_ms = phases
                .iter()
                .filter(|(phase, _)| phase_is_timing_metric(phase))
                .map(|(_, value)| *value)
                .sum();
            sample_ms.push(total_ms);
            for (phase, value) in phases {
                *phase_totals.entry(phase).or_insert(0.0) += value;
            }
        }

        let avg_ms = if sample_ms.is_empty() {
            0.0
        } else {
            sample_ms.iter().sum::<f64>() / sample_ms.len() as f64
        };
        let mut phase_avg = BTreeMap::new();
        if iterations > 0 {
            for (phase, total) in phase_totals {
                phase_avg.insert(phase, total / iterations as f64);
            }
        }
        for required in ["parse", "typeck", "mir", "codegen", "link"] {
            phase_avg.entry(required.to_string()).or_insert(0.0);
        }

        println!("  - {}: avg={:.2}ms", case_name, avg_ms);

        results.push(BenchCaseResult {
            name: case_name,
            iterations,
            warmup: 0,
            sample_ms: sample_ms.clone(),
            p50_ms: percentile(&sample_ms, 0.50),
            p95_ms: percentile(&sample_ms, 0.95),
            phases: Some(phase_avg),
            total_ms: Some(avg_ms),
            before_ms: None,
            after_ms: None,
            cache_reused_modules: None,
            generic_cache_hit_ratio: None,
            generic_rebuilt_instances: None,
            frontend_scheduler: Some(frontend_snapshot.frontend_scheduler.clone()),
            frontend_fallback_events: if frontend_snapshot.fallback_events.is_empty() {
                None
            } else {
                Some(frontend_snapshot.fallback_events.clone())
            },
            frontend_planner_trace: None,
        });
    }

    let report = BenchReport {
        schema_version: 1,
        kind: "compile".to_string(),
        suite: suite.to_string(),
        generated_at_unix_ms: now_unix_ms(),
        cases: results,
    };
    let out = write_bench_report(&report)?;
    println!("Compile benchmark report: {}", out.to_string_lossy());
    for line in diff_report_against_baseline(&report) {
        println!("  baseline: {}", line);
    }
    Ok(())
}

fn mutate_incremental_source(case_name: &str, original: &str, iter: u32) -> String {
    if case_name.contains("generic_body_change") {
        let replacement = format!("x + {}", (iter % 5) + 2);
        return original.replace("x + 1", &replacement);
    }
    if case_name.contains("generic_signature_change") {
        return original
            .replace(
                "def generic_sig<T>(marker: T, x: i64) -> i64 { x + 1 }",
                "def generic_sig<T>(marker: T, x: i64, y: i64) -> i64 { x + y }",
            )
            .replace("generic_sig(0, 1)", "generic_sig(0, 1, 2)")
            .replace("generic_sig(1)", "generic_sig(1, 2)");
    }
    if case_name.contains("generic_new_instantiation") {
        return original
            .replace(
                "    generic_inst(0, 1)",
                "    let a = generic_inst(0, 1);\n    let b = generic_inst(true, 1);\n    a + b",
            )
            .replace("    generic_inst(1)", "    let v = 1;\n    generic_inst(v)");
    }

    let mut mutated = original.to_string();
    mutated.push_str(&format!("\n// bench-incremental-mut-{}\n", iter));
    mutated
}

fn phase_is_timing_metric(phase: &str) -> bool {
    !phase.ends_with("_removed")
}

pub(crate) async fn cmd_bench_incremental(
    suite: &str,
    opt_level: u8,
    iterations: u32,
) -> Result<()> {
    let suite_path = resolve_bench_suite_path("incremental", suite)?;
    let cases = collect_bench_cases(&suite_path)?
        .into_iter()
        .filter(|case| {
            case.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_root.sg"))
        })
        .collect::<Vec<_>>();
    if cases.is_empty() {
        return Err(miette::miette!(
            "no incremental benchmark root cases found under {}",
            suite_path.to_string_lossy()
        ));
    }

    println!(
        "Benchmark incremental suite: {} ({} case(s))",
        suite_path.to_string_lossy(),
        cases.len()
    );

    let mut results = Vec::new();
    for case in cases {
        let case_name = case
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| case.to_string_lossy().to_string());

        let original = fs::read_to_string(&case)
            .into_diagnostic()
            .map_err(|e| miette::miette!("failed to read benchmark case {}: {}", case_name, e))?;

        let mut before_samples = Vec::new();
        let mut after_samples = Vec::new();
        let mut reused_module_samples = Vec::new();
        let mut generic_hit_ratio_samples = Vec::new();
        let mut generic_rebuilt_samples = Vec::new();

        for i in 0..iterations {
            if let Some(cache_path) = build_cache_metadata_path(&case) {
                let _ = fs::remove_file(cache_path);
            }
            if let Some(cache_path) = generic_cache_metadata_path(&case) {
                let _ = fs::remove_file(cache_path);
            }
            if let Some(session_path) = frontend_session_metadata_path(&case) {
                let _ = fs::remove_file(session_path);
            }
            fs::write(&case, &original)
                .into_diagnostic()
                .map_err(|e| miette::miette!("failed to reset benchmark case: {}", e))?;

            let before_args = vec![
                "build".to_string(),
                case.to_string_lossy().to_string(),
                "-O".to_string(),
                opt_level.to_string(),
            ];
            before_samples.push(measure_sgc_command_ms(&before_args)?);
            let before_modules = module_fingerprints_for_source(&case, &original);

            let mutated = mutate_incremental_source(&case_name, &original, i);
            fs::write(&case, &mutated)
                .into_diagnostic()
                .map_err(|e| miette::miette!("failed to mutate benchmark case: {}", e))?;
            let after_modules = module_fingerprints_for_source(&case, &mutated);
            let reused_modules =
                module_invalidation_stats(&before_modules, &after_modules).reused_modules;
            reused_module_samples.push(reused_modules);

            let after_args = vec![
                "build".to_string(),
                case.to_string_lossy().to_string(),
                "-O".to_string(),
                opt_level.to_string(),
            ];
            after_samples.push(measure_sgc_command_ms(&after_args)?);
            if let Some(metrics) = read_workset_generic_metrics(&case, "build") {
                let hit_ratio = if metrics.generic_total_instances == 0 {
                    1.0
                } else {
                    metrics.generic_cache_hits as f64 / metrics.generic_total_instances as f64
                };
                generic_hit_ratio_samples.push(hit_ratio);
                generic_rebuilt_samples.push(metrics.generic_rebuilt_instances);
            }
        }

        fs::write(&case, original)
            .into_diagnostic()
            .map_err(|e| miette::miette!("failed to restore benchmark case: {}", e))?;

        let before_avg = if before_samples.is_empty() {
            0.0
        } else {
            before_samples.iter().sum::<f64>() / before_samples.len() as f64
        };
        let after_avg = if after_samples.is_empty() {
            0.0
        } else {
            after_samples.iter().sum::<f64>() / after_samples.len() as f64
        };
        let reused_avg = if reused_module_samples.is_empty() {
            0
        } else {
            (reused_module_samples.iter().sum::<u32>() as f64 / reused_module_samples.len() as f64)
                .round() as u32
        };
        let generic_hit_ratio_avg = if generic_hit_ratio_samples.is_empty() {
            None
        } else {
            Some(
                generic_hit_ratio_samples.iter().sum::<f64>()
                    / generic_hit_ratio_samples.len() as f64,
            )
        };
        let generic_rebuilt_avg = if generic_rebuilt_samples.is_empty() {
            None
        } else {
            Some(
                (generic_rebuilt_samples.iter().sum::<u32>() as f64
                    / generic_rebuilt_samples.len() as f64)
                    .round() as u32,
            )
        };

        println!(
            "  - {}: before={:.2}ms after={:.2}ms reused_modules={} generic_hit_ratio={:.2} generic_rebuilt={}",
            case_name,
            before_avg,
            after_avg,
            reused_avg,
            generic_hit_ratio_avg.unwrap_or(0.0),
            generic_rebuilt_avg.unwrap_or(0),
        );

        results.push(BenchCaseResult {
            name: case_name,
            iterations,
            warmup: 0,
            sample_ms: Vec::new(),
            p50_ms: None,
            p95_ms: None,
            phases: None,
            total_ms: None,
            before_ms: Some(before_avg),
            after_ms: Some(after_avg),
            cache_reused_modules: Some(reused_avg),
            generic_cache_hit_ratio: generic_hit_ratio_avg,
            generic_rebuilt_instances: generic_rebuilt_avg,
            frontend_scheduler: None,
            frontend_fallback_events: None,
            frontend_planner_trace: None,
        });
    }

    let report = BenchReport {
        schema_version: 1,
        kind: "incremental".to_string(),
        suite: suite.to_string(),
        generated_at_unix_ms: now_unix_ms(),
        cases: results,
    };
    let out = write_bench_report(&report)?;
    println!("Incremental benchmark report: {}", out.to_string_lossy());
    for line in diff_report_against_baseline(&report) {
        println!("  baseline: {}", line);
    }
    Ok(())
}

pub(crate) async fn cmd_bench_reflection(
    suite: &str,
    opt_level: u8,
    warmup: u32,
    iterations: u32,
) -> Result<()> {
    let suite_path = resolve_bench_suite_path("runtime", suite)?;
    let cases = collect_bench_cases(&suite_path)?;
    let case = cases
        .first()
        .cloned()
        .ok_or_else(|| miette::miette!("no reflection benchmark case found"))?;
    let case_name = case
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| case.to_string_lossy().to_string());

    println!(
        "Benchmark reflection suite: {} (case={})",
        suite_path.to_string_lossy(),
        case_name
    );

    let base_args = vec![
        "build".to_string(),
        case.to_string_lossy().to_string(),
        "-O".to_string(),
        opt_level.to_string(),
        "--force-rebuild".to_string(),
    ];
    let mut reflect_args = base_args.clone();
    reflect_args.push("--reflect".to_string());

    let module_id = canonical_or_lossy(&case);
    let artifact_path = default_build_output_path_for_case(&case);
    let sidecar_path = reflection_sidecar_path_for_artifact(&artifact_path);
    let llvm_ir_path = artifact_path.with_extension("ll");
    let clang_exe = find_clang();
    let runtime_c = find_runtime_c();
    let mut native_prepare_warning: Option<String> = None;
    let mut native_bound_measurements = 0u32;

    for _ in 0..warmup {
        let _ = measure_sgc_command_ms(&base_args)?;
        let _ = measure_sgc_command_ms(&reflect_args)?;
        let _ = measure_sgc_command_ms(&reflect_args)?;
        if !sidecar_path.exists() {
            return Err(miette::miette!(
                "reflection sidecar missing during warmup: {}",
                sidecar_path.to_string_lossy()
            ));
        }
        let native_library_path = match maybe_prepare_reflection_native_library(
            clang_exe.as_deref(),
            runtime_c.as_deref(),
            &llvm_ir_path,
            &artifact_path,
            &sidecar_path,
            opt_level,
        ) {
            Ok(path) => path,
            Err(err) => {
                if native_prepare_warning.is_none() {
                    native_prepare_warning = Some(err.to_string());
                }
                None
            }
        };
        let _ =
            measure_reflection_used_ms(&sidecar_path, &module_id, native_library_path.as_deref())?;
    }

    let mut disabled_samples = Vec::new();
    let mut enabled_unused_samples = Vec::new();
    let mut enabled_used_samples = Vec::new();
    for _ in 0..iterations {
        disabled_samples.push(measure_sgc_command_ms(&base_args)?);

        enabled_unused_samples.push(measure_sgc_command_ms(&reflect_args)?);

        let build_ms = measure_sgc_command_ms(&reflect_args)?;
        if !sidecar_path.exists() {
            return Err(miette::miette!(
                "reflection sidecar missing after reflected build: {}",
                sidecar_path.to_string_lossy()
            ));
        }
        let native_library_path = match maybe_prepare_reflection_native_library(
            clang_exe.as_deref(),
            runtime_c.as_deref(),
            &llvm_ir_path,
            &artifact_path,
            &sidecar_path,
            opt_level,
        ) {
            Ok(path) => path,
            Err(err) => {
                if native_prepare_warning.is_none() {
                    native_prepare_warning = Some(err.to_string());
                }
                None
            }
        };
        let (used_ms, native_bound) =
            measure_reflection_used_ms(&sidecar_path, &module_id, native_library_path.as_deref())?;
        if native_bound {
            native_bound_measurements += 1;
        }
        enabled_used_samples.push(build_ms + used_ms);
    }

    let avg = |samples: &[f64]| -> f64 {
        if samples.is_empty() {
            0.0
        } else {
            samples.iter().sum::<f64>() / samples.len() as f64
        }
    };
    let disabled_p50 = percentile(&disabled_samples, 0.50).unwrap_or(0.0);
    let enabled_unused_p50 = percentile(&enabled_unused_samples, 0.50).unwrap_or(0.0);
    let enabled_used_p50 = percentile(&enabled_used_samples, 0.50).unwrap_or(0.0);
    if disabled_p50 > 0.0 {
        println!(
            "  - disabled p50={:.2}ms, enabled-unused overhead={:+.2}%, enabled-used overhead={:+.2}%",
            disabled_p50,
            ((enabled_unused_p50 - disabled_p50) / disabled_p50) * 100.0,
            ((enabled_used_p50 - disabled_p50) / disabled_p50) * 100.0,
        );
    }
    if let Some(warning) = native_prepare_warning {
        println!(
            "  - note: native reflection binding unavailable in bench, fallback handler used ({})",
            warning
        );
    } else if iterations > 0 {
        println!(
            "  - native reflection binding used in {}/{} measured iteration(s)",
            native_bound_measurements, iterations
        );
    }

    let report = BenchReport {
        schema_version: 1,
        kind: "reflection".to_string(),
        suite: suite.to_string(),
        generated_at_unix_ms: now_unix_ms(),
        cases: vec![
            BenchCaseResult {
                name: "disabled".to_string(),
                iterations,
                warmup,
                sample_ms: disabled_samples.clone(),
                p50_ms: percentile(&disabled_samples, 0.50),
                p95_ms: percentile(&disabled_samples, 0.95),
                phases: None,
                total_ms: Some(avg(&disabled_samples)),
                before_ms: None,
                after_ms: None,
                cache_reused_modules: None,
                generic_cache_hit_ratio: None,
                generic_rebuilt_instances: None,
                frontend_scheduler: None,
                frontend_fallback_events: None,
                frontend_planner_trace: None,
            },
            BenchCaseResult {
                name: "enabled-unused".to_string(),
                iterations,
                warmup,
                sample_ms: enabled_unused_samples.clone(),
                p50_ms: percentile(&enabled_unused_samples, 0.50),
                p95_ms: percentile(&enabled_unused_samples, 0.95),
                phases: None,
                total_ms: Some(avg(&enabled_unused_samples)),
                before_ms: None,
                after_ms: None,
                cache_reused_modules: None,
                generic_cache_hit_ratio: None,
                generic_rebuilt_instances: None,
                frontend_scheduler: None,
                frontend_fallback_events: None,
                frontend_planner_trace: None,
            },
            BenchCaseResult {
                name: "enabled-used".to_string(),
                iterations,
                warmup,
                sample_ms: enabled_used_samples.clone(),
                p50_ms: percentile(&enabled_used_samples, 0.50),
                p95_ms: percentile(&enabled_used_samples, 0.95),
                phases: None,
                total_ms: Some(avg(&enabled_used_samples)),
                before_ms: None,
                after_ms: None,
                cache_reused_modules: None,
                generic_cache_hit_ratio: None,
                generic_rebuilt_instances: None,
                frontend_scheduler: None,
                frontend_fallback_events: None,
                frontend_planner_trace: None,
            },
        ],
    };

    let out = write_bench_report(&report)?;
    println!("Reflection benchmark report: {}", out.to_string_lossy());
    for line in diff_report_against_baseline(&report) {
        println!("  baseline: {}", line);
    }
    Ok(())
}
