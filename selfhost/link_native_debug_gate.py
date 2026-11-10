"""Qualify automatic SG source-to-native CodeView/PDB mapping and live debugging."""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import sys
import time

from link_debug_gate import ROOT, sha, u16, u32, msf_streams
from pdb_type_hash_oracle import validate_tpi_hashes


def closure():
    paths = {path.stem: path for path in (ROOT / "selfhost/src").glob("*.sg")}
    entry = ROOT / "selfhost/tests/link_native_debug.sg"
    pending = re.findall(r"import\s+selfhost_([a-zA-Z0-9_]+)\s*;", entry.read_text(encoding="utf-8"))
    result = {}
    while pending:
        name = pending.pop()
        if name in result:
            continue
        result[name] = paths[name]
        pending += re.findall(r"import\s+selfhost_([a-zA-Z0-9_]+)\s*;", paths[name].read_text(encoding="utf-8"))
    return result


def validate_pair(image, pdb, source):
    streams = msf_streams(pdb)
    if len(streams) != 12:
        raise AssertionError("PDB stream set differs")
    validate_tpi_hashes(streams)
    rva = u32(image, 248)
    debug_raw = None
    for index in range(u16(image, 70)):
        row = 328 + index * 40
        size, start, _, raw = struct.unpack_from("<IIII", image, row + 8)
        if start <= rva < start + size:
            debug_raw = raw + rva - start
    if debug_raw is None or image[debug_raw + 28:debug_raw + 32] != b"RSDS" or image[debug_raw + 32:debug_raw + 48] != streams[1][12:28]:
        raise AssertionError("source native PE/PDB signature differs")
    module_info = streams[3][64:128]
    size, lines_size = u32(module_info, 36), u32(module_info, 44)
    module = streams[6]
    functions, locals_, ranges = [], [], []
    at = 4
    while at < size:
        length, kind = struct.unpack_from("<HH", module, at)
        if kind == 0x1110:
            name = module[at + 39:module.index(0, at + 39)].decode()
            functions.append({"name": name, "offset": u32(module, at + 32), "segment": u16(module, at + 36), "size": u32(module, at + 16), "prologue": u32(module, at + 20), "epilogue": u32(module, at + 24)})
        elif kind == 0x113E:
            locals_.append(module[at + 10:module.index(0, at + 10)].decode())
        elif kind == 0x1142:
            ranges.append({"stack": u32(module, at + 4), "start": u32(module, at + 8), "segment": u16(module, at + 12), "size": u16(module, at + 14)})
        at += length + 2
    lines = []
    checksum = None
    at = size
    while at < size + lines_size:
        kind, length = struct.unpack_from("<II", module, at)
        data = module[at + 8:at + 8 + length]
        if kind == 0xF4:
            checksum = data[6:38]
        elif kind == 0xF2:
            count = u32(data, 16)
            function = next(row for row in functions if row["offset"] == u32(data, 0) and row["segment"] == u16(data, 4))
            if function["size"] != u32(data, 8):
                raise AssertionError("CodeView function size differs")
            for index in range(count):
                offset, encoded = struct.unpack_from("<II", data, 24 + index * 8)
                line = encoded & 0xFFFFFF
                if offset >= function["size"] or not 1 <= line <= len(source.splitlines()) + 1:
                    raise AssertionError("automatic native line map extent differs")
                lines.append({"function": function["name"], "offset": offset, "line": line})
        at += length + 8
    if checksum != hashlib.sha256(source).digest() or "answer" not in locals_:
        raise AssertionError("automatic source/local identity differs")
    return {"guid": streams[1][12:28].hex(), "source_sha256": checksum.hex(), "functions": functions, "locals": locals_, "ranges": ranges, "lines": lines}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter", "lldb-python-home", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("output must stay in target/")
    output.mkdir(parents=True, exist_ok=True)
    modules = closure()
    entry = ROOT / "selfhost/tests/link_native_debug.sg"
    snapshot_root = output / "producer-snapshot"
    origins = [entry, *modules.values()]
    origins += [path for path in (ROOT / "tools/stdlib").rglob("*") if path.is_file() and not any(part in {"target", "build", "__pycache__"} for part in path.relative_to(ROOT / "tools/stdlib").parts)]
    producer_origins, snapshots = {}, {}
    for path in sorted(set(origins)):
        data = path.read_bytes()
        producer_origins[str(path.relative_to(ROOT))] = hashlib.sha256(data).hexdigest()
        destination = snapshot_root / path.relative_to(ROOT)
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(data)
        snapshots[str(destination.relative_to(ROOT))] = sha(destination)
    if any(sha(ROOT / name) != digest for name, digest in producer_origins.items()):
        raise AssertionError("source changed while capturing immutable producer snapshot")
    modules = {name: snapshot_root / path.relative_to(ROOT) for name, path in modules.items()}
    producer_entry = snapshot_root / entry.relative_to(ROOT)
    tools = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter")}
    tools.update(debugger=tools["clang"].parent / "lldb.exe", pdb_inspector=tools["clang"].parent / "llvm-pdbutil.exe", coff_inspector=tools["clang"].parent / "llvm-readobj.exe")
    python_home = args.lldb_python_home.resolve()
    for path in sorted(python_home.glob("*.pyd")) + sorted(python_home.glob("*.dll")) + [python_home / "python310.zip", tools["clang"].parent / "liblldb.dll"]:
        tools["debug-dependency-" + path.name] = path
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
    fixtures = [("multiple-functions", ROOT / "selfhost/tests/link_native_debug_source.sg", 6), ("multiple-returns", ROOT / "selfhost/tests/link_debug_ranges_source.sg", 4),
                ("floating-parameters", ROOT / "selfhost/tests/link_native_debug_types_source.sg", 3), ("mixed-scalar-parameters", ROOT / "selfhost/tests/link_native_debug_scalars_source.sg", 4),
                ("readonly-reference", ROOT / "selfhost/tests/link_native_debug_refs_source.sg", 3), ("mutable-reference", ROOT / "selfhost/tests/link_native_debug_mut_ref_source.sg", 2),
                ("raw-void-pointer", ROOT / "selfhost/tests/link_native_debug_raw_ptr_source.sg", 6),
                ("aggregate-locals", ROOT / "selfhost/tests/link_native_debug_aggregate_source.sg", 11),
                ("tagged-enum-local", ROOT / "selfhost/tests/link_native_debug_enum_source.sg", 8)]
    inputs = [Path(__file__).resolve(), ROOT / "selfhost/link_debug_gate.py", ROOT / "selfhost/bootstrap.lock", ROOT / "Cargo.toml", ROOT / "Cargo.lock", ROOT / "rust-toolchain.toml"]
    inputs += [source for _, source, _ in fixtures] + [ROOT / name for name in snapshots]
    for folder in (ROOT / "runtime",):
        inputs += [path for path in folder.rglob("*") if path.is_file() and not any(part in {"target", "build", "__pycache__"} for part in path.relative_to(folder).parts)]
    if (ROOT / ".cargo/config.toml").is_file():
        inputs.append(ROOT / ".cargo/config.toml")
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    cargo_configs = {str(path.resolve()): sha(path) for path in (cargo_home / "config.toml", cargo_home / "config") if path.is_file()}
    revision = lambda: subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    report = {"schema": 1, "result": "running", "stage": "automatic-sg-source-native-codeview-pdb", "m5_complete": False,
              "source_revision": revision(), "started_at": dt.datetime.now(dt.timezone.utc).isoformat(), "host": platform.platform(), "python": sys.version,
              "sources": {str(p.relative_to(ROOT)): sha(p) for p in sorted(set(inputs))}, "modules": sorted(modules), "tools": {}, "steps": [],
              "producer_source_origins": producer_origins, "producer_snapshots": snapshots,
              "user_cargo_configs": cargo_configs,
              "limits": ["one unchanged UTF-8 source; supported scalar/pointer/reference, nongeneric struct/tuple/array/tagged-enum types with entry-block local initialization; main remains signed-i64()",
                         "instruction/terminator/prologue/epilogue code ranges and stack slots come from the SG native compiler, not fixture addresses",
                         "locals with moves/storage-dead/unsupported lexical scope or unproven initialization fail explicitly; generic/class/recursive/function types and full async/reflection/SDK debug corpus remain open",
                         "enum view preserves actual tag plus a payload union; active-variant conditional display and inactive-payload initialization are not qualified",
                         "immutable SG module and stdlib/runtime.c producer snapshots are hashed independently of later checkout changes",
                         "MIR maps are checked and deterministically merged in SG; product source-to-object-to-link-to-PDB runs have no compiler/linker tools in PATH",
                         "this test compiler selects wrapping integer arithmetic and does not claim checked runtime panic helper integration"]}
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_", "PYTHON", "LLDB_")):
            env.pop(name)
    env.update(SENGOO_STDLIB=str(snapshot_root / "tools/stdlib"), SENGOO_RUNTIME=str(snapshot_root / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(tools["clang"]),
               SENGOO_MODULE_MAP=";".join("selfhost_" + name + "=" + str(path) for name, path in modules.items()))
    env["PATH"] = str(tools["clang"].parent) + os.pathsep + env.get("PATH", "")
    for name in ("cargo", "rustc", "rustup"):
        resolved = shutil.which(name, path=env["PATH"])
        if resolved is None:
            raise AssertionError("development producer tool is unavailable: " + name)
        tools[name] = Path(resolved).resolve()
    for name in ("cargo", "rustc"):
        resolved = subprocess.check_output([str(tools["rustup"]), "which", name], cwd=ROOT, env=env, text=True, timeout=30).strip()
        tools[name + "-resolved"] = Path(resolved).resolve()
    empty = output / "empty-toolchain-path"
    empty.mkdir(exist_ok=True)
    pure_env = {key: value for key, value in env.items() if not key.upper().startswith("SENGOO_")}
    pure_env["PATH"] = str(empty)
    debug_env = dict(env, PYTHONHOME=str(python_home), PYTHONPATH=os.pathsep.join((str(python_home / "python310.zip"), str(python_home))), LLDB_USE_NATIVE_PDB_READER="1")
    debug_env["PATH"] = str(python_home) + os.pathsep + debug_env["PATH"]

    def run(name, command, code=0, marker=None, pure=False, debugger=False):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=debug_env if debugger else pure_env if pure else env, capture_output=True, timeout=1200)
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
            if name in {"cargo", "rustc", "rustup", "cargo-resolved", "rustc-resolved"}:
                report["tools"][name]["version"] = subprocess.check_output([str(path), "--version"], cwd=ROOT, env=env, text=True, stderr=subprocess.STDOUT, timeout=30).strip()
        for path in (ROOT / "selfhost/src/link_native_debug.sg", ROOT / "selfhost/tests/link_native_debug.sg"):
            run("format-" + path.stem, [tools["formatter"], "--check", path])
        binary = output / "sg-native-debug.exe"
        run("build-sg", [tools["seed"], "--runtime-mode", "source-development", "build", producer_entry, "--force-rebuild", "-O", "0", "-o", binary])
        artifacts, cases = {binary.name: sha(binary)}, []
        for name, source, line in fixtures:
            obj, image, pdb = output / (name + ".obj"), output / (name + ".exe"), output / (name + ".pdb")
            command = [binary, source.as_posix(), obj.as_posix(), image.as_posix(), pdb.as_posix()]
            run(name + "-sg-source-to-pdb", command, pure=True, marker=b"selfhost SG source/native/COFF/CodeView/PDB exact resources: ok")
            before = [path.read_bytes() for path in (obj, image, pdb)]
            run(name + "-deterministic-repeat", command, pure=True)
            if before != [path.read_bytes() for path in (obj, image, pdb)]:
                raise AssertionError("automatic native object/PE/PDB bytes differ on repeat")
            pair = validate_pair(image.read_bytes(), pdb.read_bytes(), source.read_bytes())
            if name == "multiple-functions" and next(row["offset"] for row in pair["functions"] if row["name"] == "main") == 0:
                raise AssertionError("main did not test nonzero CodeView addresses")
            if name == "multiple-returns" and len(pair["ranges"]) < 2:
                raise AssertionError("automatic multiple returns did not split local ranges")
            run(name + "-execute", [image], 42, pure=True)
            run(name + "-inspect-coff", [tools["coff_inspector"], "--codeview", "--relocations", obj], marker=b"S_LOCAL")
            run(name + "-inspect-pdb", [tools["pdb_inspector"], "dump", "--summary", "--modules", "--symbols", "--types", "--globals", "--publics", pdb], marker=b"answer")
            if name == "raw-void-pointer":
                run(name + "-inspect-imports", [tools["coff_inspector"], "--coff-imports", image], marker=b"GetCurrentProcess")
            variables = "pair tuple array answer" if name == "aggregate-locals" else "choice answer" if name == "tagged-enum-local" else "left right answer" if name == "floating-parameters" else "flag small big answer" if name == "mixed-scalar-parameters" else "value answer" if name in ("readonly-reference", "raw-void-pointer") else "value" if name == "mutable-reference" else "answer"
            commands = ["breakpoint set --file " + source.name + " --line " + str(line), "run", "frame variable " + variables, "thread step-over", "frame variable " + ("value" if name == "mutable-reference" else "answer"), "continue"]
            debugged = run(name + "-live-source-step-local", [tools["debugger"], "--batch", "--no-lldbinit", *[item for command in commands for item in ("-o", command)], image], debugger=True)
            for marker in (b"stop reason = breakpoint", b"(&value = 40)" if name == "mutable-reference" else b"answer = 40", b"stop reason = step over", b"(&value = 42)" if name == "mutable-reference" else b"answer = 42", b"exited with status = 42"):
                if marker not in debugged:
                    raise AssertionError("automatic source debugger evidence missing: " + repr(marker))
            additional = (b"(double) left = 20", b"(double) right = 20", b"(double) answer = 42") if name == "floating-parameters" else (b"flag = true", b"small = 40", b"big = 2") if name == "mixed-scalar-parameters" else (b"(const long long &)", b"(&value = 40)") if name == "readonly-reference" else (b"(long long &)",) if name == "mutable-reference" else (b"(void *)", b"0xffffffffffffffff") if name == "raw-void-pointer" else ()
            if any(marker not in debugged for marker in additional):
                raise AssertionError("automatic typed parameter values differ")
            aggregate_values = (b"flag = true", b"small = 40", b"big = 2", b"_0 = 40", b"_1 = 2", b"[0] = 40", b"[1] = 2") if name == "aggregate-locals" else (b"__tag = Value", b"_0 = 40") if name == "tagged-enum-local" else ()
            if any(marker not in debugged for marker in aggregate_values):
                raise AssertionError("automatic aggregate fields differ")
            artifacts.update({path.name: sha(path) for path in (obj, image, pdb)})
            cases.append({"name": name, "pair": pair, "exact_resource_scopes_per_invocation": 5})
        negatives = []
        source = fixtures[0][1]
        for number in range(1, 10):
            expected = 7301 if number == 4 else 7311
            obj, image, pdb = (output / ("invalid-" + str(number) + suffix) for suffix in (".obj", ".exe", ".pdb"))
            sentinel = b"preserve all previous automatic debug artifacts"
            for path in (obj, image, pdb):
                path.write_bytes(sentinel)
            result = run("reject-native-map-" + str(number), [binary, source.as_posix(), obj.as_posix(), image.as_posix(), pdb.as_posix(), chr(64 + number)], 65, pure=True)
            if result.strip() != str(expected).encode() or any(path.read_bytes() != sentinel for path in (obj, image, pdb)):
                raise AssertionError("native map diagnostic/output preservation differs: " + repr(result))
            negatives.append({"case": number, "expected_code": expected, "previous_outputs_preserved": True})
        unsupported = {
            "floating-return": "def main() -> f64 { return 42.0; }\n",
            "generic-aggregate-reference": "struct Pair<T> { left: T, right: T }\ndef other(value: &Pair<i64>) -> i64 { return 0; }\ndef main() -> i64 { return 42; }\n",
            "recursive-aggregate-reference": "struct Node { next: *const Node }\ndef other(value: &Node) -> i64 { return 0; }\ndef main() -> i64 { return 42; }\n",
            "nested-reference": "def other(value: & &i64) -> i64 { return 0; }\ndef main() -> i64 { return 42; }\n",
            "nested-entry-local": "def main() -> i64 { { let answer: i64 = 40; }; return 42; }\n",
            "branch-local-init": "def main() -> i64 { if true { let answer: i64 = 42; return answer; }; return 1; }\n",
        }
        for number in range(10, 16):
            obj.write_bytes(b"previous object")
            image.write_bytes(b"previous image")
            pdb.write_bytes(b"previous pdb")
            rejected = run("reject-aggregate-name-map-" + str(number), [binary, ROOT / "selfhost/tests/link_native_debug_aggregate_source.sg", obj, image, pdb, chr(64 + number)], 65, pure=True)
            expected = 7301 if number == 15 else 7313 if number == 14 else 7311
            if rejected.strip() != str(expected).encode() or (obj.read_bytes(), image.read_bytes(), pdb.read_bytes()) != (b"previous object", b"previous image", b"previous pdb"):
                raise AssertionError("aggregate field-name map rejection differs")
            negatives.append({"case": number, "expected_code": expected, "previous_outputs_preserved": True})
        for name, text in unsupported.items():
            path = output / (name + ".sg")
            path.write_text(text, encoding="utf-8")
            obj, image, pdb = (output / (name + suffix) for suffix in (".obj", ".exe", ".pdb"))
            result = run("reject-unsupported-" + name, [binary, path.as_posix(), obj.as_posix(), image.as_posix(), pdb.as_posix()], 65, pure=True)
            if result.strip() != b"7313" or any(path.exists() for path in (obj, image, pdb)):
                raise AssertionError("unsupported native debug input was misrepresented: " + repr(result))
            negatives.append({"name": name, "expected_code": 7313, "input_sha256": sha(path)})
        report.update(result="passed", cases=cases, negative_cases=negatives, artifacts=artifacts)
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items() if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        report.update(changed_sources=changed, changed_tools=changed_tools, source_revision_end=revision(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        report["producer_checkout_changes"] = [name for name, digest in producer_origins.items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_configs = [name for name, digest in cargo_configs.items() if not Path(name).is_file() or sha(Path(name)) != digest]
        report["changed_user_cargo_configs"] = changed_configs
        if changed or changed_tools or changed_configs:
            report.update(result="failed", error="automatic native debug input identity changed")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools or changed_configs:
            raise AssertionError("automatic native debug input identity changed")


if __name__ == "__main__":
    main()
