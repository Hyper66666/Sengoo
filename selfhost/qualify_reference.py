"""Build and qualify the explicit Rust release reference for self-hosting."""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
SOURCE_ROOTS = ("compiler", "runtime", "tools/sgc", "tools/sgfmt", "tools/sgpm", "tools/sglsp",
                "tools/stdlib", "examples", "packages", "openspec/specs")
DIRECT_INPUTS = ("Cargo.toml", "Cargo.lock", "rust-toolchain.toml", "selfhost/bootstrap.lock",
                 "selfhost/qualify_reference.py", "selfhost/src/core.sg", "selfhost/tests/core.sg",
                 "docs/language-reference.md", "docs/compatibility-policy.md")


def sha(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(65536), b""):
            digest.update(block)
    return digest.hexdigest()


def inventory() -> dict[str, str]:
    # Include untracked non-ignored inputs as well as tracked tests/fixtures.
    # Git's ignore rules exclude native caches and SG generated build trees.
    names = subprocess.check_output(["git", "ls-files", "--cached", "--others", "--exclude-standard", "-z", "--",
                                     *SOURCE_ROOTS, *DIRECT_INPUTS,
                                     "selfhost/tests/fixtures", "selfhost/bench",
                                     "selfhost/baseline.py", "selfhost/baseline-workloads.json"], cwd=ROOT).split(b"\0")
    paths = {name.decode("utf-8") for name in names if name}
    paths.update(DIRECT_INPUTS)
    return {name: sha(ROOT / name) for name in sorted(paths)}


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--clang", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path, default=ROOT / "target/selfhost/seed-reference")
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/reference-qualification")
    args = parser.parse_args()
    out, build = args.output.resolve(), args.target_dir.resolve()
    if not out.is_relative_to(ROOT / "target") or not build.is_relative_to(ROOT / "target"):
        raise ValueError("outputs must stay inside ignored target/")
    out.mkdir(parents=True, exist_ok=True)
    report_path = out / "evidence.json"
    report = {"schema": 1, "result": "running", "profile": "release", "source_revision": None,
              "recorded_at_utc": datetime.now(timezone.utc).isoformat(),
              "host": {"platform": platform.platform(), "machine": platform.machine(), "name": platform.node()},
              "tools": {}, "native_tools": {}, "steps": [],
              "limits": ["reference-source-development qualification, not installed release or self-hosting",
                         "local binaries are not published seed assets; M8 compatibility suite remains separate",
                         "SDK/MSVC and Rust dependency caches are host inputs; no byte-identical clean-machine rebuilding claim"]}
    # Replace old success before checking any supplied tool/input identity.
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    env = dict(os.environ)
    sources = {}

    def save() -> None:
        temporary = report_path.with_suffix(".tmp")
        temporary.write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        temporary.replace(report_path)

    def run(name: str, command: list[Path | str], marker: str | None = None, min_tests: int | None = None):
        started = time.monotonic()
        step = {"name": name, "command": list(map(str, command)), "result": "running"}
        report["steps"].append(step)
        save()
        try:
            result = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=1800)
        except subprocess.TimeoutExpired as error:
            (out / (name + ".stdout.log")).write_bytes(error.stdout or b"")
            (out / (name + ".stderr.log")).write_bytes(error.stderr or b"")
            step.update(result="timeout", seconds=time.monotonic() - started)
            raise
        stdout = result.stdout.decode("utf-8", errors="replace")
        stderr = result.stderr.decode("utf-8", errors="replace")
        (out / (name + ".stdout.log")).write_bytes(result.stdout)
        (out / (name + ".stderr.log")).write_bytes(result.stderr)
        summaries = re.findall(r"test result: (\w+)\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out", stdout)
        step.update(exit_code=result.returncode, seconds=time.monotonic() - started,
                    test_results=re.findall(r"test result: .*", stdout))
        if result.returncode != 0 or (marker is not None and marker not in stdout):
            step["result"] = "failed"
            raise RuntimeError(f"{name} failed; see {out}")
        if min_tests is not None:
            if not summaries or any(status != "ok" or int(failed) or int(ignored) or int(filtered)
                                    for status, passed, failed, ignored, measured, filtered in summaries):
                step["result"] = "failed"
                raise RuntimeError(f"{name}: missing/failed/skipped/filtered test summary")
            if sum(int(item[1]) for item in summaries) < min_tests:
                step["result"] = "failed"
                raise RuntimeError(f"{name}: fewer tests than the frozen reference")
        step["result"] = "passed"
        save()
        print(f"{name}: passed", flush=True)
        return stdout, stderr

    try:
        if os.name != "nt" or platform.machine().lower() not in ("amd64", "x86_64"):
            raise ValueError("native Windows x64 required")
        lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text(encoding="utf-8"))
        clang = args.clang.resolve(strict=True)
        if sha(clang) != lock["clang"]["sha256"]:
            raise ValueError("clang identity mismatch")
        report["native_tools"]["clang"] = {"path": str(clang), "sha256": sha(clang)}
        for name in list(env):
            upper = name.upper()
            if upper.startswith(("SENGOO_", "SGPLATFORM_", "SDL2_", "CARGO_PROFILE_", "CARGO_TARGET_")) or upper in (
                    "RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC", "CARGO_BUILD_RUSTC", "RUSTC_WRAPPER",
                    "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_TARGET", "CARGO_BUILD_RUSTC_WRAPPER",
                    "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTFLAGS"):
                env.pop(name)
        env.update(SENGOO_CLANG=str(clang), SENGOO_STDLIB=str(ROOT / "tools/stdlib"),
                   SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"), RUSTUP_TOOLCHAIN=lock["rust_toolchain"])
        revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
        report["source_revision"] = revision
        report["working_tree"] = subprocess.check_output(["git", "status", "--porcelain=v1"], cwd=ROOT, text=True)
        env["SENGOO_BUILD_HASH"] = revision[:12]
        sources = inventory()
        report["sources"] = sources
        rustup = shutil.which("rustup")
        if not rustup:
            raise ValueError("rustup metadata locator is required")
        for name in ("rustc", "cargo"):
            location, _ = run(name + "-location", [rustup, "which", "--toolchain", lock["rust_toolchain"], name])
            executable = Path(location.strip()).resolve(strict=True)
            digest = sha(executable)
            if digest != lock[name + "_sha256"]:
                raise ValueError(f"{name} native toolchain hash mismatch")
            report["native_tools"][name] = {"path": str(executable), "sha256": digest}
            version, _ = run(name + "-version", [executable, "--version"])
            if not version.startswith(lock[name + "_version_prefix"]):
                raise ValueError(f"{name} native toolchain version mismatch")
            report["native_tools"][name]["version"] = version.strip()
        clang_version, _ = run("clang-version", [clang, "--version"])
        if lock["clang"]["version_contains"] not in clang_version:
            raise ValueError("clang version mismatch")
        report["native_tools"]["clang"]["version"] = clang_version.strip()
        cargo = Path(report["native_tools"]["cargo"]["path"])
        rustc = Path(report["native_tools"]["rustc"]["path"])
        env["RUSTC"] = str(rustc)
        env["PATH"] = os.pathsep.join((str(clang.parent), str(cargo.parent), env.get("PATH", "")))
        flags = ["--locked", "--release", "--target-dir", build]
        run("build", [cargo, "build", *flags, "-p", "sgc", "-p", "sgfmt", "-p", "sgpm", "-p", "sglsp"])
        for name in ("sgc", "sgfmt", "sgpm", "sglsp"):
            binary = build / "release" / (name + ".exe")
            report["tools"][name] = {"path": str(binary), "sha256": sha(binary), "bytes": binary.stat().st_size}
            version, _ = run(name + "-version", [binary, "--version"])
            if not version.strip().startswith(name + " ") or not version.strip().endswith("(" + revision[:12] + ")"):
                raise ValueError(f"{name} embedded source revision mismatch")
            report["tools"][name]["version"] = version.strip()
        run("compiler-tests", [cargo, "test", *flags, "-p", "sengoo-compiler", "--lib"], min_tests=1178)
        run("formatter-tests", [cargo, "test", *flags, "-p", "sgfmt", "--lib"], min_tests=11)
        run("package-tests", [cargo, "test", *flags, "-p", "sgpm", "--bin", "sgpm"], min_tests=64)
        run("lsp-tests", [cargo, "test", *flags, "-p", "sglsp", "--bin", "sglsp"], min_tests=192)
        core = out / "core.exe"
        core.unlink(missing_ok=True)
        env["SENGOO_MODULE_MAP"] = "selfhost_core=" + str(ROOT / "selfhost/src/core.sg")
        run("build-core", [build / "release/sgc.exe", "--runtime-mode", "source-development", "build",
                            ROOT / "selfhost/tests/core.sg", "--force-rebuild", "-O", "2", "-o", core])
        run("native-core", [core], "selfhost core: ok")
        report["generated_core"] = {"path": str(core), "sha256": sha(core)}
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = []
        if sources:
            try:
                current = inventory()
                changed = sorted(name for name in sources.keys() | current.keys() if sources.get(name) != current.get(name))
            except BaseException as error:
                changed = ["source inventory unavailable: " + repr(error)]
        changed_tools = []
        for category in ("tools", "native_tools"):
            for name, identity in report[category].items():
                try:
                    if sha(Path(identity["path"])) != identity["sha256"]:
                        changed_tools.append(category + "/" + name)
                except OSError:
                    changed_tools.append(category + "/" + name)
        revision_changed = False
        if report["source_revision"]:
            try:
                revision_changed = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip() != report["source_revision"]
            except BaseException:
                revision_changed = True
        report.update(changed_sources=changed, changed_tools=changed_tools, source_revision_changed=revision_changed)
        if changed or changed_tools or revision_changed:
            report.update(result="failed", error="qualification source/tool/revision changed")
        save()
        if report["result"] == "failed" and not sys.exc_info()[0]:
            raise RuntimeError(report["error"])


if __name__ == "__main__":
    main()
