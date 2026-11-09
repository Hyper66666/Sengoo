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
MODULES = ("core", "binary", "coff", "object", "link_model", "link_coff", "link_imports", "link_archive", "link_pe")


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


def archive_members(raw):
    if raw[:8] != b"!<arch>\n":
        raise AssertionError("archive signature differs")
    rows, at = [], 8
    while at < len(raw):
        size = int(raw[at + 48:at + 58])
        rows.append((at, at + 60, size, raw[at:at + 16].rstrip()))
        at += 60 + size + size % 2
    if at != len(raw):
        raise AssertionError("archive extent differs")
    return rows


def image_imports(raw):
    sections = []
    for index in range(u16(raw, 70)):
        at = 328 + index * 40
        size, rva, padded, offset = struct.unpack_from("<IIII", raw, at + 8)
        sections.append((rva, max(size, padded), offset))

    def location(rva):
        for start, size, offset in sections:
            if start <= rva < start + size:
                return offset + rva - start
        raise AssertionError("RVA outside sections")

    def name(rva):
        at = location(rva)
        return raw[at:raw.index(0, at)].decode("ascii")

    result, at = [], location(u32(raw, 208))
    while u32(raw, at):
        dll = name(u32(raw, at + 12))
        int_at, iat_at = location(u32(raw, at)), location(u32(raw, at + 16))
        while struct.unpack_from("<Q", raw, int_at)[0]:
            value = struct.unpack_from("<Q", raw, int_at)[0]
            if value != struct.unpack_from("<Q", raw, iat_at)[0]:
                raise AssertionError("INT/IAT differ before loading")
            export = value & 65535 if value >> 63 else name(value + 2)
            result.append((dll, export))
            int_at += 8
            iat_at += 8
        if struct.unpack_from("<Q", raw, iat_at)[0]:
            raise AssertionError("IAT terminator differs")
        at += 20
    return result


def short_import(symbol, dll, export=None, name_type=1, ordinal=0, kind=0):
    payload = symbol.encode() + b"\0" + dll.encode() + b"\0"
    if name_type == 4:
        payload += export.encode() + b"\0"
    return struct.pack("<HHHHIIHH", 0, 65535, 0, 0x8664, 0, len(payload), ordinal, kind | name_type << 2) + payload


def dll_export_ordinal(path, symbol):
    raw = path.read_bytes()
    pe = u32(raw, 60)
    sections = pe + 24 + u16(raw, pe + 20)

    def location(rva):
        for index in range(u16(raw, pe + 6)):
            row = sections + 40 * index
            size, start, padded, offset = struct.unpack_from("<IIII", raw, row + 8)
            if start <= rva < start + max(size, padded):
                return offset + rva - start
        raise AssertionError("SDK export RVA outside sections")

    directory = location(u32(raw, pe + 24 + 112))
    count = u32(raw, directory + 24)
    names, ordinals = location(u32(raw, directory + 32)), location(u32(raw, directory + 36))
    for index in range(count):
        at = location(u32(raw, names + index * 4))
        if raw[at:raw.index(0, at)].decode("ascii") == symbol:
            return u32(raw, directory + 16) + u16(raw, ordinals + index * 2)
    raise AssertionError("native DLL export was not found")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m5-linker-final")
    parser.add_argument("--sdk-kernel32", type=Path)
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
              "limits": ["initialized AMD64 COFF subset; auxiliary/weak/common/COMDAT/bigobj and full static SDK objects remain open",
                         "at most 128 input objects/1024 contributions/96 PE sections/100000 symbols and relocations/64 MiB data and image",
                         "standard indexed COFF archives and AMD64 MSVC short import members; tested real kernel32 SDK import library",
                         "named/ordinal code and IAT imports with stable DLL grouping; native DLL/TLS/resources/CRT startup remain open",
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
        librarian = tools["clang"].parent / "llvm-lib.exe"
        report["tools"]["librarian"] = {"path": str(librarian), "sha256": sha(librarian)}
        archive = output / "static 世界 &.lib"
        run("create-independent-static-archive", [librarian, "/out:" + str(archive), a, b])
        archives = archive_members(archive.read_bytes())
        report["static_archive"] = {"sha256": sha(archive), "members": [{"offset": row[0], "size": row[2], "name": row[3].decode("ascii")} for row in archives]}
        for label, arguments in (("lazy-static", [a, archive]), ("entry-from-archive", [archive])):
            linked = output / (label + ".exe")
            run(label, [binary, "link", linked, "main", *arguments], pure=True)
            if linked.read_bytes() != image.read_bytes():
                raise AssertionError("archive extraction changed selected object bytes")
            run(label + "-execute", [linked], 42, pure=True)
            report["artifacts"][linked.name] = sha(linked)
        first_archive, second_archive = output / "first-only.lib", output / "second-only.lib"
        run("create-first-static-archive", [librarian, "/out:" + str(first_archive), a])
        run("create-second-static-archive", [librarian, "/out:" + str(second_archive), b])
        cross_archive = output / "cross-archive.exe"
        run("cross-archive-fixed-point", [binary, "link", cross_archive, "main", second_archive, first_archive], pure=True)
        if cross_archive.read_bytes() != image.read_bytes():
            raise AssertionError("cross-archive fixed point changed selected object bytes")
        run("cross-archive-execute", [cross_archive], 42, pure=True)
        report["artifacts"].update({p.name: sha(p) for p in (first_archive, second_archive, cross_archive, archive)})

        if args.sdk_kernel32:
            sdk = args.sdk_kernel32.resolve()
            report["tools"]["sdk-kernel32"] = {"path": str(sdk), "sha256": sha(sdk), "size": sdk.stat().st_size}
            sdk_object, sdk_image = output / "sdk-main.obj", output / "sdk-linked.exe"
            run("emit-sdk-object", [binary, "sdk", sdk_object], pure=True)
            run("sg-sdk-archive-link", [binary, "link", sdk_image, "main", sdk, sdk_object], pure=True)
            run("sdk-execute", [sdk_image], 42, pure=True)
            sdk_repeat = output / "sdk-repeat.exe"
            run("sg-sdk-repeat", [binary, "link", sdk_repeat, "main", sdk_object, sdk], pure=True)
            if sdk_image.read_bytes() != sdk_repeat.read_bytes():
                raise AssertionError("SDK link bytes depend on input ordering/repetition")
            expected_imports = {("KERNEL32.dll", name) for name in ("GetCurrentProcessId", "GetCurrentThreadId", "ExitProcess")}
            if set(image_imports(sdk_image.read_bytes())) != expected_imports:
                raise AssertionError("SDK selected imports differ")
            inspection = run("inspect-sdk-image", [inspector, "--coff-imports", "--unwind", sdk_image]).stdout
            if any(name.encode() not in inspection for _, name in expected_imports):
                raise AssertionError("independent SDK imports differ")
            native_dll = Path(os.environ["SystemRoot"]) / "System32/kernel32.dll"
            report["tools"]["native-kernel32"] = {"path": str(native_dll), "sha256": sha(native_dll)}
            ordinal = dll_export_ordinal(native_dll, "ExitProcess")
            extras = [short_import("_GetTickCount64", "KERNEL32.dll", name_type=2),
                      short_import("_GetTickCount@0", "KERNEL32.dll", name_type=3),
                      short_import("sg_unused_system_time", "KERNEL32.dll", export="GetSystemTimeAsFileTime", name_type=4),
                      short_import("GetDesktopWindow", "USER32.dll"),
                      short_import("sg_exit_ordinal", "KERNEL32.dll", name_type=0, ordinal=ordinal),
                      short_import("sg_data_iat", "KERNEL32.dll", export="GetTickCount64", name_type=4, kind=1)]
            extra_paths = []
            for index, raw in enumerate(extras):
                path = output / ("extra-import-" + str(index) + ".obj")
                path.write_bytes(raw)
                extra_paths.append(path)
            multi_dll = output / "multiple-dlls.exe"
            run("multiple-dll-name-types", [binary, "link", multi_dll, "main", sdk_object, sdk, *extra_paths], pure=True)
            run("multiple-dlls-execute", [multi_dll], 42, pure=True)
            imports = image_imports(multi_dll.read_bytes())
            if set(imports) != expected_imports | {("KERNEL32.dll", "GetTickCount64"), ("KERNEL32.dll", "GetTickCount"), ("KERNEL32.dll", "GetSystemTimeAsFileTime"), ("USER32.dll", "GetDesktopWindow"), ("KERNEL32.dll", ordinal)}:
                raise AssertionError("multiple DLL/name type imports differ")
            report["sdk_imports"] = {"selected": image_imports(sdk_image.read_bytes()), "multiple_dlls": imports}
            for path in [sdk_object, sdk_image, sdk_repeat, multi_dll, *extra_paths]:
                report["artifacts"][path.name] = sha(path)
            malformed = bytearray(sdk.read_bytes())
            struct.pack_into(">I", malformed, 72, 9)
            invalid_archive = output / "invalid-sdk-index.lib"
            invalid_archive.write_bytes(malformed)
            run("sdk-exact-resource-scopes", [binary, "sdk-resources", sdk_object, sdk, invalid_archive], marker=b"selfhost SG archive/SDK exact resources: ok", pure=True)
            report["sdk_resource_scopes"] = 10

            bad_inputs = []
            def changed(raw, at, value, fmt):
                mutated = bytearray(raw)
                struct.pack_into(fmt, mutated, at, value)
                return bytes(mutated)
            first_index = next(row for row in archive_members(sdk.read_bytes()) if row[3] == b"/")
            bad_inputs += [("archive-member-offset", bytes(malformed), 7201),
                           ("archive-index-count", changed(sdk.read_bytes(), first_index[1], 100001, ">I"), 7202),
                           ("archive-member-trailer", changed(sdk.read_bytes(), 66, 0, "<B"), 7201),
                           ("archive-index-kind", changed(sdk.read_bytes(), first_index[0], ord("X"), "<B"), 7202)]
            static_raw = archive.read_bytes()
            long_reference = next(row for row in archive_members(static_raw) if row[3].startswith(b"/0"))
            second_index = [row for row in archive_members(static_raw) if row[3] == b"/"][1]
            bad_inputs += [("archive-long-name-offset", changed(static_raw, long_reference[0] + 1, ord("9"), "<B"), 7201),
                           ("archive-second-index-target", changed(static_raw, second_index[1] + 4, 9, "<I"), 7201)]
            short = short_import("GetCurrentProcessId", "KERNEL32.dll")
            bad_inputs += [("import-data-size", changed(short, 12, 1, "<I"), 7201),
                           ("import-machine", changed(short, 6, 0x14c, "<H"), 7203),
                           ("import-name-type", changed(short, 18, 20, "<H"), 7203),
                           ("import-reserved-flags", changed(short, 18, 36, "<H"), 7203),
                           ("import-empty-export", short_import("_", "KERNEL32.dll", name_type=2), 7201),
                           ("import-exportas-tail", changed(extras[2] + b"x", 12, len(extras[2]) - 19, "<I"), 7201)]
            for name, raw, expected in bad_inputs:
                path, linked = output / (name + ".lib"), output / (name + ".exe")
                path.write_bytes(raw)
                sentinel = b"keep prior executable"
                linked.write_bytes(sentinel)
                done = run("reject-" + name, [binary, "link", linked, "main", sdk_object, path], 65, pure=True)
                if done.stdout.strip() != str(expected).encode() or linked.read_bytes() != sentinel:
                    raise AssertionError(f"{name} diagnostic/output differs: {done.stdout!r}")
                report["negative_cases"].append({"name": name, "expected_code": expected, "input_sha256": sha(path), "previous_output_preserved": True})
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
