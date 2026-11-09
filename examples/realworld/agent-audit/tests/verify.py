"""Synthetic provider acceptance; never reads user credentials or invokes tools."""
from __future__ import annotations
import argparse
import hashlib
import http.server
import json
import os
import pathlib
import subprocess
import tempfile
import threading
import time
import traceback


def remove_tree(root: pathlib.Path):
    entries = list(root.rglob("*"))
    assert all(p.resolve().is_relative_to(root.resolve()) for p in entries)
    for entry in entries:
        if entry.is_file(): entry.unlink()
    for entry in sorted((p for p in entries if p.is_dir()), key=lambda p: len(p.parts), reverse=True): entry.rmdir()
    root.rmdir()


class Peer(http.server.ThreadingHTTPServer):
    daemon_threads = True
    def __init__(self, mode, mutate):
        super().__init__(("127.0.0.1", 0), Handler)
        self.mode, self.mutate = mode, mutate
        self.lock = threading.Lock()
        self.initial = set()
        self.ready = threading.Event()
        self.calls = {}
        self.observed = []
        self.errors = []


class Handler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *args): pass
    def do_POST(self):
        try:
            data = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
            model = data["model"]
            assert self.headers.get("Authorization", "").endswith("fixture-secret") or self.headers.get("x-api-key") == "fixture-secret"
            with self.server.lock:
                turn = self.server.calls.get(model, 0)
                self.server.calls[model] = turn + 1
                self.server.observed.append({"model": model, "turn": turn, "path": self.path})
                if not turn:
                    self.server.initial.add(model)
                    if len(self.server.initial) == 2:
                        self.server.mutate.write_text("CHANGED AFTER CAPTURE", encoding="utf-8")
                        self.server.ready.set()
            assert self.server.ready.wait(5), "second reviewer was not started concurrently"
            if self.server.mode == "slow":
                time.sleep(1)
            if self.server.mode == "http500":
                self.send_response(500); self.send_header("Content-Length", "0"); self.end_headers(); return
            if self.server.mode == "malformed":
                body = b"data: {invalid json}\n\n"
            else:
                self.check_history(data, turn)
                calls = []
                text = ""
                if turn == 0:
                    calls = [("read-1", "read_file", {"path": "main.sg"}), ("denied-1", "read_file", {"path": "../private.txt"})]
                    if self.server.mode == "toolnul": calls += [("nul-read", "read_file", {"path": "main.sg\0suffix"}), ("nul-query", "search_files", {"query": "original\0suffix"})]
                elif turn == 1:
                    calls = [("search-1", "search_files", {"query": "original"})]
                    if self.server.mode == "conflict": calls = [("read-1", "read_file", {"path": "unselected.txt"})]
                else:
                    text = f"{model}: main.sg contains the original fixture."
                    if self.server.mode == "echo": text += " fixture-secret end"
                body = self.events(data, turn, calls, text)
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            # Intentionally split Unicode, JSON, SSE field names and arguments.
            for offset in range(0, len(body), 17):
                self.wfile.write(body[offset:offset+17]); self.wfile.flush()
                time.sleep(.001)
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError): pass
        except BaseException as error:
            self.server.errors.append(traceback.format_exc())
            try: self.send_error(500)
            except OSError: pass

    def check_history(self, data, turn):
        if not turn: return
        encoded = json.dumps(data, ensure_ascii=False)
        assert "original 中文😀" in encoded and "CHANGED AFTER CAPTURE" not in encoded
        assert "Read-only tool rejected (status 7)" in encoded
        if self.server.mode == "toolnul": assert encoded.count("Read-only tool rejected (status 10)") >= 2
        if turn > 1: assert "search-1" in encoded

    def events(self, data, turn, calls, text):
        def event(value): return "data: " + json.dumps(value, ensure_ascii=False) + "\n\n"
        if self.path.endswith("chat/completions"):
            delta = {"content": text} if text else {"tool_calls": [{"index": i, "id": id, "type": "function", "function": {"name": name, "arguments": json.dumps(args)}} for i, (id, name, args) in enumerate(calls)]}
            pieces = [{"content": text[:text.index("fixture-secret")+7]}, {"content": text[text.index("fixture-secret")+7:]}] if text and self.server.mode == "echo" else [delta]
            return ("".join(event({"choices": [{"delta": part, "finish_reason": None}]}) for part in pieces) + event({"choices": [{"delta": {}, "finish_reason": "tool_calls" if calls else "stop"}]}) + event({"choices": [], "usage": {"prompt_tokens": 21, "completion_tokens": 9}}) + "data: [DONE]\n\n").encode("utf-8")
        if self.path.endswith("responses"):
            output = [{"type": "function_call", "id": f"item-{turn}-{i}", "call_id": id, "name": name, "arguments": json.dumps(args)} for i, (id, name, args) in enumerate(calls)]
            if text: output = [{"type": "message", "id": f"message-{turn}", "role": "assistant", "content": [{"type": "output_text", "text": text}]}]
            frames = "".join(event({"type": "response.output_item.added", "output_index": i, "item": item}) for i, item in enumerate(output))
            response = {"id": "fixture-response", "status": "completed", "output": output, "usage": {"input_tokens": 21, "output_tokens": 9}}
            if self.server.mode == "continuation" and turn == 2: response["end_turn"] = False
            return (frames + event({"type": "response.completed", "response": response})).encode("utf-8")
        assert self.path.endswith("messages")
        frames = event({"type": "message_start", "message": {"usage": {"input_tokens": 21}}})
        blocks = [("tool_use", {"id": id, "name": name, "input": {}}) for id, name, args in calls] if calls else [("text", {"text": ""})]
        for i, (kind, fields) in enumerate(blocks):
            frames += event({"type": "content_block_start", "index": i, "content_block": {"type": kind, **fields}})
            delta = {"type": "input_json_delta", "partial_json": json.dumps(calls[i][2])} if calls else {"type": "text_delta", "text": text}
            frames += event({"type": "content_block_delta", "index": i, "delta": delta})
            frames += event({"type": "content_block_stop", "index": i})
        frames += event({"type": "message_delta", "delta": {"stop_reason": "tool_use" if calls else "end_turn"}, "usage": {"output_tokens": 9}})
        frames += event({"type": "message_stop"})
        return frames.encode("utf-8")


def scenario(exe, mode, protocols=("chatCompletions", "responses")):
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="audit-fixture-中文-"))
    root = scratch / "project"; root.mkdir()
    original = root / "main.sg"; original.write_text("original 中文😀", encoding="utf-8")
    if mode == "oversize": original.write_text("x" * 32769, encoding="utf-8")
    if mode == "invalid-utf8": original.write_bytes(b"\xff\xfeinvalid")
    (root / "unselected.txt").write_text("NEVER SENT", encoding="utf-8")
    peer = Peer(mode, original)
    thread = threading.Thread(target=peer.serve_forever, daemon=True); thread.start()
    try:
        base = f"http://127.0.0.1:{peer.server_address[1]}/v1"
        config = {"question": "Review code", "files": ["main.sg"], "deadline_ms": 150 if mode == "slow" else 20000, "targets": [{"label": label, "base": base, "protocol": protocol, "model": "model-" + label, "key_env": "SENGOO_AUDIT_FIXTURE_KEY"} for label, protocol in zip(("alpha", "beta"), protocols)]}
        if mode == "traversal": config["files"] = ["../private.txt"]
        if mode == "nul": config["files"] = ["main.sg\0unselected.txt"]
        path = scratch / "config.json"; path.write_text(json.dumps(config, ensure_ascii=False), encoding="utf-8")
        env = {k: v for k, v in os.environ.items() if not k.startswith(("SENGOO_", "SGPM_"))}
        env["SENGOO_AUDIT_FIXTURE_KEY"] = "fixture-secret"
        env["PATH"] = str(pathlib.Path(os.environ["SystemRoot"]) / "System32")
        result = subprocess.run([str(exe), str(root), str(path)], cwd=scratch, env=env, capture_output=True, encoding="utf-8", errors="replace", timeout=40)
        assert "fixture-secret" not in result.stdout + result.stderr
        assert "NEVER SENT" not in result.stdout + result.stderr
        assert "Resource ownership did not return to baseline" not in result.stdout, result.stdout
        events = [json.loads(line) for line in result.stdout.splitlines() if line.startswith("{")]
        assert not peer.errors, peer.errors
        if mode in ("normal", "toolnul", "echo", "continuation"):
            assert result.returncode == 0, result.stdout + result.stderr
            reports = [event for event in events if event["type"] == "report"]
            assert {item["reviewer"] for item in reports} == {"alpha", "beta"}
            assert all(item["model"] == "model-" + item["reviewer"] for item in reports)
            assert peer.calls == {"model-alpha": 3, "model-beta": 4 if mode == "continuation" else 3}, peer.calls
            assert len([event for event in events if event["type"] == "tool"]) == (10 if mode == "toolnul" else 6)
            if mode == "echo": assert "[REDACTED]" in result.stdout
        else:
            assert result.returncode != 0, result.stdout
            assert not any(event["type"] == "report" for event in events)
            if mode in ("traversal", "nul", "oversize", "invalid-utf8"): assert not peer.observed
            if mode == "http500": assert any("HTTP 500" in event["text"] for event in events)
            if mode == "malformed": assert any("HTTP 200" in event["text"] for event in events)
            if mode == "slow": assert any(event["type"] == "cancelled" for event in events)
        return {"mode": mode, "protocols": protocols, "exit_code": result.returncode, "events": len(events), "requests": peer.observed}
    finally:
        peer.shutdown(); peer.server_close(); thread.join(timeout=5); remove_tree(scratch)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("exe", type=pathlib.Path)
    parser.add_argument("--report", type=pathlib.Path, required=True)
    args = parser.parse_args(); exe = args.exe.resolve()
    records = []
    report = {"schema": 1, "result": "running", "scope": "synthetic local providers; no paid models or user credentials", "cases": records, "executable_sha256": hashlib.sha256(exe.read_bytes()).hexdigest()}
    try:
        records.append(scenario(exe, "normal"))
        records.append(scenario(exe, "normal", ("messages", "chatCompletions")))
        for mode in ("toolnul", "echo", "continuation"): records.append(scenario(exe, mode))
        for mode in ("http500", "malformed", "slow", "traversal", "nul", "conflict", "oversize", "invalid-utf8"): records.append(scenario(exe, mode))
        report["result"] = "passed"
    except BaseException as error:
        report.update(result="failed", error=repr(error)); raise
    finally:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"result": report["result"], "cases": len(records)}))


if __name__ == "__main__": main()
