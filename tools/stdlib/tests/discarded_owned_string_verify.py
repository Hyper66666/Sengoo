"""Run native ownership regressions with one pinned source-runtime compiler."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--clang", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    root = Path(__file__).resolve().parents[3]
    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ)
    environment.update(SENGOO_CLANG=str(args.clang), SENGOO_STDLIB=str(root / "tools/stdlib"), SENGOO_RUNTIME=str(root / "tools/stdlib/runtime.c"), SENGOO_MODULE_MAP="sgllm=" + str(root / "packages/sgllm/src/lib.sg"))
    environment["PATH"] = str(args.clang.parent) + os.pathsep + environment.get("PATH", "")
    guarded = sorted(set((root / "compiler/src").rglob("*.rs")) | set((root / "tools/sgc/src").rglob("*.rs")) | set((root / "tools/stdlib").glob("*.sg")) | set((root / "tools/stdlib").glob("runtime*.c")) | {Path(__file__).resolve(), root / "tools/stdlib/tests/discarded_owned_string.sg", root / "tools/stdlib/tests/native_copy_boundaries.sg", root / "tools/stdlib/tests/provider_resource_boundaries.sg", root / "tools/stdlib/tests/borrowed_fresh_owner.sg", root / "packages/sgllm/src/lib.sg"})
    guarded = sorted(set(guarded) | set((root / "tools/stdlib").glob("runtime*.h")))
    before = {str(path.relative_to(root)): sha(path) for path in guarded}
    report = {"schema": 1, "compiler": str(args.compiler.resolve()), "compiler_sha256": sha(args.compiler), "clang": str(args.clang.resolve()), "clang_sha256": sha(args.clang), "source_sha256": before, "steps": [], "passed": False}

    def run(label: str, command: list[str], expected: int = 0) -> str:
        completed = subprocess.run(command, cwd=root, env=environment, text=True, encoding="utf-8", errors="replace", stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=240)
        report["steps"].append({"label": label, "command": command, "exit_code": completed.returncode, "stdout": completed.stdout, "stderr": completed.stderr})
        output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(label, completed.returncode, flush=True)
        if completed.returncode != expected:
            raise RuntimeError(f"{label} failed: {completed.stdout[-3000:]} {completed.stderr[-3000:]}")
        return completed.stdout

    try:
        for level in (0, 2):
            exe = output.parent / f"discarded-owned-string-O{level}.exe"
            run(f"build-O{level}", [str(args.compiler), "--runtime-mode", "source-development", "build", "tools/stdlib/tests/discarded_owned_string.sg", f"-O{level}", "--force-rebuild", "-o", str(exe)])
            result = run(f"native-O{level}", [str(exe)])
            if result.splitlines()[1::2] != ["0"] * 6:
                raise RuntimeError("native resource deltas were not all zero")
        fixtures = [("owned-result", "tools/stdlib/tests/try_owned_result.sg"), ("discarded-document", "packages/sgmcp/tests/unused_owned.sg"), ("borrowed-close", "packages/sgmcp/tests/borrowed_close.sg"), ("fresh-reference", "tools/stdlib/tests/borrowed_fresh_owner.sg"), ("conditional-borrow", "tools/stdlib/tests/conditional_borrow_owner.sg")]
        for label, source in fixtures:
            exe = output.parent / f"{label}.exe"
            run(f"build-{label}", [str(args.compiler), "--runtime-mode", "source-development", "build", source, "-O0", "--force-rebuild", "-o", str(exe)])
            run(f"native-{label}", [str(exe)])
        for label, source, arguments in [("native-copy", "tools/stdlib/tests/native_copy_boundaries.sg", ["fixture"]), ("provider-boundaries", "tools/stdlib/tests/provider_resource_boundaries.sg", [])]:
            exe = output.parent / f"{label}.exe"
            run(f"build-{label}", [str(args.compiler), "--runtime-mode", "source-development", "build", source, "-O0", "--force-rebuild", "-o", str(exe)])
            run(f"native-{label}", [str(exe)] + arguments)
        run("emit-fixed-ir", [str(args.compiler), "--runtime-mode", "source-development", "build", "tools/stdlib/tests/discarded_owned_string.sg", "-O0", "--emit-llvm", "--force-rebuild", "-o", str(output.parent / "discarded-owned-string-fixed.ll")])
        after = {str(path.relative_to(root)): sha(path) for path in guarded}
        report["source_guard_passed"] = before == after
        if before != after:
            raise RuntimeError("compiler/runtime source changed during native verification")
        report["passed"] = True
    except Exception as exc:
        report["error"] = str(exc)
    output.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    sys.exit(main())
