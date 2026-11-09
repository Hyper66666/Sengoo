"""Build copied SDK consumers outside the checkout; use real stdio peers."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import tempfile
import threading
import time
import traceback

from backpressure import verify as verify_backpressure


def digest(path): return hashlib.sha256(path.read_bytes()).hexdigest()


def clean(root):
    paths = list(root.rglob("*"))
    for path in paths:
        if not path.resolve().is_relative_to(root.resolve()): raise RuntimeError("cleanup escaped owned directory")
    for path in paths:
        if path.is_file() or path.is_symlink(): path.unlink()
    for path in sorted((p for p in paths if p.is_dir()), key=lambda p: len(p.parts), reverse=True): path.rmdir()
    root.rmdir()


def independent_client(server):
    process = subprocess.Popen([str(server)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    incoming = queue.Queue()
    def reader():
        for line in process.stdout:
            try: incoming.put(json.loads(line.decode("utf-8")))
            except BaseException as exc: incoming.put(exc)
    threading.Thread(target=reader, daemon=True).start()
    def send(value, fragmented=False):
        data = (json.dumps(value, ensure_ascii=False) + "\n").encode()
        if fragmented:
            for offset in range(0, len(data), 3): process.stdin.write(data[offset:offset+3]); process.stdin.flush()
        else: process.stdin.write(data); process.stdin.flush()
    def receive():
        value = incoming.get(timeout=10)
        if isinstance(value, BaseException): raise value
        return value
    try:
        send({"jsonrpc":"2.0","id":"init","method":"initialize","params":{"protocolVersion":"2025-11-25","capabilities":{},"clientInfo":{"name":"independent-python","version":"1"}}})
        assert receive()["result"]["protocolVersion"] == "2025-11-25"
        send({"jsonrpc":"2.0","method":"notifications/initialized"})
        send({"jsonrpc":"2.0","id":1,"method":"tools/list"})
        page = receive()["result"]; assert len(page["tools"]) == 1 and page["nextCursor"] == "1"
        send({"jsonrpc":"2.0","id":2,"method":"tools/list","params":{"cursor":page["nextCursor"]}})
        assert receive()["result"]["tools"][0]["name"] == "slow"
        send({"jsonrpc":"2.0","id":"opaque-中文😀","method":"tools/call","params":{"name":"echo","arguments":{"message":"split Unicode 中文😀"},"_meta":{"progressToken":"token-中文"}}}, True)
        progress = receive(); result = receive()
        assert progress["method"] == "notifications/progress" and progress["params"]["progressToken"] == "token-中文"
        assert result["id"] == "opaque-中文😀" and result["result"]["structuredContent"] == {"message":"split Unicode 中文😀"}
        send({"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"echo","arguments":{"message":False}}})
        assert receive()["error"]["code"] == -32602
        send({"jsonrpc":"2.0","id":4,"method":"resources/list"})
        assert receive()["error"]["code"] == -32601
        send({"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"echo\0suffix","arguments":{"message":"not dispatched"}}})
        assert receive()["error"]["code"] == -32602
        send({"jsonrpc":"2.0","id":"opaque\0suffix","method":"tools/call","params":{"name":"echo","arguments":{"message":"A\0B"}}})
        value=receive()
        assert value["id"] == "opaque\0suffix" and value["result"]["structuredContent"]["message"] == "A\0B" and value["result"]["content"][0]["text"] == "A\0B"
        send({"jsonrpc":"2.0","id":"slow-call","method":"tools/call","params":{"name":"slow","arguments":{"message":"wait"}}})
        send({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":"slow-call"}})
        process.wait(timeout=10)
        assert process.returncode == 0 and not process.stdout.read() and not process.stderr.read()
    finally:
        if process.poll() is None: process.kill(); process.wait(timeout=5)
        for pipe in (process.stdin,process.stdout,process.stderr): pipe.close()


def main():
    parser=argparse.ArgumentParser(); parser.add_argument("--compiler",type=Path,required=True)
    parser.add_argument("--clang",type=Path,required=True); parser.add_argument("--formatter",type=Path,required=True); parser.add_argument("--output",type=Path,required=True)
    args=parser.parse_args(); package=Path(__file__).resolve().parents[1]; root=package.parents[1]
    compiler=args.compiler.resolve(strict=True); clang=args.clang.resolve(strict=True)
    formatter=args.formatter.resolve(strict=True)
    scratch=Path(tempfile.mkdtemp(prefix="sgmcp-consumer-世界-")); report={"schema":1,"result":"running","scope":"copied-outside-checkout-native-consumers-and-independent-stdio-fixtures","host":os.environ.get("COMPUTERNAME"),"platform":sys.platform,"compiler_sha256":digest(compiler),"clang_sha256":digest(clang),"revision":subprocess.check_output(["git","rev-parse","HEAD"],cwd=root,text=True).strip(),"checks":[],"steps":[]}
    inputs=[p for name in ("sgmcp","sgschema") for p in (root/"packages"/name).rglob("*") if p.is_file() and p.suffix in (".sg",".py",".toml") and "build" not in p.parts and "__pycache__" not in p.parts]
    hashes={str(p):digest(p) for p in inputs}; success=False
    report["formatter_sha256"]=digest(formatter)
    foundation=[p for p in (root/"tools/stdlib").iterdir() if p.is_file() and p.suffix in (".sg",".c",".h")]
    foundation += [p for p in (root/"runtime/src").rglob("*.rs") if p.is_file()]
    foundation += [root/"runtime/Cargo.toml",root/"Cargo.lock"]
    foundation_hashes={str(p):digest(p) for p in foundation}
    report["foundation_sources"]=foundation_hashes
    def run(name, command, timeout=300):
        began=time.monotonic(); completed=subprocess.run(command,cwd=scratch,env=environment,capture_output=True,text=True,encoding="utf-8",errors="replace",timeout=timeout)
        report["steps"].append({"name":name,"exit_code":completed.returncode,"seconds":time.monotonic()-began,"stdout":completed.stdout,"stderr":completed.stderr})
        if completed.returncode: raise RuntimeError(name+" failed:\n"+completed.stdout+completed.stderr)
        return completed
    try:
        for name in ("sgmcp","sgschema"):
            destination=scratch/name; destination.mkdir()
            for source in (root/"packages"/name).rglob("*"):
                if source.is_file() and source.suffix in (".sg",".py",".toml") and "build" not in source.parts and "__pycache__" not in source.parts:
                    target=destination/source.relative_to(root/"packages"/name); target.parent.mkdir(parents=True,exist_ok=True); shutil.copyfile(source,target)
        environment=dict(os.environ); environment.update(SENGOO_STDLIB=str(root/"tools/stdlib"),SENGOO_RUNTIME=str(root/"tools/stdlib/runtime.c"),SENGOO_MODULE_MAP=f"sgmcp={scratch/'sgmcp/src/lib.sg'};sgschema={scratch/'sgschema/src/lib.sg'}",SENGOO_CLANG=str(clang))
        environment["PATH"]=str(clang.parent)+os.pathsep+environment.get("PATH","")
        for source in inputs:
            if source.suffix == ".sg": run("format-"+str(source.relative_to(root)),[str(formatter),"--check",str(source)],30)
        suffix=".exe" if os.name=="nt" else ""; binaries={}
        for name,relative in (("protocol","tests/protocol.sg"),("server","examples/echo-server.sg"),("client","examples/echo-client.sg"),("client-tests","tests/client.sg"),("borrowed-close","tests/borrowed_close.sg"),("unused-owned","tests/unused_owned.sg"),("formatter-return","tests/formatter_return.sg"),("backpressure","tests/backpressure.sg")):
            binary=scratch/(name+suffix); binaries[name]=binary
            run(name+"-build",[str(compiler),"--runtime-mode","source-development","build",str(scratch/"sgmcp"/relative),"--force-rebuild","-O","0","-o",str(binary)])
            report.setdefault("binaries",{})[name]={"path":str(binary),"sha256":digest(binary)}
        run("protocol-native",[str(binaries["protocol"])],60)
        run("borrowed-close-scope-cleanup",[str(binaries["borrowed-close"])],30)
        run("unused-owned-statement-cleanup",[str(binaries["unused-owned"])],30)
        run("formatted-self-return-semantics",[str(binaries["formatter-return"])],30)
        example=run("native-example-pair",[str(binaries["client"]),str(binaries["server"])],30)
        assert "Hello 中文😀" in example.stdout
        run("client-real-process-and-hostile-peers",[str(binaries["client-tests"]),str(binaries["server"]),sys.executable,str(scratch/"sgmcp/tests/hostile_server.py")],90)
        independent_client(binaries["server"])
        report["blocked_output_cases"] = verify_backpressure(binaries["backpressure"])
        report["checks"].append("blocked-server-output-concurrent-input-cancel-fixed-deadline-and-exact-resources")
        report["checks"] += ["copied-sdk-builds-outside-checkout","native-byte-framing-and-lifecycle","native-client-and-server-paging-structured-output","independent-python-client-split-utf8-progress-schema-cancellation","hostile-server-stderr-oversize-timeout-crash-cancel-rpc-errors","failed-result-repeated-json-and-process-resource-baselines"]
        for path,sha in hashes.items():
            if digest(Path(path)) != sha: raise RuntimeError("SDK source changed during verification: "+path)
        for path,sha in foundation_hashes.items():
            if digest(Path(path)) != sha: raise RuntimeError("Foundation source changed during verification: "+path)
        report["source_hashes"]=hashes; report["result"]="passed"; success=True
    except BaseException:
        report["result"]="failed"; report["error"]=traceback.format_exc(); report["retained_fixture"]=str(scratch)
    finally:
        report["finished_at"]=time.time(); args.output.parent.mkdir(parents=True,exist_ok=True)
        args.output.write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding="utf-8")
        if success: clean(scratch)
    print(json.dumps({"result":report["result"],"checks":report["checks"],"evidence":str(args.output),"error":report.get("error")},ensure_ascii=False,indent=2))
    return 0 if success else 1


if __name__=="__main__": raise SystemExit(main())
