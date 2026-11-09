"""Extract Rust test string fixtures and compare the SG parser with its oracle.

This is test infrastructure, not an implementation of Sengoo tokenization,
parsing, macros or semantics. Extracted values are never interpolated or repaired.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import platform
import re
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SG_DECL = re.compile(r"\b(?:def|struct|enum|impl|trait|import)\b")


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def cooked_value(content: str) -> str:
    """Decode a Rust UTF-8 str literal, including continuation and Unicode."""
    out: list[str] = []
    i = 0
    escapes = {"n": "\n", "r": "\r", "t": "\t", "0": "\0", "\\": "\\", '"': '"', "'": "'"}
    while i < len(content):
        ch = content[i]
        if ch == "\r":
            if content[i:i + 2] != "\r\n":
                raise ValueError("bare CR in Rust string")
            out.append("\n")
            i += 2
            continue
        if ch != "\\":
            out.append(ch)
            i += 1
            continue
        i += 1
        if i >= len(content):
            raise ValueError("trailing escape")
        code = content[i]
        if code in escapes:
            out.append(escapes[code])
            i += 1
        elif code in "\r\n":
            if code == "\r":
                if content[i:i + 2] != "\r\n":
                    raise ValueError("invalid CR continuation")
                i += 1
            i += 1
            while i < len(content) and content[i] in " \t\n\r":
                i += 1
        elif code == "x":
            digits = content[i + 1:i + 3]
            if len(digits) != 2 or not re.fullmatch(r"[0-9a-fA-F]{2}", digits):
                raise ValueError("invalid hex escape")
            value = int(digits, 16)
            if value > 127:
                raise ValueError("non-ASCII Rust str hex escape")
            out.append(chr(value))
            i += 3
        elif code == "u" and content[i + 1:i + 2] == "{":
            end = content.find("}", i + 2)
            if end < 0:
                raise ValueError("unterminated Unicode escape")
            raw = content[i + 2:end]
            digits = raw.replace("_", "")
            if not digits or len(digits) > 6 or not re.fullmatch(r"[0-9a-fA-F_]+", raw):
                raise ValueError("invalid Unicode escape")
            value = int(digits, 16)
            if value > 0x10FFFF or 0xD800 <= value <= 0xDFFF:
                raise ValueError("invalid Unicode scalar")
            out.append(chr(value))
            i = end + 1
        else:
            raise ValueError(f"unsupported Rust escape {code!r}")
    return "".join(out)


def char_end(text: str, start: int) -> int | None:
    """Skip actual char literals, without swallowing lifetime syntax."""
    i = start + 1
    if i >= len(text):
        return None
    if text[i] == "\\":
        i += 1
        if text[i:i + 2] == "u{":
            end = text.find("}", i + 2)
            if end < 0:
                return None
            i = end + 1
        elif text[i:i + 1] == "x":
            i += 3
        else:
            i += 1
    else:
        i += 1
    return i + 1 if text[i:i + 1] == "'" else None


def rust_literals(text: str):
    """Yield exact source ranges and decoded ordinary/raw Rust str values."""
    i = 0
    while i < len(text):
        if text.startswith("//", i):
            end = text.find("\n", i + 2)
            i = len(text) if end < 0 else end + 1
            continue
        if text.startswith("/*", i):
            depth = 1
            i += 2
            while i < len(text) and depth:
                if text.startswith("/*", i):
                    depth += 1
                    i += 2
                elif text.startswith("*/", i):
                    depth -= 1
                    i += 2
                else:
                    i += 1
            continue
        if text[i] == "'":
            end = char_end(text, i)
            i = end if end is not None else i + 1
            continue
        if text[i:i + 2] in ("br", "cr"):
            cursor = i + 2
            while text[cursor:cursor + 1] == "#":
                cursor += 1
            if text[cursor:cursor + 1] == '"':
                terminator = '"' + text[i + 2:cursor]
                end = text.find(terminator, cursor + 1)
                if end < 0:
                    break
                i = end + len(terminator)
                continue
        # Byte/C strings are not Rust str values used as SG source fixtures.
        if text[i:i + 2] in ('b"', 'c"'):
            prefix = True
            start = i
            i += 1
        else:
            prefix = False
            start = i
        if text[i:i + 1] == "r" and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            cursor = i + 1
            while text[cursor:cursor + 1] == "#":
                cursor += 1
            if text[cursor:cursor + 1] == '"':
                terminator = '"' + text[i + 1:cursor]
                end = text.find(terminator, cursor + 1)
                if end < 0:
                    break
                finish = end + len(terminator)
                yield start, finish, "raw", text[cursor + 1:end].replace("\r\n", "\n"), None
                i = finish
                continue
        if text[i:i + 1] != '"':
            i += 1
            continue
        content_start = i + 1
        cursor = content_start
        while cursor < len(text):
            if text[cursor] == "\\":
                cursor += 2
            elif text[cursor] == '"':
                break
            else:
                cursor += 1
        if cursor >= len(text):
            break
        finish = cursor + 1
        if not prefix:
            try:
                value = cooked_value(text[content_start:cursor])
                error = None
            except ValueError as exc:
                value, error = "", str(exc)
            yield start, finish, "cooked", value, error
        i = finish


def extraction_checks() -> None:
    examples = [
        ('r#"def f() { "text" }"#', 'def f() { "text" }'),
        ('r###"def f() { r#"x"# }"###', 'def f() { r#"x"# }'),
        ('"def f() { \\"x\\" }"'.replace('\\\\"', '\\"'), 'def f() { "x" }'),
        ('"def f() { \\u{4e2d} \\x41 }"', 'def f() { 中 A }'),
        ('"def f() \\\n    { 1 }"', 'def f() { 1 }'),
        ('"def f() \\\r\n\t { 1 }"', 'def f() { 1 }'),
    ]
    for raw, expected in examples:
        values = list(rust_literals(raw))
        if len(values) != 1 or values[0][3] != expected or values[0][4]:
            raise AssertionError((raw, expected, values))
    noise = '// "def not_a_literal()"\n/* nested /* "struct X" */ */ let x = \'"\'; r##"enum E {}"##'
    values = list(rust_literals(noise))
    if [value[3] for value in values] != ['enum E {}']:
        raise AssertionError(values)


def delimiter_stack(tokens: list[dict]) -> list[str]:
    """Validate the exact reviewed exception's delimiter residue using oracle tokens."""
    stack: list[str] = []
    closing = {")": "(", "]": "[", "}": "{"}
    for token in tokens:
        text = token["text"]
        if text in ("(", "[", "{"):
            stack.append(text)
        elif text in closing:
            if not stack or stack.pop() != closing[text]:
                raise ValueError("reviewed incomplete fixture has mismatched delimiters")
    return stack


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oracle", type=Path, required=True)
    parser.add_argument("--parser", dest="sg_parser", type=Path, required=True)
    parser.add_argument("--output", type=Path, default=ROOT / "target/selfhost/m2-embedded")
    parser.add_argument("--limit", type=int, default=0)
    parser.add_argument("--scan-reference-rejected", action="store_true",
                        help="record reference-rejected literals for manual classification; not an automatic negative policy")
    args = parser.parse_args()
    extraction_checks()
    out = args.output.resolve()
    if not out.is_relative_to(ROOT / "target"):
        raise ValueError("output must remain in target/")
    out.mkdir(parents=True, exist_ok=True)
    fixture_dir = out / "fixtures"
    fixture_dir.mkdir(exist_ok=True)
    oracle, sg_parser = args.oracle.resolve(), args.sg_parser.resolve()
    tracked = subprocess.check_output(["git", "ls-files", "--", "*.rs"], cwd=ROOT, text=True).splitlines()
    paths = [ROOT / path for path in tracked if path.startswith(("compiler/", "tools/"))]
    paths.extend((ROOT / "selfhost/reference_fixtures.py", ROOT / "selfhost/reference-exceptions.json"))
    snapshots = {str(path.relative_to(ROOT)): digest(path.read_bytes()) for path in paths}
    candidates: dict[str, dict] = {}
    decode_errors = []
    for path in paths:
        if path.suffix != ".rs":
            continue
        data = path.read_bytes()
        text = data.decode("utf-8")
        for start, end, kind, source, error in rust_literals(text):
            literal = text[start:end].encode("utf-8")
            origin = {"file": str(path.relative_to(ROOT)), "offset": len(text[:start].encode("utf-8")),
                      "end": len(text[:end].encode("utf-8")), "literal_sha256": digest(literal), "kind": kind}
            if error:
                decode_errors.append({**origin, "error": error})
                continue
            if not SG_DECL.search(source):
                continue
            key = digest(source.encode("utf-8"))
            row = candidates.setdefault(key, {"sha256": key, "source": source, "origins": []})
            row["origins"].append(origin)
    values = list(candidates.values())
    if args.limit:
        values = values[:args.limit]
    exception_data = json.loads((ROOT / "selfhost/reference-exceptions.json").read_text(encoding="utf-8"))
    if exception_data["schema"] != 1:
        raise ValueError("unsupported reference exception schema")
    exceptions = {case["source_sha256"]: case for case in exception_data["cases"]}
    if len(exceptions) != len(exception_data["cases"]):
        raise ValueError("duplicate reference exception")
    extracted = {value["sha256"]: value for value in values}
    if not args.limit and exceptions.keys() - extracted.keys():
        raise ValueError("reviewed reference exception no longer appears in Rust literals")
    completions = []
    for key, case in exceptions.items():
        if key not in extracted:
            continue
        value = extracted[key]
        if value["origins"] != case["origins"]:
            raise ValueError(f"reference exception origin changed: {key}")
        completions.append({"sha256": key, "source": value["source"] + case["completion_suffix"]})
    (out / "extracted.json").write_text(json.dumps(values, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"Extracted {len(values)} unique Rust string values; decode errors: {len(decode_errors)}", flush=True)
    requests = "".join(json.dumps({"source": value["source"], "parse": True}, ensure_ascii=False) + "\n"
                       for value in [*values, *completions])
    oracle_hash, parser_hash = digest(oracle.read_bytes()), digest(sg_parser.read_bytes())
    started = time.monotonic()
    result = subprocess.run([str(oracle)], input=requests, cwd=ROOT, capture_output=True,
                            encoding="utf-8", errors="strict", timeout=600)
    (out / "oracle.stdout.jsonl").write_text(result.stdout, encoding="utf-8")
    (out / "oracle.stderr.log").write_text(result.stderr, encoding="utf-8")
    if result.returncode:
        raise RuntimeError("Rust fixture oracle failed")
    expected = [json.loads(line) for line in result.stdout.splitlines()]
    if len(expected) != len(values) + len(completions):
        raise RuntimeError("Rust fixture oracle response count mismatch")
    completed_reference = {value["sha256"]: response for value, response
                           in zip(completions, expected[len(values):])}
    comparisons = []
    negative_discovery = []
    for index, (value, reference) in enumerate(zip(values, expected[:len(values)])):
        case = exceptions.get(value["sha256"])
        if not reference["parse"]["ok"]:
            if case:
                raise ValueError("reviewed exception is no longer accepted by reference")
            if args.scan_reference_rejected:
                path = fixture_dir / f"{index:05d}-{value['sha256'][:12]}-reference-rejected.sg"
                path.write_bytes(value["source"].encode("utf-8"))
                try:
                    actual = subprocess.run([str(sg_parser), str(path)], cwd=ROOT, capture_output=True,
                                            encoding="utf-8", errors="replace", timeout=30)
                    negative_discovery.append({"sha256": value["sha256"], "source": value["source"],
                                               "origins": value["origins"], "fixture": str(path.relative_to(ROOT)),
                                               "reference_parse": reference["parse"], "exit_code": actual.returncode,
                                               "stdout": actual.stdout, "stderr": actual.stderr})
                except subprocess.TimeoutExpired:
                    negative_discovery.append({"sha256": value["sha256"], "source": value["source"],
                                               "origins": value["origins"], "fixture": str(path.relative_to(ROOT)),
                                               "reference_parse": reference["parse"], "exit_code": None, "timeout": True})
            continue
        source_path = fixture_dir / f"{index:05d}-{value['sha256'][:12]}.sg"
        source_path.write_bytes(value["source"].encode("utf-8"))
        try:
            run = subprocess.run([str(sg_parser), str(source_path)], cwd=ROOT, capture_output=True,
                                 encoding="utf-8", errors="replace", timeout=30)
            row = {"sha256": value["sha256"], "origins": value["origins"], "fixture": str(source_path.relative_to(ROOT)),
                   "reference_parse": reference["parse"], "exit_code": run.returncode,
                   "stdout": run.stdout, "stderr": run.stderr}
            if case:
                tail = run.stdout.splitlines()[-3:]
                at_eof = (len(tail) == 3 and tail[0] == str(case["required_error"])
                          and tail[1:] == [str(len(value["source"].encode("utf-8")))] * 2)
                residue = delimiter_stack(reference["tokens"])
                completed = next(item for item in completions if item["sha256"] == value["sha256"])
                complete_path = source_path.with_name(source_path.stem + "-completed.sg")
                complete_path.write_bytes(completed["source"].encode("utf-8"))
                complete_run = subprocess.run([str(sg_parser), str(complete_path)], cwd=ROOT, capture_output=True,
                                              encoding="utf-8", errors="replace", timeout=30)
                oracle_complete = completed_reference[value["sha256"]]["parse"]["ok"]
                passed = (run.returncode != 0 and at_eof and residue == case["unclosed_delimiters"]
                          and oracle_complete and complete_run.returncode == 0)
                row.update(explicit_rejection=True, exception_reason=case["reason"],
                           exception_assertion_passed=passed, unclosed_delimiters=residue,
                           completed_fixture=str(complete_path.relative_to(ROOT)),
                           completed_reference_accepts=oracle_complete,
                           completed_exit_code=complete_run.returncode,
                           completed_stdout=complete_run.stdout, completed_stderr=complete_run.stderr)
        except subprocess.TimeoutExpired:
            row = {"sha256": value["sha256"], "origins": value["origins"], "fixture": str(source_path.relative_to(ROOT)),
                   "reference_parse": reference["parse"], "exit_code": None, "timeout": True}
        comparisons.append(row)
        if len(comparisons) % 250 == 0:
            print(f"Compared {len(comparisons)} reference-positive fixtures", flush=True)
    changed = [path for path, sha in snapshots.items() if digest((ROOT / path).read_bytes()) != sha]
    tool_changed = oracle_hash != digest(oracle.read_bytes()) or parser_hash != digest(sg_parser.read_bytes())
    failures = sum(not row.get("exception_assertion_passed", False) if row.get("explicit_rejection")
                   else row["exit_code"] != 0 for row in comparisons)
    explicit = sum(row.get("explicit_rejection", False) and row.get("exception_assertion_passed", False)
                   for row in comparisons)
    expected_explicit = sum(value["sha256"] in exceptions for value in values)
    if explicit != expected_explicit:
        failures += 1
    report = {"schema": 1, "result": "failed" if failures or changed or tool_changed or decode_errors else
              ("development-passed" if args.limit else "passed"),
              "scope": "decoded Rust string fixtures accepted by reference parse; no template substitution",
              "revision": subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip(),
              "host": platform.platform(), "development_limit": args.limit,
              "tools": {"oracle": {"path": str(oracle), "sha256": oracle_hash},
                        "parser": {"path": str(sg_parser), "sha256": parser_hash}},
              "inputs": snapshots, "changed_sources": changed, "tool_changed": tool_changed,
              "extracted_unique": len(values), "reference_valid": len(comparisons), "parser_failures": failures,
              "explicit_rejections": explicit, "expected_explicit_rejections": expected_explicit,
              "negative_discovery_count": len(negative_discovery),
              "reference_rejected_sg_accepted": sum(row["exit_code"] == 0 for row in negative_discovery),
              "negative_discovery_is_analysis_only": True,
              "rust_literal_decode_errors": decode_errors, "seconds": time.monotonic() - started}
    (out / "corpus.json").write_text(json.dumps(comparisons, ensure_ascii=False, indent=2), encoding="utf-8")
    if args.scan_reference_rejected:
        (out / "negative-discovery.json").write_text(json.dumps(negative_discovery, ensure_ascii=False, indent=2), encoding="utf-8")
    (out / "evidence.json").write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(f"Rust literals: {len(values)}, reference-valid: {len(comparisons)}, "
          f"explicit rejections: {explicit}, unexpected failures: {failures}")
    if failures or changed or tool_changed or decode_errors:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
