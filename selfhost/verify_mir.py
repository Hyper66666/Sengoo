"""Verify the partial typed HIR/MIR and ownership contract checkpoint.

This is a development-seed correctness gate. It does not qualify performance,
complete M3, release the compiler, or demonstrate native self-compilation.
"""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
MODULES = ("core", "types", "lexer", "parser", "semantic_base", "semantic",
           "borrow", "hir", "mir", "mir_validate", "mir_lower")
SEED_SHA = "9b1055c5a5cb7f09c016228a44f7427b7519b6a92e69cad5993004831c3aa1e5"
CLANG_SHA = "56ef1abd0df11a3b1b77822193e927c6345078c24511567ae10b3afd803d03a3"


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def revision() -> str:
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()


def inputs() -> dict[str, str]:
    paths = {Path(__file__).resolve(), ROOT / "Cargo.toml", ROOT / "Cargo.lock",
             ROOT / "rust-toolchain.toml", ROOT / "selfhost/tests/borrow.sg", ROOT / "selfhost/tests/mir.sg"}
    # Guard the complete declared frontend import closure. Sibling native backend
    # files are independent jobs and are not inputs to these two test programs.
    for name in MODULES:
        path = ROOT / "selfhost/src" / (name + ".sg")
        paths.add(path)
        for dependency in re.findall(r"^\s*import\s+selfhost_([a-z0-9_]+)\s*;", path.read_text(encoding="utf-8"), re.MULTILINE):
            if dependency not in MODULES:
                raise RuntimeError("missing declared frontend dependency: " + dependency)
    for directory in ("tools/stdlib", "runtime"):
        for path in (ROOT / directory).rglob("*"):
            if path.is_file() and not any(part in {"target", "build", "__pycache__"} for part in path.relative_to(ROOT / directory).parts):
                paths.add(path)
    return {path.relative_to(ROOT).as_posix(): sha(path) for path in sorted(paths)}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--seed", type=Path, default=ROOT / "target/agent-conditional-compiler/bin/sgc.exe")
    parser.add_argument("--clang", type=Path, default=Path("C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe"))
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m3-mir-contracts")
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    tools = {args.seed.resolve(): SEED_SHA, args.clang.resolve(): CLANG_SHA}
    report: dict = {
        "schema": 1, "stage": "m3-hir-mir-ownership-contracts", "result": "running",
        "m3_complete": False, "hir_schema": 5, "mir_schema": 1, "runtime_abi": 1,
        "target": "x86_64-pc-windows-msvc", "optimization": 0,
        "started_at": dt.datetime.now(dt.timezone.utc).isoformat(),
        "host": {"system": platform.system(), "release": platform.release(), "machine": platform.machine(), "python": platform.python_version(), "python_executable_sha256": sha(Path(sys.executable))},
        "qualification": "partial correctness checkpoint; verified development seed; no performance or self-hosting claim",
        "tests": {"ownership_resource_rounds": 50, "hir_roundtrip_poison_observed_abi_and_lowering_rounds": 10,
                  "raw_mir_cfg_ssa_abi_bounds_rounds": 30, "exact_resources": ["String", "Buffer", "opaque"]},
        "remaining": ["complete ownership-aware generic monomorphization; generic owned arguments and generic foreign declarations reject 5009", "owned patterns and variant-aware cleanup", "complete inherited/dynamic methods and pattern/slice iteration",
                      "complete try propagation with inactive reference/dyn/Future payloads; closures/contracts/async/concurrency/cancellation", "resolved runtime resource drop ABI/glue", "conditional partial-field flags", "full frozen supported-corpus parity"],
        "steps": [],
    }
    before: dict[str, str] = {}

    def save() -> None:
        (output / "evidence.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    save()  # Invalidate a previous passed result before any identity/input read.
    try:
        report["source_revision"] = revision()
        for path, expected in tools.items():
            if sha(path) != expected:
                raise RuntimeError("tool identity mismatch: " + str(path))
        report["tools"] = {str(path): {"sha256": sha(path), "version": subprocess.check_output([str(path), "--version"], text=True, stderr=subprocess.STDOUT, timeout=30).strip()} for path in tools}
        before = inputs()
        report["inputs_before"] = before
        env = os.environ.copy()
        env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(args.clang.resolve()),
                   SENGOO_MODULE_MAP=";".join("selfhost_" + name + "=" + str(ROOT / "selfhost/src" / (name + ".sg")) for name in MODULES))
        env["PATH"] = str(args.clang.resolve().parent) + os.pathsep + env.get("PATH", "")
        jobs = [
            ("borrow-build", [str(args.seed.resolve()), "--runtime-mode", "source-development", "build", "selfhost/tests/borrow.sg", "--force-rebuild", "-O", "0", "-o", str(output / "borrow-tests.exe")], None),
            ("borrow-native", [str(output / "borrow-tests.exe")], "selfhost ownership: ok"),
            ("mir-build", [str(args.seed.resolve()), "--runtime-mode", "source-development", "build", "selfhost/tests/mir.sg", "--force-rebuild", "-O", "0", "-o", str(output / "mir-tests.exe")], None),
            ("mir-native", [str(output / "mir-tests.exe")], "selfhost typed HIR/MIR: 10 roundtrip/lowering/cleanup and 30 CFG/SSA/ABI exact resource rounds passed"),
        ]
        for name, command, marker in jobs:
            started = time.perf_counter()
            process = subprocess.run(command, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, timeout=900)
            log = output / (name + ".log")
            log.write_bytes(process.stdout)
            report["steps"].append({"name": name, "command": command, "exit_code": process.returncode, "elapsed_s": time.perf_counter() - started,
                                    "log": str(log.relative_to(ROOT)) if log.is_relative_to(ROOT) else str(log), "log_sha256": sha(log)})
            save()
            print(name, process.returncode, flush=True)
            if process.returncode:
                raise RuntimeError(name + " failed")
            if marker is not None and process.stdout.decode("utf-8", "strict").strip() != marker:
                raise RuntimeError(name + " success marker mismatch")
        report["artifacts"] = {name: sha(output / name) for name in ("borrow-tests.exe", "mir-tests.exe")}
        report["result"] = "passed"
    except BaseException as error:
        report["result"] = "failed"
        report["failure"] = type(error).__name__ + ": " + str(error)
    finally:
        try:
            after = inputs()
            report["inputs_after"] = after
            report["source_changes"] = sorted(path for path in before.keys() | after.keys() if before.get(path) != after.get(path))
            report["tool_changes"] = [str(path) for path, expected in tools.items() if sha(path) != expected]
            report["source_revision_after"] = revision()
            if report["source_changes"] or report["tool_changes"]:
                report["result"] = "failed"
                report["source_guard_failure"] = "inputs changed during contract gate"
        except BaseException as error:
            report["result"] = "failed"
            report["finally_failure"] = type(error).__name__ + ": " + str(error)
        report["finished_at"] = dt.datetime.now(dt.timezone.utc).isoformat()
        save()
    print(json.dumps({key: report.get(key) for key in ("result", "failure", "source_changes", "tool_changes")}))
    return 0 if report["result"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
