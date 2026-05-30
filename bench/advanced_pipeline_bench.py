#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import math
import os
import socket
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


INCREMENTAL_ITERS = 3
SCALE_LOC_BUCKETS = [1000, 10000, 100000, 1000000]
SCALE_ITERS_BY_LOC = {
    1000: 3,
    10000: 2,
    100000: 3,
    1000000: 1,
}
DAEMON_REAL_INCREMENTAL_WARMUP_ITERS = 1
REACHABILITY_LOC = 100000
REACHABILITY_ITERS = 2
REACHABILITY_PROFILES = [
    "all_reachable",
    "half_reachable",
    "library_entryless",
]


INCREMENTAL_SCENARIOS = [
    "loop_body_change",
    "function_signature_change",
    "add_new_function",
]

TARGET_REAL_INCREMENTAL_MS = 200.0
TARGET_FULL_BUILD_100K_MS = 2000.0
TARGET_FRONTEND_100K_MS = 300.0
TARGET_CODEGEN_100K_MS = 1500.0
TARGET_LINK_100K_MS = 500.0
DEFAULT_DAEMON_ADDR = "127.0.0.1:48768"
MEMORY_LOC_BUCKETS = [10000, 100000, 1000000]
MEMORY_ITERS_BY_LOC = {
    10000: 3,
    100000: 2,
    1000000: 1,
}
MEMORY_SAMPLE_INTERVAL_S = 0.01
BYTES_PER_MB = 1024.0 * 1024.0
FRONTEND_BASELINE_PROFILE = "frontend-memory-baseline.json"
ROLLBACK_MAX_FRONTEND_100K_REGRESSION_PCT = 12.0
ROLLBACK_MAX_FRONTEND_1000K_REGRESSION_PCT = 12.0
ROLLBACK_MAX_RSS_100K_REGRESSION_PCT = 12.0
ROLLBACK_MAX_RSS_1000K_REGRESSION_PCT = 12.0


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


def bytes_to_mb(value: int | float | None) -> float | None:
    if value is None:
        return None
    return float(value) / BYTES_PER_MB


def average_bytes_as_mb(values: list[int]) -> float | None:
    if not values:
        return None
    return bytes_to_mb(int(sum(values) / len(values)))


def percentile_bytes_as_mb(values: list[int], p: float) -> float | None:
    if not values:
        return None
    pct = percentile([float(v) for v in values], p)
    return bytes_to_mb(pct)


def resolve_rustc_binary() -> str:
    override = os.environ.get("RUSTC_REAL")
    if override:
        candidate = Path(override).expanduser().resolve()
        if candidate.exists():
            return str(candidate)

    rustup = shutil.which("rustup")
    if rustup:
        proc = subprocess.run(
            [rustup, "which", "rustc"],
            text=True,
            capture_output=True,
            encoding="utf-8",
            errors="replace",
        )
        if proc.returncode == 0:
            candidate = Path(proc.stdout.strip())
            if candidate.exists():
                return str(candidate)

    return require_tool("rustc")


def read_process_memory_bytes(pid: int) -> tuple[int | None, int | None]:
    if sys.platform.startswith("win"):
        try:
            import ctypes
            from ctypes import wintypes
        except Exception:
            return None, None

        class PROCESS_MEMORY_COUNTERS_EX(ctypes.Structure):
            _fields_ = [
                ("cb", wintypes.DWORD),
                ("PageFaultCount", wintypes.DWORD),
                ("PeakWorkingSetSize", ctypes.c_size_t),
                ("WorkingSetSize", ctypes.c_size_t),
                ("QuotaPeakPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPagedPoolUsage", ctypes.c_size_t),
                ("QuotaPeakNonPagedPoolUsage", ctypes.c_size_t),
                ("QuotaNonPagedPoolUsage", ctypes.c_size_t),
                ("PagefileUsage", ctypes.c_size_t),
                ("PeakPagefileUsage", ctypes.c_size_t),
                ("PrivateUsage", ctypes.c_size_t),
            ]

        PROCESS_QUERY_LIMITED_INFORMATION = 0x1000
        PROCESS_VM_READ = 0x0010
        kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
        psapi = ctypes.WinDLL("psapi", use_last_error=True)
        handle = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_VM_READ, False, pid)
        if not handle:
            return None, None
        try:
            counters = PROCESS_MEMORY_COUNTERS_EX()
            counters.cb = ctypes.sizeof(PROCESS_MEMORY_COUNTERS_EX)
            ok = psapi.GetProcessMemoryInfo(handle, ctypes.byref(counters), counters.cb)
            if not ok:
                return None, None
            return int(counters.WorkingSetSize), int(counters.PrivateUsage)
        finally:
            kernel32.CloseHandle(handle)

    status_path = Path("/proc") / str(pid) / "status"
    if status_path.exists():
        rss_bytes: int | None = None
        private_bytes: int | None = None
        try:
            text = status_path.read_text(encoding="utf-8", errors="replace")
            for raw_line in text.splitlines():
                line = raw_line.strip()
                if line.startswith("VmRSS:"):
                    parts = line.split()
                    if len(parts) >= 2 and parts[1].isdigit():
                        rss_bytes = int(parts[1]) * 1024
                elif line.startswith("RssAnon:"):
                    parts = line.split()
                    if len(parts) >= 2 and parts[1].isdigit():
                        private_bytes = int(parts[1]) * 1024
            return rss_bytes, private_bytes
        except OSError:
            return None, None

    ps = shutil.which("ps")
    if not ps:
        return None, None
    proc = subprocess.run(
        [ps, "-o", "rss=", "-p", str(pid)],
        text=True,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
    )
    if proc.returncode != 0:
        return None, None
    try:
        rss_kb = int(proc.stdout.strip())
    except ValueError:
        return None, None
    return rss_kb * 1024, None


def measure_command_peak_memory(
    cmd: list[str],
    cwd: Path | None = None,
    env: dict[str, str] | None = None,
) -> dict[str, float | None]:
    merged_env = os.environ.copy()
    if env:
        merged_env.update(env)

    started = time.perf_counter()
    proc = subprocess.Popen(
        cmd,
        cwd=str(cwd) if cwd else None,
        env=merged_env,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        encoding="utf-8",
        errors="replace",
    )
    peak_rss = 0
    peak_private = 0
    saw_private = False

    while True:
        rss_bytes, private_bytes = read_process_memory_bytes(proc.pid)
        if isinstance(rss_bytes, int) and rss_bytes > peak_rss:
            peak_rss = rss_bytes
        if isinstance(private_bytes, int):
            saw_private = True
            if private_bytes > peak_private:
                peak_private = private_bytes

        if proc.poll() is not None:
            break
        time.sleep(MEMORY_SAMPLE_INTERVAL_S)

    stdout, stderr = proc.communicate()
    elapsed_ms = (time.perf_counter() - started) * 1000.0
    if proc.returncode != 0:
        raise RuntimeError(
            f"command failed ({proc.returncode}): {' '.join(cmd)}\nstdout:\n{stdout}\nstderr:\n{stderr}"
        )

    return {
        "elapsed_ms": elapsed_ms,
        "peak_rss_bytes": float(peak_rss) if peak_rss > 0 else None,
        "peak_private_bytes": float(peak_private) if saw_private else None,
    }


def write_file(path: Path, content: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    if path.exists():
        existing = path.read_text(encoding="utf-8")
        if existing == content:
            return
    path.write_text(content, encoding="utf-8", newline="\n")


def clear_dir(path: Path) -> None:
    if path.exists():
        shutil.rmtree(path)
    path.mkdir(parents=True, exist_ok=True)


def clear_pycache(path: Path) -> None:
    for pycache in path.rglob("__pycache__"):
        if pycache.is_dir():
            shutil.rmtree(pycache)


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
    candidates = [
        sengoo_root / "target" / "release" / exe_name("sgc"),
        sengoo_root / "target" / "debug" / exe_name("sgc"),
    ]
    existing = [candidate for candidate in candidates if candidate.exists()]
    if not existing:
        raise RuntimeError("sgc binary not found; run `cargo build -p sgc --release` first")

    for candidate in existing:
        if supports_daemon_subcommand(candidate):
            return candidate
    return existing[0]


def supports_daemon_subcommand(binary: Path) -> bool:
    proc = subprocess.run(
        [str(binary), "--help"],
        text=True,
        capture_output=True,
        encoding="utf-8",
        errors="replace",
    )
    if proc.returncode != 0:
        return False
    return "daemon" in proc.stdout.lower()


_LINKER_FLAGS_CACHE: list[str] | None = None


def preferred_linker_flags() -> list[str]:
    global _LINKER_FLAGS_CACHE
    if _LINKER_FLAGS_CACHE is None:
        has_lld = shutil.which("lld-link") is not None or shutil.which("ld.lld") is not None
        _LINKER_FLAGS_CACHE = ["-fuse-ld=lld"] if has_lld else []
    return list(_LINKER_FLAGS_CACHE)


def clang_link_cmd(clangpp: str, objects: list[Path | str], output: Path | str) -> list[str]:
    cmd = [clangpp]
    cmd.extend(preferred_linker_flags())
    cmd.extend(str(obj) for obj in objects)
    cmd.extend(["-o", str(output)])
    return cmd


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Run advanced benchmark pipeline "
            "(real incremental + scale curve + compile-memory compare + optional daemon comparison)."
        )
    )
    parser.add_argument(
        "--daemon-compare",
        action="store_true",
        help="run additional Sengoo real-incremental measurements with sgc daemon enabled",
    )
    parser.add_argument(
        "--daemon-addr",
        default=DEFAULT_DAEMON_ADDR,
        help=f"sgc daemon listen/connect address (default: {DEFAULT_DAEMON_ADDR})",
    )
    parser.add_argument(
        "--skip-memory-compare",
        action="store_true",
        help="skip compile-memory comparison block",
    )
    return parser.parse_args()


def wait_for_tcp(addr: str, timeout_s: float) -> bool:
    host, port_text = addr.rsplit(":", 1)
    port = int(port_text)
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as sock:
            sock.settimeout(0.3)
            try:
                sock.connect((host, port))
                return True
            except OSError:
                time.sleep(0.1)
    return False


def start_sgc_daemon(sgc_bin: Path, sengoo_root: Path, addr: str) -> subprocess.Popen[Any]:
    proc = subprocess.Popen(
        [str(sgc_bin), "daemon", "--addr", addr],
        cwd=str(sengoo_root),
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        text=True,
    )
    if wait_for_tcp(addr, timeout_s=8.0):
        return proc

    stop_sgc_daemon(proc)
    raise RuntimeError(f"sgc daemon failed to start on {addr}")


def stop_sgc_daemon(proc: subprocess.Popen[Any] | None) -> None:
    if proc is None:
        return
    if proc.poll() is not None:
        return
    proc.terminate()
    try:
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        proc.kill()
        proc.wait(timeout=5)


def latest_advanced_report_path(results_dir: Path) -> Path | None:
    files = sorted(
        results_dir.glob("*-advanced-pipeline.json"),
        key=lambda p: p.stat().st_mtime,
        reverse=True,
    )
    if not files:
        return None
    return files[0]


def sengoo_build_cmd(
    sgc_bin: Path,
    input_file: Path,
    opt_level: int,
    force_rebuild: bool,
    *,
    emit_llvm: bool = False,
    output: Path | None = None,
    daemon_addr: str | None = None,
) -> list[str]:
    cmd = [str(sgc_bin), "build", str(input_file), "-O", str(opt_level)]
    if emit_llvm:
        cmd.append("--emit-llvm")
    if output is not None:
        cmd.extend(["-o", str(output)])
    if force_rebuild:
        cmd.append("--force-rebuild")
    if daemon_addr:
        cmd.extend(["--daemon", "--daemon-addr", daemon_addr])
    return cmd


def render_incremental_sources_sengoo(scenario: str, mutated: bool) -> dict[str, str]:
    if scenario == "loop_body_change":
        body = "acc = acc + mix(i)" if not mutated else "acc = acc + mix(i) - (i % 3)"
        return {
            "main.sg": f"""def bump(x: i64) -> i64 {{
    x + 1
}}

def mix(x: i64) -> i64 {{
    bump(x) * 2
}}

def calc(n: i64) -> i64 {{
    let i = 0
    let acc = 0
    while i < n {{
        {body}
        i = i + 1
    }}
    acc
}}

def main() -> i64 {{
    print(calc(100000))
    0
}}
""",
        }
    if scenario == "function_signature_change":
        if not mutated:
            main = """def bump(x: i64) -> i64 {
    x + 1
}

def mix(x: i64) -> i64 {
    bump(x) * 2
}

def calc(n: i64) -> i64 {
    let i = 0
    let acc = 0
    while i < n {
        acc = acc + mix(i)
        i = i + 1
    }
    acc
}

def main() -> i64 {
    print(calc(100000))
    0
}
"""
        else:
            main = """def bump(x: i64, k: i64) -> i64 {
    x + k
}

def mix(x: i64, scale: i64) -> i64 {
    bump(x, 1) * scale
}

def calc(n: i64) -> i64 {
    let i = 0
    let acc = 0
    while i < n {
        acc = acc + mix(i, 2)
        i = i + 1
    }
    acc
}

def main() -> i64 {
    print(calc(100000))
    0
}
"""
        return {"main.sg": main}
    if scenario == "add_new_function":
        extra = ""
        use_extra = "acc = acc + mix(i)"
        if mutated:
            extra = """
def extra(v: i64) -> i64 {
    v + 3
}
"""
            use_extra = "acc = acc + mix(i) + extra(i % 5)"
        return {
            "main.sg": f"""def bump(x: i64) -> i64 {{
    x + 1
}}

def mix(x: i64) -> i64 {{
    bump(x) * 2
}}
{extra}
def calc(n: i64) -> i64 {{
    let i = 0
    let acc = 0
    while i < n {{
        {use_extra}
        i = i + 1
    }}
    acc
}}

def main() -> i64 {{
    print(calc(100000))
    0
}}
""",
        }
    raise ValueError(f"unknown scenario: {scenario}")


def render_incremental_sources_cpp(scenario: str, mutated: bool) -> dict[str, str]:
    if scenario == "loop_body_change":
        body = "acc += mix(i);" if not mutated else "acc += mix(i) - (i % 3);"
        return {
            "pch.hpp": """#pragma once
#include <array>
#include <cstdint>
#include <iostream>
#include <string>
#include <vector>
""",
            "math_util.hpp": """#pragma once
long long bump(long long x);
long long mix(long long x);
""",
            "math_util.cpp": """#include "math_util.hpp"
long long bump(long long x) { return x + 1; }
long long mix(long long x) { return bump(x) * 2; }
""",
            "main.cpp": f"""#include "math_util.hpp"
int main() {{
    long long acc = 0;
    for (long long i = 0; i < 100000; ++i) {{
        {body}
    }}
    std::cout << acc << "\\n";
    return 0;
}}
""",
        }
    if scenario == "function_signature_change":
        if not mutated:
            util_h = """#pragma once
long long bump(long long x);
long long mix(long long x);
"""
            util_cpp = """#include "math_util.hpp"
long long bump(long long x) { return x + 1; }
long long mix(long long x) { return bump(x) * 2; }
"""
            main_cpp = """#include "math_util.hpp"
int main() {
    long long acc = 0;
    for (long long i = 0; i < 100000; ++i) {
        acc += mix(i);
    }
    std::cout << acc << "\\n";
    return 0;
}
"""
        else:
            util_h = """#pragma once
long long bump(long long x, long long k);
long long mix(long long x, long long scale);
"""
            util_cpp = """#include "math_util.hpp"
long long bump(long long x, long long k) { return x + k; }
long long mix(long long x, long long scale) { return bump(x, 1) * scale; }
"""
            main_cpp = """#include "math_util.hpp"
int main() {
    long long acc = 0;
    for (long long i = 0; i < 100000; ++i) {
        acc += mix(i, 2);
    }
    std::cout << acc << "\\n";
    return 0;
}
"""
        return {
            "pch.hpp": """#pragma once
#include <array>
#include <cstdint>
#include <iostream>
#include <string>
#include <vector>
""",
            "math_util.hpp": util_h,
            "math_util.cpp": util_cpp,
            "main.cpp": main_cpp,
        }
    if scenario == "add_new_function":
        extra_fn = ""
        body = "acc += mix(i);"
        if mutated:
            extra_fn = """
static inline long long extra(long long v) {
    return v + 3;
}
"""
            body = "acc += mix(i) + extra(i % 5);"
        return {
            "pch.hpp": """#pragma once
#include <array>
#include <cstdint>
#include <iostream>
#include <string>
#include <vector>
""",
            "math_util.hpp": """#pragma once
long long bump(long long x);
long long mix(long long x);
""",
            "math_util.cpp": """#include "math_util.hpp"
long long bump(long long x) { return x + 1; }
long long mix(long long x) { return bump(x) * 2; }
""",
            "main.cpp": f"""#include "math_util.hpp"
{extra_fn}
int main() {{
    long long acc = 0;
    for (long long i = 0; i < 100000; ++i) {{
        {body}
    }}
    std::cout << acc << "\\n";
    return 0;
}}
""",
        }
    raise ValueError(f"unknown scenario: {scenario}")


def render_incremental_sources_rust(scenario: str, mutated: bool) -> dict[str, str]:
    cargo_toml = """[package]
name = "inc_case"
version = "0.1.0"
edition = "2021"

[workspace]
"""
    if scenario == "loop_body_change":
        body = "acc += util::mix(i);" if not mutated else "acc += util::mix(i) - (i % 3);"
        main_rs = f"""mod util;

fn main() {{
    let mut acc: i64 = 0;
    for i in 0..100000i64 {{
        {body}
    }}
    println!("{{}}", acc);
}}
"""
        util_rs = """pub fn bump(x: i64) -> i64 {
    x + 1
}

pub fn mix(x: i64) -> i64 {
    bump(x) * 2
}
"""
        return {"Cargo.toml": cargo_toml, "src/main.rs": main_rs, "src/util.rs": util_rs}
    if scenario == "function_signature_change":
        if not mutated:
            main_rs = """mod util;

fn main() {
    let mut acc: i64 = 0;
    for i in 0..100000i64 {
        acc += util::mix(i);
    }
    println!("{}", acc);
}
"""
            util_rs = """pub fn bump(x: i64) -> i64 {
    x + 1
}

pub fn mix(x: i64) -> i64 {
    bump(x) * 2
}
"""
        else:
            main_rs = """mod util;

fn main() {
    let mut acc: i64 = 0;
    for i in 0..100000i64 {
        acc += util::mix(i, 2);
    }
    println!("{}", acc);
}
"""
            util_rs = """pub fn bump(x: i64, k: i64) -> i64 {
    x + k
}

pub fn mix(x: i64, scale: i64) -> i64 {
    bump(x, 1) * scale
}
"""
        return {"Cargo.toml": cargo_toml, "src/main.rs": main_rs, "src/util.rs": util_rs}
    if scenario == "add_new_function":
        extra = ""
        body = "acc += util::mix(i);"
        if mutated:
            extra = """
#[inline(always)]
fn extra(v: i64) -> i64 {
    v + 3
}
"""
            body = "acc += util::mix(i) + extra(i % 5);"
        main_rs = f"""mod util;
{extra}
fn main() {{
    let mut acc: i64 = 0;
    for i in 0..100000i64 {{
        {body}
    }}
    println!("{{}}", acc);
}}
"""
        util_rs = """pub fn bump(x: i64) -> i64 {
    x + 1
}

pub fn mix(x: i64) -> i64 {
    bump(x) * 2
}
"""
        return {"Cargo.toml": cargo_toml, "src/main.rs": main_rs, "src/util.rs": util_rs}
    raise ValueError(f"unknown scenario: {scenario}")


def render_incremental_sources_python(scenario: str, mutated: bool) -> dict[str, str]:
    if scenario == "loop_body_change":
        body = "acc += mix(i)" if not mutated else "acc += mix(i) - (i % 3)"
        return {
            "util.py": """def bump(x: int) -> int:
    return x + 1

def mix(x: int) -> int:
    return bump(x) * 2
""",
            "main.py": f"""from util import mix

def main() -> None:
    acc = 0
    for i in range(100000):
        {body}
    print(acc)

if __name__ == "__main__":
    main()
""",
        }
    if scenario == "function_signature_change":
        if not mutated:
            util_py = """def bump(x: int) -> int:
    return x + 1

def mix(x: int) -> int:
    return bump(x) * 2
"""
            main_py = """from util import mix

def main() -> None:
    acc = 0
    for i in range(100000):
        acc += mix(i)
    print(acc)

if __name__ == "__main__":
    main()
"""
        else:
            util_py = """def bump(x: int, k: int) -> int:
    return x + k

def mix(x: int, scale: int) -> int:
    return bump(x, 1) * scale
"""
            main_py = """from util import mix

def main() -> None:
    acc = 0
    for i in range(100000):
        acc += mix(i, 2)
    print(acc)

if __name__ == "__main__":
    main()
"""
        return {"util.py": util_py, "main.py": main_py}
    if scenario == "add_new_function":
        extra = ""
        body = "acc += mix(i)"
        if mutated:
            extra = """
def extra(v: int) -> int:
    return v + 3
"""
            body = "acc += mix(i) + extra(i % 5)"
        return {
            "util.py": """def bump(x: int) -> int:
    return x + 1

def mix(x: int) -> int:
    return bump(x) * 2
""",
            "main.py": f"""from util import mix
{extra}
def main() -> None:
    acc = 0
    for i in range(100000):
        {body}
    print(acc)

if __name__ == "__main__":
    main()
""",
        }
    raise ValueError(f"unknown scenario: {scenario}")


def write_sources(base_dir: Path, file_map: dict[str, str]) -> None:
    for rel_path, content in file_map.items():
        write_file(base_dir / rel_path, content)


def build_cpp_pch(clangpp: str, cpp_dir: Path) -> None:
    pch_header = cpp_dir / "pch.hpp"
    pch_file = cpp_dir / "pch.hpp.pch"
    run_checked(
        [clangpp, "-std=c++20", "-O2", "-x", "c++-header", str(pch_header), "-o", str(pch_file)],
        cwd=cpp_dir,
    )


def cpp_full_build_ms(clangpp: str, cpp_dir: Path) -> float:
    main_obj = cpp_dir / "main.obj"
    util_obj = cpp_dir / "math_util.obj"
    out_exe = cpp_dir / exe_name("app")
    pch_file = cpp_dir / "pch.hpp.pch"
    started = time.perf_counter()
    run_checked(
        [clangpp, "-std=c++20", "-O2", "-include-pch", str(pch_file), "-c", str(cpp_dir / "math_util.cpp"), "-o", str(util_obj)],
        cwd=cpp_dir,
    )
    run_checked(
        [clangpp, "-std=c++20", "-O2", "-include-pch", str(pch_file), "-c", str(cpp_dir / "main.cpp"), "-o", str(main_obj)],
        cwd=cpp_dir,
    )
    run_checked(clang_link_cmd(clangpp, [main_obj, util_obj], out_exe), cwd=cpp_dir)
    return (time.perf_counter() - started) * 1000.0


def cpp_incremental_build_ms(clangpp: str, cpp_dir: Path, changed_units: list[str]) -> float:
    main_obj = cpp_dir / "main.obj"
    util_obj = cpp_dir / "math_util.obj"
    out_exe = cpp_dir / exe_name("app")
    pch_file = cpp_dir / "pch.hpp.pch"
    started = time.perf_counter()
    if "math_util.cpp" in changed_units:
        run_checked(
            [clangpp, "-std=c++20", "-O2", "-include-pch", str(pch_file), "-c", str(cpp_dir / "math_util.cpp"), "-o", str(util_obj)],
            cwd=cpp_dir,
        )
    if "main.cpp" in changed_units:
        run_checked(
            [clangpp, "-std=c++20", "-O2", "-include-pch", str(pch_file), "-c", str(cpp_dir / "main.cpp"), "-o", str(main_obj)],
            cwd=cpp_dir,
        )
    run_checked(clang_link_cmd(clangpp, [main_obj, util_obj], out_exe), cwd=cpp_dir)
    return (time.perf_counter() - started) * 1000.0


def measure_real_incremental(
    bench_root: Path,
    sgc_bin: Path,
    clangpp: str,
    cargo: str,
    py: str,
    sengoo_root: Path,
    *,
    include_languages: tuple[str, ...] = ("sengoo", "cpp", "rust", "python"),
    sengoo_daemon_addr: str | None = None,
) -> dict[str, dict[str, Any]]:
    work = bench_root / ".advanced-work" / "real-incremental"
    clear_dir(work)
    results: dict[str, dict[str, Any]] = {}
    enabled = set(include_languages)

    rust_env = {
        "CARGO_INCREMENTAL": "1",
    }

    for scenario in INCREMENTAL_SCENARIOS:
        results[scenario] = {}

        if "sengoo" in enabled:
            sg_dir = work / "sengoo" / scenario
            clear_dir(sg_dir)
            before_samples: list[float] = []
            after_samples: list[float] = []
            ll_path = sg_dir / "main.ll"
            # Warm once per scenario so measured samples reflect steady-state incremental behavior.
            for _ in range(DAEMON_REAL_INCREMENTAL_WARMUP_ITERS):
                write_sources(sg_dir, render_incremental_sources_sengoo(scenario, mutated=False))
                run_checked(
                    sengoo_build_cmd(
                        sgc_bin,
                        sg_dir / "main.sg",
                        2,
                        True,
                        emit_llvm=True,
                        output=ll_path,
                        daemon_addr=sengoo_daemon_addr,
                    ),
                    cwd=sengoo_root,
                )
                write_sources(sg_dir, render_incremental_sources_sengoo(scenario, mutated=True))
                run_checked(
                    sengoo_build_cmd(
                        sgc_bin,
                        sg_dir / "main.sg",
                        2,
                        False,
                        emit_llvm=True,
                        output=ll_path,
                        daemon_addr=sengoo_daemon_addr,
                    ),
                    cwd=sengoo_root,
                )
            for _ in range(INCREMENTAL_ITERS):
                write_sources(sg_dir, render_incremental_sources_sengoo(scenario, mutated=False))
                before_samples.append(
                    measure_command_ms(
                        sengoo_build_cmd(
                            sgc_bin,
                            sg_dir / "main.sg",
                            2,
                            True,
                            emit_llvm=True,
                            output=ll_path,
                            daemon_addr=sengoo_daemon_addr,
                        ),
                        cwd=sengoo_root,
                    )
                )
                write_sources(sg_dir, render_incremental_sources_sengoo(scenario, mutated=True))
                after_samples.append(
                    measure_command_ms(
                        sengoo_build_cmd(
                            sgc_bin,
                            sg_dir / "main.sg",
                            2,
                            False,
                            emit_llvm=True,
                            output=ll_path,
                            daemon_addr=sengoo_daemon_addr,
                        ),
                        cwd=sengoo_root,
                    )
                )
            bavg = average(before_samples) or 0.0
            aavg = average(after_samples) or 0.0
            results[scenario]["sengoo"] = {
                "before_samples_ms": before_samples,
                "after_samples_ms": after_samples,
                "before_avg_ms": bavg,
                "after_avg_ms": aavg,
                "reduction_pct": ((bavg - aavg) / bavg * 100.0) if bavg > 0 else None,
            }

        if "cpp" in enabled:
            cpp_dir = work / "cpp" / scenario
            clear_dir(cpp_dir)
            before_samples = []
            after_samples = []
            changed_units = ["main.cpp"]
            if scenario == "function_signature_change":
                changed_units = ["math_util.cpp", "main.cpp"]
            for _ in range(INCREMENTAL_ITERS):
                write_sources(cpp_dir, render_incremental_sources_cpp(scenario, mutated=False))
                build_cpp_pch(clangpp, cpp_dir)
                before_samples.append(cpp_full_build_ms(clangpp, cpp_dir))
                write_sources(cpp_dir, render_incremental_sources_cpp(scenario, mutated=True))
                after_samples.append(cpp_incremental_build_ms(clangpp, cpp_dir, changed_units))
            bavg = average(before_samples) or 0.0
            aavg = average(after_samples) or 0.0
            results[scenario]["cpp"] = {
                "before_samples_ms": before_samples,
                "after_samples_ms": after_samples,
                "before_avg_ms": bavg,
                "after_avg_ms": aavg,
                "reduction_pct": ((bavg - aavg) / bavg * 100.0) if bavg > 0 else None,
                "fairness": {"pch_enabled": True},
            }

        if "rust" in enabled:
            rust_dir = work / "rust" / scenario
            clear_dir(rust_dir)
            before_samples = []
            after_samples = []
            for _ in range(INCREMENTAL_ITERS):
                write_sources(rust_dir, render_incremental_sources_rust(scenario, mutated=False))
                target_dir = rust_dir / "target"
                if target_dir.exists():
                    shutil.rmtree(target_dir)
                before_samples.append(
                    measure_command_ms([cargo, "build", "--quiet"], cwd=rust_dir, env=rust_env)
                )
                write_sources(rust_dir, render_incremental_sources_rust(scenario, mutated=True))
                after_samples.append(
                    measure_command_ms([cargo, "build", "--quiet"], cwd=rust_dir, env=rust_env)
                )
            bavg = average(before_samples) or 0.0
            aavg = average(after_samples) or 0.0
            results[scenario]["rust"] = {
                "before_samples_ms": before_samples,
                "after_samples_ms": after_samples,
                "before_avg_ms": bavg,
                "after_avg_ms": aavg,
                "reduction_pct": ((bavg - aavg) / bavg * 100.0) if bavg > 0 else None,
                "fairness": {"cargo_incremental": True},
            }

        if "python" in enabled:
            py_dir = work / "python" / scenario
            clear_dir(py_dir)
            before_samples = []
            after_samples = []
            for _ in range(INCREMENTAL_ITERS):
                write_sources(py_dir, render_incremental_sources_python(scenario, mutated=False))
                clear_pycache(py_dir)
                before_samples.append(
                    measure_command_ms([py, "-m", "compileall", "-q", str(py_dir)], cwd=py_dir)
                )
                write_sources(py_dir, render_incremental_sources_python(scenario, mutated=True))
                after_samples.append(
                    measure_command_ms([py, "-m", "compileall", "-q", str(py_dir)], cwd=py_dir)
                )
            bavg = average(before_samples) or 0.0
            aavg = average(after_samples) or 0.0
            results[scenario]["python"] = {
                "before_samples_ms": before_samples,
                "after_samples_ms": after_samples,
                "before_avg_ms": bavg,
                "after_avg_ms": aavg,
                "reduction_pct": ((bavg - aavg) / bavg * 100.0) if bavg > 0 else None,
            }

    return results


def make_scale_source_sengoo(target_loc: int) -> str:
    lines = []
    fn_count = max(50, target_loc // 4)
    for i in range(fn_count):
        lines.append(f"def f{i}(x: i64) -> i64 {{")
        lines.append(f"    x + {i % 7}")
        lines.append("}")
        lines.append("")
    lines.extend(
        [
            "def main() -> i64 {",
            "    let acc = 0",
            "    let i = 0",
            "    while i < 1000 {",
            f"        acc = acc + f{fn_count - 1}(i)",
            "        i = i + 1",
            "    }",
            "    print(acc)",
            "    0",
            "}",
        ]
    )
    return "\n".join(lines) + "\n"


def make_reachability_source_sengoo(profile: str, target_loc: int) -> tuple[str, str]:
    fn_count = max(100, target_loc // 4)
    reachable_count = fn_count
    include_main = True
    if profile == "half_reachable":
        reachable_count = max(1, fn_count // 2)
    elif profile == "library_entryless":
        include_main = False
    elif profile != "all_reachable":
        raise ValueError(f"unknown reachability profile: {profile}")

    lines: list[str] = []
    for i in range(fn_count):
        lines.append(f"def f{i}(x: i64) -> i64 {{")
        if i < reachable_count - 1:
            lines.append(f"    f{i + 1}(x + {i % 7})")
        else:
            lines.append(f"    x + {i % 7}")
        lines.append("}")
        lines.append("")

    if include_main:
        lines.extend(
            [
                "def main() -> i64 {",
                "    print(f0(42))",
                "    0",
                "}",
            ]
        )

    file_name = "main.sg" if include_main else "lib.sg"
    return file_name, "\n".join(lines) + "\n"


def make_scale_source_cpp(target_loc: int) -> str:
    lines = ['#include "pch.hpp"', "", "static inline long long seed(long long x) { return x; }", ""]
    fn_count = max(50, target_loc // 3)
    for i in range(fn_count):
        lines.append(f"static inline long long f{i}(long long x) {{ return x + {i % 7}; }}")
    lines.extend(
        [
            "",
            "int main() {",
            "    long long acc = 0;",
            "    for (long long i = 0; i < 1000; ++i) {",
            f"        acc += f{fn_count - 1}(i);",
            "    }",
            '    std::cout << acc << "\\n";',
            "    return 0;",
            "}",
            "",
        ]
    )
    return "\n".join(lines)


def make_scale_source_rust(target_loc: int) -> str:
    lines = []
    fn_count = max(50, target_loc // 3)
    for i in range(fn_count):
        lines.append(f"#[inline(always)] fn f{i}(x: i64) -> i64 {{ x + {i % 7} }}")
    lines.extend(
        [
            "",
            "fn main() {",
            "    let mut acc: i64 = 0;",
            "    for i in 0..1000i64 {",
            f"        acc += f{fn_count - 1}(i);",
            "    }",
            '    println!("{}", acc);',
            "}",
            "",
        ]
    )
    return "\n".join(lines)


def make_scale_source_python(target_loc: int) -> str:
    lines = []
    fn_count = max(50, target_loc // 3)
    for i in range(fn_count):
        lines.append(f"def f{i}(x: int) -> int:")
        lines.append(f"    return x + {i % 7}")
        lines.append("")
    lines.extend(
        [
            "def main() -> None:",
            "    acc = 0",
            "    for i in range(1000):",
            f"        acc += f{fn_count - 1}(i)",
            "    print(acc)",
            "",
            'if __name__ == "__main__":',
            "    main()",
            "",
        ]
    )
    return "\n".join(lines)


def measure_scale_curve(
    bench_root: Path,
    sgc_bin: Path,
    clangpp: str,
    cargo: str,
    py: str,
    sengoo_root: Path,
    *,
    sengoo_daemon_addr: str | None = None,
) -> dict[str, dict[str, Any]]:
    work = bench_root / ".advanced-work" / "scale-curve"
    clear_dir(work)
    results: dict[str, dict[str, Any]] = {}

    rust_env = {
        "CARGO_INCREMENTAL": "1",
    }
    runtime_c = sengoo_root / "tools" / "stdlib" / "runtime.c"
    if not runtime_c.exists():
        raise RuntimeError(f"runtime source not found: {runtime_c}")
    runtime_obj = work / "sengoo_runtime.obj"
    run_checked([clangpp, "-O2", "-x", "c", "-c", str(runtime_c), "-o", str(runtime_obj)], cwd=sengoo_root)

    for loc in SCALE_LOC_BUCKETS:
        iters = SCALE_ITERS_BY_LOC.get(loc, 1)
        loc_key = str(loc)
        results[loc_key] = {}

        # Sengoo (front-end LLVM + backend obj + link)
        sg_dir = work / "sengoo" / loc_key
        clear_dir(sg_dir)
        write_file(sg_dir / "main.sg", make_scale_source_sengoo(loc))
        front_samples: list[float] = []
        backend_samples: list[float] = []
        link_samples: list[float] = []
        e2e_samples: list[float] = []
        # Warm once to avoid counting cold cache/bootstrap overhead in steady-state samples.
        ll = sg_dir / "main.ll"
        run_checked(
            sengoo_build_cmd(
                sgc_bin,
                sg_dir / "main.sg",
                2,
                True,
                emit_llvm=True,
                output=ll,
                daemon_addr=sengoo_daemon_addr,
            ),
            cwd=sengoo_root,
        )
        for _ in range(iters):
            ll = sg_dir / "main.ll"
            obj = sg_dir / "main.obj"
            exe = sg_dir / exe_name("main")
            front_ms = measure_command_ms(
                sengoo_build_cmd(
                    sgc_bin,
                    sg_dir / "main.sg",
                    2,
                    False,
                    emit_llvm=True,
                    output=ll,
                    daemon_addr=sengoo_daemon_addr,
                ),
                cwd=sengoo_root,
            )
            backend_ms = measure_command_ms([clangpp, "-O2", "-x", "ir", "-c", str(ll), "-o", str(obj)], cwd=sg_dir)
            link_ms = measure_command_ms(clang_link_cmd(clangpp, [obj, runtime_obj], exe), cwd=sg_dir)
            front_samples.append(front_ms)
            backend_samples.append(backend_ms)
            link_samples.append(link_ms)
            e2e_samples.append(front_ms + backend_ms + link_ms)
        results[loc_key]["sengoo"] = {
            "compile_frontend_llvm_avg_ms": average(front_samples),
            "codegen_obj_avg_ms": average(backend_samples),
            "link_avg_ms": average(link_samples),
            "e2e_avg_ms": average(e2e_samples),
            "e2e_p50_ms": percentile(e2e_samples, 0.50),
        }

        # C++ with PCH (compile obj + link)
        cpp_dir = work / "cpp" / loc_key
        clear_dir(cpp_dir)
        write_file(
            cpp_dir / "pch.hpp",
            """#pragma once
#include <array>
#include <cstdint>
#include <iostream>
#include <string>
#include <vector>
""",
        )
        write_file(cpp_dir / "main.cpp", make_scale_source_cpp(loc))
        build_cpp_pch(clangpp, cpp_dir)
        compile_samples = []
        link_samples = []
        e2e_samples = []
        for _ in range(iters):
            obj = cpp_dir / "main.obj"
            exe = cpp_dir / exe_name("main")
            compile_ms = measure_command_ms(
                [clangpp, "-std=c++20", "-O2", "-include-pch", str(cpp_dir / "pch.hpp.pch"), "-c", str(cpp_dir / "main.cpp"), "-o", str(obj)],
                cwd=cpp_dir,
            )
            link_ms = measure_command_ms(clang_link_cmd(clangpp, [obj], exe), cwd=cpp_dir)
            compile_samples.append(compile_ms)
            link_samples.append(link_ms)
            e2e_samples.append(compile_ms + link_ms)
        results[loc_key]["cpp"] = {
            "compile_obj_avg_ms": average(compile_samples),
            "link_avg_ms": average(link_samples),
            "e2e_avg_ms": average(e2e_samples),
            "e2e_p50_ms": percentile(e2e_samples, 0.50),
            "fairness": {"pch_enabled": True},
        }

        # Rust (cargo build e2e includes link)
        rust_dir = work / "rust" / loc_key
        clear_dir(rust_dir)
        write_file(
            rust_dir / "Cargo.toml",
            """[package]
name = "scale_case"
version = "0.1.0"
edition = "2021"

[workspace]
""",
        )
        write_file(rust_dir / "src" / "main.rs", make_scale_source_rust(loc))
        e2e_samples = []
        for _ in range(iters):
            target_dir = rust_dir / "target"
            if target_dir.exists():
                shutil.rmtree(target_dir)
            e2e_samples.append(
                measure_command_ms([cargo, "build", "--quiet"], cwd=rust_dir, env=rust_env)
            )
        results[loc_key]["rust"] = {
            "e2e_avg_ms": average(e2e_samples),
            "e2e_p50_ms": percentile(e2e_samples, 0.50),
            "fairness": {"cargo_incremental": True},
            "note": "cargo build includes compile+link; split link-only timing is not isolated",
        }

        # Python (py_compile, no link stage)
        py_dir = work / "python" / loc_key
        clear_dir(py_dir)
        write_file(py_dir / "main.py", make_scale_source_python(loc))
        compile_samples = []
        for _ in range(iters):
            clear_pycache(py_dir)
            compile_samples.append(measure_command_ms([py, "-m", "py_compile", str(py_dir / "main.py")], cwd=py_dir))
        results[loc_key]["python"] = {
            "compile_avg_ms": average(compile_samples),
            "e2e_avg_ms": average(compile_samples),
            "e2e_p50_ms": percentile(compile_samples, 0.50),
            "note": "python has no native link stage in this benchmark",
        }

    return results


def measure_compile_memory_compare(
    bench_root: Path,
    sgc_bin: Path,
    clangpp: str,
    py: str,
    sengoo_root: Path,
    *,
    sengoo_daemon_addr: str | None = None,
) -> dict[str, Any]:
    work = bench_root / ".advanced-work" / "compile-memory-compare"
    clear_dir(work)
    rustc = resolve_rustc_binary()

    out: dict[str, Any] = {
        "_meta": {
            "loc_buckets": MEMORY_LOC_BUCKETS,
            "iters_by_loc": MEMORY_ITERS_BY_LOC,
            "sample_interval_ms": int(MEMORY_SAMPLE_INTERVAL_S * 1000),
            "note": "peak_rss_mb_* is available on all platforms; peak_private_mb_* depends on OS support.",
        }
    }

    for loc in MEMORY_LOC_BUCKETS:
        loc_key = str(loc)
        iters = MEMORY_ITERS_BY_LOC.get(loc, 1)
        out[loc_key] = {}

        # Sengoo (front-end compile only: emit LLVM IR).
        sg_dir = work / "sengoo" / loc_key
        clear_dir(sg_dir)
        write_file(sg_dir / "main.sg", make_scale_source_sengoo(loc))
        sg_elapsed: list[float] = []
        sg_rss: list[int] = []
        sg_private: list[int] = []
        for _ in range(iters):
            ll = sg_dir / "main.ll"
            sample = measure_command_peak_memory(
                sengoo_build_cmd(
                    sgc_bin,
                    sg_dir / "main.sg",
                    2,
                    True,
                    emit_llvm=True,
                    output=ll,
                    daemon_addr=sengoo_daemon_addr,
                ),
                cwd=sengoo_root,
            )
            sg_elapsed.append(float(sample["elapsed_ms"] or 0.0))
            if isinstance(sample["peak_rss_bytes"], float):
                sg_rss.append(int(sample["peak_rss_bytes"]))
            if isinstance(sample["peak_private_bytes"], float):
                sg_private.append(int(sample["peak_private_bytes"]))
        out[loc_key]["sengoo"] = {
            "compile_avg_ms": average(sg_elapsed),
            "peak_rss_mb_avg": average_bytes_as_mb(sg_rss),
            "peak_rss_mb_p50": percentile_bytes_as_mb(sg_rss, 0.50),
            "peak_private_mb_avg": average_bytes_as_mb(sg_private),
            "peak_private_mb_p50": percentile_bytes_as_mb(sg_private, 0.50),
            "iters": iters,
            "compile_mode": "frontend_llvm_emit",
        }

        # C++ (compile only, PCH enabled).
        cpp_dir = work / "cpp" / loc_key
        clear_dir(cpp_dir)
        write_file(
            cpp_dir / "pch.hpp",
            """#pragma once
#include <array>
#include <cstdint>
#include <iostream>
#include <string>
#include <vector>
""",
        )
        write_file(cpp_dir / "main.cpp", make_scale_source_cpp(loc))
        build_cpp_pch(clangpp, cpp_dir)
        cpp_elapsed: list[float] = []
        cpp_rss: list[int] = []
        cpp_private: list[int] = []
        for _ in range(iters):
            obj = cpp_dir / "main.obj"
            sample = measure_command_peak_memory(
                [
                    clangpp,
                    "-std=c++20",
                    "-O2",
                    "-include-pch",
                    str(cpp_dir / "pch.hpp.pch"),
                    "-c",
                    str(cpp_dir / "main.cpp"),
                    "-o",
                    str(obj),
                ],
                cwd=cpp_dir,
            )
            cpp_elapsed.append(float(sample["elapsed_ms"] or 0.0))
            if isinstance(sample["peak_rss_bytes"], float):
                cpp_rss.append(int(sample["peak_rss_bytes"]))
            if isinstance(sample["peak_private_bytes"], float):
                cpp_private.append(int(sample["peak_private_bytes"]))
        out[loc_key]["cpp"] = {
            "compile_avg_ms": average(cpp_elapsed),
            "peak_rss_mb_avg": average_bytes_as_mb(cpp_rss),
            "peak_rss_mb_p50": percentile_bytes_as_mb(cpp_rss, 0.50),
            "peak_private_mb_avg": average_bytes_as_mb(cpp_private),
            "peak_private_mb_p50": percentile_bytes_as_mb(cpp_private, 0.50),
            "iters": iters,
            "fairness": {"pch_enabled": True},
            "compile_mode": "compile_obj_only",
        }

        # Rust (direct rustc compile-to-object, bypass rustup shim when available).
        rust_dir = work / "rust" / loc_key
        clear_dir(rust_dir)
        write_file(rust_dir / "main.rs", make_scale_source_rust(loc))
        rust_elapsed: list[float] = []
        rust_rss: list[int] = []
        rust_private: list[int] = []
        for _ in range(iters):
            obj = rust_dir / "main.o"
            sample = measure_command_peak_memory(
                [rustc, "-O", "-Awarnings", "--emit=obj", str(rust_dir / "main.rs"), "-o", str(obj)],
                cwd=rust_dir,
            )
            rust_elapsed.append(float(sample["elapsed_ms"] or 0.0))
            if isinstance(sample["peak_rss_bytes"], float):
                rust_rss.append(int(sample["peak_rss_bytes"]))
            if isinstance(sample["peak_private_bytes"], float):
                rust_private.append(int(sample["peak_private_bytes"]))
        out[loc_key]["rust"] = {
            "compile_avg_ms": average(rust_elapsed),
            "peak_rss_mb_avg": average_bytes_as_mb(rust_rss),
            "peak_rss_mb_p50": percentile_bytes_as_mb(rust_rss, 0.50),
            "peak_private_mb_avg": average_bytes_as_mb(rust_private),
            "peak_private_mb_p50": percentile_bytes_as_mb(rust_private, 0.50),
            "iters": iters,
            "fairness": {"rustc_direct": True},
            "compile_mode": "compile_obj_only",
        }

        # Python (bytecode compile only).
        py_dir = work / "python" / loc_key
        clear_dir(py_dir)
        write_file(py_dir / "main.py", make_scale_source_python(loc))
        py_elapsed: list[float] = []
        py_rss: list[int] = []
        py_private: list[int] = []
        for _ in range(iters):
            clear_pycache(py_dir)
            sample = measure_command_peak_memory([py, "-m", "py_compile", str(py_dir / "main.py")], cwd=py_dir)
            py_elapsed.append(float(sample["elapsed_ms"] or 0.0))
            if isinstance(sample["peak_rss_bytes"], float):
                py_rss.append(int(sample["peak_rss_bytes"]))
            if isinstance(sample["peak_private_bytes"], float):
                py_private.append(int(sample["peak_private_bytes"]))
        out[loc_key]["python"] = {
            "compile_avg_ms": average(py_elapsed),
            "peak_rss_mb_avg": average_bytes_as_mb(py_rss),
            "peak_rss_mb_p50": percentile_bytes_as_mb(py_rss, 0.50),
            "peak_private_mb_avg": average_bytes_as_mb(py_private),
            "peak_private_mb_p50": percentile_bytes_as_mb(py_private, 0.50),
            "iters": iters,
            "compile_mode": "py_compile",
            "note": "python benchmark has no native link stage",
        }

    return out


def measure_reachability_matrix(
    bench_root: Path,
    sgc_bin: Path,
    clangpp: str,
    sengoo_root: Path,
    *,
    sengoo_daemon_addr: str | None = None,
) -> dict[str, Any]:
    work = bench_root / ".advanced-work" / "reachability-matrix"
    clear_dir(work)
    results: dict[str, Any] = {}

    runtime_c = sengoo_root / "tools" / "stdlib" / "runtime.c"
    if not runtime_c.exists():
        raise RuntimeError(f"runtime source not found: {runtime_c}")
    runtime_obj = work / "sengoo_runtime.obj"
    run_checked([clangpp, "-O2", "-x", "c", "-c", str(runtime_c), "-o", str(runtime_obj)], cwd=sengoo_root)

    for profile in REACHABILITY_PROFILES:
        profile_dir = work / profile
        clear_dir(profile_dir)
        source_file, source_text = make_reachability_source_sengoo(profile, REACHABILITY_LOC)
        src_path = profile_dir / source_file
        write_file(src_path, source_text)

        front_samples: list[float] = []
        backend_samples: list[float] = []
        link_samples: list[float] = []
        e2e_samples: list[float] = []

        # Warm once to focus the matrix on reachable-set behavior, not first compile bootstrap.
        ll = profile_dir / f"{profile}.ll"
        run_checked(
            sengoo_build_cmd(
                sgc_bin,
                src_path,
                2,
                True,
                emit_llvm=True,
                output=ll,
                daemon_addr=sengoo_daemon_addr,
            ),
            cwd=sengoo_root,
        )
        for _ in range(REACHABILITY_ITERS):
            ll = profile_dir / f"{profile}.ll"
            obj = profile_dir / f"{profile}.obj"
            exe = profile_dir / exe_name(profile)
            front_ms = measure_command_ms(
                sengoo_build_cmd(
                    sgc_bin,
                    src_path,
                    2,
                    False,
                    emit_llvm=True,
                    output=ll,
                    daemon_addr=sengoo_daemon_addr,
                ),
                cwd=sengoo_root,
            )
            backend_ms = measure_command_ms([clangpp, "-O2", "-x", "ir", "-c", str(ll), "-o", str(obj)], cwd=profile_dir)
            front_samples.append(front_ms)
            backend_samples.append(backend_ms)

            if profile == "library_entryless":
                e2e_samples.append(front_ms + backend_ms)
            else:
                link_ms = measure_command_ms(clang_link_cmd(clangpp, [obj, runtime_obj], exe), cwd=profile_dir)
                link_samples.append(link_ms)
                e2e_samples.append(front_ms + backend_ms + link_ms)

        results[profile] = {
            "compile_frontend_llvm_avg_ms": average(front_samples),
            "codegen_obj_avg_ms": average(backend_samples),
            "link_avg_ms": average(link_samples) if link_samples else None,
            "e2e_avg_ms": average(e2e_samples),
            "e2e_p50_ms": percentile(e2e_samples, 0.50),
            "iters": REACHABILITY_ITERS,
            "loc_target": REACHABILITY_LOC,
        }

    all_reachable = results.get("all_reachable", {})
    half_reachable = results.get("half_reachable", {})
    entryless = results.get("library_entryless", {})
    results["delta_vs_all_reachable_ms"] = {
        "half_reachable_e2e_delta_ms": float(half_reachable.get("e2e_avg_ms", 0.0) or 0.0)
        - float(all_reachable.get("e2e_avg_ms", 0.0) or 0.0),
        "library_entryless_e2e_delta_ms": float(entryless.get("e2e_avg_ms", 0.0) or 0.0)
        - float(all_reachable.get("e2e_avg_ms", 0.0) or 0.0),
    }
    return results


def print_incremental_tables(real_incremental: dict[str, dict[str, Any]]) -> None:
    langs = ["sengoo", "cpp", "rust", "python"]
    print("")
    print("Real Incremental Scenarios (before/after/reduction)")
    for scenario in INCREMENTAL_SCENARIOS:
        print("")
        print(f"Scenario: {scenario}")
        print("| Language | Before avg (ms) | After avg (ms) | Reduction (%) |")
        print("|---|---:|---:|---:|")
        for lang in langs:
            metrics = real_incremental[scenario][lang]
            reduction = metrics.get("reduction_pct")
            red_str = "n/a" if reduction is None else f"{reduction:.2f}"
            print(
                f"| {lang} | {metrics['before_avg_ms']:.2f} | {metrics['after_avg_ms']:.2f} | {red_str} |"
            )


def print_scale_tables(scale_curve: dict[str, dict[str, Any]]) -> None:
    langs = ["sengoo", "cpp", "rust", "python"]
    print("")
    print("Scale Curve: E2E Build Time (includes link where applicable)")
    print("| LOC | Sengoo (ms) | C++ (ms) | Rust (ms) | Python (ms) |")
    print("|---:|---:|---:|---:|---:|")
    for loc_key in [str(x) for x in SCALE_LOC_BUCKETS]:
        row = []
        for lang in langs:
            row.append(f"{scale_curve[loc_key][lang]['e2e_avg_ms']:.2f}")
        print(f"| {loc_key} | {' | '.join(row)} |")

    print("")
    print("Link Share (Sengoo/C++)")
    print("| LOC | Sengoo link share (%) | C++ link share (%) |")
    print("|---:|---:|---:|")
    for loc_key in [str(x) for x in SCALE_LOC_BUCKETS]:
        sg = scale_curve[loc_key]["sengoo"]
        cpp = scale_curve[loc_key]["cpp"]
        sg_share = (sg["link_avg_ms"] / sg["e2e_avg_ms"] * 100.0) if sg["e2e_avg_ms"] else 0.0
        cpp_share = (cpp["link_avg_ms"] / cpp["e2e_avg_ms"] * 100.0) if cpp["e2e_avg_ms"] else 0.0
        print(f"| {loc_key} | {sg_share:.2f} | {cpp_share:.2f} |")

    print("")
    print("Frontend Share (Sengoo)")
    print("| LOC | Frontend share (%) |")
    print("|---:|---:|")
    frontend_share_by_loc: dict[str, float] = {}
    for loc_key in [str(x) for x in SCALE_LOC_BUCKETS]:
        sg = scale_curve[loc_key]["sengoo"]
        share = (
            (float(sg["compile_frontend_llvm_avg_ms"]) / float(sg["e2e_avg_ms"]) * 100.0)
            if sg["e2e_avg_ms"]
            else 0.0
        )
        frontend_share_by_loc[loc_key] = share
        print(f"| {loc_key} | {share:.2f} |")

    share_100k = frontend_share_by_loc.get("100000", 0.0)
    share_1000k = frontend_share_by_loc.get("1000000", 0.0)
    delta_pp = share_1000k - share_100k
    if delta_pp > 0.05:
        trend = "increasing"
    elif delta_pp < -0.05:
        trend = "decreasing"
    else:
        trend = "stable"
    print(
        f"Frontend share trend 100k -> 1000k: {share_100k:.2f}% -> {share_1000k:.2f}% ({trend}, {delta_pp:+.2f}pp)"
    )


def print_compile_memory_compare(compile_memory_compare: dict[str, Any]) -> None:
    print("")
    print("Compile Peak Memory (RSS MB, lower is better)")
    print("| LOC | Sengoo RSS | C++ RSS | Rust RSS | Python RSS |")
    print("|---:|---:|---:|---:|---:|")
    for loc_key in [str(x) for x in MEMORY_LOC_BUCKETS]:
        loc_metrics = compile_memory_compare.get(loc_key, {})
        row: list[str] = []
        for lang in ("sengoo", "cpp", "rust", "python"):
            lang_metrics = loc_metrics.get(lang, {})
            rss = lang_metrics.get("peak_rss_mb_avg")
            row.append("n/a" if rss is None else f"{float(rss):.2f}")
        print(f"| {loc_key} | {' | '.join(row)} |")

    print("")
    print("Compile Peak Memory (private MB, when available)")
    print("| LOC | Sengoo Private | C++ Private | Rust Private | Python Private |")
    print("|---:|---:|---:|---:|---:|")
    for loc_key in [str(x) for x in MEMORY_LOC_BUCKETS]:
        loc_metrics = compile_memory_compare.get(loc_key, {})
        row: list[str] = []
        for lang in ("sengoo", "cpp", "rust", "python"):
            lang_metrics = loc_metrics.get(lang, {})
            private = lang_metrics.get("peak_private_mb_avg")
            row.append("n/a" if private is None else f"{float(private):.2f}")
        print(f"| {loc_key} | {' | '.join(row)} |")


def print_reachability_matrix(reachability_matrix: dict[str, Any]) -> None:
    print("")
    print("Reachability Matrix (Sengoo, 100k LOC)")
    print("| Profile | Frontend (ms) | Codegen (ms) | Link (ms) | E2E (ms) |")
    print("|---|---:|---:|---:|---:|")
    for profile in REACHABILITY_PROFILES:
        metrics = reachability_matrix.get(profile, {})
        frontend = float(metrics.get("compile_frontend_llvm_avg_ms", 0.0) or 0.0)
        codegen = float(metrics.get("codegen_obj_avg_ms", 0.0) or 0.0)
        link = metrics.get("link_avg_ms")
        link_str = "n/a" if link is None else f"{float(link):.2f}"
        e2e = float(metrics.get("e2e_avg_ms", 0.0) or 0.0)
        print(f"| {profile} | {frontend:.2f} | {codegen:.2f} | {link_str} | {e2e:.2f} |")

    deltas = reachability_matrix.get("delta_vs_all_reachable_ms", {})
    half_delta = float(deltas.get("half_reachable_e2e_delta_ms", 0.0) or 0.0)
    entryless_delta = float(deltas.get("library_entryless_e2e_delta_ms", 0.0) or 0.0)
    print("")
    print(f"Reachability deltas vs all_reachable: half={half_delta:.2f}ms, entryless={entryless_delta:.2f}ms")


def compute_phase_deltas(
    real_incremental: dict[str, dict[str, Any]],
    scale_curve: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    incremental: dict[str, Any] = {}
    for scenario in INCREMENTAL_SCENARIOS:
        sengoo = real_incremental.get(scenario, {}).get("sengoo", {})
        before_avg = float(sengoo.get("before_avg_ms", 0.0) or 0.0)
        after_avg = float(sengoo.get("after_avg_ms", 0.0) or 0.0)
        incremental[scenario] = {
            "before_minus_after_ms": before_avg - after_avg,
            "after_minus_target_ms": after_avg - TARGET_REAL_INCREMENTAL_MS,
        }

    scale: dict[str, Any] = {}
    for loc in [str(x) for x in SCALE_LOC_BUCKETS]:
        sengoo = scale_curve.get(loc, {}).get("sengoo", {})
        frontend = float(sengoo.get("compile_frontend_llvm_avg_ms", 0.0) or 0.0)
        codegen = float(sengoo.get("codegen_obj_avg_ms", 0.0) or 0.0)
        link = float(sengoo.get("link_avg_ms", 0.0) or 0.0)
        e2e = float(sengoo.get("e2e_avg_ms", 0.0) or 0.0)
        scale[loc] = {
            "frontend_share_pct": (frontend / e2e * 100.0) if e2e > 0 else 0.0,
            "codegen_share_pct": (codegen / e2e * 100.0) if e2e > 0 else 0.0,
            "link_share_pct": (link / e2e * 100.0) if e2e > 0 else 0.0,
        }

    scale_100k = scale_curve.get("100000", {}).get("sengoo", {})
    frontend_share_100k = float(scale.get("100000", {}).get("frontend_share_pct", 0.0) or 0.0)
    frontend_share_1000k = float(scale.get("1000000", {}).get("frontend_share_pct", 0.0) or 0.0)
    frontend_share_delta_pp = frontend_share_1000k - frontend_share_100k
    if frontend_share_delta_pp > 0.05:
        frontend_share_trend = "increasing"
    elif frontend_share_delta_pp < -0.05:
        frontend_share_trend = "decreasing"
    else:
        frontend_share_trend = "stable"
    return {
        "incremental_vs_target_ms": incremental,
        "scale_phase_share_pct": scale,
        "frontend_share_trend_100k_to_1000k": {
            "share_100k_pct": frontend_share_100k,
            "share_1000k_pct": frontend_share_1000k,
            "delta_pp": frontend_share_delta_pp,
            "trend": frontend_share_trend,
        },
        "scale_100k_vs_target_ms": {
            "frontend_minus_target_ms": float(scale_100k.get("compile_frontend_llvm_avg_ms", 0.0) or 0.0)
            - TARGET_FRONTEND_100K_MS,
            "codegen_minus_target_ms": float(scale_100k.get("codegen_obj_avg_ms", 0.0) or 0.0)
            - TARGET_CODEGEN_100K_MS,
            "link_minus_target_ms": float(scale_100k.get("link_avg_ms", 0.0) or 0.0) - TARGET_LINK_100K_MS,
            "e2e_minus_target_ms": float(scale_100k.get("e2e_avg_ms", 0.0) or 0.0) - TARGET_FULL_BUILD_100K_MS,
        },
    }


def flatten_phase_snapshot(report: dict[str, Any]) -> dict[str, float]:
    snapshot: dict[str, float] = {}
    real_incremental = report.get("real_incremental", {})
    if isinstance(real_incremental, dict):
        for scenario in INCREMENTAL_SCENARIOS:
            value = (
                real_incremental.get(scenario, {})
                .get("sengoo", {})
                .get("after_avg_ms")
            )
            if isinstance(value, (int, float)):
                snapshot[f"real_incremental/{scenario}/sengoo/after_avg_ms"] = float(value)

    scale_curve = report.get("scale_curve", {})
    if isinstance(scale_curve, dict):
        sengoo_100k = scale_curve.get("100000", {}).get("sengoo", {})
        for key in (
            "compile_frontend_llvm_avg_ms",
            "codegen_obj_avg_ms",
            "link_avg_ms",
            "e2e_avg_ms",
        ):
            value = sengoo_100k.get(key)
            if isinstance(value, (int, float)):
                snapshot[f"scale_curve/100000/sengoo/{key}"] = float(value)
        sengoo_1000k = scale_curve.get("1000000", {}).get("sengoo", {})
        for key in (
            "compile_frontend_llvm_avg_ms",
            "codegen_obj_avg_ms",
            "link_avg_ms",
            "e2e_avg_ms",
        ):
            value = sengoo_1000k.get(key)
            if isinstance(value, (int, float)):
                snapshot[f"scale_curve/1000000/sengoo/{key}"] = float(value)

    reachability_matrix = report.get("reachability_matrix", {})
    if isinstance(reachability_matrix, dict):
        for profile in REACHABILITY_PROFILES:
            metrics = reachability_matrix.get(profile, {})
            if not isinstance(metrics, dict):
                continue
            for key in (
                "compile_frontend_llvm_avg_ms",
                "codegen_obj_avg_ms",
                "e2e_avg_ms",
            ):
                value = metrics.get(key)
                if isinstance(value, (int, float)):
                    snapshot[f"reachability_matrix/{profile}/{key}"] = float(value)

    compile_memory_compare = report.get("compile_memory_compare", {})
    if isinstance(compile_memory_compare, dict):
        for loc in [str(x) for x in MEMORY_LOC_BUCKETS]:
            metrics = compile_memory_compare.get(loc, {}).get("sengoo", {})
            if not isinstance(metrics, dict):
                continue
            for key in ("peak_rss_mb_avg", "peak_private_mb_avg"):
                value = metrics.get(key)
                if isinstance(value, (int, float)):
                    snapshot[f"compile_memory_compare/{loc}/sengoo/{key}"] = float(value)

    return snapshot


def compute_delta_vs_previous(previous_report: dict[str, Any], current_report: dict[str, Any]) -> dict[str, Any]:
    previous = flatten_phase_snapshot(previous_report)
    current = flatten_phase_snapshot(current_report)
    deltas: dict[str, Any] = {}
    for key, current_value in sorted(current.items()):
        if key not in previous:
            continue
        previous_value = previous[key]
        delta_ms = current_value - previous_value
        delta_pct = (delta_ms / previous_value * 100.0) if previous_value != 0 else None
        deltas[key] = {
            "previous": previous_value,
            "current": current_value,
            "delta_ms": delta_ms,
            "delta_pct": delta_pct,
        }
    return deltas


def compute_daemon_comparison(
    oneshot_real_incremental: dict[str, dict[str, Any]],
    daemon_real_incremental: dict[str, dict[str, Any]],
) -> dict[str, Any]:
    out: dict[str, Any] = {}
    for scenario in INCREMENTAL_SCENARIOS:
        oneshot = oneshot_real_incremental.get(scenario, {}).get("sengoo", {})
        daemon = daemon_real_incremental.get(scenario, {}).get("sengoo", {})
        one_after = float(oneshot.get("after_avg_ms", 0.0) or 0.0)
        daemon_after = float(daemon.get("after_avg_ms", 0.0) or 0.0)
        delta_ms = daemon_after - one_after
        delta_pct = (delta_ms / one_after * 100.0) if one_after > 0 else None
        out[scenario] = {
            "oneshot_after_avg_ms": one_after,
            "daemon_after_avg_ms": daemon_after,
            "daemon_minus_oneshot_ms": delta_ms,
            "daemon_minus_oneshot_pct": delta_pct,
        }
    return out


def load_frontend_baseline_profile(bench_root: Path) -> dict[str, Any] | None:
    path = bench_root / FRONTEND_BASELINE_PROFILE
    if not path.exists():
        return None
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError:
        return None


def baseline_metric(
    baseline_profile: dict[str, Any],
    bucket: str,
    metric_key: str,
) -> float | None:
    metrics = baseline_profile.get("metrics")
    if not isinstance(metrics, dict):
        return None
    bucket_metrics = metrics.get(bucket)
    if not isinstance(bucket_metrics, dict):
        return None
    value = bucket_metrics.get(metric_key)
    if not isinstance(value, (int, float)):
        return None
    return float(value)


def compute_rollback_evidence(
    report: dict[str, Any],
    baseline_profile: dict[str, Any] | None,
) -> dict[str, Any]:
    evidence: dict[str, Any] = {
        "schema_version": 1,
        "baseline_report_id": None,
        "baseline_report_path": None,
        "thresholds": {
            "frontend_regression_pct": {
                "100000": ROLLBACK_MAX_FRONTEND_100K_REGRESSION_PCT,
                "1000000": ROLLBACK_MAX_FRONTEND_1000K_REGRESSION_PCT,
            },
            "rss_regression_pct": {
                "100000": ROLLBACK_MAX_RSS_100K_REGRESSION_PCT,
                "1000000": ROLLBACK_MAX_RSS_1000K_REGRESSION_PCT,
            },
        },
        "comparisons": [],
        "gate_decision": "pass",
        "reasons": [],
    }

    if not isinstance(baseline_profile, dict):
        evidence["gate_decision"] = "insufficient-data"
        evidence["reasons"].append(f"missing baseline profile ({FRONTEND_BASELINE_PROFILE})")
        return evidence

    evidence["baseline_report_id"] = baseline_profile.get("baseline_report_id")
    evidence["baseline_report_path"] = baseline_profile.get("baseline_report_path")

    scale_curve = report.get("scale_curve")
    compile_memory_compare = report.get("compile_memory_compare")
    memory_available = isinstance(compile_memory_compare, dict)
    if not memory_available:
        evidence["gate_decision"] = "insufficient-data"
        evidence["reasons"].append("compile_memory_compare block missing")

    for bucket in ("100000", "1000000"):
        sengoo_scale = (
            scale_curve.get(bucket, {}).get("sengoo", {})
            if isinstance(scale_curve, dict)
            else {}
        )
        measured_frontend = sengoo_scale.get("compile_frontend_llvm_avg_ms")
        baseline_frontend = baseline_metric(
            baseline_profile,
            bucket,
            "compile_frontend_llvm_avg_ms",
        )
        if isinstance(measured_frontend, (int, float)) and baseline_frontend and baseline_frontend > 0:
            threshold = (
                ROLLBACK_MAX_FRONTEND_100K_REGRESSION_PCT
                if bucket == "100000"
                else ROLLBACK_MAX_FRONTEND_1000K_REGRESSION_PCT
            )
            delta_pct = ((float(measured_frontend) - baseline_frontend) / baseline_frontend) * 100.0
            passed = delta_pct <= threshold
            evidence["comparisons"].append(
                {
                    "bucket": bucket,
                    "metric": "compile_frontend_llvm_avg_ms",
                    "measured": float(measured_frontend),
                    "baseline": float(baseline_frontend),
                    "delta_pct": float(delta_pct),
                    "max_regression_pct": float(threshold),
                    "pass": passed,
                }
            )
            if not passed:
                evidence["reasons"].append(
                    f"frontend_time/{bucket} regression {delta_pct:+.2f}% exceeds {threshold:.2f}%"
                )
        else:
            evidence["reasons"].append(f"frontend_time/{bucket} missing measured/baseline value")

        if memory_available:
            sengoo_mem = compile_memory_compare.get(bucket, {}).get("sengoo", {})
            measured_rss = sengoo_mem.get("peak_rss_mb_avg")
            baseline_rss = baseline_metric(baseline_profile, bucket, "peak_rss_mb_avg")
            if isinstance(measured_rss, (int, float)) and baseline_rss and baseline_rss > 0:
                threshold = (
                    ROLLBACK_MAX_RSS_100K_REGRESSION_PCT
                    if bucket == "100000"
                    else ROLLBACK_MAX_RSS_1000K_REGRESSION_PCT
                )
                delta_pct = ((float(measured_rss) - baseline_rss) / baseline_rss) * 100.0
                passed = delta_pct <= threshold
                evidence["comparisons"].append(
                    {
                        "bucket": bucket,
                        "metric": "peak_rss_mb_avg",
                        "measured": float(measured_rss),
                        "baseline": float(baseline_rss),
                        "delta_pct": float(delta_pct),
                        "max_regression_pct": float(threshold),
                        "pass": passed,
                    }
                )
                if not passed:
                    evidence["reasons"].append(
                        f"frontend_rss/{bucket} regression {delta_pct:+.2f}% exceeds {threshold:.2f}%"
                    )
            else:
                evidence["reasons"].append(f"frontend_rss/{bucket} missing measured/baseline value")

    if evidence["gate_decision"] == "insufficient-data":
        return evidence

    if evidence["reasons"]:
        evidence["gate_decision"] = "fail"
    return evidence


def print_phase_delta_summary(phase_deltas: dict[str, Any]) -> None:
    print("")
    print("Phase Deltas (Sengoo)")
    print("| Metric | Delta (ms) |")
    print("|---|---:|")
    scale_100k = phase_deltas.get("scale_100k_vs_target_ms", {})
    for key in (
        "frontend_minus_target_ms",
        "codegen_minus_target_ms",
        "link_minus_target_ms",
        "e2e_minus_target_ms",
    ):
        value = float(scale_100k.get(key, 0.0) or 0.0)
        print(f"| 100k {key} | {value:.2f} |")
    trend = phase_deltas.get("frontend_share_trend_100k_to_1000k", {})
    if isinstance(trend, dict):
        delta_pp = float(trend.get("delta_pp", 0.0) or 0.0)
        share_100k = float(trend.get("share_100k_pct", 0.0) or 0.0)
        share_1000k = float(trend.get("share_1000k_pct", 0.0) or 0.0)
        trend_label = str(trend.get("trend", "unknown"))
        print(
            f"| frontend_share_100k_to_1000k | {delta_pp:.2f} |"
        )
        print(
            f"frontend_share trend detail: 100k={share_100k:.2f}% 1000k={share_1000k:.2f}% ({trend_label})"
        )


def print_daemon_comparison(daemon_comparison: dict[str, Any]) -> None:
    print("")
    print("Daemon vs One-shot (Sengoo Real Incremental)")
    print("| Scenario | One-shot after (ms) | Daemon after (ms) | Delta (ms) |")
    print("|---|---:|---:|---:|")
    for scenario in INCREMENTAL_SCENARIOS:
        metrics = daemon_comparison.get(scenario, {})
        print(
            "| "
            f"{scenario} | "
            f"{float(metrics.get('oneshot_after_avg_ms', 0.0)):.2f} | "
            f"{float(metrics.get('daemon_after_avg_ms', 0.0)):.2f} | "
            f"{float(metrics.get('daemon_minus_oneshot_ms', 0.0)):.2f} |"
        )


def print_rollback_evidence(rollback_evidence: dict[str, Any]) -> None:
    print("")
    print("Frontend Rollback Evidence")
    print(
        f"decision={rollback_evidence.get('gate_decision', 'unknown')} "
        f"baseline={rollback_evidence.get('baseline_report_id', 'n/a')}"
    )
    print("| Metric | Bucket | Measured | Baseline | Delta % | Limit % | Pass |")
    print("|---|---:|---:|---:|---:|---:|---:|")
    for item in rollback_evidence.get("comparisons", []):
        if not isinstance(item, dict):
            continue
        print(
            "| "
            f"{item.get('metric', 'unknown')} | "
            f"{item.get('bucket', 'n/a')} | "
            f"{float(item.get('measured', 0.0)):.2f} | "
            f"{float(item.get('baseline', 0.0)):.2f} | "
            f"{float(item.get('delta_pct', 0.0)):+.2f} | "
            f"{float(item.get('max_regression_pct', 0.0)):.2f} | "
            f"{'yes' if bool(item.get('pass')) else 'no'} |"
        )
    reasons = rollback_evidence.get("reasons", [])
    if isinstance(reasons, list) and reasons:
        print("rollback reasons:")
        for reason in reasons:
            print(f"- {reason}")


def main() -> int:
    args = parse_args()
    bench_root = Path(__file__).resolve().parent
    baseline_profile = load_frontend_baseline_profile(bench_root)
    sengoo_root = resolve_sengoo_root(bench_root)
    results_dir = bench_root / "results"
    results_dir.mkdir(parents=True, exist_ok=True)
    previous_report_path = latest_advanced_report_path(results_dir)

    sgc_bin = resolve_sgc_binary(sengoo_root)
    clangpp = require_tool("clang++")
    cargo = require_tool("cargo")
    py = sys.executable
    if args.daemon_compare and not supports_daemon_subcommand(sgc_bin):
        raise RuntimeError(
            f"selected sgc binary does not support daemon mode: {sgc_bin} (rebuild sgc)"
        )

    real_incremental = measure_real_incremental(bench_root, sgc_bin, clangpp, cargo, py, sengoo_root)
    scale_curve = measure_scale_curve(bench_root, sgc_bin, clangpp, cargo, py, sengoo_root)
    compile_memory_compare: dict[str, Any] | None = None
    if not args.skip_memory_compare:
        compile_memory_compare = measure_compile_memory_compare(
            bench_root,
            sgc_bin,
            clangpp,
            py,
            sengoo_root,
        )
    reachability_matrix = measure_reachability_matrix(bench_root, sgc_bin, clangpp, sengoo_root)
    phase_deltas = compute_phase_deltas(real_incremental, scale_curve)

    daemon_comparison: dict[str, Any] | None = None
    if args.daemon_compare:
        daemon_proc: subprocess.Popen[Any] | None = None
        try:
            daemon_proc = start_sgc_daemon(sgc_bin, sengoo_root, args.daemon_addr)
            daemon_incremental = measure_real_incremental(
                bench_root,
                sgc_bin,
                clangpp,
                cargo,
                py,
                sengoo_root,
                include_languages=("sengoo",),
                sengoo_daemon_addr=args.daemon_addr,
            )
            daemon_comparison = compute_daemon_comparison(real_incremental, daemon_incremental)
        finally:
            stop_sgc_daemon(daemon_proc)

    report: dict[str, Any] = {
        "schema_version": 2,
        "generated_at_unix_ms": now_unix_ms(),
        "config": {
            "incremental_iterations": INCREMENTAL_ITERS,
            "scale_loc_buckets": SCALE_LOC_BUCKETS,
            "scale_iterations_by_loc": SCALE_ITERS_BY_LOC,
            "memory_loc_buckets": MEMORY_LOC_BUCKETS,
            "memory_iters_by_loc": MEMORY_ITERS_BY_LOC,
            "reachability_loc": REACHABILITY_LOC,
            "reachability_iters": REACHABILITY_ITERS,
            "reachability_profiles": REACHABILITY_PROFILES,
        },
        "fairness": {
            "cpp": "precompiled header (PCH) enabled",
            "rust": "cargo incremental enabled (CARGO_INCREMENTAL=1)",
            "memory_compare_rust": "direct rustc compile-to-object when rustup toolchain path is available",
        },
        "real_incremental": real_incremental,
        "scale_curve": scale_curve,
        "reachability_matrix": reachability_matrix,
        "phase_deltas": phase_deltas,
        "notes": [
            "Scale curve e2e includes link time for compiled languages.",
            "Sengoo uses split timing: front-end LLVM emit + clang IR codegen + clang link.",
            "Rust uses cargo build e2e timing; split link-only timing is not isolated.",
            "Compile-memory comparison tracks peak process RSS per compiler command.",
            "Reachability matrix validates optimization generality across all/half/entryless profiles.",
        ],
    }
    if compile_memory_compare is not None:
        report["compile_memory_compare"] = compile_memory_compare
    if daemon_comparison is not None:
        report["daemon_comparison"] = daemon_comparison
    if previous_report_path and previous_report_path.exists():
        previous_report = json.loads(previous_report_path.read_text(encoding="utf-8"))
        report["phase_deltas"]["delta_vs_previous"] = compute_delta_vs_previous(
            previous_report,
            report,
        )
        report["phase_deltas"]["previous_report"] = str(previous_report_path)
    report["rollback_evidence"] = compute_rollback_evidence(report, baseline_profile)
    report["rollback_evidence"]["baseline_profile_path"] = str(
        (bench_root / FRONTEND_BASELINE_PROFILE).resolve()
    )

    out_path = results_dir / f"{now_unix_ms()}-advanced-pipeline.json"
    out_path.write_text(json.dumps(report, indent=2), encoding="utf-8", newline="\n")

    print(f"Advanced bench report: {out_path}")
    print_incremental_tables(real_incremental)
    print_scale_tables(scale_curve)
    if compile_memory_compare is not None:
        print_compile_memory_compare(compile_memory_compare)
    print_reachability_matrix(reachability_matrix)
    print_phase_delta_summary(phase_deltas)
    if daemon_comparison is not None:
        print_daemon_comparison(daemon_comparison)
    print_rollback_evidence(report.get("rollback_evidence", {}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())

