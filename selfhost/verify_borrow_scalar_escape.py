"""Qualify primitive scalar borrow escape with baseline causality and ownership/HIR/native regressions."""
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
import shutil
import sys

ROOT = Path(__file__).resolve().parents[1]
ENTRIES = ['selfhost/tests/borrow_scalar_escape.sg','selfhost/tests/hir_origins.sg','selfhost/tests/hir_body_coverage.sg','selfhost/tests/borrow.sg','selfhost/tests/mir.sg','selfhost/tests/handle_disarm_hir.sg','selfhost/tests/optional_borrow.sg','selfhost/tools/native.sg','selfhost/tools/semantic.sg']
EXTRAS=['selfhost/verify_mir.py','selfhost/verify_projection.py','selfhost/verify_handle_disarm.py','selfhost/tests/projection_probe.c','selfhost/tests/handle_disarm.sg','selfhost/tests/handle_disarm_probe.c','selfhost/tests/borrow_scalar_escape_probe.c']
OLD='if path >= 0 { let binding = sh_b_path_root(result, path);'
NEW='if path >= 0 and not (sh_s_kind(checked, sh_s_resolve(checked, sh_node_type(checked, node))) >= 1 and sh_s_kind(checked, sh_s_resolve(checked, sh_node_type(checked, node))) <= 4) { let binding = sh_b_path_root(result, path);'
RUNTIME_SOURCE='extern "C"{fn se_acquire()->i64;fn se_release(value:i64);fn se_counts(round:i64)->i64;} struct Owner{value:i64} impl Drop for Owner{def drop(&mut self){se_release(self.value);}} struct Option<T>{is_some:bool,value:T} def next(value:&Owner)->Option<&Owner>{Option{is_some:true,value:value}} def exercise()->i64{let owner=Owner{value:se_acquire()};let item=next(&owner);if item.is_some{item.value.value}else{0}} def main()->i64{let mut round=0;while round<100{if exercise()!=42{return 1;};round=round+1;if se_counts(round)!=42{return 2;};};42}'


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
    paths.update(ROOT/name for name in EXTRAS)
    paths.update(ROOT/name for name in ENTRIES)
    paths.update(path for path in(ROOT/'openspec/changes/add-selfhost-handle-disarm').rglob('*.md')if path.name!='tasks.md')
    paths.add(ROOT/'openspec/changes/add-selfhost-semantics/scalar-borrow-escape.md')
    paths.add(ROOT/'openspec/changes/add-selfhost-semantics/specs/selfhost-scalar-borrow-escape/spec.md')
    return {str(path):sha(path)for path in sorted(paths)}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output",type=Path,default=ROOT/"target/selfhost/m3-scalar-borrow-escape/checkpoint")
    parser.add_argument("--seed",type=Path,default=ROOT/"target/agent-conditional-compiler/bin/sgc.exe")
    parser.add_argument("--reference",type=Path,default=ROOT/"target/selfhost/seed-reference/release/sgc.exe")
    parser.add_argument("--clang",type=Path,default=Path("C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe"))
    args=parser.parse_args();out=args.output.resolve()
    if not out.is_relative_to(ROOT/"target"):raise ValueError("outputs must remain within target/")
    out.mkdir(parents=True,exist_ok=True)
    report={"schema":1,"stage":"primitive-scalar-borrow-escape","result":"running","m3_complete":False,"hir_schema":7,"mir_schema":1,"started_at":dt.datetime.now(dt.timezone.utc).isoformat(),"host":{"platform":platform.platform(),"python":platform.python_version(),"python_executable_sha256":sha(sys.executable)},"steps":[],"scope":"immutable source snapshots; primitive path reads only; no raw pointer, owner Copy or provider permission"}
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
        for name in ("tools/stdlib","runtime",*EXTRAS,"Cargo.toml","Cargo.lock","rust-toolchain.toml","selfhost/bootstrap.lock","selfhost/reference-baseline.lock"):
            source=ROOT/name
            paths=source.rglob("*")if source.is_dir()else [source]
            for path in paths:
                if not path.is_file()or source.is_dir()and any(part in {"build","target","__pycache__"}for part in path.relative_to(source).parts):continue
                dest=snapshot/path.relative_to(ROOT);dest.parent.mkdir(parents=True,exist_ok=True);data=path.read_bytes();dest.write_bytes(data);origins[str(path)]=hashlib.sha256(data).hexdigest();snapshots[str(dest)]=sha(dest)
        for source in (ROOT/'openspec/changes/add-selfhost-handle-disarm').rglob('*.md'):
            if source.name!='tasks.md':
                dest=snapshot/source.relative_to(ROOT);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(source.read_bytes());snapshots[str(dest)]=sha(dest)
        if any(sha(path)!=digest for path,digest in origins.items()):raise RuntimeError("mixed source snapshot")
        report["producer_origins"]=origins;report["producer_snapshots"]=snapshots
        env={key:value for key,value in os.environ.items()if not key.upper().startswith(("SENGOO_","SGPLATFORM_","SDL2_"))and not rust_override(key)}
        env.update(SENGOO_STDLIB=str(snapshot/"tools/stdlib"),SENGOO_RUNTIME=str(snapshot/"tools/stdlib/runtime.c"),SENGOO_CLANG=str(args.clang.resolve()),SENGOO_MODULE_MAP=";".join("selfhost_"+name+"="+str(path)for name,path in modules.items()),RUSTC=str(rustbin/"rustc.exe"),CARGO_BUILD_RUSTC=str(rustbin/"rustc.exe"),CARGO=str(rustbin/"cargo.exe"),RUSTUP_TOOLCHAIN=rustbin.parent.name,RUSTC_WRAPPER="",RUSTC_WORKSPACE_WRAPPER="",CARGO_BUILD_RUSTC_WRAPPER="",CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER="")
        env["PATH"]=str(rustbin)+os.pathsep+str(args.clang.resolve().parent)+os.pathsep+env.get("PATH","")
        cargo_home=Path(env.get("CARGO_HOME",str(Path.home()/".cargo")))
        candidates={parent/".cargo"/name for parent in(ROOT,*ROOT.parents,snapshot,*snapshot.parents)for name in("config","config.toml")}
        candidates.update(cargo_home/name for name in("config","config.toml"))
        for path in sorted(candidates):
            raw_config=path.read_bytes()if path.is_file()else None
            configs[str(path)]=hashlib.sha256(raw_config).hexdigest()if raw_config is not None else None
            if raw_config is not None and "include"in tomllib.loads(raw_config.decode("utf-8")):raise RuntimeError("included Cargo configuration needs separate qualification")
        report["cargo_config_candidates"]=configs
        report["excluded_nonproducer_evidence_ledger"]="openspec/changes/add-selfhost-semantics/scalar-borrow-escape-tasks.md"
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
        raw=(snapshot/'selfhost/src/borrow.sg').read_bytes()
        if raw.count(NEW.encode())!=1:raise RuntimeError('primitive guard is not the unique expected delta')
        baseline=raw.replace(NEW.encode(),OLD.encode())
        if hashlib.sha256(baseline).hexdigest()!='990fb05324a641ecd2ebee7cd696d3224e55f8992df5bdd4118718969bdff45c':raise RuntimeError('old ownership baseline reconstruction differs')
        baseline_path=out/'borrow-baseline.sg';baseline_path.write_bytes(baseline);snapshots[str(baseline_path)]=sha(baseline_path);report['legacy_baseline_sha256']=sha(baseline_path)
        selected_map=env['SENGOO_MODULE_MAP'];env['SENGOO_MODULE_MAP']=';'.join(part for part in selected_map.split(';')if not part.startswith('selfhost_borrow='))+';selfhost_borrow='+str(baseline_path)
        baseline_exe=out/'baseline.exe';run('baseline-focused-build',[args.seed.resolve(),'--runtime-mode','source-development','build',snapshot/'selfhost/tests/borrow_scalar_escape.sg','--force-rebuild','-O','0','-o',baseline_exe])
        snapshots[str(baseline_exe)]=sha(baseline_exe)
        tested=run('baseline-focused-fifty-first-resources',[baseline_exe,'baseline'])
        if tested.stdout.strip()!=b'scalar escape: fifty first-call resource scopes passed'or tested.stderr:raise RuntimeError('baseline focused oracle mismatch')
        env['SENGOO_MODULE_MAP']=selected_map
        def build(name,entry):
            exe=out/(name+'.exe');run(name+'-build',[args.seed.resolve(),'--runtime-mode','source-development','build',snapshot/entry,'--force-rebuild','-O','0','-o',exe]);snapshots[str(exe)]=sha(exe);return exe
        focused=build('focused','selfhost/tests/borrow_scalar_escape.sg');tested=run('candidate-focused-fifty-first-resources',[focused])
        if tested.stdout.strip()!=b'scalar escape: fifty first-call resource scopes passed'or tested.stderr:raise RuntimeError('candidate focused oracle mismatch')
        owned=build('owned-hir6','selfhost/tests/hir_origins.sg');tested=run('owned-hir6-first-resources',[owned])
        if tested.stdout.strip()!=b'HIR6 exact first-call resources passed'or tested.stderr:raise RuntimeError('owning HIR6 oracle mismatch')
        provider_report=ROOT/'target/selfhost/m3-collection-provider-ast-checkpoint/evidence.json'
        if sha(provider_report)!='a63152e01208dff39e1d5cbcbd326187901808c8f76fa0d3075a08e0a9fa1cd3':raise RuntimeError('qualified provider report changed')
        qualified=json.loads(provider_report.read_bytes());provider_root=provider_report.parent/'implementation'
        for path,digest in qualified['sources'].items():
            if sha(path)!=digest:raise RuntimeError('provider input changed: '+path)
            before[path]=digest
        before[str(provider_report)]=sha(provider_report)
        for relative,digest in qualified['artifacts'].items():
            path=provider_report.parent/relative
            if sha(path)!=digest:raise RuntimeError('provider artifact changed')
            before[str(path)]=digest
        report['inputs_before']=before;report['qualified_provider_report_sha256']=sha(provider_report)
        full_hir=build('full-hir6','selfhost/tests/hir_body_coverage.sg');hir_source=out/'hir-full.sg';hir_source.write_text('import std::collections;import collection_runtime;def identity(value:i64)->i64{value} def construct()->Vec<i64>{vec_new()} def main()->i64{identity(42)}',encoding='utf8');snapshots[str(hir_source)]=sha(hir_source)
        tested=run('full-hir6-three-first-resources',[full_hir,provider_root,qualified['provider_manifest_sha256'],qualified['provider_object_sha256'],hir_source])
        if tested.stdout.strip()!=b'hir body coverage: three first-call resource scopes passed'or tested.stderr:raise RuntimeError('full std HIR6 oracle mismatch')
        optional=build('optional-guard','selfhost/tests/optional_borrow.sg')
        for mode in [3,4]:
            tested=run('optional-false-some-'+str(mode),[optional,*(['bad']*(mode+1))],42)
            if tested.stdout.decode('utf8').splitlines()!=['5007']*5 or tested.stderr:raise RuntimeError('optional Some guard regression')
        report['regressions']={}
        for name,script,extra,wanted in [('mir','verify_mir.py',[],'passed'),('projection','verify_projection.py',['--reference',args.reference.resolve()],'passed'),('disarm','verify_handle_disarm.py',['--reference',args.reference.resolve()],'passed-with-reference-zero-drop-policy-difference')]:
            dest=snapshot/'target'/name;run('existing-'+name,[sys.executable,snapshot/'selfhost'/script,'--seed',args.seed.resolve(),'--clang',args.clang.resolve(),'--output',dest,*extra])
            evidence=dest/'evidence.json';child=json.loads(evidence.read_bytes())
            if child['result']!=wanted:raise RuntimeError(name+' regression failed')
            for key,value in child.items():
                if key.startswith(('source_changes','snapshot_changes','tool_changes','cargo_config_changes','checkout_changes','changed_'))and value:raise RuntimeError(name+' input guard changed: '+key)
            report['regressions'][name]={'report':str(evidence),'sha256':sha(evidence),'steps':len(child['steps']),'result':child['result']}
            snapshots[str(evidence)]=sha(evidence)
            for artifact in dest.rglob('*'):
                if artifact.is_file()and artifact.suffix in {'.exe','.obj','.ll'}and not any(part in {'target','build'}for part in artifact.relative_to(dest).parts[:-1]):snapshots[str(artifact)]=sha(artifact)
        source=out/'scalar-result 世界 &.sg';source.write_text(RUNTIME_SOURCE,encoding='utf8');snapshots[str(source)]=sha(source)
        compiler=snapshot/'target/projection/native.exe';obj=out/'scalar.obj';native=out/'scalar-native.exe';ir=out/'scalar.ll';reference=out/'scalar-reference.exe';probe=snapshot/'selfhost/tests/borrow_scalar_escape_probe.c'
        emitted=run('scalar-native-emit',[compiler,source,obj])
        if json.loads(emitted.stdout)!={'schema_version':1,'ok':True,'phase':'object'}or emitted.stderr:raise RuntimeError('scalar native emit mismatch')
        snapshots[str(obj)]=sha(obj);run('scalar-native-link',[args.clang.resolve(),probe,obj,'-o',native]);snapshots[str(native)]=sha(native);tested=run('scalar-native-hundred-owning-scopes',[native],42)
        if tested.stdout or tested.stderr:raise RuntimeError('scalar native resource oracle mismatch')
        run('scalar-reference-ir',[args.reference.resolve(),'--runtime-mode','source-development','build',source,'--force-rebuild','-O','0','--emit-llvm','-o',ir]);snapshots[str(ir)]=sha(ir);run('scalar-reference-link',[args.clang.resolve(),probe,ir,'-o',reference]);snapshots[str(reference)]=sha(reference);tested=run('scalar-reference-hundred-owning-scopes',[reference],42)
        if tested.stdout or tested.stderr:raise RuntimeError('scalar reference resource oracle mismatch')
        report['artifacts']={str(path):sha(path)for path in [baseline_exe,focused,owned,full_hir,optional,obj,native,ir,reference]};report['focused_resource_scopes']=100;report['ordinary_owner_runtime_scopes_per_compiler']=100;report['provider_reference_authority']=False;report['result']='passed-primitive-scalar-borrow-escape'
    except BaseException as error:report.update(result="failed",failure=type(error).__name__+": "+str(error))
    finally:
        try:
            after=dependency_inputs()if before else {};after.update({path:sha(path)for path in before if path not in after});report["inputs_after"]=after
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
    return 0 if report["result"]=="passed-primitive-scalar-borrow-escape"else 1

if __name__=="__main__":raise SystemExit(main())
