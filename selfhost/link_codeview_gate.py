"""Verify actual COFF CodeView emission, relocation, decoding and live debugging."""
from __future__ import annotations

import argparse
import datetime as dt
import json
import os
from pathlib import Path
import platform
import re
import struct
import subprocess
import sys
import time

from link_debug_gate import ROOT, MODULES as DEBUG_MODULES, sha, u16, u32, validate_pair

MODULES = DEBUG_MODULES + ("link_codeview",)


def mutations(raw):
    sections = {}
    for index in range(u16(raw, 2)):
        at = 20 + index * 40
        name = raw[at:at + 8].rstrip(b"\0")
        sections[name] = (at, u32(raw, at + 20), u32(raw, at + 16))
    header, start, size = sections[b".debug$S"]
    fragments, symbols, ranges = {}, {}, []
    at = start + 4
    while at < start + size:
        kind, length = struct.unpack_from("<II", raw, at)
        fragments[kind] = (at, at + 8, length)
        if kind == 0xF1:
            symbol = at + 8
            while symbol < at + 8 + length:
                symbols.setdefault(u16(raw, symbol + 2), symbol)
                if u16(raw, symbol + 2) == 0x1142:
                    ranges.append(symbol)
                symbol += u16(raw, symbol) + 2
        at = (at + length + 11) & ~3

    def change(at, value, fmt="<I"):
        changed = bytearray(raw)
        struct.pack_into(fmt, changed, at, value)
        return bytes(changed)

    cases = [
        ("subsection-kind", change(start + 4, 0xF6), 7303),
        ("subsection-size", change(start + 8, 0xFFFFFFFF), 7301),
        ("signature", change(start, 2), 7303),
        ("symbol-kind", change(symbols[0x1101] + 2, 0x7777, "<H"), 7303),
        ("symbol-length", change(symbols[0x1101], 0, "<H"), 7301),
        ("procedure-type", change(symbols[0x1110] + 28, 0x1002), 7303),
        ("frame-flags", change(symbols[0x1012] + 26, 0), 7303),
        ("local-type", change(symbols[0x113E] + 4, 0x40), 7303),
        ("local-stack-range", change(symbols[0x1142] + 4, 0xFFFFFFFF), 7301),
        ("local-lifetime", change(symbols[0x1142] + 14, 65535, "<H"), 7301),
        ("source-checksum", change(fragments[0xF4][1] + 6, raw[fragments[0xF4][1] + 6] ^ 128, "<B"), 7301),
        ("checksum-algorithm", change(fragments[0xF4][1] + 5, 1, "<B"), 7303),
        ("line-offset", change(fragments[0xF2][1] + 24, 29), 7301),
        ("line-file", change(fragments[0xF2][1] + 12, 1), 7303),
        ("line-count", change(fragments[0xF2][1] + 16, 65537), 7301),
        ("line-end-delta", change(fragments[0xF2][1] + 28, 0x81000001), 7303),
        ("type-leaf", change(sections[b".debug$T"][1] + 6, 0x1505, "<H"), 7303),
        ("source-name", change(fragments[0xF3][1] + 1, ord("X"), "<B"), 7301),
        ("unknown-debug-section", change(sections[b".debug$T"][0] + 7, ord("X"), "<B"), 7303),
    ]
    if len(ranges) > 1:
        cases += [("second-range-overlap", change(ranges[1] + 8, 38), 7301),
                  ("second-range-extent", change(ranges[1] + 14, 18, "<H"), 7301),
                  ("second-range-zero", change(ranges[1] + 14, 0, "<H"), 7301),
                  ("second-range-stack-base", change(ranges[1] + 4, 41), 7303)]
    return cases


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter", "lldb-python-home", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--ranges", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("output must stay in target/")
    output.mkdir(parents=True, exist_ok=True)
    tools = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter")}
    tools.update(debugger=tools["clang"].parent / "lldb.exe", pdb_inspector=tools["clang"].parent / "llvm-pdbutil.exe", coff_inspector=tools["clang"].parent / "llvm-readobj.exe")
    python_home = args.lldb_python_home.resolve()
    for path in sorted(python_home.glob("*.pyd")) + sorted(python_home.glob("*.dll")) + [python_home / "python310.zip", tools["clang"].parent / "liblldb.dll"]:
        tools["debug-dependency-" + path.name] = path
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
    source = ROOT / "selfhost/tests" / ("link_debug_ranges_source.sg" if args.ranges else "link_debug_source.sg")
    inputs = [Path(__file__).resolve(), ROOT / "selfhost/link_debug_gate.py", ROOT / "selfhost/tests/link_codeview.sg", source, ROOT / "selfhost/bootstrap.lock", ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "rust-toolchain.toml"]
    inputs += [ROOT / "selfhost/src" / (name + ".sg") for name in MODULES]
    if args.ranges:
        inputs.append(ROOT / "selfhost/tests/link_debug_ranges.sg")
        inputs.append(ROOT / "selfhost/tests/link_debug_ranges_probe.c")
    for folder in (ROOT / "tools/stdlib", ROOT / "runtime"):
        inputs += [path for path in folder.rglob("*") if path.is_file() and not any(part in {"target", "build", "__pycache__"} for part in path.relative_to(folder).parts)]
    revision = lambda: subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    report = {"schema": 1, "result": "running", "stage": "sg-coff-codeview-pdb-checkpoint", "m5_complete": False,
              "source_revision": revision(), "started_at": dt.datetime.now(dt.timezone.utc).isoformat(), "host": platform.platform(), "python": sys.version,
              "sources": {str(p.relative_to(ROOT)): sha(p) for p in sorted(set(inputs))}, "tools": {}, "steps": [],
              "limits": ["the SG writer emits real .debug$S/.debug$T and SECTION/SECREL relocations; the SG reader consumes actual relocated contributions",
                         "one module with named i64/no-argument procedure prototype, SP-relative i64 locals and SHA256 source checksums; unknown debug symbols/types/subsections/sections fail",
                         "COFF auxiliary/COMDAT/weak/full SDK CodeView, multiple modules, aggregate/generic debug types and automatic native compiler debug mapping remain open",
                         "debug contributions currently remain in mapped PE sections; complete debug stripping/public CLI policy is later integration work"]}
    report["multiple_return_ranges"] = args.ranges
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_", "PYTHON", "LLDB_")):
            env.pop(name)
    env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(tools["clang"]),
               SENGOO_MODULE_MAP=";".join("selfhost_" + name + "=" + str(ROOT / "selfhost/src" / (name + ".sg")) for name in MODULES))
    env["PATH"] = str(tools["clang"].parent) + os.pathsep + env.get("PATH", "")
    empty = output / "empty-toolchain-path"
    empty.mkdir(exist_ok=True)
    pure_env = {key: value for key, value in env.items() if not key.upper().startswith("SENGOO_")}
    pure_env["PATH"] = str(empty)
    debug_env = dict(env, PYTHONHOME=str(python_home), PYTHONPATH=os.pathsep.join((str(python_home / "python310.zip"), str(python_home))), LLDB_USE_NATIVE_PDB_READER="1")
    debug_env["PATH"] = str(python_home) + os.pathsep + debug_env["PATH"]

    def run(name, command, code=0, marker=None, pure=False, debugger=False):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=debug_env if debugger else pure_env if pure else env, capture_output=True, timeout=600)
        (output / (name + ".stdout.log")).write_bytes(done.stdout)
        (output / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)), "exit_code": done.returncode, "expected_exit_code": code, "seconds": time.monotonic() - started, "toolchain_unavailable_path": pure})
        if done.returncode != code or marker and marker not in done.stdout + done.stderr:
            raise AssertionError(f"{name}: {done.returncode}")
        print(name, done.returncode, flush=True)
        return done.stdout + done.stderr

    try:
        for name, path in tools.items():
            digest = sha(path)
            if name in lock and digest != lock[name]["sha256"]:
                raise ValueError(name + " identity mismatch")
            report["tools"][name] = {"path": str(path), "sha256": digest}
        for name in MODULES:
            for imported in re.findall(r"import\s+selfhost_([a-zA-Z0-9_]+)\s*;", (ROOT / "selfhost/src" / (name + ".sg")).read_text()):
                if imported not in MODULES:
                    raise AssertionError("unrecorded module dependency")
        format_paths = [ROOT / "selfhost/src/link_codeview.sg", ROOT / "selfhost/tests/link_codeview.sg"]
        if args.ranges:
            format_paths.append(ROOT / "selfhost/tests/link_debug_ranges.sg")
        for path in format_paths:
            run("format-" + path.stem, [tools["formatter"], "--check", path])
        binary = output / "sg-codeview.exe"
        run("build-sg", [tools["seed"], "--runtime-mode", "source-development", "build", ROOT / "selfhost/tests/link_codeview.sg", "--force-rebuild", "-O", "0", "-o", binary])
        reader = binary
        if args.ranges:
            binary = output / "sg-ranges.exe"
            run("build-sg-ranges", [tools["seed"], "--runtime-mode", "source-development", "build", ROOT / "selfhost/tests/link_debug_ranges.sg", "--force-rebuild", "-O", "0", "-o", binary])
        object_path, image, pdb = output / "debug-input.obj", output / "codeview-linked.exe", output / "codeview-linked.pdb"
        run("emit-coff-decode-and-exact-resources", [binary, object_path.as_posix(), image.as_posix(), pdb.as_posix(), source.as_posix()], pure=True, marker=b"selfhost SG RBP/multiple-return CodeView ranges: ok" if args.ranges else b"selfhost SG COFF CodeView remapping: ok")
        original_object, original_image, original_pdb = object_path.read_bytes(), image.read_bytes(), pdb.read_bytes()
        run("read-existing-coff-repeat", [reader, "read", object_path.as_posix(), image.as_posix(), pdb.as_posix(), source.as_posix()], pure=True)
        if original_image != image.read_bytes() or original_pdb != pdb.read_bytes():
            raise AssertionError("COFF round-trip PE/PDB bytes differ")
        expected_map = {"expected_code_size": 61, "expected_lines": ((0, 1), (11, 2), (19, 3), (30, 4), (35, 5), (44, 7), (52, 8))} if args.ranges else {}
        report["pair"] = validate_pair(original_image, original_pdb, source.read_bytes(), pdb, **expected_map)
        run("execute-codeview-linked-pe", [image], 42, pure=True)
        inspected = run("inspect-native-coff-codeview", [tools["coff_inspector"], "--codeview", "--relocations", object_path])
        for marker in (b"IMAGE_REL_AMD64_SECTION", b"IMAGE_REL_AMD64_SECREL", b"S_GPROC32", b"S_LOCAL", b"SHA256", b"FunctionLineTable"):
            if marker not in inspected:
                raise AssertionError("COFF inspector marker missing: " + repr(marker))
        pdb_details = run("inspect-decoded-pdb", [tools["pdb_inspector"], "dump", "--summary", "--modules", "--symbols", "--types", "--publics", "--globals", pdb], marker=b"answer")
        if args.ranges and (pdb_details.count(b"S_DEFRANGE_FRAMEPOINTER_REL") != 2 or b"+20)" not in pdb_details or b"+12)" not in pdb_details):
            raise AssertionError("multiple-return lifetime ranges differ")
        if args.ranges:
            run("inspect-rbp-unwind", [tools["coff_inspector"], "--unwind", image], marker=b"PUSH_NONVOL reg=RBP")
            probe = output / "rbp-unwind-probe.exe"
            run("build-independent-rbp-probe", [tools["clang"], ROOT / "selfhost/tests/link_debug_ranges_probe.c", "-Werror", "-o", probe])
            run("windows-rbp-and-two-epilogues-unwind", [probe, image], marker=b"SG RBP frame and both epilogues Windows runtime unwind: ok")
        commands = ["breakpoint set --file " + source.name + " --line " + ("4" if args.ranges else "3"), "run", "frame variable answer", "thread step-over", "frame variable answer", "continue"]
        debugged = run("live-source-breakpoint-step-local", [tools["debugger"], "--batch", "--no-lldbinit", *[item for command in commands for item in ("-o", command)], image], debugger=True)
        for marker in (b"stop reason = breakpoint", b"answer = 40", b"stop reason = step over", b"answer = 42", b"exited with status = 42"):
            if marker not in debugged:
                raise AssertionError("debugger evidence missing: " + repr(marker))
        if args.ranges:
            gap_commands = ["breakpoint set --file " + source.name + " --line 5", "run", "frame variable answer", "thread step-inst", "frame variable answer", "continue"]
            gap = run("live-epilogue-lifetime-gap", [tools["debugger"], "--batch", "--no-lldbinit", *[item for command in gap_commands for item in ("-o", command)], image], debugger=True)
            for marker in (b"answer = 42", b"answer = <variable not available>", b"exited with status = 42"):
                if marker not in gap:
                    raise AssertionError("epilogue local lifetime evidence missing: " + repr(marker))
        negatives = []
        for name, raw, expected in mutations(original_object):
            path = output / (name + ".obj")
            path.write_bytes(raw)
            sentinel = b"preserve prior debug artifact"
            image.write_bytes(sentinel)
            pdb.write_bytes(sentinel)
            result = run("reject-" + name, [reader, "read", path, image.as_posix(), pdb.as_posix(), source.as_posix()], 65, pure=True)
            if result.strip() != str(expected).encode() or image.read_bytes() != sentinel or pdb.read_bytes() != sentinel:
                raise AssertionError(name + " diagnostic/output preservation differs: " + repr(result))
            negatives.append({"name": name, "expected_code": expected, "input_sha256": sha(path), "previous_outputs_preserved": True})
        image.write_bytes(original_image)
        pdb.write_bytes(original_pdb)
        report.update(result="passed", negative_cases=negatives, native_resource_scopes=20, artifacts={path.name: sha(path) for path in (binary, reader, object_path, image, pdb)})
        if args.ranges:
            report["artifacts"][probe.name] = sha(probe)
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items() if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        report.update(changed_sources=changed, changed_tools=changed_tools, source_revision_end=revision(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if changed or changed_tools:
            report.update(result="failed", error="CodeView input identity changed")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("CodeView input identity changed")


if __name__ == "__main__":
    main()
