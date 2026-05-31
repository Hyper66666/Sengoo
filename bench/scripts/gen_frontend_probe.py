#!/usr/bin/env python3
"""Generate a synthetic Sengoo source file for frontend phase profiling.

This is the in-tree, reproducible version of the probe used by the
`frontend-compile-perf` program (Phase 0 profiling and the Phase 3 peak-RSS
target). It emits ``N`` independent functions plus a ``main`` so the
``parse / typeck / hir_lower / mir_lower / mir_opt`` frontend stages do real,
measurable work per function.

Pair it with the per-phase timings and native peak-RSS recorded by the
compiler:

    python bench/scripts/gen_frontend_probe.py 10000 .tmp/frontend_bench_10k.sg
    $env:SENGOO_PHASE_TIMINGS = "1"      # PowerShell (use `export` on POSIX)
    sgc build .tmp/frontend_bench_10k.sg --emit-llvm -O0

…or drive it through the in-process compile benchmark, which records
``peak_rss_bytes`` next to the phase timings:

    sgc bench compile <suite-or-path> -O0 --iterations 1

Each generated function exercises let-bindings, arithmetic, a branch, and a
loop, so the type checker performs genuine inference/unification per function.

Reachability / pruning note
---------------------------
By default ``main`` only calls ``f0`` (matching the original Phase 0 probe), so
at counts at/above the compiler's unreachable-prune threshold (~20k functions)
the dead functions are pruned and the measured frontend collapses to ~one
function. For honest scale-curve runs at 100k / 1000k functions, pass
``--all-reachable``: it additionally emits a ``driver`` that calls every
function in a flat (non-nested) sequence and points ``main`` at it, so the full
frontend runs on every function regardless of scale.
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


def render(n: int, all_reachable: bool) -> str:
    lines: list[str] = []
    for i in range(n):
        lines.append(f"def f{i}(x: i64, y: i64) -> i64 {{")
        lines.append("    let a = x + y;")
        lines.append(f"    let b = a * {i % 97 + 1};")
        lines.append("    let acc = 0;")
        lines.append("    let k = 0;")
        lines.append("    while k < b {")
        lines.append("        acc = acc + k;")
        lines.append("        k = k + 1;")
        lines.append("    }")
        lines.append("    if acc > a { acc } else { a + b }")
        lines.append("}")

    if all_reachable and n > 0:
        # A flat sequence of calls keeps every function reachable without deep
        # expression nesting, so unreachable-pruning cannot hide them at scale.
        lines.append("def driver() -> i64 {")
        lines.append("    let total = 0;")
        for i in range(n):
            lines.append(f"    total = total + f{i}(1, 2);")
        lines.append("    total")
        lines.append("}")
        lines.append("def main() -> i64 {")
        lines.append("    driver()")
        lines.append("}")
    else:
        lines.append("def main() -> i64 {")
        lines.append("    f0(1, 2)" if n > 0 else "    0")
        lines.append("}")

    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "count",
        nargs="?",
        type=int,
        default=10000,
        help="number of independent functions to generate (default: 10000)",
    )
    parser.add_argument(
        "out",
        nargs="?",
        default=".tmp/frontend_bench.sg",
        help="output .sg path (default: .tmp/frontend_bench.sg)",
    )
    parser.add_argument(
        "--all-reachable",
        action="store_true",
        help="emit a driver so every function is reachable (use for 100k/1000k)",
    )
    args = parser.parse_args(argv)

    if args.count < 0:
        parser.error("count must be non-negative")

    out_path = Path(args.out)
    if out_path.parent != Path(""):
        out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(render(args.count, args.all_reachable), encoding="utf-8")

    mode = "all-reachable" if args.all_reachable else "main->f0"
    print(f"wrote {out_path}: {args.count} functions + main ({mode})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
