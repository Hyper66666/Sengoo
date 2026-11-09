"""Exercise the SG formatter CLI against real files and pipe failures."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import platform
import subprocess

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--formatter", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m2-format-cli")
    args = parser.parse_args()
    binary, output = args.formatter.resolve(), args.output.resolve()
    if not output.is_relative_to(ROOT / "target"):
        raise ValueError("output must stay inside target/")
    output.mkdir(parents=True, exist_ok=True)
    sources = {str(p.relative_to(ROOT)): sha(p) for p in (
        ROOT / "selfhost/src/formatter.sg", ROOT / "selfhost/tools/sgfmt.sg",
        ROOT / "selfhost/src/core.sg", ROOT / "selfhost/src/parser.sg", ROOT / "selfhost/src/lexer.sg")}
    identity = sha(binary)
    report = {"schema": 1, "stage": "m2-formatter-cli-preview", "result": "running",
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "host": platform.platform(), "binary": {"path": str(binary), "sha256": identity},
              "sources": sources, "steps": [], "limits": ["full sgfmt configuration compatibility remains M7 work"]}

    def run(name, arguments, code=0, contains=None):
        command = [str(binary), *map(str, arguments)]
        done = subprocess.run(command, cwd=ROOT, capture_output=True, timeout=60)
        (output / (name + ".stdout.log")).write_bytes(done.stdout)
        (output / (name + ".stderr.log")).write_bytes(done.stderr)
        report["steps"].append({"name": name, "command": command, "exit_code": done.returncode,
                                "expected_exit_code": code})
        if done.returncode != code or contains and contains not in done.stdout + done.stderr:
            raise AssertionError(f"{name} failed: {done.returncode}")
        return done.stdout

    try:
        run("version", ["--version"], contains=b"SG syntax preview")
        run("help", ["--help"], contains=b"sgfmt")
        run("no-input", [], 64)
        run("bad-option", ["--unknown"], 64)
        fixture = output / "format source 世界 &.sg"
        fixture.write_text('// retain comment\ndef main()->i64{let x=1;/* keep 中 */x+2}', encoding="utf-8")
        original = sha(fixture)
        formatted = run("stdout", [fixture], contains=b"// retain comment")
        if sha(fixture) != original or "/* keep 中 */".encode() not in formatted:
            raise AssertionError("stdout formatting mutated source or dropped a comment")
        run("unformatted-check", ["--check", fixture], 1, b"needs_formatting")
        run("write", ["--write", fixture])
        if fixture.read_bytes() != formatted:
            raise AssertionError("write differs from stdout result")
        run("formatted-check", ["--check", fixture])
        if run("idempotent", [fixture]) != formatted:
            raise AssertionError("CLI formatter is not idempotent")
        custom = run("custom-layout", ["-i", "2", "-m", "20", fixture])
        if b"\n  let x" not in custom:
            raise AssertionError("custom indentation did not affect output")
        nul = output / "nul-literal.sg"
        nul.write_bytes(b'def main()->i64{let value="a\x00b";0}')
        nul_stdout = run("nul-stdout", [nul])
        if b'"a\x00b"' not in nul_stdout:
            raise AssertionError("NUL literal was truncated in stdout")
        run("nul-write", ["--write", nul])
        if nul.read_bytes() != nul_stdout:
            raise AssertionError("NUL write differs from stdout")
        run("nul-check", ["--check", nul])
        run("zero-width", ["-m", "0", fixture], 64)
        run("oversized-indent", ["-i", "65", fixture], 65)
        run("conflicting-actions", ["--check", "--write", fixture], 64)
        run("missing-file", [output / "never-created.sg"], 65)
        invalid = output / "invalid.sg"
        invalid.write_text("def invalid( {", encoding="utf-8")
        invalid_hash = sha(invalid)
        run("invalid-syntax", ["--write", invalid], 65)
        if sha(invalid) != invalid_hash:
            raise AssertionError("invalid syntax was overwritten")
        long_source = output / "large.sg"
        long_source.write_text("// " + "x" * 100000 + "\ndef main()->i64{0}", encoding="utf-8")
        process = subprocess.Popen([str(binary), str(long_source)], cwd=ROOT, stdout=subprocess.PIPE,
                                   stderr=subprocess.PIPE)
        process.stdout.close()
        error = process.stderr.read()
        code = process.wait(timeout=60)
        report["steps"].append({"name": "broken-stdout", "exit_code": code, "expected_exit_code": 65})
        (output / "broken-stdout.stderr.log").write_bytes(error)
        if code != 65:
            raise AssertionError("formatter did not propagate stdout failure")
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error))
        raise
    finally:
        changed = [path for path, digest in sources.items() if sha(ROOT / path) != digest]
        if changed or sha(binary) != identity:
            report.update(result="failed", error="formatter inputs changed")
        report["changed_sources"] = changed
        (output / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
        if changed:
            raise AssertionError("formatter inputs changed")


if __name__ == "__main__":
    main()
