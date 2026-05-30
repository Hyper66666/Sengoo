#!/usr/bin/env python3
from __future__ import annotations

import ctypes
import json
import os
import platform
import shutil
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


CALL_PLAN = [
    ("rule_fast_track", 888),
    ("rule_review", 120000),
    ("rule_block", 250000),
]


def exe_name(stem: str) -> str:
    return f"{stem}.exe" if sys.platform.startswith("win") else stem


def resolve_sengoo_root(demo_dir: Path) -> Path:
    bench_root = demo_dir.parents[1]
    env_root = os.environ.get("SENGOO_ROOT")

    candidates = []
    if env_root:
        candidates.append(Path(env_root).expanduser().resolve())
    candidates.append(bench_root.parent.resolve())
    candidates.append((bench_root.parent / "Sengoo").resolve())

    for candidate in candidates:
        if (candidate / "Cargo.toml").exists() and (candidate / "tools" / "sgc" / "src" / "main.rs").exists():
            return candidate

    raise RuntimeError(
        "cannot resolve Sengoo root; set SENGOO_ROOT or place bench next to Sengoo directory"
    )


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


def count_loc(path: Path) -> int:
    count = 0
    for raw in path.read_text(encoding="utf-8").splitlines():
        line = raw.strip()
        if not line:
            continue
        if line.startswith("//"):
            continue
        count += 1
    return count


def parse_json_line(stdout: str) -> dict[str, Any]:
    for line in reversed(stdout.splitlines()):
        line = line.strip()
        if not line:
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            return value
    raise RuntimeError(f"failed to parse JSON from output:\n{stdout}")


def suffix_for_shared_library() -> str:
    if sys.platform.startswith("win"):
        return ".dll"
    if sys.platform == "darwin":
        return ".dylib"
    return ".so"


def compile_sengoo_and_load_symbols(demo_dir: Path, repo_root: Path, build_dir: Path) -> tuple[dict[str, Any], str, Path]:
    sgc_candidates = [
        repo_root / "target" / "release" / exe_name("sgc"),
        repo_root / "target" / "debug" / exe_name("sgc"),
    ]
    sgc = next((p for p in sgc_candidates if p.exists()), None)
    if sgc is None:
        raise RuntimeError("sgc binary not found; run `cargo build -p sgc --release` first")

    rules_sg = demo_dir / "rules.sg"
    rules_exe = build_dir / exe_name("rules")
    build_cmd = [
        str(sgc),
        "build",
        str(rules_sg),
        "-O",
        "2",
        "--force-rebuild",
        "-o",
        str(rules_exe),
    ]
    proc = run_checked(build_cmd, cwd=repo_root)
    auto_enabled = "reflection: auto enabled" in proc.stdout
    if not auto_enabled:
        raise RuntimeError(
            "auto reflection did not enable; expected build output to contain `reflection: auto enabled`"
        )

    sidecar = Path(str(rules_exe) + ".sgreflect.json")
    if not sidecar.exists():
        raise RuntimeError(f"reflection sidecar missing: {sidecar}")

    payload = json.loads(sidecar.read_text(encoding="utf-8"))
    modules = payload.get("modules", [])
    if not modules:
        raise RuntimeError("reflection sidecar has no modules")
    symbols = modules[0].get("symbols", [])
    if not symbols:
        raise RuntimeError("reflection sidecar has no symbols")

    module_id = modules[0]["module_id"]
    return {
        "auto_enabled": auto_enabled,
        "module_id": module_id,
        "symbols": symbols,
        "build_stdout": proc.stdout,
    }, module_id, rules_exe


def build_shared_for_reflection(
    repo_root: Path,
    rules_exe: Path,
    symbols: list[dict[str, Any]],
) -> Path:
    clang = shutil.which("clang")
    if not clang:
        raise RuntimeError("clang not found in PATH")

    llvm_ir = rules_exe.with_suffix(".ll")
    if not llvm_ir.exists():
        raise RuntimeError(f"LLVM IR missing: {llvm_ir}")

    runtime_c = repo_root / "tools" / "stdlib" / "runtime.c"
    if not runtime_c.exists():
        raise RuntimeError(f"runtime source missing: {runtime_c}")

    out_dir = rules_exe.parent
    ll_obj = out_dir / "rules_reflect.obj"
    rt_obj = out_dir / "runtime_reflect.obj"
    shared = out_dir / f"rules.sgreflect{suffix_for_shared_library()}"

    run_checked([clang, "-O2", "-c", "-x", "ir", str(llvm_ir), "-o", str(ll_obj)], cwd=repo_root)
    run_checked([clang, "-O2", "-c", str(runtime_c), "-o", str(rt_obj)], cwd=repo_root)

    native_symbols = []
    for symbol in symbols:
        native = symbol.get("native_symbol")
        if isinstance(native, str) and native:
            native_symbols.append(native)

    link_cmd = [clang, "-shared", str(ll_obj), str(rt_obj), "-o", str(shared)]
    if sys.platform.startswith("win"):
        for name in native_symbols:
            link_cmd.append(f"-Wl,/EXPORT:{name}")
    run_checked(link_cmd, cwd=repo_root)

    if not shared.exists():
        raise RuntimeError(f"failed to create shared library: {shared}")
    return shared


def invoke_sengoo_reflection(shared: Path, symbols: list[dict[str, Any]]) -> dict[str, Any]:
    lib = ctypes.CDLL(str(shared))

    native_map = {}
    for symbol in symbols:
        raw = str(symbol.get("symbol", ""))
        short = raw.rsplit("::", 1)[-1]
        native = symbol.get("native_symbol")
        if isinstance(native, str) and native:
            native_map[short] = native

    missing = 0
    sum_value = 0
    invoked = 0
    for name, arg in CALL_PLAN:
        native = native_map.get(name)
        if not native:
            missing += 1
            continue
        try:
            fn = getattr(lib, native)
        except AttributeError:
            missing += 1
            continue
        fn.argtypes = [ctypes.c_longlong]
        fn.restype = ctypes.c_longlong
        sum_value += int(fn(ctypes.c_longlong(arg)))
        invoked += 1

    return {
        "requested_rules": len(CALL_PLAN),
        "discovered_rules": len(native_map),
        "invoked_rules": invoked,
        "missing_rules": missing,
        "decision_sum": sum_value,
    }


def run_cpp_manual(demo_dir: Path, repo_root: Path, build_dir: Path) -> dict[str, Any]:
    clangxx = shutil.which("clang++")
    if not clangxx:
        raise RuntimeError("clang++ not found in PATH")

    src = demo_dir / "manual_reflection.cpp"
    exe = build_dir / exe_name("cpp_manual_reflection")
    run_checked([clangxx, "-std=c++20", "-O2", str(src), "-o", str(exe)], cwd=repo_root)
    proc = run_checked([str(exe)], cwd=repo_root)
    return parse_json_line(proc.stdout)


def main() -> int:
    demo_dir = Path(__file__).resolve().parent
    repo_root = resolve_sengoo_root(demo_dir)
    build_dir = demo_dir / "build"
    build_dir.mkdir(parents=True, exist_ok=True)

    reflected, _, rules_exe = compile_sengoo_and_load_symbols(demo_dir, repo_root, build_dir)
    symbols = reflected["symbols"]
    shared = build_shared_for_reflection(repo_root, rules_exe, symbols)
    sengoo_result = invoke_sengoo_reflection(shared, symbols)
    cpp_result = run_cpp_manual(demo_dir, repo_root, build_dir)

    sengoo_loc = count_loc(demo_dir / "rules.sg")
    cpp_loc = count_loc(demo_dir / "manual_reflection.cpp")

    summary = {
        "scenario": "auto-reflection-vs-cpp-manual-registry",
        "generated_at_unix_ms": int(time.time() * 1000),
        "platform": {
            "system": platform.system(),
            "release": platform.release(),
            "python": sys.version.split()[0],
        },
        "sengoo": {
            "auto_enabled": reflected["auto_enabled"],
            "loc_rules_file": sengoo_loc,
            "requested_rules": sengoo_result["requested_rules"],
            "discovered_rules": sengoo_result["discovered_rules"],
            "invoked_rules": sengoo_result["invoked_rules"],
            "missing_rules": sengoo_result["missing_rules"],
            "decision_sum": sengoo_result["decision_sum"],
            "reflection_sidecar": str(Path(str(rules_exe) + ".sgreflect.json")),
            "reflection_library": str(shared),
        },
        "cpp": {
            "loc_manual_file": cpp_loc,
            "declared_rules": cpp_result.get("declared_rules"),
            "registered_rules": cpp_result.get("registered_rules"),
            "requested_rules": cpp_result.get("requested_rules"),
            "missing_rules": cpp_result.get("missing_rules"),
            "decision_sum": cpp_result.get("decision_sum"),
        },
        "contrast": {
            "sengoo_manual_registry_entries": 0,
            "cpp_manual_registry_entries": int(cpp_result.get("registered_rules", 0)),
            "loc_ratio_cpp_vs_sengoo": (cpp_loc / sengoo_loc) if sengoo_loc > 0 else None,
        },
    }

    results_dir = demo_dir / "results"
    results_dir.mkdir(parents=True, exist_ok=True)
    out = results_dir / f"{int(time.time() * 1000)}-reflection-auto-vs-cpp.json"
    out.write_text(json.dumps(summary, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    print("Reflection Ergonomics Demo: Sengoo Auto vs C++ Manual Registry")
    print("| Metric | Sengoo | C++ |")
    print("|---|---:|---:|")
    print(f"| Rule file LOC | {sengoo_loc} | {cpp_loc} |")
    print(f"| Manual registry entries | 0 | {int(cpp_result.get('registered_rules', 0))} |")
    print(f"| Requested dynamic rules | {sengoo_result['requested_rules']} | {int(cpp_result.get('requested_rules', 0))} |")
    print(f"| Missing dynamic rules | {sengoo_result['missing_rules']} | {int(cpp_result.get('missing_rules', 0))} |")
    print(f"| Decision sum | {sengoo_result['decision_sum']} | {int(cpp_result.get('decision_sum', 0))} |")
    print("")
    print(f"Sengoo auto reflection enabled: {reflected['auto_enabled']}")
    print(f"Report: {out}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
