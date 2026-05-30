#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any


DEFAULT_REQUESTS = 5000
DEFAULT_MAX_BATCH = 32
DEFAULT_MAX_NEW_PER_STEP = 8
DEFAULT_MAX_LEN = 24
DEFAULT_SAMPLES = 3
DEFAULT_WARMUP = 1
DEFAULT_SEED = 42
CI_SMOKE_PRESET = {
    "requests": 64,
    "max_batch": 8,
    "max_new_per_step": 4,
    "max_len": 6,
    "samples": 1,
    "warmup": 0,
    "light_kernel_iters": 0,
    "heavy_kernel_iters": 8,
}


def now_unix_ms() -> int:
    return int(time.time() * 1000)


def average(values: list[float]) -> float | None:
    if not values:
        return None
    return sum(values) / len(values)


def percentile(values: list[float], p: float) -> float | None:
    if not values:
        return None
    ordered = sorted(values)
    idx = int(math.ceil((len(ordered) - 1) * p))
    return ordered[idx]


def exe_name(stem: str) -> str:
    return f"{stem}.exe" if sys.platform.startswith("win") else stem


def parse_args(argv: list[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "LLM scheduler benchmark: compare Python scheduler vs Sengoo Runtime "
            "scheduler on the same decode-step kernel."
        )
    )
    parser.add_argument("--preset", choices=["ci-smoke"])
    parser.add_argument("--smoke", action="store_true", help="alias for --preset ci-smoke")
    parser.add_argument("--requests", type=int, default=DEFAULT_REQUESTS)
    parser.add_argument("--max-batch", type=int, default=DEFAULT_MAX_BATCH)
    parser.add_argument("--max-new-per-step", type=int, default=DEFAULT_MAX_NEW_PER_STEP)
    parser.add_argument("--max-len", type=int, default=DEFAULT_MAX_LEN)
    parser.add_argument("--samples", type=int, default=DEFAULT_SAMPLES)
    parser.add_argument("--warmup", type=int, default=DEFAULT_WARMUP)
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--light-kernel-iters", type=int, default=1)
    parser.add_argument("--heavy-kernel-iters", type=int, default=32)
    parser.add_argument("--skip-build", action="store_true")
    parser.add_argument("--out", help="optional output path")
    return parser.parse_args(argv)


def apply_preset(args: argparse.Namespace) -> argparse.Namespace:
    if getattr(args, "smoke", False):
        args.preset = "ci-smoke"

    preset = getattr(args, "preset", None)
    if preset is None:
        return args
    if preset != "ci-smoke":
        raise RuntimeError(f"unsupported preset: {preset}")

    for key, value in CI_SMOKE_PRESET.items():
        setattr(args, key, value)
    return args


def run_checked(
    cmd: list[str],
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> subprocess.CompletedProcess[str]:
    merged_env = None if env is None else {**os.environ, **env}
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


def measure_command_ms(cmd: list[str], cwd: Path | None = None) -> tuple[float, subprocess.CompletedProcess[str]]:
    started = time.perf_counter()
    proc = run_checked(cmd, cwd=cwd)
    return (time.perf_counter() - started) * 1000.0, proc


def parse_json_from_stdout(stdout: str) -> dict[str, Any]:
    for line in reversed(stdout.splitlines()):
        candidate = line.strip()
        if not candidate:
            continue
        try:
            parsed = json.loads(candidate)
        except json.JSONDecodeError:
            continue
        if isinstance(parsed, dict):
            return parsed
    raise RuntimeError(f"runner output does not contain JSON object:\n{stdout}")


def resolve_sengoo_root(bench_root: Path) -> Path:
    env_root = os.environ.get("SENGOO_ROOT")
    if env_root:
        root = Path(env_root).expanduser().resolve()
        if (root / "Cargo.toml").exists() and (root / "runtime" / "Cargo.toml").exists():
            return root
        raise RuntimeError(f"SENGOO_ROOT does not look like Sengoo root: {root}")

    candidate = bench_root.parent
    if (candidate / "Cargo.toml").exists() and (candidate / "runtime" / "Cargo.toml").exists():
        return candidate
    raise RuntimeError("cannot resolve Sengoo source root; set SENGOO_ROOT")


def clear_dir(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True, exist_ok=True)


def make_temp_work_dir(base_dir: Path, prefix: str = "llm-scheduler-work-") -> Path:
    base_dir.mkdir(parents=True, exist_ok=True)
    return Path(tempfile.mkdtemp(prefix=prefix, dir=base_dir))


def write_file(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(content, encoding="utf-8", newline="\n")


def cargo_build_release_ms(project_dir: Path) -> float:
    started = time.perf_counter()
    build_env = {"PYO3_USE_ABI3_FORWARD_COMPATIBILITY": "1"}
    try:
        run_checked(["cargo", "build", "--release", "--quiet"], cwd=project_dir, env=build_env)
    except RuntimeError as exc:
        msg = str(exc).lower()
        hints = ("failed to download", "could not connect", "timeout", "download of config.json failed")
        if any(token in msg for token in hints):
            run_checked(
                ["cargo", "build", "--release", "--quiet", "--offline"],
                cwd=project_dir,
                env=build_env,
            )
        else:
            raise
    return (time.perf_counter() - started) * 1000.0


def prepare_workload(work_dir: Path) -> tuple[Path, Path]:
    module_dir = work_dir / "workload"
    module_dir.mkdir(parents=True, exist_ok=True)

    write_file(
        module_dir / "decode_kernel.py",
        """#!/usr/bin/env python3
from __future__ import annotations

try:
    import numpy as np  # type: ignore
except ModuleNotFoundError:  # pragma: no cover - fallback for minimal environments
    np = None


def _decode_step_python(batch_ids: list[int], step: int, kernel_iters: int) -> list[int]:
    mask = 0x7FFFFFFF
    x = [((req_id ^ (step * 0x9E3779B1)) & mask) for req_id in batch_ids]
    for i in range(kernel_iters):
        x = [((value * 1103515245 + 12345 + i * 17) ^ (value >> 7)) & mask for value in x]
    return x


def decode_step(batch_ids: list[int], step: int, kernel_iters: int) -> list[int]:
    if np is None:
        return _decode_step_python(batch_ids, step, kernel_iters)

    arr = np.asarray(batch_ids, dtype=np.int64)
    x = (arr ^ (step * 0x9E3779B1)) & 0x7FFFFFFF
    for i in range(kernel_iters):
        x = ((x * 1103515245 + 12345 + i * 17) ^ (x >> 7)) & 0x7FFFFFFF
    return x.tolist()
""",
    )

    py_runner = work_dir / "python_scheduler_runner.py"
    write_file(
        py_runner,
        """#!/usr/bin/env python3
from __future__ import annotations

import importlib
import json
import sys
import time
from pathlib import Path


def wrap_i64(value: int) -> int:
    value &= 0xFFFFFFFFFFFFFFFF
    if value >= 0x8000000000000000:
        value -= 0x10000000000000000
    return value


def lcg_next(state: int) -> int:
    return (state * 1664525 + 1013904223) & 0xFFFFFFFF


def build_arrivals(total_requests: int, max_len: int, seed: int) -> list[tuple[int, int, int]]:
    arr: list[tuple[int, int, int]] = []
    state = seed & 0xFFFFFFFF
    spread = max(1, total_requests // 3)
    for req_id in range(total_requests):
        state = lcg_next(state)
        arrival = state % spread
        state = lcg_next(state)
        remaining = 1 + (state % max_len)
        arr.append((arrival, req_id, remaining))
    arr.sort(key=lambda t: (t[0], t[1]))
    return arr


def run_scheduler(
    decode_step,
    total_requests: int,
    max_batch: int,
    max_new_per_step: int,
    max_len: int,
    seed: int,
    kernel_iters: int,
) -> tuple[int, int, int]:
    arrivals = build_arrivals(total_requests, max_len, seed)
    idx = 0
    active: list[list[int]] = []
    finished = 0
    checksum = 0
    total_tokens = 0
    step = 0

    while finished < total_requests:
        admitted = 0
        while idx < len(arrivals) and arrivals[idx][0] <= step and admitted < max_new_per_step:
            _, req_id, remaining = arrivals[idx]
            active.append([req_id, remaining])
            idx += 1
            admitted += 1

        if not active:
            step += 1
            continue

        batch = active[:max_batch]
        batch_ids = [item[0] for item in batch]
        tokens = decode_step(batch_ids, step, kernel_iters)
        if len(tokens) != len(batch):
            raise RuntimeError("decode_step token count mismatch")

        next_active: list[list[int]] = []
        consumed = min(max_batch, len(active))
        for i in range(consumed):
            req_id, remaining = active[i]
            token = int(tokens[i])
            checksum = wrap_i64(checksum + token + req_id + step)
            remaining -= 1
            total_tokens += 1
            if remaining <= 0:
                finished += 1
            else:
                next_active.append([req_id, remaining])

        if consumed < len(active):
            next_active.extend(active[consumed:])
        active = next_active
        step += 1

    return checksum, total_tokens, step


def main() -> int:
    module_dir = Path(sys.argv[1]).resolve()
    total_requests = int(sys.argv[2])
    max_batch = int(sys.argv[3])
    max_new_per_step = int(sys.argv[4])
    max_len = int(sys.argv[5])
    seed = int(sys.argv[6])
    kernel_iters = int(sys.argv[7])

    sys.path.insert(0, str(module_dir))

    t0 = time.perf_counter()
    module = importlib.import_module("decode_kernel")
    decode_step = module.decode_step
    init_ms = (time.perf_counter() - t0) * 1000.0

    t1 = time.perf_counter()
    checksum, total_tokens, steps = run_scheduler(
        decode_step,
        total_requests,
        max_batch,
        max_new_per_step,
        max_len,
        seed,
        kernel_iters,
    )
    loop_ms = (time.perf_counter() - t1) * 1000.0
    total_ms = init_ms + loop_ms
    tokens_per_sec = (total_tokens * 1000.0 / loop_ms) if loop_ms > 0 else 0.0

    print(
        json.dumps(
            {
                "init_ms": init_ms,
                "loop_ms": loop_ms,
                "total_ms": total_ms,
                "tokens_per_sec": tokens_per_sec,
                "total_tokens": total_tokens,
                "steps": steps,
                "checksum": checksum,
            }
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
""",
    )

    return module_dir, py_runner


def prepare_workload_with_fallback(preferred_dir: Path, temp_root: Path) -> tuple[Path, Path, Path]:
    try:
        preferred_dir.mkdir(parents=True, exist_ok=True)
        module_dir, py_runner = prepare_workload(preferred_dir)
        return module_dir, py_runner, preferred_dir
    except OSError:
        fallback_dir = make_temp_work_dir(temp_root)
        module_dir, py_runner = prepare_workload(fallback_dir)
        return module_dir, py_runner, fallback_dir


def resolve_skip_build_binary(preferred_dir: Path) -> Path:
    binary = preferred_dir / "sengoo-scheduler-runner" / "target" / "release" / exe_name("sengoo_scheduler_runner")
    if not binary.exists():
        raise RuntimeError("--skip-build used but Sengoo runner is not built")
    return binary


def build_sengoo_runner(work_dir: Path, sengoo_root: Path) -> tuple[list[str], float]:
    project_dir = work_dir / "sengoo-scheduler-runner"
    clear_dir(project_dir)

    runtime_path = (sengoo_root / "runtime").resolve().as_posix()
    write_file(
        project_dir / "Cargo.toml",
        f"""[package]
name = "sengoo_scheduler_runner"
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

#[derive(Clone, Copy)]
struct ActiveRequest {
    req_id: i64,
    remaining: i64,
}

fn lcg_next(state: u32) -> u32 {
    state.wrapping_mul(1664525).wrapping_add(1013904223)
}

fn parse_args() -> Result<(String, usize, usize, usize, usize, u32, usize), String> {
    let mut args = std::env::args();
    let _program = args.next();
    let module_dir = args.next().ok_or("missing module_dir argument")?;
    let total_requests = args
        .next()
        .ok_or("missing total_requests")?
        .parse::<usize>()
        .map_err(|e| format!("invalid total_requests: {e}"))?;
    let max_batch = args
        .next()
        .ok_or("missing max_batch")?
        .parse::<usize>()
        .map_err(|e| format!("invalid max_batch: {e}"))?;
    let max_new_per_step = args
        .next()
        .ok_or("missing max_new_per_step")?
        .parse::<usize>()
        .map_err(|e| format!("invalid max_new_per_step: {e}"))?;
    let max_len = args
        .next()
        .ok_or("missing max_len")?
        .parse::<usize>()
        .map_err(|e| format!("invalid max_len: {e}"))?;
    let seed = args
        .next()
        .ok_or("missing seed")?
        .parse::<u32>()
        .map_err(|e| format!("invalid seed: {e}"))?;
    let kernel_iters = args
        .next()
        .ok_or("missing kernel_iters")?
        .parse::<usize>()
        .map_err(|e| format!("invalid kernel_iters: {e}"))?;
    Ok((
        module_dir,
        total_requests,
        max_batch,
        max_new_per_step,
        max_len,
        seed,
        kernel_iters,
    ))
}

fn build_arrivals(total_requests: usize, max_len: usize, seed: u32) -> Vec<(usize, i64, i64)> {
    let mut out = Vec::with_capacity(total_requests);
    let mut state = seed;
    let spread = std::cmp::max(1, total_requests / 3);
    for req_id in 0..total_requests {
        state = lcg_next(state);
        let arrival = (state as usize) % spread;
        state = lcg_next(state);
        let remaining = 1 + ((state as usize) % max_len);
        out.push((arrival, req_id as i64, remaining as i64));
    }
    out.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));
    out
}

fn main() -> Result<(), String> {
    let (
        module_dir,
        total_requests,
        max_batch,
        max_new_per_step,
        max_len,
        seed,
        kernel_iters,
    ) = parse_args()?;

    Python::with_gil(|py| -> Result<(), String> {
        let interop = PythonInterop::new(py);

        let t0 = Instant::now();
        let sys = interop.import("sys").map_err(|e| e.to_string())?;
        let path_any = sys
            .bind(py)
            .getattr("path")
            .map_err(|e| e.to_string())?;
        let path: Bound<'_, PyList> = path_any
            .downcast_into()
            .map_err(|e| e.to_string())?;
        path.insert(0, module_dir.as_str())
            .map_err(|e| e.to_string())?;
        let module = interop.import("decode_kernel").map_err(|e| e.to_string())?;
        let decode_step = module
            .bind(py)
            .getattr("decode_step")
            .map_err(|e| e.to_string())?
            .unbind();
        let init_ms = t0.elapsed().as_secs_f64() * 1000.0;

        let arrivals = build_arrivals(total_requests, max_len, seed);
        let mut arrival_idx: usize = 0;
        let mut active: Vec<ActiveRequest> = Vec::new();
        active.reserve(max_batch.saturating_mul(4));
        let mut finished: usize = 0;
        let mut checksum: i64 = 0;
        let mut total_tokens: usize = 0;
        let mut step: usize = 0;

        let t1 = Instant::now();
        while finished < total_requests {
            let mut admitted = 0usize;
            while arrival_idx < arrivals.len()
                && arrivals[arrival_idx].0 <= step
                && admitted < max_new_per_step
            {
                let (_, req_id, remaining) = arrivals[arrival_idx];
                active.push(ActiveRequest { req_id, remaining });
                arrival_idx += 1;
                admitted += 1;
            }

            if active.is_empty() {
                step += 1;
                continue;
            }

            let consumed = std::cmp::min(max_batch, active.len());
            let mut batch_ids: Vec<i64> = Vec::with_capacity(consumed);
            for i in 0..consumed {
                batch_ids.push(active[i].req_id);
            }

            let value_obj = interop
                .call(
                    &decode_step,
                    (batch_ids.clone(), step as i64, kernel_iters as i64),
                )
                .map_err(|e| e.to_string())?;
            let tokens: Vec<i64> = value_obj.extract(py).map_err(|e| e.to_string())?;
            if tokens.len() != consumed {
                return Err("decode_step token count mismatch".to_string());
            }

            let mut next_active: Vec<ActiveRequest> = Vec::with_capacity(active.len());
            for i in 0..consumed {
                let req = active[i];
                checksum = checksum
                    .wrapping_add(tokens[i])
                    .wrapping_add(req.req_id)
                    .wrapping_add(step as i64);
                let next_remaining = req.remaining - 1;
                total_tokens += 1;
                if next_remaining <= 0 {
                    finished += 1;
                } else {
                    next_active.push(ActiveRequest {
                        req_id: req.req_id,
                        remaining: next_remaining,
                    });
                }
            }
            if consumed < active.len() {
                next_active.extend_from_slice(&active[consumed..]);
            }
            active = next_active;
            step += 1;
        }

        let loop_ms = t1.elapsed().as_secs_f64() * 1000.0;
        let total_ms = init_ms + loop_ms;
        let tokens_per_sec = if loop_ms > 0.0 {
            (total_tokens as f64 * 1000.0) / loop_ms
        } else {
            0.0
        };

        println!(
            "{{\\\"init_ms\\\":{init_ms:.6},\\\"loop_ms\\\":{loop_ms:.6},\\\"total_ms\\\":{total_ms:.6},\\\"tokens_per_sec\\\":{tokens_per_sec:.6},\\\"total_tokens\\\":{total_tokens},\\\"steps\\\":{step},\\\"checksum\\\":{checksum}}}"
        );
        Ok(())
    })
}
""",
    )

    build_ms = cargo_build_release_ms(project_dir)
    binary = project_dir / "target" / "release" / exe_name("sengoo_scheduler_runner")
    if not binary.exists():
        raise RuntimeError(f"built runner not found: {binary}")
    return [str(binary)], build_ms


def measure_runner(
    cmd: list[str],
    *,
    warmup: int,
    samples: int,
    cwd: Path,
) -> dict[str, Any]:
    for _ in range(warmup):
        run_checked(cmd, cwd=cwd)

    process_samples: list[float] = []
    init_samples: list[float] = []
    loop_samples: list[float] = []
    total_samples: list[float] = []
    tps_samples: list[float] = []
    checksums: list[int] = []
    tokens: list[int] = []
    steps: list[int] = []

    for _ in range(samples):
        wall_ms, proc = measure_command_ms(cmd, cwd=cwd)
        payload = parse_json_from_stdout(proc.stdout)
        process_samples.append(wall_ms)
        init_samples.append(float(payload.get("init_ms", 0.0)))
        loop_samples.append(float(payload.get("loop_ms", 0.0)))
        total_samples.append(float(payload.get("total_ms", 0.0)))
        tps_samples.append(float(payload.get("tokens_per_sec", 0.0)))
        checksums.append(int(payload.get("checksum", 0)))
        tokens.append(int(payload.get("total_tokens", 0)))
        steps.append(int(payload.get("steps", 0)))

    return {
        "process_wall_samples_ms": process_samples,
        "init_samples_ms": init_samples,
        "loop_samples_ms": loop_samples,
        "total_samples_ms": total_samples,
        "tokens_per_sec_samples": tps_samples,
        "process_wall_avg_ms": average(process_samples),
        "init_avg_ms": average(init_samples),
        "loop_avg_ms": average(loop_samples),
        "loop_p50_ms": percentile(loop_samples, 0.50),
        "total_avg_ms": average(total_samples),
        "tokens_per_sec_avg": average(tps_samples),
        "tokens_per_sec_p50": percentile(tps_samples, 0.50),
        "checksum": checksums[0] if checksums else None,
        "checksum_consistent": len(set(checksums)) <= 1,
        "total_tokens": tokens[0] if tokens else 0,
        "steps": steps[0] if steps else 0,
    }


def run_scenario(
    *,
    scenario_id: str,
    kernel_iters: int,
    bench_root: Path,
    module_dir: Path,
    python_runner: Path,
    sengoo_cmd_base: list[str],
    args: argparse.Namespace,
) -> dict[str, Any]:
    shared = [
        str(module_dir),
        str(args.requests),
        str(args.max_batch),
        str(args.max_new_per_step),
        str(args.max_len),
        str(args.seed),
        str(kernel_iters),
    ]
    py_cmd = [sys.executable, str(python_runner)] + shared
    sg_cmd = sengoo_cmd_base + shared

    py = measure_runner(py_cmd, warmup=args.warmup, samples=args.samples, cwd=bench_root)
    sg = measure_runner(sg_cmd, warmup=args.warmup, samples=args.samples, cwd=bench_root)

    if not bool(py.get("checksum_consistent")):
        raise RuntimeError(f"{scenario_id}: python runner checksum inconsistent")
    if not bool(sg.get("checksum_consistent")):
        raise RuntimeError(f"{scenario_id}: sengoo runner checksum inconsistent")
    if py.get("checksum") != sg.get("checksum"):
        raise RuntimeError(
            f"{scenario_id}: checksum mismatch python={py.get('checksum')} sengoo={sg.get('checksum')}"
        )

    py_loop = float(py.get("loop_avg_ms", 0.0) or 0.0)
    sg_loop = float(sg.get("loop_avg_ms", 0.0) or 0.0)
    speed_ratio = (py_loop / sg_loop) if sg_loop > 0 else None
    delta_pct = ((sg_loop - py_loop) / py_loop * 100.0) if py_loop > 0 else None

    return {
        "id": scenario_id,
        "kernel_iters": kernel_iters,
        "workload": {
            "requests": args.requests,
            "max_batch": args.max_batch,
            "max_new_per_step": args.max_new_per_step,
            "max_len": args.max_len,
            "seed": args.seed,
        },
        "python": py,
        "sengoo": sg,
        "comparison": {
            "python_vs_sengoo_loop_speed_ratio_x": speed_ratio,
            "sengoo_loop_delta_pct_vs_python": delta_pct,
            "tokens_per_sec_gain_pct": (
                ((float(sg.get("tokens_per_sec_avg", 0.0) or 0.0) - float(py.get("tokens_per_sec_avg", 0.0) or 0.0))
                 / float(py.get("tokens_per_sec_avg", 1.0) or 1.0)
                 * 100.0)
                if float(py.get("tokens_per_sec_avg", 0.0) or 0.0) > 0
                else None
            ),
        },
    }


def print_table(report: dict[str, Any]) -> None:
    scenarios = report.get("scenarios", [])
    if not isinstance(scenarios, list):
        return
    print("LLM Scheduler Benchmark")
    print("| Scenario | kernel iters | Python loop avg(ms) | Sengoo loop avg(ms) | Py/Sengoo | tps gain |")
    print("|---|---:|---:|---:|---:|---:|")
    for row in scenarios:
        if not isinstance(row, dict):
            continue
        py = row.get("python", {})
        sg = row.get("sengoo", {})
        cmp = row.get("comparison", {})
        py_loop = float(py.get("loop_avg_ms", 0.0) or 0.0)
        sg_loop = float(sg.get("loop_avg_ms", 0.0) or 0.0)
        ratio = cmp.get("python_vs_sengoo_loop_speed_ratio_x")
        gain = cmp.get("tokens_per_sec_gain_pct")
        ratio_str = f"{float(ratio):.2f}x" if ratio is not None else "-"
        gain_str = f"{float(gain):+.2f}%" if gain is not None else "-"
        print(
            f"| {row.get('id', 'unknown')} | {int(row.get('kernel_iters', 0))} | {py_loop:.2f} | "
            f"{sg_loop:.2f} | {ratio_str} | {gain_str} |"
        )


def build_report(args: argparse.Namespace, scenarios: list[dict[str, Any]], build_ms: float | None) -> dict[str, Any]:
    return {
        "schema_version": 1,
        "scenario": "llm-scheduler-bench",
        "generated_at_unix_ms": now_unix_ms(),
        "inputs": {
            "preset": getattr(args, "preset", None),
            "requests": args.requests,
            "max_batch": args.max_batch,
            "max_new_per_step": args.max_new_per_step,
            "max_len": args.max_len,
            "seed": args.seed,
            "samples": args.samples,
            "warmup": args.warmup,
            "light_kernel_iters": args.light_kernel_iters,
            "heavy_kernel_iters": args.heavy_kernel_iters,
            "sengoo_runner_built_this_run": build_ms is not None,
        },
        "sengoo_runner_build_ms": build_ms,
        "scenarios": scenarios,
    }


def run_benchmark(args: argparse.Namespace, bench_root: Path | None = None) -> tuple[dict[str, Any], Path]:
    args = apply_preset(args)
    if args.requests <= 0:
        raise RuntimeError("--requests must be > 0")
    if args.max_batch <= 0:
        raise RuntimeError("--max-batch must be > 0")
    if args.max_new_per_step <= 0:
        raise RuntimeError("--max-new-per-step must be > 0")
    if args.max_len <= 0:
        raise RuntimeError("--max-len must be > 0")
    if args.samples <= 0:
        raise RuntimeError("--samples must be > 0")
    if args.warmup < 0:
        raise RuntimeError("--warmup must be >= 0")
    if args.light_kernel_iters < 0 or args.heavy_kernel_iters < 0:
        raise RuntimeError("kernel iters must be >= 0")

    bench_root = bench_root or Path(__file__).resolve().parent
    sengoo_root = resolve_sengoo_root(bench_root)
    results_dir = bench_root / "results"
    results_dir.mkdir(parents=True, exist_ok=True)
    temp_root = results_dir / ".tmp"

    preferred_work_dir = bench_root / ".llm-scheduler-work"
    build_ms: float | None
    if args.skip_build:
        module_dir, python_runner, _ = prepare_workload_with_fallback(preferred_work_dir, temp_root)
        binary = resolve_skip_build_binary(preferred_work_dir)
        sengoo_cmd_base = [str(binary)]
        build_ms = None
    else:
        try:
            clear_dir(preferred_work_dir)
            module_dir, python_runner = prepare_workload(preferred_work_dir)
            sengoo_cmd_base, build_ms = build_sengoo_runner(preferred_work_dir, sengoo_root)
        except OSError:
            fallback_work_dir = make_temp_work_dir(temp_root)
            module_dir, python_runner = prepare_workload(fallback_work_dir)
            sengoo_cmd_base, build_ms = build_sengoo_runner(fallback_work_dir, sengoo_root)

    scenarios = [
        run_scenario(
            scenario_id="prefill_decode_orchestration_light_kernel",
            kernel_iters=args.light_kernel_iters,
            bench_root=bench_root,
            module_dir=module_dir,
            python_runner=python_runner,
            sengoo_cmd_base=sengoo_cmd_base,
            args=args,
        ),
        run_scenario(
            scenario_id="prefill_decode_orchestration_heavy_kernel",
            kernel_iters=args.heavy_kernel_iters,
            bench_root=bench_root,
            module_dir=module_dir,
            python_runner=python_runner,
            sengoo_cmd_base=sengoo_cmd_base,
            args=args,
        ),
    ]

    report = build_report(args, scenarios, build_ms)

    if args.out:
        out_path = Path(args.out).expanduser().resolve()
        out_path.parent.mkdir(parents=True, exist_ok=True)
    else:
        out_path = results_dir / f"{now_unix_ms()}-llm-scheduler-bench.json"
    out_path.write_text(json.dumps(report, indent=2), encoding="utf-8", newline="\n")
    return report, out_path


def main(argv: list[str] | None = None) -> int:
    report, out_path = run_benchmark(parse_args(argv))

    print_table(report)
    build_ms = report.get("sengoo_runner_build_ms")
    if isinstance(build_ms, (int, float)):
        print(f"Sengoo scheduler runner build time: {build_ms:.2f} ms")
    print(f"LLM scheduler report: {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())




