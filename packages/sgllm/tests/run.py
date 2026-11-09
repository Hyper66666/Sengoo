"""Compile SDK consumers outside its checkout, then run native and HTTP peers.

All network traffic is synthetic loopback. No user configuration or credential
store is read. The evidence distinguishes compilation, fixture and package use.
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


def remove_owned_tree(root):
    root = root.resolve(); paths = list(root.rglob("*"))
    assert all(p.resolve().is_relative_to(root) for p in paths)
    for path in paths:
        if path.is_file() or path.is_symlink(): path.unlink()
    for path in sorted((p for p in paths if p.is_dir()), key=lambda p: len(p.parts), reverse=True): path.rmdir()
    root.rmdir()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def foundation_snapshot(language):
    files = {p for p in (language / "tools/stdlib").iterdir()
             if p.is_file() and p.suffix in (".sg", ".c", ".h")}
    files.update((language / "runtime/src").rglob("*.rs"))
    files.update(language / name for name in ("Cargo.toml", "Cargo.lock", "runtime/Cargo.toml"))
    return {str(path.relative_to(language)): digest(path) for path in sorted(files)}


def main():
    parser = argparse.ArgumentParser(); parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--sengoo", type=Path, required=True); parser.add_argument("--clang", type=Path, required=True)
    parser.add_argument("--sgpm", type=Path); parser.add_argument("--formatter", type=Path)
    parser.add_argument("--evidence", type=Path, required=True)
    parser.add_argument("--example-only", action="store_true")
    args = parser.parse_args(); package = Path(__file__).resolve().parents[1]; language = args.sengoo.resolve()
    inputs = {str(p.relative_to(package)): hashlib.sha256(p.read_bytes()).hexdigest()
              for p in package.rglob("*") if p.is_file() and not any(x in p.parts for x in ("target", "build", "__pycache__"))}
    scratch = Path(tempfile.mkdtemp(prefix="sgllm-consumer-世界-")); independent = scratch / "sdk"; independent.mkdir()
    for name in inputs:
        source = package / name; target = independent / name; target.parent.mkdir(parents=True, exist_ok=True); shutil.copyfile(source, target)
    environment = dict(os.environ); environment.update(SENGOO_STDLIB=str(language / "tools/stdlib"),
        SENGOO_RUNTIME=str(language / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(args.clang.resolve()),
        SENGOO_MODULE_MAP="sgllm=" + str(independent / "src/lib.sg"))
    environment["PATH"] = str(args.clang.resolve().parent) + os.pathsep + environment.get("PATH", "")
    foundation = foundation_snapshot(language)
    tools = {"compiler": args.compiler.resolve(), "clang": args.clang.resolve()}
    if args.sgpm: tools["sgpm"] = args.sgpm.resolve()
    if args.formatter: tools["formatter"] = args.formatter.resolve()
    tool_hashes = {name: digest(path) for name, path in tools.items()}
    evidence = {"schema": 1, "result": "running", "host": os.environ.get("COMPUTERNAME"),
        "platform": os.name, "scope": "source-runtime outside-checkout package consumers and synthetic loopback HTTP",
        "compiler_sha256": tool_hashes["compiler"], "clang_sha256": tool_hashes["clang"],
        "tool_paths": {name: str(path) for name, path in tools.items()}, "tool_sha256": tool_hashes,
        "foundation_sources": foundation,
        "example_only": args.example_only, "sources": inputs, "steps": []}

    def run(name, command, cwd=independent, timeout=240):
        began = time.monotonic(); result = subprocess.run([str(v) for v in command], cwd=cwd,
            env=environment, text=True, encoding="utf-8", errors="replace", capture_output=True, timeout=timeout)
        step = {"name": name, "exit_code": result.returncode, "seconds": time.monotonic()-began,
            "stdout": result.stdout, "stderr": result.stderr}; evidence["steps"].append(step)
        print(f"{name}: {result.returncode}", flush=True)
        assert result.returncode == 0, step
        return result

    try:
        spec = importlib.util.spec_from_file_location("sgllm_stream_fixture", independent / "tests/stream_fixture.py")
        fixture = importlib.util.module_from_spec(spec); spec.loader.exec_module(fixture)
        output = independent / "target"; output.mkdir()
        if args.formatter:
            evidence["formatter_sha256"] = hashlib.sha256(args.formatter.read_bytes()).hexdigest()
            for name in inputs:
                if name.endswith(".sg"):
                    run("fmt-"+name.replace("\\", "/"), [args.formatter.resolve(), "--check", independent / name])
        for name in (() if args.example_only else ("protocol", "http", "media", "config", "resources", "transport", "failures")):
            binary = output / (name + (".exe" if os.name == "nt" else ""))
            run(name+"-build", [args.compiler.resolve(), "--runtime-mode", "source-development", "build", independent / f"tests/{name}.sg",
                "--force-rebuild", "-O", "0", "-o", binary])
            if name in ("transport", "failures"):
                step = fixture.exercise(binary, name == "failures"); step.update(name=name+"-loopback", binary_sha256=hashlib.sha256(binary.read_bytes()).hexdigest())
                evidence["steps"].append(step); print(name+"-loopback: 0", flush=True)
            else: run(name+"-native", [binary], timeout=30)
        example = output / ("stream" + (".exe" if os.name == "nt" else ""))
        run("example-build", [args.compiler.resolve(), "--runtime-mode", "source-development", "build",
            independent / "examples/stream.sg", "--force-rebuild", "-O", "0", "-o", example])
        step = fixture.exercise(example, extra_arguments=("responses", "fixture", "fixture-key"), minimum_active=1)
        assert step["stdout"].replace("\r\n", "\n") == "世界😀\n17\n8\n", step["stdout"]
        step.update(name="example-loopback", binary_sha256=hashlib.sha256(example.read_bytes()).hexdigest())
        evidence["steps"].append(step); print("example-loopback: 0", flush=True)
        if args.sgpm and not args.example_only:
            consumer = scratch / "app"; (consumer / "src").mkdir(parents=True)
            shutil.copyfile(independent / "tests/config.sg", consumer / "src/main.sg")
            (consumer / "Sengoo.toml").write_text('[package]\nname="sgllm_consumer"\nversion="0.1.0"\nedition="2026"\n[bin]\npath="src/main.sg"\n[dependencies]\nsgllm={path="../sdk"}\n', encoding="utf-8")
            environment["SGPM_SGC"] = str(args.compiler.resolve()); environment.pop("SENGOO_MODULE_MAP", None)
            run("external-sgpm-update", [args.sgpm.resolve(), "update"], cwd=consumer)
            assert (consumer / "Sengoo.lock").is_file()
            before = (consumer / "Sengoo.lock").read_bytes()
            run("external-sgpm-check-locked", [args.sgpm.resolve(), "--runtime-mode", "source-development", "check", "--locked"], cwd=consumer)
            run("external-sgpm-run-locked", [args.sgpm.resolve(), "--runtime-mode", "source-development", "run", "--locked"], cwd=consumer)
            assert (consumer / "Sengoo.lock").read_bytes() == before
        evidence["sdk_source_guard_passed"] = inputs == {str(p.relative_to(package)): digest(p)
            for p in package.rglob("*") if p.is_file() and not any(x in p.parts for x in ("target", "build", "__pycache__"))}
        evidence["foundation_source_guard_passed"] = foundation == foundation_snapshot(language)
        evidence["tool_guard_passed"] = tool_hashes == {name: digest(path) for name, path in tools.items()}
        assert evidence["sdk_source_guard_passed"], "SDK changed during verification"
        assert evidence["foundation_source_guard_passed"], "language/runtime changed during verification"
        assert evidence["tool_guard_passed"], "tool binary changed during verification"
        evidence["result"] = "passed"
    except BaseException as error:
        evidence["result"] = "failed"; evidence["error"] = repr(error); raise
    finally:
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        args.evidence.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
        remove_owned_tree(scratch)


if __name__ == "__main__": main()
