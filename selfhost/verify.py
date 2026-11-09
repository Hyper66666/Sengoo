"""Verify the M1 stage0 foundation. No Python implementation of compiler logic."""
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

ROOT = Path(__file__).resolve().parents[1]
LOCK = ROOT / "selfhost/bootstrap.lock"


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(65536), b""):
            digest.update(block)
    return digest.hexdigest()


def run_gate(args: argparse.Namespace) -> None:
    lock = json.loads(LOCK.read_text(encoding="utf-8"))
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("evidence/output directory must be inside the ignored repository target/")
    output.mkdir(parents=True, exist_ok=True)
    report = {"schema": 1, "stage": "m1-stage0-source-development", "result": "running",
              "host": platform.node(), "platform": platform.platform(),
              "python": sys.version, "lock_sha256": sha(LOCK), "steps": [],
              "limits": ["Rust reference compiler and C/clang/Rust runtime are required",
                         "not an SG compiler, native SDK, or self-hosting fixed-point proof",
                         "local seed is not distributed by a source checkout",
                         "MSVC linker and system SDK headers/libraries are host inputs not fully pinned by M1"]}
    # Replace any previous pass before preflight can fail.
    (output / "evidence.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    env = dict(os.environ)
    # Never serialize environment values: the host can contain provider secrets.
    tools = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter")}
    source_files = [ROOT / "selfhost/bootstrap.sg", ROOT / "selfhost/src/core.sg",
                    ROOT / "selfhost/tests/core.sg", LOCK, Path(__file__).resolve()]
    source_files += sorted(p for p in (ROOT / "selfhost/tests/fixtures").iterdir() if p.is_file())
    source_files += [ROOT / "selfhost/README.md", ROOT / "selfhost/.gitignore"]

    def run(name: str, command: list[str | Path], expected: int | None = 0,
            required: str | None = None) -> subprocess.CompletedProcess[str]:
        started = time.monotonic()
        result = subprocess.run(list(map(str, command)), cwd=ROOT, env=env,
                                capture_output=True, encoding="utf-8", errors="replace",
                                timeout=600)
        (output / f"{name}.stdout.log").write_text(result.stdout, encoding="utf-8")
        (output / f"{name}.stderr.log").write_text(result.stderr, encoding="utf-8")
        step = {"name": name, "command": list(map(str, command)), "exit_code": result.returncode,
                "seconds": time.monotonic() - started, "expected_exit_code": expected}
        report["steps"].append(step)
        if expected is not None and result.returncode != expected:
            raise RuntimeError(f"{name}: exit {result.returncode}, expected {expected}; see {output}")
        if required is not None and required not in result.stdout + result.stderr:
            raise RuntimeError(f"{name}: missing expected output {required!r}")
        print(f"{name}: {result.returncode}", flush=True)
        return result

    try:
        if lock["schema"] != 1 or lock["target"] != "x86_64-pc-windows-msvc":
            raise ValueError("unsupported bootstrap lock schema or target")
        if lock["stage"] != report["stage"] or lock["runtime_mode"] != "source-development" or lock["optimization"] != 0:
            raise ValueError("unsupported stage0 runtime mode or optimization policy")
        if platform.system() != "Windows" or platform.machine().lower() not in ("amd64", "x86_64"):
            raise ValueError("M1 gate requires a native Windows x64 host")
        # Hash every executable before executing even --version.
        report["tools"] = {}
        for name, path in tools.items():
            actual = sha(path)
            if actual != lock[name]["sha256"]:
                raise ValueError(f"bootstrap::{name}_mismatch: {path}")
            report["tools"][name] = {"path": str(path), "sha256": actual}
        report["revision"] = run("revision", ["git", "rev-parse", "HEAD"]).stdout.strip()
        report["working_tree"] = run("working-tree", ["git", "status", "--porcelain=v1"]).stdout
        for path, expected in lock["source_objects"].items():
            actual = run("object-" + path.replace("/", "-"),
                         ["git", "rev-parse", f"{lock['source_revision']}:{path}"]).stdout.strip()
            if actual != expected:
                raise ValueError(f"source object mismatch: {path}")
        run("source-guard", ["git", "diff", "--exit-code", lock["source_revision"], "--",
                             *lock["source_objects"]])
        untracked_command = ["git", "ls-files", "--others", "--exclude-standard", "--",
                             *lock["source_objects"]]
        if run("untracked-seed-inputs", untracked_command).stdout.strip():
            raise ValueError("untracked source in pinned seed/runtime inputs")
        baseline_files = run("baseline-files", ["git", "ls-tree", "-r", "--name-only",
                             lock["source_revision"], "--", *lock["source_objects"]]).stdout.splitlines()
        all_sources = set(source_files) | {ROOT / path for path in baseline_files}
        report["sources"] = {str(p.relative_to(ROOT)): sha(p) for p in sorted(all_sources)}
        for name in ("seed", "formatter"):
            version = run(name + "-version", [tools[name], "--version"]).stdout.strip()
            if version != lock[name]["version"]:
                raise ValueError(f"{name} embedded version mismatch")
        clang_version = run("clang-version", [tools["clang"], "--version"]).stdout
        if lock["clang"]["version_contains"] not in clang_version:
            raise ValueError("clang version mismatch")
        rustup = shutil.which("rustup")
        if not rustup:
            raise ValueError("rustup is required to locate the explicit locked Rust toolchain")
        env["RUSTUP_TOOLCHAIN"] = lock["rust_toolchain"]
        for name in ("rustc", "cargo"):
            executable = Path(run(name + "-location", [rustup, "which", "--toolchain",
                                      lock["rust_toolchain"], name]).stdout.strip())
            digest = sha(executable)
            if digest != lock[name + "_sha256"]:
                raise ValueError(f"{name} native binary hash mismatch")
            version = run(name + "-version", [executable, "--version"]).stdout
            if not version.startswith(lock[name + "_version_prefix"]):
                raise ValueError(f"{name} version mismatch")
            report["tools"][name] = {"path": str(executable), "version": version.strip(),
                                     "sha256": digest}
        rust_bin = str(Path(report["tools"]["cargo"]["path"]).parent)
        for name in list(env):
            if name.startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
                env.pop(name)
        env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"),
                   SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"),
                   SENGOO_CLANG=str(tools["clang"]),
                   SENGOO_MODULE_MAP="selfhost_core=" + str(ROOT / "selfhost/src/core.sg"))
        # Discard inherited compiler wrappers/flags which could alter runtime objects.
        for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "RUSTC_WRAPPER",
                     "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET"):
            env.pop(name, None)
        env["PATH"] = os.pathsep.join((str(tools["clang"].parent), rust_bin, env.get("PATH", "")))
        if lock["seed"]["sha256"] not in (ROOT / "selfhost/bootstrap.sg").read_text(encoding="utf-8"):
            raise ValueError("SG driver seed identity differs from bootstrap.lock")
        if args.preflight_only:
            report["result"] = "preflight-passed"
            return

        for name in ("bootstrap.exe", "core-tests.exe", "bootstrap-rebuilt.exe",
                     "program with spaces 世界 &.exe", "absent-seed.exe"):
            (output / name).unlink(missing_ok=True)

        for path in source_files:
            if path.suffix == ".sg":
                name = path.relative_to(ROOT / "selfhost").as_posix().replace("/", "-")
                run("format-" + name, [tools["formatter"], "--check", path])
        driver = output / "bootstrap.exe"
        run("build-driver", [tools["seed"], "--runtime-mode", "source-development", "build",
                             ROOT / "selfhost/bootstrap.sg", "--force-rebuild", "-O", "0", "-o", driver])
        marker = "sengoo bootstrap M1 (stage0 source-development)"
        run("driver-version", [driver, "--version"], required=marker)
        run("driver-help", [driver, "--help"], required="bootstrap build")
        run("driver-default-help", [driver], required="bootstrap build")
        run("driver-bad-operation", [driver, "unknown"], 64, "bootstrap::usage")
        run("driver-missing-arguments", [driver, "build"], 64, "bootstrap::usage")
        prefix = [driver, "build", tools["seed"], ROOT]
        core = output / "core-tests.exe"
        run("driver-build-core", [*prefix, ROOT / "selfhost/tests/core.sg", core])
        run("core-native", [core], required="selfhost core: ok")
        run("driver-check", [driver, "check", tools["seed"], ROOT,
                             ROOT / "selfhost/tests/fixtures/hello.sg"])
        negative = ROOT / "selfhost/tests/fixtures/rejected.sg"
        direct = run("seed-reject", [tools["seed"], "--runtime-mode", "source-development",
                                     "check", negative], expected=None,
                     required="selfhost_unknown_identifier")
        if direct.returncode == 0:
            raise AssertionError("negative fixture unexpectedly accepted")
        run("driver-propagates-rejection", [driver, "check", tools["seed"], ROOT, negative],
            direct.returncode, "selfhost_unknown_identifier")
        fake_seed = output / "changed-seed.exe"
        fake_seed.write_bytes(b"not the pinned seed\n")
        run("driver-tampered-seed", [driver, "check", fake_seed, ROOT, negative],
            65, "bootstrap::seed_mismatch")
        run("driver-missing-seed", [driver, "check", output / "absent-seed.exe", ROOT, negative],
            66, "bootstrap::input_or_process")
        unusual = output / "source with spaces 世界 &.sg"
        shutil.copyfile(ROOT / "selfhost/tests/fixtures/hello.sg", unusual)
        program = output / "program with spaces 世界 &.exe"
        run("driver-unicode-paths", [*prefix, unusual, program])
        run("unicode-native", [program], required="selfhost fixture: ok")
        rebuilt = output / "bootstrap-rebuilt.exe"
        run("driver-rebuild", [*prefix, ROOT / "selfhost/bootstrap.sg", rebuilt])
        run("rebuilt-driver-version", [rebuilt, "--version"], required=marker)
        run("rebuilt-driver-check", [rebuilt, "check", tools["seed"], ROOT,
                                     ROOT / "selfhost/tests/fixtures/hello.sg"])
        report["binaries"] = {p.name: sha(p) for p in (driver, core, program, rebuilt)}
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=str(error))
        raise
    finally:
        if "sources" in report:
            changed = []
            for path, digest in report["sources"].items():
                try:
                    matches = sha(ROOT / path) == digest
                except OSError:
                    matches = False
                if not matches:
                    changed.append(path)
            report["changed_sources"] = changed
            if changed:
                report.update(result="failed", error="source changed during verification")
            try:
                ending_diff = run("source-guard-end", ["git", "diff", "--exit-code",
                                  lock["source_revision"], "--", *lock["source_objects"]], expected=None)
                if ending_diff.returncode != 0:
                    report.update(result="failed", error="pinned source changed during verification")
                if run("untracked-seed-inputs-end", untracked_command).stdout.strip():
                    report.update(result="failed", error="untracked source added during verification")
            except Exception as error:
                report.update(result="failed", error=f"source guard failed: {error}")
        for name, identity in report.get("tools", {}).items():
            try:
                matches = sha(Path(identity["path"])) == identity["sha256"]
            except OSError:
                matches = False
            if not matches:
                report.update(result="failed", error=f"{name} changed during verification")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n",
                                              encoding="utf-8")
        if report["result"] == "failed" and not sys.exc_info()[0]:
            raise RuntimeError(report["error"])


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m1")
    parser.add_argument("--preflight-only", action="store_true")
    run_gate(parser.parse_args())


if __name__ == "__main__":
    main()
