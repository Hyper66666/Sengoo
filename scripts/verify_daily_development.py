#!/usr/bin/env python3
"""Fail-closed installed/source native acceptance outside the toolchain checkout.

Run after building the official distribution with package_ecosystem.py. This
script provides machine evidence; it cannot replace the independent human pilot.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

from package_ecosystem import unpack, verify


def copy_project(source: Path, destination: Path) -> None:
    shutil.copytree(source, destination, ignore=shutil.ignore_patterns(
        "target", "build", "__pycache__", "*.pyc", "*.egg-info"))


def acceptance(repo: Path, bin_dir: Path, distribution: Path, mode: str, records: list) -> dict:
    repo, bin_dir, distribution = repo.resolve(), bin_dir.resolve(), distribution.resolve()
    suffix = ".exe" if os.name == "nt" else ""
    tools = {name: bin_dir / (name + suffix) for name in ("sgc", "sgpm", "sgfmt", "sglsp")}
    for path in tools.values():
        if not path.is_file():
            raise ValueError(f"required tool is missing: {path}")
    verify(distribution)
    environment = os.environ.copy()
    for name in ("SENGOO_ROOT", "SENGOO_STDLIB", "SENGOO_RUNTIME", "SENGOO_MODULE_MAP"):
        environment.pop(name, None)
    environment.update(SGPM_SGC=str(tools["sgc"]), SGPM_SGFMT=str(tools["sgfmt"]))
    environment["PATH"] = str(bin_dir) + os.pathsep + environment["PATH"]

    def run(tool: str, args: list[str], cwd: Path, *, expected: int = 0, runtime: bool = True):
        command = [str(tools[tool])]
        if runtime and tool in ("sgc", "sgpm"):
            command += ["--runtime-mode", mode]
        command += args
        result = subprocess.run(command, cwd=cwd, env=environment, capture_output=True,
            text=True, encoding="utf-8", errors="replace", timeout=600)
        records.append({"tool": tool, "args": args, "project": cwd.name,
            "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr})
        if result.returncode != expected:
            raise RuntimeError(f"{command!r} failed in {cwd}:\n{result.stdout}\n{result.stderr}")
        return result

    def locked_loop(project: Path, binary: bool, arguments: list[str] | None = None,
                    *, refresh_lock: bool = True):
        if refresh_lock:
            run("sgpm", ["update"], project)
        original = (project / "Sengoo.lock").read_bytes()
        for args in (["check", "--locked"], ["fmt", "--check", "--locked"],
                ["test", "--locked"], ["doc", "--locked"], ["build", "--locked"]):
            run("sgpm", args, project)
            if (project / "Sengoo.lock").read_bytes() != original:
                raise AssertionError(f"{project.name}: {args} changed the lockfile")
        if binary:
            run("sgpm", ["run", "--locked", *(arguments or [])], project)
        if (project / "Sengoo.lock").read_bytes() != original:
            raise AssertionError(f"{project.name}: run changed the lockfile")

    with tempfile.TemporaryDirectory(prefix="sengoo-daily-outside-") as temporary:
        work = Path(temporary).resolve()
        if work.is_relative_to(repo):
            raise ValueError("acceptance directory must be outside the toolchain checkout")
        doctor = json.loads(run("sgc", ["doctor", "--format", "json"], work).stdout)
        if doctor["schema_version"] != 1 or not doctor["ok"]:
            raise AssertionError("doctor did not return a passing schema-v1 report")
        environment["SENGOO_PYTHON"] = str(work / "missing-python")
        try:
            failed_doctor = json.loads(run("sgc", ["doctor", "--format", "json",
                "--profile", "python"], work, expected=1).stdout)
        finally:
            environment.pop("SENGOO_PYTHON", None)
        failed_backend = [check for check in failed_doctor["checks"]
            if check["id"] == "python-backend"]
        if (failed_doctor["ok"] or failed_doctor["schema_version"] != 1
                or len(failed_backend) != 1 or failed_backend[0]["ok"]
                or not failed_backend[0].get("fix")):
            raise AssertionError("doctor failure did not contain an actionable schema-v1 report")
        for kind in ("bin", "lib"):
            for operation in ("new", "init"):
                name = f"daily_{operation}_{kind}"
                project = work / name
                args = [operation]
                if operation == "new":
                    args += [name]
                    cwd = work
                else:
                    project.mkdir()
                    cwd = project
                if kind == "lib":
                    args += ["--lib"]
                run("sgpm", args, cwd)
                if not (project / "tests/smoke.sg").is_file():
                    raise AssertionError("scaffold has no runnable test")
                locked_loop(project, binary=kind == "bin")

        consumer = work / "library_consumer"
        run("sgpm", ["new", consumer.name], work)
        manifest = consumer / "Sengoo.toml"
        with manifest.open("a", encoding="utf-8") as output:
            output.write('\n[dependencies]\ndaily_new_lib = { path = "../daily_new_lib" }\n')
        (consumer / "src/main.sg").write_text(
            "import daily_new_lib;\n\ndef main() -> i64 {\n    if answer() == 42 {\n        0;\n    } else {\n        1;\n    }\n}\n", encoding="utf-8")
        run("sgpm", ["fmt"], consumer)
        locked_loop(consumer, binary=True)

        for version in ("v0.1.0-rc.1", "v0.2.0-rc.1"):
            project = work / ("compat_" + version)
            copy_project(repo / "examples/compat" / version, project)
            locked_loop(project, binary=False, refresh_lock=False)

        bundle = work / "official"
        unpack(distribution / "sengoo-official-registry-0.1.0.tar.gz", bundle)
        for name in ("sgcli", "sgdata", "sgsqlite"):
            project = bundle / "registry" / name / "0.1.0"
            locked_loop(project, binary=False)
        index = work / "file-csv-index"
        copy_project(repo / "examples/realworld/file-csv-index", index)
        shutil.copytree(bundle / "registry", index / "registry")
        locked_loop(index, binary=True, arguments=["--", "--help"])
        indexed = run("sgpm", ["run", "--locked", "--", "--db", "index.db", "index", "--csv", "sample.csv"], index)
        queried = run("sgpm", ["run", "--locked", "--", "--db", "index.db", "query", "--name", "Alice"], index)
        if "Alice" not in queried.stdout:
            raise AssertionError("restarted index process did not query its persistent SQLite rows")

        # Run the delivered executable with development-tool paths removed.
        binary = index / "target/debug" / ("file_csv_index" + suffix)
        if not binary.is_file():
            binaries = list((index / "target/debug").glob("*.exe")) if suffix else [
                file for file in (index / "target/debug").iterdir() if file.is_file() and os.access(file, os.X_OK)]
            if len(binaries) != 1:
                raise AssertionError("could not identify the delivered index executable")
            binary = binaries[0]
        delivered = work / "delivered"
        delivered.mkdir()
        shutil.copyfile(binary, delivered / binary.name)
        shutil.copystat(binary, delivered / binary.name)
        shutil.copyfile(index / "index.db", delivered / "index.db")
        if index.parent != work or not index.name == "file-csv-index":
            raise ValueError("unexpected disposable consumer source path")
        shutil.rmtree(index)
        environment_without_tools = environment.copy()
        environment_without_tools["PATH"] = str(delivered)
        for key in list(environment_without_tools):
            if key.startswith(("SENGOO_", "SGPM_", "CARGO_", "RUST")):
                environment_without_tools.pop(key)
        result = subprocess.run([str(delivered / binary.name), "--db", "index.db", "query", "--name", "Alice"],
            cwd=delivered, env=environment_without_tools, capture_output=True, text=True,
            encoding="utf-8", errors="replace", timeout=30)
        if result.returncode or "Alice" not in result.stdout:
            raise AssertionError(f"delivered index failed without development tools: {result.stdout}\n{result.stderr}")
        records.append({"check": "delivered-index-without-tools", "ok": True, "stdout": result.stdout})
        unicode_result = subprocess.run([str(delivered / binary.name), "--db", "index.db",
            "query", "--name", "世界"], cwd=delivered, env=environment_without_tools,
            capture_output=True, text=True, encoding="utf-8", errors="replace", timeout=30)
        if unicode_result.returncode or "世界" not in unicode_result.stdout:
            raise AssertionError(f"delivered Unicode query failed: {unicode_result.stdout}\n{unicode_result.stderr}")
        invalid_result = subprocess.run([str(delivered / binary.name), "--unknown"],
            cwd=delivered, env=environment_without_tools, capture_output=True,
            text=True, encoding="utf-8", errors="replace", timeout=30)
        if invalid_result.returncode != 2:
            raise AssertionError("delivered index did not return 2 for an unknown option")
        records.append({"check": "delivered-Unicode-and-CLI-error", "ok": True,
            "stdout": unicode_result.stdout, "invalid_option_exit": invalid_result.returncode})

        # A checksum failure must be observable before extraction/build.
        corrupt = work / "corrupt-distribution"
        shutil.copytree(distribution, corrupt)
        with (corrupt / "sgdata-0.1.0.tar.gz").open("ab") as stream:
            stream.write(b"corrupt")
        try:
            verify(corrupt)
        except ValueError:
            records.append({"check": "corrupt-package-rejected", "ok": True})
        else:
            raise AssertionError("tampered package was accepted")

    revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
    return {"schema_version": 1, "revision": revision, "runtime_mode": mode,
        "outside_checkout": True, "ok": True, "commands": records}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--bin-dir", type=Path, required=True)
    parser.add_argument("--distribution", type=Path, required=True)
    parser.add_argument("--runtime-mode", choices=("installed", "source-development"), default="installed")
    parser.add_argument("--report", type=Path, required=True)
    args = parser.parse_args()
    args.report.parent.mkdir(parents=True, exist_ok=True)
    records = []
    try:
        result = acceptance(args.repo, args.bin_dir, args.distribution, args.runtime_mode, records)
    except Exception as error:
        args.report.write_text(json.dumps({"schema_version": 1, "ok": False,
            "runtime_mode": args.runtime_mode, "error": str(error), "commands": records}, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        raise
    args.report.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(f"native daily-development acceptance passed: {args.report}")


if __name__ == "__main__":
    main()
