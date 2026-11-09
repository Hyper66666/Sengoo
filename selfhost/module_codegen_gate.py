"""Qualify located multi-file compilation through an already source-qualified SG emitter."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import time
import tomllib

from linker_evidence import copy_linker

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def copy_verified_binary(source, destination, expected):
    raw = source.read_bytes()
    observed = hashlib.sha256(raw).hexdigest()
    if observed != expected:
        raise ValueError("native binary does not match producer")
    destination.write_bytes(raw)
    if sha(destination) != expected:
        raise ValueError("native binary copy changed")
    return {"source_path": str(source), "source_read_count": 1, "expected_sha256": expected,
            "read_sha256": observed, "copy_sha256": sha(destination)}


def parse_cli(raw, success):
    def pairs(items):
        result = {}
        for key, value in items:
            if key in result:
                raise ValueError("duplicate native CLI JSON field: " + key)
            result[key] = value
        return result
    def constant(value):
        raise ValueError("invalid native CLI JSON constant: " + value)
    value = json.loads(raw.decode("utf-8"), object_pairs_hook=pairs, parse_constant=constant)
    expected = {"schema_version", "ok", "phase"}
    if not success:
        expected.update({"code", "source", "path", "start", "end"})
    if not isinstance(value, dict) or set(value) != expected or type(value.get("schema_version")) is not int or value["schema_version"] != 1 or value.get("ok") is not success:
        raise ValueError("native CLI schema/status differs")
    if type(value["phase"]) is not str or success and value["phase"] != "object":
        raise ValueError("native CLI phase differs")
    if not success and (any(type(value[name]) is not int for name in ("code", "source", "start", "end")) or type(value["path"]) is not str or not value["path"]):
        raise ValueError("native CLI diagnostic field types differ")
    return value


def rust_override(name):
    upper = name.upper()
    return upper.startswith(("RUSTC", "RUSTDOC", "RUSTUP_TOOLCHAIN", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTDOC", "CARGO_PROFILE_")) or upper in {
        "CARGO", "RUSTFLAGS", "RUSTDOCFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_ENCODED_RUSTDOCFLAGS",
        "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_RUSTDOCFLAGS", "CARGO_BUILD_TARGET", "CARGO_TARGET_DIR", "CARGO_INCREMENTAL"
    } or upper.startswith("CARGO_TARGET_") and upper.endswith(("_RUSTFLAGS", "_RUSTDOCFLAGS", "_RUSTC"))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-evidence", required=True, type=Path)
    parser.add_argument("--linker-evidence", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("outputs must remain inside target/")
    output.mkdir(parents=True, exist_ok=False)
    evidence = args.native_evidence.resolve()
    report = {"schema": 1, "stage": "sg-native-module-graph", "result": "running",
              "source_revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "host": platform.platform(), "sources": {}, "tools": {}, "steps": [], "cases": [],
              "m3_complete": False, "m4_complete": False,
              "limits": ["transparent import environment with physical file provenance; complete namespace/stdlib/self-build gates remain open",
                         "qualified tool is copied by identity; exact immutable implementation snapshots retain its producer provenance",
                         "external clang links independent C and frozen-reference LLVM artifacts; this gate does not qualify SG linker/runtime SDK"]}
    inputs = {Path(__file__), ROOT / "selfhost/linker_evidence.py", ROOT / "selfhost/tests/linker_evidence.py", ROOT / "selfhost/tests/native_source_probe.c", ROOT / "selfhost/tests/native_driver.sg", evidence}
    if args.linker_evidence:
        inputs.add(args.linker_evidence.resolve())
    report["sources"] = {str(path.relative_to(ROOT)): sha(path) for path in inputs if path.is_file()}

    def run(name, command, env, expected=0):
        started = time.monotonic()
        if env.get("SENGOO_MODULE_MAP") == "":
            env = {key: value for key, value in env.items() if key != "SENGOO_MODULE_MAP"}
        result = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=600)
        (output / (name + ".stdout.log")).write_bytes(result.stdout)
        (output / (name + ".stderr.log")).write_bytes(result.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)), "exit_code": result.returncode,
                                "expected_exit_code": expected, "seconds": time.monotonic() - started,
                                "module_map": env.get("SENGOO_MODULE_MAP", ""), "path": env.get("PATH", "")})
        print(name, result.returncode, flush=True)
        if result.returncode != expected:
            raise AssertionError(name + " returned " + str(result.returncode))
        return result

    def write(directory, files):
        for name, raw in files.items():
            path = directory / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(raw if isinstance(raw, bytes) else raw.encode("utf-8"))
            inputs.add(path)
            report["sources"][str(path.relative_to(ROOT))] = sha(path)

    try:
        rejected_schema = [b'{}', b'{"schema_version":1,"ok":false,"phase":"object"}',
                           b'{"schema_version":true,"ok":true,"phase":"object"}', b'{"schema_version":1,"ok":1,"phase":"object"}',
                           b'{"schema_version":1,"ok":true,"phase":"native"}', b'{"schema_version":1,"ok":true,"ok":true,"phase":"object"}',
                           b'{"schema_version":1,"ok":true,"phase":"object","extra":1}', b'[]', b'not-json']
        for raw in rejected_schema:
            try:
                parse_cli(raw, True)
            except (ValueError, UnicodeError):
                pass
            else:
                raise AssertionError("malformed/false success schema was accepted")
        false_diagnostic = b'{"schema_version":1,"ok":true,"phase":"semantics","code":5104,"source":1,"path":"worker.sg","start":0,"end":1}'
        try:
            parse_cli(false_diagnostic, False)
        except ValueError:
            pass
        else:
            raise AssertionError("a negative diagnostic was allowed to report ok=true")
        bad_binary, guarded_copy = output / "negative-binary-copy.input", output / "negative-binary-copy.output"
        bad_binary.write_bytes(b"producer artifact changed")
        guarded_copy.write_bytes(b"preserve previous copied executable")
        try:
            copy_verified_binary(bad_binary, guarded_copy, hashlib.sha256(b"qualified producer bytes").hexdigest())
        except ValueError:
            pass
        else:
            raise AssertionError("mismatched producer artifact was copied")
        if guarded_copy.read_bytes() != b"preserve previous copied executable":
            raise AssertionError("failed binary identity check replaced output")
        report["guard_negative_controls"] = {"rejected_success_schemas": len(rejected_schema), "rejected_negative_ok_true": True, "mismatched_binary_preserved_previous_copy": True}
        producer = json.loads(evidence.read_text(encoding="utf-8"))
        if producer.get("result") != "passed" or producer.get("stage") != "sg-source-to-native-object" or producer.get("changed_sources") != [] or producer.get("changed_tools") != []:
            raise ValueError("native producer is not source-qualified")
        for name, digest in producer["sources"].items():
            path = (ROOT / name).resolve()
            if not path.is_relative_to(ROOT) or sha(path) != digest:
                raise ValueError("producer implementation changed: " + name)
        snapshot = (ROOT / producer["producer_snapshot_root"]).resolve()
        if not snapshot.is_relative_to(ROOT / "target"):
            raise ValueError("producer snapshot is outside target/")
        for name, digest in producer["producer_source_origins"].items():
            path = (snapshot / name).resolve()
            if not path.is_relative_to(snapshot) or producer["producer_snapshots"].get(str(path.relative_to(ROOT))) != digest or sha(path) != digest:
                raise ValueError("producer snapshot identity differs: " + name)
            saved = output / "implementation" / name
            saved.parent.mkdir(parents=True, exist_ok=True)
            saved.write_bytes(path.read_bytes())
            if sha(saved) != digest:
                raise ValueError("implementation changed during snapshot")
            inputs.add(saved)
            report["sources"][str(saved.relative_to(ROOT))] = digest
        resource_entry = output / "implementation/selfhost/tests/native_driver.sg"
        resource_entry.parent.mkdir(parents=True, exist_ok=True)
        resource_entry.write_bytes((ROOT / "selfhost/tests/native_driver.sg").read_bytes())
        report["sources"][str(resource_entry.relative_to(ROOT))] = sha(resource_entry)
        for name, identity in producer["tools"].items():
            if sha(Path(identity["path"])) != identity["sha256"]:
                raise ValueError("producer tool changed: " + name)
            report["tools"][name] = identity
        dependencies = {ROOT / name for name in ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", ".cargo/config.toml", ".cargo/config") if (ROOT / name).is_file()}
        cargo_home = Path(os.environ.get("CARGO_HOME", str(Path.home() / ".cargo"))).resolve()
        candidates = {parent / ".cargo" / name for parent in (ROOT, *ROOT.parents) for name in ("config", "config.toml")}
        candidates.update(cargo_home / name for name in ("config", "config.toml"))
        report["cargo_config_candidates"] = {}
        for path in sorted(candidates):
            if not path.is_file():
                report["cargo_config_candidates"][str(path)] = None
                continue
            raw = path.read_bytes()
            digest = hashlib.sha256(raw).hexdigest()
            report["cargo_config_candidates"][str(path)] = digest
            report.setdefault("external_sources", {})[str(path)] = digest
            if "include" in tomllib.loads(raw.decode("utf-8")):
                raise ValueError("Cargo config include requires an expanded input manifest: " + str(path))
        report["cargo_config_scope"] = "actual cwd and every ancestor plus CARGO_HOME; include directives rejected explicitly"
        dependencies.update(path for path in (ROOT / "runtime").rglob("*") if path.is_file()
                            and not any(part in {"target", "build", "__pycache__"} for part in path.relative_to(ROOT / "runtime").parts))
        for path in sorted(dependencies):
            raw = path.read_bytes()
            digest = hashlib.sha256(raw).hexdigest()
            saved = output / "implementation" / path.relative_to(ROOT)
            saved.parent.mkdir(parents=True, exist_ok=True)
            saved.write_bytes(raw)
            report["sources"][str(saved.relative_to(ROOT))] = digest
            report["sources"][str(path.relative_to(ROOT))] = digest
        report["resource_provider_scope"] = "fresh resource-test build uses snapshotted C/stdlib and guarded actual Rust/Cargo development runtime inputs"
        source_binary = evidence.parent / "native.exe"
        native = output / "native.exe"
        report["native_binary_copy"] = copy_verified_binary(source_binary, native, producer["artifacts"]["native.exe"])
        report["tools"]["native"] = {"path": str(native), "sha256": producer["artifacts"]["native.exe"]}
        report["producer"] = {"path": str(evidence), "sha256": sha(evidence), "source_revision": producer["revision"]}
        linker = None
        if args.linker_evidence:
            linker, linker_producer = copy_linker(ROOT, args.linker_evidence.resolve(), output, report)
            report["limits"].append("optional SG linking uses a pinned historical fully source-bound linker; current linker rebuild, full runtime SDK and self-build qualification remain open")
        report["removed_rust_environment_overrides"] = sorted(name for name in os.environ if rust_override(name))
        env = {key: value for key, value in os.environ.items() if not key.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")) and not rust_override(key)}
        clang = Path(report["tools"]["clang"]["path"])
        reference = Path(report["tools"]["reference"]["path"])
        env.update(SENGOO_STDLIB=str(output / "implementation/tools/stdlib"), SENGOO_RUNTIME=str(output / "implementation/tools/stdlib/runtime.c"), SENGOO_CLANG=str(clang))
        env["PATH"] = str(clang.parent) + os.pathsep + env.get("PATH", "")
        for name in ("cargo", "rustc", "rustup"):
            path = shutil.which(name, path=env["PATH"])
            if not path:
                raise ValueError("missing development runtime tool: " + name)
            path = Path(path).resolve()
            report["tools"][name] = {"path": str(path), "sha256": sha(path)}
            report["tools"][name]["version"] = run(name + "-version", [path, "--version"], env).stdout.decode("utf-8").strip()
        selected = {}
        for name in ("cargo", "rustc"):
            raw = run("resolve-" + name, [report["tools"]["rustup"]["path"], "which", name], env).stdout.decode("utf-8").strip()
            path = Path(raw).resolve(strict=True)
            selected[name] = path
            report["tools"][name + "-selected"] = {"path": str(path), "sha256": sha(path)}
        if selected["cargo"].parent != selected["rustc"].parent:
            raise ValueError("selected Cargo and rustc use different toolchains")
        toolchain_root = selected["rustc"].parent.parent
        env.update(CARGO=str(selected["cargo"]), RUSTC=str(selected["rustc"]), RUSTC_WRAPPER="", RUSTC_WORKSPACE_WRAPPER="",
                   CARGO_BUILD_RUSTC=str(selected["rustc"]), CARGO_BUILD_RUSTC_WRAPPER="", CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER="",
                   RUSTUP_TOOLCHAIN=toolchain_root.name)
        env["PATH"] = str(selected["cargo"].parent) + os.pathsep + env["PATH"]
        if Path(shutil.which("cargo", path=env["PATH"])).resolve() != selected["cargo"] or Path(shutil.which("rustc", path=env["PATH"])).resolve() != selected["rustc"]:
            raise ValueError("actual PATH does not select pinned Rust tools")
        for name in ("cargo", "rustc"):
            report["tools"][name + "-selected"]["version"] = run(name + "-selected-verbose-version", [selected[name], "-vV"], env).stdout.decode("utf-8").strip()
        sysroot = run("rustc-selected-sysroot", [selected["rustc"], "--print", "sysroot"], env).stdout.decode("utf-8").strip()
        if Path(sysroot).resolve() != toolchain_root:
            raise ValueError("selected rustc sysroot differs from pinned toolchain")
        report["pinned_rust_environment"] = {name: env[name] for name in ("CARGO", "RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC", "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "RUSTUP_TOOLCHAIN")}
        report["pinned_rust_environment"]["path_prefix"] = str(selected["cargo"].parent)
        if linker is not None:
            report["tools"]["runner-python"] = {"path": sys.executable, "sha256": sha(Path(sys.executable)), "version": sys.version}
            run("linker-qualification-negative-controls", [sys.executable, ROOT / "selfhost/tests/linker_evidence.py", "-v"], env)
        rust_probe = output / "rust-tool-probe"
        write(rust_probe, {"Cargo.toml": '[package]\nname="module-gate-rust-tool"\nversion="0.0.0"\nedition="2021"\n[workspace]\n',
                           "build.rs": 'fn main(){println!("cargo:warning=module_gate_rustc={}",std::env::var("RUSTC").unwrap());}\n',
                           "src/lib.rs": 'pub fn module_gate_tool_probe()->u64{42}\n'})
        probe = run("verify-cargo-actual-rustc", [selected["cargo"], "build", "--offline", "--verbose", "--manifest-path", rust_probe / "Cargo.toml", "--target-dir", rust_probe / "target"], env)
        observed = (probe.stdout + probe.stderr).decode("utf-8")
        marker = "module_gate_rustc=" + str(selected["rustc"])
        compiler_runs = [line for line in observed.splitlines() if line.lstrip().startswith("Running ") and str(selected["rustc"]).casefold() in line.casefold()]
        if marker not in observed or len(compiler_runs) < 2:
            raise ValueError("Cargo did not execute the pinned rustc for build-script and library")
        report["cargo_compiler_probe"] = {"rustc": str(selected["rustc"]), "compiler_invocations": compiler_runs, "build_script_rustc_matches": True}
        scenarios = {
            "diamond": ({"main.sg": "import left;import right;def main()->i64{left()+right()}",
                         "left.sg": "import leaf;def left()->i64{leaf()+1}", "right.sg": "import leaf;def right()->i64{leaf()+1}",
                         "leaf.sg": "pub def leaf()->i64{20}"}, {}),
            "mapped-unicode": ({"main.sg": "import worker;def main()->i64{answer()}",
                                "中文 &/worker.sg": "pub def answer()->i64{42}"}, {"worker": "中文 &/worker.sg"}),
            "identity-aliases": ({"main.sg": "import first;import second;def main()->i64{answer()}",
                                  "shared.sg": "pub def answer()->i64{42}"}, {"first": "shared.sg", "second": "shared.sg"}),
            "relative-priority": ({"main.sg": "import worker;def main()->i64{answer()}",
                                   "worker.sg": "pub def answer()->i64{42}", "ignored.sg": "def answer()->i64{1}"}, {"worker": "ignored.sg"}),
            "physical-bom": ({"main.sg": "import worker;def main()->i64{answer()}",
                              "worker.sg": b"\xef\xbb\xbfpub def answer()->i64{42}"}, {}),
        }
        for name, (files, mapping) in scenarios.items():
            directory = output / (name + " 世界 &")
            write(directory, files)
            case_env = dict(env, SENGOO_MODULE_MAP=";".join(key + "=" + str(directory / value) for key, value in mapping.items()))
            source = directory / "main.sg"
            obj, repeated, llvm = directory / "native.obj", directory / "repeat.obj", directory / "reference.ll"
            exe, ref_exe = directory / "native.exe", directory / "reference.exe"
            emitted = run(name + "-emit", [native, source, obj], case_env)
            repeated_result = run(name + "-repeat", [native, source, repeated], case_env)
            emitted_json, repeated_json = parse_cli(emitted.stdout, True), parse_cli(repeated_result.stdout, True)
            if obj.read_bytes() != repeated.read_bytes():
                raise AssertionError("module object is nondeterministic")
            run(name + "-link", [clang, output / "implementation/selfhost/tests/native_source_probe.c", obj, "-o", exe], case_env)
            actual = run(name + "-execute", [exe], case_env, 42)
            run(name + "-reference-ir", [reference, "--runtime-mode", "source-development", "build", source, "--force-rebuild", "-O", "0", "--emit-llvm", "-o", llvm], case_env)
            run(name + "-reference-link", [clang, output / "implementation/selfhost/tests/native_source_probe.c", llvm, "-o", ref_exe], case_env)
            expected = run(name + "-reference-execute", [ref_exe], case_env, 42)
            if (actual.stdout, actual.stderr) != (expected.stdout, expected.stderr):
                raise AssertionError("module execution differs")
            report["cases"].append({"name": name, "result_equal": True, "object_sha256": sha(obj), "reference_ir_sha256": sha(llvm), "emit_status": emitted_json, "repeat_status": repeated_json})
        if linker is not None:
            sdk = linker_producer.get("sdk")
            if not sdk:
                raise ValueError("pinned linker qualification lacks its SDK import library")
            runtime_source = ROOT / "selfhost/tests/native_numeric_sdk.sg"
            runtime_raw = runtime_source.read_bytes()
            runtime_digest = hashlib.sha256(runtime_raw).hexdigest()
            report["sources"][str(runtime_source.relative_to(ROOT))] = runtime_digest
            pure_env = {key: value for key, value in env.items() if not key.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_"))}
            pure_env.update(PATH="", SENGOO_STDLIB=str(output / "implementation/tools/stdlib"))
            first, linked_rows = {}, []
            for round_name in ("first clean 世界 &", "second clean 世界 &"):
                directory = output / "sg-linked-modules" / round_name
                directory.mkdir(parents=True)
                runtime_input, runtime_object = directory / "numeric sdk.sg", directory / "numeric sdk.obj"
                runtime_input.write_bytes(runtime_raw)
                report["sources"][str(runtime_input.relative_to(ROOT))] = runtime_digest
                parse_cli(run(round_name + "-numeric-sdk-emit", [native, runtime_input, runtime_object], pure_env).stdout, True)
                runtime_object_digest = sha(runtime_object)
                if first.get("runtime", runtime_object_digest) != runtime_object_digest:
                    raise AssertionError("numeric runtime object differs across clean directories")
                first["runtime"] = runtime_object_digest
                for name, (files, mapping) in scenarios.items():
                    case_directory = directory / name
                    write(case_directory, files)
                    case_env = dict(pure_env, SENGOO_MODULE_MAP=";".join(key + "=" + str(case_directory / value) for key, value in mapping.items()))
                    source, obj, exe = case_directory / "main.sg", case_directory / "native.obj", case_directory / "native.exe"
                    parse_cli(run(round_name + "-" + name + "-sg-emit", [native, source, obj], case_env).stdout, True)
                    linked = run(round_name + "-" + name + "-sg-link", [linker, "link", exe, "main", obj, runtime_object, sdk["path"]], pure_env)
                    if linked.stdout or linked.stderr:
                        raise AssertionError("SG linker success output differs")
                    executed = run(round_name + "-" + name + "-sg-execute", [exe], pure_env, 42)
                    identity = {"object_sha256": sha(obj), "image_sha256": sha(exe), "runtime_object_sha256": runtime_object_digest,
                                "stdout_sha256": hashlib.sha256(executed.stdout).hexdigest(), "stderr_sha256": hashlib.sha256(executed.stderr).hexdigest()}
                    if name in first and first[name] != identity:
                        raise AssertionError("SG-linked module object/PE differs across fresh directories: " + name)
                    first[name] = identity
                    linked_rows.append({"name": name, "round": round_name, "empty_path": True, **identity})
            report["sg_linked_cases"] = linked_rows
            report["sg_linked_clean_directories"] = 2
            report["sg_linked_deterministic_objects_and_images"] = True
        failures = {
            "missing": ({"main.sg": "import missing;def main()->i64{42}"}, "main.sg", "modules", 5201),
            "cycle": ({"main.sg": "import other;def main()->i64{42}", "other.sg": "import main;def answer()->i64{1}"}, None, "modules", 5202),
            "dependency-type": ({"main.sg": "import worker;def main()->i64{answer()}", "worker.sg": "pub def answer()->i64{true}"}, "worker.sg", "semantics", 5104),
            "dependency-utf8": ({"main.sg": "import worker;def main()->i64{42}", "worker.sg": b"\xff"}, "main.sg", "modules", 5207),
        }
        for name, (files, expected_file, phase, code) in failures.items():
            directory = output / name
            write(directory, files)
            obj = directory / "preserved.obj"
            sentinel = b"preserve previous output on module failure"
            obj.write_bytes(sentinel)
            done = run(name, [native, directory / "main.sg", obj], dict(env, SENGOO_MODULE_MAP=""), 65)
            diagnostic = parse_cli(done.stdout, False)
            path = Path(diagnostic["path"])
            if diagnostic.get("phase") != phase or diagnostic.get("code") != code or obj.read_bytes() != sentinel:
                raise AssertionError("module diagnostic or previous output differs")
            if expected_file and path.resolve() != (directory / expected_file).resolve():
                raise AssertionError("module diagnostic reports the wrong physical file")
            raw = path.read_bytes()
            if not 0 <= diagnostic["start"] <= diagnostic["end"] <= len(raw):
                raise AssertionError("module diagnostic has an invalid original byte interval")
            report["cases"].append({"name": name, "diagnostic": diagnostic, "previous_output_preserved": True})
        build_env = dict(env, SENGOO_MODULE_MAP=";".join("selfhost_" + path.stem + "=" + str(path)
                         for path in sorted((output / "implementation/selfhost/src").glob("*.sg"))))
        tests = output / "module-resource-tests.exe"
        run("build-driver-resource-tests", [report["tools"]["seed"]["path"], "--runtime-mode", "source-development", "build",
                    resource_entry, "--force-rebuild", "-O", "0", "-o", tests], build_env)
        observed = output / "resource-output.obj"
        resources = run("driver-exact-resources", [tests, output / "diamond 世界 &/main.sg", output / "dependency-type/main.sg", observed], env)
        if b"selfhost native module driver exact resources: ok" not in resources.stdout:
            raise AssertionError("driver resource lifecycles did not complete")
        rows = resources.stdout.splitlines()
        if len(rows) != 21 or rows[-1] != b"selfhost native module driver exact resources: ok":
            raise AssertionError("resource test did not report exactly ten success/failure pairs")
        resource_status = [parse_cli(row, index % 2 == 0) for index, row in enumerate(rows[:-1])]
        if any(value["phase"] != "semantics" or value["code"] != 5104 for value in resource_status[1::2]):
            raise AssertionError("in-process failures did not retain their diagnostic schema")
        if observed.read_bytes() != (output / "diamond 世界 &/native.obj").read_bytes():
            raise AssertionError("a failed in-process compile replaced the previous valid object")
        report["resource_scopes"] = 10
        report["resource_baseline"] = {"warmup": False, "first_success_checked": True, "first_failure_checked": True, "checks_after_each_compile": True, "statuses": resource_status}
        report["artifacts"] = {tests.name: sha(tests), observed.name: sha(observed)}
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items() if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items() if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        changed_external = [name for name, digest in report.get("external_sources", {}).items() if not Path(name).is_file() or sha(Path(name)) != digest]
        changed_configs = [name for name, digest in report.get("cargo_config_candidates", {}).items() if (sha(Path(name)) if Path(name).is_file() else None) != digest]
        report.update(changed_sources=changed, changed_tools=changed_tools)
        report["changed_external_sources"] = changed_external
        report["changed_cargo_configs"] = changed_configs
        report["source_revision_end"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        if changed or changed_tools or changed_external or changed_configs:
            report.update(result="failed", error="module inputs changed")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools or changed_external or changed_configs:
            raise AssertionError("module inputs changed")


if __name__ == "__main__":
    main()
