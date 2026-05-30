#!/usr/bin/env python3
"""Sprint-01 generic benchmark gate.

This utility validates that required generic incremental benchmark scenarios
are present in a benchmark report payload.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

REQUIRED_CASES = (
    "generic_body_change_root.sg",
    "generic_new_instantiation_root.sg",
    "generic_signature_change_root.sg",
)


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description="Validate sprint-01 generic benchmark cases")
    parser.add_argument("--report", required=True, help="Path to benchmark JSON report")
    parser.add_argument("--mode", default="soft", choices=("soft", "hard"))
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    report_path = Path(args.report)
    if not report_path.exists():
        print(f"missing report: {report_path}")
        return 1

    payload = json.loads(report_path.read_text(encoding="utf-8"))
    text = json.dumps(payload, ensure_ascii=False)

    missing = [name for name in REQUIRED_CASES if name not in text]
    if not missing:
        print("sprint01 generic gate: pass")
        return 0

    print("missing scenarios:")
    for name in missing:
        print(f"- {name}")

    return 1 if args.mode == "hard" else 0


if __name__ == "__main__":
    raise SystemExit(main())
