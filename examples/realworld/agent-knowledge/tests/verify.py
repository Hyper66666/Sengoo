"""Independent MCP consumer; all files, DBs, links and processes are owned fixtures."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
import pathlib
import queue
import sqlite3
import subprocess
import tempfile
import threading
import time


class Peer:
    def __init__(self, executable, root, database):
        self.process = subprocess.Popen([str(executable), str(root), str(database)], cwd=root,
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
            creationflags=subprocess.CREATE_NO_WINDOW if os.name == "nt" else 0)
        self.lines = queue.Queue(); self.errors = bytearray(); self.sequence = 0
        def output():
            for line in self.process.stdout:
                try: self.lines.put(json.loads(line))
                except BaseException as error: self.lines.put(error)
        def errors():
            self.errors.extend(self.process.stderr.read())
        self.output_thread = threading.Thread(target=output, daemon=True); self.output_thread.start()
        self.error_thread = threading.Thread(target=errors, daemon=True); self.error_thread.start()
        initialized = self.request("initialize", {"protocolVersion":"2025-11-25", "capabilities":{}, "clientInfo":{"name":"independent-knowledge-test","version":"1"}})
        assert initialized["result"]["protocolVersion"] == "2025-11-25"
        self.send({"jsonrpc":"2.0", "method":"notifications/initialized"})

    def send(self, value):
        self.process.stdin.write((json.dumps(value, ensure_ascii=False)+"\n").encode()); self.process.stdin.flush()

    def request(self, method, params):
        self.sequence += 1; identifier = f"fixture-{self.sequence}"
        self.send({"jsonrpc":"2.0", "id":identifier, "method":method, "params":params})
        deadline = time.monotonic()+10
        while time.monotonic() < deadline:
            try: response = self.lines.get(timeout=.1)
            except queue.Empty:
                assert self.process.poll() is None, ("server exited", self.process.returncode, bytes(self.errors))
                continue
            if isinstance(response, BaseException): raise response
            if response.get("id") == identifier: return response
        raise AssertionError(("request timed out", method, bytes(self.errors)))

    def tool(self, name, arguments):
        response = self.request("tools/call", {"name":name,"arguments":arguments})
        assert "result" in response, response
        return response["result"]

    def close(self):
        if self.process.poll() is None:
            self.process.stdin.close()
            try: self.process.wait(timeout=5)
            except subprocess.TimeoutExpired: self.process.kill(); self.process.wait(timeout=5)
        self.output_thread.join(timeout=2); self.error_thread.join(timeout=2)
        assert self.process.returncode == 0, (self.process.returncode, bytes(self.errors))


def main():
    parser = argparse.ArgumentParser(); parser.add_argument("executable", type=pathlib.Path); parser.add_argument("--report", type=pathlib.Path, required=True)
    args = parser.parse_args(); executable = args.executable.resolve(); report = args.report.resolve(); report.parent.mkdir(parents=True, exist_ok=True)
    scratch = pathlib.Path(tempfile.mkdtemp(prefix="knowledge-fixture-中文😀-")); root = scratch / "project";root.mkdir()
    outside = scratch / "project-neighbor";outside.mkdir();(outside/"private.txt").write_text("OUTSIDE-SECRET-FIXTURE",encoding="utf-8")
    (root/"notes.md").write_text("# 本地知识\nSengoo agent notes 😀 keyword-keep",encoding="utf-8")
    (root/"中文.txt").write_text("Unicode 查询 😀",encoding="utf-8")
    (root/"binary.txt").write_bytes(b"prefix\xffsuffix")
    (root/"large.txt").write_bytes(b"x"*65537)
    (root/"nul.txt").write_bytes(b"before\0after")
    (root/"ignored.bin").write_bytes(b"not indexed")
    nested=root/"nested";nested.mkdir();(nested/"source.sg").write_text("agent keyword-keep nested",encoding="utf-8")
    junction=root/"escape"; junction_created=False
    if os.name == "nt":
        linked=subprocess.run(["cmd.exe","/d","/c","mklink","/J",str(junction),str(outside)],capture_output=True)
        assert linked.returncode==0,(linked.stdout,linked.stderr);junction_created=True
    else:
        junction.symlink_to(outside,target_is_directory=True);junction_created=True
    database=scratch/"knowledge.sqlite";peer=None;checks=[]
    evidence={"schema":1,"result":"running","scope":"independent local MCP client; synthetic file root; Windows snapshot guards", "executable_sha256":hashlib.sha256(executable.read_bytes()).hexdigest(),"checks":checks}
    try:
        peer=Peer(executable,root,database)
        listed=peer.request("tools/list",{})["result"]["tools"]
        assert {x["name"] for x in listed}=={"knowledge_status","knowledge_reindex","knowledge_search","knowledge_read"}
        assert all(x["inputSchema"]["type"]=="object" for x in listed)
        initial=peer.tool("knowledge_status",{})["structuredContent"];assert initial["indexed"]==initial["generation"]==0
        checks.append("negotiation-tool-list-public-input-schemas-explicit-empty-start")
        invalid=peer.request("tools/call",{"name":"knowledge_search","arguments":{"query":3}});assert "error" in invalid
        unknown=peer.request("tools/call",{"name":"knowledge_status","arguments":{"root":str(outside)}});assert "error" in unknown
        checks.append("schema-rejects-wrong-types-and-model-root-override")
        stats=peer.tool("knowledge_reindex",{})["structuredContent"]
        assert stats["indexed"]==3 and stats["skipped"]>=5,stats
        matches=peer.tool("knowledge_search",{"query":"keyword-keep"})["structuredContent"]["matches"]
        assert {x["path"] for x in matches}=={"notes.md","nested/source.sg"},matches
        unicode=peer.tool("knowledge_search",{"query":"查询"})["structuredContent"]["matches"];assert unicode[0]["path"]=="中文.txt"
        read=peer.tool("knowledge_read",{"path":"中文.txt"})["structuredContent"];assert read["text"]=="Unicode 查询 😀"
        assert read["sha256"]==hashlib.sha256(read["text"].encode()).hexdigest()
        checks.append("atomic-explicit-index-query-unicode-digest-and-current-safe-read")
        for path in ("../project-neighbor/private.txt","/absolute.txt","C:/Windows/win.ini","nested/../../project-neighbor/private.txt","escape/private.txt","notes.md\0hidden","nested\\..\\notes.md"):
            result=peer.tool("knowledge_read",{"path":path});assert result.get("isError"),path
            assert "OUTSIDE-SECRET-FIXTURE" not in json.dumps(result),path
        assert peer.tool("knowledge_read",{"path":"large.txt"})["isError"]
        assert peer.tool("knowledge_read",{"path":"binary.txt"})["isError"]
        assert peer.tool("knowledge_read",{"path":"nul.txt"})["isError"]
        assert peer.tool("knowledge_search",{"query":"keyword\0hidden"})["isError"]
        assert peer.tool("knowledge_search",{"query":"OUTSIDE-SECRET-FIXTURE"})["structuredContent"]["matches"]==[]
        checks.append("traversal-sibling-prefix-reparse-nul-invalid-utf8-and-oversize-rejected")
        first_status=peer.tool("knowledge_status",{})["structuredContent"]
        connection=sqlite3.connect(database)
        try:
            connection.execute("CREATE TRIGGER fail_index BEFORE INSERT ON knowledge_files BEGIN SELECT RAISE(ABORT,'synthetic insert failure'); END");connection.commit()
        finally:connection.close()
        assert peer.tool("knowledge_reindex",{})["isError"]
        assert peer.tool("knowledge_status",{})["structuredContent"]==first_status
        assert len(peer.tool("knowledge_search",{"query":"keyword-keep"})["structuredContent"]["matches"])==2
        connection=sqlite3.connect(database)
        try:connection.execute("DROP TRIGGER fail_index");connection.commit()
        finally:connection.close()
        checks.append("failed-reindex-transaction-preserves-generation-and-complete-old-index")
        peer.close();peer=None
        (root/"notes.md").unlink()
        peer=Peer(executable,root,database)
        assert peer.tool("knowledge_status",{})["structuredContent"]==first_status
        persisted=peer.tool("knowledge_search",{"query":"keyword-keep"})["structuredContent"]["matches"];assert len(persisted)==2
        assert peer.tool("knowledge_read",{"path":"notes.md"})["isError"]
        refreshed=peer.tool("knowledge_reindex",{})["structuredContent"];assert refreshed["indexed"]==2
        latest=peer.tool("knowledge_search",{"query":"keyword-keep"})["structuredContent"]["matches"];assert len(latest)==1 and latest[0]["path"]=="nested/source.sg"
        checks.append("persistent-index-survives-process-restart-source-deletion-and-explicit-refresh")
        peer.close();peer=None
        connection=sqlite3.connect(database)
        try:
            assert connection.execute("select generation from knowledge_meta").fetchone()[0]==2
            assert connection.execute("select count(*) from knowledge_files").fetchone()[0]==2
        finally:connection.close()
        evidence["result"]="passed"
    except BaseException as error:
        evidence["result"]="failed";evidence["error"]=repr(error);raise
    finally:
        if peer is not None:
            if peer.process.poll() is None:peer.process.kill();peer.process.wait(timeout=5)
            peer.output_thread.join(timeout=2);peer.error_thread.join(timeout=2)
        if junction_created:os.rmdir(junction) if os.name=="nt" else junction.unlink()
        inventory=list(scratch.rglob("*"));assert all(p.resolve().is_relative_to(scratch.resolve()) for p in inventory)
        for p in inventory:
            if p.is_file():p.unlink()
        for p in sorted((p for p in inventory if p.is_dir()),key=lambda p:len(p.parts),reverse=True):p.rmdir()
        scratch.rmdir();evidence["temporary_fixture_removed"]=True
        report.write_text(json.dumps(evidence,ensure_ascii=False,indent=2),encoding="utf-8")
    print(json.dumps(evidence,ensure_ascii=False,indent=2))


if __name__=="__main__":main()
