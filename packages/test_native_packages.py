#!/usr/bin/env python3
"""Compile and run official package C acceptance with an optional sanitizer.

The .sg tests exercise the real Sengoo runtime and compiler separately. These
native harnesses replace only owned String/Buffer allocation so malformed input,
transaction failure, and stale-handle paths can be checked under sanitizers.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import subprocess
import tempfile


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clang", default="clang")
    parser.add_argument("--sanitize", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    root = Path(__file__).resolve().parent
    compiler = subprocess.run([args.clang, "--version"], capture_output=True,
        text=True, check=True).stdout.splitlines()[0]
    records = []
    with tempfile.TemporaryDirectory(prefix="sengoo-native-packages-") as work:
        directory = Path(work)
        for name in ("sgcli", "sgdata", "sgsqlite"):
            executable = directory / (name + (".exe" if os.name == "nt" else ""))
            command = [args.clang, "-std=c11", "-O1", "-g"]
            if args.sanitize:
                command += ["-fsanitize=address" if os.name == "nt"
                    else "-fsanitize=address,undefined", "-fno-omit-frame-pointer"]
            command += [str(root / name / "tests/native.c"), "-o", str(executable)]
            build = subprocess.run(command, cwd=directory, capture_output=True,
                text=True, errors="replace", timeout=240)
            if build.returncode:
                raise RuntimeError(f"{name} compilation failed:\n{build.stdout}\n{build.stderr}")
            result = subprocess.run([str(executable), "--native-unicode-test", "世界"], cwd=directory,
                capture_output=True, text=True, errors="replace", timeout=60)
            records.append({"package": name, "exit_code": result.returncode,
                "stdout": result.stdout, "stderr": result.stderr,
                "compiler_stderr": build.stderr})
            if result.returncode:
                raise RuntimeError(f"{name} native test failed:\n{result.stdout}\n{result.stderr}")
            print(result.stdout, end="")
    revision = subprocess.check_output(["git", "rev-parse", "HEAD"],
        cwd=root.parent, text=True).strip()
    source_dirty = bool(subprocess.check_output(["git", "status", "--porcelain"],
        cwd=root.parent, text=True).strip())
    report = {"schema_version": 1, "revision": revision, "source_dirty": source_dirty,
        "compiler": compiler,
        "sanitizers": (["address"] if os.name == "nt" else ["address", "undefined"])
            if args.sanitize else [], "tests": records}
    if args.output:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8")


if __name__ == "__main__":
    main()
