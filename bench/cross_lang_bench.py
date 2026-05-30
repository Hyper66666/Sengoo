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


RUNTIME_WARMUP = 1
RUNTIME_ITERS = 5
COMPILE_ITERS = 3
INCREMENTAL_ITERS = 3
EXPECTED_RUNTIME_OUTPUT = "149997"
EXPECTED_INCREMENTAL_OUTPUT = "43"


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


def require_tool(name: str) -> str:
    resolved = shutil.which(name)
    if not resolved:
        raise RuntimeError(f"required tool not found in PATH: {name}")
    return resolved


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
        joined = " ".join(cmd)
        raise RuntimeError(
            f"command failed ({proc.returncode}): {joined}\nstdout:\n{proc.stdout}\nstderr:\n{proc.stderr}"
        )
    return proc


def measure_command_ms(cmd: list[str], cwd: Path | None = None) -> float:
    started = time.perf_counter()
    run_checked(cmd, cwd=cwd)
    return (time.perf_counter() - started) * 1000.0


def measure_commands_ms(commands: list[list[str]], cwd: Path | None = None) -> float:
    started = time.perf_counter()
    for cmd in commands:
        run_checked(cmd, cwd=cwd)
    return (time.perf_counter() - started) * 1000.0


def measure_runtime_ms(cmd: list[str], expected_output: str, cwd: Path | None = None) -> float:
    started = time.perf_counter()
    proc = run_checked(cmd, cwd=cwd)
    elapsed = (time.perf_counter() - started) * 1000.0
    lines = [line.strip() for line in proc.stdout.splitlines() if line.strip()]
    if expected_output not in lines:
        raise RuntimeError(
            f"unexpected runtime output for {' '.join(cmd)}: expected '{expected_output}', got '{proc.stdout.strip()}'"
        )
    return elapsed


def clear_pycache(path: Path) -> None:
    pycache = path / "__pycache__"
    if pycache.exists():
        shutil.rmtree(pycache)


def write_file(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


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

    raise RuntimeError(
        "cannot resolve Sengoo source root; set SENGOO_ROOT to the Sengoo repository path"
    )


def resolve_sgc_binary(sengoo_root: Path) -> Path:
    candidates = [
        sengoo_root / "target" / "release" / exe_name("sgc"),
        sengoo_root / "target" / "debug" / exe_name("sgc"),
    ]
    for candidate in candidates:
        if candidate.exists():
            return candidate
    raise RuntimeError(
        f"sgc binary not found under {sengoo_root / 'target'}; run cargo build -p sgc --release first"
    )


def make_sources(work_dir: Path) -> dict[str, dict[str, Path]]:
    sources: dict[str, dict[str, Path]] = {
        "sengoo": {},
        "cpp": {},
        "rust": {},
        "python": {},
    }

    # Runtime workload: same math loop as bench/suites/runtime/basic_loop.sg
    write_file(
        work_dir / "sengoo" / "runtime" / "basic_loop.sg",
        """def main() -> i64 {
    let n = 50000
    let acc = 0
    let i = 0
    while i < n {
        acc = acc + (i % 7)
        i = i + 1
    }
    print(acc)
    0
}
""",
    )
    write_file(
        work_dir / "cpp" / "runtime" / "basic_loop.cpp",
        """#include <iostream>

int main() {
    long long n = 50000;
    long long acc = 0;
    for (long long i = 0; i < n; ++i) {
        acc += (i % 7);
    }
    std::cout << acc << "\\n";
    return 0;
}
""",
    )
    write_file(
        work_dir / "rust" / "runtime" / "basic_loop.rs",
        """fn main() {
    let n: i64 = 50_000;
    let mut acc: i64 = 0;
    let mut i: i64 = 0;
    while i < n {
        acc += i % 7;
        i += 1;
    }
    println!("{}", acc);
}
""",
    )
    write_file(
        work_dir / "python" / "runtime" / "basic_loop.py",
        """def main() -> None:
    n = 50000
    acc = 0
    i = 0
    while i < n:
        acc += i % 7
        i += 1
    print(acc)


if __name__ == "__main__":
    main()
""",
    )

    # Full compile workload: same shape as bench/suites/compile/mod_tree_root.sg
    write_file(
        work_dir / "sengoo" / "compile" / "mod_tree_root.sg",
        """def helper(x: i64) -> i64 {
    x + 1
}

def main() -> i64 {
    let i = 0
    let sum = 0
    while i < 10000 {
        sum = sum + helper(i)
        i = i + 1
    }
    print(sum)
    0
}
""",
    )
    write_file(
        work_dir / "cpp" / "compile" / "mod_tree_root.cpp",
        """#include <iostream>

long long helper(long long x) { return x + 1; }

int main() {
    long long sum = 0;
    for (long long i = 0; i < 10000; ++i) {
        sum += helper(i);
    }
    std::cout << sum << "\\n";
    return 0;
}
""",
    )
    write_file(
        work_dir / "rust" / "compile" / "mod_tree_root.rs",
        """fn helper(x: i64) -> i64 {
    x + 1
}

fn main() {
    let mut sum: i64 = 0;
    let mut i: i64 = 0;
    while i < 10000 {
        sum += helper(i);
        i += 1;
    }
    println!("{}", sum);
}
""",
    )
    write_file(
        work_dir / "python" / "compile" / "mod_tree_root.py",
        """def helper(x: int) -> int:
    return x + 1


def main() -> None:
    i = 0
    total = 0
    while i < 10000:
        total += helper(i)
        i += 1
    print(total)


if __name__ == "__main__":
    main()
""",
    )

    # Incremental workload: root + util module. Mutate root implementation with comment.
    write_file(
        work_dir / "sengoo" / "incremental" / "math_util.sg",
        """def double(x: i64) -> i64 {
    x * 2
}
""",
    )
    write_file(
        work_dir / "sengoo" / "incremental" / "change_impl_root.sg",
        """import math_util;

def calc(x: i64) -> i64 {
    x * 2 + 1
}

def main() -> i64 {
    print(calc(21))
    0
}
""",
    )
    write_file(
        work_dir / "cpp" / "incremental" / "math_util.hpp",
        """#pragma once

long long double_value(long long x);
""",
    )
    write_file(
        work_dir / "cpp" / "incremental" / "math_util.cpp",
        """#include "math_util.hpp"

long long double_value(long long x) { return x * 2; }
""",
    )
    write_file(
        work_dir / "cpp" / "incremental" / "change_impl_root.cpp",
        """#include <iostream>
#include "math_util.hpp"

long long calc(long long x) {
    return double_value(x) + 1;
}

int main() {
    std::cout << calc(21) << "\\n";
    return 0;
}
""",
    )
    write_file(
        work_dir / "rust" / "incremental" / "math_util.rs",
        """pub fn double_value(x: i64) -> i64 {
    x * 2
}
""",
    )
    write_file(
        work_dir / "rust" / "incremental" / "change_impl_root.rs",
        """mod math_util;

fn calc(x: i64) -> i64 {
    math_util::double_value(x) + 1
}

fn main() {
    println!("{}", calc(21));
}
""",
    )
    write_file(
        work_dir / "python" / "incremental" / "math_util.py",
        """def double_value(x: int) -> int:
    return x * 2
""",
    )
    write_file(
        work_dir / "python" / "incremental" / "change_impl_root.py",
        """import math_util


def calc(x: int) -> int:
    return math_util.double_value(x) + 1


def main() -> None:
    print(calc(21))


if __name__ == "__main__":
    main()
""",
    )

    for lang in sources:
        sources[lang]["runtime"] = work_dir / lang / "runtime"
        sources[lang]["compile"] = work_dir / lang / "compile"
        sources[lang]["incremental"] = work_dir / lang / "incremental"
    return sources


def main() -> int:
    bench_root = Path(__file__).resolve().parent
    sengoo_root = resolve_sengoo_root(bench_root)
    results_dir = bench_root / "results"
    results_dir.mkdir(parents=True, exist_ok=True)
    work_dir = bench_root / ".cross-lang-work"
    if work_dir.exists():
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    sgc_bin = resolve_sgc_binary(sengoo_root)
    clangpp = require_tool("clang++")
    rustc = require_tool("rustc")
    py = sys.executable

    sources = make_sources(work_dir)

    # Runtime benchmark
    runtime_cmds: dict[str, list[str]] = {}
    sg_runtime_case = sources["sengoo"]["runtime"] / "basic_loop.sg"
    run_checked([str(sgc_bin), "build", str(sg_runtime_case), "-O", "1", "--force-rebuild"], cwd=sengoo_root)
    runtime_cmds["sengoo"] = [str(sources["sengoo"]["runtime"] / "build" / exe_name("basic_loop"))]

    cpp_runtime_case = sources["cpp"]["runtime"] / "basic_loop.cpp"
    cpp_runtime_exe = sources["cpp"]["runtime"] / exe_name("basic_loop_cpp")
    run_checked([clangpp, "-std=c++20", "-O1", str(cpp_runtime_case), "-o", str(cpp_runtime_exe)], cwd=sengoo_root)
    runtime_cmds["cpp"] = [str(cpp_runtime_exe)]

    rust_runtime_case = sources["rust"]["runtime"] / "basic_loop.rs"
    rust_runtime_exe = sources["rust"]["runtime"] / exe_name("basic_loop_rust")
    run_checked(
        [rustc, "--edition", "2021", "-C", "opt-level=1", str(rust_runtime_case), "-o", str(rust_runtime_exe)],
        cwd=sengoo_root,
    )
    runtime_cmds["rust"] = [str(rust_runtime_exe)]

    py_runtime_case = sources["python"]["runtime"] / "basic_loop.py"
    runtime_cmds["python"] = [py, str(py_runtime_case)]

    runtime_results: dict[str, dict[str, Any]] = {}
    for lang, cmd in runtime_cmds.items():
        for _ in range(RUNTIME_WARMUP):
            measure_runtime_ms(cmd, EXPECTED_RUNTIME_OUTPUT, cwd=sengoo_root)
        samples = [measure_runtime_ms(cmd, EXPECTED_RUNTIME_OUTPUT, cwd=sengoo_root) for _ in range(RUNTIME_ITERS)]
        runtime_results[lang] = {
            "samples_ms": samples,
            "p50_ms": percentile(samples, 0.50),
            "p95_ms": percentile(samples, 0.95),
        }

    # Full compile benchmark
    full_compile_results: dict[str, dict[str, Any]] = {}

    sg_compile_case = sources["sengoo"]["compile"] / "mod_tree_root.sg"
    sg_compile_samples = [
        measure_command_ms([str(sgc_bin), "build", str(sg_compile_case), "-O", "2", "--force-rebuild"], cwd=sengoo_root)
        for _ in range(COMPILE_ITERS)
    ]
    full_compile_results["sengoo"] = {
        "samples_ms": sg_compile_samples,
        "avg_ms": average(sg_compile_samples),
        "p50_ms": percentile(sg_compile_samples, 0.50),
    }

    cpp_compile_case = sources["cpp"]["compile"] / "mod_tree_root.cpp"
    cpp_compile_exe = sources["cpp"]["compile"] / exe_name("mod_tree_root_cpp")
    cpp_compile_samples = [
        measure_command_ms([clangpp, "-std=c++20", "-O0", str(cpp_compile_case), "-o", str(cpp_compile_exe)], cwd=sengoo_root)
        for _ in range(COMPILE_ITERS)
    ]
    full_compile_results["cpp"] = {
        "samples_ms": cpp_compile_samples,
        "avg_ms": average(cpp_compile_samples),
        "p50_ms": percentile(cpp_compile_samples, 0.50),
    }

    rust_compile_case = sources["rust"]["compile"] / "mod_tree_root.rs"
    rust_compile_exe = sources["rust"]["compile"] / exe_name("mod_tree_root_rust")
    rust_compile_samples = [
        measure_command_ms(
            [rustc, "--edition", "2021", "-C", "opt-level=0", str(rust_compile_case), "-o", str(rust_compile_exe)],
            cwd=sengoo_root,
        )
        for _ in range(COMPILE_ITERS)
    ]
    full_compile_results["rust"] = {
        "samples_ms": rust_compile_samples,
        "avg_ms": average(rust_compile_samples),
        "p50_ms": percentile(rust_compile_samples, 0.50),
    }

    py_compile_dir = sources["python"]["compile"]
    py_compile_case = py_compile_dir / "mod_tree_root.py"
    py_compile_samples: list[float] = []
    for _ in range(COMPILE_ITERS):
        clear_pycache(py_compile_dir)
        py_compile_samples.append(measure_command_ms([py, "-m", "py_compile", str(py_compile_case)], cwd=sengoo_root))
    full_compile_results["python"] = {
        "samples_ms": py_compile_samples,
        "avg_ms": average(py_compile_samples),
        "p50_ms": percentile(py_compile_samples, 0.50),
    }

    # Incremental compile benchmark
    incremental_results: dict[str, dict[str, Any]] = {}

    # Sengoo incremental: before uses --force-rebuild, after uses cache with root implementation-only change.
    sg_inc_dir = sources["sengoo"]["incremental"]
    sg_inc_root = sg_inc_dir / "change_impl_root.sg"
    sg_original = sg_inc_root.read_text(encoding="utf-8")
    sg_before: list[float] = []
    sg_after: list[float] = []
    try:
        for i in range(INCREMENTAL_ITERS):
            sg_inc_root.write_text(sg_original, encoding="utf-8", newline="\n")
            sg_before.append(
                measure_command_ms(
                    [str(sgc_bin), "build", str(sg_inc_root), "-O", "2", "--force-rebuild"],
                    cwd=sengoo_root,
                )
            )
            sg_inc_root.write_text(f"{sg_original}\n// bench-mut-{i}\n", encoding="utf-8", newline="\n")
            sg_after.append(
                measure_command_ms([str(sgc_bin), "build", str(sg_inc_root), "-O", "2"], cwd=sengoo_root)
            )
    finally:
        sg_inc_root.write_text(sg_original, encoding="utf-8", newline="\n")
    sg_before_avg = average(sg_before) or 0.0
    sg_after_avg = average(sg_after) or 0.0
    sg_reduction = ((sg_before_avg - sg_after_avg) / sg_before_avg * 100.0) if sg_before_avg > 0 else None
    incremental_results["sengoo"] = {
        "before_samples_ms": sg_before,
        "after_samples_ms": sg_after,
        "before_avg_ms": sg_before_avg,
        "after_avg_ms": sg_after_avg,
        "reduction_pct": sg_reduction,
    }

    # C++ incremental: full rebuild before, then rebuild changed root object + link.
    cpp_inc_dir = sources["cpp"]["incremental"]
    cpp_main = cpp_inc_dir / "change_impl_root.cpp"
    cpp_util = cpp_inc_dir / "math_util.cpp"
    cpp_main_obj = cpp_inc_dir / "change_impl_root.obj"
    cpp_util_obj = cpp_inc_dir / "math_util.obj"
    cpp_inc_exe = cpp_inc_dir / exe_name("change_impl_root_cpp")
    cpp_original = cpp_main.read_text(encoding="utf-8")
    cpp_before: list[float] = []
    cpp_after: list[float] = []
    try:
        for i in range(INCREMENTAL_ITERS):
            cpp_main.write_text(cpp_original, encoding="utf-8", newline="\n")
            cpp_before.append(
                measure_commands_ms(
                    [
                        [clangpp, "-std=c++20", "-O2", "-c", str(cpp_util), "-o", str(cpp_util_obj)],
                        [clangpp, "-std=c++20", "-O2", "-c", str(cpp_main), "-o", str(cpp_main_obj)],
                        [clangpp, str(cpp_main_obj), str(cpp_util_obj), "-o", str(cpp_inc_exe)],
                    ],
                    cwd=sengoo_root,
                )
            )
            cpp_main.write_text(f"{cpp_original}\n// bench-mut-{i}\n", encoding="utf-8", newline="\n")
            cpp_after.append(
                measure_commands_ms(
                    [
                        [clangpp, "-std=c++20", "-O2", "-c", str(cpp_main), "-o", str(cpp_main_obj)],
                        [clangpp, str(cpp_main_obj), str(cpp_util_obj), "-o", str(cpp_inc_exe)],
                    ],
                    cwd=sengoo_root,
                )
            )
    finally:
        cpp_main.write_text(cpp_original, encoding="utf-8", newline="\n")
    cpp_before_avg = average(cpp_before) or 0.0
    cpp_after_avg = average(cpp_after) or 0.0
    cpp_reduction = ((cpp_before_avg - cpp_after_avg) / cpp_before_avg * 100.0) if cpp_before_avg > 0 else None
    incremental_results["cpp"] = {
        "before_samples_ms": cpp_before,
        "after_samples_ms": cpp_after,
        "before_avg_ms": cpp_before_avg,
        "after_avg_ms": cpp_after_avg,
        "reduction_pct": cpp_reduction,
    }

    # Rust incremental: same incremental cache directory reused after source implementation-only change.
    rust_inc_dir = sources["rust"]["incremental"]
    rust_root = rust_inc_dir / "change_impl_root.rs"
    rust_inc_exe = rust_inc_dir / exe_name("change_impl_root_rust")
    rust_cache_dir = rust_inc_dir / ".rust-incremental-cache"
    rust_original = rust_root.read_text(encoding="utf-8")
    rust_before: list[float] = []
    rust_after: list[float] = []
    try:
        for i in range(INCREMENTAL_ITERS):
            rust_root.write_text(rust_original, encoding="utf-8", newline="\n")
            if rust_cache_dir.exists():
                shutil.rmtree(rust_cache_dir)
            rust_before.append(
                measure_command_ms(
                    [
                        rustc,
                        "--edition",
                        "2021",
                        "-C",
                        "opt-level=2",
                        "-C",
                        f"incremental={rust_cache_dir}",
                        str(rust_root),
                        "-o",
                        str(rust_inc_exe),
                    ],
                    cwd=sengoo_root,
                )
            )
            rust_root.write_text(f"{rust_original}\n// bench-mut-{i}\n", encoding="utf-8", newline="\n")
            rust_after.append(
                measure_command_ms(
                    [
                        rustc,
                        "--edition",
                        "2021",
                        "-C",
                        "opt-level=2",
                        "-C",
                        f"incremental={rust_cache_dir}",
                        str(rust_root),
                        "-o",
                        str(rust_inc_exe),
                    ],
                    cwd=sengoo_root,
                )
            )
    finally:
        rust_root.write_text(rust_original, encoding="utf-8", newline="\n")
    rust_before_avg = average(rust_before) or 0.0
    rust_after_avg = average(rust_after) or 0.0
    rust_reduction = ((rust_before_avg - rust_after_avg) / rust_before_avg * 100.0) if rust_before_avg > 0 else None
    incremental_results["rust"] = {
        "before_samples_ms": rust_before,
        "after_samples_ms": rust_after,
        "before_avg_ms": rust_before_avg,
        "after_avg_ms": rust_after_avg,
        "reduction_pct": rust_reduction,
    }

    # Python incremental: compileall with timestamp cache, mutate root file and recompile.
    py_inc_dir = sources["python"]["incremental"]
    py_root = py_inc_dir / "change_impl_root.py"
    py_original = py_root.read_text(encoding="utf-8")
    py_before: list[float] = []
    py_after: list[float] = []
    try:
        for i in range(INCREMENTAL_ITERS):
            py_root.write_text(py_original, encoding="utf-8", newline="\n")
            clear_pycache(py_inc_dir)
            py_before.append(measure_command_ms([py, "-m", "compileall", "-q", str(py_inc_dir)], cwd=sengoo_root))
            py_root.write_text(f"{py_original}\n# bench-mut-{i}\n", encoding="utf-8", newline="\n")
            py_after.append(measure_command_ms([py, "-m", "compileall", "-q", str(py_inc_dir)], cwd=sengoo_root))
    finally:
        py_root.write_text(py_original, encoding="utf-8", newline="\n")
    py_before_avg = average(py_before) or 0.0
    py_after_avg = average(py_after) or 0.0
    py_reduction = ((py_before_avg - py_after_avg) / py_before_avg * 100.0) if py_before_avg > 0 else None
    incremental_results["python"] = {
        "before_samples_ms": py_before,
        "after_samples_ms": py_after,
        "before_avg_ms": py_before_avg,
        "after_avg_ms": py_after_avg,
        "reduction_pct": py_reduction,
    }

    # Ensure incremental workload still computes expected result.
    check_cmds = {
        "sengoo": [str(sgc_bin), "run", str(sg_inc_root), "--force-rebuild"],
        "cpp": [str(cpp_inc_exe)],
        "rust": [str(rust_inc_exe)],
        "python": [py, str(py_root)],
    }
    for lang, cmd in check_cmds.items():
        _ = measure_runtime_ms(cmd, EXPECTED_INCREMENTAL_OUTPUT, cwd=sengoo_root)

    report: dict[str, Any] = {
        "schema_version": 1,
        "generated_at_unix_ms": now_unix_ms(),
        "config": {
            "runtime": {"warmup": RUNTIME_WARMUP, "iterations": RUNTIME_ITERS},
            "full_compile": {"iterations": COMPILE_ITERS},
            "incremental_compile": {"iterations": INCREMENTAL_ITERS},
        },
        "runtime": runtime_results,
        "full_compile": full_compile_results,
        "incremental_compile": incremental_results,
        "notes": [
            f"Sengoo root: {sengoo_root}",
            "Runtime is process execution time only (compile step excluded).",
            "Full compile uses O2 for Sengoo, O0 for C++/Rust, py_compile for Python.",
            "Incremental compile uses implementation-only root mutation and language-native cache behavior.",
        ],
    }

    out_path = results_dir / f"{now_unix_ms()}-cross-language.json"
    out_path.write_text(json.dumps(report, indent=2), encoding="utf-8", newline="\n")

    langs = ["sengoo", "cpp", "rust", "python"]
    print(f"Cross-language benchmark report: {out_path}")
    print("")
    print("| Metric | Sengoo | C++ | Rust | Python |")
    print("|---|---:|---:|---:|---:|")
    print(
        "| Runtime p50 (ms) | "
        + " | ".join(f"{runtime_results[lang]['p50_ms']:.2f}" for lang in langs)
        + " |"
    )
    print(
        "| Full compile avg (ms) | "
        + " | ".join(f"{full_compile_results[lang]['avg_ms']:.2f}" for lang in langs)
        + " |"
    )
    print(
        "| Incremental before avg (ms) | "
        + " | ".join(f"{incremental_results[lang]['before_avg_ms']:.2f}" for lang in langs)
        + " |"
    )
    print(
        "| Incremental after avg (ms) | "
        + " | ".join(f"{incremental_results[lang]['after_avg_ms']:.2f}" for lang in langs)
        + " |"
    )
    print(
        "| Incremental reduction (%) | "
        + " | ".join(f"{incremental_results[lang]['reduction_pct']:.2f}" for lang in langs)
        + " |"
    )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
