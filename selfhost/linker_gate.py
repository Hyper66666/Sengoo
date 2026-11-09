"""Verify SG-owned AMD64 COFF resolution and directly emitted PE32+ images."""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import struct
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
MODULES = ("core", "binary", "coff", "object", "link_model", "link_coff", "link_pe")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def u16(raw, offset):
    return struct.unpack_from("<H", raw, offset)[0]


def u32(raw, offset):
    return struct.unpack_from("<I", raw, offset)[0]


def validate_image(raw):
    if raw[:2] != b"MZ" or u32(raw, 60) != 64 or raw[64:68] != b"PE\0\0":
        raise AssertionError("DOS/PE signature differs")
    machine, count, timestamp, symbols_at, symbols, optional, flags = struct.unpack_from("<HHIIIHH", raw, 68)
    if machine != 0x8664 or count != 7 or timestamp or symbols_at or symbols or optional != 240 or flags != 0x22:
        raise AssertionError("PE header identity differs")
    if u16(raw, 88) != 0x20B or struct.unpack_from("<Q", raw, 112)[0] != 0x140000000:
        raise AssertionError("PE32+ base differs")
    if u32(raw, 120) != 4096 or u32(raw, 124) != 512 or u16(raw, 156) != 3:
        raise AssertionError("PE alignment/subsystem differs")
    if u16(raw, 158) & 0x160 != 0x160 or u32(raw, 196) != 16:
        raise AssertionError("NX/ASLR/directory identity differs")
    sections = {}
    for index in range(count):
        row = 328 + 40 * index
        name = raw[row:row + 8].split(b"\0", 1)[0].decode("ascii")
        size, rva, padded, offset = struct.unpack_from("<IIII", raw, row + 8)
        characteristics = u32(raw, row + 36)
        if rva % 4096 or offset % 512 or padded % 512 or size > padded or offset + padded > len(raw):
            raise AssertionError("section extent/alignment differs")
        if any(raw[offset + size:offset + padded]):
            raise AssertionError("section padding is nondeterministic")
        sections[name] = {"index": index + 1, "size": size, "rva": rva, "raw": offset, "flags": characteristics}
    text, data = sections[".text"], sections[".rdata"]
    if struct.unpack_from("<i", raw, text["raw"] + 5)[0] != 23:
        raise AssertionError("cross-object REL32 call differs")
    if struct.unpack_from("<Q", raw, text["raw"] + 11)[0] != 0x140000000 + data["rva"] + 16:
        raise AssertionError("code ADDR64 pointer differs")
    if struct.unpack_from("<Q", raw, data["raw"] + 16)[0] != 0x140000000 + data["rva"] + 8:
        raise AssertionError("data ADDR64 pointer differs")
    if u32(raw, data["raw"] + 24) != data["rva"] + 8 or u16(raw, data["raw"] + 28) != data["index"]:
        raise AssertionError("ADDR32NB/SECTION relocation differs")
    if u32(raw, data["raw"] + 32) != 8 or u32(raw, data["raw"] + 36) != 1234:
        raise AssertionError("SECREL/absolute ADDR32 relocation differs")
    for index in range(5):
        at = data["raw"] + 40 + index * 4
        expected = data["rva"] + 8 - (data["rva"] + 40 + index * 4 + 4 + index + 1)
        if struct.unpack_from("<i", raw, at)[0] != expected:
            raise AssertionError("REL32_N relocation differs")
    imports = sections[".idata"]
    if (u32(raw, 208), u32(raw, 212)) != (imports["rva"], 40):
        raise AssertionError("import directory differs")
    if raw[imports["raw"] + 86:imports["raw"] + 99] != b"KERNEL32.dll\0":
        raise AssertionError("import DLL differs")
    if raw[imports["raw"] + 74:imports["raw"] + 86] != b"ExitProcess\0":
        raise AssertionError("import symbol differs")
    if u32(raw, 224) != sections[".pdata"]["rva"] or u32(raw, 228) != 24:
        raise AssertionError("exception directory differs")
    reloc = sections[".reloc"]
    sites, index = [], 0
    while index < reloc["size"]:
        page, size = struct.unpack_from("<II", raw, reloc["raw"] + index)
        if size < 8 or size % 4 or index + size > reloc["size"]:
            raise AssertionError("base relocation block differs")
        for field in range(8, size, 2):
            value = u16(raw, reloc["raw"] + index + field)
            if value >> 12:
                if value >> 12 != 10:
                    raise AssertionError("unexpected base relocation kind")
                sites.append(page + (value & 4095))
        index += size
    if sites != [text["rva"] + 11, data["rva"] + 16]:
        raise AssertionError("DIR64 site set differs")
    return sections


def mutations(a, b):
    def changed(raw, offset, value, fmt="<I"):
        output = bytearray(raw)
        struct.pack_into(fmt, output, offset, value)
        return bytes(output)

    syms = u32(a, 8)
    first_relocations = u32(a, 20 + 24)
    b_data = u32(b, 60 + 20)
    return [
        ("truncated", a[:19], b, "main", 7201),
        ("wrong-machine", changed(a, 0, 0x14C, "<H"), b, "main", 7203),
        ("bigobj", changed(changed(a, 0, 0, "<H"), 2, 0xFFFF, "<H"), b, "main", 7203),
        ("symbol-count-cap", changed(a, 12, 100001), b, "main", 7202),
        ("raw-overlaps-header", changed(a, 40, 0), b, "main", 7201),
        ("relocations-overlap-symbols", changed(a, 44, syms), b, "main", 7201),
        ("auxiliary-symbol", changed(a, syms + 17, 1, "<B"), b, "main", 7203),
        ("bad-storage", changed(a, syms + 16, 105, "<B"), b, "main", 7203),
        ("bad-symbol-section", changed(a, syms + 12, 0xFFFE, "<H"), b, "main", 7203),
        ("unsupported-comdat", changed(a, 56, u32(a, 56) | 0x1000), b, "main", 7203),
        ("unsupported-bss", changed(a, 56, u32(a, 56) | 0x80), b, "main", 7203),
        ("bad-string-offset", changed(a, syms + 2 * 18 + 4, 0xFFFFFFFF), b, "main", 7201),
        ("relocation-target", changed(a, first_relocations + 4, u32(a, 12)), b, "main", 7201),
        ("relocation-kind", changed(a, first_relocations + 8, 99, "<H"), b, "main", 7203),
        ("relocation-range", changed(a, first_relocations, 29), b, "main", 7201),
        ("overlapping-relocations", changed(a, first_relocations + 10, 7), b, "main", 7201),
        ("truncated-string-table", a[:-1], b, "main", 7201),
        ("duplicate-strong", a, a, "main", 7205),
        ("unresolved", a, None, "main", 7204),
        ("missing-entry", a, b, "no_such_entry", 7204),
        ("data-entry", a, b, "linked_value", 7207),
        ("addr32-overflow", a, changed(b, b_data + 28, 0xFFFFFFFF), "main", 7206),
        ("bad-pdata-overflow", changed(a, u32(a, 140 + 20) + 4, 0xFFFFFFFE), b, "main", 7206),
        ("bad-pdata-range", changed(a, u32(a, 140 + 20) + 4, 0x100000), b, "main", 7207),
        ("entry-at-section-end", changed(a, syms + 8, 30), b, "main", 7207),
    ]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m5-linker-final")
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("output must remain inside target/")
    output.mkdir(parents=True, exist_ok=True)
    tools = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter")}
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text(encoding="utf-8"))
    inputs = [Path(__file__).resolve(), ROOT / "selfhost/bootstrap.lock", ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "rust-toolchain.toml", ROOT / "selfhost/tests/linker.sg", ROOT / "selfhost/tests/linker_probe.c"]
    inputs += [ROOT / "selfhost/src" / (name + ".sg") for name in MODULES]
    for folder in (ROOT / "tools/stdlib", ROOT / "runtime"):
        inputs += [path for path in folder.rglob("*") if path.is_file() and not any(part in {"target", "build", "__pycache__"} for part in path.relative_to(folder).parts)]
    report = {"schema": 1, "result": "running", "stage": "sg-coff-pe-linker-foundation",
              "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "started_at": dt.datetime.now(dt.timezone.utc).isoformat(), "host": platform.platform(), "python": sys.version,
              "sources": {str(p.relative_to(ROOT)): sha(p) for p in sorted(set(inputs)) if p.is_file()}, "tools": {}, "steps": [], "m5_complete": False,
              "limits": ["initialized AMD64 COFF subset with no auxiliary/weak/common/COMDAT/bigobj/archive/import-library input yet",
                         "at most 128 input objects/1024 contributions/96 PE sections/100000 symbols and relocations/64 MiB data and image",
                         "exact-name section merging; general subsection ordering, native DLL/TLS/resources/SDK support remain open",
                         "explicit kernel32 ExitProcess import and i64 entry-result wrapper; general native imports/CRT startup remain open",
                         "unwind metadata preserved and validated for the tested subset; full CodeView/PDB/debugger gates remain open",
                         "SG emits PE itself with unavailable toolchain PATH during linking; final self-build process tracing remains M6-M8 work"]}
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
            env.pop(name)
    env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(tools["clang"]),
               SENGOO_MODULE_MAP=";".join("selfhost_" + name + "=" + str(ROOT / "selfhost/src" / (name + ".sg")) for name in MODULES))
    env["PATH"] = str(tools["clang"].parent) + os.pathsep + env.get("PATH", "")
    empty = output / "empty-toolchain-path"
    empty.mkdir(exist_ok=True)
    pure_env = {key: value for key, value in env.items() if not key.upper().startswith("SENGOO_")}
    pure_env["PATH"] = str(empty)

    def run(name, command, code=0, marker=None, pure=False):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=pure_env if pure else env, capture_output=True, timeout=600)
        (output / (name + ".stdout.log")).write_bytes(done.stdout)
        (output / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)), "exit_code": done.returncode, "expected_exit_code": code, "seconds": time.monotonic() - started, "toolchain_unavailable_path": pure})
        if done.returncode != code or marker and marker not in done.stdout + done.stderr:
            raise AssertionError(f"{name}: {done.returncode}")
        print(name, done.returncode, flush=True)
        return done

    try:
        for name, path in tools.items():
            digest = sha(path)
            if digest != lock[name]["sha256"]:
                raise ValueError(f"{name} identity mismatch")
            report["tools"][name] = {"path": str(path), "sha256": digest}
        for name in MODULES:
            for imported in re.findall(r"import\s+selfhost_([a-zA-Z0-9_]+)\s*;", (ROOT / "selfhost/src" / (name + ".sg")).read_text(encoding="utf-8")):
                if imported not in MODULES:
                    raise AssertionError("unrecorded SG module dependency")
        for path in [ROOT / "selfhost/src" / (name + ".sg") for name in MODULES if name.startswith("link_")] + [ROOT / "selfhost/tests/linker.sg"]:
            run("format-" + path.stem, [tools["formatter"], "--check", path])
        binary = output / "sg-linker.exe"
        binary.unlink(missing_ok=True)
        run("build-sg", [tools["seed"], "--runtime-mode", "source-development", "build", ROOT / "selfhost/tests/linker.sg", "--force-rebuild", "-O", "0", "-o", binary])
        run("native-scopes", [binary], marker=b"selfhost SG COFF/PE linker foundation: ok", pure=True)
        a, b = output / "first 世界 &.obj", output / "second 世界 &.obj"
        run("emit-objects", [binary, "emit", a, b], pure=True)
        image, repeated = output / "linked 世界 &.exe", output / "repeated.exe"
        run("sg-link", [binary, "link", image, "main", a, b], pure=True)
        run("sg-link-repeat", [binary, "link", repeated, "main", a, b], pure=True)
        if image.read_bytes() != repeated.read_bytes():
            raise AssertionError("PE bytes differ on repeat link")
        report["image_sections"] = validate_image(image.read_bytes())
        run("execute-image", [image], 42, pure=True)
        inspector = tools["clang"].parent / "llvm-readobj.exe"
        report["tools"]["inspector"] = {"path": str(inspector), "sha256": sha(inspector)}
        inspected = run("inspect-image", [inspector, "--file-headers", "--sections", "--coff-imports", "--coff-basereloc", "--unwind", image]).stdout
        for marker in (b"COFF-x86-64", b"ExitProcess", b"KERNEL32.dll", b"DIR64", b"ALLOC_SMALL size=40"):
            if marker not in inspected:
                raise AssertionError(f"missing image inspection marker {marker!r}")
        probe = output / "linker-probe.exe"
        run("build-windows-probe", [tools["clang"], ROOT / "selfhost/tests/linker_probe.c", "-Werror", "-o", probe])
        run("forced-rebase-and-unwind", [probe, image], marker=b"SG-linked PE forced rebasing and Windows unwind: ok")
        negatives = []
        for name, first, second, entry, expected in mutations(a.read_bytes(), b.read_bytes()):
            if expected is None:
                continue
            first_path, second_path, result_path = output / (name + "-first.obj"), output / (name + "-second.obj"), output / (name + ".exe")
            first_path.write_bytes(first)
            if second is not None:
                second_path.write_bytes(second)
            sentinel = b"preserve previous output after a failed link"
            result_path.write_bytes(sentinel)
            command = [binary, "link", result_path, entry, first_path] + ([second_path] if second is not None else [])
            done = run("reject-" + name, command, 65, pure=True)
            if done.stdout.strip() != str(expected).encode() or result_path.read_bytes() != sentinel:
                raise AssertionError(f"{name} diagnostic/output preservation differs: {done.stdout!r}")
            negatives.append({"name": name, "expected_code": expected, "input_sha256": sha(first_path), "previous_output_preserved": True})
        report["negative_cases"] = negatives
        report["artifacts"] = {p.name: sha(p) for p in (binary, a, b, image, repeated, probe)}
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items() if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        report.update(changed_sources=changed, changed_tools=changed_tools, source_revision_end=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if changed or changed_tools:
            report.update(result="failed", error="linker inputs changed")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("linker inputs changed")


if __name__ == "__main__":
    main()
