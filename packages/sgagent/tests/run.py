"""Frozen copied-package core and actual maintenance-host acceptance.

The SQLite store's adversarial/multiprocess gate is a separate driver owned by
the ledger implementation; this runner exercises it through the real sample.
"""
from __future__ import annotations
import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import time


def clean(root):
    root = root.resolve(); paths = list(root.rglob("*")); assert all(p.resolve().is_relative_to(root) for p in paths)
    for p in paths:
        if p.is_file() or p.is_symlink(): p.unlink()
    for p in sorted((p for p in paths if p.is_dir()), key=lambda p: len(p.parts), reverse=True): p.rmdir()
    root.rmdir()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def package_sources(language, names):
    sources = {}
    for name in names:
        directory = language / "packages" / name
        for path in directory.rglob("*"):
            if path.is_file() and not any(part in path.relative_to(directory).parts
                    for part in ("target", "build", "__pycache__")):
                sources[str(path.relative_to(language / "packages"))] = digest(path)
    return sources


def runtime_sources(language):
    directory = language / "tools/stdlib"
    return {str(path.relative_to(directory)): digest(path) for path in directory.rglob("*")
        if path.is_file() and path.suffix in (".sg", ".c", ".h")
        and not any(part in path.relative_to(directory).parts
            for part in ("tests", "target", "build", "__pycache__"))}


def main():
    parser = argparse.ArgumentParser(); parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--sengoo", type=Path, required=True); parser.add_argument("--clang", type=Path, required=True)
    parser.add_argument("--formatter", type=Path, required=True); parser.add_argument("--sgpm", type=Path)
    parser.add_argument("--tests", nargs="*", choices=("scheduler", "tool_schedule", "request_retry", "kernel", "budgets", "guards"),
        default=("scheduler", "tool_schedule", "request_retry", "kernel", "budgets", "guards"))
    parser.add_argument("--evidence", type=Path, required=True); args = parser.parse_args()
    package = Path(__file__).resolve().parents[1]; language = args.sengoo.resolve()
    names = ("sgagent", "sgschema", "sgsqlite", "sgllm")
    sources = package_sources(language, names)
    runtime = runtime_sources(language)
    tools = {name: {"path": str(path.resolve()), "sha256": digest(path)}
        for name, path in (("compiler", args.compiler), ("formatter", args.formatter),
            ("clang", args.clang), ("sgpm", args.sgpm)) if path}
    scratch = Path(tempfile.mkdtemp(prefix="sgagent-external-世界-")); libraries = scratch / "packages"
    for name in sources:
        target = libraries / name; target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(language / "packages" / name, target)
    environment = dict(os.environ); environment.update(SENGOO_CLANG=str(args.clang.resolve()),
        SENGOO_STDLIB=str(language / "tools/stdlib"), SENGOO_RUNTIME=str(language / "tools/stdlib/runtime.c"),
        SENGOO_MODULE_MAP=";".join(name + "=" + str(libraries / name / "src/lib.sg") for name in names))
    environment["PATH"] = str(args.clang.resolve().parent) + os.pathsep + environment.get("PATH", "")
    report = {"schema": 1, "result": "running", "host": os.environ.get("COMPUTERNAME"),
        "scope": "outside-checkout copied packages; native core and real maintenance file/command/SQLite with loopback model",
        "compiler_sha256": hashlib.sha256(args.compiler.read_bytes()).hexdigest(),
        "formatter_sha256": hashlib.sha256(args.formatter.read_bytes()).hexdigest(),
        "tools": tools, "sources": sources, "runtime_sources": runtime,
        "selected_native_tests": args.tests, "steps": []}

    def run(name, command, cwd=libraries / "sgagent", timeout=300):
        began = time.monotonic(); result = subprocess.run(list(map(str, command)), cwd=cwd, env=environment,
            encoding="utf-8", errors="replace", capture_output=True, timeout=timeout)
        step = {"name": name, "exit_code": result.returncode, "stdout": result.stdout,
            "stderr": result.stderr, "seconds": time.monotonic() - began}; report["steps"].append(step)
        print(name + ": " + str(result.returncode), flush=True); assert result.returncode == 0, step

    try:
        for p in (libraries / "sgagent").rglob("*.sg"):
            run("format-" + str(p.relative_to(libraries / "sgagent")), [args.formatter.resolve(), "--check", p])
        output = scratch / "bin"; output.mkdir()
        for name in args.tests:
            binary = output / (name + (".exe" if os.name == "nt" else ""))
            run(name + "-build", [args.compiler.resolve(), "--runtime-mode", "source-development", "build",
                libraries / f"sgagent/tests/{name}.sg", "--force-rebuild", "-O", "0", "-o", binary])
            run(name + "-native", [binary], timeout=60)
        consumer = scratch / "maintenance"; (consumer / "src").mkdir(parents=True)
        shutil.copyfile(libraries / "sgagent/examples/maintenance.sg", consumer / "src/main.sg")
        (consumer / "Sengoo.toml").write_text('[package]\nname="maintenance_agent"\nversion="0.1.0"\nedition="2026"\n[bin]\npath="src/main.sg"\n[dependencies]\nsgagent={path="../packages/sgagent"}\nsgllm={path="../packages/sgllm"}\n', encoding="utf-8")
        binary = output / ("maintenance" + (".exe" if os.name == "nt" else ""))
        if args.sgpm:
            environment["SGPM_SGC"] = str(args.compiler.resolve()); environment.pop("SENGOO_MODULE_MAP", None)
            run("maintenance-update", [args.sgpm.resolve(), "update"], cwd=consumer)
            before = (consumer / "Sengoo.lock").read_bytes()
            run("maintenance-check-locked", [args.sgpm.resolve(), "--runtime-mode", "source-development", "check", "--locked"], cwd=consumer)
            # Explicit output is compiled with sgc and the exact package graph
            # after sgpm independently resolved and checked the locked consumer.
            environment["SENGOO_MODULE_MAP"] = ";".join(name + "=" + str(libraries / name / "src/lib.sg") for name in names)
            run("maintenance-build", [args.compiler.resolve(), "--runtime-mode", "source-development", "build", consumer / "src/main.sg", "--force-rebuild", "-O", "0", "-o", binary])
            assert before == (consumer / "Sengoo.lock").read_bytes()
        else:
            run("maintenance-build", [args.compiler.resolve(), "--runtime-mode", "source-development", "build", consumer / "src/main.sg", "--force-rebuild", "-O", "0", "-o", binary])
        # Runtime proof retains only the binary: no package source or compiler
        # directories are passed to its environment. This removes copied files
        # individually; native ABI components are linked into the executable.
        spec = importlib.util.spec_from_file_location("sgagent_maintenance_fixture", package / "tests/maintenance_fixture.py")
        fixture = importlib.util.module_from_spec(spec); spec.loader.exec_module(fixture)
        clean(libraries); clean(consumer)
        normal = fixture.exercise(binary); crash = fixture.exercise(binary, crash=True)
        delayed_tree = fixture.exercise(binary, delayed_tree=True)
        timeout_tree = fixture.exercise(binary, timeout_tree=True)
        report["steps"].append({"name": "maintenance-runtime-and-crash", "exit_code": 0,
            "binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "normal": normal, "crash": crash,
            "delayed_tree": delayed_tree, "timeout_tree": timeout_tree})
        assert sources == package_sources(language, names), "package source set changed during acceptance"
        assert runtime == runtime_sources(language), "runtime source set changed during acceptance"
        assert all(info["sha256"] == digest(Path(info["path"])) for info in tools.values()), "selected tool changed during acceptance"
        report["result"] = "passed"
    except BaseException as error:
        report["result"] = "failed"; report["error"] = repr(error); raise
    finally:
        args.evidence.parent.mkdir(parents=True, exist_ok=True); args.evidence.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        clean(scratch)


if __name__ == "__main__": main()
