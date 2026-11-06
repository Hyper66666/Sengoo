#!/usr/bin/env python3
"""Instrument generated ownership code and the split C runtime sources.

This is a required gate: missing tools, inactive instrumentation, failed probes,
sanitizer reports, resource leaks, and compiler errors all return a nonzero code.
Clang's ASan LLVM pass instruments textual IR after sanitize_address attributes
are added to a test-only copy. UBSan is a C frontend pass, so its coverage is
explicitly limited to the C runtime/probe; no UBSan claim is made for raw IR.
"""
from __future__ import annotations

import argparse
from contextlib import nullcontext
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import runpy
import shutil
import subprocess
import sys
import tempfile


RUNTIME_SOURCES = (
    "runtime.c", "runtime_breadth.c", "runtime_collections.c", "runtime_json.c",
    "runtime_process.c", "runtime_terminal.c", "runtime_ipc.c", "runtime_pty.c", "runtime_hash_file.c", "runtime_system.c", "runtime_stream.c", "runtime_string.c",
)
PROBE_C = r"""
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
static int64_t live_resources, created_resources, dropped_resources;
int64_t ownership_probe_new(int64_t value) {
    int64_t *resource = (int64_t *)malloc(sizeof(*resource));
    if (!resource) abort();
    *resource = value;
    ++live_resources; ++created_resources;
    return (int64_t)(intptr_t)resource;
}
void ownership_probe_drop(int64_t handle) {
    int64_t *resource = (int64_t *)(intptr_t)handle;
    /* Real freed allocations expose early/duplicate Drop to ASan. */
    if (*resource <= 0) abort();
    *resource = -1;
    --live_resources; ++dropped_resources;
    free(resource);
}
int64_t ownership_probe_read(int64_t handle) {
    return *(int64_t *)(intptr_t)handle;
}
int64_t ownership_probe_live(void) { return live_resources; }
int64_t ownership_probe_created(void) { return created_resources; }
int64_t ownership_probe_dropped(void) { return dropped_resources; }
"""
ASAN_CANARY = """
declare i8* @malloc(i64)
define i32 @main() {
entry:
  %allocation = call i8* @malloc(i64 8)
  %outside = getelementptr i8, i8* %allocation, i64 16
  store volatile i8 1, i8* %outside
  ret i32 0
}
"""


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def windows_environment(environment: dict[str, str]) -> dict[str, str]:
    """Find a usable MSVC/SDK pair without selecting the Installer directory."""
    roots = []
    configured = environment.get("VCTOOLSINSTALLDIR")
    if configured:
        roots.append(Path(configured))
    for base in (environment.get("PROGRAMFILES(X86)"), environment.get("PROGRAMFILES")):
        if base:
            roots.extend((Path(base) / "Microsoft Visual Studio").glob("*/*/VC/Tools/MSVC/*"))
    valid = [p for p in roots if (p / "include/vcruntime.h").is_file()
        and (p / "lib/x64/libcmt.lib").is_file()]
    if not valid:
        raise RuntimeError("MSVC x64 C headers/libraries are required; install Visual Studio C++ Build Tools")
    vc = max(valid, key=lambda p: tuple(int(n) for n in re.findall(r"\d+", p.name)))
    kits = Path(environment.get("PROGRAMFILES(X86)", "C:/Program Files (x86)")) / "Windows Kits/10"
    sdk_versions = [p for p in (kits / "Include").glob("*")
        if (p / "ucrt/stdlib.h").is_file() and (kits / "Lib" / p.name / "um/x64/kernel32.lib").is_file()]
    if not sdk_versions:
        raise RuntimeError("Windows SDK x64 headers and libraries are required")
    sdk = max(sdk_versions, key=lambda p: tuple(int(n) for n in re.findall(r"\d+", p.name)))
    include = [vc / "include", *(sdk / name for name in ("ucrt", "um", "shared"))]
    libraries = [vc / "lib/x64", kits / "Lib" / sdk.name / "ucrt/x64", kits / "Lib" / sdk.name / "um/x64"]
    environment["INCLUDE"] = os.pathsep.join([*(str(p) for p in include), environment.get("INCLUDE", "")])
    environment["LIB"] = os.pathsep.join([*(str(p) for p in libraries), environment.get("LIB", "")])
    environment["PATH"] = str(vc / "bin/Hostx64/x64") + os.pathsep + environment.get("PATH", "")
    return environment


def add_asan_attributes(source: str) -> tuple[str, int]:
    """Only this gate's copy changes: Clang does not add this to raw .ll input."""
    count = 0

    def instrument(match: re.Match[str]) -> str:
        nonlocal count
        count += 1
        return match.group(0).rstrip()[:-1].rstrip() + " sanitize_address {"

    prepared = re.sub(r"^define\b[^\n]*\{", instrument, source, flags=re.MULTILINE)
    if count == 0:
        raise RuntimeError("sgc emitted no LLVM function definitions to instrument")
    return prepared, count


def main() -> int:
    repo = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sgc", type=Path, default=repo / "target/debug" / ("sgc.exe" if os.name == "nt" else "sgc"))
    parser.add_argument("--clang", default=shutil.which("clang-19") or shutil.which("clang") or "clang")
    parser.add_argument("--fixture", type=Path, default=repo / "scripts/fixtures/ownership-sanitizer.sg")
    parser.add_argument("--output", type=Path, default=repo / "target/ownership-sanitizer-evidence.json")
    parser.add_argument("--work-dir", type=Path, help="Retain sources, IR, command logs and native artifacts here (the default also retains them)")
    parser.add_argument("--generated-seed", type=lambda value: int(value, 0), default=20261104)
    parser.add_argument("--generated-cases", type=int, default=4, help="Bounded positive programs, 1..32; three compile-negative cases are always required")
    args = parser.parse_args()
    args.output = args.output.resolve()
    report: dict = {"schema_version": 1, "ok": False, "host": platform.platform(),
        "commands": [], "coverage": {
            "generated_ir": ["address"],
            "c_runtime": ["address"] if os.name == "nt" else ["address", "undefined"],
            "raw_ir_ubsan": False,
            "raw_ir_ubsan_reason": "Clang UBSan requires C frontend semantics; raw LLVM IR is checked by ASan and explicit ownership assertions",
            "windows_leak_sanitizer": False,
        }}
    try:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        directory = args.work_dir.resolve() if args.work_dir else Path(tempfile.mkdtemp(
            prefix="sengoo-owned-sanitizers-", dir=args.output.parent))
        directory.mkdir(parents=True, exist_ok=True)
        report["artifacts"] = str(directory)
        generator = runpy.run_path(str(repo / "scripts/ownership-combinations.py"))
        generated = generator["write_corpus"](directory / "generated", args.generated_seed, args.generated_cases)
        report["generated_corpus"] = {key: value for key, value in generated.items() if key != "cases"}
        report["generated_cases"] = [{**case, "status": "not_run"} for case in generated["cases"]]
        environment = os.environ.copy()
        if os.name == "nt":
            environment = {key.upper(): value for key, value in environment.items()}
            environment = windows_environment(environment)
        clang = shutil.which(args.clang, path=environment.get("PATH"))
        if not clang:
            raise RuntimeError(f"required clang is unavailable: {args.clang}")
        sgc = args.sgc.resolve()
        if not sgc.is_file():
            raise RuntimeError(f"required sgc executable is missing: {sgc}")
        environment["PATH"] = str(Path(clang).parent) + os.pathsep + environment.get("PATH", "")
        environment["ASAN_OPTIONS"] = "halt_on_error=1:strict_string_checks=1:detect_leaks=" + ("0" if os.name == "nt" else "1")
        environment["UBSAN_OPTIONS"] = "halt_on_error=1:print_stacktrace=1"

        def run(command: list[str | Path], cwd: Path, *, expected_failure: str | None = None) -> subprocess.CompletedProcess:
            argv = [str(arg) for arg in command]
            # Version probes run from the repository; their logs still belong
            # to the retained gate directory, never a new repository folder.
            logs = (directory if cwd == repo else cwd) / "commands"
            logs.mkdir(parents=True, exist_ok=True)
            log_stem = logs / f"{len(report['commands']):03d}"
            record = {"argv": argv, "cwd": str(cwd), "expected_failure": expected_failure,
                "stdout_log": str(log_stem.with_suffix(".stdout.txt")), "stderr_log": str(log_stem.with_suffix(".stderr.txt"))}
            report["commands"].append(record)
            try:
                result = subprocess.run(argv, cwd=cwd, env=environment, capture_output=True,
                    text=True, encoding="utf-8", errors="replace", timeout=300)
            except subprocess.TimeoutExpired as error:
                record.update({"exit_code": None, "timeout_seconds": 300})
                for stream in ("stdout", "stderr"):
                    value = getattr(error, stream) or ""
                    if isinstance(value, bytes):
                        value = value.decode("utf-8", errors="replace")
                    Path(record[stream + "_log"]).write_text(value, encoding="utf-8")
                log_stem.with_suffix(".command.json").write_text(json.dumps(record, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
                raise
            record.update({"exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr})
            Path(record["stdout_log"]).write_text(result.stdout, encoding="utf-8")
            Path(record["stderr_log"]).write_text(result.stderr, encoding="utf-8")
            log_stem.with_suffix(".command.json").write_text(json.dumps(record, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            if expected_failure is None and result.returncode:
                raise RuntimeError(f"command failed ({result.returncode}): {argv!r}\n{result.stdout}\n{result.stderr}")
            if expected_failure is not None and (result.returncode == 0 or expected_failure not in result.stderr):
                raise RuntimeError(f"required failure diagnostic {expected_failure!r} was not detected: {argv!r}\n{result.stdout}\n{result.stderr}")
            return result

        compiler = run([clang, "--version"], repo)
        report["clang"] = compiler.stdout.splitlines()[0]
        report["sgc"] = run([sgc, "--version"], repo).stdout.strip()
        report["revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
        report["working_tree_dirty"] = bool(subprocess.check_output(["git", "status", "--porcelain"], cwd=repo, text=True).strip())
        with nullcontext(str(directory)) as temporary:
            directory = Path(temporary)
            directory.mkdir(parents=True, exist_ok=True)
            fixture = directory / "ownership.sg"
            fixture.write_bytes(args.fixture.read_bytes())
            report["fixture_sha256"] = digest(fixture)
            raw_ir = directory / "ownership.raw.ll"
            run([sgc, "--runtime-mode", "source-development", "build", fixture, "--emit-llvm",
                "--force-rebuild", "-O1", "--contract-checks", "off", "-o", raw_ir], directory)
            report["raw_ir_sha256"] = digest(raw_ir)

            sanitizer = "-fsanitize=address" if os.name == "nt" else "-fsanitize=address,undefined"
            flags = ["-O1", "-g", "-fno-omit-frame-pointer", sanitizer, "-fno-sanitize-recover=all"]
            c_standard = "-std=c11" if os.name == "nt" else "-std=gnu11"
            suffix = ".exe" if os.name == "nt" else ""

            def instrument_ir(source: Path, name: str, artifact_directory: Path | None = None) -> Path:
                artifact_directory = artifact_directory or directory
                prepared, functions = add_asan_attributes(source.read_text(encoding="utf-8"))
                tagged = artifact_directory / (name + ".asan-input.ll")
                tagged.write_text(prepared, encoding="utf-8")
                instrumented = artifact_directory / (name + ".instrumented.ll")
                run([clang, *flags, "-x", "ir", "-S", "-emit-llvm", tagged, "-o", instrumented], artifact_directory)
                text = instrumented.read_text(encoding="utf-8")
                bodies = re.findall(r"^define\b[^\n]*@([^ (]+)[^\n]*\{(.*?)^\}", text, flags=re.MULTILINE | re.DOTALL)
                protected = [symbol for symbol, body in bodies if re.search(r"\bcall\b[^\n]*@__asan_(?:report|stack|load|store)", body)]
                if not protected:
                    raise RuntimeError(f"ASan pass emitted no checked memory operations in {source}")
                report.setdefault("ir_instrumentation", {})[name] = {
                    "test_only_sanitize_address_attributes": functions,
                    "functions_with_asan_checks": protected,
                    "instrumented_ir_sha256": digest(instrumented),
                }
                obj = artifact_directory / (name + ".o")
                # Compile already-instrumented IR without rerunning the ASan pass.
                run([clang, "-O0", "-c", instrumented, "-o", obj], artifact_directory)
                return obj

            canary = directory / "asan-canary.ll"
            canary.write_text(ASAN_CANARY, encoding="utf-8")
            canary_object = instrument_ir(canary, "asan-canary")
            canary_binary = directory / ("asan-canary" + suffix)
            run([clang, sanitizer, canary_object, "-o", canary_binary], directory)
            run([canary_binary], directory, expected_failure="AddressSanitizer: heap-buffer-overflow")
            report["asan_canary_detected"] = True
            if os.name != "nt":
                ubsan = directory / "ubsan-canary.c"
                ubsan.write_text("int main(void) { volatile int zero = 0; return 7 / zero; }\n", encoding="utf-8")
                ubsan_binary = directory / "ubsan-canary"
                run([clang, *flags, ubsan, "-o", ubsan_binary], directory)
                run([ubsan_binary], directory, expected_failure="runtime error: division by zero")
                report["ubsan_canary_detected"] = True

            ownership_object = instrument_ir(raw_ir, "ownership")
            objects = [ownership_object]
            runtime_records = []
            for name in RUNTIME_SOURCES:
                source = repo / "tools/stdlib" / name
                obj = directory / (source.stem + ".o")
                run([clang, c_standard, *flags, "-c", source, "-o", obj], directory)
                runtime_records.append({"name": name, "sha256": digest(source)})
                objects.append(obj)
            report["runtime_sources"] = runtime_records
            report["runtime_shared_header_sha256"] = digest(repo / "tools/stdlib/runtime_shared.h")
            probe = directory / "ownership-probe.c"
            probe.write_text(PROBE_C, encoding="utf-8")
            probe_object = directory / "ownership-probe.o"
            run([clang, c_standard, *flags, "-c", probe, "-o", probe_object], directory)
            objects.append(probe_object)
            binary = directory / ("ownership-sanitizer" + suffix)
            libraries = (["-lws2_32", "-lbcrypt", "-lcrypt32", "-ladvapi32", "-luserenv", "-lsecur32", "-lncrypt"]
                if os.name == "nt" else ["-pthread", "-lm", *([] if sys.platform == "darwin" else ["-ldl"])])
            run([clang, sanitizer, *objects, *libraries, "-o", binary], directory)
            result = run([binary], directory)
            if "ownership-sanitizer: passed" not in result.stdout:
                raise RuntimeError("ownership fixture did not report its checked successful completion")
            report["ownership_stdout"] = result.stdout
            counts = re.search(r"created/dropped:\s+(\d+)\s+(\d+)", result.stdout)
            strings = re.search(r"String handles before/after:\s+(\d+)\s+(\d+)", result.stdout)
            if not counts or not strings or counts[1] != counts[2] or strings[1] != strings[2]:
                raise RuntimeError("ownership fixture did not report balanced resource counts")
            report["ownership_counts"] = {
                "created": int(counts[1]), "dropped": int(counts[2]),
                "string_handles_before": int(strings[1]), "string_handles_after": int(strings[2]),
            }
            failures = []
            # Runtime/probe objects are compiled once and reused by every case.
            for case in report["generated_cases"]:
                source = Path(case["source"])
                case_directory = source.parent
                case_ir = case_directory / "raw.ll"
                command = [sgc, "--runtime-mode", "source-development", "build", source, "--emit-llvm",
                    "--force-rebuild", "-O1", "--contract-checks", "off", "-o", case_ir]
                case["reproduce_build_argv"] = [str(arg) for arg in command]
                case["command_start"] = len(report["commands"])
                try:
                    if case["expect"] == "diagnostic":
                        run(command, case_directory, expected_failure=f"[{case['diagnostic']}]")
                        case["status"] = "expected_rejection"
                    else:
                        run(command, case_directory)
                        case["raw_ir_sha256"] = digest(case_ir)
                        case_object = instrument_ir(case_ir, case["name"], case_directory)
                        case_binary = case_directory / ("run" + suffix)
                        run([clang, sanitizer, case_object, *objects[1:], *libraries, "-o", case_binary], case_directory)
                        result = run([case_binary], case_directory)
                        generator["verify_output"](case, result.stdout)
                        case["status"] = "passed"
                except Exception as error:
                    case.update({"status": "failed", "error": str(error)})
                    failures.append(case["name"])
                finally:
                    case["command_end"] = len(report["commands"])
                    (case_directory / "result.json").write_text(json.dumps(case, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
            if failures:
                raise RuntimeError(f"generated ownership cases failed: {', '.join(failures)}; artifacts retained in {directory}")
            report["ok"] = True
        print("generated ownership + split runtime sanitizer gate: passed")
    except Exception as error:
        report["error"] = str(error)
        print(f"ownership sanitizer gate failed: {error}", file=sys.stderr)
    finally:
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
