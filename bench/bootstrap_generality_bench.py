#!/usr/bin/env python3
from __future__ import annotations

import json
import math
import os
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


COMPILE_ITERS = 2
RUNTIME_WARMUP = 1
RUNTIME_ITERS = 5


SCENARIOS: list[dict[str, Any]] = [
    {
        "id": "arith_for",
        "title": "Arithmetic For-Range",
        "capability": "control_flow_for",
        "entry": "main.sg",
        "sources": {
            "main.sg": """def main() -> i64 {
    let acc = 0
    for i in 0..50000 {
        acc = acc + (i % 7)
    }
    print(acc)
    0
}
""",
        },
        "reference_py": """def main() -> None:
    acc = 0
    for i in range(0, 50000):
        acc += i % 7
    print(acc)


if __name__ == "__main__":
    main()
""",
    },
    {
        "id": "branch_mix",
        "title": "Branch + While Mix",
        "capability": "branching_while",
        "entry": "main.sg",
        "sources": {
            "main.sg": """def main() -> i64 {
    let i = 0
    let acc = 0
    while i < 20000 {
        if i % 2 < 1 {
            acc = acc + i
        } else {
            acc = acc - (i % 5)
        }
        i = i + 1
    }
    print(acc)
    0
}
""",
        },
        "reference_py": """def main() -> None:
    i = 0
    acc = 0
    while i < 20000:
        if i % 2 < 1:
            acc += i
        else:
            acc -= i % 5
        i += 1
    print(acc)


if __name__ == "__main__":
    main()
""",
    },
    {
        "id": "array_index",
        "title": "Array Index Reduce",
        "capability": "array_ops",
        "entry": "main.sg",
        "sources": {
            "main.sg": """def main() -> i64 {
    let arr = [1, 3, 5, 7]
    let i = 0
    let acc = 0
    while i < 200000 {
        acc = acc + arr[i % 4]
        i = i + 1
    }
    print(acc)
    0
}
""",
        },
        "reference_py": """def main() -> None:
    arr = [1, 3, 5, 7]
    i = 0
    acc = 0
    while i < 200000:
        acc += arr[i % 4]
        i += 1
    print(acc)


if __name__ == "__main__":
    main()
""",
    },
    {
        "id": "recursion_fib",
        "title": "Recursive Fibonacci",
        "capability": "recursion",
        "entry": "main.sg",
        "sources": {
            "main.sg": """def fib(n: i64) -> i64 {
    if n < 2 {
        n
    } else {
        fib(n - 1) + fib(n - 2)
    }
}

def main() -> i64 {
    print(fib(20))
    0
}
""",
        },
        "reference_py": """def fib(n: int) -> int:
    if n < 2:
        return n
    return fib(n - 1) + fib(n - 2)


def main() -> None:
    print(fib(20))


if __name__ == "__main__":
    main()
""",
    },
    {
        "id": "method_dispatch",
        "title": "Impl Method Dispatch",
        "capability": "impl_method",
        "entry": "main.sg",
        "sources": {
            "main.sg": """impl i64 {
    def bump(self) -> i64 {
        self * 2 + 1
    }
}

def main() -> i64 {
    let acc = 0
    for i in 0..20000 {
        let x: i64 = i
        acc = acc + x.bump()
    }
    print(acc)
    0
}
""",
        },
        "reference_py": """def bump(x: int) -> int:
    return x * 2 + 1


def main() -> None:
    acc = 0
    for i in range(0, 20000):
        acc += bump(i)
    print(acc)


if __name__ == "__main__":
    main()
""",
    },
    {
        "id": "import_graph",
        "title": "Multi-Module Import Graph",
        "capability": "module_import_graph",
        "entry": "main.sg",
        "sources": {
            "main.sg": """import dep_a;
import dep_b;

def main() -> i64 {
    let x = 40
    if x > 10 {
        print(42)
    } else {
        print(0)
    }
    0
}
""",
            "dep_a.sg": """def helper_a(v: i64) -> i64 {
    v + 1
}
""",
            "dep_b.sg": """def helper_b(v: i64) -> i64 {
    v * 2
}
""",
        },
        "reference_py": """def main() -> None:
    x = 40
    if x > 10:
        print(42)
    else:
        print(0)


if __name__ == "__main__":
    main()
""",
    },
]


def now_unix_ms() -> int:
    return int(time.time() * 1000)


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


def measure_command_ms(cmd: list[str], cwd: Path | None = None) -> float:
    started = time.perf_counter()
    run_checked(cmd, cwd=cwd)
    return (time.perf_counter() - started) * 1000.0


def last_nonempty_line(text: str) -> str:
    lines = [line.strip() for line in text.splitlines() if line.strip()]
    return lines[-1] if lines else ""


def measure_runtime_ms(cmd: list[str], expected_output: str, cwd: Path | None = None) -> float:
    started = time.perf_counter()
    proc = run_checked(cmd, cwd=cwd)
    elapsed = (time.perf_counter() - started) * 1000.0
    got = last_nonempty_line(proc.stdout)
    if got != expected_output:
        raise RuntimeError(
            f"unexpected output for {' '.join(cmd)}: expected '{expected_output}', got '{got}'"
        )
    return elapsed


def write_file(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


def clear_dir(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True, exist_ok=True)


def exe_name(stem: str) -> str:
    return f"{stem}.exe" if sys.platform.startswith("win") else stem


def resolve_sengoo_root(bench_root: Path) -> Path:
    env_root = os.environ.get("SENGOO_ROOT")
    if env_root:
        root = Path(env_root).expanduser().resolve()
        if not (root / "Cargo.toml").exists():
            raise RuntimeError(f"SENGOO_ROOT does not look like Sengoo root: {root}")
        return root

    candidate = bench_root.parent
    if (candidate / "Cargo.toml").exists() and (candidate / "tools" / "sgc" / "src" / "main.rs").exists():
        return candidate

    raise RuntimeError("cannot resolve Sengoo source root; set SENGOO_ROOT")


def resolve_sgc_binary(sengoo_root: Path) -> Path:
    for candidate in [
        sengoo_root / "target" / "release" / exe_name("sgc"),
        sengoo_root / "target" / "debug" / exe_name("sgc"),
    ]:
        if candidate.exists():
            return candidate
    raise RuntimeError("sgc binary not found; run `cargo build -p sgc --release` first")


def run_python_reference(py: str, scenario_dir: Path, source: str) -> str:
    ref_path = scenario_dir / "reference.py"
    write_file(ref_path, source)
    proc = run_checked([py, str(ref_path)], cwd=scenario_dir)
    return last_nonempty_line(proc.stdout)


def run_scenario(
    scenario: dict[str, Any],
    scenario_dir: Path,
    sgc_bin: Path,
    sengoo_root: Path,
    py: str,
) -> dict[str, Any]:
    clear_dir(scenario_dir)
    for rel, content in scenario["sources"].items():
        write_file(scenario_dir / rel, content)

    expected = run_python_reference(py, scenario_dir, scenario["reference_py"])
    entry = scenario_dir / scenario["entry"]
    exe_path = scenario_dir / "build" / exe_name(Path(scenario["entry"]).stem)

    compile_samples: list[float] = []
    runtime_samples: list[float] = []
    errors: list[str] = []
    observed_output = None
    success = False
    compile_ok = False

    try:
        for _ in range(COMPILE_ITERS):
            compile_samples.append(
                measure_command_ms(
                    [str(sgc_bin), "build", str(entry), "-O", "2", "--force-rebuild"],
                    cwd=sengoo_root,
                )
            )
        compile_ok = True
    except Exception as exc:  # noqa: BLE001
        errors.append(f"compile: {exc}")

    if compile_ok:
        try:
            if not exe_path.exists():
                raise RuntimeError(f"expected executable not found: {exe_path}")
            for _ in range(RUNTIME_WARMUP):
                _ = measure_runtime_ms([str(exe_path)], expected, cwd=scenario_dir)
            for _ in range(RUNTIME_ITERS):
                runtime_samples.append(measure_runtime_ms([str(exe_path)], expected, cwd=scenario_dir))
            proc = run_checked([str(exe_path)], cwd=scenario_dir)
            observed_output = last_nonempty_line(proc.stdout)
            success = observed_output == expected
            if not success:
                errors.append(f"output mismatch: expected={expected}, got={observed_output}")
        except Exception as exc:  # noqa: BLE001
            errors.append(f"runtime: {exc}")

    return {
        "id": scenario["id"],
        "title": scenario["title"],
        "capability": scenario["capability"],
        "entry": scenario["entry"],
        "expected_output": expected,
        "observed_output": observed_output,
        "compile_avg_ms": average(compile_samples),
        "compile_p50_ms": percentile(compile_samples, 0.50),
        "runtime_avg_ms": average(runtime_samples),
        "runtime_p50_ms": percentile(runtime_samples, 0.50),
        "compile_samples_ms": compile_samples,
        "runtime_samples_ms": runtime_samples,
        "success": success,
        "errors": errors,
    }


def main() -> int:
    bench_root = Path(__file__).resolve().parent
    sengoo_root = resolve_sengoo_root(bench_root)
    sgc_bin = resolve_sgc_binary(sengoo_root)
    py = sys.executable

    results_dir = bench_root / "results"
    results_dir.mkdir(parents=True, exist_ok=True)
    work_dir = bench_root / ".bootstrap-generality-work"
    clear_dir(work_dir)

    results: list[dict[str, Any]] = []
    for scenario in SCENARIOS:
        scenario_dir = work_dir / scenario["id"]
        results.append(run_scenario(scenario, scenario_dir, sgc_bin, sengoo_root, py))

    total = len(results)
    passed = sum(1 for r in results if r["success"])
    pass_rate = (passed / total * 100.0) if total > 0 else 0.0
    capabilities = sorted({str(r["capability"]) for r in results})
    compile_avgs = [float(r["compile_avg_ms"]) for r in results if isinstance(r["compile_avg_ms"], (int, float))]
    runtime_p50s = [float(r["runtime_p50_ms"]) for r in results if isinstance(r["runtime_p50_ms"], (int, float))]

    proof_criteria = {
        "all_scenarios_pass": passed == total,
        "capabilities_covered_at_least_5": len(capabilities) >= 5,
        "no_missing_runtime_samples": all(bool(r["runtime_samples_ms"]) for r in results if r["success"]),
    }
    proof_status = "pass" if all(proof_criteria.values()) else "fail"

    report: dict[str, Any] = {
        "schema_version": 1,
        "generated_at_unix_ms": now_unix_ms(),
        "config": {
            "compile_iterations": COMPILE_ITERS,
            "runtime_warmup": RUNTIME_WARMUP,
            "runtime_iterations": RUNTIME_ITERS,
        },
        "bootstrap_proof": {
            "status": proof_status,
            "criteria": proof_criteria,
            "summary": {
                "total_scenarios": total,
                "passed_scenarios": passed,
                "pass_rate_pct": pass_rate,
                "capabilities_covered": capabilities,
                "compile_avg_ms_overall": average(compile_avgs),
                "runtime_p50_ms_overall": average(runtime_p50s),
            },
        },
        "scenarios": results,
        "notes": [
            "Each scenario is validated against a Python reference implementation output.",
            "This suite focuses on general-purpose feature coverage and correctness, then reports compile/runtime cost.",
            "import_graph currently validates multi-module compile graph participation (cross-module symbol calls are not yet covered).",
        ],
    }

    out_path = results_dir / f"{now_unix_ms()}-bootstrap-generality.json"
    out_path.write_text(json.dumps(report, indent=2), encoding="utf-8", newline="\n")

    print(f"Bootstrap generality report: {out_path}")
    print("")
    print("| Scenario | Capability | Compile avg (ms) | Runtime p50 (ms) | Pass |")
    print("|---|---|---:|---:|---|")
    for row in results:
        compile_str = "n/a" if row["compile_avg_ms"] is None else f"{float(row['compile_avg_ms']):.2f}"
        runtime_str = "n/a" if row["runtime_p50_ms"] is None else f"{float(row['runtime_p50_ms']):.2f}"
        pass_str = "yes" if row["success"] else "no"
        print(f"| {row['id']} | {row['capability']} | {compile_str} | {runtime_str} | {pass_str} |")

    print("")
    print(
        f"Bootstrap proof: {proof_status} "
        f"(pass_rate={pass_rate:.2f}%, capabilities={len(capabilities)})"
    )
    if proof_status != "pass":
        for row in results:
            if row["success"]:
                continue
            for err in row["errors"]:
                print(f"- {row['id']}: {err}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
