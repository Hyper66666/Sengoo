"""Synthetic three-protocol byte-fragmentation and error peers for real sgllm."""
from __future__ import annotations
import http.server
import json
import subprocess
import threading
import time


class Provider(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_): pass

    def do_POST(self):
        try:
            length = int(self.headers.get("Content-Length", 0))
            assert 0 < length <= 1048576
            body = json.loads(self.rfile.read(length))
            assert self.headers.get("User-Agent") == "sgllm/0.1"
            anthropic = self.path.endswith("/messages")
            assert self.headers.get("x-api-key" if anthropic else "Authorization") == ("fixture-key" if anthropic else "Bearer fixture-key")
            assert self.headers.get("Content-Type") == "application/json" and body.get("stream")
            with self.server.lock:
                self.server.active += 1
                self.server.maximum_active = max(self.server.active, self.server.maximum_active)
                self.server.paths.append(self.path)
            try:
                if "/http500/" in self.path:
                    payload = b"SECRET remote error body"
                    self.send_response(500); self.send_header("Retry-After", "1")
                    self.send_header("Content-Length", str(len(payload))); self.end_headers(); self.wfile.write(payload); return
                self.send_response(200); self.send_header("Content-Type", "text/event-stream")
                self.send_header("Transfer-Encoding", "chunked"); self.send_header("Connection", "close"); self.end_headers()
                if "/malformed/" in self.path:
                    self.chunk(b"data: {SECRET invalid}\n\n"); self.chunk(b""); return
                if "/missing/" in self.path:
                    self.chunk(b'data: {"choices":[{"delta":{"content":"partial"},"finish_reason":null}]}\n\n'); self.chunk(b""); return
                if "/truncated/" in self.path:
                    self.chunk(b'data: {"choices":[{"delta":{"content":"partial"},"finish_reason":null}]}\n\n'); return
                if "/partial-sse/" in self.path:
                    self.chunk(b'data: {"choices":'); self.chunk(b""); return
                if "/slow/" in self.path:
                    self.chunk(b": waiting\n\n"); self.server.release.wait(4); return
                if self.path.endswith("/chat/completions"):
                    events = [
                        {"choices": [{"delta": {"content": "世界😀"}, "finish_reason": None}]},
                        {"choices": [{"delta": {"tool_calls": [{"index": 0, "id": "c1", "function": {"name": "read_file", "arguments": "{\"path\":"}}]}, "finish_reason": None}]},
                        {"choices": [{"delta": {"tool_calls": [{"index": 0, "function": {"arguments": "\"test.sg\"}"}}]}, "finish_reason": "tool_calls"}], "usage": {"prompt_tokens": 17, "completion_tokens": 8}}, "[DONE]"]
                elif self.path.endswith("/responses"):
                    events = [{"type": "response.output_text.delta", "delta": "世界😀"},
                        {"type": "response.output_item.added", "output_index": 1, "item": {"type": "function_call", "call_id": "r1", "name": "read_file", "arguments": ""}},
                        {"type": "response.function_call_arguments.delta", "output_index": 1, "delta": "{\"path\":\"test.sg\"}"},
                        {"type": "response.completed", "response": {"status": "completed", "usage": {"input_tokens": 17, "output_tokens": 8}}}]
                else:
                    assert anthropic
                    events = [{"type": "message_start", "message": {"usage": {"input_tokens": 17}}},
                        {"type": "content_block_delta", "index": 0, "delta": {"type": "text_delta", "text": "世界😀"}},
                        {"type": "content_block_start", "index": 1, "content_block": {"type": "tool_use", "id": "a1", "name": "read_file", "input": {}}},
                        {"type": "content_block_delta", "index": 1, "delta": {"type": "input_json_delta", "partial_json": "{\"path\":\"test.sg\"}"}},
                        {"type": "message_delta", "delta": {"stop_reason": "tool_use"}, "usage": {"output_tokens": 8}}, {"type": "message_stop"}]
                for index, event in enumerate(events):
                    data = event if isinstance(event, str) else json.dumps(event, ensure_ascii=index % 2 == 0)
                    for byte in ("data: " + data + "\r\n\r\n").encode(): self.chunk(bytes([byte]))
                    time.sleep(.025)
                self.chunk(b"")
            finally:
                with self.server.lock: self.server.active -= 1
        except (BrokenPipeError, ConnectionResetError, ConnectionAbortedError): pass
        except BaseException as error: self.server.errors.append(repr(error))
        finally: self.close_connection = True

    def chunk(self, payload):
        self.wfile.write(f"{len(payload):x}\r\n".encode() + payload + b"\r\n"); self.wfile.flush()


def exercise(executable, failures=False, extra_arguments=(), minimum_active=3):
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Provider)
    server.errors = []; server.paths = []; server.active = 0; server.maximum_active = 0
    server.lock = threading.Lock(); server.release = threading.Event()
    threading.Thread(target=server.serve_forever, daemon=True).start()
    try:
        result = subprocess.run([str(executable), f"http://127.0.0.1:{server.server_port}/v1", *extra_arguments], capture_output=True,
            text=True, encoding="utf-8", errors="replace", timeout=40)
        assert result.returncode == 0, (result.returncode, result.stdout, result.stderr)
        assert not server.errors, server.errors
        if not failures: assert server.maximum_active >= minimum_active, server.maximum_active
        if failures:
            for path in ("http500", "malformed", "missing", "truncated", "partial-sse"):
                assert sum(f"/{path}/" in value for value in server.paths) == 1, "SDK silently retried a request"
        return {"exit_code": result.returncode, "stdout": result.stdout, "stderr": result.stderr,
                "maximum_active_requests": server.maximum_active, "requests": len(server.paths), "provider_errors": server.errors}
    finally:
        server.release.set(); server.shutdown(); server.server_close()
