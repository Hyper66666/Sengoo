"""Prove incremental baseline validation rejects an actual stale executable."""
from __future__ import annotations

import argparse
import json
import os
from pathlib import Path
import platform
import runpy

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--reference-qualification", type=Path, required=True)
    parser.add_argument("--clang", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m0-stale-negative")
    args = parser.parse_args()
    output = args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("outputs must be inside target/")
    output.mkdir(parents=True, exist_ok=True)
    namespace = runpy.run_path(str(ROOT / "selfhost/baseline.py"))
    qualification = json.loads(args.reference_qualification.read_text(encoding="utf-8"))
    if qualification["result"] != "passed" or qualification["profile"] != "release":
        raise ValueError("qualified release reference required")
    tools = qualification["tools"]
    sha = namespace["sha"]
    for name, tool in tools.items():
        if sha(Path(tool["path"])) != tool["sha256"]:
            raise ValueError(f"reference tool changed: {name}")
    config = json.loads((ROOT / "selfhost/baseline-workloads.json").read_text())
    initial = namespace["inventory"](config)
    initial["selfhost/test_baseline.py"] = sha(Path(__file__))
    clang = args.clang.resolve()
    lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
    if sha(clang) != lock["clang"]["sha256"]:
        raise ValueError("clang mismatch")
    env = dict(os.environ)
    env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"),
               SENGOO_CLANG=str(clang))
    env["PATH"] = str(clang.parent) + os.pathsep + env.get("PATH", "")
    report = {"schema": 1, "result": "running", "host": platform.platform(), "raw": [], "steps": [],
              "sources": initial, "qualification_sha256": sha(args.reference_qualification), "tools": tools}
    try:
        runner = namespace["Runner"](output, tools, env, config, "reference", report)
        runner.prepare()
        original = sha(runner.binary)
        library = runner.project / "library.sg"
        library.write_text(library.read_text().replace("    7;", "    8;"), encoding="utf-8")
        # Deliberately omit recompilation, simulating an incorrect cache hit.
        rejected = False
        try:
            runner.execute("stale-value-8", [runner.binary], b"selfhost baseline runtime: ok\n8\n")
        except RuntimeError:
            rejected = True
        if not rejected:
            raise AssertionError("stale revision-7 executable was accepted as revision 8")
        stale_record = report["raw"][-1]
        stale_output = (runner.logs / "stale-value-8.stdout.log").read_bytes().replace(b"\r\n", b"\n")
        if stale_record["name"] != "stale-value-8" or stale_record["exit_code"] != 0 or stale_output != b"selfhost baseline runtime: ok\n7\n" or sha(runner.binary) != original:
            raise AssertionError("negative must reject the actual unchanged successful revision-7 executable")
        report["steps"].append({"name": "stale-executable-rejected", "passed": True,
                                "unchanged_binary_sha256": original})
        runner.execute("fresh-build", runner.sgc("build", runner.project / "main.sg", "-O", "2", "-o", runner.binary))
        runner.execute("fresh-value-8", [runner.binary], b"selfhost baseline runtime: ok\n8\n")
        report["steps"].append({"name": "new-executable-produces-8", "passed": True,
                                "binary_sha256": sha(runner.binary)})
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        ending = namespace["inventory"](config)
        ending["selfhost/test_baseline.py"] = sha(Path(__file__))
        changed = ending != initial
        report["inputs_changed"] = changed
        if changed:
            report.update(result="failed", error="inputs changed")
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed:
            raise AssertionError("baseline inputs changed")


if __name__ == "__main__":
    main()
