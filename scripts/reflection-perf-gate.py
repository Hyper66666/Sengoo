#!/usr/bin/env python3
"""Gate reflection benchmark overhead and disabled-path regressions."""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
from typing import Any, Dict, Optional


def case_metric(case: Dict[str, Any]) -> Optional[float]:
    if isinstance(case.get("p50_ms"), (int, float)):
        return float(case["p50_ms"])
    if isinstance(case.get("total_ms"), (int, float)):
        return float(case["total_ms"])
    samples = case.get("sample_ms")
    if isinstance(samples, list) and samples:
        nums = [float(v) for v in samples if isinstance(v, (int, float))]
        if nums:
            return sum(nums) / len(nums)
    return None


def load_json(path: pathlib.Path) -> Dict[str, Any]:
    return json.loads(path.read_text(encoding="utf-8-sig"))


def load_baseline_case(baseline_path: pathlib.Path, key: str) -> Optional[float]:
    if not baseline_path.exists():
        return None
    data = load_json(baseline_path)
    cases = data.get("cases", {})
    if not isinstance(cases, dict):
        return None
    case = cases.get(key)
    if not isinstance(case, dict):
        return None
    if isinstance(case.get("p50_ms"), (int, float)):
        return float(case["p50_ms"])
    if isinstance(case.get("total_ms"), (int, float)):
        return float(case["total_ms"])
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sample", required=True, help="Reflection benchmark JSON sample path")
    parser.add_argument(
        "--mode",
        default="soft",
        choices=["soft", "hard"],
        help="Gate strictness preset",
    )
    parser.add_argument(
        "--baseline",
        default="bench/baseline.json",
        help="Baseline JSON path for disabled-case regression check",
    )
    parser.add_argument("--max-enabled-unused-overhead-pct", type=float, default=None)
    parser.add_argument("--max-enabled-used-overhead-pct", type=float, default=None)
    parser.add_argument("--max-disabled-regression-pct", type=float, default=None)
    args = parser.parse_args()

    defaults = {
        "soft": {
            "max_enabled_unused_overhead_pct": 25.0,
            "max_enabled_used_overhead_pct": 45.0,
            "max_disabled_regression_pct": 20.0,
        },
        "hard": {
            "max_enabled_unused_overhead_pct": 15.0,
            "max_enabled_used_overhead_pct": 30.0,
            "max_disabled_regression_pct": 12.0,
        },
    }[args.mode]

    max_enabled_unused_overhead_pct = (
        args.max_enabled_unused_overhead_pct
        if args.max_enabled_unused_overhead_pct is not None
        else defaults["max_enabled_unused_overhead_pct"]
    )
    max_enabled_used_overhead_pct = (
        args.max_enabled_used_overhead_pct
        if args.max_enabled_used_overhead_pct is not None
        else defaults["max_enabled_used_overhead_pct"]
    )
    max_disabled_regression_pct = (
        args.max_disabled_regression_pct
        if args.max_disabled_regression_pct is not None
        else defaults["max_disabled_regression_pct"]
    )

    sample_path = pathlib.Path(args.sample)
    report = load_json(sample_path)
    kind = report.get("kind")
    suite = report.get("suite")
    if kind != "reflection":
        print(f"[gate] expected kind=reflection, got {kind!r}")
        return 1

    case_map: Dict[str, Dict[str, Any]] = {}
    for case in report.get("cases", []):
        if isinstance(case, dict):
            name = case.get("name")
            if isinstance(name, str):
                case_map[name] = case

    missing = [name for name in ("disabled", "enabled-unused", "enabled-used") if name not in case_map]
    if missing:
        print(f"[gate] missing reflection benchmark cases: {', '.join(missing)}")
        return 1

    disabled = case_metric(case_map["disabled"])
    enabled_unused = case_metric(case_map["enabled-unused"])
    enabled_used = case_metric(case_map["enabled-used"])
    if disabled is None or enabled_unused is None or enabled_used is None:
        print("[gate] missing usable p50/total/sample metrics in reflection report")
        return 1

    if disabled <= 0.0:
        print(f"[gate] invalid disabled metric: {disabled}")
        return 1

    failures = []
    enabled_unused_overhead = ((enabled_unused - disabled) / disabled) * 100.0
    enabled_used_overhead = ((enabled_used - disabled) / disabled) * 100.0

    if enabled_unused_overhead > max_enabled_unused_overhead_pct:
        failures.append(
            f"enabled-unused overhead {enabled_unused_overhead:.2f}% > {max_enabled_unused_overhead_pct:.2f}%"
        )
    if enabled_used_overhead > max_enabled_used_overhead_pct:
        failures.append(
            f"enabled-used overhead {enabled_used_overhead:.2f}% > {max_enabled_used_overhead_pct:.2f}%"
        )

    baseline_key = f"reflection/{suite}/disabled"
    baseline_disabled = load_baseline_case(pathlib.Path(args.baseline), baseline_key)
    if baseline_disabled is not None and baseline_disabled > 0.0:
        disabled_regression = ((disabled - baseline_disabled) / baseline_disabled) * 100.0
        if disabled_regression > max_disabled_regression_pct:
            failures.append(
                f"disabled regression {disabled_regression:.2f}% > {max_disabled_regression_pct:.2f}% (baseline {baseline_disabled:.2f}ms)"
            )
        print(
            f"[gate] disabled={disabled:.2f}ms baseline={baseline_disabled:.2f}ms regression={disabled_regression:+.2f}%"
        )
    else:
        print(f"[gate] baseline key not found, skipped disabled regression check: {baseline_key}")

    print(
        f"[gate] disabled={disabled:.2f}ms enabled-unused-overhead={enabled_unused_overhead:+.2f}% enabled-used-overhead={enabled_used_overhead:+.2f}%"
    )

    if failures:
        print("[gate] FAIL")
        for failure in failures:
            print(f"  - {failure}")
        return 1

    print("[gate] PASS")
    return 0


if __name__ == "__main__":
    sys.exit(main())
