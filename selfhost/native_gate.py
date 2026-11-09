"""Verify SG type/binary/x64/COFF foundations; this is not full native codegen."""
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
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/native-foundations")
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target"):
        raise ValueError("outputs must stay inside target/")
    out.mkdir(parents=True, exist_ok=True)
    tools = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter")}
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
    report = {"schema": 1, "result": "running", "stage": "typed-and-native-binary-foundations",
              "host": platform.platform(), "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "steps": [], "sources": {}, "tools": {},
              "limits": ["encoder and single-text COFF foundation, not MIR code generation or a completed backend",
                         "C CPU/COFF probes and object inspection/linking use explicit external verification tools",
                         "Windows x64 source-development seed/runtime remains required"]}
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
            env.pop(name)
    names = ("core", "types", "binary", "x64", "coff")
    env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"),
               SENGOO_CLANG=str(tools["clang"]), SENGOO_MODULE_MAP=";".join(
                   "selfhost_" + name + "=" + str(ROOT / "selfhost/src" / (name + ".sg")) for name in names))
    env["PATH"] = str(tools["clang"].parent) + os.pathsep + env.get("PATH", "")
    (out / "evidence.json").write_text(json.dumps(report), encoding="utf-8")

    def run(name, command, code=0, marker=None):
        start = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=600)
        (out / (name + ".stdout.log")).write_bytes(done.stdout)
        (out / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)), "exit_code": done.returncode,
                                "expected_exit_code": code, "seconds": time.monotonic() - start})
        if done.returncode != code or marker and marker not in done.stdout:
            raise AssertionError(f"{name} failed: {done.returncode}; see {out}")
        print(f"{name}: {done.returncode}", flush=True)
        return done

    try:
        for name, path in tools.items():
            digest = sha(path)
            if digest != lock[name]["sha256"]:
                raise ValueError(f"{name} identity mismatch")
            report["tools"][name] = {"path": str(path), "sha256": digest}
        inputs = [Path(__file__).resolve(), ROOT / "selfhost/bootstrap.lock"]
        inputs += [ROOT / "selfhost/src" / (name + ".sg") for name in names]
        inputs += [ROOT / "selfhost/tests" / (name + ".sg") for name in names if name != "core"]
        inputs += [ROOT / "selfhost/tests/x64_probe.c", ROOT / "selfhost/tests/coff_probe.c"]
        report["sources"] = {str(path.relative_to(ROOT)): sha(path) for path in inputs}
        binaries = {}
        for name in names:
            if name == "core":
                continue
            for folder in ("src", "tests"):
                run("format-" + folder + "-" + name, [tools["formatter"], "--check", ROOT / "selfhost" / folder / (name + ".sg")])
            binary = out / (name + "-tests.exe")
            binary.unlink(missing_ok=True)
            run("build-" + name, [tools["seed"], "--runtime-mode", "source-development", "build",
                                  ROOT / "selfhost/tests" / (name + ".sg"), "--force-rebuild", "-O", "0", "-o", binary])
            marker = {"types": b"selfhost types: ok", "binary": b"selfhost binary: ok",
                      "x64": b"selfhost x64 encoding: ok", "coff": b"selfhost COFF: ok"}[name]
            run("native-" + name, [binary], marker=marker)
            binaries[name] = binary
        encoded = out / "code.bin"
        run("emit-x64", [binaries["x64"], encoded])
        probe = out / "cpu-probe.exe"
        run("build-cpu-probe", [tools["clang"], ROOT / "selfhost/tests/x64_probe.c", "-O2", "-o", probe])
        run("cpu-execute", [probe, encoded], marker=b"exact encoding and native execution returned 42")
        corrupt = out / "corrupt.bin"
        changed = bytearray(encoded.read_bytes())
        changed[2] = 41
        corrupt.write_bytes(changed)
        run("cpu-reject-wrong-encoding", [probe, corrupt], 66)
        short, long = out / "short.obj", out / "long.obj"
        run("emit-coff-short", [binaries["coff"], short])
        run("emit-coff-long", [binaries["coff"], long, "long"])
        repeated = out / "short-repeat.obj"
        run("emit-coff-repeat", [binaries["coff"], repeated])
        if short.read_bytes() != repeated.read_bytes() or short.read_bytes()[4:8] != bytes(4):
            raise AssertionError("COFF bytes are not deterministic")
        inspector = tools["clang"].parent / "llvm-readobj.exe"
        report["tools"]["llvm-readobj"] = {"path": str(inspector), "sha256": sha(inspector)}
        run("object-inspect", [inspector, "--file-headers", "--sections", "--symbols", short, long],
            marker=b"sengoo_native_probe_long")
        linked = out / "coff-probe.exe"
        run("coff-link", [tools["clang"], ROOT / "selfhost/tests/coff_probe.c", short, long, "-o", linked])
        run("coff-execute", [linked], marker=b"short and long symbols linked and returned 42")
        report["artifacts"] = {p.name: sha(p) for p in (*binaries.values(), encoded, short, long, linked, probe)}
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = []
        for path, digest in report["sources"].items():
            try:
                equal = sha(ROOT / path) == digest
            except OSError:
                equal = False
            if not equal:
                changed.append(path)
        changed_tools = []
        for name, identity in report["tools"].items():
            try:
                equal = sha(Path(identity["path"])) == identity["sha256"]
            except OSError:
                equal = False
            if not equal:
                changed_tools.append(name)
        report.update(changed_sources=changed, changed_tools=changed_tools)
        if changed or changed_tools:
            report.update(result="failed", error="native foundation inputs changed")
        (out / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("native foundation inputs changed")


if __name__ == "__main__":
    main()
