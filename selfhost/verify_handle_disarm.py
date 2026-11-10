"""Verify production canonical-handle disarm, HIR6 facts and actual cleanup."""
from __future__ import annotations
import argparse
import datetime as dt
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import subprocess
import sys
import time
import tomllib

ROOT = Path(__file__).resolve().parents[1]
ENTRIES = ["selfhost/tests/handle_disarm_hir.sg", "selfhost/tools/native.sg", "selfhost/tools/semantic.sg"]

def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()

def revision():
    return subprocess.check_output(["git","rev-parse","HEAD"],cwd=ROOT,text=True).strip()

def rust_override(name):
    key=name.upper()
    return key.startswith(("RUSTC","RUSTDOC","RUSTUP_TOOLCHAIN","CARGO_BUILD_RUSTC","CARGO_BUILD_RUSTDOC","CARGO_PROFILE_")) or key in {"CARGO","RUSTFLAGS","RUSTDOCFLAGS","CARGO_ENCODED_RUSTFLAGS","CARGO_ENCODED_RUSTDOCFLAGS","CARGO_BUILD_RUSTFLAGS","CARGO_BUILD_RUSTDOCFLAGS","CARGO_BUILD_TARGET","CARGO_TARGET_DIR","CARGO_INCREMENTAL"} or key.startswith("CARGO_TARGET_") and key.endswith(("_RUSTFLAGS","_RUSTDOCFLAGS","_RUSTC"))

def dependency_inputs():
    paths={Path(__file__).resolve(),ROOT/"Cargo.toml",ROOT/"Cargo.lock",ROOT/"rust-toolchain.toml",ROOT/"selfhost/bootstrap.lock",ROOT/"selfhost/reference-baseline.lock"}
    for directory in ("runtime","tools/stdlib"):
        paths.update(path for path in(ROOT/directory).rglob("*")if path.is_file()and not any(part in {"build","target","__pycache__"}for part in path.relative_to(ROOT/directory).parts))
    # tasks.md is a post-run evidence ledger; including its report SHA would be circular.
    paths.update(path for path in (ROOT/"openspec/changes/add-selfhost-handle-disarm").rglob("*.md") if path.name != "tasks.md")
    for name in ("selfhost/src/borrow.sg","selfhost/src/hir.sg","selfhost/src/mir_lower.sg","selfhost/tests/borrow.sg","selfhost/tests/mir.sg","selfhost/tests/handle_disarm_hir.sg","selfhost/tests/handle_disarm.sg","selfhost/tests/handle_disarm_probe.c"):
        paths.add(ROOT/name)
    return {str(path):sha(path)for path in sorted(paths)}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,default=ROOT/"target/selfhost/m3-handle-disarm/checkpoint")
    parser.add_argument("--seed",type=Path,default=ROOT/"target/agent-conditional-compiler/bin/sgc.exe")
    parser.add_argument("--reference",type=Path,default=ROOT/"target/selfhost/seed-reference/release/sgc.exe")
    parser.add_argument("--clang",type=Path,default=Path("C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe"))
    args=parser.parse_args();out=args.output.resolve()
    if not out.is_relative_to(ROOT/"target"):raise ValueError("outputs must remain within target/")
    out.mkdir(parents=True,exist_ok=True)
    report={"schema":1,"stage":"canonical-handle-disarm-hir6","result":"running","m3_complete":False,"hir_schema":6,"mir_schema":1,"started_at":dt.datetime.now(dt.timezone.utc).isoformat(),"host":{"platform":platform.platform(),"python":platform.python_version(),"python_executable_sha256":sha(sys.executable)},"steps":[],"scope":"production source snapshots; canonical disarm only; no root resurrection, raw pointer or SDK widening"}
    before={};origins={};snapshots={};tools={};configs={}
    def save():(out/"evidence.json").write_text(json.dumps(report,indent=2)+"\n",encoding="utf-8")
    save()
    try:
        report["source_revision"]=revision()
        lock=json.loads((ROOT/"selfhost/bootstrap.lock").read_text(encoding="utf-8"))
        baseline=json.loads((ROOT/"selfhost/reference-baseline.lock").read_text(encoding="utf-8"))
        rustbin=Path(os.environ.get("RUSTUP_HOME",str(Path.home()/".rustup")))/"toolchains"/(lock["rust_toolchain"]+"-x86_64-pc-windows-msvc")/"bin"
        expected={"seed":(args.seed.resolve(),lock["seed"]["sha256"]),"clang":(args.clang.resolve(),lock["clang"]["sha256"]),"reference":(args.reference.resolve(),baseline["tools"]["sgc"]["sha256"]),"rustc":(rustbin/"rustc.exe",lock["rustc_sha256"]),"cargo":(rustbin/"cargo.exe",lock["cargo_sha256"])}
        for name,(path,digest)in expected.items():
            if sha(path)!=digest:raise RuntimeError(name+" identity mismatch")
            tools[name]={"path":str(path),"sha256":digest}
        report["tools"]=tools
        before=dependency_inputs();report["inputs_before"]=before
        snapshot=out/"implementation";snapshot.mkdir(exist_ok=True)
        pending=[ROOT/name for name in ENTRIES];seen=set();modules={}
        while pending:
            path=pending.pop()
            if path in seen:continue
            seen.add(path);data=path.read_bytes();origins[str(path)]=hashlib.sha256(data).hexdigest()
            dest=snapshot/path.relative_to(ROOT);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data);snapshots[str(dest)]=sha(dest)
            if path.parent==ROOT/"selfhost/src":modules[path.stem]=dest
            for name in re.findall(r"^\s*import\s+selfhost_([a-z0-9_]+)\s*;",data.decode("utf-8"),re.M):
                pending.append(ROOT/"selfhost/src"/(name+".sg"))
        for name in ("tools/stdlib","selfhost/tests/handle_disarm.sg","selfhost/tests/handle_disarm_probe.c"):
            source=ROOT/name
            paths=source.rglob("*")if source.is_dir()else [source]
            for path in paths:
                if not path.is_file()or source.is_dir()and any(part in {"build","target","__pycache__"}for part in path.relative_to(source).parts):continue
                dest=snapshot/path.relative_to(ROOT);dest.parent.mkdir(parents=True,exist_ok=True);data=path.read_bytes();dest.write_bytes(data);origins[str(path)]=hashlib.sha256(data).hexdigest();snapshots[str(dest)]=sha(dest)
        if any(sha(path)!=digest for path,digest in origins.items()):raise RuntimeError("mixed source snapshot")
        report["producer_origins"]=origins;report["producer_snapshots"]=snapshots
        env={key:value for key,value in os.environ.items()if not key.upper().startswith(("SENGOO_","SGPLATFORM_","SDL2_"))and not rust_override(key)}
        env.update(SENGOO_STDLIB=str(snapshot/"tools/stdlib"),SENGOO_RUNTIME=str(snapshot/"tools/stdlib/runtime.c"),SENGOO_CLANG=str(args.clang.resolve()),SENGOO_MODULE_MAP=";".join("selfhost_"+name+"="+str(path)for name,path in modules.items()),RUSTC=str(rustbin/"rustc.exe"),CARGO_BUILD_RUSTC=str(rustbin/"rustc.exe"),CARGO=str(rustbin/"cargo.exe"),RUSTUP_TOOLCHAIN=rustbin.parent.name,RUSTC_WRAPPER="",RUSTC_WORKSPACE_WRAPPER="",CARGO_BUILD_RUSTC_WRAPPER="",CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER="")
        env["PATH"]=str(rustbin)+os.pathsep+str(args.clang.resolve().parent)+os.pathsep+env.get("PATH","")
        cargo_home=Path(env.get("CARGO_HOME",str(Path.home()/".cargo")))
        candidates={parent/".cargo"/name for parent in(ROOT,*ROOT.parents)for name in("config","config.toml")}
        candidates.update(cargo_home/name for name in("config","config.toml"))
        for path in sorted(candidates):
            raw_config=path.read_bytes()if path.is_file()else None
            configs[str(path)]=hashlib.sha256(raw_config).hexdigest()if raw_config is not None else None
            if raw_config is not None and "include"in tomllib.loads(raw_config.decode("utf-8")):raise RuntimeError("included Cargo configuration needs separate qualification")
        report["cargo_config_candidates"]=configs
        report["excluded_nonproducer_evidence_ledger"]="openspec/changes/add-selfhost-handle-disarm/tasks.md"
        report["development_compiler_environment"]={name:env[name]for name in("CARGO","RUSTC","CARGO_BUILD_RUSTC","RUSTUP_TOOLCHAIN","RUSTC_WRAPPER","RUSTC_WORKSPACE_WRAPPER","CARGO_BUILD_RUSTC_WRAPPER","CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")}
        def run(name,command,expected_exit=0):
            start=time.perf_counter();result=subprocess.run(list(map(str,command)),cwd=ROOT,env=env,capture_output=True,timeout=900)
            stdout,stderr=out/(name+".stdout.log"),out/(name+".stderr.log");stdout.write_bytes(result.stdout);stderr.write_bytes(result.stderr)
            report["steps"].append({"name":name,"command":list(map(str,command)),"exit_code":result.returncode,"expected_exit_code":expected_exit,"elapsed_s":time.perf_counter()-start,"stdout_sha256":sha(stdout),"stderr_sha256":sha(stderr)});save();print(name,result.returncode,flush=True)
            if result.returncode!=expected_exit:raise RuntimeError(name+" failed")
            return result
        for name,identity in tools.items():
            version=run(name+"-version",[identity["path"],"-vV"if name=="rustc"else "--version"])
            identity["version"]=version.stdout.decode("utf-8","strict").strip()
        for name,entry in(("hir-tests","selfhost/tests/handle_disarm_hir.sg"),("native","selfhost/tools/native.sg"),("semantic","selfhost/tools/semantic.sg")):
            exe=out/(name+".exe");exe.unlink(missing_ok=True)
            run(name+"-build",[args.seed.resolve(),"--runtime-mode","source-development","build",snapshot/entry,"--force-rebuild","-O","0","-o",exe])
            if name=="hir-tests":
                tested=run("hir-native-20-resource-rounds",[exe])
                if tested.stdout.strip()!=b"detach HIR facts: ok"or tested.stderr:raise RuntimeError("HIR resource marker mismatch")
        for name in("runtime","runtime_string"):
            obj=out/(name+".obj");obj.unlink(missing_ok=True)
            run(name+"-compile",[args.clang.resolve(),"-O0",*(["-Dsengoo_ffi_buffer_free=sengoo_actual_ffi_buffer_free"]if name=="runtime"else []),"-c",snapshot/"tools/stdlib"/(name+".c"),"-o",obj])
        source=snapshot/"selfhost/tests/handle_disarm.sg";probe=snapshot/"selfhost/tests/handle_disarm_probe.c"
        obj=out/"disarm.obj";obj.unlink(missing_ok=True)
        emitted=run("native-emit",[out/"native.exe",source,obj])
        if json.loads(emitted.stdout)!={"schema_version":1,"ok":True,"phase":"object"}or emitted.stderr:raise RuntimeError("native protocol mismatch")
        exe=out/"disarm.exe";exe.unlink(missing_ok=True)
        run("native-link",[args.clang.resolve(),probe,out/"runtime.obj",out/"runtime_string.obj",obj,"-o",exe])
        result=run("native-100-transfer-scopes",[exe],42)
        if result.stdout or result.stderr:raise RuntimeError("native transfer/resource output mismatch")
        llvm=out/"reference.ll";llvm.unlink(missing_ok=True)
        run("reference-ir",[args.reference.resolve(),"--runtime-mode","source-development","build",source,"--force-rebuild","-O","0","--emit-llvm","-o",llvm])
        control=out/"reference-probe.c";raw=probe.read_text(encoding="utf-8")
        original="if(created!=round||freed!=round||source_drops||sengoo_buffer_live_handle_count()||sengoo_string_live_handle_count()||sengoo_opaque_live_handle_count())exit(75);"
        replacement='if(created!=round||freed!=round||source_drops||sengoo_buffer_live_handle_count()||sengoo_string_live_handle_count()||sengoo_opaque_live_handle_count()){fprintf(stderr,"zero-drop-policy-difference created=%lld freed=%lld source_drops=%lld live=%lld round=%lld\\n",(long long)created,(long long)freed,(long long)source_drops,sengoo_buffer_live_handle_count(),(long long)round);exit(75);}'
        if raw.count(original)!=1:raise RuntimeError("reference control edit no longer exact")
        control.write_text("#include <stdio.h>\n"+raw.replace(original,replacement),encoding="utf-8");snapshots[str(control)]=sha(control)
        ref_exe=out/"reference.exe";ref_exe.unlink(missing_ok=True)
        run("reference-link",[args.clang.resolve(),control,out/"runtime.obj",out/"runtime_string.obj",llvm,"-o",ref_exe])
        result=run("reference-zero-drop-policy-control",[ref_exe],75)
        if result.stdout or result.stderr.replace(b"\r\n",b"\n")!=b"zero-drop-policy-difference created=1 freed=1 source_drops=1 live=0 round=1\n":raise RuntimeError("reference failed outside the precise zero-drop policy difference")
        report["reference_policy_difference"]={"created":1,"freed":1,"source_drops":1,"live":0,"classification":"source free(0) destructor call difference; not a resource leak","parity_pass":False}
        core_source=snapshot/"selfhost/src/core.sg"
        actual=run("actual-core-checking",[out/"semantic.exe",core_source],65)
        core=json.loads(actual.stdout)
        if actual.stderr or core.get("phase")!="ownership"or len(core.get("diagnostics",[]))!=112:raise RuntimeError("core checking boundary differs from the scoped checkpoint")
        entry=out/"actual-string.sg";entry.write_text("import std::string;def main()->i64{42}",encoding="utf-8");snapshots[str(entry)]=sha(entry)
        actual=run("actual-string-checking",[out/"semantic.exe",entry])
        string=json.loads(actual.stdout)
        if actual.stderr or string.get("ok")is not True or string.get("phase")!="ownership"or string.get("diagnostics")!=[]:raise RuntimeError("String checking regression")
        report["actual_core"]={"diagnostic_count":len(core["diagnostics"]),"phase":core["phase"],"scope":"still incomplete"};report["actual_string"]={"diagnostic_count":0,"phase":string["phase"]}
        report["mir_flag_trace_asserted"]={"source_initialization_op25_value1":1,"source_extraction_op25_value0":1,"source_zero_assignment_op25":0,"source_zero_assignment_store0":1,"source_op21_drop":0}
        report["artifacts"]={name:sha(out/name)for name in("hir-tests.exe","native.exe","semantic.exe","disarm.obj","disarm.exe","reference.ll","reference.exe")}
        report["result"]="passed-with-reference-zero-drop-policy-difference"
    except BaseException as error:report.update(result="failed",failure=type(error).__name__+": "+str(error))
    finally:
        try:
            after=dependency_inputs()if before else {};report["inputs_after"]=after
            report["source_changes"]=[path for path in sorted(set(before)|set(after))if before.get(path)!=after.get(path)]
            report["snapshot_changes"]=[path for path,digest in snapshots.items()if sha(path)!=digest]
            report["checkout_changes"]=[path for path,digest in origins.items()if sha(path)!=digest]
            report["tool_changes"]=[name for name,identity in tools.items()if sha(identity["path"])!=identity["sha256"]]
            report["changed_cargo_configs"]=[path for path,digest in configs.items()if(sha(path)if Path(path).is_file()else None)!=digest]
            report["source_revision_after"]=revision()
            if report["source_changes"]or report["snapshot_changes"]or report["tool_changes"]or report["changed_cargo_configs"]:report["result"]="failed"
        except BaseException as error:report.update(result="failed",finally_failure=type(error).__name__+": "+str(error))
        report["finished_at"]=dt.datetime.now(dt.timezone.utc).isoformat();save()
    print(json.dumps({name:report.get(name)for name in("result","failure","source_changes","snapshot_changes","checkout_changes","tool_changes")}),flush=True)
    return 0 if report["result"]=="passed-with-reference-zero-drop-policy-difference"else 1

if __name__=="__main__":raise SystemExit(main())
