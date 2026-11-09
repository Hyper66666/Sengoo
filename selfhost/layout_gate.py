"""Check SG Windows x64 layout against native C and bounded negative fixtures."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m4-layout-final")
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target"):
        raise ValueError("outputs must stay inside target/")
    out.mkdir(parents=True, exist_ok=True)
    paths = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter")}
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
    names = ("core", "types", "lexer", "mir", "mir_validate", "layout")
    inputs = [Path(__file__), ROOT / "selfhost/bootstrap.lock", ROOT / "selfhost/tests/layout.sg",
              ROOT / "selfhost/tests/layout_probe.c"]
    inputs += [ROOT / "selfhost/src" / (name + ".sg") for name in names]
    inputs += sorted((ROOT / "tools/stdlib").rglob("*.sg"))
    inputs += [ROOT / "tools/stdlib/runtime.c", ROOT / "tools/stdlib/runtime_abi.h"]
    inputs = [p for p in inputs if p.is_file()]
    report = {"schema": 1, "result": "running", "stage": "windows-x64-concrete-type-layout",
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "host": platform.platform(), "sources": {str(p.relative_to(ROOT)): sha(p) for p in inputs},
              "tools": {}, "steps": [], "m4_complete": False,
              "limits": ["requested concrete type only; unspecialized templates and raw slice values reject",
                         "size limit at most 2147483647 bytes; recursion limit 128",
                         "not native MIR code generation, complete C ABI classification or self-compilation"]}
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
            env.pop(name)
    env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"),
               SENGOO_CLANG=str(paths["clang"]), SENGOO_MODULE_MAP=";".join(
                   "selfhost_" + name + "=" + str(ROOT / "selfhost/src" / (name + ".sg")) for name in names))
    env["PATH"] = str(paths["clang"].parent) + os.pathsep + env.get("PATH", "")

    def run(name, command, marker=None):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True, timeout=600)
        (out / (name + ".stdout.log")).write_bytes(done.stdout)
        (out / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)),
                                "exit_code": done.returncode, "seconds": time.monotonic() - started})
        if done.returncode or marker and marker not in done.stdout:
            raise AssertionError(f"{name} failed: {done.returncode}")
        print(f"{name}: {done.returncode}", flush=True)
        return done.stdout

    try:
        for name, path in paths.items():
            digest = sha(path)
            if digest != lock[name]["sha256"]:
                raise ValueError(f"{name} identity mismatch")
            report["tools"][name] = {"path": str(path), "sha256": digest}
        for folder in ("src", "tests"):
            run("format-" + folder, [paths["formatter"], "--check", ROOT / "selfhost" / folder / "layout.sg"])
        binary = out / "layout-tests.exe"
        binary.unlink(missing_ok=True)
        run("build-sg", [paths["seed"], "--runtime-mode", "source-development", "build",
                         ROOT / "selfhost/tests/layout.sg", "--force-rebuild", "-O", "0", "-o", binary])
        run("native-sg", [binary], b"selfhost Windows x64 layout: ok")
        actual = run("sg-layout", [binary, "emit"])
        probe = out / "layout-probe.exe"
        run("build-c", [paths["clang"], ROOT / "selfhost/tests/layout_probe.c", "-std=c11", "-Werror", "-o", probe])
        expected = run("c-layout", [probe])
        actual_values = [int(value) for value in actual.splitlines()]
        expected_values = [int(value) for value in expected.splitlines()]
        if len(actual_values) != 10 or actual_values != expected_values:
            raise AssertionError("SG/C size, alignment or field offsets differ")
        report["c_layout_values"] = expected_values
        report["sg_layout_values"] = actual_values
        report["artifacts"] = {p.name: sha(p) for p in (binary, probe)}
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [name for name, digest in report["sources"].items()
                   if not (ROOT / name).is_file() or sha(ROOT / name) != digest]
        changed_tools = [name for name, identity in report["tools"].items()
                         if not Path(identity["path"]).is_file() or sha(Path(identity["path"])) != identity["sha256"]]
        report.update(changed_sources=changed, changed_tools=changed_tools)
        if changed or changed_tools:
            report.update(result="failed", error="layout inputs changed during verification")
        (out / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise AssertionError("layout inputs changed")


if __name__ == "__main__":
    main()
