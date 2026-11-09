"""Measure a real sgllm consumer with local synthetic streams, never paid APIs.

Cold means a new external project without its Sengoo build cache. It does not
flush OS caches or erase global runtime/toolchain artifacts. Native byte-queue
high-water is unavailable in the current public counters and stays null.
"""
from __future__ import annotations
import argparse
import ctypes
from ctypes import wintypes
import hashlib
import http.server
import json
import math
import os
import platform
from pathlib import Path
import re
import shutil
import statistics
import subprocess
import tempfile
import threading
import time

from package_ecosystem import bundle_name, unpack, verify


def sha(path): return hashlib.sha256(path.read_bytes()).hexdigest()


def source_identity(repo):
    roots = (repo / "compiler", repo / "tools/sgc", repo / "runtime", repo / "tools/stdlib")
    return {str(path.relative_to(repo)): sha(path) for root in roots
        for path in root.rglob("*") if path.is_file()
        and path.suffix in (".rs", ".sg", ".c", ".h", ".toml")
        and not any(part in ("target", "build", "tests", "__pycache__") for part in path.relative_to(root).parts)}


def cleanup(root):
    root=root.resolve(); inventory=list(root.rglob("*"))
    assert all(p.resolve().is_relative_to(root) for p in inventory)
    for p in inventory:
        if p.is_file():p.unlink()
    for p in sorted((p for p in inventory if p.is_dir()),key=lambda p:len(p.parts),reverse=True):p.rmdir()
    root.rmdir()


class ProcessRss:
    """Sample only the launched PID and descendants discovered through its tree."""
    def __init__(self, root_pid, interval_ms=10):
        assert os.name=="nt", "this driver currently measures Windows working sets only"
        self.root_pid=root_pid; self.interval=interval_ms/1000; self.records={}; self.sampled_tree_peak=0
        self.samples=0; self.gaps=[]; self.previous=None; self.stop=threading.Event(); self.failures=0
        self.kernel=ctypes.WinDLL("kernel32",use_last_error=True);self.psapi=ctypes.WinDLL("psapi",use_last_error=True)
        size=ctypes.c_size_t
        class Memory(ctypes.Structure):
            _fields_=[("cb",wintypes.DWORD),("PageFaultCount",wintypes.DWORD),
                *[(name,size) for name in ("PeakWorkingSetSize","WorkingSetSize","QuotaPeakPagedPoolUsage","QuotaPagedPoolUsage","QuotaPeakNonPagedPoolUsage","QuotaNonPagedPoolUsage","PagefileUsage","PeakPagefileUsage")]]
        class Entry(ctypes.Structure):
            _fields_=[("dwSize",wintypes.DWORD),("cntUsage",wintypes.DWORD),("th32ProcessID",wintypes.DWORD),("th32DefaultHeapID",size),("th32ModuleID",wintypes.DWORD),("cntThreads",wintypes.DWORD),("th32ParentProcessID",wintypes.DWORD),("pcPriClassBase",wintypes.LONG),("dwFlags",wintypes.DWORD),("szExeFile",wintypes.WCHAR*260)]
        self.Memory=Memory;self.Entry=Entry
        self.kernel.CreateToolhelp32Snapshot.argtypes=[wintypes.DWORD,wintypes.DWORD];self.kernel.CreateToolhelp32Snapshot.restype=wintypes.HANDLE
        self.kernel.Process32FirstW.argtypes=[wintypes.HANDLE,ctypes.POINTER(Entry)];self.kernel.Process32NextW.argtypes=[wintypes.HANDLE,ctypes.POINTER(Entry)]
        self.kernel.OpenProcess.argtypes=[wintypes.DWORD,wintypes.BOOL,wintypes.DWORD];self.kernel.OpenProcess.restype=wintypes.HANDLE
        self.kernel.CloseHandle.argtypes=[wintypes.HANDLE]
        self.kernel.GetExitCodeProcess.argtypes=[wintypes.HANDLE,ctypes.POINTER(wintypes.DWORD)]
        self.psapi.GetProcessMemoryInfo.argtypes=[wintypes.HANDLE,ctypes.POINTER(Memory),wintypes.DWORD]
        self.thread=threading.Thread(target=self.loop,daemon=True);self.thread.start()

    def processes(self):
        snapshot=self.kernel.CreateToolhelp32Snapshot(2,0)
        if snapshot==ctypes.c_void_p(-1).value:return {}
        try:
            entry=self.Entry();entry.dwSize=ctypes.sizeof(entry);result={}
            exists=self.kernel.Process32FirstW(snapshot,ctypes.byref(entry))
            while exists:
                result[int(entry.th32ProcessID)]=(int(entry.th32ParentProcessID),entry.szExeFile)
                exists=self.kernel.Process32NextW(snapshot,ctypes.byref(entry))
            return result
        finally:self.kernel.CloseHandle(snapshot)

    def sample(self):
        now=time.perf_counter()
        if self.previous is not None:self.gaps.append((now-self.previous)*1000)
        self.previous=now; entries=self.processes(); owned={self.root_pid};changed=True
        while changed:
            changed=False
            for pid,(parent,_) in entries.items():
                if parent in owned and pid not in owned:owned.add(pid);changed=True
        current=0
        for pid in owned:
            record=self.records.get(pid)
            if record is None:
                handle=self.kernel.OpenProcess(0x410,False,pid)
                if not handle:continue
                record={"pid":pid,"image":entries.get(pid,(0,"unknown"))[1],"handle":handle,"sampled_rss_peak_bytes":0,"os_peak_working_set_bytes":0,"samples":0};self.records[pid]=record
            memory=self.Memory();memory.cb=ctypes.sizeof(memory)
            if self.psapi.GetProcessMemoryInfo(record["handle"],ctypes.byref(memory),memory.cb):
                exit_code=wintypes.DWORD()
                self.kernel.GetExitCodeProcess(record["handle"],ctypes.byref(exit_code))
                if exit_code.value==259:current+=int(memory.WorkingSetSize)
                record["sampled_rss_peak_bytes"]=max(record["sampled_rss_peak_bytes"],int(memory.WorkingSetSize));record["os_peak_working_set_bytes"]=max(record["os_peak_working_set_bytes"],int(memory.PeakWorkingSetSize));record["samples"]+=1
        self.sampled_tree_peak=max(self.sampled_tree_peak,current);self.samples+=1

    def loop(self):
        while not self.stop.is_set():
            try:self.sample()
            except BaseException:self.failures+=1
            self.stop.wait(self.interval)

    def finish(self):
        self.stop.set();self.thread.join(timeout=5)
        processes=[]
        for record in self.records.values():
            memory=self.Memory();memory.cb=ctypes.sizeof(memory)
            if self.psapi.GetProcessMemoryInfo(record["handle"],ctypes.byref(memory),memory.cb):
                record["os_peak_working_set_bytes"]=max(record["os_peak_working_set_bytes"],int(memory.PeakWorkingSetSize))
            self.kernel.CloseHandle(record["handle"])
            processes.append({k:v for k,v in record.items() if k!="handle"})
        return {"metric":"Windows resident working set; tree sums shared pages per-process, not unique physical footprint","sample_interval_target_ms":self.interval*1000,"samples":self.samples,"max_sampling_gap_ms":max(self.gaps,default=0),"sample_errors":self.failures,"sampled_process_tree_peak_rss_bytes":self.sampled_tree_peak,"processes":processes,"scope":"only launched process tree; fixture server/driver excluded; short-lived unsampled children may be absent","pid_reuse_note":"short-run PID/parent discovery; OS process handles retain observed identity, but pathological PID reuse during one phase is not separately distinguished"}


class Fixture(http.server.ThreadingHTTPServer):
    daemon_threads=True
    def __init__(self):
        super().__init__(("127.0.0.1",0),Handler);self.lock=threading.Lock();self.active=0;self.peak=0;self.requests=0;self.errors=[];self.barrier=threading.Barrier(4)


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version="HTTP/1.1"
    def log_message(self,*args):pass
    def do_POST(self):
        server=self.server
        with server.lock:server.active+=1;server.requests+=1;server.peak=max(server.peak,server.active)
        try:
            size=int(self.headers.get("Content-Length","0"));assert 0<size<65536
            body=json.loads(self.rfile.read(size));assert self.path=="/chat/completions" and self.headers.get("Authorization")=="Bearer SYNTHETIC-ONLY"
            assert body["model"]=="synthetic-model" and body["stream"] is True
            wave=body["messages"][0]["content"]
            assert wave in ("cancel-wave", "complete-wave", "slow-wave")
            cancel=wave=="cancel-wave"; burst=wave!="complete-wave"
            server.barrier.wait(timeout=5)
            self.send_response(200);self.send_header("Content-Type","text/event-stream");self.send_header("Connection","close");self.end_headers()
            def frame(value):
                encoded=("data: "+json.dumps(value,ensure_ascii=False,separators=(",",":"))+"\n\n").encode()
                # Five-byte fragments cut UTF-8 scalars and SSE fields. This is
                # deliberately paced local transport, not inference latency.
                if burst:
                    self.wfile.write(encoded); self.wfile.flush()
                else:
                    for start in range(0,len(encoded),5):
                        self.wfile.write(encoded[start:start+5]);self.wfile.flush();time.sleep(.001)
            for _ in range(10000 if cancel else (2048 if burst else 16)):
                frame({"choices":[{"delta":{"content":"世界😀 "*(8 if burst else 1)},"finish_reason":None}]})
            frame({"choices":[{"delta":{},"finish_reason":"stop"}],"usage":{"prompt_tokens":1,"completion_tokens":16}})
            self.wfile.write(b"data: [DONE]\n\n");self.wfile.flush()
        except (BrokenPipeError,ConnectionResetError,ConnectionAbortedError):pass
        except BaseException as error:
            with server.lock:server.errors.append(type(error).__name__+": "+str(error))
        finally:
            with server.lock:server.active-=1
            self.close_connection=True


def distribution(values):
    ordered=sorted(values)
    if not ordered:return {"count":0,"min":None,"p50":None,"p95":None,"max":None}
    return {"count":len(ordered),"min":ordered[0],"p50":statistics.median(ordered),"p95":ordered[max(0,math.ceil(len(ordered)*.95)-1)],"max":ordered[-1]}


def main():
    parser=argparse.ArgumentParser()
    for name in ("distribution","sgpm","sgc","clang","evidence"):parser.add_argument("--"+name,type=Path,required=True)
    parser.add_argument("--trials",type=int,default=5)
    parser.add_argument("--startup-trials",type=int,default=30)
    args=parser.parse_args();assert os.name=="nt" and 1<=args.trials<=20 and 1<=args.startup_trials<=100
    repo=Path(__file__).resolve().parents[1];output=args.evidence.resolve();output.parent.mkdir(parents=True,exist_ok=True)
    verify(args.distribution,"agents")
    scratch=Path(tempfile.mkdtemp(prefix="sengoo-agent-bench-中文-"));workspace=scratch/"project"
    env=dict(os.environ);env.update(SENGOO_STDLIB=str(repo/"tools/stdlib"),SENGOO_RUNTIME=str(repo/"tools/stdlib/runtime.c"),SENGOO_CLANG=str(args.clang.resolve()),SGPM_SGC=str(args.sgc.resolve()))
    env["PATH"]=str(args.clang.resolve().parent)+os.pathsep+env.get("PATH","");env.pop("SENGOO_MODULE_MAP",None)
    records=[];fixture=None;fixture_thread=None
    identities=source_identity(repo)
    report={"schema":1,"result":"running","host":{"name":platform.node(),"platform":platform.platform(),"architecture":platform.machine(),"processor":platform.processor(),"logical_cpus":os.cpu_count()},"scope":"Windows local synthetic four-stream sgllm SDK baseline; no paid or live model; not a competitor comparison","cold_definition":"fresh external project without project frontend/build cache; OS and shared runtime/native caches are not flushed","compiler_sha256":sha(args.sgc.resolve()),"sgpm_sha256":sha(args.sgpm.resolve()),"distribution_manifest_sha256":sha(args.distribution/"ecosystem-manifest.json"),"concurrency_note":"Other host workloads and OS caches are not controlled; integration owner should run when compiler slots are idle","builds":records,"runtime_trials":[]}
    report.update(clang_sha256=sha(args.clang.resolve()),source_identity=identities,
        git_revision=subprocess.check_output(["git","rev-parse","HEAD"],cwd=repo,text=True).strip(),
        startup_trials=[],startup_scope="fresh native SDK processes from Popen through sdk_ready marker and clean exit after argument parsing; no network; not CLI interactive readiness")
    def run(name,command,cwd,environment,timeout=360):
        started=time.perf_counter();process=subprocess.Popen(list(map(str,command)),cwd=cwd,env=environment,stdout=subprocess.PIPE,stderr=subprocess.PIPE,creationflags=subprocess.CREATE_NO_WINDOW)
        meter=ProcessRss(process.pid)
        try:
            stdout,stderr=process.communicate(timeout=timeout)
        except BaseException:
            if process.poll() is None:subprocess.run(["taskkill","/PID",str(process.pid),"/T","/F"],capture_output=True);process.wait(timeout=10)
            raise
        finally:memory=meter.finish()
        elapsed=time.perf_counter()-started;text=stdout.decode("utf-8","replace");errors=stderr.decode("utf-8","replace")
        (output.parent/(name+".log")).write_text(text+errors,encoding="utf-8")
        observation={"name":name,"exit_code":process.returncode,"seconds":elapsed,"rss":memory}
        assert process.returncode==0,(name,process.returncode,text,errors)
        assert memory["samples"]>0 and memory["processes"] and memory["sample_errors"]==0,(name,"RSS sampler failed",memory)
        return observation,text
    try:
        unpack(args.distribution/bundle_name("agents"),workspace)
        (workspace/"src").mkdir()
        source=repo/"examples/bench/agent-stream/src"
        for name in ("main.sg","business.sg"):shutil.copyfile(source/name,workspace/"src"/name)
        (workspace/"Sengoo.toml").write_text('[package]\nname="agent_stream_bench"\nversion="0.1.0"\nedition="2026"\n[bin]\npath="src/main.sg"\n[registries.official]\npath="registry"\n[dependencies]\nsgllm={version="=0.1.0",registry="official"}\n',encoding="utf-8")
        update,_=run("registry-update",[args.sgpm.resolve(),"--runtime-mode","source-development","update"],workspace,env);report["registry_update"]=update
        lock=(workspace/"Sengoo.lock").read_bytes();assert str(repo).encode() not in lock
        report["lock_sha256"]=hashlib.sha256(lock).hexdigest();main_hash=sha(workspace/"src/main.sg")
        def build(phase):
            observation,text=run(phase,[args.sgpm.resolve(),"--runtime-mode","source-development","build","--locked"],workspace,env)
            observation["frontend_sessions"]=[{"reused_modules":int(a),"rebuilt_modules":int(b)} for a,b in re.findall(r"frontend session: reused_modules=(\d+) rebuilt_modules=(\d+)",text)]
            observation["worksets"]=[{"rebuild_modules":int(a),"reuse_modules":int(b),"rebuild_symbols":int(c),"reuse_symbols":int(d)} for a,b,c,d in re.findall(r"codegen workset: rebuild_modules=(\d+) reuse_modules=(\d+) rebuild_symbols=(\d+) reuse_symbols=(\d+)",text)]
            observation["build_cache_hit_observed"]="build cache hit" in text.lower();observation["main_sha256"]=sha(workspace/"src/main.sg");observation["business_sha256"]=sha(workspace/"src/business.sg");records.append(observation)
            assert (workspace/"Sengoo.lock").read_bytes()==lock
        assert not (workspace/"src/build").exists() and not (workspace/"target").exists()
        build("project-cache-cold")
        before=sha(workspace/"src/business.sg");build("unchanged-hot");assert sha(workspace/"src/business.sg")==before
        business=workspace/"src/business.sg";old=business.read_text(encoding="utf-8");assert old.count("\n    1;\n")==1
        business.write_text(old.replace("\n    1;\n","\n    2;\n",1),encoding="utf-8");assert sha(business)!=before and sha(workspace/"src/main.sg")==main_hash
        build("business-body-only-rebuild")
        runtime=scratch/"runtime";runtime.mkdir();exe=runtime/"agent-stream.exe";shutil.copyfile(workspace/"target/debug/agent_stream_bench.exe",exe)
        report["executable_sha256"]=sha(exe);report["sources"]=[{"path":str((source/name).relative_to(repo)),"sha256":sha(source/name)} for name in ("main.sg","business.sg")]
        cleanup(workspace);report["copied_sources_and_registry_removed_before_runtime"]=True
        clean={k:v for k,v in os.environ.items() if not k.startswith(("SENGOO_","SGPM_"))};clean["PATH"]=str(Path(os.environ["SystemRoot"])/"System32")
        for trial in range(args.startup_trials):
            started=time.perf_counter()
            process=subprocess.Popen([str(exe),"--startup-probe"],cwd=runtime,env=clean,
                stdout=subprocess.PIPE,stderr=subprocess.PIPE,creationflags=subprocess.CREATE_NO_WINDOW)
            # communicate has a timeout, so a broken readiness path cannot block
            # the driver indefinitely. Its timestamp includes exit/reaping too.
            try: stdout,stderr=process.communicate(timeout=5)
            except BaseException:
                if process.poll() is None: process.kill(); process.wait(timeout=5)
                raise
            elapsed=(time.perf_counter()-started)*1000
            assert process.returncode==0 and not stderr and json.loads(stdout)=={"type":"sdk_ready"}
            report["startup_trials"].append({"trial":trial,"process_ready_and_exit_ms":elapsed})
        fixture=Fixture();fixture_thread=threading.Thread(target=fixture.serve_forever,daemon=True);fixture_thread.start();base=f"http://127.0.0.1:{fixture.server_port}"
        for trial in range(args.trials):
            observation,text=run(f"streams-{trial}",[exe,base],runtime,clean,timeout=20)
            events=[json.loads(line) for line in text.splitlines() if line.strip()];complete=[e for e in events if e["type"]=="completed_stream"];restored=[e for e in events if e["type"]=="baseline_restored"]
            assert len(complete)==8 and len(restored)==3 and all(e["workers"]==e["handles"]==e["request_copy_bytes"]==0 for e in restored)
            assert all(e["delta_bytes"]==e["expected_utf8_bytes"] for e in complete)
            assert sorted(e["expected_utf8_bytes"] for e in complete)==[176]*4+[180224]*4
            waves=[e for e in events if e["type"] in ("complete_wave","cancel_wave","slow_wave")]
            assert len(waves)==3 and all(e["workers_sampled_peak"]==e["handles_sampled_peak"]==4 and e["business_revision"]==2 for e in waves)
            observation["events"]=events;report["runtime_trials"].append(observation)
            deadline=time.monotonic()+5
            while fixture.active and time.monotonic()<deadline:time.sleep(.01)
            assert fixture.active==0 and not fixture.errors,fixture.errors
        events=[e for trial in report["runtime_trials"] for e in trial["events"]]
        completed=[e for e in events if e["type"]=="completed_stream"]
        report["runtime_summary"]={
            "paced_first_delta_ms":distribution([e["first_delta_ms"] for e in completed if e["expected_utf8_bytes"]==176]),
            "paced_completion_ms":distribution([e["completion_ms"] for e in completed if e["expected_utf8_bytes"]==176]),
            "delayed_consumer_completion_ms":distribution([e["completion_ms"] for e in completed if e["expected_utf8_bytes"]==180224]),
            "cancel_call_ms":distribution([e["cancel_calls_ms"] for e in events if e["type"]=="cancel_requested"]),
            "post_scope_cleanup_wait_ms":distribution([e["cleanup_wait_ms"] for e in events if e["type"]=="baseline_restored"]),
            "process_tree_sampled_peak_rss_bytes":max(x["rss"]["sampled_process_tree_peak_rss_bytes"] for x in report["runtime_trials"]),
            "native_workers_sampled_peak":max(e["workers_sampled_peak"] for e in events if e["type"] in ("complete_wave","cancel_wave","slow_wave")),
            "native_request_copy_bytes_sampled_peak":max(e["request_copy_bytes_sampled_peak"] for e in events if e["type"] in ("complete_wave","cancel_wave","slow_wave")),
            "decoder_pending_metric":"per-stream HTTP pending + SSE pending + SSE data; observed before/after poll, transient maxima may be missed",
            "decoder_pending_sampled_high_bytes":max(e["decoder_pending_sampled_high_bytes"] for e in events if e["type"] in ("complete_wave","cancel_wave","slow_wave")),
            "native_queue_configured_capacity_per_stream_bytes":4096,"native_queue_high_water_bytes":None,
            "native_queue_metric_note":"No existing public runtime getter; configuration capacity is not a measured high-water mark",
            "delayed_consumer_scope":"four burst streams, each 180224 canonical UTF-8 bytes; 250 ms initial no-poll window, 5 ms between polls; queue occupancy/unblocking are not directly measured",
            "fixture_server_parallel_requests_peak":fixture.peak,"fixture_requests":fixture.requests}
        report["startup_summary"]={"process_ready_and_exit_ms":distribution([x["process_ready_and_exit_ms"] for x in report["startup_trials"]]),
            "scope":"fresh SDK process through ready marker and clean exit; includes process creation/reaping; no CLI/TUI startup claim"}
        assert fixture.peak>=4 and fixture.requests==args.trials*12
        final_identity=source_identity(repo)
        report["source_changes_during_measurement"]=[name for name in sorted(set(identities)|set(final_identity))
            if identities.get(name)!=final_identity.get(name)]
        assert not report["source_changes_during_measurement"],"toolchain/runtime sources changed during measurement"
        assert sha(args.sgc.resolve())==report["compiler_sha256"] and sha(args.sgpm.resolve())==report["sgpm_sha256"]
        assert sha(args.clang.resolve())==report["clang_sha256"]
        assert sha(args.distribution/"ecosystem-manifest.json")==report["distribution_manifest_sha256"]
        report["result"]="passed"
    except BaseException as error:report.update(result="failed",error=repr(error));raise
    finally:
        if fixture:fixture.shutdown();fixture.server_close()
        if fixture_thread:fixture_thread.join(timeout=5)
        cleanup(scratch);report["temporary_inventory_removed"]=True
        output.write_text(json.dumps(report,ensure_ascii=False,indent=2),encoding="utf-8")
    print(json.dumps({"result":report["result"],"builds":[{"name":x["name"],"seconds":x["seconds"],"sampled_tree_peak_rss_bytes":x["rss"]["sampled_process_tree_peak_rss_bytes"],"frontend_sessions":x["frontend_sessions"]} for x in records],"runtime":report["runtime_summary"],"evidence":str(output)},ensure_ascii=False,indent=2))


if __name__=="__main__":main()
