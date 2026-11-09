"""Real external maintenance host, loopback model and durable SQLite restart."""
from __future__ import annotations
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import sqlite3
import subprocess
import tempfile
import threading
import queue
from contextlib import closing


class Model(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"
    def log_message(self, *_): pass

    def do_POST(self):
        try:
            assert self.path.endswith("/responses") and self.headers["Authorization"] == "Bearer fixture-key"
            body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            calls = {v["call_id"]: v for v in body["input"] if v.get("type") == "function_call"}
            results = {v["call_id"]: v for v in body["input"] if v.get("type") == "function_call_output"}
            assert calls.keys() == results.keys(), "host sent unpaired tool history"
            self.server.requests.append(body)
            names = {v["name"] for v in calls.values()}
            next_tool = next((name for name in ("read_fixture", "patch_fixture", "verify_fixture") if name not in names), None)
            events = []
            if next_tool:
                self.server.tools.append(next_tool)
                events = [{"type": "response.output_item.added", "output_index": 0,
                    "item": {"type": "function_call", "call_id": next_tool + "-once", "name": next_tool, "arguments": ""}},
                    {"type": "response.function_call_arguments.delta", "output_index": 0, "delta": "{}"}]
            else:
                assert json.loads(results["verify_fixture-once"]["output"])["content"] == "PASS", results["verify_fixture-once"]
                events = [{"type": "response.output_text.delta", "delta": "Verified maintenance complete 世界"}]
            events += [{"type": "response.completed", "response": {"status": "completed", "usage": {"input_tokens": 7, "output_tokens": 3}}}]
            data = "".join("data: " + json.dumps(v) + "\n\n" for v in events).encode()
            self.send_response(200); self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(data))); self.send_header("Connection", "close"); self.end_headers(); self.wfile.write(data)
        except BaseException as error:
            self.server.errors.append(repr(error)); self.send_error(500)
        finally: self.close_connection = True


def clean(root):
    root = root.resolve(); paths = list(root.rglob("*"))
    assert all(p.resolve().is_relative_to(root) for p in paths)
    for p in paths:
        if p.is_file() or p.is_symlink(): p.unlink()
    for p in sorted((p for p in paths if p.is_dir()), key=lambda p: len(p.parts), reverse=True): p.rmdir()
    root.rmdir()


def exercise(executable, crash=False, delayed_tree=False, timeout_tree=False):
    root = Path(tempfile.mkdtemp(prefix="sgagent-maintenance-世界-"))
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Model)
    server.requests = []; server.errors = []; server.tools = []
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        (root / "fixture.txt").write_text("before", encoding="utf-8")
        (root / "fixture-key.txt").write_text("fixture-key", encoding="utf-8")
        (root / "verify.cmd").write_text('@echo off\r\nset /p actual=<fixture.txt\r\nif "%actual%"=="after" (exit /b 0) else (exit /b 1)\r\n', encoding="ascii")
        if delayed_tree or timeout_tree:
            # The root exits while a real descendant remains in its Windows Job.
            # Root exit/reap is deliberately insufficient proof of quiescence.
            (root / "linger.cmd").write_text('@echo off\r\nC:\\Windows\\System32\\ping.exe -n ' + ('30' if timeout_tree else '2') + ' 127.0.0.1 >nul\r\necho done>tree-finished.txt\r\n', encoding="ascii")
            (root / "verify.cmd").write_text('@echo off\r\nset /p actual=<fixture.txt\r\nif not "%actual%"=="after" exit /b 1\r\nstart "" /b C:\\Windows\\System32\\cmd.exe /d /c linger.cmd\r\nexit /b 0\r\n', encoding="ascii")
        database = root / "agent.sqlite3"
        base = f"http://127.0.0.1:{server.server_port}/v1"
        arguments = [str(executable), str(database), base, str(root / "fixture-key.txt")]
        environment = {k: v for k, v in os.environ.items() if not k.startswith("SENGOO_")}
        environment["PATH"] = str(Path(os.environ["SystemRoot"]) / "System32")
        if crash:
            running = subprocess.Popen([*arguments, "crash-after-effect"], cwd=root, env=environment,
                stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, encoding="utf-8", errors="replace")
            lines = queue.Queue()
            threading.Thread(target=lambda: [lines.put(value) for value in running.stdout], daemon=True).start()
            try:
                seen = ""
                while "EFFECT_APPLIED_AWAITING_RESULT" not in seen: seen += lines.get(timeout=30)
                assert seen.replace("\r\n", "\n") == "EFFECT_APPLIED_AWAITING_RESULT\n", seen
                running.kill(); running.wait(timeout=10)
            finally:
                if running.poll() is None: running.kill(); running.wait(timeout=10)
            before = len(server.requests)
            restart = subprocess.run([*arguments, "continue"], cwd=root, env=environment, capture_output=True,
                encoding="utf-8", errors="replace", timeout=15)
            assert restart.returncode != 0 and len(server.requests) == before, (restart.stdout, restart.stderr)
            assert (root / "fixture.txt").read_text(encoding="utf-8") == "after"
            assert (root / "patch-count.txt").read_text(encoding="utf-8") == "once\n"
            with closing(sqlite3.connect(database.as_uri() + "?mode=ro", uri=True)) as db:
                snapshot = json.loads(db.execute("SELECT snapshot FROM sgagent_heads WHERE run_id='maintenance'").fetchone()[0])
            assert snapshot["cleanup_required"] is True and snapshot["phase"] == 5
            assert not server.errors, server.errors
            return {"result": "passed", "scope": "crash after file effect before result commit",
                "checks": ["crash-before-result-retains-claim", "ordinary-restart-blocks-before-provider",
                           "unknown-tool-is-not-replayed", "file-side-effect-count-remains-one"],
                "requests": len(server.requests), "provider_errors": server.errors}
        first = subprocess.run([*arguments, "pause-after-patch"], cwd=root, env=environment, capture_output=True,
            encoding="utf-8", errors="replace", timeout=30)
        assert first.returncode == 0 and "CHECKPOINT_AFTER_PATCH" in first.stdout, (first.stdout, first.stderr)
        assert first.stdout.replace("\r\n", "\n") == "CHECKPOINT_AFTER_PATCH\n", first.stdout
        assert (root / "fixture.txt").read_text(encoding="utf-8") == "after"
        assert (root / "patch-count.txt").read_text(encoding="utf-8") == "once\n"
        before = len(server.requests)
        second = subprocess.run([*arguments, "continue"], cwd=root, env=environment, capture_output=True,
            encoding="utf-8", errors="replace", timeout=30)
        if timeout_tree:
            assert second.returncode != 0 and "tool_outcome_unknown" in second.stdout, (second.stdout, second.stderr)
            assert len(server.requests) == before + 1 and not (root / "tree-finished.txt").exists()
            with closing(sqlite3.connect(database.as_uri() + "?mode=ro", uri=True)) as db:
                snapshot = json.loads(db.execute("SELECT snapshot FROM sgagent_heads WHERE run_id='maintenance'").fetchone()[0])
            assert snapshot["phase"] == 10 and snapshot["cleanup_required"] is False, snapshot
            assert snapshot["unresolved_call"] == "verify_fixture-once"
            calls = len(server.requests)
            retry = subprocess.run([*arguments, "continue"], cwd=root, env=environment, capture_output=True,
                encoding="utf-8", errors="replace", timeout=15)
            assert retry.returncode != 0 and len(server.requests) == calls
            assert (root / "patch-count.txt").read_text(encoding="utf-8") == "once\n"
            assert not server.errors, server.errors
            return {"result": "passed", "scope": "root exits but descendant exceeds original ten-second deadline",
                "checks": ["wait-for-actual-job-quiescence", "deadline-terminates-and-reaps-tree", "unknown-outcome-retained", "restart-does-not-replay"],
                "requests": calls, "provider_errors": server.errors}
        assert second.returncode == 0 and "MAINTENANCE_COMPLETE" in second.stdout, (second.stdout, second.stderr, server.errors, server.tools)
        assert second.stdout.replace("\r\n", "\n") == "Verified maintenance complete 世界\nMAINTENANCE_COMPLETE\n", second.stdout
        assert len(server.requests) > before and server.tools == ["read_fixture", "patch_fixture", "verify_fixture"]
        assert (root / "patch-count.txt").read_text(encoding="utf-8") == "once\n", "restart replayed a side effect"
        assert not server.errors, server.errors
        with closing(sqlite3.connect(database.as_uri() + "?mode=ro", uri=True)) as db:
            rows = db.execute("SELECT run_id,fence,revision,snapshot FROM sgagent_heads").fetchall()
        assert len(rows) == 1 and rows[0][0] == "maintenance" and rows[0][1] == 2
        snapshot = json.loads(rows[0][3]); assert snapshot["phase"] == 9 and snapshot["requests"] == 4 and snapshot["tokens"] == 40
        assert snapshot["cleanup_required"] is False and len(snapshot["ledger"]) == 3
        if delayed_tree: assert (root / "tree-finished.txt").read_text(encoding="ascii").strip() == "done"
        return {"result": "passed", "binary_sha256": hashlib.sha256(Path(executable).read_bytes()).hexdigest(),
                "scope": "outside-checkout real host, loopback model, real file/command/SQLite, System32-only PATH",
                "checks": ["read-exact-patch-real-command", "checkpoint-exit-and-restart", "no-side-effect-replay",
                           "fencing-and-cumulative-accounting", "no-compiler-or-source-runtime-on-runtime-path"],
                "requests": len(server.requests), "tool_order": server.tools, "provider_errors": server.errors,
                "first_stdout": first.stdout, "second_stdout": second.stdout, "delayed_tree": delayed_tree}
    finally:
        server.shutdown(); server.server_close(); clean(root)


def main():
    parser = argparse.ArgumentParser(); parser.add_argument("executable", type=Path); parser.add_argument("--evidence", type=Path)
    args = parser.parse_args(); result = exercise(args.executable.resolve()); result["crash"] = exercise(args.executable.resolve(), crash=True)
    result["delayed_tree"] = exercise(args.executable.resolve(), delayed_tree=True)
    result["timeout_tree"] = exercise(args.executable.resolve(), timeout_tree=True)
    print(json.dumps(result, ensure_ascii=False, indent=2))
    if args.evidence: args.evidence.parent.mkdir(parents=True, exist_ok=True); args.evidence.write_text(json.dumps(result, ensure_ascii=False, indent=2), encoding="utf-8")


if __name__ == "__main__": main()
