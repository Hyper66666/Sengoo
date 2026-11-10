"""Qualify SG-generated MSF/CodeView/PDB bytes with LLVM and live LLDB."""
from __future__ import annotations

from pdb_type_hash_oracle import validate_tpi_hashes

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import struct
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
MODULES = ("core", "binary", "coff", "object", "link_model", "link_coff", "link_imports", "link_pe", "link_hash", "link_msf", "link_cv_types", "link_debug", "link_pdb")


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def u16(raw, offset):
    return struct.unpack_from("<H", raw, offset)[0]


def u32(raw, offset):
    return struct.unpack_from("<I", raw, offset)[0]


def msf_streams(raw):
    if raw[:32] != b"Microsoft C/C++ MSF 7.00\r\n\x1aDS\0\0\0":
        raise AssertionError("MSF signature differs")
    size, fpm, count, directory_size, reserved, block_map = struct.unpack_from("<6I", raw, 32)
    if size != 4096 or fpm not in (1, 2) or count > 4096 or len(raw) != count * size or reserved:
        raise AssertionError("MSF superblock differs")
    directory_pages = (directory_size + size - 1) // size
    blocks = struct.unpack_from("<" + "I" * directory_pages, raw, block_map * size)
    directory = b"".join(raw[index * size:(index + 1) * size] for index in blocks)[:directory_size]
    streams = u32(directory, 0)
    sizes = struct.unpack_from("<" + "I" * streams, directory, 4)
    used = [0, 1, 2, block_map, *blocks]
    at, result = 4 + 4 * streams, []
    for length in sizes:
        pages = (length + size - 1) // size
        indices = struct.unpack_from("<" + "I" * pages, directory, at)
        used.extend(indices)
        at += 4 * pages
        result.append(b"".join(raw[index * size:(index + 1) * size] for index in indices)[:length])
        if pages and any(raw[indices[-1] * size + (length - 1) % size + 1:(indices[-1] + 1) * size]):
            raise AssertionError("MSF stream padding differs")
    if at != directory_size or len(used) != len(set(used)) or set(used) != set(range(count)):
        raise AssertionError("MSF directory allocation differs")
    for index in range(4096 * 8):
        for page in (1, 2):
            free = bool(raw[page * size + index // 8] & 1 << (index % 8))
            if free == (index in used):
                raise AssertionError("MSF free-page map differs")
    return result


def validate_pair(image, pdb, source, pdb_path, expected_code_size=29, expected_lines=((0, 1), (4, 2), (13, 3), (19, 4), (24, 5))):
    streams = msf_streams(pdb)
    if len(streams) != 12 or u32(streams[1], 0) != 20000404 or u32(streams[1], 8) != 1:
        raise AssertionError("PDB stream identities differ")
    if u32(streams[3], 4) != 19990903 or u16(streams[3], 58) != 0x8664:
        raise AssertionError("DBI identity differs")
    validate_tpi_hashes(streams)
    guid = streams[1][12:28]
    codeview_rva, debug_size = struct.unpack_from("<II", image, 248)
    sections = []
    for index in range(u16(image, 70)):
        at = 328 + index * 40
        length, rva, padded, raw = struct.unpack_from("<IIII", image, at + 8)
        sections.append((rva, length, raw))
    debug_raw = next(raw + codeview_rva - rva for rva, length, raw in sections if rva <= codeview_rva < rva + length)
    if debug_size != 28 or u32(image, debug_raw + 12) != 2:
        raise AssertionError("PE debug directory differs")
    data_raw = u32(image, debug_raw + 24)
    if image[data_raw:data_raw + 4] != b"RSDS" or image[data_raw + 4:data_raw + 20] != guid or u32(image, data_raw + 20) != 1:
        raise AssertionError("PE/PDB signature association differs")
    expected_path = str(pdb_path).replace("\\", "/").encode() + b"\0"
    if image[data_raw + 24:data_raw + 24 + len(expected_path)] != expected_path:
        raise AssertionError("RSDS path differs")
    module_info = streams[3][64:128]
    symbols_size, c13_size = u32(module_info, 36), u32(module_info, 44)
    module = streams[6]
    at, lines, checksum = symbols_size, [], None
    while at < symbols_size + c13_size:
        kind, length = struct.unpack_from("<II", module, at)
        data = module[at + 8:at + 8 + length]
        if kind == 0xF2:
            offset, segment, flags, size = struct.unpack_from("<IHHI", data)
            file_id, count, block_size = struct.unpack_from("<III", data, 12)
            if segment != 1 or flags or offset or size != expected_code_size or file_id or block_size != 12 + 8 * count:
                raise AssertionError("CodeView line block differs")
            lines.extend((u32(data, 24 + index * 8), u32(data, 28 + index * 8)) for index in range(count))
        elif kind == 0xF4:
            if length != 40 or data[:6] != b"\x01\0\0\0\x20\x03":
                raise AssertionError("CodeView checksum identity differs")
            checksum = data[6:38]
        else:
            raise AssertionError("unexpected CodeView subsection")
        at += 8 + length
    if lines != [(offset, number | 0x80000000) for offset, number in expected_lines]:
        raise AssertionError("source address/line map differs")
    if checksum != hashlib.sha256(source).digest():
        raise AssertionError("SG source SHA-256 differs")
    return {"guid_bytes": guid.hex(), "streams": list(map(len, streams)), "source_sha256": checksum.hex(), "lines": lines}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter", "lldb-python-home"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("output must stay in target/")
    output.mkdir(parents=True, exist_ok=True)
    tools = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter")}
    tools.update(debugger=tools["clang"].parent / "lldb.exe", pdb_inspector=tools["clang"].parent / "llvm-pdbutil.exe", image_inspector=tools["clang"].parent / "llvm-readobj.exe")
    python_home = args.lldb_python_home.resolve()
    tools.update(lldb_python_dll=python_home / "python310.dll", lldb_python_stdlib=python_home / "python310.zip")
    for path in sorted(python_home.glob("*.pyd")) + sorted(python_home.glob("*.dll")) + sorted(tools["clang"].parent.glob("*lldb*.dll")):
        tools["debug-dependency-" + path.name] = path
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
    source = ROOT / "selfhost/tests/link_debug_source.sg"
    inputs = [Path(__file__).resolve(), ROOT / "selfhost/tests/link_debug.sg", source, ROOT / "selfhost/bootstrap.lock", ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "rust-toolchain.toml"]
    inputs += [ROOT / "selfhost/src" / (name + ".sg") for name in MODULES]
    for folder in (ROOT / "tools/stdlib", ROOT / "runtime"):
        inputs += [path for path in folder.rglob("*") if path.is_file() and not any(part in {"target", "build", "__pycache__"} for part in path.relative_to(folder).parts)]
    revision = lambda: subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    report = {"schema": 1, "result": "running", "stage": "sg-codeview-pdb-debugger-checkpoint", "m5_complete": False,
              "source_revision": revision(), "started_at": dt.datetime.now(dt.timezone.utc).isoformat(), "host": platform.platform(), "python": sys.version,
              "sources": {str(p.relative_to(ROOT)): sha(p) for p in sorted(set(inputs))}, "tools": {}, "steps": [],
              "limits": ["one UTF-8 source module, up to 256 nonoverlapping no-argument i64 functions, 4096 SP-relative i64 locals and 65536 ordered lines",
                         "1 MiB source, 16 MiB PDB, 4096 MSF blocks and 256 streams; complete static SDK, native CodeView input/type merging and M5 remain open",
                         "the SG test constructs corresponding x64 instructions explicitly; automatic compiler source/type/local address mapping remains integration work",
                         "the source fixture retains statement-per-line bytes intentionally; formatting it would invalidate the tested line map",
                         "LLDB and LLVM tools inspect/debug artifacts independently; product PE/PDB emission uses SG with an empty toolchain PATH"]}
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
        for name in ("link_hash", "link_msf", "link_debug", "link_pdb"):
            run("format-" + name, [tools["formatter"], "--check", ROOT / "selfhost/src" / (name + ".sg")])
        run("format-test", [tools["formatter"], "--check", ROOT / "selfhost/tests/link_debug.sg"])
        binary = output / "sg-debug.exe"
        run("build-sg", [tools["seed"], "--runtime-mode", "source-development", "build", ROOT / "selfhost/tests/link_debug.sg", "--force-rebuild", "-O", "0", "-o", binary])
        image, pdb = output / "debug-linked.exe", output / "debug-linked.pdb"
        arguments = [binary, image.as_posix(), pdb.as_posix(), source.as_posix()]
        run("emit-and-exact-resource-scopes", arguments, marker=b"selfhost SG CodeView/PDB exact resources: ok", pure=True)
        original_image, original_pdb = image.read_bytes(), pdb.read_bytes()
        run("repeat-identical-options", arguments, pure=True)
        if original_image != image.read_bytes() or original_pdb != pdb.read_bytes():
            raise AssertionError("PE/PDB bytes differ on equal repeated inputs/options")
        report["pair"] = validate_pair(original_image, original_pdb, source.read_bytes(), pdb)
        run("execute-sg-pe", [image], 42, pure=True)
        details = run("inspect-pdb", [tools["pdb_inspector"], "dump", "--summary", "--modules", "--types", "--globals", "--publics", "--symbols", pdb])
        for marker in (b"Has Globals: true", b"Has Publics: true", b"S_GPROC32", b"answer", b"local fp reg = RSP", b"LF_PROCEDURE"):
            if marker not in details:
                raise AssertionError("PDB inspector marker missing: " + repr(marker))
        yaml = run("inspect-lines-checksums", [tools["pdb_inspector"], "pdb2yaml", "--all", pdb])
        if b"Kind:            SHA256" not in yaml or b"LineStart:       3" not in yaml:
            raise AssertionError("source/line/checksum inspection differs")
        run("inspect-pe-rsds", [tools["image_inspector"], "--coff-debug-directory", "--unwind", image], marker=b"PDBGUID")
        runtime_probe = "script import ctypes, sys; buf = ctypes.create_unicode_buffer(32768); win = ctypes.WinDLL('kernel32'); win.GetModuleHandleW.restype = ctypes.c_void_p; win.GetModuleFileNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_uint]; win.GetModuleFileNameW(win.GetModuleHandleW('python310.dll'), buf, len(buf)); print('LLDB_PYTHON_DLL=' + buf.value); print('LLDB_PYTHON_VERSION=' + sys.version); sys.stdout.flush()"
        runtime = run("verify-debugger-python-runtime", [tools["debugger"], "--batch", "--no-lldbinit", "-o", runtime_probe, image], debugger=True)
        expected_dll = str(tools["lldb_python_dll"]).lower().replace("\\", "/").encode()
        if b"lldb_python_dll=" + expected_dll not in runtime.lower().replace(b"\\", b"/"):
            raise AssertionError("debugger loaded an unrecorded Python DLL")
        commands = ["breakpoint set --file link_debug_source.sg --line 3", "run", "frame variable answer", "thread step-over", "frame variable answer", "continue"]
        debugged = run("live-source-breakpoint-step-locals", [tools["debugger"], "--batch", "--no-lldbinit", *[item for command in commands for item in ("-o", command)], image], debugger=True)
        for marker in (b"stop reason = breakpoint", b"answer = 40", b"stop reason = step over", b"answer = 42", b"exited with status = 42"):
            if marker not in debugged:
                raise AssertionError("real debugger outcome missing: " + repr(marker))
        grown_image, grown_pdb = output / "grown-headers.exe", output / "grown-headers.pdb"
        run("grow-pe-header-table", [binary, "grow", grown_image.as_posix(), grown_pdb.as_posix(), source.as_posix()], pure=True)
        report["grown_pair"] = validate_pair(grown_image.read_bytes(), grown_pdb.read_bytes(), source.read_bytes(), grown_pdb)
        if u16(grown_image.read_bytes(), 70) != 18 or u32(grown_image.read_bytes(), 148) != 1536:
            raise AssertionError("new debug section did not grow PE headers as expected")
        run("execute-grown-headers", [grown_image], 42, pure=True)
        bound_source, bound_image, bound_pdb = output / "bound-source.sg", output / "bound-source.exe", output / "bound-source.pdb"
        bound_source.write_bytes(source.read_bytes() + b"// A\n")
        run("source-identity-first", [binary, bound_image.as_posix(), bound_pdb.as_posix(), bound_source.as_posix()], pure=True)
        first_pair = validate_pair(bound_image.read_bytes(), bound_pdb.read_bytes(), bound_source.read_bytes(), bound_pdb)
        bound_source.write_bytes(source.read_bytes() + b"// B\n")
        run("source-identity-second", [binary, bound_image.as_posix(), bound_pdb.as_posix(), bound_source.as_posix()], pure=True)
        second_pair = validate_pair(bound_image.read_bytes(), bound_pdb.read_bytes(), bound_source.read_bytes(), bound_pdb)
        if first_pair["guid_bytes"] == second_pair["guid_bytes"] or first_pair["source_sha256"] == second_pair["source_sha256"]:
            raise AssertionError("equal-length source changes failed to update bound debug identity")
        report["source_bound_identity"] = {"first": first_pair, "second": second_pair}
        corpus = [b"", b"abc", b"a" * 1000000]
        corpus += [bytes((index * 13 + length) & 255 for index in range(length)) for length in (1, 2, 3, 55, 56, 57, 63, 64, 65, 127, 128, 4095, 4096, 4097)]
        rng = random.Random(0x5347504442)
        corpus += [rng.randbytes(rng.randrange(1025)) for _ in range(1007)]
        corpus_input, corpus_output = output / "sha256-corpus.bin", output / "sha256-digests.bin"
        corpus_input.write_bytes(b"".join(struct.pack("<I", len(raw)) + raw for raw in corpus))
        run("sha256-independent-corpus", [binary, "hash-corpus", corpus_input, corpus_output], pure=True)
        if corpus_output.read_bytes() != b"".join(hashlib.sha256(raw).digest() for raw in corpus):
            raise AssertionError("SG SHA-256 differs from hashlib")
        report["sha256_corpus"] = {"cases": len(corpus), "input_sha256": sha(corpus_input), "output_sha256": sha(corpus_output)}
        msf = output / "multi-page.msf"
        run("emit-multi-page-msf", [binary, "msf", msf], pure=True)
        streams = msf_streams(msf.read_bytes())
        expected = [bytes((number * 17 + index * 31) & 255 for index in range(length)) for number, length in enumerate((0, 1, 4095, 4096, 4097, 9000, 17000))]
        if streams != expected:
            raise AssertionError("multi-page MSF stream bytes differ")
        negatives = []
        for number in range(1, 13):
            expected_code = 7303 if number in (4, 9, 12) else 7305 if number in (7, 8) else 7301
            sentinel = b"preserve previous debug output"
            image.write_bytes(sentinel)
            pdb.write_bytes(sentinel)
            result = run("reject-debug-" + str(number), [binary, "invalid", chr(64 + number), image, pdb, source], 65, pure=True)
            if result.strip() != str(expected_code).encode() or image.read_bytes() != sentinel or pdb.read_bytes() != sentinel:
                raise AssertionError("debug failure diagnostic/output preservation differs")
            negatives.append({"case": number, "expected_code": expected_code, "outputs_preserved": True})
        image.write_bytes(original_image)
        pdb.write_bytes(original_pdb)
        report.update(negative_cases=negatives, native_resource_scopes=20, resource_failure_cases_per_scope=12,
                      artifacts={path.name: sha(path) for path in (binary, image, pdb, msf, corpus_input, corpus_output, grown_image, grown_pdb, bound_source, bound_image, bound_pdb)}, result="passed")
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items() if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        report.update(changed_sources=changed, changed_tools=changed_tools, source_revision_end=revision(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if changed or changed_tools:
            report.update(result="failed", error="debug input identity changed")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("debug input identity changed")


if __name__ == "__main__":
    main()
