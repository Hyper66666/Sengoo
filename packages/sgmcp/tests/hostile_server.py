"""Synthetic peers for native SDK tests. Contains no network or credentials."""
import json
import os
import sys
import time

mode = sys.argv[1]
calls = 0
for line in sys.stdin:
    message = json.loads(line)
    method = message.get("method")
    if method == "notifications/cancelled":
        raise SystemExit(0)
    if "id" not in message:
        continue
    identifier = message["id"]
    if method == "initialize":
        if mode == "init-timeout":
            time.sleep(3)
            continue
        value = {"protocolVersion": "unsupported" if mode == "bad-version" else "2024-11-05", "capabilities": {"tools": {}}, "serverInfo": {"name": "fixture", "version": "1"}}
        if mode == "nul-version": value["protocolVersion"] += "\0suffix"
        if mode == "nul-peer": value["serverInfo"]["name"] += "\0suffix"
        print(json.dumps({"jsonrpc": "2.0", "id": identifier, "result": value}), flush=True)
        continue
    calls += 1
    if mode == "progress-burst":
        for index in range(160):
            print(json.dumps({"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"burst","progress":index,"total":160}}))
        sys.stdout.flush()
    if mode == "stderr":
        sys.stderr.buffer.write(b"x" * 262144); sys.stderr.flush()
    elif mode == "rpc-error":
        print(json.dumps({"jsonrpc": "2.0", "id": identifier, "error": {"code": -32602, "message": "synthetic failure"}}), flush=True)
        continue
    elif mode == "oversize":
        sys.stdout.write("x" * 1048577); sys.stdout.flush(); time.sleep(3)
        continue
    elif mode == "wrong-id":
        identifier += 1
    elif mode == "nul-envelope":
        print(json.dumps({"jsonrpc":"2.0\0suffix","id":identifier,"result":{}}),flush=True)
        continue
    elif mode == "nul-key":
        print(json.dumps({"jsonrpc":"2.0","id":identifier,"result":{"content":[],"structuredContent":{"key\0suffix":"value"}}}),flush=True)
        continue
    elif mode in ("nul-tool", "nul-cursor"):
        value = {"tools":[{"name":"echo\0suffix" if mode == "nul-tool" else "echo"}]}
        if mode == "nul-cursor": value["nextCursor"] = "page\0suffix"
        print(json.dumps({"jsonrpc":"2.0","id":identifier,"result":value}),flush=True)
        continue
    elif mode == "empty-object":
        print("{}", flush=True)
        time.sleep(3)
        continue
    elif mode == "invalid-utf8":
        sys.stdout.buffer.write(b"\xff\n"); sys.stdout.flush(); time.sleep(3)
        continue
    elif mode == "crash":
        os._exit(7)
    elif mode == "cancel" and calls > 1:
        time.sleep(3)
        continue
    print(json.dumps({"jsonrpc": "2.0", "id": identifier, "result": {"content": [], "structuredContent": {}}}), flush=True)
