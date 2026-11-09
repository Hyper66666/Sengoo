"""Inspect and execute SG COFF sections/symbols/relocations independently."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import struct
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m4-object-final")
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target"):
        raise ValueError("outputs must stay inside target/")
    out.mkdir(parents=True, exist_ok=True)
    paths = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter")}
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
    names = ("core", "binary", "coff", "object")
    inputs = [Path(__file__), ROOT / "selfhost/bootstrap.lock", ROOT / "selfhost/tests/object.sg",
              ROOT / "selfhost/tests/object_probe.c"]
    inputs += [ROOT / "selfhost/src" / (name + ".sg") for name in names]
    inputs += sorted((ROOT / "tools/stdlib").rglob("*.sg"))
    inputs += [ROOT / "tools/stdlib/runtime.c", ROOT / "tools/stdlib/runtime_abi.h"]
    report = {"schema": 1, "result": "running", "stage": "coff-sections-symbols-relocations",
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "host": platform.platform(), "sources": {str(p.relative_to(ROOT)): sha(p) for p in inputs if p.is_file()},
              "tools": {}, "steps": [], "m4_complete": False,
              "limits": ["COFF logical schema 1, no auxiliary/weak/common/COMDAT symbol records or BSS sections yet",
                         "at most 1024 sections, 100000 symbols/relocations, 65535 relocations per section, 64 MiB output",
                         "SG generates object bytes; external tools only inspect and independently link verification fixtures",
                         "not a native MIR consumer, SG linker, unwind/CodeView writer or self-compilation"]}
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
            env.pop(name)
    env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"),
               SENGOO_CLANG=str(paths["clang"]), SENGOO_MODULE_MAP=";".join(
                   "selfhost_" + name + "=" + str(ROOT / "selfhost/src" / (name + ".sg")) for name in names))
    env["PATH"] = str(paths["clang"].parent) + os.pathsep + env.get("PATH", "")

    def run(name, command, marker=None):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=600)
        (out / (name + ".stdout.log")).write_bytes(done.stdout)
        (out / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)),
                                "exit_code": done.returncode, "seconds": time.monotonic() - started})
        if done.returncode or marker and marker not in done.stdout:
            raise AssertionError(f"{name} failed: {done.returncode}")
        print(f"{name}: {done.returncode}", flush=True)
        return done.stdout

    try:
        for name, path in paths.items():
            digest = sha(path)
            if digest != lock[name]["sha256"]:
                raise ValueError(f"{name} identity mismatch")
            report["tools"][name] = {"path": str(path), "sha256": digest}
        for folder in ("src", "tests"):
            run("format-" + folder, [paths["formatter"], "--check", ROOT / "selfhost" / folder / "object.sg"])
        binary = out / "object-tests.exe"
        binary.unlink(missing_ok=True)
        run("build-sg", [paths["seed"], "--runtime-mode", "source-development", "build",
                         ROOT / "selfhost/tests/object.sg", "--force-rebuild", "-O", "0", "-o", binary])
        run("native-sg", [binary], b"selfhost COFF sections and relocations: ok")
        object_file, repeated = out / "sections.obj", out / "repeat.obj"
        for name, path in (("emit", object_file), ("emit-repeat", repeated)):
            path.unlink(missing_ok=True)
            run(name, [binary, path])
        raw = object_file.read_bytes()
        if raw != repeated.read_bytes() or len(raw) < 140:
            raise AssertionError("object bytes are not deterministic")
        machine, sections, timestamp, symbols_at, symbol_count, optional, flags = struct.unpack_from("<HHIIIHH", raw)
        if (machine, sections, timestamp, symbol_count, optional, flags) != (0x8664, 3, 0, 4, 0, 0):
            raise AssertionError("unexpected COFF header")
        if symbols_at + symbol_count * 18 + 4 > len(raw):
            raise AssertionError("symbol table outside file")
        string_size = struct.unpack_from("<I", raw, symbols_at + symbol_count * 18)[0]
        if symbols_at + symbol_count * 18 + string_size != len(raw):
            raise AssertionError("string table extent differs")
        inspector = paths["clang"].parent / "llvm-readobj.exe"
        report["tools"]["inspector"] = {"path": str(inspector), "sha256": sha(inspector)}
        inspected = run("inspect", [inspector, "--file-headers", "--sections", "--symbols", "--relocations", object_file])
        for marker in (b".rodata.verylong", b"sg_object_answer", b"sg_object_external", b"IMAGE_REL_AMD64_REL32", b"IMAGE_REL_AMD64_ADDR64"):
            if marker not in inspected:
                raise AssertionError(f"missing inspection marker {marker!r}")
        probe = out / "object-probe.exe"
        run("link-c", [paths["clang"], ROOT / "selfhost/tests/object_probe.c", object_file, "-Werror", "-o", probe])
        run("execute-c", [probe], b"SG COFF external call, data and absolute pointer: ok")
        report["artifacts"] = {p.name: sha(p) for p in (binary, object_file, repeated, probe)}
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items()
                   if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items()
                         if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        report.update(changed_sources=changed, changed_tools=changed_tools)
        if changed or changed_tools:
            report.update(result="failed", error="object inputs changed during verification")
        (out / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("object inputs changed")


if __name__ == "__main__":
    main()
