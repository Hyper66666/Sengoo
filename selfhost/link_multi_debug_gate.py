"""Qualify one SG object/PDB module with multiple physical source files."""
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
import tomllib

from link_debug_gate import ROOT, sha, u16, u32, msf_streams


def rust_override(name):
    upper = name.upper()
    return upper.startswith(("RUSTC", "RUSTDOC", "RUSTUP_TOOLCHAIN", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTDOC", "CARGO_PROFILE_")) or upper in {
        "CARGO", "RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS",
        "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_RUSTDOCFLAGS", "CARGO_BUILD_TARGET", "CARGO_TARGET_DIR", "CARGO_INCREMENTAL"
    } or upper.startswith("CARGO_TARGET_") and upper.endswith(("_RUSTFLAGS", "_RUSTDOCFLAGS", "_RUSTC"))


def closure(entry):
    result, pending = {}, [entry]
    while pending:
        path = pending.pop()
        if path in result:
            continue
        result[path] = sha(path)
        pending += [ROOT / "selfhost/src" / (name + ".sg") for name in re.findall(r"import\s+selfhost_([A-Za-z0-9_]+)\s*;", path.read_text(encoding="utf-8"))]
    return result


def coff_fragments(raw):
    for index in range(u16(raw, 2)):
        row = 20 + index * 40
        if raw[row:row + 8] == b".debug$S":
            first, size = u32(raw, row + 20), u32(raw, row + 16)
            result, at = {}, first + 4
            while at < first + size:
                kind, length = struct.unpack_from("<II", raw, at)
                result.setdefault(kind, []).append((at, at + 8, length))
                at = (at + 8 + length + 3) & ~3
            return result
    raise AssertionError("COFF source debug section is missing")


def mutations(raw):
    fragments = coff_fragments(raw)
    _, checksums, _ = fragments[0xF4][0]
    _, strings, _ = fragments[0xF3][0]
    _, line, _ = fragments[0xF2][0]
    def change(offset, value, fmt="<I"):
        result = bytearray(raw)
        struct.pack_into(fmt, result, offset, value)
        return result
    second_path = u32(raw, checksums + 40)
    return [("unaligned-checksum-reference", change(line + 12, 1), 7303),
            ("unresolved-checksum-reference", change(line + 12, 80), 7301),
            ("known-file-wrong-function", change(line + 12, 40 - u32(raw, line + 12)), 7301),
            ("duplicate-checksum-name-offset", change(checksums + 40, u32(raw, checksums)), 7303),
            ("second-file-sha256", change(checksums + 46, raw[checksums + 46] ^ 128, "<B"), 7301),
            ("second-file-path", change(strings + second_path, ord("X"), "<B"), 7301),
            ("checksum-padding", change(checksums + 78, 1, "<B"), 7301),
            ("empty-file-block", change(line + 16, 0), 7301),
            ("line-count-cap", change(line + 16, 65537), 7301)]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter", "lldb-python-home", "output"):
        parser.add_argument("--" + name, required=True, type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("output must stay inside target/")
    output.mkdir(parents=True, exist_ok=False)
    entry = ROOT / "selfhost/tests/link_native_debug_modules.sg"
    originals = closure(entry)
    originals.update({p: sha(p) for p in (ROOT / "tools/stdlib").rglob("*") if p.is_file() and not any(v in {"target", "build", "__pycache__"} for v in p.relative_to(ROOT / "tools/stdlib").parts)})
    snapshot = output / "producer-snapshot"
    sources = {str(Path(__file__).relative_to(ROOT)): sha(Path(__file__)), "selfhost/link_debug_gate.py": sha(ROOT / "selfhost/link_debug_gate.py"), "selfhost/bootstrap.lock": sha(ROOT / "selfhost/bootstrap.lock")}
    for path, expected in originals.items():
        raw = path.read_bytes()
        if hashlib.sha256(raw).hexdigest() != expected:
            raise AssertionError("producer changed during capture")
        saved = snapshot / path.relative_to(ROOT)
        saved.parent.mkdir(parents=True, exist_ok=True)
        saved.write_bytes(raw)
        if saved.read_bytes() != raw:
            raise AssertionError("producer copy differs from captured bytes")
        sources[str(saved.relative_to(ROOT))] = sha(saved)
    if any(not path.is_file() or sha(path) != expected for path, expected in originals.items()):
        raise AssertionError("producer checkout changed before capture completed")
    for folder in (ROOT / "runtime",):
        sources.update({str(p.relative_to(ROOT)): sha(p) for p in folder.rglob("*") if p.is_file() and not any(v in {"target", "build", "__pycache__"} for v in p.relative_to(folder).parts)})
    sources.update({name: sha(ROOT / name) for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config", ".cargo/config.toml") if (ROOT / name).is_file()})
    tools = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter")}
    tools.update(lldb=tools["clang"].parent / "lldb.exe", pdbutil=tools["clang"].parent / "llvm-pdbutil.exe", readobj=tools["clang"].parent / "llvm-readobj.exe", liblldb=tools["clang"].parent / "liblldb.dll")
    tools["runner-python"] = Path(sys.executable).resolve()
    py = args.lldb_python_home.resolve()
    for path in sorted(py.glob("*.dll")) + sorted(py.glob("*.pyd")) + [py / "python310.zip"]:
        tools["lldb-" + path.name] = path
    env = {k: v for k, v in os.environ.items() if not k.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_", "PYTHON", "LLDB_")) and not rust_override(k)}
    env["PATH"] = str(tools["clang"].parent) + os.pathsep + env.get("PATH", "")
    for name in ("cargo", "rustc", "rustup"):
        path = shutil.which(name, path=env["PATH"])
        if path is None:
            raise ValueError("missing development runtime tool")
        tools[name] = Path(path).resolve()
    for name in ("cargo", "rustc"):
        tools[name + "-selected"] = Path(subprocess.check_output([tools["rustup"], "which", name], cwd=ROOT, env=env, text=True).strip()).resolve()
    if tools["cargo-selected"].parent != tools["rustc-selected"].parent:
        raise ValueError("selected Cargo and rustc differ")
    env.update(CARGO=str(tools["cargo-selected"]), RUSTC=str(tools["rustc-selected"]), RUSTC_WRAPPER="", RUSTC_WORKSPACE_WRAPPER="", CARGO_BUILD_RUSTC=str(tools["rustc-selected"]), CARGO_BUILD_RUSTC_WRAPPER="", CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER="", RUSTUP_TOOLCHAIN=tools["rustc-selected"].parent.parent.name)
    env["PATH"] = str(tools["cargo-selected"].parent) + os.pathsep + env["PATH"]
    env.update(SENGOO_STDLIB=str(snapshot / "tools/stdlib"), SENGOO_RUNTIME=str(snapshot / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(tools["clang"]), SENGOO_MODULE_MAP=";".join("selfhost_" + path.stem + "=" + str(snapshot / path.relative_to(ROOT)) for path in originals if path.parent == ROOT / "selfhost/src"))
    pure = {k: v for k, v in env.items() if not k.upper().startswith("SENGOO_")}
    pure["PATH"] = ""
    debug = dict(env, PYTHONHOME=str(py), PYTHONPATH=os.pathsep.join((str(py / "python310.zip"), str(py))), LLDB_USE_NATIVE_PDB_READER="1")
    debug["PATH"] = str(py) + os.pathsep + debug["PATH"]
    report = {"schema": 1, "stage": "one-module-multi-source-native-debug", "result": "running", "m5_complete": False,
              "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), "started_at": dt.datetime.now(dt.timezone.utc).isoformat(), "host": platform.platform(),
              "sources": sources, "producer_origins": {str(p.relative_to(ROOT)): h for p, h in originals.items()}, "tools": {n: {"path": str(p), "sha256": sha(p)} for n, p in tools.items()}, "steps": [],
              "limits": ["one SG object and one PDB module with checked physical function-source registry; one physical file per function; independent COFF modules and inline-origin policies remain open"]}
    report["removed_rust_environment_overrides"] = sorted(k for k in os.environ if rust_override(k))
    report["pinned_rust_environment"] = {k: env[k] for k in ("CARGO", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN")}
    cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo")))
    configs = {parent / ".cargo" / name for parent in (ROOT, *ROOT.parents) for name in ("config", "config.toml")}
    configs.update(cargo_home / name for name in ("config", "config.toml"))
    report["cargo_config_candidates"] = {}
    for path in configs:
        raw = path.read_bytes() if path.is_file() else None
        report["cargo_config_candidates"][str(path.resolve())] = None if raw is None else hashlib.sha256(raw).hexdigest()
        if raw is not None and "include" in tomllib.loads(raw.decode("utf-8")):
            raise ValueError("included Cargo configs require their own complete manifest")
    def run(name, command, mode="build", code=0):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=pure if mode == "pure" else debug if mode == "debug" else env, capture_output=True, timeout=900)
        (output / (name + ".stdout.log")).write_bytes(done.stdout)
        (output / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)), "exit_code": done.returncode, "expected_exit_code": code, "seconds": time.monotonic() - started, "empty_path": mode == "pure"})
        print(name, done.returncode, flush=True)
        if done.returncode != code:
            raise AssertionError(name + ": " + str(done.returncode))
        return done.stdout + done.stderr
    try:
        lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text(encoding="utf-8"))
        for name in ("seed", "clang", "formatter"):
            if report["tools"][name]["sha256"] != lock[name]["sha256"]:
                raise ValueError(name + " identity differs from verified toolchain")
        for name in ("cargo", "rustc"):
            report["tools"][name + "-selected"]["version"] = run(name + "-version", [tools[name + "-selected"], "-vV"]).decode("utf-8").strip()
        probe = output / "rust-tool-probe"
        (probe / "src").mkdir(parents=True)
        (probe / "Cargo.toml").write_text('[package]\nname="multi-debug-rust-tool"\nversion="0.0.0"\nedition="2021"\n[workspace]\n', encoding="utf-8")
        (probe / "build.rs").write_text('fn main(){println!("cargo:warning=multi_debug_rustc={}",std::env::var("RUSTC").unwrap());}\n', encoding="utf-8")
        (probe / "src/lib.rs").write_text('pub fn tool_probe()->u64{42}\n', encoding="utf-8")
        for path in (probe / "Cargo.toml", probe / "build.rs", probe / "src/lib.rs"):
            report["sources"][str(path.relative_to(ROOT))] = sha(path)
        observed = run("verify-cargo-actual-rustc", [tools["cargo-selected"], "build", "--offline", "--verbose", "--manifest-path", probe / "Cargo.toml", "--target-dir", probe / "target"]).decode("utf-8")
        compiler_runs = [line for line in observed.splitlines() if line.lstrip().startswith("Running ") and str(tools["rustc-selected"]).casefold() in line.casefold()]
        if "multi_debug_rustc=" + str(tools["rustc-selected"]) not in observed or len(compiler_runs) < 2:
            raise AssertionError("Cargo did not actually select the pinned rustc")
        report["cargo_compiler_probe"] = {"compiler_invocations": compiler_runs, "build_script_rustc_matches": True}
        for path in (entry, *[p for p in originals if p.parent == ROOT / "selfhost/src" and p.stem.startswith("link_")]):
            run("format-" + path.stem, [tools["formatter"], "--check", snapshot / path.relative_to(ROOT)])
        binary = output / "multi-debug.exe"
        run("build-sg", [tools["seed"], "--runtime-mode", "source-development", "build", snapshot / entry.relative_to(ROOT), "--force-rebuild", "-O", "0", "-o", binary])
        for mode in ("--fail-equality", "--fail-truth"):
            failed = run("assertion" + mode, [binary, mode], "pure", 1)
            if b"link_native_debug_modules.sg" not in failed:
                raise AssertionError("assertion negative control did not report its test source")
        directory = output / "sources 世界 &"
        directory.mkdir()
        for name, text in {"main.sg": "import worker;\ndef main() -> i64 {\n    let input: i64 = 40;\n    let answer: i64 = worker(input);\n    return answer;\n}\n", "worker.sg": "pub def worker(value: i64) -> i64 {\n    let mut answer: i64 = value;\n    answer = answer + 2;\n    return answer;\n}\n"}.items():
            path = directory / name
            path.write_bytes(text.encode("utf-8"))
            report["sources"][str(path.relative_to(ROOT))] = sha(path)
        main_source = directory / "main.sg"
        obj, image, pdb = [output / ("physical" + suffix) for suffix in (".obj", ".exe", ".pdb")]
        command = [binary, main_source, obj, image, pdb]
        succeeded = run("sg-source-coff-link-pdb-first-resources", command, "pure")
        if succeeded.strip() != b"selfhost multi-source native debug first-call resources: ok":
            raise AssertionError("multi-source first-call resource checks did not complete")
        original = {path.name: path.read_bytes() for path in (obj, image, pdb)}
        if run("deterministic-repeat", command, "pure").strip() != succeeded.strip():
            raise AssertionError("repeat resource checks did not complete")
        if original != {path.name: path.read_bytes() for path in (obj, image, pdb)}:
            raise AssertionError("multi-source artifacts are nondeterministic")
        run("execute", [image], "pure", 42)
        inspected = run("inspect-coff", [tools["readobj"], "--codeview", "--relocations", obj])
        inspected += run("inspect-pdb", [tools["pdbutil"], "dump", "--summary", "--modules", "--files", "-l", pdb])
        for path in directory.glob("*.sg"):
            if sha(path).upper().encode() not in inspected.replace(b"-", b""):
                raise AssertionError("physical file SHA-256 missing from inspector")
        frame_format = 'settings set frame-format "SG_PHYSICAL ${line.file.fullpath}:${line.number}\\n"'
        commands = [frame_format, 'breakpoint set --file "' + main_source.as_posix() + '" --line 4', "run", "frame variable input", "frame info", "thread step-in", "frame info", "thread step-over", "frame variable value answer", "thread step-over", "frame variable answer", "thread step-out", "thread step-over", "frame variable answer", "continue"]
        live = run("cross-file-source-step", [tools["lldb"], "--batch", "--no-lldbinit", *[x for c in commands for x in ("-o", c)], image], "debug")
        for marker in (b"input = 40", b"value = 40", b"answer = 40", b"answer = 42", b"exited with status = 42"):
            if marker not in live:
                raise AssertionError("live debugger values missing")
        for path, line in ((main_source, 4), (directory / "worker.sg", 2), (directory / "worker.sg", 3), (directory / "worker.sg", 4), (main_source, 5)):
            if ("SG_PHYSICAL " + str(path) + ":" + str(line)).encode("utf-8") not in live:
                raise AssertionError("debugger did not report the exact physical source path")
        worker_commands = [frame_format, 'breakpoint set --file "' + (directory / "worker.sg").as_posix() + '" --line 3', "run", "frame variable value answer", "thread step-over", "frame variable answer", "continue"]
        worker_live = run("worker-physical-breakpoint", [tools["lldb"], "--batch", "--no-lldbinit", *[x for c in worker_commands for x in ("-o", c)], image], "debug")
        for marker in (("SG_PHYSICAL " + str(directory / "worker.sg") + ":3").encode("utf-8"), b"value = 40", b"answer = 40", b"answer = 42", b"exited with status = 42"):
            if marker not in worker_live:
                raise AssertionError("worker physical source breakpoint/value missing")
        negative = []
        sentinel = b"preserve previous PE and PDB"
        for label, raw, expected in mutations(original[obj.name]):
            changed = output / (label + ".obj")
            changed.write_bytes(raw)
            image.write_bytes(sentinel)
            pdb.write_bytes(sentinel)
            rejected = run("reject-" + label, [binary, main_source, changed, image, pdb, "read"], "pure", 65)
            if rejected.strip() != str(expected).encode() or image.read_bytes() != sentinel or pdb.read_bytes() != sentinel:
                raise AssertionError("multi-source reader diagnostic/output preservation differs: " + repr(rejected))
            negative.append({"name": label, "expected_code": expected, "input_sha256": sha(changed), "five_first_failure_resource_scopes": True})
        for number in range(1, 10):
            image.write_bytes(sentinel)
            pdb.write_bytes(sentinel)
            expected = 7303 if number in (2, 6) else 7302 if number == 8 else 7301
            rejected = run("reject-registry-" + str(number), [binary, main_source, obj, image, pdb, chr(64 + number)], "pure", 65)
            if rejected.strip() != str(expected).encode() or image.read_bytes() != sentinel or pdb.read_bytes() != sentinel:
                raise AssertionError("function/source registry rejection differs: " + repr(rejected))
            negative.append({"registry_case": number, "expected_code": expected, "five_first_failure_resource_scopes": True})
        for path in (obj, image, pdb):
            path.write_bytes(original[path.name])
        report.update(result="passed", negative_cases=negative, first_call_success_resource_scopes=5, physical_files=2, pdb_modules=1, artifacts={path.name: sha(path) for path in (binary, obj, image, pdb)})
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        report["changed_sources"] = [name for name, value in sources.items() if not (ROOT / name).is_file() or sha(ROOT / name) != value]
        report["changed_tools"] = [name for name, value in report["tools"].items() if not Path(value["path"]).is_file() or sha(Path(value["path"])) != value["sha256"]]
        report["checkout_changes"] = [str(path.relative_to(ROOT)) for path, value in originals.items() if not path.is_file() or sha(path) != value]
        report["changed_cargo_configs"] = [name for name, value in report["cargo_config_candidates"].items() if (sha(Path(name)) if Path(name).is_file() else None) != value]
        report.update(source_revision_end=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if report["changed_sources"] or report["changed_tools"] or report["changed_cargo_configs"]:
            report.update(result="failed", error="multi-source debug producer input changed")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if report["changed_sources"] or report["changed_tools"] or report["changed_cargo_configs"]:
            raise AssertionError("multi-source debug inputs changed")


if __name__ == "__main__":
    main()
