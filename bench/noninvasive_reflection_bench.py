#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


DEFAULT_SUITE_FILE = "suites/reflection/noninvasive_reflection.sg"
DEFAULT_WARMUP = 1
DEFAULT_ITERATIONS = 5
DEFAULT_OPT_LEVEL = 2


def now_unix_ms() -> int:
    return int(time.time() * 1000)


def percentile(values: list[float], p: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    idx = int(math.ceil((len(ordered) - 1) * p))
    return ordered[idx]


def average(values: list[float]) -> float | None:
    if not values:
        return None
    return sum(values) / len(values)


def resolve_sengoo_root(bench_root: Path) -> Path:
    env_root = os.environ.get("SENGOO_ROOT")
    if env_root:
        root = Path(env_root).expanduser().resolve()
        if not (root / "Cargo.toml").exists():
            raise RuntimeError(f"SENGOO_ROOT does not look like Sengoo root: {root}")
        return root

    candidate = bench_root.parent
    if (candidate / "Cargo.toml").exists() and (candidate / "tools" / "sgc" / "src" / "main.rs").exists():
        return candidate

    raise RuntimeError("cannot resolve Sengoo source root; set SENGOO_ROOT")


def exe_name(stem: str) -> str:
    return f"{stem}.exe" if sys.platform.startswith("win") else stem


def resolve_sgc_binary(sengoo_root: Path) -> Path:
    candidates = [
        sengoo_root / "target" / "release" / exe_name("sgc"),
        sengoo_root / "target" / "debug" / exe_name("sgc"),
    ]
    existing = [candidate for candidate in candidates if candidate.exists()]
    if not existing:
        raise RuntimeError("sgc binary not found; run `cargo build -p sgc --release` first")
    for candidate in existing:
        if supports_reflection_bench(candidate):
            return candidate
    raise RuntimeError(
        "found sgc binary, but it does not support `bench reflection`; rebuild sgc from latest source"
    )


def supports_reflection_bench(binary: Path) -> bool:
    proc = subprocess.run(
        [str(binary), "bench", "--help"],
        text=True,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
    )
    if proc.returncode != 0:
        return False
    return "reflection" in proc.stdout.lower()


def run_checked(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess[str]:
    proc = subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        text=True,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
    )
    if proc.returncode != 0:
        raise RuntimeError(
            f"command failed ({proc.returncode}): {' '.join(cmd)}\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
    return proc


def latest_reflection_report(results_dir: Path, since_ms: int, expected_suite: str) -> Path:
    reports = sorted(
        results_dir.glob("*-reflection*.json"),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    for report in reports:
        if int(report.stat().st_mtime * 1000) < since_ms:
            continue
        try:
            payload = json.loads(report.read_text(encoding="utf-8-sig"))
        except Exception:
            continue
        if payload.get("kind") != "reflection":
            continue
        suite = payload.get("suite")
        if isinstance(suite, str) and suite == expected_suite:
            return report

    # Fallback: newest reflection report newer than `since_ms`.
    for report in reports:
        if int(report.stat().st_mtime * 1000) < since_ms:
            continue
        try:
            payload = json.loads(report.read_text(encoding="utf-8-sig"))
        except Exception:
            continue
        if payload.get("kind") == "reflection":
            return report

    # Final fallback: newest reflection report regardless of timestamp.
    for report in reports:
        try:
            payload = json.loads(report.read_text(encoding="utf-8-sig"))
        except Exception:
            continue
        if payload.get("kind") == "reflection":
            return report
    raise RuntimeError(f"no reflection report found in {results_dir}")


def case_metric(case: dict[str, Any]) -> float | None:
    p50 = case.get("p50_ms")
    if isinstance(p50, (int, float)):
        return float(p50)
    total = case.get("total_ms")
    if isinstance(total, (int, float)):
        return float(total)
    samples = case.get("sample_ms")
    if isinstance(samples, list):
        nums = [float(v) for v in samples if isinstance(v, (int, float))]
        if nums:
            return average(nums)
    return None


def parse_reflection_report(path: Path) -> dict[str, Any]:
    payload = json.loads(path.read_text(encoding="utf-8-sig"))
    if payload.get("kind") != "reflection":
        raise RuntimeError(f"unexpected report kind in {path}: {payload.get('kind')!r}")

    case_map: dict[str, dict[str, Any]] = {}
    for case in payload.get("cases", []):
        if isinstance(case, dict):
            name = case.get("name")
            if isinstance(name, str):
                case_map[name] = case

    for name in ("disabled", "enabled-unused", "enabled-used"):
        if name not in case_map:
            raise RuntimeError(f"missing case {name!r} in report {path}")

    disabled = case_metric(case_map["disabled"])
    enabled_unused = case_metric(case_map["enabled-unused"])
    enabled_used = case_metric(case_map["enabled-used"])
    if disabled is None or enabled_unused is None or enabled_used is None:
        raise RuntimeError(f"missing usable numeric metric in report {path}")
    if disabled <= 0.0:
        raise RuntimeError(f"disabled metric <= 0 in report {path}")

    enabled_unused_overhead_pct = ((enabled_unused - disabled) / disabled) * 100.0
    enabled_used_overhead_pct = ((enabled_used - disabled) / disabled) * 100.0

    return {
        "disabled_ms": disabled,
        "enabled_unused_ms": enabled_unused,
        "enabled_used_ms": enabled_used,
        "enabled_unused_overhead_pct": enabled_unused_overhead_pct,
        "enabled_used_overhead_pct": enabled_used_overhead_pct,
        "raw_report": payload,
    }


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Run non-invasive reflection benchmark against sgc reflection suite."
    )
    parser.add_argument(
        "--suite",
        default=DEFAULT_SUITE_FILE,
        help=f"reflection suite file/path (default: {DEFAULT_SUITE_FILE})",
    )
    parser.add_argument("--warmup", type=int, default=DEFAULT_WARMUP)
    parser.add_argument("--iterations", type=int, default=DEFAULT_ITERATIONS)
    parser.add_argument("--opt-level", type=int, default=DEFAULT_OPT_LEVEL)
    parser.add_argument(
        "--max-enabled-unused-overhead-pct",
        type=float,
        default=25.0,
        help="fail if enabled-unused overhead exceeds this threshold",
    )
    parser.add_argument(
        "--max-enabled-used-overhead-pct",
        type=float,
        default=45.0,
        help="fail if enabled-used overhead exceeds this threshold",
    )
    parser.add_argument(
        "--strict",
        action="store_true",
        help="return non-zero if overhead threshold is exceeded",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    bench_root = Path(__file__).resolve().parent
    results_dir = bench_root / "results"
    results_dir.mkdir(parents=True, exist_ok=True)

    sengoo_root = resolve_sengoo_root(bench_root)
    sgc_bin = resolve_sgc_binary(sengoo_root)

    suite_path = Path(args.suite)
    if not suite_path.is_absolute():
        suite_path = (bench_root / suite_path).resolve()
    if not suite_path.exists():
        raise RuntimeError(f"suite path not found: {suite_path}")

    started_ms = now_unix_ms()
    cmd = [
        str(sgc_bin),
        "bench",
        "reflection",
        str(suite_path),
        "--warmup",
        str(args.warmup),
        "--iterations",
        str(args.iterations),
        "-O",
        str(args.opt_level),
    ]
    proc = run_checked(cmd, cwd=sengoo_root)
    print(proc.stdout, end="")
    if proc.stderr:
        print(proc.stderr, end="", file=sys.stderr)

    reflection_report = latest_reflection_report(results_dir, started_ms, str(suite_path))
    summary = parse_reflection_report(reflection_report)

    violations: list[str] = []
    if summary["enabled_unused_overhead_pct"] > args.max_enabled_unused_overhead_pct:
        violations.append(
            (
                f"enabled-unused overhead {summary['enabled_unused_overhead_pct']:.2f}% > "
                f"{args.max_enabled_unused_overhead_pct:.2f}%"
            )
        )
    if summary["enabled_used_overhead_pct"] > args.max_enabled_used_overhead_pct:
        violations.append(
            (
                f"enabled-used overhead {summary['enabled_used_overhead_pct']:.2f}% > "
                f"{args.max_enabled_used_overhead_pct:.2f}%"
            )
        )

    output = {
        "schema_version": 1,
        "kind": "noninvasive_reflection_bench",
        "generated_at_unix_ms": now_unix_ms(),
        "sengoo_root": str(sengoo_root),
        "suite": str(suite_path),
        "sgc_report_path": str(reflection_report),
        "metrics": {
            "disabled_ms": summary["disabled_ms"],
            "enabled_unused_ms": summary["enabled_unused_ms"],
            "enabled_used_ms": summary["enabled_used_ms"],
            "enabled_unused_overhead_pct": summary["enabled_unused_overhead_pct"],
            "enabled_used_overhead_pct": summary["enabled_used_overhead_pct"],
        },
        "thresholds": {
            "max_enabled_unused_overhead_pct": args.max_enabled_unused_overhead_pct,
            "max_enabled_used_overhead_pct": args.max_enabled_used_overhead_pct,
        },
        "violations": violations,
    }

    output_path = results_dir / f"{now_unix_ms()}-noninvasive-reflection-bench.json"
    output_path.write_text(json.dumps(output, ensure_ascii=False, indent=2), encoding="utf-8")

    print(f"[noninvasive-reflection] sgc report: {reflection_report}")
    print(f"[noninvasive-reflection] summary: {output_path}")
    print(
        (
            "[noninvasive-reflection] disabled={:.2f}ms "
            "enabled-unused-overhead={:+.2f}% enabled-used-overhead={:+.2f}%"
        ).format(
            summary["disabled_ms"],
            summary["enabled_unused_overhead_pct"],
            summary["enabled_used_overhead_pct"],
        )
    )

    if violations:
        print("[noninvasive-reflection] threshold violations:")
        for violation in violations:
            print(f"  - {violation}")
        if args.strict:
            return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
