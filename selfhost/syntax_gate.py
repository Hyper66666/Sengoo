"""Differential tests for the SG syntax core; Rust is an explicit test oracle."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def sha(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("seed", "clang", "formatter", "oracle"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m2")
    parser.add_argument("--limit", type=int, default=0, help="development scan limit; zero scans all sources")
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target"):
        raise ValueError("output must be inside target/")
    out.mkdir(parents=True, exist_ok=True)
    tools = {name: getattr(args, name).resolve() for name in ("seed", "clang", "formatter", "oracle")}
    env = dict(os.environ)
    for name in list(env):
        if name.upper().startswith(("SENGOO_", "SGPLATFORM_", "SDL2_")):
            env.pop(name)
    env.update(SENGOO_STDLIB=str(ROOT / "tools/stdlib"), SENGOO_RUNTIME=str(ROOT / "tools/stdlib/runtime.c"),
               SENGOO_CLANG=str(tools["clang"]), SENGOO_MODULE_MAP=";".join(
                   "selfhost_" + name + "=" + str(ROOT / "selfhost/src" / (name + ".sg"))
                   for name in ("core", "lexer", "parser", "formatter", "derive", "expand", "protocols", "frontend")))
    env["PATH"] = str(tools["clang"].parent) + os.pathsep + env.get("PATH", "")
    report = {"schema": 1, "stage": "m2-syntax-pipeline", "transform_parity": "component-tests; canonical corpus comparison is in transform_gate.py",
              "result": "running", "host": platform.platform(), "steps": [],
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "tools": {name: {"path": str(path), "sha256": sha(path)} for name, path in tools.items()},
              "limits": ["syntax differential oracle uses current Rust reference; not a native self-build",
                         "syntax and expansion acceptance are not semantic compatibility or compiler self-hosting"],
              "development_limit": args.limit}
    (out / "evidence.json").write_text(json.dumps(report), encoding="utf-8")
    fingerprints = {}

    def m2_path(path):
        normalized = str(path).replace("\\", "/")
        name = Path(normalized).name
        return "/build/" not in normalized and not (
            normalized.startswith("selfhost/") and name.startswith(("semantic", "borrow", "mir")))

    def candidates():
        names = subprocess.check_output(["git", "ls-files", "-co", "--exclude-standard", "-z", "--", "*.sg"], cwd=ROOT)
        return sorted({ROOT / name.decode("utf-8") for name in names.split(b"\0") if name and m2_path(name.decode("utf-8"))})

    def run(name, command, expected=0, marker=None):
        started = time.monotonic()
        done = subprocess.run(list(map(str, command)), cwd=ROOT, env=env, capture_output=True,
                              encoding="utf-8", errors="replace", timeout=600)
        (out / (name + ".stdout.log")).write_text(done.stdout, encoding="utf-8")
        (out / (name + ".stderr.log")).write_text(done.stderr, encoding="utf-8")
        report["steps"].append({"name": name, "command": list(map(str, command)), "exit_code": done.returncode,
                                "seconds": time.monotonic() - started})
        if expected is not None and done.returncode != expected:
            raise RuntimeError(f"{name}: exit {done.returncode}; see {out}")
        if marker and marker not in done.stdout:
            raise RuntimeError(f"{name}: missing marker")
        print(f"{name}: {done.returncode}", flush=True)
        return done

    try:
        lock = json.loads((ROOT / "selfhost/bootstrap.lock").read_text())
        for name in ("seed", "clang", "formatter"):
            if report["tools"][name]["sha256"] != lock[name]["sha256"]:
                raise ValueError(f"unverified {name}")
        paths = candidates()
        relevant = {p for p in (ROOT / "selfhost").rglob("*")
                    if p.is_file() and not any(part in ("build", "__pycache__", "target") for part in p.parts)
                    and m2_path(p.relative_to(ROOT))}
        relevant.update(paths)
        fingerprints = {str(p.relative_to(ROOT)): sha(p) for p in relevant}
        for name in ("lexer", "parser", "formatter", "derive", "expand", "protocols", "frontend"):
            source = ROOT / "selfhost/tests" / (name + ".sg")
            binary = out / (name + "-tests.exe")
            run("format-test-" + name, [tools["formatter"], "--check", source])
            run("build-test-" + name, [tools["seed"], "--runtime-mode", "source-development", "build",
                                       source, "--force-rebuild", "-O", "0", "-o", binary])
            command = [binary, binary] if name == "derive" else [binary]
            marker = "selfhost derive:" if name == "derive" else "selfhost " + name + ": ok"
            run("native-test-" + name, command, marker=marker)
        programs = {}
        for name in ("syntax", "sgfmt", "expand", "frontend"):
            binary = out / (name + ".exe")
            run("build-" + name, [tools["seed"], "--runtime-mode", "source-development", "build",
                                  ROOT / "selfhost/tools" / (name + ".sg"), "--force-rebuild", "-O", "0", "-o", binary])
            programs[name] = binary
        if args.limit:
            paths = paths[:args.limit]
        # Preserve on-disk CRLF: spans are UTF-8 byte offsets, not Python lines.
        sources = [p.read_bytes().decode("utf-8") for p in paths]
        requests = "".join(json.dumps({"source": source, "parse": True}, ensure_ascii=False) + "\n" for source in sources)
        oracle = subprocess.run([str(tools["oracle"])], input=requests, text=True, capture_output=True,
                                encoding="utf-8", cwd=ROOT, timeout=300)
        if oracle.returncode != 0:
            raise RuntimeError("reference oracle failed")
        (out / "oracle.jsonl").write_text(oracle.stdout, encoding="utf-8")
        expected_results = [json.loads(line) for line in oracle.stdout.splitlines()]
        if len(expected_results) != len(paths):
            raise AssertionError("reference result count differs from corpus")
        kind_map = {spelling: int(value) for spelling, value in re.findall(
            r'if spelling == "([^"\\]*)"\s*\{\s*return (\d+);',
            (ROOT / "selfhost/src/lexer.sg").read_text(encoding="utf-8"))}
        literal_kinds = {"identifier": 1, "integer": 2, "float": 3, "invalid-number": 4,
                         "string": 5, "raw-string": 6, "bytes": 7, "char": 8, "fstring": 9}
        comparisons = []
        for index, (path, expected) in enumerate(zip(paths, expected_results)):
            name = f"corpus-{index:05d}"
            lexed = run(name + "-lex", [programs["syntax"], "lex", path], expected=None)
            values = [int(value) for value in lexed.stdout.splitlines()]
            row = {"path": str(path.relative_to(ROOT)), "reference_parse": expected["parse"],
                   "sg_lex_exit": lexed.returncode}
            expected_tokens = [(literal_kinds.get(token["kind"], kind_map.get(token["text"], -999)),
                                token["start"], token["end"]) for token in expected["tokens"]]
            actual_tokens = list(zip(values[::3], values[1::3], values[2::3])) if len(values) % 3 == 0 else []
            row["tokens_equal"] = lexed.returncode == 0 and actual_tokens == expected_tokens
            if not row["tokens_equal"]:
                row["token_first_difference"] = next((i for i, pair in enumerate(zip(actual_tokens, expected_tokens))
                                                      if pair[0] != pair[1]), min(len(actual_tokens), len(expected_tokens)))
            if expected["parse"]["ok"]:
                parsed = run(name + "-parse", [programs["syntax"], "parse", path], expected=None)
                row["sg_parse_exit"] = parsed.returncode
                row["sg_parse_output"] = parsed.stdout.strip()
            comparisons.append(row)
        report["corpus"] = comparisons
        (out / "corpus.json").write_text(json.dumps(comparisons, ensure_ascii=False, indent=2), encoding="utf-8")
        report["token_mismatches"] = sum(not row["tokens_equal"] for row in comparisons)
        report["valid_source_parse_failures"] = sum(row.get("sg_parse_exit", 0) != 0 for row in comparisons)
        if report["token_mismatches"] or report["valid_source_parse_failures"]:
            raise AssertionError(f"syntax corpus differs: tokens={report['token_mismatches']}, parse={report['valid_source_parse_failures']}")
        report["result"] = "development-passed" if args.limit else "passed"
    except BaseException as error:
        report.update(result="failed", error=str(error))
        raise
    finally:
        changed = []
        for path, digest in fingerprints.items():
            try:
                equal = sha(ROOT / path) == digest
            except OSError:
                equal = False
            if not equal:
                changed.append(path)
        report.update(sources=fingerprints, changed_sources=changed)
        changed_tools = []
        for name, identity in report["tools"].items():
            try:
                equal = sha(Path(identity["path"])) == identity["sha256"]
            except OSError:
                equal = False
            if not equal:
                changed_tools.append(name)
        report["changed_tools"] = changed_tools
        if changed:
            report.update(result="failed", error="syntax source changed during gate")
        if changed_tools:
            report.update(result="failed", error="syntax tool changed during gate")
        (out / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed or changed_tools:
            raise RuntimeError("syntax inputs changed")


if __name__ == "__main__":
    main()
