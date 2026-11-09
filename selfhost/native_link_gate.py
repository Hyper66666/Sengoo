"""Execute qualified SG source emission and SG PE linking with an empty PATH."""
from __future__ import annotations

import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

from source_codegen_gate import CASES

ROOT = Path(__file__).resolve().parents[1]
NUMERIC_SDK = ROOT / "selfhost/tests/native_numeric_sdk.sg"


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def revision():
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--native-evidence", type=Path, required=True)
    parser.add_argument("--linker-evidence", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target"):
        raise ValueError("outputs must stay inside target/")
    out.mkdir(parents=True, exist_ok=False)
    report = {"schema": 1, "result": "running", "stage": "sg-source-sg-link-execute",
              "source_revision": revision(), "started_at": dt.datetime.now(dt.timezone.utc).isoformat(),
              "host": platform.platform(), "sources": {}, "tools": {}, "qualifications": {},
              "steps": [], "cases": [], "m4_complete": False, "m5_complete": False,
              "self_build_fixed_point": False,
              "limits": ["consumes previously qualified tools built with the verified Rust seed and clang",
                         "this run invokes only SG native emission, SG linking and generated Windows programs",
                         "PATH is empty and source-runtime/toolchain overrides are removed for every command",
                         "numeric SDK is an SG verification fixture using ExitProcess; diagnostics and full runtime SDK remain open",
                         "single-source compiler preview; no compiler self-rebuild, full ABI or CodeView/PDB qualification",
                         "qualified implementation sources are copied and hashed before execution; the pinned tools do not reread mutable checkout sources",
                         "final self-build child-process tracing and supported-corpus gates remain open"]}
    initial_sources = {Path(__file__), NUMERIC_SDK, ROOT / "selfhost/source_codegen_gate.py"}

    def qualify(name, evidence, artifact, stage):
        evidence = evidence.resolve()
        if not evidence.is_relative_to(ROOT / "target"):
            raise ValueError("qualification evidence must stay inside target/")
        initial_sources.add(evidence)
        data = json.loads(evidence.read_text(encoding="utf-8"))
        if data.get("schema") != 1 or data.get("result") != "passed" or data.get("stage") != stage:
            raise ValueError(name + " does not have a successful matching qualification")
        if data.get("changed_sources") != [] or data.get("changed_tools") != []:
            raise ValueError(name + " qualification has changed or missing identity guards")
        if not data.get("sources") or not data.get("steps") or not data.get("tools"):
            raise ValueError(name + " qualification lacks required provenance")
        snapshot_root = out / "qualified-sources" / name
        source_manifest = {}
        for source, digest in data["sources"].items():
            path = (ROOT / source).resolve()
            if not path.is_relative_to(ROOT) or not path.is_file() or sha(path) != digest:
                raise ValueError(name + " source identity no longer matches: " + source)
            raw = path.read_bytes()
            if hashlib.sha256(raw).hexdigest() != digest:
                raise ValueError(name + " source changed while taking the qualified snapshot: " + source)
            snapshot = snapshot_root / path.relative_to(ROOT)
            snapshot.parent.mkdir(parents=True, exist_ok=True)
            snapshot.write_bytes(raw)
            initial_sources.add(snapshot)
            source_manifest[source] = {"sha256": digest, "snapshot": str(snapshot.relative_to(ROOT))}
        for tool, identity in data["tools"].items():
            path = Path(identity["path"])
            if not path.is_file() or sha(path) != identity["sha256"]:
                raise ValueError(name + " tool identity no longer matches: " + tool)
        binary = evidence.parent / artifact
        if not binary.is_file() or sha(binary) != data.get("artifacts", {}).get(artifact):
            raise ValueError(name + " qualified artifact differs")
        report["qualifications"][name] = {"path": str(evidence), "sha256": sha(evidence),
                                           "source_revision": data.get("source_revision", data.get("revision")),
                                           "steps": len(data["steps"]), "sources": source_manifest}
        report["tools"][name] = {"path": str(binary), "sha256": sha(binary)}
        return binary, data

    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
            env.pop(name)
    env["PATH"] = ""

    def run(name, command, expected=0):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=out, env=env, capture_output=True, timeout=120)
        (out / (name + ".stdout.log")).write_bytes(done.stdout)
        (out / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)),
                                "exit_code": done.returncode, "expected_exit_code": expected,
                                "seconds": time.monotonic() - started, "path": ""})
        if done.returncode != expected:
            raise AssertionError(f"{name}: expected {expected}, observed {done.returncode}")
        print(f"{name}: {done.returncode}", flush=True)
        return done

    try:
        native, _ = qualify("native", args.native_evidence, "native.exe", "sg-source-to-native-object")
        linker, linker_report = qualify("linker", args.linker_evidence, "sg-linker.exe", "sg-coff-pe-linker-foundation")
        sdk_identity = linker_report["tools"]["sdk-kernel32"]
        sdk = Path(sdk_identity["path"])
        report["tools"]["sdk-kernel32"] = sdk_identity
        report["sources"] = {str(p.relative_to(ROOT)): sha(p) for p in sorted(initial_sources)}
        cases = {name: text for name, text in CASES.items()
                 if name not in ("c-call", "drop-scope", "drop-return", "string-utf8", "string-empty", "string-nul")}
        cases["windows-sdk"] = ('extern "C"{fn GetCurrentProcessId()->u32;fn GetCurrentThreadId()->u32;}'
                                'def main()->i64{if GetCurrentProcessId()>0u32 and GetCurrentThreadId()>0u32{42}else{1}}')
        cases["windows-string"] = ('extern "C"{fn lstrlenA(value:&str)->i32;}'
                                   'def main()->i64{if lstrlenA("世界")==6i32 and lstrlenA("")==0i32 and lstrlenA("A\\0B")==1i32{42}else{1}}')
        first_artifacts = {}
        for round_name in ("first 世界 &", "second 世界 &"):
            directory = out / round_name
            directory.mkdir()
            runtime_source = directory / "numeric sdk.sg"
            runtime_source.write_bytes(NUMERIC_SDK.read_bytes())
            runtime = directory / "numeric sdk.obj"
            run(round_name + "-runtime", [native, runtime_source, runtime])
            if round_name.startswith("first"):
                first_artifacts["runtime"] = sha(runtime)
            elif sha(runtime) != first_artifacts["runtime"]:
                raise AssertionError("numeric SDK objects differ across clean directories")
            for name, text in cases.items():
                source, obj, exe = (directory / (name + ext) for ext in (".sg", ".obj", ".exe"))
                source.write_bytes(text.encode("utf-8"))
                emitted = run(round_name + "-" + name + "-emit", [native, source, obj])
                if json.loads(emitted.stdout) != {"schema_version": 1, "ok": True, "phase": "object"}:
                    raise AssertionError("source compiler success protocol differs")
                run(round_name + "-" + name + "-link", [linker, "link", exe, "main", obj, runtime, sdk])
                executed = run(round_name + "-" + name + "-execute", [exe], 42)
                row = {"round": round_name, "name": name, "source_sha256": sha(source),
                       "object_sha256": sha(obj), "image_sha256": sha(exe),
                       "stdout_sha256": hashlib.sha256(executed.stdout).hexdigest(),
                       "stderr_sha256": hashlib.sha256(executed.stderr).hexdigest()}
                identity = {key: value for key, value in row.items() if key.endswith("sha256")}
                if round_name.startswith("first"):
                    first_artifacts[name] = identity
                elif identity != first_artifacts[name]:
                    raise AssertionError(name + " differs across clean output directories")
                report["cases"].append(row)
        directory = out / "first 世界 &"
        for name, text, expected in (
            ("overflow", "def main()->i64{let x=9223372036854775807;let y=1;x+y}", 63),
            ("division-zero", "def divide(x:i64,y:i64)->i64{x/y} def main()->i64{divide(42,0)}", 62),
        ):
            source, obj, exe = (directory / (name + ext) for ext in (".sg", ".obj", ".exe"))
            source.write_bytes(text.encode("utf-8"))
            run(name + "-emit", [native, source, obj])
            run(name + "-link", [linker, "link", exe, "main", obj, directory / "numeric sdk.obj", sdk])
            run(name + "-execute", [exe], expected)
        obj = directory / "constant.obj"
        missing_output = directory / "preserved.exe"
        sentinel = b"previous executable survives unresolved-symbol failure"
        missing_output.write_bytes(sentinel)
        run("missing-runtime", [linker, "link", missing_output, "main", directory / "call.obj", sdk], 65)
        if missing_output.read_bytes() != sentinel:
            raise AssertionError("link failure replaced the previous output")
        report["result"] = "passed"
        report["positive_programs"] = len(cases)
        report["clean_directories"] = 2
        report["deterministic_objects_and_images"] = True
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items()
                   if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items()
                         if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        report.update(changed_sources=changed, changed_tools=changed_tools, source_revision_end=revision(),
                      completed_at=dt.datetime.now(dt.timezone.utc).isoformat())
        if changed or changed_tools:
            report.update(result="failed", error="qualified source or tool changed during verification")
        (out / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("qualified source or tool changed")


if __name__ == "__main__":
    main()
