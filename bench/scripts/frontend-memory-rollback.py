#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
from typing import Any


def load_decision(path: Path) -> dict[str, Any]:
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except FileNotFoundError as exc:
        raise RuntimeError(f"decision artifact not found: {path}") from exc
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"failed to parse decision artifact {path}: {exc}") from exc


def write_env_file(path: Path, mode: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(f"SENGOO_FRONTEND_MEMORY_MODE={mode}\n", encoding="utf-8")


def maybe_write_github_output(mode: str) -> None:
    github_output = os.environ.get("GITHUB_OUTPUT")
    if not github_output:
        return
    output_path = Path(github_output)
    with output_path.open("a", encoding="utf-8") as handle:
        handle.write(f"frontend_memory_mode={mode}\n")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description="Apply frontend memory mode rollback decision from gate evidence.",
    )
    parser.add_argument("--decision", required=True, help="path to advanced gate decision json")
    parser.add_argument(
        "--rollback-env-out",
        default="bench/results/frontend-memory-rollback.env",
        help="path to write SENGOO_FRONTEND_MEMORY_MODE override env file when rollback is needed",
    )
    parser.add_argument(
        "--no-github-output",
        action="store_true",
        help="do not export frontend_memory_mode to GITHUB_OUTPUT",
    )
    return parser.parse_args()


def main() -> int:
    args = parse_args()
    decision_path = Path(args.decision).expanduser().resolve()
    env_out = Path(args.rollback_env_out).expanduser().resolve()
    decision = load_decision(decision_path)

    gate_decision = str(decision.get("gate_decision", "")).lower()
    mode = str(decision.get("mode", "")).lower()
    next_mode = "stream"
    reason = "gate pass"
    if gate_decision == "fail" and mode == "hard":
        next_mode = "legacy"
        reason = "gate regression detected"
        write_env_file(env_out, next_mode)
        print(f"rollback decision: {reason}; wrote {env_out}")
    elif gate_decision == "fail":
        reason = "soft gate warning only; rollback skipped"
        print(f"rollback decision: {reason}")
    else:
        print("rollback decision: no rollback required")

    if not args.no_github_output:
        maybe_write_github_output(next_mode)
        print(f"github-output frontend_memory_mode={next_mode}")

    print(f"recommended frontend memory mode: {next_mode}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
