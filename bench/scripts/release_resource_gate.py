#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import platform
import shutil
import statistics
import subprocess
import sys
import tempfile
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any


DEFAULT_BUDGETS = {
    "sgc_artifact_mib": 20.0,
    "program_artifact_mib": 5.0,
    "startup_avg_ms": 250.0,
    "check_avg_ms": 750.0,
    "full_build_ms": 5_000.0,
    "runtime_avg_ms": 250.0,
}


def sgc_command(
    sgc: Path, runtime_mode: str | None, *arguments: str
) -> list[str]:
    command = [str(sgc)]
    if runtime_mode is not None:
        command.extend(["--runtime-mode", runtime_mode])
    command.extend(arguments)
    return command


def run_timed(command: list[str], cwd: Path, timeout_seconds: float) -> tuple[float, str]:
    started = time.perf_counter()
    completed = subprocess.run(
        command,
        cwd=cwd,
        capture_output=True,
        text=True,
        timeout=timeout_seconds,
        check=False,
    )
    elapsed_ms = (time.perf_counter() - started) * 1_000.0
    if completed.returncode != 0:
        raise RuntimeError(
            f"command failed with exit {completed.returncode}: {' '.join(command)}\n"
            f"stdout:\n{completed.stdout}\nstderr:\n{completed.stderr}"
        )
    return elapsed_ms, (completed.stdout + completed.stderr).strip()


def measure_repeated(
    command: list[str], cwd: Path, iterations: int, timeout_seconds: float
) -> tuple[list[float], str]:
    samples: list[float] = []
    last_output = ""
    for _ in range(iterations):
        elapsed_ms, last_output = run_timed(command, cwd, timeout_seconds)
        samples.append(elapsed_ms)
    return samples, last_output


def summarize(samples: list[float]) -> dict[str, Any]:
    return {
        "samples_ms": samples,
        "avg_ms": statistics.fmean(samples),
        "median_ms": statistics.median(samples),
        "max_ms": max(samples),
    }


def full_build_budget_value(metrics: dict[str, Any]) -> float:
    summary = metrics.get("full_build")
    if isinstance(summary, dict):
        median_ms = summary.get("median_ms")
        if isinstance(median_ms, (int, float)):
            return float(median_ms)
    return float(metrics["full_build_ms"])


def evaluate_report(report: dict[str, Any], budgets: dict[str, float]) -> list[str]:
    metrics = report["metrics"]
    measured = {
        "sgc_artifact_mib": float(metrics["sgc_artifact_bytes"]) / (1024.0 * 1024.0),
        "program_artifact_mib": float(metrics["program_artifact_bytes"])
        / (1024.0 * 1024.0),
        "startup_avg_ms": float(metrics["startup"]["avg_ms"]),
        "check_avg_ms": float(metrics["check"]["avg_ms"]),
        "full_build_ms": full_build_budget_value(metrics),
        "runtime_avg_ms": float(metrics["runtime"]["avg_ms"]),
    }
    violations = []
    for name, ceiling in budgets.items():
        value = measured[name]
        if value > ceiling:
            violations.append(f"{name}={value:.2f} exceeds budget {ceiling:.2f}")
    report["measured_for_budget"] = measured
    report["budgets"] = budgets
    report["violations"] = violations
    report["passed"] = not violations
    return violations


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Measure release artifact and runtime budgets.")
    parser.add_argument("--sgc", type=Path, required=True)
    parser.add_argument("--scenario", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--iterations", type=int, default=10)
    parser.add_argument(
        "--runtime-mode",
        choices=("installed", "source-development"),
        help="Explicit sgc runtime provenance mode; omit for the installed default.",
    )
    for name, value in DEFAULT_BUDGETS.items():
        parser.add_argument(f"--max-{name.replace('_', '-')}", type=float, default=value)
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    root = Path(__file__).resolve().parents[2]
    sgc = args.sgc.resolve()
    scenario = args.scenario.resolve()
    if not sgc.is_file():
        raise RuntimeError(f"sgc binary not found: {sgc}")
    if not scenario.is_file():
        raise RuntimeError(f"resource scenario not found: {scenario}")
    if args.iterations < 3 or args.iterations > 100:
        raise RuntimeError("iterations must be in 3..=100")

    startup_samples, version = measure_repeated(
        sgc_command(sgc, args.runtime_mode, "--version"),
        root,
        args.iterations,
        10.0,
    )
    with tempfile.TemporaryDirectory(prefix="sengoo-release-resource-") as temp:
        temp_root = Path(temp)
        temp_scenario_dir = temp_root / "scenario"
        temp_scenario_dir.mkdir()
        temp_scenario = temp_scenario_dir / scenario.name
        shutil.copy2(scenario, temp_scenario)

        check_samples, _ = measure_repeated(
            sgc_command(sgc, args.runtime_mode, "check", str(temp_scenario)),
            root,
            args.iterations,
            30.0,
        )
        suffix = ".exe" if sys.platform == "win32" else ""
        program = temp_root / f"runtime-loop{suffix}"
        full_build_samples, _ = measure_repeated(
            sgc_command(
                sgc,
                args.runtime_mode,
                "build",
                str(temp_scenario),
                "--force-rebuild",
                "-O",
                "2",
                "-o",
                str(program),
            ),
            root,
            args.iterations,
            120.0,
        )
        if not program.is_file():
            raise RuntimeError(f"sgc did not produce expected program: {program}")
        runtime_samples, _ = measure_repeated(
            [str(program)], root, args.iterations, 10.0
        )
        program_size = program.stat().st_size
    full_build = summarize(full_build_samples)

    report: dict[str, Any] = {
        "schema_version": 1,
        "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
        "scenario": scenario.relative_to(root).as_posix(),
        "host": {
            "platform": platform.platform(),
            "machine": platform.machine(),
            "python": platform.python_version(),
        },
        "tool": {"path": str(sgc), "version": version},
        "iterations": args.iterations,
        "metrics": {
            "sgc_artifact_bytes": sgc.stat().st_size,
            "program_artifact_bytes": program_size,
            "startup": summarize(startup_samples),
            "check": summarize(check_samples),
            "full_build": full_build,
            "full_build_ms": float(full_build["median_ms"]),
            "runtime": summarize(runtime_samples),
        },
    }
    budgets = {
        name: float(getattr(args, f"max_{name}")) for name in DEFAULT_BUDGETS
    }
    violations = evaluate_report(report, budgets)

    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"release resource report: {args.output}")
    for name, value in report["measured_for_budget"].items():
        print(f"  {name}={value:.2f} budget<={budgets[name]:.2f}")
    if violations:
        for violation in violations:
            print(f"budget violation: {violation}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
