"""Qualify the SG UTF-8 algorithms with pinned preview tools and independent C/Python oracles."""
from __future__ import annotations

import argparse
import ctypes
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

from linker_evidence import copy_linker

ROOT = Path(__file__).resolve().parents[1]
CORPUS_SHA = "5ba19a59d8b565bd44b0d1e7715ddbb593ee8e651570b91ff64c2df047f6bc47"
COUNTS = {"records": 1221242, "valid_records": 1150662, "invalid_records": 70580, "bytes": 9374470}
TEXT_LIMIT = 67108864


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def sha(path):
    return digest(Path(path).read_bytes())


def revision():
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()


def git_status():
    return subprocess.check_output(["git", "status", "--porcelain=v1", "--untracked-files=normal"], cwd=ROOT, text=True, encoding="utf-8").splitlines()


def strict_json(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate JSON field: " + key)
            result[key] = value
        return result

    def constant(value):
        raise ValueError("non-finite JSON constant: " + value)

    return json.loads(raw.decode("utf-8", errors="strict"), object_pairs_hook=pairs, parse_constant=constant)


def exact_json(raw, expected):
    value = strict_json(raw)
    if type(value) is not dict or set(value) != set(expected):
        raise ValueError("JSON field set differs")
    if any(type(value[key]) is not type(wanted) or value[key] != wanted for key, wanted in expected.items()):
        raise ValueError("JSON field type/value differs: " + repr(value))
    return value


def probe_expected(mode):
    result = {"schema": 1, "ok": True, "corpus_records": 0, "valid_records": 0, "invalid_records": 0,
              "guard_cases": 60, "validation_calls": 75, "boundary_calls": 423, "no_read_calls": 220,
              "instrumented": True, "memory_reads": 730, "input_unchanged": True, "guard_pages": True,
              "large_limit_checked": False}
    if mode == "corpus":
        result.update(corpus_records=1221242, valid_records=1150662, invalid_records=70580,
                      validation_calls=1221317, boundary_calls=9258120, no_read_calls=2443718, memory_reads=62091698)
    elif mode == "large":
        result.update(validation_calls=77, boundary_calls=426, no_read_calls=222,
                      memory_reads=201327322, large_limit_checked=True)
    elif mode == "production":
        result.update(instrumented=False, memory_reads=None)
    else:
        raise ValueError("unknown probe mode")
    return result


def oracle_controls():
    expected = {"schema_version": 1, "ok": True, "phase": "object"}
    good = json.dumps(expected).encode()
    malformed = [good + b"extra stdout", b'[]', b'{"schema_version":1,"schema_version":1,"ok":true,"phase":"object"}',
                 b'{"schema_version":true,"ok":true,"phase":"object"}',
                 b'{"schema_version":1,"ok":1,"phase":"object"}',
                 b'{"schema_version":1,"ok":true,"phase":"object","extra":0}',
                 b'{"schema_version":1,"ok":true,"phase":NaN}']
    for raw in malformed:
        try:
            exact_json(raw, expected)
        except (ValueError, UnicodeError):
            continue
        raise AssertionError("JSON oracle accepted malformed native success")
    for field, replacement in [("instrumented", False), ("memory_reads", None), ("validation_calls", True),
                               ("guard_cases", 59), ("large_limit_checked", True)]:
        changed = dict(probe_expected("corpus"), **{field: replacement})
        try:
            exact_json(json.dumps(changed).encode(), probe_expected("corpus"))
        except ValueError:
            continue
        raise AssertionError("probe JSON oracle accepted " + field)
    return {"malformed_success_json_rejected": len(malformed), "wrong_probe_mode_type_or_count_rejected": 5}


def verify_corpus(raw):
    if digest(raw) != CORPUS_SHA or raw[:8] != b"SGUTF8\x01\x00" or len(raw) != COUNTS["bytes"]:
        raise ValueError("independent corpus bytes differ")
    count = struct.unpack_from("<Q", raw, 8)[0]
    if count != COUNTS["records"]:
        raise ValueError("corpus header count differs")
    at, valid_count, invalid_count = 16, 0, 0
    for _ in range(count):
        if at + 2 > len(raw):
            raise ValueError("truncated corpus prefix")
        length, valid = raw[at:at + 2]
        width = (length + 8) // 8
        if length > 64 or valid not in (0, 1) or at + 2 + width + length > len(raw):
            raise ValueError("invalid corpus record extent")
        mask = raw[at + 2:at + 2 + width]
        text = raw[at + 2 + width:at + 2 + width + length]
        try:
            decoded = text.decode("utf-8", errors="strict")
        except UnicodeDecodeError:
            expected_valid, expected_mask = 0, bytes(width)
        else:
            offset, bits = 0, 1
            for character in decoded:
                offset += len(character.encode("utf-8", errors="strict"))
                bits |= 1 << offset
            if offset != length:
                raise AssertionError("strict character re-encoding length differs")
            expected_valid, expected_mask = 1, bits.to_bytes(width, "little")
        if (valid, mask) != (expected_valid, expected_mask):
            raise ValueError("corpus strict-decode/boundary oracle differs")
        valid_count += valid
        invalid_count += 1 - valid
        at += 2 + width + length
    if at != len(raw) or (valid_count, invalid_count) != (COUNTS["valid_records"], COUNTS["invalid_records"]):
        raise ValueError("corpus count or trailing data differs")
    return {**COUNTS, "sha256": digest(raw), "complete_strict_decode_reencode_checked": True}


def dependencies(path):
    text = path.read_text(encoding="utf-8").replace("\\\n", " ")
    if not text.startswith("runtime_text_dependency:"):
        raise ValueError("unexpected clang dependency target")
    # Windows clang preserves directory separators but Make-escapes spaces.
    # POSIX shlex would silently strip every ordinary Windows separator.
    payload, words, word, at = text.split(":", 1)[1], [], [], 0
    while at < len(payload):
        char = payload[at]
        if char == "\\" and at + 1 < len(payload) and payload[at + 1] in " \t#":
            word.append(payload[at + 1])
            at += 2
        elif char == "$" and at + 1 < len(payload) and payload[at + 1] == "$":
            word.append("$")
            at += 2
        elif char.isspace():
            if word:
                words.append("".join(word))
                word = []
            at += 1
        else:
            word.append(char)
            at += 1
    if word:
        words.append("".join(word))
    paths = {Path(word).resolve() for word in words}
    if not paths or any(not item.is_file() for item in paths):
        raise ValueError("missing clang header dependency")
    return paths


def coff_image(raw, relocation_kinds=(4,)):
    machine, count, stamp, symbol_at, symbol_count, optional, characteristics = struct.unpack_from("<HHIIIHH", raw)
    if machine != 0x8664 or optional or characteristics or not 1 <= count <= 8:
        raise ValueError("unexpected development C COFF header")
    strings_at = symbol_at + symbol_count * 18
    string_size = struct.unpack_from("<I", raw, strings_at)[0]
    if strings_at + string_size != len(raw) or string_size < 4:
        raise ValueError("invalid development C COFF string extent")

    def name(encoded):
        offset = struct.unpack_from("<I", encoded, 4)[0] if encoded[:4] == bytes(4) else int(encoded[1:].rstrip(b"\0")) if encoded.startswith(b"/") else None
        if offset is None:
            return encoded.rstrip(b"\0").decode("ascii")
        if not 4 <= offset < string_size:
            raise ValueError("invalid COFF name offset")
        end = raw.index(0, strings_at + offset, strings_at + string_size)
        return raw[strings_at + offset:end].decode("ascii")

    sections, symbols, indices = {}, {}, {}
    for index in range(count):
        encoded, virtual, address, size, begin, reloc, lines, reloc_count, line_count, flags = struct.unpack_from("<8sIIIIIIHHI", raw, 20 + index * 40)
        section = name(encoded)
        if section in sections or virtual or address or lines or line_count or begin + size > symbol_at or reloc + reloc_count * 10 > symbol_at:
            raise ValueError("invalid COFF section extent")
        sections[section] = {"data": raw[begin:begin + size], "flags": flags, "relocations": [], "relocation_at": reloc, "relocation_count": reloc_count}
        indices[index + 1] = section
    index = 0
    while index < symbol_count:
        encoded, value, section, kind, storage, auxiliary = struct.unpack_from("<8sIhHBB", raw, symbol_at + index * 18)
        if index + auxiliary >= symbol_count or section > count:
            raise ValueError("invalid COFF auxiliary symbol extent")
        symbols[index] = (name(encoded), indices[section] if section > 0 else section, value, kind, storage, auxiliary)
        index += 1 + auxiliary
    for section in sections.values():
        for index in range(section["relocation_count"]):
            offset, target, kind = struct.unpack_from("<IIH", raw, section["relocation_at"] + index * 10)
            if target not in symbols or kind not in relocation_kinds or offset + 4 > len(section["data"]):
                raise ValueError("unexpected leaf C relocation")
            section["relocations"].append((offset, kind, symbols[target][:5]))
        del section["relocation_at"], section["relocation_count"]
    return {"timestamp": stamp, "sections": sections, "symbols": list(symbols.values())}


def sg_stack_frame(raw, function, minimum):
    image = coff_image(raw, (3, 4))
    matches = [symbol for symbol in image["symbols"] if symbol[:2] == (function, ".text")]
    if len(matches) != 1:
        raise ValueError("missing SG function for stack inspection")
    begin = matches[0][2]
    code = image["sections"][".text"]["data"][begin:begin + 22]
    if len(code) != 22 or code[:6] != bytes.fromhex("55 48 89 e5 48 b8") or code[14] != 0xe8 or code[19:] != bytes.fromhex("48 29 c4"):
        raise ValueError("SG stack frame does not use the qualified RAX-size stack-probe sequence")
    frame = struct.unpack_from("<Q", code, 6)[0]
    calls = [row for row in image["sections"][".text"]["relocations"] if row[0] == begin + 15]
    if frame < minimum or frame % 16 or len(calls) != 1 or calls[0][1] != 4 or calls[0][2][0] != "__chkstk":
        raise ValueError("missing large real SG stack-probe call")
    return {"function": function, "frame_bytes": frame, "rax_size_then_call_then_sub_rsp": True, "call_relocation": begin + 15}


def compare_c_metadata(before, after, exported):
    original, normalized = coff_image(before), coff_image(after)
    wanted = {name: section for name, section in original["sections"].items() if name not in {".data", ".bss"}}
    for name in set(original["sections"]) - set(wanted):
        section = original["sections"][name]
        if section["data"] or section["relocations"]:
            raise ValueError("cannot discard a nonempty C SDK section")
    if not set(wanted) <= {".text", ".rdata"} or ".text" not in wanted or normalized["sections"] != wanted:
        raise ValueError("C SDK code/data/flags/relocations changed")
    old = {symbol[:5] for symbol in original["symbols"]}
    kept = {symbol[:5] for symbol in normalized["symbols"]}
    referenced = {target for section in wanted.values() for _, _, target in section["relocations"]}
    if not referenced <= kept or not kept <= old or any(symbol[5] or symbol[4] not in (2, 3) for symbol in normalized["symbols"]):
        raise ValueError("C SDK referenced symbols or section-relative addresses changed")
    if [symbol[:5] for symbol in normalized["symbols"] if symbol[0] == exported] != [(exported, ".text", 0, 32, 2)]:
        raise ValueError("C SDK public function changed")
    removed = [symbol for symbol in original["symbols"] if symbol[:5] not in kept]
    if any(symbol[:5] in referenced or symbol[0] not in {*original["sections"], ".file", "@feat.00"} for symbol in removed):
        raise ValueError("C SDK normalization removed a live/non-metadata symbol")
    return {"raw_sha256": digest(before), "normalized_sha256": digest(after), "raw_timestamp": original["timestamp"],
            "normalized_timestamp": normalized["timestamp"], "removed_metadata_symbols": [symbol[0] for symbol in removed],
            "sections": {name: {"sha256": digest(section["data"]), "flags": section["flags"], "relocations": section["relocations"]}
                         for name, section in wanted.items()}, "referenced_symbol_addresses_unchanged": True}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("native-evidence", "formatter-evidence", "linker-evidence", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target"):
        raise ValueError("gate output must remain inside target/")
    out.mkdir(parents=True, exist_ok=False)
    report = {"schema": 1, "stage": "sg-runtime-text-utf8", "result": "running", "source_revision": revision(),
              "git_status_start": git_status(),
              "started_at": dt.datetime.now(dt.timezone.utc).isoformat(), "host": platform.platform(),
              "sources": {}, "external_sources": {}, "tools": {}, "steps": [], "qualifications": {},
              "cases": [], "full_runtime_complete": False, "final_sdk_complete": False, "self_build_fixed_point": False,
              "limits": ["UTF-8 validation and validated boundaries are emitted from SG; C product adapter only loads one raw byte",
                         "qualified SG syntax formatter is an explicitly selected preview, not the locked Rust formatter",
                         "C/Python probes and the development SDK are independent verification dependencies",
                         "production-adapter smoke has no instrumented read-count claim",
                         "no String algorithm replacement, full SDK packaging, concurrency or performance qualification"]}
    cache, tools, checkout = {}, {}, {}

    def save():
        (out / "evidence.json").write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    def read(path, expected=None, tool=False):
        path = Path(path).resolve()
        if path not in cache:
            cache[path] = path.read_bytes()
        raw = cache[path]
        observed = digest(raw)
        if expected is not None and observed != expected:
            raise ValueError("input identity differs: " + str(path))
        if not tool:
            if path.is_relative_to(ROOT):
                report["sources"][str(path.relative_to(ROOT))] = observed
                if not path.is_relative_to(ROOT / "target"):
                    checkout[path] = observed
            else:
                report["external_sources"][str(path)] = observed
        return raw

    def track_tool(name, path, expected=None):
        path = Path(path).resolve()
        raw = read(path, expected, tool=True)
        tools[path] = digest(raw)
        report["tools"][name] = {"path": str(path), "sha256": digest(raw)}
        return path

    def snapshot(source, destination, expected=None):
        raw = read(source, expected)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(raw)
        read(destination, digest(raw))
        return destination

    clean = {key: value for key, value in os.environ.items()
             if not key.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_", "PYTHON", "RUSTC", "RUSTDOC", "CARGO_"))
             and key.upper() not in {"RUSTFLAGS", "RUSTUP_TOOLCHAIN"}}
    pure = dict(clean, PATH="")

    def run(name, command, env=pure, expected=0, timeout=900, empty=False):
        command = list(map(str, command))
        started = time.monotonic()
        done = subprocess.run(command, cwd=out, env=env, capture_output=True, timeout=timeout)
        stdout, stderr = out / (name + ".stdout.log"), out / (name + ".stderr.log")
        stdout.write_bytes(done.stdout)
        stderr.write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": command, "exit_code": done.returncode,
                                "expected_exit_code": expected, "seconds": time.monotonic() - started,
                                "empty_path": env.get("PATH") == "", "stdout_sha256": sha(stdout), "stderr_sha256": sha(stderr)})
        save()
        print(name, done.returncode, flush=True)
        if done.returncode != expected or empty and (done.stdout or done.stderr):
            raise AssertionError(name + " returned unexpected status/output")
        return done

    def qualify(name, evidence, stage):
        evidence = evidence.resolve()
        if not evidence.is_relative_to(ROOT / "target"):
            raise ValueError("qualification must remain inside target/")
        raw = read(evidence)
        data = strict_json(raw)
        if type(data.get("schema")) is not int or data["schema"] != 1 or data.get("result") != "passed" or data.get("stage") != stage:
            raise ValueError(name + " qualification did not pass the required stage")
        if data.get("changed_sources") != [] or data.get("checkout_changes", []) != [] or any(value != [] for key, value in data.items() if key.startswith("changed_")):
            raise ValueError(name + " qualification failed its identity guard")
        if not data.get("sources") or not data.get("steps"):
            raise ValueError(name + " qualification lacks provenance")
        manifest = {}
        for key, expected in data["sources"].items():
            source = (ROOT / key).resolve()
            if not source.is_relative_to(ROOT):
                raise ValueError("qualified source escapes workspace")
            saved = snapshot(source, out / "qualified-sources" / name / source.relative_to(ROOT), expected)
            manifest[key] = {"sha256": expected, "snapshot": str(saved.relative_to(ROOT))}
        if name == "native":
            if data.get("changed_tools") != [] or not data.get("tools"):
                raise ValueError("native qualification lacks tool guards")
            for key, identity in data["tools"].items():
                track_tool("native-producer-" + key, identity["path"], identity["sha256"])
            source_binary, expected = evidence.parent / "native.exe", data["artifacts"]["native.exe"]
        else:
            source_binary, expected = Path(data["binary"]["path"]), data["binary"]["sha256"]
        source_binary = track_tool(name + "-qualified", source_binary, expected)
        binary = out / ("native.exe" if name == "native" else "sgfmt-preview.exe")
        binary.write_bytes(read(source_binary, expected, tool=True))
        track_tool(name, binary, expected)
        report["qualifications"][name] = {"path": str(evidence), "sha256": digest(raw), "stage": stage,
                                          "revision": data.get("source_revision", data.get("revision")), "sources": manifest,
                                          "steps": len(data["steps"]), "source_binary": str(source_binary), "copied_binary": str(binary)}
        return binary, data

    save()
    try:
        read(Path(__file__))
        read(ROOT / "selfhost/linker_evidence.py")
        python = track_tool("runner-python", sys.executable)
        report["python"] = {"version": sys.version, "executable": str(python), "sha256": tools[python]}
        report["json_negative_controls"] = oracle_controls()
        frozen_path = ROOT / "target/selfhost/runtime-text-oracle/freeze.json"
        frozen = strict_json(read(frozen_path))
        expected_frozen = {"selfhost/tests/runtime_text_probe.c", "selfhost/tests/runtime_text_cases.py"}
        if set(frozen["files"]) != expected_frozen:
            raise ValueError("probe freeze has an unexpected file set")
        for name, expected in frozen["files"].items():
            read(ROOT / name, expected)
        if frozen["corpus"]["sha256"] != CORPUS_SHA or any(frozen["corpus"][key] != value for key, value in COUNTS.items()):
            raise ValueError("probe freeze corpus differs")
        report["oracle_freeze"] = {"path": str(frozen_path), "sha256": digest(read(frozen_path)), "files": frozen["files"]}
        paths = ["selfhost/runtime/utf8.sg", "selfhost/runtime/memory_read.c", "selfhost/tests/runtime_text_smoke.sg",
                 "selfhost/tests/runtime_text_fixture.c", *sorted(expected_frozen)]
        implementation = out / "implementation"
        for name in paths:
            snapshot(ROOT / name, implementation / name)
        change = ROOT / "openspec/changes/add-selfhost-runtime-text-core"
        for path in change.rglob("*.md"):
            if path.name != "tasks.md":
                snapshot(path, implementation / path.relative_to(ROOT))
        report["excluded_ledger"] = "child tasks.md, README and umbrella ledger are not algorithm inputs"
        native, native_data = qualify("native", args.native_evidence, "sg-source-to-native-object")
        formatter, _ = qualify("formatter", args.formatter_evidence, "m2-formatter-cli-preview")
        linker, linked = copy_linker(ROOT, args.linker_evidence.resolve(), out, report)
        for name, identity in list(report["tools"].items()):
            track_tool(name, identity["path"], identity["sha256"])
        for name, expected in list(report["sources"].items()):
            read(ROOT / name, expected)
        sdk = linked.get("sdk")
        if not sdk:
            raise ValueError("historical SG linker lacks the qualified Kernel32 SDK")
        kernel = track_tool("sdk-kernel32", sdk["path"], sdk["sha256"])
        clang = track_tool("clang", native_data["tools"]["clang"]["path"], native_data["tools"]["clang"]["sha256"])
        lld = track_tool("clang-probe-linker", clang.parent / "lld-link.exe")
        objcopy = track_tool("llvm-objcopy-c-sdk-metadata", clang.parent / "llvm-objcopy.exe")
        objdump = track_tool("llvm-objdump-c-sdk-leaf-check", clang.parent / "llvm-objdump.exe")
        dlltool = track_tool("llvm-dlltool-ntdll-import", clang.parent / "llvm-dlltool.exe")
        readobj = track_tool("llvm-readobj-independent-pe", clang.parent / "llvm-readobj.exe")
        cenv = dict(clean, PATH=str(clang.parent) + os.pathsep + clean.get("PATH", ""), SOURCE_DATE_EPOCH="0")
        report["clang_version"] = run("clang-version", [clang, "--version"], cenv).stdout.decode("utf-8").strip()
        for name, tool in (("objcopy", objcopy), ("objdump", objdump)):
            report[name + "_version"] = run(name + "-version", [tool, "--version"], cenv).stdout.decode("utf-8").strip()
        if ctypes.sizeof(ctypes.c_void_p) != 8:
            raise ValueError("the NTDLL provider gate requires a native 64-bit Windows Python")
        ntdll = ctypes.WinDLL("ntdll.dll")
        windows = ctypes.WinDLL("kernel32.dll", use_last_error=True)
        windows.GetModuleFileNameW.argtypes = [ctypes.c_void_p, ctypes.c_wchar_p, ctypes.c_uint32]
        windows.GetModuleFileNameW.restype = ctypes.c_uint32
        windows.GetProcAddress.argtypes = [ctypes.c_void_p, ctypes.c_char_p]
        windows.GetProcAddress.restype = ctypes.c_void_p
        filename = ctypes.create_unicode_buffer(32768)
        length = windows.GetModuleFileNameW(ntdll._handle, filename, len(filename))
        if not 0 < length < len(filename):
            raise ValueError("cannot identify the actual loaded NTDLL provider")
        dll_path = Path(filename.value).resolve()
        dll_raw = read(dll_path)
        exports = run("inspect-actual-ntdll-export", [readobj, "--coff-exports", dll_path], cenv)
        matching = re.findall(r"Export\s*\{\s*Ordinal:\s*(\d+)\s*Name:\s*__chkstk\s*RVA:\s*(0x[0-9A-Fa-f]+)\s*\}", exports.stdout.decode("utf-8"))
        pointer = windows.GetProcAddress(ntdll._handle, b"__chkstk")
        if len(matching) != 1 or pointer is None or pointer - ntdll._handle != int(matching[0][1], 16):
            raise ValueError("the loaded stack-probe address differs from the actual DLL export")
        definition = out / "ntdll-chkstk.def"
        definition.write_bytes(b"LIBRARY ntdll.dll\nEXPORTS\n__chkstk\n")
        read(definition)
        stack_sdk = out / "ntdll-chkstk.lib"
        run("build-explicit-ntdll-import", [dlltool, "-m", "i386:x86-64", "-D", "ntdll.dll", "-d", definition, "-l", stack_sdk], cenv)
        read(stack_sdk)
        report["stack_probe_provider"] = {"path": str(dll_path), "sha256": digest(dll_raw), "name": "__chkstk",
                                           "ordinal": int(matching[0][0]), "rva": int(matching[0][1], 16),
                                           "loaded_export_rva_equal": True, "definition_sha256": sha(definition),
                                           "import_library_sha256": sha(stack_sdk), "public_windows_sdk_api_claimed": False,
                                           "scope": "explicit development ABI dependency on this pinned actual Windows NTDLL; not a claim that the public SDK ntdll.lib exports __chkstk"}
        report["execution_environment"] = {"sg_emit_path": "", "sg_link_path": "", "generated_program_path": "",
                                            "source_runtime_overrides": False, "c_driver": str(clang), "c_probe_linker": str(lld)}
        preview = out / "format-preview"
        preview.mkdir()
        for name in ("selfhost/runtime/utf8.sg", "selfhost/tests/runtime_text_smoke.sg"):
            source = implementation / name
            run("format-check-" + source.stem, [formatter, "--check", source], empty=True)
            formatted = run("format-preview-" + source.stem, [formatter, source])
            if formatted.stderr or not formatted.stdout:
                raise AssertionError("SG formatter preview returned an unexpected protocol")
            saved = preview / source.name
            saved.write_bytes(formatted.stdout)
            run("format-idempotent-" + source.stem, [formatter, "--check", saved], empty=True)
        generator = implementation / "selfhost/tests/runtime_text_cases.py"
        corpus_rows = []
        for name in ("corpus.bin", "corpus-repeat.bin"):
            corpus = out / name
            generated = run("generate-" + name, [python, "-X", "utf8", generator, "--output", corpus])
            exact_json(generated.stdout, {"sha256": CORPUS_SHA, **COUNTS})
            if generated.stderr:
                raise AssertionError("corpus generator wrote stderr")
            manifest = strict_json(read(corpus.with_suffix(corpus.suffix + ".json")))
            if manifest["generator_sha256"] != sha(generator) or manifest["python_executable_sha256"] != tools[python]:
                raise ValueError("generator manifest tool identity differs")
            checked = verify_corpus(read(corpus, CORPUS_SHA))
            corpus_rows.append(dict(path=str(corpus), manifest=str(corpus) + ".json", **checked))
        report["corpus"] = {"generations": corpus_rows, "identical_bytes": True}
        first, round_objects = {}, []
        cflags = ["-std=c11", "-Wall", "-Wextra", "-Werror", "-Werror=date-time", "-g0", "-fno-ident", "-O0"]

        def compile_c(name, source, target, definitions=(), sdk=False):
            discovered, consumed = target.with_suffix(".before.d"), target.with_suffix(".d")
            flags = cflags + (["-O2", "-fno-addrsig", "-fomit-frame-pointer", "-fno-asynchronous-unwind-tables", "-fno-unwind-tables"] if sdk else [])
            run(name + "-headers", [clang, *flags, *definitions, "-M", "-MT", "runtime_text_dependency", "-MF", discovered, source], cenv)
            headers = dependencies(discovered)
            if source.resolve() not in headers:
                raise AssertionError("C dependency manifest omitted its source")
            pins = {path: digest(read(path)) for path in headers}
            compiled = target.with_suffix(".raw.obj") if sdk else target
            run(name, [clang, *flags, *definitions, "-MD", "-MT", "runtime_text_dependency", "-MF", consumed, "-c", source, "-o", compiled], cenv)
            if dependencies(consumed) != headers or any(sha(path) != expected for path, expected in pins.items()):
                raise AssertionError("C source/header closure changed during compilation")
            report.setdefault("c_header_closures", {})[name] = {str(path): expected for path, expected in sorted(pins.items())}
            if sdk:
                raw = compiled.read_bytes()
                assembly = run(name + "-verify-leaf", [objdump, "-d", "--no-show-raw-insn", compiled], cenv).stdout.decode("utf-8")
                instructions = assembly.split("Disassembly of section .text:", 1)[1]
                if re.search(r"%(?:rsp|rbp)|\b(?:call\w*|push\w*|pop\w*|enter|leave)\b", instructions):
                    raise ValueError("C SDK helper is not a stack-free leaf")
                run(name + "-normalize-metadata", [objcopy, "--strip-unneeded", "--strip-symbol=.file", "--remove-section=.bss", "--remove-section=.data", compiled, target], cenv)
                normalized = bytearray(target.read_bytes())
                normalized[4:8] = bytes(4)
                target.write_bytes(normalized)
                exported = "sengoo_runtime_memory_read_u8" if source.stem == "memory_read" else "sengoo_runtime_text_fixture"
                proof = compare_c_metadata(raw, bytes(normalized), exported)
                proof.update(timestamp_operation="set only COFF header bytes 4..7 to zero after llvm-objcopy", leaf_disassembly_no_stack_or_calls=True)
                changes = {}
                text_at = struct.unpack_from("<I", normalized, 40)[0]
                changed = bytearray(normalized)
                changed[text_at] ^= 1
                changes["instruction-byte"] = changed
                symbol_at = struct.unpack_from("<I", normalized, 8)[0]
                changed = bytearray(normalized)
                struct.pack_into("<I", changed, symbol_at + 8, 1)
                changes["function-address"] = changed
                if ".rdata" in proof["sections"]:
                    data_at = struct.unpack_from("<I", normalized, 80)[0]
                    changed = bytearray(normalized)
                    changed[data_at] ^= 1
                    changes["static-data-byte"] = changed
                    relocation_at = struct.unpack_from("<I", normalized, 44)[0]
                    changed = bytearray(normalized)
                    struct.pack_into("<I", changed, relocation_at + 4, 2)
                    changes["relocation-target"] = changed
                for label, changed in changes.items():
                    try:
                        compare_c_metadata(raw, bytes(changed), exported)
                    except ValueError:
                        continue
                    raise AssertionError("C metadata comparison accepted " + label)
                proof["rejected_poison_controls"] = sorted(changes)
                report.setdefault("c_sdk_metadata_normalization", {})[name] = proof

        def inspect_stack_image(name, exe):
            image = exe.read_bytes()
            pe = struct.unpack_from("<I", image, 60)[0]
            if image[pe:pe + 4] != b"PE\0\0" or struct.unpack_from("<HH", image, pe + 4)[0] != 0x8664 or struct.unpack_from("<H", image, pe + 24)[0] != 0x20b:
                raise ValueError("unexpected generated PE target")
            reserve, commit = struct.unpack_from("<QQ", image, pe + 24 + 72)
            if commit != 4096 or reserve < 1048576:
                raise ValueError("generated PE stack commitment differs")
            inspected = run(name, [readobj, "--coff-imports", exe], cenv)
            imports = re.findall(r"Import\s*\{(.*?)\}", inspected.stdout.decode("utf-8"), re.S)
            actual = [entry for entry in imports if re.search(r"Name:\s*ntdll\.dll\s", entry, re.I)]
            if len(actual) != 1 or re.findall(r"Symbol:\s*([^\s(]+)", actual[0]) != ["__chkstk"]:
                raise ValueError("generated PE does not import the actual named NTDLL stack probe")
            return {"stack_reserve": reserve, "stack_commit": commit, "ntdll_chkstk_import_verified": True}

        for number, round_name in enumerate(("first fresh 世界 &", "second fresh 世界 &")):
            directory = out / round_name
            directory.mkdir()
            objects = {}
            for stem, original in (("utf8", "selfhost/runtime/utf8.sg"), ("runtime_text_smoke", "selfhost/tests/runtime_text_smoke.sg")):
                source = snapshot(implementation / original, directory / (stem + ".sg"))
                obj = directory / (stem + ".obj")
                emitted = run(str(number) + "-emit-" + stem, [native, source, obj, "--wrap"])
                exact_json(emitted.stdout, {"schema_version": 1, "ok": True, "phase": "object"})
                if emitted.stderr:
                    raise AssertionError("SG emitter wrote stderr on success")
                objects[stem] = obj
            for stem, original in (("memory_read", "selfhost/runtime/memory_read.c"), ("runtime_text_fixture", "selfhost/tests/runtime_text_fixture.c")):
                source = snapshot(implementation / original, directory / (stem + ".c"))
                obj = directory / (stem + ".obj")
                compile_c(str(number) + "-compile-" + stem, source, obj, sdk=True)
                objects[stem] = obj
            exe = directory / "runtime-text.exe"
            frame = sg_stack_frame(objects["utf8"].read_bytes(), "sengoo_sg_utf8_validate", 4097)
            run(str(number) + "-sg-link", [linker, "link", exe, "main", *objects.values(), kernel, stack_sdk])
            stack_image = inspect_stack_image(str(number) + "-inspect-stack-import", exe)
            executed = run(str(number) + "-sg-execute", [exe], expected=42, empty=True)
            artifacts = {name: sha(path) for name, path in objects.items()}
            artifacts["image"] = sha(exe)
            if number == 0:
                first = artifacts
            elif artifacts != first:
                raise AssertionError("fresh-directory COFF objects or SG-linked images differ")
            report["cases"].append({"round": round_name, "objects": artifacts, "execution_exit": executed.returncode,
                                     "stdout_empty": True, "stderr_empty": True, "entry": "main", "object_count": 4,
                                     "stack_frame": frame, "stack_image": stack_image, "sdk_import_libraries": 2})
            round_objects.append(objects)
        stress = out / "actual-large-sg-stack.sg"
        stress.write_bytes(("pub unsafe extern \"C\" fn main()->i64{let values=[42," + "0," * 4094 + "0];values[0]+values[4095]}").encode())
        read(stress)
        stress_obj, stress_exe = stress.with_suffix(".obj"), stress.with_suffix(".exe")
        emitted = run("large-sg-stack-emit", [native, stress, stress_obj, "--wrap"])
        exact_json(emitted.stdout, {"schema_version": 1, "ok": True, "phase": "object"})
        if emitted.stderr:
            raise AssertionError("large SG stack emitter wrote stderr")
        frame = sg_stack_frame(stress_obj.read_bytes(), "main", 65536)
        run("large-sg-stack-link", [linker, "link", stress_exe, "main", stress_obj, kernel, stack_sdk])
        stack_image = inspect_stack_image("large-sg-stack-inspect", stress_exe)
        run("large-sg-stack-cold-execute", [stress_exe], expected=42, empty=True)
        report["large_sg_stack_probe"] = {"source_sha256": sha(stress), "object_sha256": sha(stress_obj), "image_sha256": sha(stress_exe),
                                          "frame": frame, "image": stack_image, "cold_exit": 42,
                                          "scope": "real SG aggregate frame exceeding 64 KiB, PE initial commit 4 KiB, actual NTDLL probe import; no fake or patched SG prologue"}
        probe = implementation / "selfhost/tests/runtime_text_probe.c"
        probe_executables = {}
        for mode in ("instrumented", "production"):
            obj, exe = out / ("probe-" + mode + ".obj"), out / ("probe-" + mode + ".exe")
            compile_c("compile-probe-" + mode, probe, obj, ["-DSENGOO_UTF8_PRODUCTION_READ"] if mode == "production" else [])
            linked_objects = [obj, round_objects[0]["utf8"]]
            if mode == "production":
                linked_objects.append(round_objects[0]["memory_read"])
            run("link-probe-" + mode, [clang, "-fuse-ld=lld", "--ld-path=" + str(lld), *linked_objects, stack_sdk, "-o", exe], cenv)
            inspect_stack_image("inspect-probe-stack-" + mode, exe)
            probe_executables[mode] = exe
        report["probe_results"] = {}
        for name, command, mode in (("instrumented-corpus", [probe_executables["instrumented"], out / "corpus.bin"], "corpus"),
                                    ("instrumented-64MiB", [probe_executables["instrumented"], "--large-limit"], "large"),
                                    ("production-adapter-smoke", [probe_executables["production"], "--smoke"], "production")):
            checked = run(name, command)
            if checked.stderr:
                raise AssertionError("probe wrote stderr on success")
            report["probe_results"][name] = exact_json(checked.stdout, probe_expected(mode))
        bad = bytearray(read(out / "corpus.bin"))
        if bad[16:19] != bytes([0, 1, 1]):
            raise AssertionError("the corpus first record is not the expected empty input")
        bad[17:19] = bytes([0, 0])
        wrong = out / "wrong-empty-oracle.bin"
        wrong.write_bytes(bad)
        failed = run("wrong-empty-oracle", [probe_executables["instrumented"], wrong], expected=1)
        marker = b"reason=validation-oracle length=0 offset=-2 expected=0 actual=1"
        if failed.stdout or marker not in failed.stderr:
            raise AssertionError("negative control did not reach the algorithm oracle")
        report["wrong_oracle_control"] = {"exit": 1, "reason": "validation-oracle", "first_record_only_changed": True,
                                            "bytes": len(bad), "sha256": digest(bad)}
        report["artifacts"] = {str(path.relative_to(out)): sha(path) for path in out.rglob("*")
                               if path.is_file() and path.suffix in {".obj", ".exe"}}
        report.update(result="passed", clean_directories=2, deterministic_sg_and_normalized_c_objects_and_images=True,
                      raw_c_objects_reproducibility_claimed=False,
                      large_limit_bytes=TEXT_LIMIT, complete_unicode_scalar_corpus=True)
    except BaseException as error:
        report.update(result="failed", error=type(error).__name__ + ": " + str(error))
    finally:
        try:
            report["changed_sources"] = [name for name, expected in report["sources"].items()
                                         if not (ROOT / name).is_file() or sha(ROOT / name) != expected]
            report["changed_external_sources"] = [name for name, expected in report["external_sources"].items()
                                                  if not Path(name).is_file() or sha(Path(name)) != expected]
            report["changed_tools"] = [name for name, identity in report["tools"].items()
                                       if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
            report["checkout_changes"] = [str(path.relative_to(ROOT)) for path, expected in checkout.items()
                                           if not path.is_file() or sha(path) != expected]
            report["initial_cached_reads"] = {str(path): 1 for path in cache}
            report["read_policy"] = "runner initial source consumption is cached once; final SHA guards reread files; historical linker helper independently rechecks its complete chain"
            report["source_revision_end"] = revision()
            report["git_status_end"] = git_status()
            report["git_status_policy"] = "dirty/unrelated concurrent edits are disclosed; only consumed source/tool fingerprints determine identity guard success"
            if any(report[key] for key in ("changed_sources", "changed_external_sources", "changed_tools", "checkout_changes")):
                report.update(result="failed", source_guard_failure="consumed source, header or tool changed")
        except BaseException as error:
            report.update(result="failed", finally_error=repr(error))
        report["completed_at"] = dt.datetime.now(dt.timezone.utc).isoformat()
        save()
    print(json.dumps({key: report.get(key) for key in ("result", "error", "changed_sources", "changed_external_sources", "changed_tools", "checkout_changes")}))
    return 0 if report["result"] == "passed" else 1


if __name__ == "__main__":
    raise SystemExit(main())
