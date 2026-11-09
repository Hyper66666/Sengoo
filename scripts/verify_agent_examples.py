"""Build the three real consumers from the checksummed offline SDK registry."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

from package_ecosystem import unpack, verify, bundle_name


def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()


def runtime_identity(repo):
    roots = (repo / "runtime", repo / "tools/stdlib")
    return {str(path.relative_to(repo)): sha(path) for root in roots for path in root.rglob("*")
        if path.is_file() and path.suffix in (".rs", ".sg", ".c", ".h", ".toml")
        and not any(part in ("target", "build", "tests", "__pycache__") for part in path.relative_to(root).parts)}


def cleanup(root):
    root = root.resolve(); paths = list(root.rglob("*"))
    assert all(p.resolve().is_relative_to(root) for p in paths)
    for p in paths:
        if p.is_file(): p.unlink()
    for p in sorted((p for p in paths if p.is_dir()), key=lambda p: len(p.parts), reverse=True): p.rmdir()
    root.rmdir()


def main():
    parser = argparse.ArgumentParser()
    for name in ("distribution", "sgpm", "sgc", "clang", "evidence"): parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--example", choices=("audit", "knowledge", "maintenance"), action="append")
    args = parser.parse_args(); repo = Path(__file__).resolve().parents[1]
    verify(args.distribution, "agents")
    scratch = Path(tempfile.mkdtemp(prefix="sengoo-agent-registry-世界-"))
    output = args.evidence.resolve(); output.parent.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env.update(SENGOO_STDLIB=str(repo / "tools/stdlib"), SENGOO_RUNTIME=str(repo / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(args.clang.resolve()), SGPM_SGC=str(args.sgc.resolve()))
    env["PATH"] = str(args.clang.resolve().parent) + os.pathsep + env.get("PATH", "")
    env.pop("SENGOO_MODULE_MAP", None)
    records = []
    report = {"schema": 1, "result": "running", "scope": "offline registry, externally locked real consumers, copied source-removed executables with synthetic peers", "compiler_sha256": sha(args.sgc), "distribution_manifest_sha256": sha(args.distribution / "ecosystem-manifest.json"), "steps": records}
    report["runtime_source_identity"] = runtime_identity(repo)
    report["tools"] = {name: {"path": str(getattr(args, name).resolve()), "sha256": sha(getattr(args, name).resolve())}
        for name in ("sgpm", "sgc", "clang")}
    report["git_revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
    report["host"] = {"name": platform.node(), "platform": platform.platform(), "architecture": platform.machine()}

    def run(name, command, cwd, environment):
        started = time.monotonic()
        result = subprocess.run(list(map(str, command)), cwd=cwd, env=environment, capture_output=True, encoding="utf-8", errors="replace", timeout=420)
        (output.parent / (name + ".log")).write_text(result.stdout + result.stderr, encoding="utf-8")
        records.append({"name": name, "exit_code": result.returncode, "seconds": time.monotonic()-started})
        print(f"{name}: {result.returncode}", flush=True)
        assert result.returncode == 0, (name, result.stdout, result.stderr)

    try:
        for name in args.example or ("audit", "knowledge", "maintenance"):
            workspace = scratch / name
            unpack(args.distribution / bundle_name("agents"), workspace)
            (workspace / "src").mkdir()
            source = repo / ("packages/sgagent/examples/maintenance.sg" if name == "maintenance" else f"examples/realworld/agent-{name}/src/main.sg")
            shutil.copyfile(source, workspace / "src/main.sg")
            dependencies = {"audit": ("sgllm", "sgschema"), "knowledge": ("sgmcp", "sgsqlite"), "maintenance": ("sgagent", "sgllm")}[name]
            manifest = f'[package]\nname="agent_{name}"\nversion="0.1.0"\nedition="2026"\n[bin]\npath="src/main.sg"\n[registries.official]\npath="registry"\n[dependencies]\n'
            manifest += "".join(f'{package}={{version="=0.1.0",registry="official"}}\n' for package in dependencies)
            (workspace / "Sengoo.toml").write_text(manifest, encoding="utf-8")
            run(name + "-update", [args.sgpm.resolve(), "--runtime-mode", "source-development", "update"], workspace, env)
            lock = (workspace / "Sengoo.lock").read_bytes()
            assert str(repo).encode() not in lock and b"path+" not in lock
            run(name + "-locked-check", [args.sgpm.resolve(), "--runtime-mode", "source-development", "check", "--locked"], workspace, env)
            run(name + "-locked-build", [args.sgpm.resolve(), "--runtime-mode", "source-development", "build", "--locked"], workspace, env)
            assert (workspace / "Sengoo.lock").read_bytes() == lock
            runtime = scratch / (name + "-runtime"); runtime.mkdir()
            executable = runtime / ("agent-" + name + ".exe")
            shutil.copyfile(workspace / ("target/debug/agent_" + name + ".exe"), executable)
            executable_sha = sha(executable)
            cleanup(workspace)
            clean = {k: v for k, v in os.environ.items() if not k.startswith(("SENGOO_", "SGPM_"))}
            clean["PATH"] = str(Path(os.environ["SystemRoot"]) / "System32")
            fixture = repo / ("packages/sgagent/tests/maintenance_fixture.py" if name == "maintenance" else f"examples/realworld/agent-{name}/tests/verify.py")
            argument = "--evidence" if name == "maintenance" else "--report"
            run(name + "-source-removed-runtime", [sys.executable, "-X", "utf8", fixture, executable, argument, output.parent / (name + "-protocol.json")], runtime, clean)
            shutil.copyfile(executable, output.parent / executable.name)
            records.append({"name": name + "-identity", "executable_sha256": executable_sha, "source_sha256": sha(source), "lock_sha256": hashlib.sha256(lock).hexdigest(), "sources_removed_before_runtime": True})
        final_runtime = runtime_identity(repo)
        original_runtime = report["runtime_source_identity"]
        report["runtime_source_changes_during_verification"] = [name for name in sorted(set(original_runtime) | set(final_runtime))
            if original_runtime.get(name) != final_runtime.get(name)]
        assert not report["runtime_source_changes_during_verification"], "runtime sources changed during verification"
        assert all(sha(Path(tool["path"])) == tool["sha256"] for tool in report["tools"].values()), "selected tool changed during verification"
        assert sha(args.distribution / "ecosystem-manifest.json") == report["distribution_manifest_sha256"], "registry changed during verification"
        verify(args.distribution, "agents")
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error)); raise
    finally:
        cleanup(scratch)
        output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")


if __name__ == "__main__": main()
