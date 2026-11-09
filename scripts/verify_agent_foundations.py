"""Verify schema/resource and two-reviewer acceptance with a pinned compiler."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time


def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    for name in ("compiler", "clang", "formatter", "evidence"): parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args(); repo = Path(__file__).resolve().parents[1]
    evidence = args.evidence.resolve(); evidence.parent.mkdir(parents=True, exist_ok=True)
    roots = [repo / "packages/sgschema", repo / "packages/sgllm", repo / "examples/realworld/agent-audit"]
    files = [p for root in roots for p in root.rglob("*") if p.is_file() and not any(x in p.parts for x in ("target", "build", "__pycache__"))]
    files += [p for p in (repo / "tools/stdlib").iterdir() if p.suffix in (".c", ".h", ".sg")]
    sources = {str(p.relative_to(repo)): sha(p) for p in files}
    report = {"schema": 1, "result": "running", "scope": "native schema exact resources and independent synthetic audit providers; no user credentials", "compiler_sha256": sha(args.compiler), "formatter_sha256": sha(args.formatter), "sources": sources, "steps": []}
    env = dict(os.environ)
    env.update(SENGOO_STDLIB=str(repo / "tools/stdlib"), SENGOO_RUNTIME=str(repo / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(args.clang.resolve()), SENGOO_MODULE_MAP=";".join(name + "=" + str(repo / "packages" / name / "src/lib.sg") for name in ("sgllm", "sgschema")))
    env["PATH"] = str(args.clang.resolve().parent) + os.pathsep + env.get("PATH", "")

    def run(name, command):
        start = time.monotonic()
        result = subprocess.run(list(map(str, command)), cwd=repo, env=env, capture_output=True, encoding="utf-8", errors="replace", timeout=300)
        (evidence.parent / (name + ".log")).write_text(result.stdout + result.stderr, encoding="utf-8")
        report["steps"].append({"name": name, "exit_code": result.returncode, "seconds": time.monotonic()-start})
        print(f"{name}: {result.returncode}", flush=True)
        assert result.returncode == 0, (name, result.stdout, result.stderr)

    try:
        for source in ("packages/sgschema/src/lib.sg", "packages/sgschema/tests/schema.sg", "examples/realworld/agent-audit/src/main.sg"):
            run("fmt-" + Path(source).parent.parent.name + "-" + Path(source).name, [args.formatter, "--check", repo / source])
        for name, source in (("schema", "packages/sgschema/tests/schema.sg"), ("audit", "examples/realworld/agent-audit/src/main.sg")):
            binary = evidence.parent / (name + ".exe")
            run(name + "-build", [args.compiler, "--runtime-mode", "source-development", "build", repo / source, "--force-rebuild", "-O", "0", "-o", binary])
            if name == "schema": run(name + "-native", [binary])
            else: run(name + "-providers", [sys.executable, "-X", "utf8", repo / "examples/realworld/agent-audit/tests/verify.py", binary, "--report", evidence.parent / "audit-protocol.json"])
            report.setdefault("binaries", {})[name] = {"path": str(binary), "sha256": sha(binary)}
        assert all(sha(repo / path) == digest for path, digest in sources.items()), "source input changed during verification"
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error)); raise
    finally:
        evidence.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")


if __name__ == "__main__": main()
