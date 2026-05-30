#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import subprocess
import sys
from pathlib import Path
from typing import Any


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run a ready-to-use Python interoperability example on top of "
            "python_interop_bench.py and print a compact summary."
        )
    )
    parser.add_argument("--calls", type=int, default=5000, help="calls per sample (default: 5000)")
    parser.add_argument("--samples", type=int, default=2, help="timed samples per runner (default: 2)")
    parser.add_argument("--warmup", type=int, default=0, help="warmup runs per runner (default: 0)")
    return parser.parse_args()


def parse_report_path(stdout: str) -> Path:
    marker = "Python interop report:"
    for line in stdout.splitlines():
        if line.startswith(marker):
            raw = line.split(":", 1)[1].strip()
            return Path(raw)
    raise RuntimeError("failed to locate report path from python_interop_bench output")


def format_float(value: Any, digits: int = 2) -> str:
    if isinstance(value, (int, float)):
        return f"{float(value):.{digits}f}"
    return "n/a"


def format_percent(value: Any) -> str:
    if isinstance(value, (int, float)):
        return f"{float(value):+.2f}%"
    return "n/a"


def print_summary(report: dict[str, Any]) -> None:
    rows = (
        report.get("summary", {})
        .get("ordered_by_loop_avg_ms", [])
    )
    if not isinstance(rows, list) or not rows:
        print("[example] no comparable runners were available.")
        return

    print("")
    print("Python Interop Example Summary")
    print("| Runner | Loop avg (ms) | Calls/s | vs Python native |")
    print("|---|---:|---:|---:|")
    for row in rows:
        if not isinstance(row, dict):
            continue
        print(
            "| "
            f"{row.get('name', 'unknown')} | "
            f"{format_float(row.get('loop_avg_ms'))} | "
            f"{format_float(row.get('calls_per_sec'))} | "
            f"{format_percent(row.get('loop_vs_python_native_pct'))} |"
        )


def main() -> int:
    args = parse_args()
    if args.calls <= 0:
        raise RuntimeError("--calls must be > 0")
    if args.samples <= 0:
        raise RuntimeError("--samples must be > 0")
    if args.warmup < 0:
        raise RuntimeError("--warmup must be >= 0")

    bench_root = Path(__file__).resolve().parents[1]
    bench_script = bench_root / "python_interop_bench.py"
    if not bench_script.exists():
        raise RuntimeError(f"python interop benchmark script not found: {bench_script}")

    cmd = [
        sys.executable,
        str(bench_script),
        "--calls",
        str(args.calls),
        "--samples",
        str(args.samples),
        "--warmup",
        str(args.warmup),
    ]
    print(f"[example] running: {' '.join(cmd)}")
    proc = subprocess.run(
        cmd,
        cwd=str(bench_root),
        text=True,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
    )
    if proc.stdout:
        print(proc.stdout.rstrip())
    if proc.returncode != 0:
        if proc.stderr:
            print(proc.stderr.rstrip(), file=sys.stderr)
        return proc.returncode

    report_path = parse_report_path(proc.stdout)
    if not report_path.is_absolute():
        report_path = (bench_root / report_path).resolve()
    if not report_path.exists():
        raise RuntimeError(f"reported JSON file does not exist: {report_path}")

    report = json.loads(report_path.read_text(encoding="utf-8"))
    print_summary(report)
    print("")
    print(f"[example] report saved at: {report_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
