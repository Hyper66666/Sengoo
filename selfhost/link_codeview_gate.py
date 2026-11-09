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
        ("procedure-type", change(symbols[0x1110] + 28, 0x1000), 7303),
        ("frame-flags", change(symbols[0x1012] + 26, 0), 7303),
        ("local-type", change(symbols[0x113E] + 4, 14), 7303),
        ("local-stack-range", change(symbols[0x1142] + 4, 0xFFFFFFFF), 7301),
        ("local-lifetime", change(symbols[0x1142] + 14, 65535, "<H"), 7301),
        ("source-checksum", change(fragments[0xF4][1] + 6, raw[fragments[0xF4][1] + 6] ^ 128, "<B"), 7301),
        ("checksum-algorithm", change(fragments[0xF4][1] + 5, 1, "<B"), 7303),
        ("line-offset", change(fragments[0xF2][1] + 24, 29), 7301),
        ("line-file", change(fragments[0xF2][1] + 12, 1), 7303),
        ("line-count", change(fragments[0xF2][1] + 16, 65537), 7301),
        ("line-end-delta", change(fragments[0xF2][1] + 28, 0x81000001), 7303),
        ("type-leaf", change(sections[b".debug$T"][1] + 6, 0x1601, "<H"), 7303),
        ("source-name", change(fragments[0xF3][1] + 1, ord("X"), "<B"), 7301),
        ("unknown-debug-section", change(sections[b".debug$T"][0] + 7, ord("X"), "<B"), 7303),
    ]
    if len(ranges) > 1:
        cases += [("second-range-overlap", change(ranges[1] + 8, 38), 7301),
                  ("second-range-extent", change(ranges[1] + 14, 18, "<H"), 7301),
                  ("second-range-zero", change(ranges[1] + 14, 0, "<H"), 7301),
                  ("second-range-stack-base", change(ranges[1] + 4, 41), 7303)]
    return cases


def type_mutations(raw):
    sections = {}
    for index in range(u16(raw, 2)):
        row = 20 + index * 40
        sections[raw[row:row + 8].rstrip(b"\0")] = (u32(raw, row + 20), u32(raw, row + 16))
    start, size = sections[b".debug$S"]
    at, locals_ = start + 4, []
    while at < start + size:
        kind, length = struct.unpack_from("<II", raw, at)
        if kind == 0xF1:
            symbol = at + 8
            while symbol < at + length + 8:
                if u16(raw, symbol + 2) == 0x113E:
                    locals_.append(symbol)
                symbol += u16(raw, symbol) + 2
        at = (at + length + 11) & ~3
    at, size = sections[b".debug$T"]
    end = at + size
    at += 4
    arguments = procedure = None
    while at < end:
        length, kind = struct.unpack_from("<HH", raw, at)
        if kind == 0x1201 and u32(raw, at + 4):
            arguments = at
        if kind == 0x1008 and u16(raw, at + 10):
            procedure = at
        at += length + 2
    if len(locals_) != 3 or arguments is None or procedure is None:
        raise AssertionError("typed fixture structure differs")

    def change(at, value, fmt="<I"):
        changed = bytearray(raw)
        struct.pack_into(fmt, changed, at, value)
        return bytes(changed)

    return [
        ("parameter-flags", change(locals_[0] + 8, 2, "<H"), 7303),
        ("parameter-prototype-type", change(locals_[0] + 4, 0x40), 7301),
        ("parameter-missing-flag", change(locals_[0] + 8, 0, "<H"), 7301),
        ("parameter-extra-flag", change(locals_[2] + 8, 1, "<H"), 7301),
        ("argument-count", change(arguments + 4, 3), 7301),
        ("argument-void", change(arguments + 8, 3), 7303),
        ("procedure-parameter-count", change(procedure + 10, 3, "<H"), 7301),
        ("procedure-result-kind", change(procedure + 4, 14), 7303),
        ("procedure-result-reference", change(procedure + 4, 0x1505), 7301),
        ("procedure-forward-list", change(procedure + 12, 0x1003), 7301),
        ("procedure-calling-convention", change(procedure + 8, 1, "<H"), 7303),
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter", "lldb-python-home", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--ranges", action="store_true")
    parser.add_argument("--primitives", action="store_true")
    parser.add_argument("--references", action="store_true")
    parser.add_argument("--aggregates", action="store_true")
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("output must stay in target/")
    output.mkdir(parents=True, exist_ok=True)
    std_snapshot = output / "producer-snapshot/tools/stdlib"
    std_origins, std_snapshots = {}, {}
    for path in sorted((ROOT / "tools/stdlib").rglob("*")):
        if not path.is_file() or any(part in {"target", "build", "__pycache__"} for part in path.relative_to(ROOT / "tools/stdlib").parts):
            continue
        destination = std_snapshot / path.relative_to(ROOT / "tools/stdlib")
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(path.read_bytes())
        std_origins[str(path.relative_to(ROOT))] = sha(destination)
        std_snapshots[str(destination.relative_to(ROOT))] = sha(destination)
    if any(sha(ROOT / name) != digest for name, digest in std_origins.items()):
        raise AssertionError("stdlib changed while capturing producer snapshot")
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
    if args.primitives:
        inputs += [ROOT / "selfhost/tests" / name for name in ("link_cv_types.sg", "link_cv_types_debug.sg", "link_native_debug_types_source.sg")]
    if args.references:
        if not args.primitives:
            raise ValueError("--references requires --primitives")
        inputs.append(ROOT / "selfhost/tests/link_cv_pointer_probe.cpp")
    if args.aggregates:
        if not args.references:
            raise ValueError("--aggregates requires --references")
        inputs += [ROOT / "selfhost/tests" / name for name in ("link_cv_aggregate.sg", "link_cv_aggregate_probe.cpp")]
    inputs += [ROOT / name for name in std_snapshots]
    for folder in (ROOT / "runtime",):
        inputs += [path for path in folder.rglob("*") if path.is_file() and not any(part in {"target", "build", "__pycache__"} for part in path.relative_to(folder).parts)]
    revision = lambda: subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    report = {"schema": 1, "result": "running", "stage": "sg-coff-codeview-pdb-checkpoint", "m5_complete": False,
              "source_revision": revision(), "started_at": dt.datetime.now(dt.timezone.utc).isoformat(), "host": platform.platform(), "python": sys.version,
              "sources": {str(p.relative_to(ROOT)): sha(p) for p in sorted(set(inputs))}, "tools": {}, "steps": [],
              "producer_stdlib_origins": std_origins, "producer_stdlib_snapshots": std_snapshots,
              "limits": ["the SG writer emits real .debug$S/.debug$T and SECTION/SECREL relocations; the SG reader consumes actual relocated contributions",
                         "one module with named i64/no-argument procedure prototype, SP-relative i64 locals and SHA256 source checksums; unknown debug symbols/types/subsections/sections fail",
                         "COFF auxiliary/COMDAT/weak/full SDK CodeView, multiple modules, aggregate/generic debug types and automatic native compiler debug mapping remain open",
                         "debug contributions currently remain in mapped PE sections; complete debug stripping/public CLI policy is later integration work"]}
    report["multiple_return_ranges"] = args.ranges
    report["primitive_signatures"] = args.primitives
    report["pointer_reference_types"] = args.references
    report["aggregate_types"] = args.aggregates
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_", "PYTHON", "LLDB_")):
            env.pop(name)
    env.update(SENGOO_STDLIB=str(std_snapshot), SENGOO_RUNTIME=str(std_snapshot / "runtime.c"), SENGOO_CLANG=str(tools["clang"]),
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
        if args.primitives:
            format_paths += [ROOT / "selfhost/src/link_cv_types.sg", ROOT / "selfhost/tests/link_cv_types.sg", ROOT / "selfhost/tests/link_cv_types_debug.sg"]
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
        if args.primitives:
            type_test = output / "cv-types.exe"
            run("build-primitive-type-pool", [tools["seed"], "--runtime-mode", "source-development", "build", ROOT / "selfhost/tests/link_cv_types.sg", "--force-rebuild", "-O", "0", "-o", type_test])
            run("primitive-type-pool-remap-100-resources", [type_test], marker=b"selfhost SG CodeView primitive signature remapping: ok", pure=True)
            type_debug = output / "typed-debug.exe"
            run("build-typed-codeview-fixture", [tools["seed"], "--runtime-mode", "source-development", "build", ROOT / "selfhost/tests/link_cv_types_debug.sg", "--force-rebuild", "-O", "0", "-o", type_debug])
            typed_source = ROOT / "selfhost/tests/link_native_debug_types_source.sg"
            typed_obj, typed_image, typed_pdb = output / "typed-input.obj", output / "typed-linked.exe", output / "typed-linked.pdb"
            typed_command = [type_debug, *[path.as_posix() for path in (typed_source, typed_obj, typed_image, typed_pdb)]]
            run("emit-typed-coff-pdb-20-resources", typed_command, marker=b"selfhost SG typed parameter/local PDB exact resources: ok", pure=True)
            typed_bytes = [path.read_bytes() for path in (typed_obj, typed_image, typed_pdb)]
            run("typed-byte-determinism", typed_command, pure=True)
            if typed_bytes != [path.read_bytes() for path in (typed_obj, typed_image, typed_pdb)]:
                raise AssertionError("typed COFF/PE/PDB bytes differ on repeat")
            run("execute-typed-linked-image", [typed_image], 42, pure=True)
            run("inspect-typed-coff", [tools["coff_inspector"], "--codeview", "--relocations", typed_obj], marker=b"double")
            types_dump = run("inspect-typed-pdb", [tools["pdb_inspector"], "dump", "--types", "--symbols", typed_pdb])
            for marker in (b"double (double, double)", b"flags = param", b"left", b"right", b"answer"):
                if marker not in types_dump:
                    raise AssertionError("typed PDB inspection missing " + repr(marker))
            typed_commands = ["breakpoint set --file " + typed_source.name + " --line 3", "run", "frame variable left right answer", "thread step-over", "frame variable answer", "continue"]
            typed_debugged = run("live-typed-parameters-locals", [tools["debugger"], "--batch", "--no-lldbinit", *[item for command in typed_commands for item in ("-o", command)], typed_image], debugger=True)
            for marker in (b"(double) left = 20", b"(double) right = 20", b"(double) answer = 40", b"(double) answer = 42", b"exited with status = 42"):
                if marker not in typed_debugged:
                    raise AssertionError("typed debugger evidence missing " + repr(marker))
            for name, raw, expected in type_mutations(typed_bytes[0]):
                path = output / (name + ".obj")
                path.write_bytes(raw)
                sentinel = b"preserve previous typed debug outputs"
                typed_image.write_bytes(sentinel)
                typed_pdb.write_bytes(sentinel)
                result = run("reject-typed-" + name, [reader, "read", path, typed_image.as_posix(), typed_pdb.as_posix(), typed_source.as_posix()], 65, pure=True)
                if result.strip() != str(expected).encode() or typed_image.read_bytes() != sentinel or typed_pdb.read_bytes() != sentinel:
                    raise AssertionError("typed failure diagnostic/output differs: " + repr(result))
                report["negative_cases"].append({"name": name, "expected_code": expected, "input_sha256": sha(path), "previous_outputs_preserved": True})
            typed_image.write_bytes(typed_bytes[1])
            typed_pdb.write_bytes(typed_bytes[2])
            report["artifacts"].update({path.name: sha(path) for path in (type_test, type_debug, typed_obj, typed_image, typed_pdb)})
            report["primitive_resource_scopes"] = {"type_pool": 100, "typed_coff_pdb_per_invocation": 20}
            if args.references:
                independent = output / "clang-pointer.obj"
                run("build-independent-pointer-types", [tools["clang"], "-g", "-gcodeview", "-O0", "-Werror", "-c", ROOT / "selfhost/tests/link_cv_pointer_probe.cpp", "-o", independent])
                run("inspect-independent-pointer-types", [tools["coff_inspector"], "--codeview", independent], marker=b"const __int64&")
                raw = independent.read_bytes()
                types_data = None
                for index in range(u16(raw, 2)):
                    row = 20 + index * 40
                    if raw[row:row + 8] == b".debug$T":
                        at, size = u32(raw, row + 20), u32(raw, row + 16)
                        types_data = raw[at:at + size]
                if types_data is None:
                    raise AssertionError("independent COFF type stream missing")
                end = 4
                for expected_kind in (0x1001, 0x1002, 0x1201, 0x1008):
                    length, kind = struct.unpack_from("<HH", types_data, end)
                    if kind != expected_kind:
                        raise AssertionError("independent pointer prefix layout differs")
                    end += length + 2
                prefix, roundtrip = output / "independent-pointer-prefix.cv", output / "sg-pointer-prefix.cv"
                prefix.write_bytes(types_data[:end])
                run("sg-remap-independent-pointer-types", [type_test, prefix, roundtrip], pure=True)
                if prefix.read_bytes() != roundtrip.read_bytes():
                    raise AssertionError("SG pointer/modifier encoding differs from clang's prefix")
                duplicate_prefix = output / "64-record-type-prefix.cv"
                duplicate_prefix.write_bytes(types_data[:4] + types_data[4:end] * 16)
                run("sg-deduplicate-64-valid-type-records", [type_test, duplicate_prefix, roundtrip], pure=True)
                if prefix.read_bytes() != roundtrip.read_bytes():
                    raise AssertionError("valid duplicate type records did not canonicalize")
                full = output / "unsupported-full-clang-types.cv"
                full.write_bytes(types_data)
                sentinel = b"preserve previous type output"
                roundtrip.write_bytes(sentinel)
                rejected = run("reject-unsupported-clang-type-leaves", [type_test, full, roundtrip], 65, pure=True)
                if rejected.strip() != b"7323" or roundtrip.read_bytes() != sentinel:
                    raise AssertionError("unsupported native type leaves were silently accepted")
                report["negative_cases"].append({"name": "unsupported-clang-type-leaves", "expected_code": 7323, "input_sha256": sha(full), "previous_outputs_preserved": True})
                roundtrip.write_bytes(prefix.read_bytes())
                pointer_negatives = [("pointer-size", 12 + 8, 32780, 7323), ("pointer-mode", 12 + 8, 65612, 7323),
                                     ("pointer-forward-target", 12 + 4, 4097, 7321), ("modifier-flags", 8, 3, 7323),
                                     ("modifier-forward-target", 4, 4096, 7321)]
                for name, offset, value, expected in pointer_negatives:
                    changed = bytearray(types_data[:end])
                    struct.pack_into("<I" if "flags" not in name else "<H", changed, 4 + offset, value)
                    path = output / (name + ".cv")
                    path.write_bytes(changed)
                    roundtrip.write_bytes(sentinel)
                    rejected = run("reject-" + name, [type_test, path, roundtrip], 65, pure=True)
                    if rejected.strip() != str(expected).encode() or roundtrip.read_bytes() != sentinel:
                        raise AssertionError("pointer/modifier failure diagnostic/output differs: " + repr(rejected))
                    report["negative_cases"].append({"name": name, "expected_code": expected, "input_sha256": sha(path), "previous_outputs_preserved": True})
                roundtrip.write_bytes(prefix.read_bytes())
                for count, modified, accepted in ((64, False, True), (65, False, False), (63, True, True), (64, True, False)):
                    chain = bytearray(struct.pack("<I", 4))
                    base = 19
                    for index in range(count):
                        chain += struct.pack("<HHII", 10, 0x1002, base, 65548)
                        base = 4096 + index
                    if modified:
                        chain += struct.pack("<HHIHH", 10, 0x1001, base, 1, 0xF1F2)
                    name = f"type-depth-{count}-{'modifier' if modified else 'pointer'}"
                    path = output / (name + ".cv")
                    path.write_bytes(chain)
                    roundtrip.write_bytes(sentinel)
                    result = run(name, [type_test, path, roundtrip], 0 if accepted else 65, pure=True)
                    if accepted:
                        if roundtrip.read_bytes() != chain:
                            raise AssertionError("maximum supported pointer/modifier depth differs")
                    else:
                        if result.strip() != b"7323" or roundtrip.read_bytes() != sentinel:
                            raise AssertionError("unsupported pointer/modifier depth was accepted")
                        report["negative_cases"].append({"name": name, "expected_code": 7323, "input_sha256": sha(path), "previous_outputs_preserved": True})
                for length in (2, 6):
                    path = output / (f"short-pointer-{length}.cv")
                    path.write_bytes(struct.pack("<IHH", 4, length, 0x1002) + bytes(length - 2))
                    roundtrip.write_bytes(sentinel)
                    result = run(f"reject-short-pointer-{length}", [type_test, path, roundtrip], 65, pure=True)
                    if result.strip() != b"7321" or roundtrip.read_bytes() != sentinel:
                        raise AssertionError("short pointer record diagnostic/output differs")
                    report["negative_cases"].append({"name": f"short-pointer-{length}", "expected_code": 7321, "input_sha256": sha(path), "previous_outputs_preserved": True})
                roundtrip.write_bytes(prefix.read_bytes())
                report["artifacts"].update({path.name: sha(path) for path in (independent, prefix, roundtrip, full, duplicate_prefix)})
                if args.aggregates:
                    aggregate_test = output / "aggregate-types.exe"
                    run("build-aggregate-type-pool", [tools["seed"], "--runtime-mode", "source-development", "build", ROOT / "selfhost/tests/link_cv_aggregate.sg", "-o", aggregate_test])
                    run("aggregate-type-pool-100-exact-resources", [aggregate_test], marker=b"aggregate types: ok", pure=True)
                    independent = output / "clang-aggregate.obj"
                    run("build-independent-aggregate-types", [tools["clang"], "-g", "-gcodeview", "-O0", "-Werror", "-c", ROOT / "selfhost/tests/link_cv_aggregate_probe.cpp", "-o", independent])
                    run("inspect-independent-aggregate-types", [tools["coff_inspector"], "--codeview", independent], marker=b"FieldOffset: 0x10")
                    raw = independent.read_bytes()
                    independent_types = None
                    for index in range(u16(raw, 2)):
                        row = 20 + index * 40
                        if raw[row:row + 8] == b".debug$T":
                            at, size = u32(raw, row + 20), u32(raw, row + 16)
                            independent_types = raw[at:at + size]
                    if independent_types is None:
                        raise AssertionError("independent aggregate type stream missing")
                    records, at = [], 4
                    while at < len(independent_types):
                        length, kind = struct.unpack_from("<HH", independent_types, at)
                        records.append((kind, independent_types[at:at + length + 2]))
                        at += length + 2
                    member_list = next(data for kind, data in records if kind == 0x1203 and b"flag\0" in data)
                    enum_list = next(data for kind, data in records if kind == 0x1203 and b"Empty\0" in data)
                    array_record = next(data for kind, data in records if kind == 0x1503 and u32(data, 4) == 116)
                    original_struct = next(data for kind, data in records if kind == 0x1505 and u16(data, 4) == 4 and b"Pair\0" in data)
                    original_enum = next(data for kind, data in records if kind == 0x1507 and b"Tag\0" in data)

                    def supported_nominal(data, name_at, fields_at):
                        # Remove unsupported C++ unique-name metadata only. The actual
                        # member records, offsets, size and declared name remain intact.
                        result = bytearray(data[:data.index(0, name_at) + 1])
                        struct.pack_into("<H", result, 6, 0)
                        struct.pack_into("<I", result, fields_at, 4096)
                        padding = -len(result) % 4
                        result += bytes(0xF0 + value for value in range(padding, 0, -1))
                        struct.pack_into("<H", result, 0, len(result) - 2)
                        return bytes(result)

                    structure = supported_nominal(original_struct, 22, 8)
                    enumeration = supported_nominal(original_enum, 16, 12)
                    streams = {"field-list": member_list, "array": array_record, "structure": member_list + structure, "enum": enum_list + enumeration}
                    for label, data in streams.items():
                        prefix = output / ("independent-aggregate-" + label + ".cv")
                        prefix.write_bytes(struct.pack("<I", 4) + data)
                        run("sg-roundtrip-independent-aggregate-" + label, [type_test, prefix, roundtrip], pure=True)
                        if prefix.read_bytes() != roundtrip.read_bytes():
                            raise AssertionError("independent aggregate encoding differs: " + label)
                        report["artifacts"][prefix.name] = sha(prefix)
                    valid = struct.pack("<I", 4) + member_list + structure
                    struct_at = 4 + len(member_list)
                    malformed = [("member-type-forward", 12, "<I", 4096, 7321),
                                 ("member-attributes", 10, "<H", 1, 7323),
                                 ("member-offset", 16, "<H", 25, 7321),
                                 ("structure-count", struct_at + 4, "<H", 3, 7321),
                                 ("structure-properties", struct_at + 6, "<H", 128, 7323),
                                 ("structure-fieldlist-forward", struct_at + 8, "<I", 4097, 7321),
                                 ("structure-derived", struct_at + 12, "<I", 1, 7323),
                                 ("structure-size", struct_at + 20, "<H", 23, 7321)]
                    for label, offset, fmt, value, expected in malformed:
                        changed = bytearray(valid)
                        struct.pack_into(fmt, changed, offset, value)
                        path = output / (label + ".cv")
                        path.write_bytes(changed)
                        roundtrip.write_bytes(sentinel)
                        rejected = run("reject-aggregate-" + label, [type_test, path, roundtrip], 65, pure=True)
                        if rejected.strip() != str(expected).encode() or roundtrip.read_bytes() != sentinel:
                            raise AssertionError("aggregate diagnostic/output differs: " + repr(rejected))
                        report["negative_cases"].append({"name": label, "expected_code": expected, "input_sha256": sha(path), "previous_outputs_preserved": True})
                    report["artifacts"].update({path.name: sha(path) for path in (aggregate_test, independent)})
                    report["aggregate_resource_scopes"] = 100
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items() if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        report.update(changed_sources=changed, changed_tools=changed_tools, source_revision_end=revision(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        report["producer_stdlib_checkout_changes"] = [name for name, digest in std_origins.items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        if changed or changed_tools:
            report.update(result="failed", error="CodeView input identity changed")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("CodeView input identity changed")


if __name__ == "__main__":
    main()
