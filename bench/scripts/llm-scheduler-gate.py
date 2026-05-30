#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise RuntimeError(f"report not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"invalid json report: {path}: {exc}") from exc


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate llm_scheduler_bench report thresholds.")
    parser.add_argument("--sample", required=True, help="path to *-llm-scheduler-bench.json")
    parser.add_argument("--mode", choices=["soft", "hard"], default="soft")
    parser.add_argument(
        "--min-light-ratio",
        type=float,
        default=1.10,
        help="minimum python_vs_sengoo_loop_speed_ratio_x for light-kernel scenario",
    )
    parser.add_argument(
        "--max-heavy-delta-pct",
        type=float,
        default=20.0,
        help="maximum allowed Sengoo slowdown percent in heavy-kernel scenario",
    )
    parser.add_argument("--fail-fast", action="store_true")
    return parser.parse_args()


def scenario_index(report: dict[str, Any]) -> dict[str, dict[str, Any]]:
    rows = report.get("scenarios")
    if not isinstance(rows, list):
        return {}
    out: dict[str, dict[str, Any]] = {}
    for row in rows:
        if isinstance(row, dict):
            sid = row.get("id")
            if isinstance(sid, str):
                out[sid] = row
    return out


def evaluate(report: dict[str, Any], args: argparse.Namespace) -> tuple[list[str], list[str]]:
    summaries: list[str] = []
    violations: list[str] = []

    def add_violation(message: str) -> bool:
        violations.append(message)
        return bool(args.fail_fast)

    idx = scenario_index(report)
    light = idx.get("prefill_decode_orchestration_light_kernel")
    heavy = idx.get("prefill_decode_orchestration_heavy_kernel")
    if not light or not heavy:
        add_violation("required scenarios missing (light/heavy)")
        return summaries, violations

    def checksum_ok(row: dict[str, Any]) -> bool:
        py = row.get("python", {})
        sg = row.get("sengoo", {})
        if not isinstance(py, dict) or not isinstance(sg, dict):
            return False
        return (
            bool(py.get("checksum_consistent"))
            and bool(sg.get("checksum_consistent"))
            and py.get("checksum") == sg.get("checksum")
        )

    light_cmp = light.get("comparison", {})
    heavy_cmp = heavy.get("comparison", {})
    if not isinstance(light_cmp, dict) or not isinstance(heavy_cmp, dict):
        add_violation("scenario comparison blocks missing")
        return summaries, violations

    light_ratio = float(light_cmp.get("python_vs_sengoo_loop_speed_ratio_x", 0.0) or 0.0)
    heavy_delta = float(heavy_cmp.get("sengoo_loop_delta_pct_vs_python", 0.0) or 0.0)

    summaries.append(f"light_ratio={light_ratio:.3f} (target>={float(args.min_light_ratio):.3f})")
    summaries.append(f"heavy_delta_pct={heavy_delta:+.2f}% (target<={float(args.max_heavy_delta_pct):.2f}%)")
    summaries.append(f"light_checksum_parity={checksum_ok(light)}")
    summaries.append(f"heavy_checksum_parity={checksum_ok(heavy)}")

    if not checksum_ok(light):
        if add_violation("light scenario checksum parity failed"):
            return summaries, violations
    if not checksum_ok(heavy):
        if add_violation("heavy scenario checksum parity failed"):
            return summaries, violations
    if light_ratio < float(args.min_light_ratio):
        if add_violation(
            f"light scenario ratio below threshold ({light_ratio:.3f} < {float(args.min_light_ratio):.3f})"
        ):
            return summaries, violations
    if heavy_delta > float(args.max_heavy_delta_pct):
        if add_violation(
            f"heavy scenario slowdown above threshold ({heavy_delta:+.2f}% > {float(args.max_heavy_delta_pct):.2f}%)"
        ):
            return summaries, violations

    return summaries, violations


def main() -> int:
    args = parse_args()
    sample_path = Path(args.sample).expanduser().resolve()
    report = load_json(sample_path)
    summaries, violations = evaluate(report, args)

    print(f"llm-scheduler-gate mode={args.mode} sample={sample_path}")
    for line in summaries:
        print(f"  {line}")

    if not violations:
        print("llm-scheduler-gate PASS")
        return 0

    print(f"llm-scheduler-gate found {len(violations)} violation(s):")
    for violation in violations:
        print(f"  - {violation}")
    if args.mode == "hard":
        print("llm-scheduler-gate HARD failure", file=sys.stderr)
        return 1
    print("llm-scheduler-gate SOFT warning (not failing build)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
