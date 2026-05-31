use crate::{
    canonical_or_lossy, default_build_output_path_for_case, find_clang, find_runtime_c,
    maybe_prepare_reflection_native_library, measure_reflection_used_ms,
    reflection_sidecar_path_for_artifact,
};
use miette::Result;

use super::{
    collect_bench_cases, diff_report_against_baseline, measure_sgc_command_ms, now_unix_ms,
    percentile, resolve_bench_suite_path, write_bench_report, BenchCaseResult, BenchReport,
};

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
                peak_rss_bytes: None,
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
                peak_rss_bytes: None,
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
                peak_rss_bytes: None,
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
