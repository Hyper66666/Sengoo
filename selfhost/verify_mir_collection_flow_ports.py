"""Qualify bounded internal collection flow configuration rejection before cloning."""
from __future__ import annotations
import argparse, datetime, hashlib, json, os, platform, re, shutil, struct, subprocess, sys, tomllib
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def revision():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def rust_override(key):
    key=key.upper()
    return key.startswith(('RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','CARGO_BUILD_RUSTC','CARGO_BUILD_RUSTDOC','CARGO_PROFILE_')) or key in {'CARGO','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS','CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_RUSTDOCFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','CARGO_INCREMENTAL'} or key.startswith('CARGO_TARGET_') and key.endswith(('_RUSTFLAGS','_RUSTDOCFLAGS','_RUSTC'))
def closure(entries):
    result={};pending=list(entries)
    while pending:
        path=pending.pop()
        if path in result:continue
        result[path]=sha(path)
        pending += [ROOT/'selfhost/src'/(n+'.sg') for n in re.findall(r'import\s+selfhost_([A-Za-z0-9_]+)\s*;',path.read_text('utf8'))]
    return result
flow_maps={}
def flow_closure(entry):
    aliases={}
    result={};pending=[entry]
    while pending:
        path=pending.pop()
        if path in result:continue
        result[path]=sha(path)
        for name in re.findall(r'import\s+(selfhost_[A-Za-z0-9_]+)\s*;',path.read_text('utf8')):
            target=aliases.get(name,ROOT/'selfhost/src'/(name[len('selfhost_'):]+'.sg'))
            assert target.is_file(),(name,target);flow_maps[name]=target;pending.append(target)
    return result

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for n in ['seed','clang','formatter','output']:parser.add_argument('--'+n,required=True,type=Path)
    args=parser.parse_args();base=args.output.resolve();assert base.is_relative_to(ROOT/'target');base.mkdir(parents=True,exist_ok=False)
    inputs={str(Path(__file__).resolve()):sha(__file__)}
    originals=flow_closure(ROOT/'selfhost/tests/mir_collection_flow_ports.sg')
    for directory in ['tools/stdlib','runtime']:
        originals.update({p:sha(p)for p in (ROOT/directory).rglob('*')if p.is_file()and not any(n in {'build','target','__pycache__'}for n in p.relative_to(ROOT/directory).parts)})
    for n in ['selfhost/bootstrap.lock','Cargo.toml','Cargo.lock','rust-toolchain.toml']:originals[ROOT/n]=sha(ROOT/n)

    snapshot=base/'implementation'
    for p,h in originals.items():
        raw=p.read_bytes();assert hashlib.sha256(raw).hexdigest()==h;copy=snapshot/p.relative_to(ROOT);copy.parent.mkdir(parents=True,exist_ok=True);copy.write_bytes(raw);assert sha(copy)==h;inputs[str(copy)]=h
    assert all(sha(p)==h for p,h in originals.items())
    for directory in ['runtime']:
        inputs.update({str(p):sha(p)for p in (ROOT/directory).rglob('*')if p.is_file()and not any(n in {'build','target','__pycache__'}for n in p.relative_to(ROOT/directory).parts)})
    for n in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:inputs[str(ROOT/n)]=sha(ROOT/n)
    for p in (ROOT/'openspec/changes/add-selfhost-collection-intrinsics/specs/selfhost-collection-effects').rglob('*.md'):
        if p.name!='tasks.md':
            raw=p.read_bytes();copy=snapshot/p.relative_to(ROOT);copy.parent.mkdir(parents=True,exist_ok=True);copy.write_bytes(raw);inputs[str(copy)]=sha(copy)
    tools={'seed':args.seed.resolve(),'clang':args.clang.resolve(),'formatter':args.formatter.resolve(),'python':Path(sys.executable)}
    lock=json.loads((ROOT/'selfhost/bootstrap.lock').read_text('utf8'))
    for n in ['seed','clang','formatter']:assert sha(tools[n])==lock[n]['sha256']
    env={k:v for k,v in os.environ.items()if not rust_override(k)and not k.upper().startswith(('SENGOO_','SGPLATFORM_','SDL2_','PYTHON','LLDB_'))and k.upper()not in {'CL','_CL_','LINK','_LINK_'}}
    env['PATH']=str(tools['clang'].parent)+os.pathsep+env.get('PATH','')
    tools['rustup']=Path(shutil.which('rustup',path=env['PATH'])).resolve()
    for n in ['cargo','rustc']:tools[n]=Path(subprocess.check_output([tools['rustup'],'which',n],env=env,text=True).strip()).resolve()
    assert tools['cargo'].parent==tools['rustc'].parent
    env.update(CARGO=str(tools['cargo']),RUSTC=str(tools['rustc']),CARGO_BUILD_RUSTC=str(tools['rustc']),RUSTUP_TOOLCHAIN=tools['rustc'].parent.parent.name,RUSTC_WRAPPER='',RUSTC_WORKSPACE_WRAPPER='',CARGO_BUILD_RUSTC_WRAPPER='',CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='')
    env['PATH']=str(tools['cargo'].parent)+os.pathsep+env['PATH']
    env.update(SENGOO_STDLIB=str(snapshot/'tools/stdlib'),SENGOO_RUNTIME=str(snapshot/'tools/stdlib/runtime.c'),SENGOO_CLANG=str(tools['clang']),SENGOO_LINKER='lld',SENGOO_MODULE_MAP=';'.join(n+'='+str(snapshot/p.relative_to(ROOT))for n,p in flow_maps.items()))
    pure={k:v for k,v in env.items()if not k.upper().startswith('SENGOO_')};pure['PATH']=''
    report={'schema':1,'stage':'bounded-internal-collection-flow-ports','result':'running','production_admitted':False,'m3_complete':False,'mir_schema':1,'host':platform.platform(),'source_revision':revision(),'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sources':inputs,'qualified_checkout_sources':{str(p):h for p,h in originals.items()},'tools':{n:{'path':str(p),'sha256':sha(p)}for n,p in tools.items()},'steps':[],'limits':['only bounded internal source lowering configuration and first-resource rejection behavior; no source Ref/provider permission','actual checked/ownership input and type/symbol state remain unchanged; invalid contexts reject before copying flow vectors or emitting MIR','ordinary public lower/native still reject candidate optional borrowing; complete fresh source authority and generic/source SDK integration are separate gates','development seed uses recorded C/Rust runtimes; no complete final linker/SDK or M6/M8/selfbuild qualification'],'sdk_environment':{n:env.get(n)for n in ['INCLUDE','LIB','LIBPATH','VCToolsInstallDir','WindowsSdkDir','WindowsSDKVersion','VSINSTALLDIR','VCINSTALLDIR']}}
    configs={p/'.cargo'/n for p in [ROOT,*ROOT.parents,snapshot,*snapshot.parents]for n in ['config','config.toml']};configs.update(Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))/n for n in ['config','config.toml']);report['cargo_config_candidates']={}
    for p in configs:
        raw=p.read_bytes()if p.is_file()else None;report['cargo_config_candidates'][str(p)]=None if raw is None else hashlib.sha256(raw).hexdigest()
        if raw is not None:assert 'include'not in tomllib.loads(raw.decode('utf8'))
    def save():(base/'evidence.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    def run(name,command,code=0,empty=False):
        done=subprocess.run(list(map(str,command)),cwd=ROOT,env=pure if empty else env,capture_output=True,timeout=900);stdout=base/(name+'.stdout.log');stderr=base/(name+'.stderr.log');stdout.write_bytes(done.stdout);stderr.write_bytes(done.stderr);report['steps'].append({'name':name,'command':list(map(str,command)),'exit_code':done.returncode,'expected_exit_code':code,'empty_path':empty,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)});save();print(name,done.returncode,flush=True);assert done.returncode==code,(name,done.returncode,done.stdout[-800:],done.stderr[-3000:]);return done
    artifacts={}
    try:
        for n in ['cargo','rustc']:run(n+'-version',[tools[n],'-vV'if n=='rustc'else'-V'])
        # Cargo's actual compiler selection is verified, not inferred from a shim/sysroot.
        probe=base/'cargo-probe';probe.mkdir();(probe/'src').mkdir();(probe/'Cargo.toml').write_text('[package]\nname="effects_rustc_probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n',encoding='utf8');(probe/'src/lib.rs').write_text('pub fn value()->i64{42}\n',encoding='utf8');(probe/'build.rs').write_text('fn main(){println!("cargo:warning=effects_rustc={}",std::env::var("RUSTC").unwrap());}\n',encoding='utf8')
        observed=run('cargo-actual-rustc',[tools['cargo'],'build','--offline','--verbose','--manifest-path',probe/'Cargo.toml','--target-dir',probe/'target']);combined=(observed.stdout+observed.stderr).decode('utf8');runs=[line for line in combined.splitlines()if line.lstrip().startswith('Running ')and str(tools['rustc']).casefold()in line.casefold()];assert len(runs)>=2 and 'effects_rustc='+str(tools['rustc'])in combined;report['cargo_actual_rustc_invocations']=runs
        for p in [probe/'Cargo.toml',probe/'Cargo.lock',probe/'build.rs',probe/'src/lib.rs']:inputs[str(p)]=sha(p)
        run('format-lower',[tools['formatter'],'--check',snapshot/'selfhost/src/mir_lower.sg']);run('format-test',[tools['formatter'],'--check',snapshot/'selfhost/tests/mir_collection_flow_ports.sg']);
        binary=base/'ports.exe';run('ports-build',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/mir_collection_flow_ports.sg','--force-rebuild','-O','0','-o',binary]);artifacts[binary.name]=sha(binary);result=run('ports-first-resources',[binary],42,True);assert result.stdout.splitlines()==[b'flow ports: 9 early rejections x100 first-resource scopes passed']and not result.stderr;report.update(result='passed-bounded-flow-ports',artifacts=artifacts,production_admitted=False,early_configuration_rejections=9,first_call_resource_scopes=100)
    except BaseException as error:report.update(result='failed',error=repr(error));raise
    finally:
        report.update(changed_sources=[n for n,h in inputs.items()if not Path(n).is_file()or sha(n)!=h],changed_tools=[n for n,value in report['tools'].items()if not Path(value['path']).is_file()or sha(value['path'])!=value['sha256']],checkout_changes=[str(p)for p,h in originals.items()if not p.is_file()or sha(p)!=h],changed_configs=[n for n,h in report['cargo_config_candidates'].items()if(sha(n)if Path(n).is_file()else None)!=h],changed_artifacts=[n for n,h in artifacts.items()if not(base/n).is_file()or sha(base/n)!=h],source_revision_end=revision(),completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
        if any(report[k]for k in ['changed_sources','changed_tools','checkout_changes','changed_configs','changed_artifacts']):report.update(result='failed',error='qualified input/output changed')
        save()
    assert report['result']=='passed-bounded-flow-ports';print(sha(base/'evidence.json'))
if __name__=='__main__':main()
