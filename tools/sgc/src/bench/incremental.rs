use crate::{module_fingerprints_for_source, module_invalidation_stats};
use miette::{IntoDiagnostic, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};

use super::{
    collect_bench_cases, diff_report_against_baseline, measure_sgc_command_ms, now_unix_ms,
    resolve_bench_suite_path, write_bench_report, BenchCaseResult, BenchReport,
};
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
