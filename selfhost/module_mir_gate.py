"""Verify physical module source spans after typed MIR lowering."""
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
import subprocess
import time
import tomllib

ROOT = Path(__file__).resolve().parents[1]
ENTRY = ROOT / "selfhost/tests/mir_module_sources.sg"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def closure():
    pending, seen = [ENTRY], set()
    while pending:
        path = pending.pop()
        if path in seen:
            continue
        seen.add(path)
        names = re.findall(r"(?m)^\s*import\s+selfhost_([A-Za-z0-9_]+)\s*;", path.read_text(encoding="utf-8"))
        pending.extend(ROOT / "selfhost/src" / (name + ".sg") for name in names)
    return sorted(seen)


def rust_override(name):
    upper = name.upper()
    return upper.startswith(("RUSTC", "RUSTDOC", "RUSTUP_TOOLCHAIN", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTDOC", "CARGO_PROFILE_")) or upper in {
        "CARGO", "RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS",
        "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_RUSTDOCFLAGS", "CARGO_BUILD_TARGET", "CARGO_TARGET_DIR", "CARGO_INCREMENTAL"
    } or upper.startswith("CARGO_TARGET_") and upper.endswith(("_RUSTFLAGS", "_RUSTDOCFLAGS", "_RUSTC"))


def validate_output(raw, files):
    declarations = {
        "main": (0, b"def main()->i64{worker()}"),
        "worker": (1, b"pub def worker()->i64{let a=identity(40);add_two!(a)}"),
        "leaf": (2, b"pub def leaf()->i64{2}"),
        "identity": (2, b"pub def identity<T>(value:T)->T{value}"),
    }
    physical_files = ("main.sg", "worker.sg", "leaf.sg")
    expected = {name: (source, files[physical_files[source]].index(declaration), files[physical_files[source]].index(declaration) + len(declaration))
                for name, (source, declaration) in declarations.items()}
    lines = raw.decode("utf-8").splitlines()
    if not lines or lines[-1] != "module MIR physical spans and first-call resources: ok":
        raise AssertionError("missing exact completion line")
    positions = {kind: [] for kind in ("function", "local", "instruction", "terminator", "block-span")}
    counts, seen = {}, set()
    for line in lines[:-1]:
        fields = line.split("|")
        if fields[0] == "count" and len(fields) == 3:
            if fields[1] in counts:
                raise AssertionError("duplicate counter")
            counts[fields[1]] = int(fields[2])
            continue
        if len(fields) != 5 or fields[0] not in positions:
            raise AssertionError("unexpected stdout record: " + line)
        kind, name, source, start, end = fields
        spelling = "identity" if re.fullmatch(r"identity__sg_instance_[0-9]+", name) else name
        if spelling not in expected:
            raise AssertionError("unexpected function name: " + name)
        if spelling == "identity" and not re.fullmatch(r"identity__sg_instance_[0-9]+", name):
            raise AssertionError("generic function lacks a concrete specialization name")
        actual = int(source), int(start), int(end)
        file, begin, limit = expected[spelling]
        if kind == "function":
            if spelling in seen or actual != expected[spelling]:
                raise AssertionError("function differs from independent declaration bytes: " + line)
            seen.add(spelling)
        elif actual[0] != file or not begin <= actual[1] <= actual[2] <= limit:
            raise AssertionError("MIR record escapes its owning function: " + line)
        positions[kind].append(fields)
    expected_counts = {"bad-length": 5002, "cross-file": 5002, "empty-expanded-map": 5002, "map-hole": 5002,
                       "map-max-origin": 5002, "empty-module-map": 5002, "wrong-front-source": 5002, "source-limit": 5001,
                       "repeated": 5002, "expanded-changed": 1, "cfg-removed": 1, "cfg-enabled": 1}
    if seen != set(expected) or set(counts) != set(expected_counts) | {"mode1", "local", "instruction", "terminator", "block-span"}:
        raise AssertionError("missing or extra functions/counters")
    if any(counts[name] != value for name, value in expected_counts.items()) or counts["mode1"] <= 0:
        raise AssertionError("transform/negative controls did not complete")
    for kind in ("local", "instruction", "terminator", "block-span"):
        if counts[kind] <= 0 or counts[kind] != len(positions[kind]):
            raise AssertionError("empty or inconsistent MIR category: " + kind)
    call = b"add_two!(a)"
    call_start = files["worker.sg"].index(call)
    if not any(row[1] == "worker" and tuple(map(int, row[2:])) == (1, call_start, call_start + len(call)) for row in positions["instruction"]):
        raise AssertionError("macro instruction was not mapped to its original worker invocation")
    return positions, counts, expected


def validate_oracle_controls(raw, files):
    lines = raw.decode("utf-8").splitlines()
    wrong_whole_file = list(lines)
    index = next(at for at, line in enumerate(lines) if line.startswith("function|"))
    fields = lines[index].split("|")
    fields[3], fields[4] = "0", str(len(files[("main.sg", "worker.sg", "leaf.sg")[int(fields[2])]]))
    wrong_whole_file[index] = "|".join(fields)
    wrong_name = [re.sub(r"identity__sg_instance_[0-9]+", "identity", line) for line in lines]
    wrong_macro = list(lines)
    call_start = files["worker.sg"].index(b"add_two!(a)")
    for at, line in enumerate(lines):
        fields = line.split("|")
        if len(fields) == 5 and fields[:2] == ["instruction", "worker"] and tuple(map(int, fields[2:])) == (1, call_start, call_start + 11):
            fields[3] = str(call_start + 1)
            wrong_macro[at] = "|".join(fields)
    controls = {"whole-file-function-span": wrong_whole_file, "unqualified-specialization-name": wrong_name,
                "macro-origin-shift": wrong_macro, "unexpected-stdout": lines[:-1] + ["unexpected output", lines[-1]]}
    for name, changed in controls.items():
        try:
            validate_output(("\n".join(changed) + "\n").encode("utf-8"), files)
        except (AssertionError, ValueError):
            continue
        raise AssertionError("independent oracle accepted negative control: " + name)
    return sorted(controls)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter", "reference", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("output must stay inside target/")
    output.mkdir(parents=True, exist_ok=False)
    report = {"schema": 1, "stage": "module-mir-physical-source-mapping", "result": "running",
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "started_at": dt.datetime.now(dt.timezone.utc).isoformat(),
              "host": platform.platform(), "steps": [], "tools": {}, "sources": {}, "m3_complete": False,
              "limits": ["MIR file IDs and original UTF-8 byte intervals, with the physical paths retained by the module graph",
                         "no HIR schema change or full namespace/multiple-debug-module qualification",
                         "seed construction uses snapshotted SG/C/stdlib inputs and the guarded actual Rust/Cargo development runtime",
                         "clang and frozen Rust reference are independent verification tools, not the final self-build chain"]}
    snapshot = output / "implementation"
    modules = closure()
    origins = modules + [ROOT / "selfhost/tests/native_source_probe.c"]
    origins += [path for path in (ROOT / "tools/stdlib").rglob("*") if path.is_file()
                and path.suffix in {".sg", ".c", ".h"}
                and not any(part in {"target", "build", "__pycache__"} for part in path.relative_to(ROOT / "tools/stdlib").parts)]
    origin_hashes = {}
    for path in sorted(set(origins)):
        raw = path.read_bytes()
        origin_hashes[str(path.relative_to(ROOT))] = hashlib.sha256(raw).hexdigest()
        saved = snapshot / path.relative_to(ROOT)
        saved.parent.mkdir(parents=True, exist_ok=True)
        saved.write_bytes(raw)
        report["sources"][str(saved.relative_to(ROOT))] = sha(saved)
    if any(sha(ROOT / name) != digest for name, digest in origin_hashes.items()):
        raise AssertionError("mixed producer snapshot")
    report["producer_origins"] = origin_hashes
    guarded = [Path(__file__).resolve(), ROOT / "selfhost/bootstrap.lock", ROOT / "selfhost/reference-baseline.lock"]
    guarded += [ROOT / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config", ".cargo/config.toml") if (ROOT / name).is_file()]
    guarded += [path for path in (ROOT / "runtime").rglob("*") if path.is_file()
                and not any(part in {"target", "build", "__pycache__"} for part in path.relative_to(ROOT / "runtime").parts)]
    report["sources"].update({str(path.relative_to(ROOT)): sha(path) for path in guarded})
    tools = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter", "reference")}
    env = {key: value for key, value in os.environ.items()
           if not key.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")) and not rust_override(key)}
    env.update(SENGOO_STDLIB=str(snapshot / "tools/stdlib"), SENGOO_RUNTIME=str(snapshot / "tools/stdlib/runtime.c"),
               SENGOO_CLANG=str(tools["clang"]), SENGOO_MODULE_MAP=";".join(
                   "selfhost_" + path.stem + "=" + str(snapshot / path.relative_to(ROOT)) for path in modules if path.parent == ROOT / "selfhost/src"))
    env["PATH"] = str(tools["clang"].parent) + os.pathsep + env.get("PATH", "")

    def run(name, command, expected=0):
        started = time.monotonic()
        result = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=900)
        (output / (name + ".stdout.log")).write_bytes(result.stdout)
        (output / (name + ".stderr.log")).write_bytes(result.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)), "exit_code": result.returncode,
                                "expected_exit_code": expected, "seconds": time.monotonic() - started})
        print(name, result.returncode, flush=True)
        if result.returncode != expected:
            raise AssertionError(name + " returned " + str(result.returncode))
        return result

    try:
        lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
        reference = json.loads((ROOT / "selfhost/reference-baseline.lock").read_text())
        for name, path in tools.items():
            expected = reference["tools"]["sgc"]["sha256"] if name == "reference" else lock[name]["sha256"]
            if sha(path) != expected:
                raise ValueError(name + " identity mismatch")
            report["tools"][name] = {"path": str(path), "sha256": expected}
        proxy = shutil.which("rustc", path=env["PATH"])
        if not proxy:
            raise ValueError("development rustc is missing")
        report["tools"]["rustc-proxy"] = {"path": proxy, "sha256": sha(Path(proxy))}
        sysroot = Path(run("rustc-sysroot", [proxy, "--print", "sysroot"]).stdout.decode("utf-8").strip())
        for name in ("cargo", "rustc"):
            path = sysroot / "bin" / (name + ".exe")
            report["tools"][name] = {"path": str(path), "sha256": sha(path)}
            run(name + "-version", [path, "--version"])
        env["PATH"] = str(sysroot / "bin") + os.pathsep + env["PATH"]
        env.update(RUSTC=report["tools"]["rustc"]["path"], CARGO_BUILD_RUSTC=report["tools"]["rustc"]["path"],
                   CARGO=report["tools"]["cargo"]["path"], RUSTUP_TOOLCHAIN=sysroot.name,
                   RUSTC_WRAPPER="", RUSTC_WORKSPACE_WRAPPER="", CARGO_BUILD_RUSTC_WRAPPER="", CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER="")
        report["development_compiler_environment"] = {name: env[name] for name in ("CARGO", "RUSTC", "CARGO_BUILD_RUSTC", "RUSTUP_TOOLCHAIN", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")}
        cargo_home = Path(env.get("CARGO_HOME", str(Path.home() / ".cargo")))
        configurations = {parent / ".cargo" / name for parent in (ROOT, *ROOT.parents) for name in ("config", "config.toml")}
        configurations.update(cargo_home / name for name in ("config", "config.toml"))
        report["external_sources"] = {}
        report["cargo_config_candidates"] = {}
        for path in sorted(configurations):
            if path.is_file():
                raw = path.read_bytes()
                digest = hashlib.sha256(raw).hexdigest()
                if "include" in tomllib.loads(raw.decode("utf-8")):
                    raise ValueError("included Cargo configuration needs separate qualification")
                report["external_sources"][str(path.resolve())] = digest
                report["cargo_config_candidates"][str(path.resolve())] = digest
            else:
                report["cargo_config_candidates"][str(path.resolve())] = None
        for path in (ENTRY, ROOT / "selfhost/src/mir_module_sources.sg"):
            run("format-" + path.parent.name, [tools["formatter"], "--check", snapshot / path.relative_to(ROOT)])
        binary = output / "module-mir-tests.exe"
        run("build-module-mir-tests", [tools["seed"], "--runtime-mode", "source-development", "build", snapshot / ENTRY.relative_to(ROOT),
                                       "--force-rebuild", "-O", "0", "-o", binary])
        for mode in ("--fail-equality", "--fail-truth"):
            failed = run("assertion-control" + mode, [binary, mode], 1)
            if b"mir_module_sources.sg" not in failed.stderr or b"Assertion failed" not in failed.stderr:
                raise AssertionError("intentional assertion did not fail through the runtime")
        directory = output / "sources 世界 &"
        directory.mkdir()
        files = {
            "main.sg": b"import worker;def main()->i64{worker()}",
            "worker.sg": b"\xef\xbb\xbf// " + "依赖世界".encode("utf-8") + b'\nimport leaf;#[cfg(not(feature="never"))] pub def worker()->i64{let a=identity(40);add_two!(a)}',
            "leaf.sg": b'macro_rules! add_two { ($x:expr)=>{$x+leaf()}; }\n#[cfg(feature="never")] def removed()->i64{999}\n' + b"pub def identity<T>(value:T)->T{value} pub def leaf()->i64{2}",
        }
        for name, raw in files.items():
            path = directory / name
            path.write_bytes(raw)
            report["sources"][str(path.relative_to(ROOT))] = sha(path)
        object_path = output / "mapped.obj"
        tested = run("module-mir-exact-first-call-resources", [binary, directory / "main.sg", object_path])
        positions, counts, independent_ranges = validate_output(tested.stdout, files)
        oracle_negative_controls = validate_oracle_controls(tested.stdout, files)
        image = output / "mapped.exe"
        run("mapped-link", [tools["clang"], snapshot / "selfhost/tests/native_source_probe.c", object_path, "-o", image])
        actual = run("mapped-execute", [image], 42)
        llvm, reference_exe = output / "reference.ll", output / "reference.exe"
        run("reference-ir", [tools["reference"], "--runtime-mode", "source-development", "build", directory / "main.sg", "--force-rebuild", "-O", "0", "--emit-llvm", "-o", llvm])
        run("reference-link", [tools["clang"], snapshot / "selfhost/tests/native_source_probe.c", llvm, "-o", reference_exe])
        expected = run("reference-execute", [reference_exe], 42)
        if (actual.stdout, actual.stderr) != (expected.stdout, expected.stderr):
            raise AssertionError("execution differs from frozen reference")
        report.update(result="passed", physical_functions=positions["function"], physical_positions=positions, observed_counts=counts,
                      independent_declaration_ranges=independent_ranges, first_call_resource_scopes=10,
                      oracle_negative_controls=oracle_negative_controls,
                      artifacts={path.name: sha(path) for path in (binary, object_path, image, llvm, reference_exe)})
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items() if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        changed_external = [name for name, digest in report.get("external_sources", {}).items() if not Path(name).is_file() or sha(Path(name)) != digest]
        changed_configs = [name for name, digest in report.get("cargo_config_candidates", {}).items() if (sha(Path(name)) if Path(name).is_file() else None) != digest]
        report.update(changed_sources=changed, changed_tools=changed_tools, changed_external_sources=changed_external,
                      changed_cargo_configs=changed_configs, revision_end=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(), completed_at=dt.datetime.now(dt.timezone.utc).isoformat(),
                      checkout_changes=[name for name, digest in origin_hashes.items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest])
        if changed or changed_tools or changed_external or changed_configs:
            report.update(result="failed", error="consumed inputs changed")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools or changed_external or changed_configs:
            raise AssertionError("consumed inputs changed")


if __name__ == "__main__":
    main()
