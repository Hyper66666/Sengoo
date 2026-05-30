#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


DEFAULT_MAX_REAL_INCREMENTAL_MS = 200.0
DEFAULT_MAX_FULL_BUILD_100K_MS = 2000.0
DEFAULT_MAX_FULL_BUILD_1000K_MS = 7000.0
DEFAULT_MAX_FRONTEND_100K_MS = 300.0
DEFAULT_MAX_FRONTEND_1000K_MS = 7000.0
DEFAULT_MAX_CODEGEN_100K_MS = 1500.0
DEFAULT_MAX_LINK_100K_MS = 500.0
DEFAULT_MAX_DAEMON_REGRESSION_MS = 50.0
DEFAULT_MAX_SENGOO_RSS_100K_MB = 300.0
DEFAULT_MAX_SENGOO_RSS_1000K_MB = 1800.0
DEFAULT_MAX_FRONTEND_100K_REGRESSION_PCT = 12.0
DEFAULT_MAX_FRONTEND_1000K_REGRESSION_PCT = 12.0
DEFAULT_MAX_FULL_BUILD_100K_REGRESSION_PCT = 12.0
DEFAULT_MAX_FULL_BUILD_1000K_REGRESSION_PCT = 12.0
DEFAULT_MAX_RSS_100K_REGRESSION_PCT = 12.0
DEFAULT_MAX_RSS_1000K_REGRESSION_PCT = 12.0
DEFAULT_REQUIRED_REACHABILITY_PROFILES = (
    "all_reachable",
    "half_reachable",
    "library_entryless",
)
DEFAULT_REQUIRED_INCREMENTAL_SCENARIOS = (
    "loop_body_change",
    "function_signature_change",
    "add_new_function",
)
DEFAULT_REQUIRED_SCALE_LOCS = ("1000", "10000", "100000", "1000000")
DEFAULT_REQUIRED_MEMORY_LOCS = ("10000", "100000", "1000000")
DEFAULT_BASELINE_PROFILE = (
    Path(__file__).resolve().parent.parent / "frontend-memory-baseline.json"
)


def load_report(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise RuntimeError(f"advanced KPI sample not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"failed to parse advanced KPI sample {path}: {exc}") from exc


def load_baseline_profile(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise RuntimeError(f"frontend baseline profile not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"failed to parse frontend baseline profile {path}: {exc}") from exc


def baseline_metric(
    baseline_profile: dict[str, Any],
    bucket: str,
    metric_key: str,
) -> float | None:
    metrics = baseline_profile.get("metrics")
    if not isinstance(metrics, dict):
        return None
    bucket_metrics = metrics.get(bucket)
    if not isinstance(bucket_metrics, dict):
        return None
    value = bucket_metrics.get(metric_key)
    if not isinstance(value, (int, float)):
        return None
    return float(value)


def evaluate_report(
    report: dict[str, Any],
    baseline_profile: dict[str, Any],
    max_real_incremental_ms: float,
    max_full_build_100k_ms: float,
    max_full_build_1000k_ms: float,
    max_frontend_100k_ms: float,
    max_frontend_1000k_ms: float,
    max_codegen_100k_ms: float,
    max_link_100k_ms: float,
    max_daemon_regression_ms: float,
    max_sengoo_rss_100k_mb: float,
    max_sengoo_rss_1000k_mb: float,
    max_frontend_100k_regression_pct: float,
    max_frontend_1000k_regression_pct: float,
    max_full_build_100k_regression_pct: float,
    max_full_build_1000k_regression_pct: float,
    max_rss_100k_regression_pct: float,
    max_rss_1000k_regression_pct: float,
    require_phase_deltas: bool,
    require_daemon_comparison: bool,
    skip_memory_compare: bool,
    fail_fast: bool,
) -> tuple[list[str], list[str], dict[str, Any]]:
    summaries: list[str] = []
    violations: list[str] = []
    decision: dict[str, Any] = {
        "baseline_report_id": baseline_profile.get("baseline_report_id"),
        "baseline_report_path": baseline_profile.get("baseline_report_path"),
        "thresholds": {
            "frontend_regression_pct": {
                "100000": float(max_frontend_100k_regression_pct),
                "1000000": float(max_frontend_1000k_regression_pct),
            },
            "full_build_regression_pct": {
                "100000": float(max_full_build_100k_regression_pct),
                "1000000": float(max_full_build_1000k_regression_pct),
            },
            "rss_regression_pct": {
                "100000": float(max_rss_100k_regression_pct),
                "1000000": float(max_rss_1000k_regression_pct),
            },
            "full_build_budget_ms": {
                "100000": float(max_full_build_100k_ms),
                "1000000": float(max_full_build_1000k_ms),
            },
        },
        "comparisons": [],
    }

    def add_violation(message: str) -> bool:
        violations.append(message)
        return fail_fast

    def check_regression_vs_baseline(
        *,
        bucket: str,
        metric_key: str,
        measured_value: float,
        metric_label: str,
        max_regression_pct: float,
    ) -> bool:
        baseline_value = baseline_metric(baseline_profile, bucket, metric_key)
        if baseline_value is None:
            return add_violation(f"missing baseline metrics/{bucket}/{metric_key}")

        if baseline_value <= 0:
            return add_violation(
                f"invalid baseline metrics/{bucket}/{metric_key}: {baseline_value}"
            )

        delta_pct = ((measured_value - baseline_value) / baseline_value) * 100.0
        comparison = {
            "bucket": bucket,
            "metric": metric_key,
            "metric_label": metric_label,
            "measured": float(measured_value),
            "baseline": float(baseline_value),
            "delta_pct": float(delta_pct),
            "max_regression_pct": float(max_regression_pct),
            "pass": delta_pct <= max_regression_pct,
        }
        decision["comparisons"].append(comparison)
        summaries.append(
            f"{metric_label}/{bucket}: measured={measured_value:.2f} baseline={baseline_value:.2f} "
            f"delta={delta_pct:+.2f}% limit<={max_regression_pct:.2f}%"
        )

        if delta_pct > max_regression_pct:
            return add_violation(
                f"{metric_label}/{bucket} regression exceeded limit "
                f"(measured={measured_value:.2f}, baseline={baseline_value:.2f}, "
                f"delta={delta_pct:+.2f}% > {max_regression_pct:.2f}%)"
            )
        return False

    real_incremental = report.get("real_incremental")
    if not isinstance(real_incremental, dict) or not real_incremental:
        if add_violation("missing real_incremental block"):
            return summaries, violations, decision
    else:
        for scenario in DEFAULT_REQUIRED_INCREMENTAL_SCENARIOS:
            if scenario not in real_incremental:
                if add_violation(f"missing real_incremental/{scenario} block"):
                    return summaries, violations, decision

        for scenario in sorted(real_incremental.keys()):
            metrics = real_incremental.get(scenario)
            if not isinstance(metrics, dict):
                if add_violation(f"real_incremental/{scenario} is not an object"):
                    return summaries, violations, decision
                continue
            sengoo = metrics.get("sengoo")
            if not isinstance(sengoo, dict):
                if add_violation(f"real_incremental/{scenario}/sengoo is missing"):
                    return summaries, violations, decision
                continue
            after_avg = sengoo.get("after_avg_ms")
            if not isinstance(after_avg, (int, float)):
                if add_violation(f"real_incremental/{scenario}/sengoo/after_avg_ms is missing"):
                    return summaries, violations, decision
                continue
            summaries.append(
                f"real_incremental/{scenario}: after={after_avg:.2f}ms target<={max_real_incremental_ms:.2f}ms"
            )
            if float(after_avg) > max_real_incremental_ms:
                if add_violation(
                    f"real_incremental/{scenario} exceeded target ({after_avg:.2f}ms > {max_real_incremental_ms:.2f}ms)"
                ):
                    return summaries, violations, decision

    scale_curve = report.get("scale_curve")
    if not isinstance(scale_curve, dict):
        if add_violation("missing scale_curve block"):
            return summaries, violations, decision
    else:
        for loc in DEFAULT_REQUIRED_SCALE_LOCS:
            loc_metrics = scale_curve.get(loc)
            if not isinstance(loc_metrics, dict):
                if add_violation(f"missing scale_curve/{loc} block"):
                    return summaries, violations, decision
                continue

            sengoo = loc_metrics.get("sengoo")
            if not isinstance(sengoo, dict):
                if add_violation(f"missing scale_curve/{loc}/sengoo block"):
                    return summaries, violations, decision
                continue

            frontend_ms = sengoo.get("compile_frontend_llvm_avg_ms")
            codegen_ms = sengoo.get("codegen_obj_avg_ms")
            link_ms = sengoo.get("link_avg_ms")
            full_build = sengoo.get("e2e_avg_ms")

            if isinstance(frontend_ms, (int, float)):
                summaries.append(f"scale/{loc}/frontend: {float(frontend_ms):.2f}ms")
            else:
                if add_violation(f"missing scale_curve/{loc}/sengoo/compile_frontend_llvm_avg_ms"):
                    return summaries, violations, decision
            if isinstance(codegen_ms, (int, float)):
                summaries.append(f"scale/{loc}/codegen: {float(codegen_ms):.2f}ms")
            else:
                if add_violation(f"missing scale_curve/{loc}/sengoo/codegen_obj_avg_ms"):
                    return summaries, violations, decision
            if isinstance(link_ms, (int, float)):
                summaries.append(f"scale/{loc}/link: {float(link_ms):.2f}ms")
            else:
                if add_violation(f"missing scale_curve/{loc}/sengoo/link_avg_ms"):
                    return summaries, violations, decision

            if loc == "1000000" and isinstance(frontend_ms, (int, float)):
                summaries.append(
                    f"scale/1000000/frontend budget: {float(frontend_ms):.2f}ms target<={max_frontend_1000k_ms:.2f}ms"
                )
                if float(frontend_ms) > max_frontend_1000k_ms:
                    if add_violation(
                        f"scale/1000000 frontend exceeded target ({float(frontend_ms):.2f}ms > {max_frontend_1000k_ms:.2f}ms)"
                    ):
                        return summaries, violations, decision
                if check_regression_vs_baseline(
                    bucket="1000000",
                    metric_key="compile_frontend_llvm_avg_ms",
                    measured_value=float(frontend_ms),
                    metric_label="frontend_time",
                    max_regression_pct=max_frontend_1000k_regression_pct,
                ):
                    return summaries, violations, decision
            if loc == "1000000" and isinstance(full_build, (int, float)):
                summaries.append(
                    f"scale/1000000/full_build: e2e={float(full_build):.2f}ms target<={max_full_build_1000k_ms:.2f}ms"
                )
                if float(full_build) > max_full_build_1000k_ms:
                    if add_violation(
                        f"scale/1000000 full build exceeded target ({float(full_build):.2f}ms > {max_full_build_1000k_ms:.2f}ms)"
                    ):
                        return summaries, violations, decision
                if check_regression_vs_baseline(
                    bucket="1000000",
                    metric_key="e2e_avg_ms",
                    measured_value=float(full_build),
                    metric_label="full_build_time",
                    max_regression_pct=max_full_build_1000k_regression_pct,
                ):
                    return summaries, violations, decision

            if loc != "100000":
                continue

            if isinstance(full_build, (int, float)):
                summaries.append(
                    f"scale/100000/full_build: e2e={float(full_build):.2f}ms target<={max_full_build_100k_ms:.2f}ms"
                )
                if float(full_build) > max_full_build_100k_ms:
                    if add_violation(
                        f"scale/100000 full build exceeded target ({float(full_build):.2f}ms > {max_full_build_100k_ms:.2f}ms)"
                    ):
                        return summaries, violations, decision
                if check_regression_vs_baseline(
                    bucket="100000",
                    metric_key="e2e_avg_ms",
                    measured_value=float(full_build),
                    metric_label="full_build_time",
                    max_regression_pct=max_full_build_100k_regression_pct,
                ):
                    return summaries, violations, decision
            else:
                if add_violation("missing scale_curve/100000/sengoo/e2e_avg_ms"):
                    return summaries, violations, decision

            if isinstance(frontend_ms, (int, float)):
                summaries.append(
                    f"scale/100000/frontend budget: {float(frontend_ms):.2f}ms target<={max_frontend_100k_ms:.2f}ms"
                )
                if float(frontend_ms) > max_frontend_100k_ms:
                    if add_violation(
                        f"scale/100000 frontend exceeded target ({float(frontend_ms):.2f}ms > {max_frontend_100k_ms:.2f}ms)"
                    ):
                        return summaries, violations, decision
                if check_regression_vs_baseline(
                    bucket="100000",
                    metric_key="compile_frontend_llvm_avg_ms",
                    measured_value=float(frontend_ms),
                    metric_label="frontend_time",
                    max_regression_pct=max_frontend_100k_regression_pct,
                ):
                    return summaries, violations, decision

            if isinstance(codegen_ms, (int, float)):
                summaries.append(
                    f"scale/100000/codegen budget: {float(codegen_ms):.2f}ms target<={max_codegen_100k_ms:.2f}ms"
                )
                if float(codegen_ms) > max_codegen_100k_ms:
                    if add_violation(
                        f"scale/100000 codegen exceeded target ({float(codegen_ms):.2f}ms > {max_codegen_100k_ms:.2f}ms)"
                    ):
                        return summaries, violations, decision

            if isinstance(link_ms, (int, float)):
                summaries.append(
                    f"scale/100000/link budget: {float(link_ms):.2f}ms target<={max_link_100k_ms:.2f}ms"
                )
                if float(link_ms) > max_link_100k_ms:
                    if add_violation(
                        f"scale/100000 link exceeded target ({float(link_ms):.2f}ms > {max_link_100k_ms:.2f}ms)"
                    ):
                        return summaries, violations, decision

    if not skip_memory_compare:
        compile_memory_compare = report.get("compile_memory_compare")
        if not isinstance(compile_memory_compare, dict):
            if add_violation("missing compile_memory_compare block"):
                return summaries, violations, decision
        else:
            for loc in DEFAULT_REQUIRED_MEMORY_LOCS:
                loc_metrics = compile_memory_compare.get(loc)
                if not isinstance(loc_metrics, dict):
                    if add_violation(f"missing compile_memory_compare/{loc} block"):
                        return summaries, violations, decision
                    continue

                for lang in ("sengoo", "cpp", "rust", "python"):
                    lang_metrics = loc_metrics.get(lang)
                    if not isinstance(lang_metrics, dict):
                        if add_violation(f"missing compile_memory_compare/{loc}/{lang} block"):
                            return summaries, violations, decision
                        continue
                    rss_mb = lang_metrics.get("peak_rss_mb_avg")
                    if not isinstance(rss_mb, (int, float)):
                        if add_violation(
                            f"missing compile_memory_compare/{loc}/{lang}/peak_rss_mb_avg"
                        ):
                            return summaries, violations, decision
                        continue
                    summaries.append(f"memory/{loc}/{lang}/rss: {float(rss_mb):.2f}MB")

                sengoo_metrics = loc_metrics.get("sengoo", {})
                if not isinstance(sengoo_metrics, dict):
                    continue
                sengoo_rss = sengoo_metrics.get("peak_rss_mb_avg")
                if not isinstance(sengoo_rss, (int, float)):
                    if add_violation(f"missing compile_memory_compare/{loc}/sengoo/peak_rss_mb_avg"):
                        return summaries, violations, decision
                    continue

                if loc == "100000":
                    summaries.append(
                        f"memory/100000/sengoo budget: {float(sengoo_rss):.2f}MB "
                        f"target<={max_sengoo_rss_100k_mb:.2f}MB"
                    )
                    if float(sengoo_rss) > max_sengoo_rss_100k_mb:
                        if add_violation(
                            "compile_memory_compare/100000/sengoo peak RSS exceeded target "
                            f"({float(sengoo_rss):.2f}MB > {max_sengoo_rss_100k_mb:.2f}MB)"
                        ):
                            return summaries, violations, decision
                    if check_regression_vs_baseline(
                        bucket="100000",
                        metric_key="peak_rss_mb_avg",
                        measured_value=float(sengoo_rss),
                        metric_label="frontend_rss",
                        max_regression_pct=max_rss_100k_regression_pct,
                    ):
                        return summaries, violations, decision

                if loc == "1000000":
                    summaries.append(
                        f"memory/1000000/sengoo budget: {float(sengoo_rss):.2f}MB "
                        f"target<={max_sengoo_rss_1000k_mb:.2f}MB"
                    )
                    if float(sengoo_rss) > max_sengoo_rss_1000k_mb:
                        if add_violation(
                            "compile_memory_compare/1000000/sengoo peak RSS exceeded target "
                            f"({float(sengoo_rss):.2f}MB > {max_sengoo_rss_1000k_mb:.2f}MB)"
                        ):
                            return summaries, violations, decision
                    if check_regression_vs_baseline(
                        bucket="1000000",
                        metric_key="peak_rss_mb_avg",
                        measured_value=float(sengoo_rss),
                        metric_label="frontend_rss",
                        max_regression_pct=max_rss_1000k_regression_pct,
                    ):
                        return summaries, violations, decision

    if require_phase_deltas:
        phase_deltas = report.get("phase_deltas")
        if not isinstance(phase_deltas, dict):
            if add_violation("missing phase_deltas block"):
                return summaries, violations, decision
        else:
            if not isinstance(phase_deltas.get("incremental_vs_target_ms"), dict):
                if add_violation("missing phase_deltas/incremental_vs_target_ms"):
                    return summaries, violations, decision
            if not isinstance(phase_deltas.get("scale_100k_vs_target_ms"), dict):
                if add_violation("missing phase_deltas/scale_100k_vs_target_ms"):
                    return summaries, violations, decision

    daemon_comparison = report.get("daemon_comparison")
    if require_daemon_comparison and not isinstance(daemon_comparison, dict):
        if add_violation("missing daemon_comparison block"):
            return summaries, violations, decision

    if isinstance(daemon_comparison, dict):
        for scenario in DEFAULT_REQUIRED_INCREMENTAL_SCENARIOS:
            metrics = daemon_comparison.get(scenario)
            if not isinstance(metrics, dict):
                if add_violation(f"missing daemon_comparison/{scenario} block"):
                    return summaries, violations, decision
                continue
            oneshot_after = metrics.get("oneshot_after_avg_ms")
            daemon_after = metrics.get("daemon_after_avg_ms")
            if not isinstance(oneshot_after, (int, float)) or not isinstance(daemon_after, (int, float)):
                if add_violation(
                    f"daemon_comparison/{scenario} missing oneshot_after_avg_ms or daemon_after_avg_ms"
                ):
                    return summaries, violations, decision
                continue
            regression = float(daemon_after) - float(oneshot_after)
            summaries.append(
                f"daemon/{scenario}: delta={regression:.2f}ms target<={max_daemon_regression_ms:.2f}ms"
            )
            if regression > max_daemon_regression_ms:
                if add_violation(
                    f"daemon/{scenario} regression too high ({regression:.2f}ms > {max_daemon_regression_ms:.2f}ms)"
                ):
                    return summaries, violations, decision

    reachability_matrix = report.get("reachability_matrix")
    if not isinstance(reachability_matrix, dict):
        if add_violation("missing reachability_matrix block"):
            return summaries, violations, decision
    else:
        for profile in DEFAULT_REQUIRED_REACHABILITY_PROFILES:
            metrics = reachability_matrix.get(profile)
            if not isinstance(metrics, dict):
                if add_violation(f"missing reachability_matrix/{profile} block"):
                    return summaries, violations, decision
                continue

            frontend_ms = metrics.get("compile_frontend_llvm_avg_ms")
            codegen_ms = metrics.get("codegen_obj_avg_ms")
            e2e_ms = metrics.get("e2e_avg_ms")
            link_ms = metrics.get("link_avg_ms")

            if not isinstance(frontend_ms, (int, float)):
                if add_violation(f"missing reachability_matrix/{profile}/compile_frontend_llvm_avg_ms"):
                    return summaries, violations, decision
                continue
            if not isinstance(codegen_ms, (int, float)):
                if add_violation(f"missing reachability_matrix/{profile}/codegen_obj_avg_ms"):
                    return summaries, violations, decision
                continue
            if not isinstance(e2e_ms, (int, float)):
                if add_violation(f"missing reachability_matrix/{profile}/e2e_avg_ms"):
                    return summaries, violations, decision
                continue

            if profile != "library_entryless" and not isinstance(link_ms, (int, float)):
                if add_violation(f"missing reachability_matrix/{profile}/link_avg_ms"):
                    return summaries, violations, decision
                continue

            if profile == "library_entryless":
                summaries.append(
                    "reachability/library_entryless: "
                    f"frontend={float(frontend_ms):.2f}ms codegen={float(codegen_ms):.2f}ms "
                    f"e2e={float(e2e_ms):.2f}ms"
                )
            else:
                summaries.append(
                    f"reachability/{profile}: frontend={float(frontend_ms):.2f}ms "
                    f"codegen={float(codegen_ms):.2f}ms link={float(link_ms):.2f}ms "
                    f"e2e={float(e2e_ms):.2f}ms"
                )

            if profile == "all_reachable":
                summaries.append(
                    f"reachability/all_reachable frontend budget: {float(frontend_ms):.2f}ms "
                    f"target<={max_frontend_100k_ms:.2f}ms"
                )
                if float(frontend_ms) > max_frontend_100k_ms:
                    if add_violation(
                        "reachability/all_reachable frontend exceeded target "
                        f"({float(frontend_ms):.2f}ms > {max_frontend_100k_ms:.2f}ms)"
                    ):
                        return summaries, violations, decision

        delta_block = reachability_matrix.get("delta_vs_all_reachable_ms")
        if not isinstance(delta_block, dict):
            if add_violation("missing reachability_matrix/delta_vs_all_reachable_ms block"):
                return summaries, violations, decision

    return summaries, violations, decision


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate Sengoo advanced benchmark KPI targets.",
    )
    parser.add_argument(
        "--mode",
        choices=["soft", "hard"],
        default="soft",
        help="hard mode exits non-zero on KPI violations",
    )
    parser.add_argument(
        "--sample",
        required=True,
        help="path to advanced benchmark json report",
    )
    parser.add_argument(
        "--baseline-profile",
        default=str(DEFAULT_BASELINE_PROFILE),
        help="path to pinned frontend baseline profile json",
    )
    parser.add_argument(
        "--max-real-incremental-ms",
        type=float,
        default=DEFAULT_MAX_REAL_INCREMENTAL_MS,
    )
    parser.add_argument(
        "--max-full-build-100k-ms",
        type=float,
        default=DEFAULT_MAX_FULL_BUILD_100K_MS,
    )
    parser.add_argument(
        "--max-full-build-1000k-ms",
        type=float,
        default=DEFAULT_MAX_FULL_BUILD_1000K_MS,
    )
    parser.add_argument(
        "--max-frontend-100k-ms",
        type=float,
        default=DEFAULT_MAX_FRONTEND_100K_MS,
    )
    parser.add_argument(
        "--max-frontend-1000k-ms",
        type=float,
        default=DEFAULT_MAX_FRONTEND_1000K_MS,
    )
    parser.add_argument(
        "--max-codegen-100k-ms",
        type=float,
        default=DEFAULT_MAX_CODEGEN_100K_MS,
    )
    parser.add_argument(
        "--max-link-100k-ms",
        type=float,
        default=DEFAULT_MAX_LINK_100K_MS,
    )
    parser.add_argument(
        "--max-daemon-regression-ms",
        type=float,
        default=DEFAULT_MAX_DAEMON_REGRESSION_MS,
        help="maximum allowed daemon-after minus oneshot-after regression per scenario",
    )
    parser.add_argument(
        "--max-sengoo-rss-100k-mb",
        type=float,
        default=DEFAULT_MAX_SENGOO_RSS_100K_MB,
    )
    parser.add_argument(
        "--max-sengoo-rss-1000k-mb",
        type=float,
        default=DEFAULT_MAX_SENGOO_RSS_1000K_MB,
    )
    parser.add_argument(
        "--max-frontend-100k-regression-pct",
        type=float,
        default=DEFAULT_MAX_FRONTEND_100K_REGRESSION_PCT,
    )
    parser.add_argument(
        "--max-frontend-1000k-regression-pct",
        type=float,
        default=DEFAULT_MAX_FRONTEND_1000K_REGRESSION_PCT,
    )
    parser.add_argument(
        "--max-full-build-100k-regression-pct",
        type=float,
        default=DEFAULT_MAX_FULL_BUILD_100K_REGRESSION_PCT,
    )
    parser.add_argument(
        "--max-full-build-1000k-regression-pct",
        type=float,
        default=DEFAULT_MAX_FULL_BUILD_1000K_REGRESSION_PCT,
    )
    parser.add_argument(
        "--max-rss-100k-regression-pct",
        type=float,
        default=DEFAULT_MAX_RSS_100K_REGRESSION_PCT,
    )
    parser.add_argument(
        "--max-rss-1000k-regression-pct",
        type=float,
        default=DEFAULT_MAX_RSS_1000K_REGRESSION_PCT,
    )
    parser.add_argument(
        "--require-phase-deltas",
        action="store_true",
        help="require phase_deltas block in the report",
    )
    parser.add_argument(
        "--require-daemon-comparison",
        action="store_true",
        help="require daemon_comparison block in the report",
    )
    parser.add_argument(
        "--fail-fast",
        action="store_true",
        help="stop evaluating after the first violation",
    )
    parser.add_argument(
        "--skip-memory-compare",
        action="store_true",
        help="skip compile_memory_compare validation",
    )
    parser.add_argument(
        "--decision-out",
        help="optional path for machine-readable gate decision json",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    sample_path = Path(args.sample).expanduser().resolve()
    baseline_profile_path = Path(args.baseline_profile).expanduser().resolve()
    report = load_report(sample_path)
    baseline_profile = load_baseline_profile(baseline_profile_path)
    summaries, violations, decision = evaluate_report(
        report,
        baseline_profile=baseline_profile,
        max_real_incremental_ms=float(args.max_real_incremental_ms),
        max_full_build_100k_ms=float(args.max_full_build_100k_ms),
        max_full_build_1000k_ms=float(args.max_full_build_1000k_ms),
        max_frontend_100k_ms=float(args.max_frontend_100k_ms),
        max_frontend_1000k_ms=float(args.max_frontend_1000k_ms),
        max_codegen_100k_ms=float(args.max_codegen_100k_ms),
        max_link_100k_ms=float(args.max_link_100k_ms),
        max_daemon_regression_ms=float(args.max_daemon_regression_ms),
        max_sengoo_rss_100k_mb=float(args.max_sengoo_rss_100k_mb),
        max_sengoo_rss_1000k_mb=float(args.max_sengoo_rss_1000k_mb),
        max_frontend_100k_regression_pct=float(args.max_frontend_100k_regression_pct),
        max_frontend_1000k_regression_pct=float(args.max_frontend_1000k_regression_pct),
        max_full_build_100k_regression_pct=float(args.max_full_build_100k_regression_pct),
        max_full_build_1000k_regression_pct=float(args.max_full_build_1000k_regression_pct),
        max_rss_100k_regression_pct=float(args.max_rss_100k_regression_pct),
        max_rss_1000k_regression_pct=float(args.max_rss_1000k_regression_pct),
        require_phase_deltas=bool(args.require_phase_deltas),
        require_daemon_comparison=bool(args.require_daemon_comparison),
        skip_memory_compare=bool(args.skip_memory_compare),
        fail_fast=bool(args.fail_fast),
    )

    print(f"advanced-kpi-gate mode={args.mode} sample={sample_path}")
    print(f"  baseline-profile={baseline_profile_path}")
    for line in summaries:
        print(f"  {line}")

    decision_payload = {
        "schema_version": 1,
        "mode": args.mode,
        "sample": str(sample_path),
        "baseline_profile": str(baseline_profile_path),
        "baseline_report_id": decision.get("baseline_report_id"),
        "baseline_report_path": decision.get("baseline_report_path"),
        "thresholds": decision.get("thresholds", {}),
        "comparisons": decision.get("comparisons", []),
        "violations": violations,
        "gate_decision": "pass" if not violations else "fail",
    }
    decision_out = (
        Path(args.decision_out).expanduser().resolve()
        if args.decision_out
        else sample_path.with_name(f"{sample_path.stem}-advanced-gate.json")
    )
    decision_out.write_text(json.dumps(decision_payload, indent=2), encoding="utf-8")
    print(f"  decision-artifact={decision_out}")

    if not violations:
        print("advanced-kpi-gate PASS")
        return 0

    print(f"advanced-kpi-gate found {len(violations)} violation(s):")
    for violation in violations:
        print(f"  - {violation}")

    if args.mode == "hard":
        print("advanced-kpi-gate HARD failure", file=sys.stderr)
        return 1

    print("advanced-kpi-gate SOFT warning (not failing build)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
