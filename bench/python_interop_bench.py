#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import os
import shlex
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


DEFAULT_CALLS = 20000
DEFAULT_SAMPLES = 5
DEFAULT_WARMUP = 1


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


def exe_name(stem: str) -> str:
    return f"{stem}.exe" if sys.platform.startswith("win") else stem


def clear_dir(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True, exist_ok=True)


def write_file(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


def run_checked(
    cmd: list[str],
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)
    proc = subprocess.run(
        cmd,
        cwd=str(cwd) if cwd else None,
        env=merged_env,
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


def measure_command_ms(
    cmd: list[str],
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> float:
    started = time.perf_counter()
    run_checked(cmd, cwd=cwd, env=env)
    return (time.perf_counter() - started) * 1000.0


def parse_json_from_stdout(stdout: str) -> dict[str, Any]:
    for line in reversed(stdout.splitlines()):
        candidate = line.strip()
        if not candidate:
            continue
        try:
            value = json.loads(candidate)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    raise RuntimeError(f"runner output does not contain JSON object:\n{stdout}")


def resolve_sengoo_root(bench_root: Path) -> Path:
    env_root = os.environ.get("SENGOO_ROOT")
    if env_root:
        root = Path(env_root).expanduser().resolve()
        if not (root / "Cargo.toml").exists():
            raise RuntimeError(f"SENGOO_ROOT does not look like Sengoo root: {root}")
        return root

    candidate = bench_root.parent
    if (candidate / "Cargo.toml").exists() and (candidate / "runtime" / "Cargo.toml").exists():
        return candidate

    raise RuntimeError("cannot resolve Sengoo source root; set SENGOO_ROOT")


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Benchmark Python interoperability overhead across Sengoo runtime, Rust, C++, and Python native."
        )
    )
    parser.add_argument("--calls", type=int, default=DEFAULT_CALLS, help=f"calls per sample (default: {DEFAULT_CALLS})")
    parser.add_argument("--samples", type=int, default=DEFAULT_SAMPLES, help=f"timed samples per runner (default: {DEFAULT_SAMPLES})")
    parser.add_argument("--warmup", type=int, default=DEFAULT_WARMUP, help=f"warmup runs per runner (default: {DEFAULT_WARMUP})")
    return parser.parse_args()


def prepare_workload(work_dir: Path) -> tuple[Path, Path]:
    module_dir = work_dir / "workload"
    module_dir.mkdir(parents=True, exist_ok=True)
    workload_file = module_dir / "interop_workload.py"
    write_file(
        workload_file,
        """def hot_fn(x: int) -> int:
    return ((x * 3 + 7) ^ 0x5A5A) & 0x7FFFFFFF
""",
    )

    runner_py = work_dir / "python_native_runner.py"
    write_file(
        runner_py,
        """#!/usr/bin/env python3
from __future__ import annotations

import importlib
import json
import sys
import time
from pathlib import Path


def main() -> int:
    module_dir = Path(sys.argv[1]).resolve()
    calls = int(sys.argv[2])
    sys.path.insert(0, str(module_dir))

    t0 = time.perf_counter()
    module = importlib.import_module("interop_workload")
    fn = module.hot_fn
    init_ms = (time.perf_counter() - t0) * 1000.0

    t1 = time.perf_counter()
    checksum = 0
    for i in range(calls):
        checksum = (checksum + int(fn(i))) & 0x7FFFFFFFFFFFFFFF
    loop_ms = (time.perf_counter() - t1) * 1000.0
    total_ms = init_ms + loop_ms

    print(
        json.dumps(
            {
                "init_ms": init_ms,
                "loop_ms": loop_ms,
                "total_ms": total_ms,
                "checksum": checksum,
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
""",
    )
    return module_dir, runner_py


def build_rust_direct_runner(work_dir: Path) -> tuple[list[str], float]:
    project_dir = work_dir / "rust-pyo3-direct"
    clear_dir(project_dir)

    write_file(
        project_dir / "Cargo.toml",
        """[package]
name = "rust_pyo3_direct"
version = "0.1.0"
edition = "2021"

[workspace]

[dependencies]
pyo3 = { version = "0.23", features = ["auto-initialize"] }
""",
    )
    write_file(
        project_dir / "src" / "main.rs",
        """use pyo3::prelude::*;
use pyo3::types::PyList;
use std::time::Instant;

fn parse_args() -> Result<(String, usize), String> {
    let mut args = std::env::args();
    let _program = args.next();
    let module_dir = args.next().ok_or("missing module_dir argument")?;
    let calls = args
        .next()
        .ok_or("missing calls argument")?
        .parse::<usize>()
        .map_err(|e| format!("invalid calls: {e}"))?;
    Ok((module_dir, calls))
}

fn main() -> PyResult<()> {
    let (module_dir, calls) = parse_args().map_err(|msg| PyErr::new::<pyo3::exceptions::PyValueError, _>(msg))?;
    Python::with_gil(|py| -> PyResult<()> {
        let t0 = Instant::now();
        let sys = py.import("sys")?;
        let path_any = sys.getattr("path")?;
        let path: Bound<'_, PyList> = path_any.downcast_into()?;
        path.insert(0, module_dir.as_str())?;
        let module = py.import("interop_workload")?;
        let func = module.getattr("hot_fn")?;
        let init_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let t1 = Instant::now();
        let mut checksum: i64 = 0;
        for i in 0..calls {
            let value: i64 = func.call1((i as i64,))?.extract()?;
            checksum = checksum.wrapping_add(value);
        }
        let loop_ms = t1.elapsed().as_secs_f64() * 1000.0;
        let total_ms = init_ms + loop_ms;

        println!(
            "{{\\\"init_ms\\\":{init_ms:.6},\\\"loop_ms\\\":{loop_ms:.6},\\\"total_ms\\\":{total_ms:.6},\\\"checksum\\\":{checksum}}}"
        );
        Ok(())
    })
}
""",
    )

    build_ms = cargo_build_release_ms(project_dir)
    binary = project_dir / "target" / "release" / exe_name("rust_pyo3_direct")
    if not binary.exists():
        raise RuntimeError(f"built runner not found: {binary}")
    return [str(binary)], build_ms


def build_sengoo_runtime_runner(work_dir: Path, sengoo_root: Path) -> tuple[list[str], float]:
    project_dir = work_dir / "sengoo-runtime-python"
    clear_dir(project_dir)

    runtime_path = (sengoo_root / "runtime").resolve().as_posix()
    write_file(
        project_dir / "Cargo.toml",
        f"""[package]
name = "sengoo_runtime_python"
version = "0.1.0"
edition = "2021"

[workspace]

[dependencies]
sengoo-runtime = {{ path = "{runtime_path}", features = ["python"] }}
pyo3 = {{ version = "0.23", features = ["auto-initialize"] }}
""",
    )
    write_file(
        project_dir / "src" / "main.rs",
        """use pyo3::prelude::*;
use pyo3::types::PyList;
use sengoo_runtime::python::PythonInterop;
use std::time::Instant;

fn parse_args() -> Result<(String, usize), String> {
    let mut args = std::env::args();
    let _program = args.next();
    let module_dir = args.next().ok_or("missing module_dir argument")?;
    let calls = args
        .next()
        .ok_or("missing calls argument")?
        .parse::<usize>()
        .map_err(|e| format!("invalid calls: {e}"))?;
    Ok((module_dir, calls))
}

fn main() -> PyResult<()> {
    let (module_dir, calls) = parse_args().map_err(|msg| PyErr::new::<pyo3::exceptions::PyValueError, _>(msg))?;
    Python::with_gil(|py| -> PyResult<()> {
        let interop = PythonInterop::new(py);

        let t0 = Instant::now();
        let sys = interop.import("sys")?;
        let path_any = sys.bind(py).getattr("path")?;
        let path: Bound<'_, PyList> = path_any.downcast_into()?;
        path.insert(0, module_dir.as_str())?;

        let module = interop.import("interop_workload")?;
        let func = module.bind(py).getattr("hot_fn")?.unbind();
        let init_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let t1 = Instant::now();
        let mut checksum: i64 = 0;
        for i in 0..calls {
            let value_obj = interop.call(&func, (i as i64,))?;
            let value: i64 = value_obj.extract(py)?;
            checksum = checksum.wrapping_add(value);
        }
        let loop_ms = t1.elapsed().as_secs_f64() * 1000.0;
        let total_ms = init_ms + loop_ms;

        println!(
            "{{\\\"init_ms\\\":{init_ms:.6},\\\"loop_ms\\\":{loop_ms:.6},\\\"total_ms\\\":{total_ms:.6},\\\"checksum\\\":{checksum}}}"
        );
        Ok(())
    })
}
""",
    )

    build_ms = cargo_build_release_ms(project_dir)
    binary = project_dir / "target" / "release" / exe_name("sengoo_runtime_python")
    if not binary.exists():
        raise RuntimeError(f"built runner not found: {binary}")
    return [str(binary)], build_ms


def discover_python_embed_meta(py_exe: str) -> dict[str, Any]:
    probe = (
        "import json,sys,sysconfig,pathlib;"
        "print(json.dumps({"
        "'platform': sys.platform,"
        "'version_major': sys.version_info.major,"
        "'version_minor': sys.version_info.minor,"
        "'include': sysconfig.get_config_var('INCLUDEPY') or '',"
        "'libdir': sysconfig.get_config_var('LIBDIR') or '',"
        "'library': sysconfig.get_config_var('LIBRARY') or '',"
        "'ldlibrary': sysconfig.get_config_var('LDLIBRARY') or '',"
        "'base_prefix': str(pathlib.Path(sys.base_prefix)),"
        "'executable': sys.executable"
        "}))"
    )
    proc = run_checked([py_exe, "-c", probe])
    meta = json.loads(proc.stdout.strip())
    if not isinstance(meta, dict):
        raise RuntimeError("failed to probe Python embed metadata")
    return meta


def build_cpp_embed_runner(work_dir: Path, py_exe: str) -> tuple[list[str], float, dict[str, str] | None]:
    clangpp = shutil.which("clang++")
    if not clangpp:
        raise RuntimeError("clang++ not found in PATH")

    project_dir = work_dir / "cpp-cpython"
    clear_dir(project_dir)
    src = project_dir / "main.cpp"
    exe = project_dir / exe_name("cpp_cpython_embed")

    write_file(
        src,
        """#include <Python.h>
#include <chrono>
#include <cstdint>
#include <iostream>
#include <string>

using Clock = std::chrono::steady_clock;

int main(int argc, char** argv) {
    if (argc < 3) {
        std::cerr << "usage: <module_dir> <calls>\\n";
        return 2;
    }

    std::string module_dir = argv[1];
    long long calls = 0;
    try {
        calls = std::stoll(argv[2]);
    } catch (...) {
        std::cerr << "invalid calls argument\\n";
        return 2;
    }

    auto t0 = Clock::now();
    Py_Initialize();
    if (!Py_IsInitialized()) {
        std::cerr << "Py_Initialize failed\\n";
        return 3;
    }

    PyObject* path_obj = PySys_GetObject("path");  // borrowed
    if (!path_obj || !PyList_Check(path_obj)) {
        std::cerr << "sys.path unavailable\\n";
        return 3;
    }
    PyObject* dir_obj = PyUnicode_FromString(module_dir.c_str());
    if (!dir_obj) {
        PyErr_Print();
        return 3;
    }
    if (PyList_Insert(path_obj, 0, dir_obj) != 0) {
        Py_DECREF(dir_obj);
        PyErr_Print();
        return 3;
    }
    Py_DECREF(dir_obj);

    PyObject* mod_name = PyUnicode_FromString("interop_workload");
    PyObject* module = PyImport_Import(mod_name);
    Py_DECREF(mod_name);
    if (!module) {
        PyErr_Print();
        return 4;
    }

    PyObject* func = PyObject_GetAttrString(module, "hot_fn");
    if (!func || !PyCallable_Check(func)) {
        PyErr_Print();
        Py_XDECREF(func);
        Py_DECREF(module);
        return 4;
    }
    auto t1 = Clock::now();

    long long checksum = 0;
    for (long long i = 0; i < calls; ++i) {
        PyObject* args = PyTuple_New(1);
        PyTuple_SET_ITEM(args, 0, PyLong_FromLongLong(i));  // steals reference
        PyObject* ret = PyObject_CallObject(func, args);
        Py_DECREF(args);
        if (!ret) {
            PyErr_Print();
            Py_DECREF(func);
            Py_DECREF(module);
            return 5;
        }
        long long value = PyLong_AsLongLong(ret);
        Py_DECREF(ret);
        if (PyErr_Occurred()) {
            PyErr_Print();
            Py_DECREF(func);
            Py_DECREF(module);
            return 5;
        }
        checksum += value;
    }
    auto t2 = Clock::now();

    Py_DECREF(func);
    Py_DECREF(module);

    double init_ms = std::chrono::duration<double, std::milli>(t1 - t0).count();
    double loop_ms = std::chrono::duration<double, std::milli>(t2 - t1).count();
    double total_ms = std::chrono::duration<double, std::milli>(t2 - t0).count();

    std::cout
        << "{\\\"init_ms\\\":" << init_ms
        << ",\\\"loop_ms\\\":" << loop_ms
        << ",\\\"total_ms\\\":" << total_ms
        << ",\\\"checksum\\\":" << checksum
        << "}\\n";

    Py_Finalize();
    return 0;
}
""",
    )

    meta = discover_python_embed_meta(py_exe)
    env_patch: dict[str, str] | None = None

    if sys.platform.startswith("win"):
        include = Path(str(meta.get("include", "")))
        base_prefix = Path(str(meta.get("base_prefix", "")))
        libdir = Path(str(meta.get("libdir", "")))
        version_major = int(meta.get("version_major", 3))
        version_minor = int(meta.get("version_minor", 0))

        candidate_names = [f"python{version_major}{version_minor}.lib"]
        for key in ("library", "ldlibrary"):
            value = str(meta.get(key, "")).strip()
            if value.lower().endswith(".lib"):
                candidate_names.append(value)
        candidates: list[Path] = []
        for name in candidate_names:
            if not name:
                continue
            p = Path(name)
            if p.is_absolute():
                candidates.append(p)
            else:
                candidates.append(libdir / name)
                candidates.append(base_prefix / "libs" / name)
                candidates.append(base_prefix / name)
        lib_file = next((cand for cand in candidates if cand.exists()), None)
        if lib_file is None:
            raise RuntimeError(f"python embed library (.lib) not found; tried: {candidates}")
        if not include.exists():
            raise RuntimeError(f"python include directory not found: {include}")

        build_cmd = [
            clangpp,
            "-std=c++20",
            "-O2",
            str(src),
            f"-I{include}",
            str(lib_file),
            "-o",
            str(exe),
        ]
        build_ms = measure_command_ms(build_cmd, cwd=project_dir)
        dll_dirs = [base_prefix, base_prefix / "DLLs"]
        current_path = os.environ.get("PATH", "")
        merged = ";".join([str(d) for d in dll_dirs if d.exists()] + [current_path])
        env_patch = {"PATH": merged}
        return [str(exe)], build_ms, env_patch

    py_config = shutil.which("python3-config")
    if py_config:
        cflags = run_checked([py_config, "--embed", "--cflags"]).stdout.strip()
        ldflags = run_checked([py_config, "--embed", "--ldflags"]).stdout.strip()
        build_cmd = [clangpp, "-std=c++20", "-O2", str(src), "-o", str(exe)] + shlex.split(cflags) + shlex.split(ldflags)
        build_ms = measure_command_ms(build_cmd, cwd=project_dir)
        return [str(exe)], build_ms, None

    raise RuntimeError("python3-config not found for non-Windows C++ embed build")


def measure_runner(
    cmd: list[str],
    *,
    warmup: int,
    samples: int,
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> dict[str, Any]:
    for _ in range(warmup):
        run_checked(cmd, cwd=cwd, env=env)

    rows: list[dict[str, Any]] = []
    for _ in range(samples):
        started = time.perf_counter()
        proc = run_checked(cmd, cwd=cwd, env=env)
        process_ms = (time.perf_counter() - started) * 1000.0
        payload = parse_json_from_stdout(proc.stdout)
        payload["process_ms"] = process_ms
        rows.append(payload)

    init_samples = [float(row.get("init_ms", 0.0)) for row in rows]
    loop_samples = [float(row.get("loop_ms", 0.0)) for row in rows]
    total_samples = [float(row.get("total_ms", 0.0)) for row in rows]
    process_samples = [float(row.get("process_ms", 0.0)) for row in rows]
    checksums = [int(row.get("checksum", 0)) for row in rows]
    checksum_ok = len(set(checksums)) == 1

    return {
        "samples": rows,
        "init_avg_ms": average(init_samples),
        "init_p50_ms": percentile(init_samples, 0.50),
        "loop_avg_ms": average(loop_samples),
        "loop_p50_ms": percentile(loop_samples, 0.50),
        "total_avg_ms": average(total_samples),
        "process_avg_ms": average(process_samples),
        "checksum": checksums[0] if checksums else None,
        "checksum_consistent": checksum_ok,
    }


def cargo_build_release_ms(project_dir: Path) -> float:
    try:
        return measure_command_ms(["cargo", "build", "--release", "--quiet"], cwd=project_dir)
    except Exception as exc:  # noqa: BLE001
        message = str(exc).lower()
        network_hint = (
            "could not connect",
            "download of config.json failed",
            "failed to download",
            "timeout",
        )
        if any(hint in message for hint in network_hint):
            return measure_command_ms(["cargo", "build", "--release", "--quiet", "--offline"], cwd=project_dir)
        raise


def main() -> int:
    args = parse_args()
    if args.calls <= 0:
        raise RuntimeError("--calls must be > 0")
    if args.samples <= 0:
        raise RuntimeError("--samples must be > 0")
    if args.warmup < 0:
        raise RuntimeError("--warmup must be >= 0")

    bench_root = Path(__file__).resolve().parent
    sengoo_root = resolve_sengoo_root(bench_root)
    results_dir = bench_root / "results"
    results_dir.mkdir(parents=True, exist_ok=True)
    work_dir = bench_root / ".interop-work"
    clear_dir(work_dir)

    py_exe = sys.executable
    module_dir, python_native_runner = prepare_workload(work_dir)

    runners: dict[str, dict[str, Any]] = {}

    runners["python_native"] = {
        "name": "Python native",
        "available": True,
        "build_ms": 0.0,
        "cmd": [py_exe, str(python_native_runner), str(module_dir), str(args.calls)],
        "env": None,
    }

    try:
        cmd, build_ms = build_sengoo_runtime_runner(work_dir, sengoo_root)
        runners["sengoo_runtime_pythoninterop"] = {
            "name": "Sengoo Runtime (PythonInterop)",
            "available": True,
            "build_ms": build_ms,
            "cmd": cmd + [str(module_dir), str(args.calls)],
            "env": None,
        }
    except Exception as exc:  # noqa: BLE001
        runners["sengoo_runtime_pythoninterop"] = {
            "name": "Sengoo Runtime (PythonInterop)",
            "available": False,
            "reason": str(exc),
        }

    try:
        cmd, build_ms = build_rust_direct_runner(work_dir)
        runners["rust_pyo3"] = {
            "name": "Rust (PyO3)",
            "available": True,
            "build_ms": build_ms,
            "cmd": cmd + [str(module_dir), str(args.calls)],
            "env": None,
        }
    except Exception as exc:  # noqa: BLE001
        runners["rust_pyo3"] = {
            "name": "Rust (PyO3)",
            "available": False,
            "reason": str(exc),
        }

    try:
        cmd, build_ms, env_patch = build_cpp_embed_runner(work_dir, py_exe)
        runners["cpp_cpython_api"] = {
            "name": "C++ (CPython C API)",
            "available": True,
            "build_ms": build_ms,
            "cmd": cmd + [str(module_dir), str(args.calls)],
            "env": env_patch,
        }
    except Exception as exc:  # noqa: BLE001
        runners["cpp_cpython_api"] = {
            "name": "C++ (CPython C API)",
            "available": False,
            "reason": str(exc),
        }

    for runner in runners.values():
        if not runner.get("available"):
            continue
        result = measure_runner(
            runner["cmd"],
            warmup=args.warmup,
            samples=args.samples,
            cwd=bench_root,
            env=runner.get("env"),
        )
        runner["result"] = result
        loop_avg = result.get("loop_avg_ms")
        if isinstance(loop_avg, (int, float)) and loop_avg > 0:
            runner["calls_per_sec"] = args.calls / (float(loop_avg) / 1000.0)
        else:
            runner["calls_per_sec"] = None

    python_loop = None
    py_runner = runners.get("python_native")
    if py_runner and py_runner.get("available"):
        py_result = py_runner.get("result", {})
        py_loop = py_result.get("loop_avg_ms")
        if isinstance(py_loop, (int, float)) and py_loop > 0:
            python_loop = float(py_loop)

    summary_rows = []
    for key, runner in runners.items():
        if not runner.get("available"):
            continue
        result = runner.get("result", {})
        loop_avg = result.get("loop_avg_ms")
        rel_vs_py = None
        if python_loop and isinstance(loop_avg, (int, float)):
            rel_vs_py = ((float(loop_avg) - python_loop) / python_loop) * 100.0
        summary_rows.append(
            {
                "id": key,
                "name": runner["name"],
                "build_ms": runner.get("build_ms"),
                "init_avg_ms": result.get("init_avg_ms"),
                "loop_avg_ms": loop_avg,
                "total_avg_ms": result.get("total_avg_ms"),
                "calls_per_sec": runner.get("calls_per_sec"),
                "loop_vs_python_native_pct": rel_vs_py,
                "checksum": result.get("checksum"),
                "checksum_consistent": result.get("checksum_consistent"),
            }
        )
    summary_rows.sort(
        key=lambda row: float(row["loop_avg_ms"]) if isinstance(row.get("loop_avg_ms"), (int, float)) else 1e18
    )

    report: dict[str, Any] = {
        "schema_version": 1,
        "generated_at_unix_ms": now_unix_ms(),
        "config": {
            "calls": args.calls,
            "samples": args.samples,
            "warmup": args.warmup,
        },
        "workload": {
            "python_module": "interop_workload.hot_fn(x): ((x*3+7)^0x5A5A)&0x7FFFFFFF",
            "note": "Runners embed/import Python module and invoke hot_fn in a tight loop.",
        },
        "runners": runners,
        "summary": {
            "ordered_by_loop_avg_ms": summary_rows,
        },
        "notes": [
            "Sengoo runner benchmarks runtime/src/python.rs PythonInterop API path.",
            "Rust runner uses direct PyO3 without Sengoo wrapper.",
            "C++ runner uses CPython C API embedding path.",
            "Python native is a baseline (no cross-language interop boundary).",
        ],
    }

    out_path = results_dir / f"{now_unix_ms()}-python-interop.json"
    out_path.write_text(json.dumps(report, indent=2), encoding="utf-8", newline="\n")

    print(f"Python interop report: {out_path}")
    print("")
    print("| Runner | Build (ms) | Init avg (ms) | Loop avg (ms) | Calls/s | vs Python native (%) |")
    print("|---|---:|---:|---:|---:|---:|")
    for row in summary_rows:
        rel = row["loop_vs_python_native_pct"]
        rel_str = "n/a" if rel is None else f"{float(rel):.2f}"
        calls_per_sec = row["calls_per_sec"]
        cps_str = "n/a" if calls_per_sec is None else f"{float(calls_per_sec):.2f}"
        print(
            "| "
            f"{row['name']} | "
            f"{float(row['build_ms']):.2f} | "
            f"{float(row['init_avg_ms']):.2f} | "
            f"{float(row['loop_avg_ms']):.2f} | "
            f"{cps_str} | "
            f"{rel_str} |"
        )

    unavailable = [runner for runner in runners.values() if not runner.get("available")]
    if unavailable:
        print("")
        print("Skipped runners:")
        for runner in unavailable:
            print(f"- {runner['name']}: {runner.get('reason', 'unknown reason')}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
