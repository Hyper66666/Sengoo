"""Verify direct SSE scalar encoding using independent bytes and CPU execution."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m4-sse-final")
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target"):
        raise ValueError("outputs must stay inside target/")
    out.mkdir(parents=True, exist_ok=True)
    paths = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter")}
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
    names = ("core", "binary", "x64", "x64_sse")
    inputs = [Path(__file__), ROOT / "selfhost/bootstrap.lock", ROOT / "selfhost/tests/x64_sse.sg", ROOT / "selfhost/tests/sse_probe.c"]
    inputs += [ROOT / "selfhost/src" / (name + ".sg") for name in names]
    inputs += sorted((ROOT / "tools/stdlib").rglob("*.sg"))
    inputs += [ROOT / "tools/stdlib/runtime.c", ROOT / "tools/stdlib/runtime_abi.h"]
    report = {"schema": 1, "result": "running", "stage": "x64-sse-scalar-encoder",
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "host": platform.platform(), "sources": {str(p.relative_to(ROOT)): sha(p) for p in inputs if p.is_file()},
              "tools": {}, "steps": [], "m4_complete": False,
              "limits": ["scalar SSE instruction encoder only; not full floating-point MIR lowering or ABI integration",
                         "independent C probe verifies bytes then executes f32/f64 arithmetic and memory instructions",
                         "NaN/ordered comparison, conversion saturation, rounding and all target semantics remain separate gates"]}
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
            env.pop(name)
    env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"),
               SENGOO_CLANG=str(paths["clang"]), SENGOO_MODULE_MAP=";".join(
                   "selfhost_" + name + "=" + str(ROOT / "selfhost/src" / (name + ".sg")) for name in names))
    env["PATH"] = str(paths["clang"].parent) + os.pathsep + env.get("PATH", "")

    def run(name, command, code=0, marker=None):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=600)
        (out / (name + ".stdout.log")).write_bytes(done.stdout)
        (out / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)), "exit_code": done.returncode,
                                "expected_exit_code": code, "seconds": time.monotonic() - started})
        if done.returncode != code or marker and marker not in done.stdout + done.stderr:
            raise AssertionError(f"{name} failed: {done.returncode}")
        print(f"{name}: {done.returncode}", flush=True)
        return done

    try:
        for name, path in paths.items():
            digest = sha(path)
            if digest != lock[name]["sha256"]:
                raise ValueError(f"{name} identity mismatch")
            report["tools"][name] = {"path": str(path), "sha256": digest}
        for folder in ("src", "tests"):
            run("format-" + folder, [paths["formatter"], "--check", ROOT / "selfhost" / folder / "x64_sse.sg"])
        binary = out / "sse-tests.exe"
        binary.unlink(missing_ok=True)
        run("build-sg", [paths["seed"], "--runtime-mode", "source-development", "build",
                         ROOT / "selfhost/tests/x64_sse.sg", "--force-rebuild", "-O", "0", "-o", binary])
        run("native-sg", [binary], marker=b"selfhost x64 SSE scalar encoding: ok")
        code, repeated = out / "code.bin", out / "repeat.bin"
        run("emit", [binary, code]); run("emit-repeat", [binary, repeated])
        if code.read_bytes() != repeated.read_bytes():
            raise AssertionError("SSE bytes differ on repeated emission")
        probe = out / "sse-probe.exe"
        run("build-probe", [paths["clang"], ROOT / "selfhost/tests/sse_probe.c", "-Werror", "-o", probe])
        run("execute", [probe, code], marker=b"exact SSE bytes and f32/f64 native execution: ok")
        damaged = out / "damaged.bin"
        raw = bytearray(code.read_bytes()); raw[0] = 0
        damaged.write_bytes(raw)
        run("reject-damaged", [probe, damaged], 66)
        report["artifacts"] = {p.name: sha(p) for p in (binary, code, repeated, probe)}
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items()
                         if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        report.update(changed_sources=changed, changed_tools=changed_tools)
        if changed or changed_tools:
            report.update(result="failed", error="SSE inputs changed during verification")
        (out / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("SSE inputs changed")


if __name__ == "__main__":
    main()
