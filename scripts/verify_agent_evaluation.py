"""Locked external sgeval build, removed-source offline execution and resources."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import time

from verify_agent_examples import cleanup


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def inputs(repo):
    roots = (repo / "tools/sgeval", repo / "tools/stdlib", repo / "runtime/src")
    return {str(p.relative_to(repo)): sha(p) for root in roots for p in root.rglob("*")
            if p.is_file() and p.suffix in (".sg", ".c", ".h", ".rs", ".py", ".toml", ".lock")
            and not any(part in p.relative_to(root).parts for part in ("target", "build", "__pycache__"))
            and not (root.name == "stdlib" and "tests" in p.relative_to(root).parts)}


def main():
    parser = argparse.ArgumentParser()
    for name in ("compiler", "clang", "formatter", "sgpm", "evidence"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    output = args.evidence.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    frozen = inputs(repo)
    tools = {name: {"path": str(getattr(args, name).resolve()), "sha256": sha(getattr(args, name))}
             for name in ("compiler", "clang", "formatter", "sgpm")}
    env = dict(os.environ)
    env.update(SENGOO_CLANG=tools["clang"]["path"], SGPM_SGC=tools["compiler"]["path"],
               SGPM_SGFMT=tools["formatter"]["path"], SENGOO_STDLIB=str(repo / "tools/stdlib"),
               SENGOO_RUNTIME=str(repo / "tools/stdlib/runtime.c"))
    env.pop("SENGOO_MODULE_MAP", None)
    env["PATH"] = str(args.clang.resolve().parent) + os.pathsep + env.get("PATH", "")
    clean = {k: v for k, v in os.environ.items() if not k.startswith(("SENGOO_", "SGPM_"))}
    clean["PATH"] = str(Path(os.environ["SystemRoot"]) / "System32")
    report = {"schema": 1, "result": "running", "host": os.environ.get("COMPUTERNAME"),
              "scope": "Windows external locked build, source-removed synthetic offline checks, native exact resources",
              "tools": tools, "sources": frozen, "steps": []}
    scratch = Path(tempfile.mkdtemp(prefix="sgeval-external-世界-"))
    package = scratch / "package"
    package.mkdir()

    def run(name, command, environment=env, cwd=scratch):
        started = time.monotonic()
        result = subprocess.run(list(map(str, command)), cwd=cwd, env=environment, capture_output=True,
                                encoding="utf-8", errors="replace", timeout=300)
        (output.parent / (name + ".log")).write_text(result.stdout + result.stderr, encoding="utf-8")
        report["steps"].append({"name": name, "exit_code": result.returncode,
                                "seconds": time.monotonic() - started})
        assert result.returncode == 0, (name, result.stdout, result.stderr)
        print(name + ": passed", flush=True)

    try:
        for relative in ("Sengoo.toml", "Sengoo.lock", "src/lib.sg", "src/main.sg", "tests/evaluation.sg"):
            destination = package / relative
            destination.parent.mkdir(exist_ok=True)
            shutil.copyfile(repo / "tools/sgeval" / relative, destination)
            if destination.suffix == ".sg":
                run("format-" + destination.stem, [args.formatter.resolve(), "--check", destination])
        lock = sha(package / "Sengoo.lock")
        run("external-locked-build", [args.sgpm.resolve(), "--runtime-mode", "source-development",
                                     "build", "--locked"], cwd=package)
        assert sha(package / "Sengoo.lock") == lock
        executable = output.parent / "sgeval.exe"
        shutil.copyfile(package / "target/debug/sgeval.exe", executable)
        cleanup(package)
        run("source-removed-offline", [sys.executable, "-X", "utf8", repo / "tools/sgeval/tests/offline.py",
                                      executable, "--report", output.parent / "offline.json"], clean)
        native = output.parent / "sgeval-native.exe"
        native_env = dict(env, SENGOO_MODULE_MAP="sgeval=" + str(repo / "tools/sgeval/src/lib.sg"))
        run("native-build", [args.compiler.resolve(), "--runtime-mode", "source-development", "build",
                             repo / "tools/sgeval/tests/evaluation.sg", "--force-rebuild", "-O", "0", "-o", native], native_env)
        run("native-resources", [native], clean)
        assert inputs(repo) == frozen, "evaluation/runtime sources changed during verification"
        assert all(sha(Path(tool["path"])) == tool["sha256"] for tool in tools.values())
        report.update(result="passed", lock_unchanged=True, sources_removed_before_runtime=True,
                      executable_sha256=sha(executable), native_sha256=sha(native))
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        cleanup(scratch)


if __name__ == "__main__":
    main()
