"""Execute SG scalar MIR machine code through independent Windows C callers."""
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
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m4-scalar-final")
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target"):
        raise ValueError("outputs must stay inside target/")
    out.mkdir(parents=True, exist_ok=True)
    paths = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter")}
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
    names = ("core", "types", "lexer", "mir", "mir_validate", "layout", "abi", "binary", "coff", "object", "x64", "x64_scalar", "x64_sse", "float_literal", "unwind", "native")
    inputs = [Path(__file__), ROOT / "selfhost/bootstrap.lock", ROOT / "selfhost/tests/native.sg", ROOT / "selfhost/tests/abi.sg", ROOT / "selfhost/tests/native_probe.c"]
    inputs += [ROOT / "selfhost/src" / (name + ".sg") for name in names]
    inputs += sorted((ROOT / "tools/stdlib").rglob("*.sg"))
    inputs += [ROOT / "tools/stdlib/runtime.c", ROOT / "tools/stdlib/runtime_abi.h"]
    report = {"schema": 1, "result": "running", "stage": "scalar-mir-native-consumer",
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "host": platform.platform(), "sources": {str(p.relative_to(ROOT)): sha(p) for p in inputs if p.is_file()},
              "tools": {}, "steps": [], "m4_complete": False,
              "limits": ["validated scalar/aggregate MIR with resolved root/structural Drop; pending runtime glue, field cleanup across SG calls, async and string data remain unsupported",
                         "stack-slot allocation; frames over 4096 bytes call __chkstk through a checked REL32 placeholder",
                         "fixed-stack unwind records; exception cleanup handlers and CodeView remain open",
                         "defined scalar division/remainder and tested ABI shapes only; shift fixtures use defined in-width counts",
                         "SG emits bytes directly; external tools only independently inspect/link the verification caller",
                         "not an SG linker, complete backend or self-compilation fixed point"]}
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
            env.pop(name)
    env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"),
               SENGOO_CLANG=str(paths["clang"]), SENGOO_MODULE_MAP=";".join(
                   "selfhost_" + name + "=" + str(ROOT / "selfhost/src" / (name + ".sg")) for name in names))
    env["PATH"] = str(paths["clang"].parent) + os.pathsep + env.get("PATH", "")

    def run(name, command, marker=None, code=0):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=600)
        (out / (name + ".stdout.log")).write_bytes(done.stdout)
        (out / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)), "exit_code": done.returncode,
                                "expected_exit_code": code, "seconds": time.monotonic() - started})
        if done.returncode != code or marker and marker not in done.stdout + done.stderr:
            raise AssertionError(f"{name} failed: {done.returncode}")
        print(f"{name}: {done.returncode}", flush=True)
        return done.stdout

    try:
        for name, path in paths.items():
            digest = sha(path)
            if digest != lock[name]["sha256"]:
                raise ValueError(f"{name} identity mismatch")
            report["tools"][name] = {"path": str(path), "sha256": digest}
        for name in ("src/native.sg", "src/x64_scalar.sg", "src/x64_sse.sg", "src/unwind.sg", "src/layout.sg", "src/abi.sg", "tests/abi.sg", "tests/native.sg"):
            run("format-" + name.replace("/", "-"), [paths["formatter"], "--check", ROOT / "selfhost" / name])
        abi_binary = out / "abi-tests.exe"
        run("build-abi", [paths["seed"], "--runtime-mode", "source-development", "build",
                          ROOT / "selfhost/tests/abi.sg", "--force-rebuild", "-O", "0", "-o", abi_binary])
        run("native-abi", [abi_binary], b"selfhost Windows x64 ABI classification: ok")
        binary = out / "native-tests.exe"
        binary.unlink(missing_ok=True)
        run("build-sg", [paths["seed"], "--runtime-mode", "source-development", "build",
                         ROOT / "selfhost/tests/native.sg", "--force-rebuild", "-O", "0", "-o", binary])
        run("native-sg", [binary], b"selfhost scalar MIR native objects: ok")
        checked, repeated, wrapping = (out / name for name in ("checked.obj", "repeated.obj", "wrapping.obj"))
        for name, path, extra in (("checked", checked, []), ("repeat", repeated, []), ("wrapping", wrapping, ["wrap"])):
            path.unlink(missing_ok=True)
            run("emit-" + name, [binary, path, *extra])
        if checked.read_bytes() != repeated.read_bytes() or checked.read_bytes()[4:8] != bytes(4):
            raise AssertionError("object bytes are not deterministic")
        inspector = paths["clang"].parent / "llvm-readobj.exe"
        disassembler = paths["clang"].parent / "llvm-objdump.exe"
        for name, path in (("inspector", inspector), ("disassembler", disassembler)):
            report["tools"][name] = {"path": str(path), "sha256": sha(path)}
        run("inspect", [inspector, "--file-headers", "--sections", "--symbols", "--relocations", "--unwind", checked], b"ALLOC_SMALL")
        run("disassemble", [disassembler, "-d", checked], b"sg_native_loop")
        probe, wrap_probe = out / "native-probe.exe", out / "wrap-probe.exe"
        for name, obj, exe in (("checked", checked, probe), ("wrap", wrapping, wrap_probe)):
            run("link-" + name, [paths["clang"], ROOT / "selfhost/tests/native_probe.c", obj, "-Werror", "-o", exe])
        executed = run("execute-checked", [probe], b"native MIR calls, loop, pointers, narrow memory and checked arithmetic: ok")
        if b"native unwind metadata: ok" not in executed:
            raise AssertionError("Windows stack walk did not traverse generated function metadata")
        if b"native large stack probe: ok" not in executed:
            raise AssertionError("large stack-frame probe did not execute")
        if b"native float arithmetic, NaN and mixed Windows ABI: ok" not in executed:
            raise AssertionError("float and mixed-ABI cases did not execute")
        if b"native aggregate Windows parameters and return pointers: ok" not in executed:
            raise AssertionError("aggregate C ABI cases did not execute")
        if b"native saturating casts, NaN, ranges and integer rounding: ok" not in executed:
            raise AssertionError("numeric conversion cases did not execute")
        if b"native signed and unsigned division and remainder: ok" not in executed:
            raise AssertionError("division/remainder cases did not execute")
        if b"native conditional initialization and exact user Drop: ok" not in executed:
            raise AssertionError("conditional owner cleanup cases did not execute")
        if b"native nested and array reverse cleanup with cleared flags: ok" not in executed:
            raise AssertionError("nested/array cleanup order or cleared initialization flag failed")
        if b"native active enum payload cleanup: ok" not in executed:
            raise AssertionError("enum cleanup touched an inactive payload or repeated a drop")
        for case in ("overflow", "overflow8", "umul-overflow", "uadd-overflow", "mul-overflow"):
            run("reject-" + case, [probe, case], b"integer overflow", 63)
        run("reject-division-zero", [probe, "division-zero"], b"division by zero", 62)
        run("reject-division-overflow", [probe, "division-overflow"], b"integer overflow", 63)
        run("execute-wrap", [wrap_probe, "wrap"], b"native wrapping arithmetic: ok")
        report["artifacts"] = {p.name: sha(p) for p in (abi_binary, binary, checked, repeated, wrapping, probe, wrap_probe)}
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
            report.update(result="failed", error="native inputs changed during verification")
        (out / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("native inputs changed")


if __name__ == "__main__":
    main()
