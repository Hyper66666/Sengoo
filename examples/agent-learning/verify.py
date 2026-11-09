"""Compile real few-shot consumers outside the checkout, then run without sources."""
from __future__ import annotations
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import threading
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def hashes(directory):
    return {str(path.relative_to(directory)): digest(path) for path in directory.rglob("*")
        if path.is_file() and not any(part in path.relative_to(directory).parts
            for part in ("target", "build", "__pycache__"))}


def runtime_hashes(language):
    directory = language / "tools/stdlib"
    return {str(path.relative_to(directory)): digest(path) for path in directory.rglob("*")
        if path.is_file() and path.suffix in (".sg", ".c", ".h")
        and not any(part in path.relative_to(directory).parts for part in ("tests", "target", "build"))}


def clean(directory):
    directory = directory.resolve(); paths = list(directory.rglob("*"))
    assert all(path.resolve().is_relative_to(directory) for path in paths)
    for path in paths:
        if path.is_file() or path.is_symlink(): path.unlink()
    for path in sorted((p for p in paths if p.is_dir()), key=lambda p: len(p.parts), reverse=True): path.rmdir()
    directory.rmdir()


class Model(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_): pass

    def do_POST(self):
        try:
            self.connection.settimeout(5)
            count = int(self.headers.get("Content-Length", "0")); assert 0 < count <= 65536
            request = json.loads(self.rfile.read(count))
            assert self.path == "/v1/responses" and request["model"] == "fixture"
            assert request["stream"] and self.headers.get("Authorization") == "Bearer fixture-key"
            events = [{"type": "response.output_text.delta", "delta": "Hello 世界"},
                {"type": "response.completed", "response": {"status": "completed",
                    "usage": {"input_tokens": 1, "output_tokens": 2}}}]
            wire = "".join("data: " + json.dumps(event, ensure_ascii=False) + "\n\n" for event in events).encode("utf-8")
            self.send_response(200); self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(wire))); self.end_headers()
            for offset in range(0, len(wire), 3):
                self.wfile.write(wire[offset:offset + 3]); self.wfile.flush(); time.sleep(.001)
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError): pass
        except BaseException as error: self.server.errors.append(repr(error))


def main():
    parser = argparse.ArgumentParser()
    for name in ("compiler", "clang", "formatter", "sgpm", "sengoo", "evidence"):
        parser.add_argument("--" + name, type=Path, required=True)
    args = parser.parse_args(); directory = Path(__file__).resolve().parent; language = args.sengoo.resolve()
    packages = ("sgschema", "sgllm", "sgsqlite", "sgagent")
    original = {name: hashes(language / "packages" / name) for name in packages}
    own = hashes(directory)
    runtime = runtime_hashes(language)
    tools = {name: {"path": str(getattr(args, name).resolve()), "sha256": digest(getattr(args, name))}
        for name in ("compiler", "clang", "formatter", "sgpm")}
    report = {"schema": 1, "reference": "sengoo-agent-reference/1", "result": "running",
        "scope": "copied, locked external consumers; loopback HTTP only; Windows native",
        "tools": tools, "package_sources": original, "example_sources": own,
        "runtime_sources": runtime, "steps": []}
    scratch = Path(tempfile.mkdtemp(prefix="agent-learning-世界-")); output = scratch / "bin"; output.mkdir()
    env = dict(os.environ); env.update(SENGOO_CLANG=str(args.clang.resolve()),
        SENGOO_STDLIB=str(language / "tools/stdlib"), SENGOO_RUNTIME=str(language / "tools/stdlib/runtime.c"),
        SGPM_SGC=str(args.compiler.resolve()))
    env["PATH"] = str(args.clang.resolve().parent) + os.pathsep + env.get("PATH", "")

    def run(name, command, cwd=scratch, expected=0, runtime=False):
        current = env
        if runtime:
            current = {key: value for key, value in os.environ.items() if not key.startswith("SENGOO_") and key != "SGPM_SGC"}
            current["PATH"] = str(Path(os.environ["SystemRoot"]) / "System32")
        began = time.monotonic()
        result = subprocess.run(list(map(str, command)), cwd=cwd, env=current, encoding="utf-8",
            errors="replace", capture_output=True, timeout=180)
        step = {"name": name, "exit_code": result.returncode, "stdout": result.stdout,
            "stderr": result.stderr, "seconds": time.monotonic() - began}
        report["steps"].append(step); print(name + ": " + str(result.returncode), flush=True)
        assert result.returncode == expected, step
        return result

    try:
        for name, files in original.items():
            for filename in files:
                target = scratch / "packages" / name / filename; target.parent.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(language / "packages" / name / filename, target)
        mapping = ";".join(name + "=" + str(scratch / "packages" / name / "src/lib.sg") for name in packages)
        binaries = {}
        for name in ("tool_schema", "llm_poll", "persist_action"):
            project = scratch / name; project.mkdir(); source = project / "main.sg"
            shutil.copyfile(directory / (name + ".sg"), source)
            manifest = '[package]\nname="' + name + '"\nversion="0.1.0"\nedition="2026"\n[bin]\npath="main.sg"\n[dependencies]\n'
            manifest += "".join(alias + '={path="../packages/' + alias + '"}\n' for alias in packages)
            (project / "Sengoo.toml").write_text(manifest, encoding="utf-8")
            run(name + "-format", [args.formatter.resolve(), "--check", source])
            env.pop("SENGOO_MODULE_MAP", None)
            run(name + "-update", [args.sgpm.resolve(), "update"], cwd=project)
            locked = (project / "Sengoo.lock").read_bytes()
            run(name + "-check-locked", [args.sgpm.resolve(), "--runtime-mode", "source-development", "check", "--locked"], cwd=project)
            env["SENGOO_MODULE_MAP"] = mapping
            binary = output / (name + ".exe"); binaries[name] = binary
            run(name + "-build", [args.compiler.resolve(), "--runtime-mode", "source-development", "build", source,
                "--force-rebuild", "-O", "0", "-o", binary], cwd=project)
            assert locked == (project / "Sengoo.lock").read_bytes(), "locked graph changed"
        repair = json.loads((directory / "repair.json").read_text(encoding="utf-8"))
        bad = scratch / "repair-before.sg"; bad.write_text(repair["before"], encoding="utf-8")
        result = run("json-diagnostic-before", [args.compiler.resolve(), "--runtime-mode", "source-development",
            "--error-format", "json", "check", bad], expected=1)
        diagnostics = []
        for channel in (result.stdout, result.stderr):
            for index, char in enumerate(channel):
                if char != "{": continue
                try: value, _ = json.JSONDecoder().raw_decode(channel[index:])
                except json.JSONDecodeError: continue
                if isinstance(value, dict) and value.get("kind") == "compile_error": diagnostics.append(value)
        assert any(all(value.get(key) == wanted for key, wanted in repair["expect"].items()) for value in diagnostics), diagnostics
        fixed = scratch / "repair-after.sg"; fixed.write_text(repair["after"], encoding="utf-8")
        repaired = output / "repaired.exe"
        run("json-diagnostic-repaired-check", [args.compiler.resolve(), "--runtime-mode", "source-development", "--error-format", "json", "check", fixed])
        run("json-diagnostic-repaired-build", [args.compiler.resolve(), "--runtime-mode", "source-development", "build", fixed, "--force-rebuild", "-O", "0", "-o", repaired])
        # No SDK source or consumer is available to the runtime proof.
        clean(scratch / "packages")
        for name in binaries: clean(scratch / name)
        bad.unlink(); fixed.unlink()
        accepted = run("schema-valid-json", [binaries["tool_schema"], '{"count":3}'], runtime=True)
        assert json.loads(accepted.stdout)["ok"] is True
        rejected = run("schema-rejected-json-exit2", [binaries["tool_schema"], '{"count":20}'], expected=2, runtime=True)
        assert json.loads(rejected.stdout)["pointer"] == "/count"
        invalid = run("schema-malformed-json-exit1", [binaries["tool_schema"], "{bad}"], expected=1, runtime=True)
        assert json.loads(invalid.stdout) == {"ok": False, "code": 10}
        database = scratch / "learning.sqlite3"
        first = run("persist-first-process", [binaries["persist_action"], database], runtime=True)
        second = run("persist-second-process", [binaries["persist_action"], database], runtime=True)
        assert json.loads(first.stdout)["type"] == json.loads(second.stdout)["type"] == "complete"
        assert json.loads(first.stdout)["fence"] == 1 and json.loads(second.stdout)["fence"] == 2
        key = scratch / "key.txt"; key.write_text("fixture-key\n", encoding="utf-8")
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Model); server.errors = []
        thread = threading.Thread(target=server.serve_forever, daemon=True); thread.start()
        try:
            base = "http://127.0.0.1:" + str(server.server_port) + "/v1"
            streamed = run("llm-real-fragmented-http", [binaries["llm_poll"], base, "fixture", key], runtime=True)
            assert streamed.stdout.replace("\r\n", "\n") == "Hello 世界\n1\n2\n", streamed.stdout
            cancelled = run("llm-real-cancel", [binaries["llm_poll"], base, "fixture", key, "--cancel"], expected=1, runtime=True)
            assert json.loads(cancelled.stdout) == {"ok": False, "code": 19}
        finally:
            server.shutdown(); server.server_close(); thread.join(timeout=5)
        assert not server.errors, server.errors
        repaired_result = run("repaired-native", [repaired], runtime=True)
        assert repaired_result.stdout.replace("\r\n", "\n") == repair["expected_stdout"]
        assert own == hashes(directory), "example source set changed"
        assert original == {name: hashes(language / "packages" / name) for name in packages}, "SDK source set changed"
        assert runtime == runtime_hashes(language), "runtime source set changed"
        assert all(info["sha256"] == digest(Path(info["path"])) for info in tools.values()), "selected tool changed"
        report["binary_sha256"] = {name: digest(binary) for name, binary in binaries.items()}
        report["result"] = "passed"
    except BaseException as error:
        report["result"] = "failed"; report["error"] = repr(error); raise
    finally:
        args.evidence.parent.mkdir(parents=True, exist_ok=True)
        args.evidence.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        clean(scratch)


if __name__ == "__main__": main()
