"""Native pipe/backpressure/cancellation and generated Sengoo writer gates.

Windows-only evidence. All child pipes are owned isolated fixtures. No real
console settings, credentials, service or user clipboard are inspected.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import runpy
import subprocess
import sys
import time


def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    for name in ("compiler", "clang", "formatter", "evidence"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[3]
    output = args.evidence.resolve(); output.parent.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    assert os.name == "nt", "This gate requires real Windows pipe cancellation; POSIX is unverified/unsupported"
    env = runpy.run_path(str(repo / "scripts/ownership-sanitizer-gate.py"))["windows_environment"]({k.upper(): v for k, v in env.items()})
    env.update(SENGOO_STDLIB=str(repo / "tools/stdlib"), SENGOO_RUNTIME=str(repo / "tools/stdlib/runtime.c"), SENGOO_CLANG=str(args.clang.resolve()))
    env["PATH"] = str(args.clang.resolve().parent) + os.pathsep + env.get("PATH", "")
    files = [repo / "tools/stdlib" / path for path in ("runtime.c", "runtime_string.c", "runtime_shared.h", "io.sg", "tests/std_writer_probe.c", "tests/std_writer.sg", "tests/verify_std_writer.py")]
    sources = {str(path.relative_to(repo)): digest(path) for path in files}
    report = {"schema": 1, "result": "running", "host": platform.platform(), "scope": "isolated real local Windows byte pipes; no POSIX or arbitrary device guarantee", "compiler_sha256": digest(args.compiler), "clang_sha256": digest(args.clang), "formatter_sha256": digest(args.formatter), "sources": sources, "steps": []}

    def run(name, command, **kwargs):
        start = time.monotonic()
        result = subprocess.run(list(map(str, command)), cwd=repo, env=env, capture_output=True, timeout=90, **kwargs)
        report["steps"].append({"name": name, "exit_code": result.returncode, "seconds": time.monotonic() - start})
        (output.parent / (name + ".log")).write_bytes(result.stdout + result.stderr)
        assert result.returncode == 0, (name, result.stdout[-1500:], result.stderr[-1500:])
        return result

    try:
        native = output.parent / "native-probe.exe"
        run("native-build", [args.clang, "-std=c11", "-O1", "-ffunction-sections", "-fdata-sections", repo / "tools/stdlib/tests/std_writer_probe.c", repo / "tools/stdlib/runtime_string.c", "-Wl,/OPT:REF", "-o", native])
        run("native-40-rounds", [native, "native"])
        for name, arg in (("blocked-reader", "blocked"), ("broken-reader", "broken")):
            start = time.monotonic()
            child = subprocess.Popen([str(native), arg], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
            try:
                assert child.stderr.readline().strip() == b"READY"
                if arg == "broken": child.stdout.close()
                child.stdin.write(b"c"); child.stdin.flush()
                # Do not read stdout until cancellation has actually completed.
                marker = child.stderr.readline().strip(); assert marker.startswith(b"CANCELED ") if arg == "blocked" else marker == b"BROKEN OK", marker
                if arg == "blocked": report["blocked_cancel_ms"] = int(marker.split()[1])
                child.stdin.close(); child.wait(timeout=5)
                assert child.returncode == 0, child.stderr.read()
                report["steps"].append({"name": name, "exit_code": 0, "seconds": time.monotonic() - start})
            finally:
                if child.poll() is None: child.kill(); child.wait()
                for pipe in (child.stdin, child.stdout, child.stderr):
                    if pipe: pipe.close()
        expected = bytes((i * 31 + i // 7) % 256 for i in range(8192))
        binary = run("native-binary", [native, "binary"])
        assert binary.stdout == expected and binary.stderr.strip() == b"BINARY OK"
        binary = run("native-stderr-binary", [native, "binary-stderr"])
        assert binary.stderr == expected and binary.stdout.strip() == b"BINARY OK"
        child = subprocess.Popen([str(native), "binary"], env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        collected = bytearray(); start = time.monotonic()
        try:
            while True:
                part = child.stdout.read(113)
                if not part: break
                collected.extend(part); time.sleep(0.001)
            child.wait(timeout=5)
            assert child.returncode == 0 and bytes(collected) == expected and child.stderr.read().strip() == b"BINARY OK"
            report["steps"].append({"name": "slow-reader-binary", "exit_code": 0, "seconds": time.monotonic() - start})
        finally:
            if child.poll() is None: child.kill(); child.wait()
            child.stdout.close(); child.stderr.close()
        for path in ("tools/stdlib/io.sg", "tools/stdlib/tests/std_writer.sg"):
            run("format-" + Path(path).stem, [args.formatter, "--check", repo / path])
        sg = output.parent / "std-writer.exe"
        run("sg-check", [args.compiler, "--runtime-mode", "source-development", "check", repo / "tools/stdlib/tests/std_writer.sg"])
        run("sg-build", [args.compiler, "--runtime-mode", "source-development", "build", repo / "tools/stdlib/tests/std_writer.sg", "--force-rebuild", "-O", "0", "-o", sg])
        generated = run("sg-binary", [sg])
        assert generated.stdout == expected and generated.stderr.strip() == b"SG BINARY OK"
        report["native_sha256"] = digest(native); report["generated_sha256"] = digest(sg)
        assert all(digest(repo / path) == value for path, value in sources.items()), "source changed during gate"
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error)); raise
    finally:
        output.write_text(json.dumps(report, indent=2), encoding="utf-8")


if __name__ == "__main__": main()
