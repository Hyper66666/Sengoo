#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path
from typing import Any


DEFAULT_REQUIRED_RUNNERS = ("python_native", "sengoo_runtime_pythoninterop")
DEFAULT_MAX_SENGOO_OVERHEAD_PCT = 50.0


def load_json(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise RuntimeError(f"report not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"invalid json report: {path}: {exc}") from exc


def evaluate_interop(
    report: dict[str, Any],
    *,
    required_runners: tuple[str, ...],
    max_sengoo_overhead_pct: float,
    fail_fast: bool,
) -> tuple[list[str], list[str]]:
    summaries: list[str] = []
    violations: list[str] = []

    def add_violation(message: str) -> bool:
        violations.append(message)
        return fail_fast

    runners = report.get("runners")
    if not isinstance(runners, dict):
        add_violation("interop report missing runners block")
        return summaries, violations

    for runner_id in required_runners:
        runner = runners.get(runner_id)
        if not isinstance(runner, dict):
            if add_violation(f"required runner missing: {runner_id}"):
                return summaries, violations
            continue
        if not runner.get("available", False):
            reason = runner.get("reason", "unknown reason")
            if add_violation(f"required runner unavailable: {runner_id} ({reason})"):
                return summaries, violations
            continue
        result = runner.get("result")
        if not isinstance(result, dict):
            if add_violation(f"runner result missing: {runner_id}"):
                return summaries, violations
            continue
        loop_avg = result.get("loop_avg_ms")
        checksum_ok = result.get("checksum_consistent")
        if not isinstance(loop_avg, (int, float)):
            if add_violation(f"runner loop_avg_ms missing: {runner_id}"):
                return summaries, violations
            continue
        summaries.append(f"interop/{runner_id}/loop_avg_ms={float(loop_avg):.3f}")
        if checksum_ok is False:
            if add_violation(f"runner checksum inconsistent: {runner_id}"):
                return summaries, violations

    py_runner = runners.get("python_native", {})
    sg_runner = runners.get("sengoo_runtime_pythoninterop", {})
    if (
        isinstance(py_runner, dict)
        and py_runner.get("available")
        and isinstance(sg_runner, dict)
        and sg_runner.get("available")
    ):
        py_loop = (
            py_runner.get("result", {}).get("loop_avg_ms")
            if isinstance(py_runner.get("result"), dict)
            else None
        )
        sg_loop = (
            sg_runner.get("result", {}).get("loop_avg_ms")
            if isinstance(sg_runner.get("result"), dict)
            else None
        )
        if isinstance(py_loop, (int, float)) and isinstance(sg_loop, (int, float)) and py_loop > 0:
            overhead_pct = ((float(sg_loop) - float(py_loop)) / float(py_loop)) * 100.0
            summaries.append(
                "interop/sengoo_overhead_pct="
                f"{overhead_pct:.2f} (target<={max_sengoo_overhead_pct:.2f})"
            )
            if overhead_pct > max_sengoo_overhead_pct:
                add_violation(
                    f"sengoo interop overhead too high ({overhead_pct:.2f}% > {max_sengoo_overhead_pct:.2f}%)"
                )

    return summaries, violations


def evaluate_bootstrap(report: dict[str, Any], *, fail_fast: bool) -> tuple[list[str], list[str]]:
    summaries: list[str] = []
    violations: list[str] = []

    def add_violation(message: str) -> bool:
        violations.append(message)
        return fail_fast

    proof = report.get("bootstrap_proof")
    if not isinstance(proof, dict):
        add_violation("bootstrap report missing bootstrap_proof block")
        return summaries, violations

    status = proof.get("status")
    criteria = proof.get("criteria")
    summary = proof.get("summary")
    summaries.append(f"bootstrap/status={status}")

    if status != "pass":
        if add_violation(f"bootstrap proof status is not pass: {status}"):
            return summaries, violations

    if not isinstance(criteria, dict):
        if add_violation("bootstrap proof missing criteria block"):
            return summaries, violations
    else:
        for key, value in criteria.items():
            summaries.append(f"bootstrap/criteria/{key}={value}")
            if value is False:
                if add_violation(f"bootstrap criteria failed: {key}"):
                    return summaries, violations

    if isinstance(summary, dict):
        total = summary.get("total_scenarios")
        passed = summary.get("passed_scenarios")
        summaries.append(f"bootstrap/scenarios={passed}/{total}")
        if isinstance(total, (int, float)) and isinstance(passed, (int, float)) and passed < total:
            add_violation(f"bootstrap scenario pass mismatch: {passed}/{total}")
    else:
        add_violation("bootstrap proof missing summary block")

    return summaries, violations


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Validate Python interop and bootstrap generality benchmark reports."
    )
    parser.add_argument("--mode", choices=["soft", "hard"], default="soft")
    parser.add_argument("--interop-sample", required=True, help="path to *-python-interop.json")
    parser.add_argument("--bootstrap-sample", required=True, help="path to *-bootstrap-generality.json")
    parser.add_argument(
        "--required-runners",
        default=",".join(DEFAULT_REQUIRED_RUNNERS),
        help=f"comma-separated required interop runners (default: {','.join(DEFAULT_REQUIRED_RUNNERS)})",
    )
    parser.add_argument(
        "--max-sengoo-overhead-pct",
        type=float,
        default=DEFAULT_MAX_SENGOO_OVERHEAD_PCT,
        help=f"max allowed Sengoo loop overhead over Python native (default: {DEFAULT_MAX_SENGOO_OVERHEAD_PCT})",
    )
    parser.add_argument("--fail-fast", action="store_true")
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    interop_path = Path(args.interop_sample).expanduser().resolve()
    bootstrap_path = Path(args.bootstrap_sample).expanduser().resolve()

    interop_report = load_json(interop_path)
    bootstrap_report = load_json(bootstrap_path)

    required_runners = tuple(
        item.strip()
        for item in str(args.required_runners).split(",")
        if item.strip()
    )

    interop_summaries, interop_violations = evaluate_interop(
        interop_report,
        required_runners=required_runners,
        max_sengoo_overhead_pct=float(args.max_sengoo_overhead_pct),
        fail_fast=bool(args.fail_fast),
    )
    bootstrap_summaries, bootstrap_violations = evaluate_bootstrap(
        bootstrap_report,
        fail_fast=bool(args.fail_fast),
    )

    print(
        "interop-bootstrap-gate "
        f"mode={args.mode} "
        f"interop={interop_path} "
        f"bootstrap={bootstrap_path}"
    )
    for line in interop_summaries:
        print(f"  {line}")
    for line in bootstrap_summaries:
        print(f"  {line}")

    violations = interop_violations + bootstrap_violations
    if not violations:
        print("interop-bootstrap-gate PASS")
        return 0

    print(f"interop-bootstrap-gate found {len(violations)} violation(s):")
    for violation in violations:
        print(f"  - {violation}")

    if args.mode == "hard":
        print("interop-bootstrap-gate HARD failure", file=sys.stderr)
        return 1

    print("interop-bootstrap-gate SOFT warning (not failing build)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
