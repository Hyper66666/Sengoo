use crate::{
    collect_module_graph_snapshot, compile_native_binary,
    compile_source_to_llvm_file_with_phase_timings, expand_imports_for_source, find_clang,
    find_runtime_c, FrontendFallbackEvent, FrontendJobs, FrontendProbeMode,
    FrontendSchedulerTelemetry,
};
use miette::{IntoDiagnostic, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

mod incremental;
mod reflection;

pub(crate) use incremental::cmd_bench_incremental;
pub(crate) use reflection::cmd_bench_reflection;

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
        let root_source = fs::read_to_string(&case)
            .into_diagnostic()
            .map_err(|e| miette::miette!("failed to read benchmark case {}: {}", case_name, e))?;
        let source = expand_imports_for_source(&case, &root_source)?;
        let frontend_snapshot = collect_module_graph_snapshot(
            &case,
            &root_source,
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

fn phase_is_timing_metric(phase: &str) -> bool {
    !phase.ends_with("_removed")
}
