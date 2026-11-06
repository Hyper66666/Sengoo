#!/usr/bin/env python3
"""Rebuild official archives, refresh an external lock, and consume shipped code.

This is local development evidence, not publication or four-host qualification.
Requires Python 3.11+, built sgpm/sgc/sgfmt and a configured native toolchain.
"""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

from package_ecosystem import PACKAGES, build, sha256, unpack, verify


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--bin", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=Path("target/ecosystem-foundations"))
    args = parser.parse_args()
    repo, binaries, output = args.repo.resolve(), args.bin.resolve(), args.output.resolve()
    suffix = ".exe" if os.name == "nt" else ""
    tools = {name: binaries / (name + suffix) for name in ("sgpm", "sgc", "sgfmt")}
    for path in tools.values():
        if not path.is_file():
            raise ValueError(f"required tool is missing: {path}")
    inventory = build(repo, output, tools["sgpm"])
    verify(output)
    environment = os.environ.copy()
    environment.pop("SENGOO_MODULE_MAP", None)
    environment.update(SGPM_SGC=str(tools["sgc"]), SGPM_SGFMT=str(tools["sgfmt"]))
    records: list[dict] = []

    def run(project: Path, *arguments: str) -> str:
        command = [str(tools["sgpm"]), "--runtime-mode", "source-development", *arguments]
        result = subprocess.run(command, cwd=project, env=environment, capture_output=True,
            text=True, encoding="utf-8", errors="replace", timeout=600)
        records.append({"project": project.name, "arguments": list(arguments),
            "exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr})
        if result.returncode:
            raise RuntimeError(f"{command!r} failed:\n{result.stdout}\n{result.stderr}")
        return result.stdout

    with tempfile.TemporaryDirectory(prefix="sengoo-official-foundations-") as temporary:
        work = Path(temporary).resolve()
        if work.is_relative_to(repo):
            raise ValueError("consumer must be outside the toolchain source directory")
        bundle = work / "official"
        unpack(output / "sengoo-official-registry-0.1.0.tar.gz", bundle)
        for name in PACKAGES:
            shipped = bundle / "registry" / name / "0.1.0"
            # Assert that archives carry these exact modified implementations,
            # rather than accidentally retesting a stale existing registry.
            for relative in ("src/lib.sg", f"native/{name}.c"):
                if sha256(shipped / relative) != sha256(repo / "packages" / name / relative):
                    raise AssertionError(f"stale packaged source: {name}/{relative}")
            run(shipped, "update")
            lock = (shipped / "Sengoo.lock").read_bytes()
            run(shipped, "check", "--locked")
            run(shipped, "fmt", "--check", "--locked")
            run(shipped, "test", "--locked")
            if (shipped / "Sengoo.lock").read_bytes() != lock:
                raise AssertionError(f"locked checks rewrote the {name} lockfile")
        consumer = work / "file-csv-index"
        shutil.copytree(repo / "examples/realworld/file-csv-index", consumer,
            ignore=shutil.ignore_patterns("target", "registry", "Sengoo.lock", "*.db"))
        shutil.copytree(bundle / "registry", consumer / "registry")
        run(consumer, "update")
        lock = (consumer / "Sengoo.lock").read_bytes()
        run(consumer, "check", "--locked")
        run(consumer, "build", "--locked")
        run(consumer, "run", "--locked", "--", "--help")
        run(consumer, "run", "--locked", "--", "--db", "index.db", "index", "--csv", "sample.csv")
        queried = run(consumer, "run", "--locked", "--", "--db", "index.db", "query", "--name", "Alice")
        if "Alice" not in queried:
            raise AssertionError("external consumer did not reopen its persistent database")
        if (consumer / "Sengoo.lock").read_bytes() != lock:
            raise AssertionError("consumer --locked operations rewrote its refreshed lock")
        (output / "consumer.Sengoo.lock").write_bytes(lock)
    evidence = {"schema": 1, "revision": inventory["revision"], "source_dirty": inventory["dirty"],
        "host": "windows-x64" if os.name == "nt" else os.uname().sysname,
        "package_sources_match": True, "registry_bundle_verified": True,
        "external_consumer_lock_refreshed": True, "checks": records,
        "limitation": "Source-development runtime on this host; no clean-machine, publication or other-host acceptance claim."}
    (output / "foundations-evidence.json").write_text(json.dumps(evidence, indent=2) + "\n", encoding="utf-8")
    print(json.dumps({"output": str(output), "revision": inventory["revision"],
        "source_dirty": inventory["dirty"], "checks_passed": len(records)}, indent=2))


if __name__ == "__main__":
    main()
