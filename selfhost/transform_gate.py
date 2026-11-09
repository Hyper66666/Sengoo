"""Compare the SG expanded/formatted program AST with the Rust reference."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("frontend", "formatter", "oracle"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--extracted", type=Path)
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m2-transforms")
    args = parser.parse_args()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target"):
        raise ValueError("output must stay inside target/")
    out.mkdir(parents=True, exist_ok=True)
    tools = {name: getattr(args, name).resolve() for name in ("frontend", "formatter", "oracle")}
    report = {"schema": 1, "result": "running", "development_limit": args.limit,
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "tools": {name: {"path": str(path), "sha256": sha(path)} for name, path in tools.items()},
              "scope": "canonical reference AST rendering before/after SG expansion and formatting",
              "steps": [], "sources": {}}
    (out / "evidence.json").write_text(json.dumps(report), encoding="utf-8")

    def reference(sources):
        payload = "".join(json.dumps({"source": text, "parse": True, "format": True}) + "\n" for text in sources)
        result = subprocess.run([str(tools["oracle"])], input=payload, capture_output=True,
                                text=True, encoding="utf-8", cwd=ROOT, timeout=300)
        if result.returncode:
            raise RuntimeError("reference oracle failed")
        answers = [json.loads(line) for line in result.stdout.splitlines()]
        if len(answers) != len(sources):
            raise AssertionError("oracle row count mismatch")
        return answers

    try:
        for name in list(os.environ):
            # No external derive provider may silently alter the default corpus.
            if name.upper().startswith("SENGOO_DERIVE_"):
                raise ValueError("default transformation gate requires no external derive override")
        names = subprocess.check_output(["git", "ls-files", "-co", "--exclude-standard", "-z", "--", "*.sg"], cwd=ROOT).split(b"\0")
        paths = sorted({ROOT / name.decode("utf-8") for name in names if name and b"/build/" not in name
                        and not (name.startswith(b"selfhost/") and Path(name.decode("utf-8")).name.startswith(
                            ("semantic", "borrow", "mir")))})
        entries = [{"origin": str(path.relative_to(ROOT)), "source": path.read_bytes().decode("utf-8")}
                   for path in paths]
        report["sources"] = {str(path.relative_to(ROOT)): sha(path) for path in paths}
        for path in (ROOT / "selfhost").glob("*.py"):
            report["sources"][str(path.relative_to(ROOT))] = sha(path)
        for path in (ROOT / "selfhost/reference-exceptions.json", args.extracted):
            if path:
                report["sources"][str(Path(path).resolve().relative_to(ROOT))] = sha(path)
        if args.extracted:
            extracted = json.loads(args.extracted.read_text(encoding="utf-8"))
            if isinstance(extracted, dict):
                extracted = extracted["fixtures"]
            for entry in extracted:
                text = entry.get("source", entry.get("text"))
                entries.append({"origin": "literal:" + entry["sha256"], "source": text})
        if args.limit:
            entries = entries[:args.limit]
        answers = reference([entry["source"] for entry in entries])
        rows = []
        for index, (entry, expected) in enumerate(zip(entries, answers)):
            if not expected["parse"]["ok"] or not expected.get("format", {}).get("ok"):
                continue
            # Reviewed partials are separately required rejections with completed positives.
            exceptions = json.loads((ROOT / "selfhost/reference-exceptions.json").read_text())
            if hashlib.sha256(entry["source"].encode("utf-8")).hexdigest() in {
                    case["source_sha256"] for case in exceptions["cases"]}:
                continue
            prefix = f"case-{index:05d}"
            source, expanded = out / (prefix + ".sg"), out / (prefix + "-expanded.sg")
            source.write_bytes(entry["source"].encode("utf-8"))
            start = time.monotonic()
            done = subprocess.run([str(tools["frontend"]), str(source), str(expanded)], cwd=ROOT,
                                  capture_output=True, timeout=60)
            (out / (prefix + ".stdout.log")).write_bytes(done.stdout)
            (out / (prefix + ".stderr.log")).write_bytes(done.stderr)
            row = {"origin": entry["origin"], "source_sha256": sha(source), "frontend_exit": done.returncode,
                   "seconds": time.monotonic() - start}
            if done.returncode == 0 and expanded.is_file():
                text = expanded.read_bytes().decode("utf-8")
                actual = reference([text])[0]
                row["expanded_ast_equal"] = actual.get("format") == expected["format"]
                row["reference_expanded_parse"] = actual["parse"]
                formatted = subprocess.run([str(tools["formatter"]), str(source)], cwd=ROOT,
                                           capture_output=True, timeout=60)
                row["formatter_exit"] = formatted.returncode
                if formatted.returncode == 0:
                    (out / (prefix + "-formatted.sg")).write_bytes(formatted.stdout)
                    formatted_reference = reference([formatted.stdout.decode("utf-8")])[0]
                    row["formatted_ast_equal"] = formatted_reference.get("format") == expected["format"]
                    twice = out / (prefix + "-formatted.sg")
                    checked = subprocess.run([str(tools["formatter"]), "--check", str(twice)], cwd=ROOT,
                                             capture_output=True, timeout=60)
                    row["idempotent"] = checked.returncode == 0
            rows.append(row)
            print(f"{prefix}: {row['frontend_exit']}", flush=True)
        report["corpus"] = rows
        report["failures"] = [row for row in rows if row["frontend_exit"] != 0
                              or not row.get("expanded_ast_equal") or not row.get("formatted_ast_equal")
                              or not row.get("idempotent")]
        if report["failures"]:
            raise AssertionError(f"transformation differences: {len(report['failures'])}")
        report["result"] = "development-passed" if args.limit else "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [path for path, digest in report["sources"].items()
                   if not (ROOT / path).is_file() or sha(ROOT / path) != digest]
        report["changed_sources"] = changed
        if changed:
            report.update(result="failed", error="source changed during transformation gate")
        (out / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed:
            raise RuntimeError("transformation source changed")


if __name__ == "__main__":
    main()
