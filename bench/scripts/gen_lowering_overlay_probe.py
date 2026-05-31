#!/usr/bin/env python3
"""Generate a materialization-heavy Sengoo source file for MIR lowering probes.

This probe targets the `frontend-compile-perf` lowering-overlay work. Each
generated function materializes a lambda, so pre-overlay lowering pays the
`Cow::to_mut()` cost once per function when registering the generated lambda
signature/name. With `--include-generic`, each function also materializes a
unique generic method instance; with `--include-async`, a matching async function
with an async block is emitted to observe the separate async-lowering cost.

Use the frontend phase timer and emit LLVM only when the probe intentionally
stresses frontend lowering rather than native linking:

    python bench/scripts/gen_lowering_overlay_probe.py 2500 .tmp/lowering_overlay.sg
    $env:SENGOO_PHASE_TIMINGS = "1"
    sgc build .tmp/lowering_overlay.sg --emit-llvm -O0 --force-rebuild
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path


def render(count: int, include_generic: bool, include_async: bool) -> str:
    lines: list[str] = []

    if include_generic:
        for i in range(count):
            lines.extend(
                [
                    f"struct Box{i}<T> {{",
                    "    value: T,",
                    "}",
                    "",
                    f"impl<T> Box{i}<T> {{",
                    "    def value_or(self, fallback: T) -> T {",
                    "        self.value",
                    "    }",
                    "}",
                    "",
                ]
            )

    for i in range(count):
        lines.extend(
            [
                f"def f{i}(x: i64) -> i64 {{",
                "    let inc = |y| y + 1;",
            ]
        )
        if include_generic:
            lines.extend(
                [
                    f"    let boxed = Box{i} {{ value: inc(x + {i}) }};",
                    "    boxed.value_or(0)",
                ]
            )
        else:
            lines.append(f"    inc(x + {i})")
        lines.extend(["}", ""])

    if include_async:
        for i in range(count):
            lines.extend(
                [
                    f"async def af{i}(x: i64) -> i64 {{",
                    f"    let fut = async {{ x + {i} }};",
                    "    let value = await fut;",
                    "    value",
                    "}",
                    "",
                ]
            )

    lines.extend(["def main() -> i64 {", "    f0(1)", "}"])
    return "\n".join(lines) + "\n"


def main(argv: list[str]) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "count",
        nargs="?",
        type=int,
        default=2500,
        help="number of materializing functions to generate (default: 2500)",
    )
    parser.add_argument(
        "out",
        nargs="?",
        default=".tmp/lowering_overlay_probe.sg",
        help="output .sg path (default: .tmp/lowering_overlay_probe.sg)",
    )
    parser.add_argument(
        "--include-generic",
        action="store_true",
        help="also materialize a unique generic method instance per function",
    )
    parser.add_argument(
        "--include-async",
        action="store_true",
        help="also emit async functions with async blocks to observe async-lowering cost",
    )
    args = parser.parse_args(argv)

    if args.count < 0:
        parser.error("count must be non-negative")

    out_path = Path(args.out)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(
        render(args.count, args.include_generic, args.include_async),
        encoding="utf-8",
    )

    extras = []
    if args.include_generic:
        extras.append("generic")
    if args.include_async:
        extras.append("async")
    suffix = f" + {'/'.join(extras)}" if extras else ""
    print(f"wrote {out_path}: {args.count} lambda materializers{suffix}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
