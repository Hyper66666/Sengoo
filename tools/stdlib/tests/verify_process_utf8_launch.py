"""Verify Windows ProcessCommand.run's UTF-8 boundary with real child processes."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import runpy
import shutil
import subprocess
import time


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("compiler", "clang", "formatter"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    output = args.output.resolve()
    if not output.is_relative_to(repo / "target"):
        raise ValueError("output must be inside ignored repository target/")
    output.mkdir(parents=True, exist_ok=True)
    tools = {name: getattr(args, name).resolve() for name in ("compiler", "clang", "formatter")}
    expected = {
        "compiler": "9b1055c5a5cb7f09c016228a44f7427b7519b6a92e69cad5993004831c3aa1e5",
        "clang": "56ef1abd0df11a3b1b77822193e927c6345078c24511567ae10b3afd803d03a3",
        "formatter": "d075a1a0fd51a8e2434650ea542f5f252f526198ff1bf0ba27a7fce3ea7ba814",
    }
    report = {"schema": 1, "result": "running", "host": platform.platform(), "steps": [],
              "scope": "Windows synchronous ProcessCommand.run and pipeline Unicode boundary; not legacy process_run or self-hosting",
              "limits": ["native probe reuses isolated opaque allocator; SG test uses real runtime",
                         "existing clang, MSVC/Windows SDK and Rust source runtime remain required"]}
    report_path = output / "evidence.json"
    report_path.write_text(json.dumps(report, indent=2), encoding="utf-8")
    env = dict(os.environ)
    sources = {}

    def run(name: str, command: list[Path | str], marker: str | None = None) -> None:
        started = time.monotonic()
        result = subprocess.run(list(map(str, command)), cwd=repo, env=env, capture_output=True,
                                timeout=600)
        (output / f"{name}.stdout.log").write_bytes(result.stdout)
        (output / f"{name}.stderr.log").write_bytes(result.stderr)
        report["steps"].append({"name": name, "command": list(map(str, command)),
                                "exit_code": result.returncode, "seconds": time.monotonic() - started})
        if result.returncode != 0:
            raise RuntimeError(f"{name}: exit {result.returncode}; see {output}")
        if marker and marker.encode() not in result.stdout:
            raise RuntimeError(f"{name}: missing success marker")
        print(f"{name}: passed", flush=True)

    try:
        if os.name != "nt" or platform.machine().lower() not in ("amd64", "x86_64"):
            raise ValueError("native Windows x64 required")
        report["tools"] = {}
        for name, path in tools.items():
            actual = sha(path)
            if actual != expected[name]:
                raise ValueError(f"unverified {name}")
            report["tools"][name] = {"path": str(path), "sha256": actual}
        for path in (repo / "tools/stdlib").rglob("*"):
            if path.is_file() and path.suffix in (".sg", ".c", ".h", ".py") and "build" not in path.parts:
                sources[str(path.relative_to(repo))] = sha(path)
        report["sources"] = sources
        report["revision"] = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repo, text=True).strip()
        env = runpy.run_path(str(repo / "scripts/ownership-sanitizer-gate.py"))["windows_environment"](
            {key.upper(): value for key, value in env.items()})
        env.update(SENGOO_STDLIB=str(repo / "tools/stdlib"),
                   SENGOO_RUNTIME=str(repo / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(tools["clang"]))
        env["PATH"] = str(tools["clang"].parent) + os.pathsep + env.get("PATH", "")
        native = output / "native-probe.exe"
        run("native-build", [tools["clang"], "-std=c11", "-O1", "-Wno-unused-function",
                             repo / "tools/stdlib/tests/process_utf8_launch_probe.c", "-o", native])
        unusual = output / "working directory 世界😀 &"
        unusual.mkdir(exist_ok=True)
        child = unusual / "native child 世界😀 &.exe"
        shutil.copyfile(native, child)
        run("native-unicode-argv-cwd-env-invalid-timeout-pipeline", [child, "--native-tests"],
            "process UTF-8 native: 30 launches, 120 invalid inputs")
        source = repo / "tools/stdlib/tests/process_utf8_launch.sg"
        run("format-sg", [tools["formatter"], "--check", source])
        generated = output / "sg-process-utf8.exe"
        run("sg-build", [tools["compiler"], "--runtime-mode", "source-development", "build", source,
                         "--force-rebuild", "-O", "0", "-o", generated])
        run("sg-native-20-rounds", [generated, child, unusual],
            "process UTF-8 SG: 20 synchronous launches and exact resources passed")
        report["binaries"] = {str(path.relative_to(output)): sha(path) for path in (native, child, generated)}
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [path for path, digest in sources.items()
                   if not (repo / path).is_file() or sha(repo / path) != digest]
        report["changed_sources"] = changed
        changed_tools = []
        for name, identity in report.get("tools", {}).items():
            try:
                if sha(Path(identity["path"])) != identity["sha256"]:
                    changed_tools.append(name)
            except OSError:
                changed_tools.append(name)
        report["changed_tools"] = changed_tools
        if changed:
            report.update(result="failed", error="source changed during verification")
        if changed_tools:
            report.update(result="failed", error="tool changed during verification")
        report_path.write_text(json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise RuntimeError(report["error"])


if __name__ == "__main__":
    main()
