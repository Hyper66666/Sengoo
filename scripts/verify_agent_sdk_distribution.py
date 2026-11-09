"""Consume checksummed SDK archives outside both source repositories."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from package_ecosystem import unpack, verify


PROGRAM = r'''import sgllm;
import sgmcp;
import sgschema;

def exercise() -> Result<bool, i64> {
    let contract = json_parse("{\"type\":\"object\",\"required\":[\"value\"]}")?;
    let input = json_parse("{\"value\":9223372036854775807}")?;
    let validation = schema_validate(contract.root(), input.root())?;
    if not validation.valid or not sgmcp_version_supported("2025-11-25") { return Result { is_ok: false, value: false, error: 99 }; };
    let mut turn = provider_turn();
    turn.event(0, "{\"choices\":[{\"delta\":{\"content\":\"独立 SDK consumer\"},\"finish_reason\":\"stop\"}]}")?;
    turn.event(0, "[DONE]")?;
    let answer = turn.finish()?;
    let content = answer.root().object_get("content")?.string_value()?;
    if content.as_str() != "独立 SDK consumer" { return Result { is_ok: false, value: false, error: 99 }; };
    Result { is_ok: true, value: true, error: 0 }
}
def main() -> i64 { let result = exercise(); if not result.is_ok() { println(result.error); return 1; }; println("offline SDK consumer passed"); 0 }
'''


def cleanup(root):
    root = root.resolve(); paths = list(root.rglob("*"))
    if any(not p.resolve().is_relative_to(root) for p in paths):
        raise ValueError("cleanup escaped disposable root")
    for path in paths:
        if path.is_file() or path.is_symlink(): path.unlink()
    for path in sorted((p for p in paths if p.is_dir()), key=lambda p: len(p.parts), reverse=True): path.rmdir()
    root.rmdir()


def main():
    parser = argparse.ArgumentParser(); parser.add_argument("--distribution", type=Path, required=True)
    parser.add_argument("--sgpm", type=Path, required=True); parser.add_argument("--sgc", type=Path, required=True)
    parser.add_argument("--sengoo", type=Path, required=True); parser.add_argument("--clang", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True); args = parser.parse_args()
    verify(args.distribution, "agents")
    scratch = Path(tempfile.mkdtemp(prefix="sengoo-offline-agent-世界-")); root = scratch / "consumer"
    unpack(args.distribution / "sengoo-agent-preview-registry-0.1.0.tar.gz", root)
    (root / "src").mkdir()
    (root / "src/main.sg").write_text(PROGRAM, encoding="utf-8")
    (root / "Sengoo.toml").write_text('''[package]
name="sdk_consumer"
version="0.1.0"
edition="2026"
[bin]
path="src/main.sg"
[registries.official]
path="registry"
[dependencies]
sgllm={version="=0.1.0",registry="official"}
sgmcp={version="=0.1.0",registry="official"}
sgschema={version="=0.1.0",registry="official"}
''', encoding="utf-8")
    env = dict(os.environ)
    env.update(SENGOO_CLANG=str(args.clang.resolve()), SGPM_SGC=str(args.sgc.resolve()),
               SENGOO_STDLIB=str(args.sengoo.resolve() / "tools/stdlib"),
               SENGOO_RUNTIME=str(args.sengoo.resolve() / "tools/stdlib/runtime.c"))
    env.pop("SENGOO_MODULE_MAP", None)
    env["PATH"] = str(args.clang.resolve().parent) + os.pathsep + env.get("PATH", "")
    evidence = {"schema": 1, "result": "running", "host": os.environ.get("COMPUTERNAME"),
                "scope": "offline relative registry, locked external project, standalone execution",
                "compiler_sha256": hashlib.sha256(args.sgc.read_bytes()).hexdigest(), "steps": []}

    def run(name, command, environment, directory=None):
        start = time.monotonic(); result = subprocess.run(list(map(str, command)), cwd=directory or root, env=environment,
            capture_output=True, encoding="utf-8", errors="replace", timeout=360)
        evidence["steps"].append({"name": name, "exit_code": result.returncode, "seconds": time.monotonic()-start,
            "stdout": result.stdout, "stderr": result.stderr})
        if result.returncode: raise RuntimeError(f"{name}: {result.stdout}\n{result.stderr}")

    try:
        run("resolve-offline", [args.sgpm, "update", "--runtime-mode", "source-development"], env)
        lock = (root / "Sengoo.lock").read_text(encoding="utf-8")
        assert "../Sengoo/packages" not in lock and str(args.sengoo.resolve()) not in lock
        for name in ("sgllm", "sgmcp", "sgschema"): assert f'name = "{name}"' in lock
        run("locked-check", [args.sgpm, "check", "--locked", "--runtime-mode", "source-development"], env)
        run("locked-build", [args.sgpm, "build", "--locked", "--runtime-mode", "source-development"], env)
        image = root / "target/debug/sdk_consumer.exe"
        isolated = scratch / "standalone"; isolated.mkdir(); copied = isolated / "sdk_consumer.exe"
        copied.write_bytes(image.read_bytes())
        clean = {k: v for k, v in env.items() if not k.startswith(("SENGOO_", "SGPM_"))}
        clean["PATH"] = str(Path(os.environ["SystemRoot"]) / "System32")
        cleanup(root)
        # Execute only the copied native image: no package/compiler/source path in argv or env.
        run("standalone-system-path", [copied], clean, isolated)
        assert "offline SDK consumer passed" in evidence["steps"][-1]["stdout"]
        evidence["binary_sha256"] = hashlib.sha256(copied.read_bytes()).hexdigest()
        evidence["lockfile"] = lock; evidence["result"] = "passed"
    except BaseException as error:
        evidence["result"] = "failed"; evidence["error"] = repr(error); raise
    finally:
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        args.evidence.write_text(json.dumps(evidence, ensure_ascii=False, indent=2), encoding="utf-8")
        if evidence["result"] == "passed": cleanup(scratch)
    print("checksummed offline SDK consumption and standalone execution passed")


if __name__ == "__main__": main()
