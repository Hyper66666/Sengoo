"""Verify managed FFI diagnostic copying against the real C provider and SG wrapper."""
from __future__ import annotations
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[3]
PROBE = ROOT / "tools/stdlib/tests/ffi_last_error_copy_probe.c"
PROGRAM = ROOT / "tools/stdlib/tests/ffi_last_error_copy.sg"

def sha(path: Path | str) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def revision() -> str:
    return subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()

def inputs() -> dict[str, str]:
    paths = {Path(__file__).resolve(), ROOT/"Cargo.toml", ROOT/"Cargo.lock",
             ROOT/"rust-toolchain.toml", ROOT/"selfhost/bootstrap.lock",
             ROOT/"selfhost/src/borrow.sg", ROOT/"selfhost/tests/borrow.sg"}
    for directory in ("tools/stdlib", "runtime"):
        paths.update(path for path in (ROOT/directory).rglob("*")
                     if path.is_file() and not any(part in {"build", "target", "__pycache__"}
                     for part in path.relative_to(ROOT/directory).parts))
    return {str(path): sha(path) for path in sorted(paths)}

def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=ROOT/"target/selfhost/ffi-last-error-managed-copy/checkpoint")
    parser.add_argument("--seed", type=Path, default=ROOT/"target/agent-conditional-compiler/bin/sgc.exe")
    parser.add_argument("--clang", type=Path, default=Path("C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe"))
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT/"target"):
        raise ValueError("outputs must remain within target/")
    out.mkdir(parents=True, exist_ok=True)
    report = {"schema": 1, "stage": "managed-ffi-diagnostic-copy", "result": "running",
              "started_at": dt.datetime.now(dt.timezone.utc).isoformat(),
              "m3_complete": False, "host": {"platform": platform.platform(), "python": platform.python_version(),
              "python_executable_sha256": sha(sys.executable)}, "steps": [],
              "scope": "four real C scenarios x100; actual SG public wrapper x100 with explicit Buffer release; old wrapper controls fail"}
    before, tools, snapshots = {}, {}, {}
    def save() -> None:
        (out/"evidence.json").write_text(json.dumps(report, indent=2)+"\n", encoding="utf-8")
    save()
    try:
        report["source_revision"] = revision()
        lock = json.loads((ROOT/"selfhost/bootstrap.lock").read_text(encoding="utf-8"))
        for name, path, expected in (("seed", args.seed.resolve(), lock["seed"]["sha256"]),
                                     ("clang", args.clang.resolve(), lock["clang"]["sha256"])):
            if sha(path) != expected:
                raise RuntimeError(name+" identity mismatch")
            tools[name] = (path, expected)
        before = inputs()
        report["inputs_before"] = before
        report["tools"] = {name: {"path": str(path), "sha256": digest,
                           "version": subprocess.check_output([str(path), "--version"], text=True,
                           stderr=subprocess.STDOUT, timeout=30).strip()}
                           for name, (path, digest) in tools.items()}
        env = os.environ.copy()
        for name in list(env):
            if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
                env.pop(name)
        env.update(SENGOO_RUNTIME=str(ROOT/"tools/stdlib/runtime.c"), SENGOO_CLANG=str(args.clang.resolve()))
        env["PATH"] = str(args.clang.resolve().parent)+os.pathsep+env.get("PATH", "")
        def run(name: str, command: list, expected: int = 0) -> subprocess.CompletedProcess:
            started = time.perf_counter()
            result = subprocess.run(list(map(str, command)), cwd=ROOT, env=env,
                                    capture_output=True, timeout=900)
            stdout, stderr = out/(name+".stdout.log"), out/(name+".stderr.log")
            stdout.write_bytes(result.stdout)
            stderr.write_bytes(result.stderr)
            report["steps"].append({"name": name, "command": list(map(str, command)),
                "exit_code": result.returncode, "expected_exit_code": expected,
                "elapsed_s": time.perf_counter()-started, "stdout_sha256": sha(stdout), "stderr_sha256": sha(stderr)})
            save()
            print(name, result.returncode, flush=True)
            if result.returncode != expected:
                raise RuntimeError(name+" failed")
            return result
        for stem in ("runtime", "runtime_string"):
            obj = out/(stem+".obj")
            obj.unlink(missing_ok=True)
            run(stem+"-compile", [args.clang.resolve(), "-O0", "-I", ROOT/"tools/stdlib",
                "-c", ROOT/"tools/stdlib"/(stem+".c"), "-o", obj])
        for name, flags in (("old", ["-DSENGOO_OLD_WRAPPER_CONTROL"]), ("next", [])):
            exe = out/(name+".exe")
            exe.unlink(missing_ok=True)
            run(name+"-link", [args.clang.resolve(), "-O0", "-I", ROOT/"tools/stdlib",
                *flags, PROBE, out/"runtime.obj", out/"runtime_string.obj", "-o", exe])
        control = run("old-wrapper-control", [out/"old.exe", "0"], 73)
        expected = b"managed-copy-lost-diagnostic before=-2002/23 after=0/0 copied=0\n"
        if control.stdout or control.stderr.replace(b"\r\n", b"\n") != expected:
            raise RuntimeError("old managed wrapper did not reproduce the exact diagnostic loss")
        for case in range(4):
            result = run("runtime-case-"+str(case), [out/"next.exe", str(case)])
            if result.stdout or result.stderr:
                raise RuntimeError("runtime resource scenario emitted output")
        for name in ("old", "next"):
            std = out/("stdlib-"+name)
            std.mkdir(exist_ok=True)
            for src in (ROOT/"tools/stdlib").rglob("*.sg"):
                if any(part in {"target", "build"} for part in src.relative_to(ROOT/"tools/stdlib").parts):
                    continue
                dest = std/src.relative_to(ROOT/"tools/stdlib")
                dest.parent.mkdir(parents=True, exist_ok=True)
                dest.write_bytes(src.read_bytes())
                snapshots[str(dest)] = sha(dest)
            if name == "old":
                path = std/"ffi.sg"
                text = path.read_text(encoding="utf-8")
                current = "let copied = sengoo_ffi_last_error_copy_to_buffer(buffer.handle);"
                original = "let copied = ffi_last_error_copy_raw(sengoo_ffi_buffer_ptr(buffer.handle), sengoo_ffi_buffer_len(buffer.handle));"
                if text.count(current) != 1:
                    raise RuntimeError("managed wrapper no longer matches the explicit old-control edit")
                path.write_text(text.replace(current, original), encoding="utf-8")
                snapshots[str(path)] = sha(path)
            env["SENGOO_STDLIB"] = str(std)
            ir, exe = out/("public-"+name+".ll"), out/("public-"+name+".exe")
            ir.unlink(missing_ok=True)
            exe.unlink(missing_ok=True)
            run("public-"+name+"-ir", [args.seed.resolve(), "--runtime-mode", "source-development",
                "build", PROGRAM, "--force-rebuild", "-O", "0", "--emit-llvm", "-o", ir])
            run("public-"+name+"-link", [args.clang.resolve(), "-O0", "-I", ROOT/"tools/stdlib",
                "-DSENGOO_PUBLIC_WRAPPER_PROBE", PROBE, out/"runtime.obj", out/"runtime_string.obj",
                ir, "-o", exe])
            result = run("public-"+name+"-execute", [exe], 5 if name == "old" else 42)
            if result.stdout or result.stderr:
                raise RuntimeError("public wrapper resource/output assertion failed")
        report["artifacts"] = {name: sha(out/name) for name in
             ("runtime.obj", "runtime_string.obj", "next.exe", "public-next.ll", "public-next.exe")}
        report["snapshots"] = snapshots
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", failure=type(error).__name__+": "+str(error))
    finally:
        try:
            after = inputs() if before else {}
            report["inputs_after"] = after
            report["source_changes"] = [path for path in sorted(before.keys() | after.keys())
                                       if before.get(path) != after.get(path)]
            report["snapshot_changes"] = [path for path, digest in snapshots.items() if sha(path) != digest]
            report["tool_changes"] = [name for name, (path, digest) in tools.items() if sha(path) != digest]
            report["source_revision_after"] = revision()
            if report["source_changes"] or report["snapshot_changes"] or report["tool_changes"]:
                report["result"] = "failed"
        except BaseException as error:
            report.update(result="failed", finally_failure=type(error).__name__+": "+str(error))
        report["finished_at"] = dt.datetime.now(dt.timezone.utc).isoformat()
        save()
    print(json.dumps({key: report.get(key) for key in
                     ("result", "failure", "source_changes", "snapshot_changes", "tool_changes")}), flush=True)
    return 0 if report["result"] == "passed" else 1

if __name__ == "__main__":
    raise SystemExit(main())
