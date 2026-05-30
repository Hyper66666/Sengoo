#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import os
import re
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


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


def exe_name(stem: str) -> str:
    return f"{stem}.exe" if sys.platform.startswith("win") else stem


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


def measure_command_ms(cmd: list[str], cwd: Path | None = None) -> tuple[float, subprocess.CompletedProcess[str]]:
    started = time.perf_counter()
    proc = run_checked(cmd, cwd=cwd)
    return (time.perf_counter() - started) * 1000.0, proc


def parse_sengoo_output(stdout: str) -> dict[str, int]:
    nums = re.findall(r"-?\d+", stdout)
    if len(nums) < 2:
        raise RuntimeError(f"failed to parse Sengoo output:\n{stdout}")
    return {"score_sum": int(nums[-2]), "flagged": int(nums[-1])}


def parse_python_output(stdout: str) -> dict[str, int]:
    for line in reversed(stdout.splitlines()):
        line = line.strip()
        if not line:
            continue
        try:
            data = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(data, dict) and "score_sum" in data and "flagged" in data:
            return {"score_sum": int(data["score_sum"]), "flagged": int(data["flagged"])}
    raise RuntimeError(f"failed to parse Python output:\n{stdout}")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Risk-scoring hot-path demo: compare Sengoo native runtime vs Python runtime "
            "on the same practical fraud pre-filter workload."
        )
    )
    parser.add_argument("--samples", type=int, default=5, help="timed runs for each runtime (default: 5)")
    parser.add_argument("--warmup", type=int, default=1, help="warmup runs for each runtime (default: 1)")
    parser.add_argument(
        "--skip-compile",
        action="store_true",
        help="skip Sengoo compile step and reuse existing native binary",
    )
    return parser.parse_args()


def resolve_sengoo_root(demo_dir: Path) -> Path:
    bench_root = demo_dir.parents[1]
    env_root = os.environ.get("SENGOO_ROOT")

    candidates = []
    if env_root:
        candidates.append(Path(env_root).expanduser().resolve())
    candidates.append(bench_root.parent.resolve())
    candidates.append((bench_root.parent / "Sengoo").resolve())

    for candidate in candidates:
        if (candidate / "Cargo.toml").exists() and (candidate / "tools" / "sgc" / "src" / "main.rs").exists():
            return candidate

    raise RuntimeError(
        "cannot resolve Sengoo root; set SENGOO_ROOT or place bench next to Sengoo directory"
    )


def main() -> int:
    args = parse_args()
    if args.samples <= 0:
        raise RuntimeError("--samples must be > 0")
    if args.warmup < 0:
        raise RuntimeError("--warmup must be >= 0")

    demo_dir = Path(__file__).resolve().parent
    repo_root = resolve_sengoo_root(demo_dir)

    sg_source = demo_dir / "risk_scoring.sg"
    py_source = demo_dir / "risk_scoring_python.py"
    build_dir = demo_dir / "build"
    build_dir.mkdir(parents=True, exist_ok=True)
    sg_binary = build_dir / exe_name("risk_scoring")

    sgc_bin_candidates = [
        repo_root / "target" / "release" / exe_name("sgc"),
        repo_root / "target" / "debug" / exe_name("sgc"),
    ]
    sgc_bin = next((p for p in sgc_bin_candidates if p.exists()), None)
    if sgc_bin is None:
        raise RuntimeError("sgc binary not found; run `cargo build -p sgc --release` first")

    compile_ms: float | None = None
    if not args.skip_compile:
        compile_cmd = [
            str(sgc_bin),
            "build",
            str(sg_source),
            "-O",
            "2",
            "--reflect",
            "off",
            "--force-rebuild",
            "-o",
            str(sg_binary),
        ]
        compile_ms, _ = measure_command_ms(compile_cmd, cwd=repo_root)
    elif not sg_binary.exists():
        raise RuntimeError("--skip-compile requested but binary does not exist; run without --skip-compile first")

    sengoo_cmd = [str(sg_binary)]
    python_cmd = [sys.executable, str(py_source)]

    sengoo_result: dict[str, int] | None = None
    python_result: dict[str, int] | None = None

    for _ in range(args.warmup):
        _, proc = measure_command_ms(sengoo_cmd, cwd=repo_root)
        sengoo_result = parse_sengoo_output(proc.stdout)
        _, proc = measure_command_ms(python_cmd, cwd=repo_root)
        python_result = parse_python_output(proc.stdout)

    sengoo_samples = []
    python_samples = []
    for _ in range(args.samples):
        ms, proc = measure_command_ms(sengoo_cmd, cwd=repo_root)
        sengoo_samples.append(ms)
        sengoo_result = parse_sengoo_output(proc.stdout)

        ms, proc = measure_command_ms(python_cmd, cwd=repo_root)
        python_samples.append(ms)
        python_result = parse_python_output(proc.stdout)

    if sengoo_result is None or python_result is None:
        raise RuntimeError("no measurement samples collected")
    if sengoo_result != python_result:
        raise RuntimeError(
            "result mismatch between Sengoo and Python:\n"
            f"sengoo={sengoo_result}\npython={python_result}"
        )

    sengoo_avg = float(average(sengoo_samples) or 0.0)
    python_avg = float(average(python_samples) or 0.0)
    sengoo_p50 = float(percentile(sengoo_samples, 0.50) or 0.0)
    python_p50 = float(percentile(python_samples, 0.50) or 0.0)

    speedup = python_avg / sengoo_avg if sengoo_avg > 0 else None
    delta_pct = ((python_avg / sengoo_avg) - 1.0) * 100.0 if sengoo_avg > 0 else None

    break_even_runs = None
    if compile_ms is not None and python_avg > sengoo_avg:
        break_even_runs = compile_ms / (python_avg - sengoo_avg)

    summary: dict[str, Any] = {
        "scenario": "transaction-risk-pre-filter-hot-path",
        "inputs": {
            "records": 3_000_000,
            "samples": args.samples,
            "warmup": args.warmup,
            "compiled_this_run": compile_ms is not None,
        },
        "correctness": {
            "score_sum": sengoo_result["score_sum"],
            "flagged": sengoo_result["flagged"],
        },
        "timing_ms": {
            "sengoo_runtime_avg": sengoo_avg,
            "sengoo_runtime_p50": sengoo_p50,
            "python_runtime_avg": python_avg,
            "python_runtime_p50": python_p50,
            "sengoo_compile": compile_ms,
        },
        "comparison": {
            "python_vs_sengoo_speedup_x": speedup,
            "python_vs_sengoo_runtime_delta_pct": delta_pct,
            "compile_break_even_runs": break_even_runs,
        },
    }

    results_dir = demo_dir / "results"
    results_dir.mkdir(parents=True, exist_ok=True)
    out = results_dir / f"{int(time.time() * 1000)}-risk-scoring-demo.json"
    out.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    print("Risk Scoring Hot-Path Demo")
    print("| Runtime | avg(ms) | p50(ms) |")
    print("|---|---:|---:|")
    print(f"| Sengoo native | {sengoo_avg:.2f} | {sengoo_p50:.2f} |")
    print(f"| Python | {python_avg:.2f} | {python_p50:.2f} |")
    if speedup is not None:
        print("")
        print(f"Python vs Sengoo speed ratio: {speedup:.2f}x")
        print(f"Runtime delta (Python relative to Sengoo): {delta_pct:+.2f}%")
    if compile_ms is not None:
        print(f"Sengoo compile time: {compile_ms:.2f} ms")
    if break_even_runs is not None:
        print(f"Estimated compile break-even: ~{break_even_runs:.1f} runs")
    print(f"Result parity check: score_sum={sengoo_result['score_sum']} flagged={sengoo_result['flagged']}")
    print(f"Demo report: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
