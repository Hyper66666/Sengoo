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
RUNTIME_ITERS = 7
COMPILE_ITERS = 3
INCREMENTAL_ITERS = 3


SCENARIOS: list[dict[str, str]] = [
    {
        "id": "arith_loop",
        "title": "Arithmetic Loop",
        "sengoo": """def main() -> i64 {
    let i = 0
    let acc = 0
    while i < 200000 {
        acc = acc + (i % 7)
        i = i + 1
    }
    print(acc)
    0
}
""",
        "cpp": """#include <iostream>

int main() {
    long long i = 0;
    long long acc = 0;
    while (i < 200000) {
        acc += (i % 7);
        i += 1;
    }
    std::cout << acc << "\\n";
    return 0;
}
""",
        "rust": """fn main() {
    let mut i: i64 = 0;
    let mut acc: i64 = 0;
    while i < 200000 {
        acc += i % 7;
        i += 1;
    }
    println!("{}", acc);
}
""",
        "python": """def main() -> None:
    i = 0
    acc = 0
    while i < 200000:
        acc += i % 7
        i += 1
    print(acc)


if __name__ == "__main__":
    main()
""",
    },
    {
        "id": "branch_mix",
        "title": "Branching Mix",
        "sengoo": """def main() -> i64 {
    let i = 0
    let acc = 0
    while i < 160000 {
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
        "cpp": """#include <iostream>

int main() {
    long long i = 0;
    long long acc = 0;
    while (i < 160000) {
        if (i % 2 < 1) {
            acc += i;
        } else {
            acc -= (i % 5);
        }
        i += 1;
    }
    std::cout << acc << "\\n";
    return 0;
}
""",
        "rust": """fn main() {
    let mut i: i64 = 0;
    let mut acc: i64 = 0;
    while i < 160000 {
        if i % 2 < 1 {
            acc += i;
        } else {
            acc -= i % 5;
        }
        i += 1;
    }
    println!("{}", acc);
}
""",
        "python": """def main() -> None:
    i = 0
    acc = 0
    while i < 160000:
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
        "id": "fn_call_hot",
        "title": "Hot Function Calls",
        "sengoo": """def helper(x: i64) -> i64 {
    x * 3 + 1
}

def main() -> i64 {
    let i = 0
    let acc = 0
    while i < 120000 {
        acc = acc + helper(i)
        i = i + 1
    }
    print(acc)
    0
}
""",
        "cpp": """#include <iostream>

static inline long long helper(long long x) {
    return x * 3 + 1;
}

int main() {
    long long i = 0;
    long long acc = 0;
    while (i < 120000) {
        acc += helper(i);
        i += 1;
    }
    std::cout << acc << "\\n";
    return 0;
}
""",
        "rust": """#[inline(always)]
fn helper(x: i64) -> i64 {
    x * 3 + 1
}

fn main() {
    let mut i: i64 = 0;
    let mut acc: i64 = 0;
    while i < 120000 {
        acc += helper(i);
        i += 1;
    }
    println!("{}", acc);
}
""",
        "python": """def helper(x: int) -> int:
    return x * 3 + 1


def main() -> None:
    i = 0
    acc = 0
    while i < 120000:
        acc += helper(i)
        i += 1
    print(acc)


if __name__ == "__main__":
    main()
""",
    },
    {
        "id": "array_index",
        "title": "Array Indexing",
        "sengoo": """def main() -> i64 {
    let arr = [1, 2, 3, 4, 5, 6, 7, 8]
    let i = 0
    let acc = 0
    while i < 200000 {
        acc = acc + arr[i % 8]
        i = i + 1
    }
    print(acc)
    0
}
""",
        "cpp": """#include <array>
#include <iostream>

int main() {
    std::array<long long, 8> arr = {1, 2, 3, 4, 5, 6, 7, 8};
    long long i = 0;
    long long acc = 0;
    while (i < 200000) {
        acc += arr[static_cast<size_t>(i % 8)];
        i += 1;
    }
    std::cout << acc << "\\n";
    return 0;
}
""",
        "rust": """fn main() {
    let arr: [i64; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    let mut i: i64 = 0;
    let mut acc: i64 = 0;
    while i < 200000 {
        acc += arr[(i % 8) as usize];
        i += 1;
    }
    println!("{}", acc);
}
""",
        "python": """def main() -> None:
    arr = [1, 2, 3, 4, 5, 6, 7, 8]
    i = 0
    acc = 0
    while i < 200000:
        acc += arr[i % 8]
        i += 1
    print(acc)


if __name__ == "__main__":
    main()
""",
    },
    {
        "id": "nested_loop",
        "title": "Nested Loop",
        "sengoo": """def main() -> i64 {
    let outer = 0
    let acc = 0
    while outer < 360 {
        let inner = 0
        while inner < 360 {
            acc = acc + ((outer * inner) % 7)
            inner = inner + 1
        }
        outer = outer + 1
    }
    print(acc)
    0
}
""",
        "cpp": """#include <iostream>

int main() {
    long long outer = 0;
    long long acc = 0;
    while (outer < 360) {
        long long inner = 0;
        while (inner < 360) {
            acc += ((outer * inner) % 7);
            inner += 1;
        }
        outer += 1;
    }
    std::cout << acc << "\\n";
    return 0;
}
""",
        "rust": """fn main() {
    let mut outer: i64 = 0;
    let mut acc: i64 = 0;
    while outer < 360 {
        let mut inner: i64 = 0;
        while inner < 360 {
            acc += (outer * inner) % 7;
            inner += 1;
        }
        outer += 1;
    }
    println!("{}", acc);
}
""",
        "python": """def main() -> None:
    outer = 0
    acc = 0
    while outer < 360:
        inner = 0
        while inner < 360:
            acc += (outer * inner) % 7
            inner += 1
        outer += 1
    print(acc)


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

    raise RuntimeError("cannot resolve Sengoo root; set SENGOO_ROOT")


def resolve_sgc_binary(sengoo_root: Path) -> Path:
    for candidate in [
        sengoo_root / "target" / "release" / exe_name("sgc"),
        sengoo_root / "target" / "debug" / exe_name("sgc"),
    ]:
        if candidate.exists():
            return candidate
    raise RuntimeError("sgc binary not found; run `cargo build -p sgc --release` first")


def make_sources(work_dir: Path) -> dict[str, dict[str, Path]]:
    paths: dict[str, dict[str, Path]] = {}
    ext = {"sengoo": "sg", "cpp": "cpp", "rust": "rs", "python": "py"}
    for scenario in SCENARIOS:
        sid = scenario["id"]
        paths[sid] = {}
        for lang in ["sengoo", "cpp", "rust", "python"]:
            source_path = work_dir / lang / f"{sid}.{ext[lang]}"
            write_file(source_path, scenario[lang])
            paths[sid][lang] = source_path
    return paths


def build_runtime_commands(
    scenario_paths: dict[str, dict[str, Path]],
    sgc_bin: Path,
    clangpp: str,
    rustc: str,
    py: str,
    sengoo_root: Path,
) -> dict[str, dict[str, list[str]]]:
    runtime_cmds: dict[str, dict[str, list[str]]] = {}
    for scenario in SCENARIOS:
        sid = scenario["id"]
        runtime_cmds[sid] = {}

        sg_path = scenario_paths[sid]["sengoo"]
        run_checked([str(sgc_bin), "build", str(sg_path), "-O", "2", "--force-rebuild"], cwd=sengoo_root)
        runtime_cmds[sid]["sengoo"] = [str(sg_path.parent / "build" / exe_name(sg_path.stem))]

        cpp_path = scenario_paths[sid]["cpp"]
        cpp_exe = cpp_path.parent / exe_name(f"{sid}_cpp")
        run_checked([clangpp, "-std=c++20", "-O2", str(cpp_path), "-o", str(cpp_exe)], cwd=sengoo_root)
        runtime_cmds[sid]["cpp"] = [str(cpp_exe)]

        rust_path = scenario_paths[sid]["rust"]
        rust_exe = rust_path.parent / exe_name(f"{sid}_rust")
        run_checked(
            [rustc, "--edition", "2021", "-C", "opt-level=2", str(rust_path), "-o", str(rust_exe)],
            cwd=sengoo_root,
        )
        runtime_cmds[sid]["rust"] = [str(rust_exe)]

        py_path = scenario_paths[sid]["python"]
        runtime_cmds[sid]["python"] = [py, str(py_path)]

    return runtime_cmds


def validate_outputs(runtime_cmds: dict[str, dict[str, list[str]]], sengoo_root: Path) -> dict[str, str]:
    expected: dict[str, str] = {}
    for sid, lang_cmds in runtime_cmds.items():
        outputs: dict[str, str] = {}
        for lang, cmd in lang_cmds.items():
            proc = run_checked(cmd, cwd=sengoo_root)
            outputs[lang] = last_nonempty_line(proc.stdout)
        unique = set(outputs.values())
        if len(unique) != 1:
            raise RuntimeError(f"output mismatch in scenario {sid}: {outputs}")
        expected[sid] = unique.pop()
    return expected


def benchmark_runtime(
    runtime_cmds: dict[str, dict[str, list[str]]],
    expected_outputs: dict[str, str],
    sengoo_root: Path,
) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for sid, lang_cmds in runtime_cmds.items():
        out[sid] = {}
        for lang, cmd in lang_cmds.items():
            for _ in range(RUNTIME_WARMUP):
                measure_runtime_ms(cmd, expected_outputs[sid], cwd=sengoo_root)
            samples = [
                measure_runtime_ms(cmd, expected_outputs[sid], cwd=sengoo_root)
                for _ in range(RUNTIME_ITERS)
            ]
            out[sid][lang] = {
                "samples_ms": samples,
                "p50_ms": percentile(samples, 0.50),
                "p95_ms": percentile(samples, 0.95),
            }
    return out


def benchmark_full_compile(
    scenario_paths: dict[str, dict[str, Path]],
    sgc_bin: Path,
    clangpp: str,
    rustc: str,
    py: str,
    sengoo_root: Path,
) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for scenario in SCENARIOS:
        sid = scenario["id"]
        out[sid] = {}

        sg_path = scenario_paths[sid]["sengoo"]
        sg_samples = [
            measure_command_ms([str(sgc_bin), "build", str(sg_path), "-O", "2", "--force-rebuild"], cwd=sengoo_root)
            for _ in range(COMPILE_ITERS)
        ]
        out[sid]["sengoo"] = {"samples_ms": sg_samples, "avg_ms": average(sg_samples), "p50_ms": percentile(sg_samples, 0.50)}

        cpp_path = scenario_paths[sid]["cpp"]
        cpp_exe = cpp_path.parent / exe_name(f"{sid}_cpp")
        cpp_samples = [
            measure_command_ms([clangpp, "-std=c++20", "-O2", str(cpp_path), "-o", str(cpp_exe)], cwd=sengoo_root)
            for _ in range(COMPILE_ITERS)
        ]
        out[sid]["cpp"] = {"samples_ms": cpp_samples, "avg_ms": average(cpp_samples), "p50_ms": percentile(cpp_samples, 0.50)}

        rust_path = scenario_paths[sid]["rust"]
        rust_exe = rust_path.parent / exe_name(f"{sid}_rust")
        rust_samples = [
            measure_command_ms(
                [rustc, "--edition", "2021", "-C", "opt-level=2", str(rust_path), "-o", str(rust_exe)],
                cwd=sengoo_root,
            )
            for _ in range(COMPILE_ITERS)
        ]
        out[sid]["rust"] = {"samples_ms": rust_samples, "avg_ms": average(rust_samples), "p50_ms": percentile(rust_samples, 0.50)}

        py_path = scenario_paths[sid]["python"]
        clear_pycache(py_path.parent)
        py_samples: list[float] = []
        for _ in range(COMPILE_ITERS):
            clear_pycache(py_path.parent)
            py_samples.append(measure_command_ms([py, "-m", "py_compile", str(py_path)], cwd=sengoo_root))
        out[sid]["python"] = {"samples_ms": py_samples, "avg_ms": average(py_samples), "p50_ms": percentile(py_samples, 0.50)}

    return out


def benchmark_incremental_compile(
    scenario_paths: dict[str, dict[str, Path]],
    sgc_bin: Path,
    clangpp: str,
    rustc: str,
    py: str,
    sengoo_root: Path,
) -> dict[str, dict[str, Any]]:
    out: dict[str, dict[str, Any]] = {}
    for scenario in SCENARIOS:
        sid = scenario["id"]
        out[sid] = {}

        # Sengoo: before is full rebuild, after is comment-only mutation with cache-enabled build.
        sg_path = scenario_paths[sid]["sengoo"]
        sg_original = sg_path.read_text(encoding="utf-8")
        sg_before: list[float] = []
        sg_after: list[float] = []
        try:
            for i in range(INCREMENTAL_ITERS):
                sg_path.write_text(sg_original, encoding="utf-8", newline="\n")
                sg_before.append(
                    measure_command_ms(
                        [str(sgc_bin), "build", str(sg_path), "-O", "2", "--force-rebuild"],
                        cwd=sengoo_root,
                    )
                )
                sg_path.write_text(f"{sg_original}\n// bench-mut-{i}\n", encoding="utf-8", newline="\n")
                sg_after.append(
                    measure_command_ms([str(sgc_bin), "build", str(sg_path), "-O", "2"], cwd=sengoo_root)
                )
        finally:
            sg_path.write_text(sg_original, encoding="utf-8", newline="\n")
        sg_before_avg = average(sg_before) or 0.0
        sg_after_avg = average(sg_after) or 0.0
        sg_reduction = ((sg_before_avg - sg_after_avg) / sg_before_avg * 100.0) if sg_before_avg > 0 else None
        out[sid]["sengoo"] = {
            "before_samples_ms": sg_before,
            "after_samples_ms": sg_after,
            "before_avg_ms": sg_before_avg,
            "after_avg_ms": sg_after_avg,
            "reduction_pct": sg_reduction,
        }

        # C++: same source file comment mutation and rebuild.
        cpp_path = scenario_paths[sid]["cpp"]
        cpp_exe = cpp_path.parent / exe_name(f"{sid}_cpp")
        cpp_original = cpp_path.read_text(encoding="utf-8")
        cpp_before: list[float] = []
        cpp_after: list[float] = []
        try:
            for i in range(INCREMENTAL_ITERS):
                cpp_path.write_text(cpp_original, encoding="utf-8", newline="\n")
                cpp_before.append(
                    measure_command_ms([clangpp, "-std=c++20", "-O2", str(cpp_path), "-o", str(cpp_exe)], cwd=sengoo_root)
                )
                cpp_path.write_text(f"{cpp_original}\n// bench-mut-{i}\n", encoding="utf-8", newline="\n")
                cpp_after.append(
                    measure_command_ms([clangpp, "-std=c++20", "-O2", str(cpp_path), "-o", str(cpp_exe)], cwd=sengoo_root)
                )
        finally:
            cpp_path.write_text(cpp_original, encoding="utf-8", newline="\n")
        cpp_before_avg = average(cpp_before) or 0.0
        cpp_after_avg = average(cpp_after) or 0.0
        cpp_reduction = ((cpp_before_avg - cpp_after_avg) / cpp_before_avg * 100.0) if cpp_before_avg > 0 else None
        out[sid]["cpp"] = {
            "before_samples_ms": cpp_before,
            "after_samples_ms": cpp_after,
            "before_avg_ms": cpp_before_avg,
            "after_avg_ms": cpp_after_avg,
            "reduction_pct": cpp_reduction,
        }

        # Rust: use rustc incremental cache for after run.
        rust_path = scenario_paths[sid]["rust"]
        rust_exe = rust_path.parent / exe_name(f"{sid}_rust")
        rust_cache_dir = rust_path.parent / ".rust-incremental-cache" / sid
        rust_original = rust_path.read_text(encoding="utf-8")
        rust_before: list[float] = []
        rust_after: list[float] = []
        try:
            for i in range(INCREMENTAL_ITERS):
                rust_path.write_text(rust_original, encoding="utf-8", newline="\n")
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
                            str(rust_path),
                            "-o",
                            str(rust_exe),
                        ],
                        cwd=sengoo_root,
                    )
                )
                rust_path.write_text(f"{rust_original}\n// bench-mut-{i}\n", encoding="utf-8", newline="\n")
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
                            str(rust_path),
                            "-o",
                            str(rust_exe),
                        ],
                        cwd=sengoo_root,
                    )
                )
        finally:
            rust_path.write_text(rust_original, encoding="utf-8", newline="\n")
        rust_before_avg = average(rust_before) or 0.0
        rust_after_avg = average(rust_after) or 0.0
        rust_reduction = ((rust_before_avg - rust_after_avg) / rust_before_avg * 100.0) if rust_before_avg > 0 else None
        out[sid]["rust"] = {
            "before_samples_ms": rust_before,
            "after_samples_ms": rust_after,
            "before_avg_ms": rust_before_avg,
            "after_avg_ms": rust_after_avg,
            "reduction_pct": rust_reduction,
        }

        # Python: py_compile with comment mutation.
        py_path = scenario_paths[sid]["python"]
        py_original = py_path.read_text(encoding="utf-8")
        py_before: list[float] = []
        py_after: list[float] = []
        try:
            for i in range(INCREMENTAL_ITERS):
                py_path.write_text(py_original, encoding="utf-8", newline="\n")
                clear_pycache(py_path.parent)
                py_before.append(measure_command_ms([py, "-m", "py_compile", str(py_path)], cwd=sengoo_root))
                py_path.write_text(f"{py_original}\n# bench-mut-{i}\n", encoding="utf-8", newline="\n")
                py_after.append(measure_command_ms([py, "-m", "py_compile", str(py_path)], cwd=sengoo_root))
        finally:
            py_path.write_text(py_original, encoding="utf-8", newline="\n")
        py_before_avg = average(py_before) or 0.0
        py_after_avg = average(py_after) or 0.0
        py_reduction = ((py_before_avg - py_after_avg) / py_before_avg * 100.0) if py_before_avg > 0 else None
        out[sid]["python"] = {
            "before_samples_ms": py_before,
            "after_samples_ms": py_after,
            "before_avg_ms": py_before_avg,
            "after_avg_ms": py_after_avg,
            "reduction_pct": py_reduction,
        }

    return out


def pct_vs(reference: float | None, current: float | None) -> float | None:
    if reference is None or current is None or reference == 0:
        return None
    return ((current - reference) / reference) * 100.0


def main() -> int:
    bench_root = Path(__file__).resolve().parent
    sengoo_root = resolve_sengoo_root(bench_root)
    results_dir = bench_root / "results"
    results_dir.mkdir(parents=True, exist_ok=True)
    work_dir = bench_root / ".scenario-matrix-work"
    if work_dir.exists():
        shutil.rmtree(work_dir)
    work_dir.mkdir(parents=True, exist_ok=True)

    sgc_bin = resolve_sgc_binary(sengoo_root)
    clangpp = require_tool("clang++")
    rustc = require_tool("rustc")
    py = sys.executable

    scenario_paths = make_sources(work_dir)
    runtime_cmds = build_runtime_commands(scenario_paths, sgc_bin, clangpp, rustc, py, sengoo_root)
    expected_outputs = validate_outputs(runtime_cmds, sengoo_root)
    runtime_results = benchmark_runtime(runtime_cmds, expected_outputs, sengoo_root)
    compile_results = benchmark_full_compile(scenario_paths, sgc_bin, clangpp, rustc, py, sengoo_root)
    incremental_results = benchmark_incremental_compile(
        scenario_paths, sgc_bin, clangpp, rustc, py, sengoo_root
    )

    comparisons: dict[str, dict[str, Any]] = {}
    for scenario in SCENARIOS:
        sid = scenario["id"]
        comparisons[sid] = {
            "runtime_p50_vs_cpp_pct": {},
            "compile_avg_vs_cpp_pct": {},
            "incremental_reduction_pct": {},
        }
        cpp_runtime = runtime_results[sid]["cpp"]["p50_ms"]
        cpp_compile = compile_results[sid]["cpp"]["avg_ms"]
        for lang in ["sengoo", "cpp", "rust", "python"]:
            comparisons[sid]["runtime_p50_vs_cpp_pct"][lang] = pct_vs(cpp_runtime, runtime_results[sid][lang]["p50_ms"])
            comparisons[sid]["compile_avg_vs_cpp_pct"][lang] = pct_vs(cpp_compile, compile_results[sid][lang]["avg_ms"])
            comparisons[sid]["incremental_reduction_pct"][lang] = incremental_results[sid][lang]["reduction_pct"]

    langs = ["sengoo", "cpp", "rust", "python"]
    runtime_p50_avg = {
        lang: average([runtime_results[s["id"]][lang]["p50_ms"] for s in SCENARIOS]) for lang in langs
    }
    compile_avg = {
        lang: average([compile_results[s["id"]][lang]["avg_ms"] for s in SCENARIOS]) for lang in langs
    }
    inc_before_avg = {
        lang: average([incremental_results[s["id"]][lang]["before_avg_ms"] for s in SCENARIOS]) for lang in langs
    }
    inc_after_avg = {
        lang: average([incremental_results[s["id"]][lang]["after_avg_ms"] for s in SCENARIOS]) for lang in langs
    }
    inc_reduction_avg = {
        lang: average([incremental_results[s["id"]][lang]["reduction_pct"] for s in SCENARIOS]) for lang in langs
    }

    report: dict[str, Any] = {
        "schema_version": 1,
        "generated_at_unix_ms": now_unix_ms(),
        "config": {
            "runtime": {"warmup": RUNTIME_WARMUP, "iterations": RUNTIME_ITERS},
            "full_compile": {"iterations": COMPILE_ITERS},
            "incremental_compile": {"iterations": INCREMENTAL_ITERS},
        },
        "scenarios": [{"id": s["id"], "title": s["title"]} for s in SCENARIOS],
        "runtime": runtime_results,
        "full_compile": compile_results,
        "incremental_compile": incremental_results,
        "summary": {
            "runtime_p50_avg_ms": runtime_p50_avg,
            "full_compile_avg_ms": compile_avg,
            "incremental_before_avg_ms": inc_before_avg,
            "incremental_after_avg_ms": inc_after_avg,
            "incremental_reduction_avg_pct": inc_reduction_avg,
        },
        "comparisons": comparisons,
        "notes": [
            f"Sengoo root: {sengoo_root}",
            "Runtime measures process execution time only (compile excluded).",
            "Full compile uses O2 for Sengoo/C++/Rust, py_compile for Python.",
            "Incremental compile uses comment-only source mutation and language-native incremental behavior.",
            "All four languages are validated to produce identical scenario outputs before timing.",
        ],
    }

    out_path = results_dir / f"{now_unix_ms()}-scenario-matrix.json"
    out_path.write_text(json.dumps(report, indent=2), encoding="utf-8", newline="\n")

    print(f"Scenario matrix report: {out_path}")
    print("")
    print("| Metric | Sengoo | C++ | Rust | Python |")
    print("|---|---:|---:|---:|---:|")
    print("| Runtime p50 avg (ms) | " + " | ".join(f"{runtime_p50_avg[lang]:.2f}" for lang in langs) + " |")
    print("| Full compile avg (ms) | " + " | ".join(f"{compile_avg[lang]:.2f}" for lang in langs) + " |")
    print("| Incremental before avg (ms) | " + " | ".join(f"{inc_before_avg[lang]:.2f}" for lang in langs) + " |")
    print("| Incremental after avg (ms) | " + " | ".join(f"{inc_after_avg[lang]:.2f}" for lang in langs) + " |")
    print("| Incremental reduction avg (%) | " + " | ".join(f"{inc_reduction_avg[lang]:.2f}" for lang in langs) + " |")
    print("")
    print("Runtime p50 (ms)")
    print("| Scenario | Sengoo | C++ | Rust | Python |")
    print("|---|---:|---:|---:|---:|")
    for scenario in SCENARIOS:
        sid = scenario["id"]
        row = " | ".join(f"{runtime_results[sid][lang]['p50_ms']:.2f}" for lang in langs)
        print(f"| {sid} | {row} |")
    print("")
    print("Full compile avg (ms)")
    print("| Scenario | Sengoo | C++ | Rust | Python |")
    print("|---|---:|---:|---:|---:|")
    for scenario in SCENARIOS:
        sid = scenario["id"]
        row = " | ".join(f"{compile_results[sid][lang]['avg_ms']:.2f}" for lang in langs)
        print(f"| {sid} | {row} |")
    print("")
    print("Incremental reduction (%)")
    print("| Scenario | Sengoo | C++ | Rust | Python |")
    print("|---|---:|---:|---:|---:|")
    for scenario in SCENARIOS:
        sid = scenario["id"]
        row = " | ".join(f"{incremental_results[sid][lang]['reduction_pct']:.2f}" for lang in langs)
        print(f"| {sid} | {row} |")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
