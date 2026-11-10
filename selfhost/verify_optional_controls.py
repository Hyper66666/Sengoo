"""Qualify old MIR27 refusal and actual Drop layout-cache controls."""
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
def dependencies(path):
    raw=path.read_text('utf8').replace('\\\n',' ');assert raw.startswith('optional_dependency:')
    payload=raw.split(':',1)[1];words=[];word=[];at=0
    while at<len(payload):
        c=payload[at]
        if c=='\\' and at+1<len(payload) and payload[at+1] in ' \t#':word.append(payload[at+1]);at+=2
        elif c=='$' and at+1<len(payload) and payload[at+1]=='$':word.append('$');at+=2
        elif c.isspace():
            if word:words.append(''.join(word));word=[]
            at+=1
        else:word.append(c);at+=1
    if word:words.append(''.join(word))
    paths={Path(w).resolve()for w in words};assert paths and all(p.is_file()for p in paths)
    return paths
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for n in ['seed','clang','formatter','output']:parser.add_argument('--'+n,required=True,type=Path)
    args=parser.parse_args();base=args.output.resolve();assert base.is_relative_to(ROOT/'target');base.mkdir(parents=True,exist_ok=False)
    inputs={str(Path(__file__).resolve()):sha(__file__)}
    originals=closure([ROOT/'selfhost/tests/optional_old_boundary.sg',ROOT/'selfhost/tests/optional_cache_drop.sg'])
    for directory in ['tools/stdlib','runtime']:
        originals.update({p:sha(p)for p in (ROOT/directory).rglob('*')if p.is_file()and not any(n in {'build','target','__pycache__'}for n in p.relative_to(ROOT/directory).parts)})
    for n in ['selfhost/tests/optional_cache_drop_probe.c','selfhost/bootstrap.lock','Cargo.toml','Cargo.lock','rust-toolchain.toml']:originals[ROOT/n]=sha(ROOT/n)
    snapshot=base/'implementation'
    for p,h in originals.items():
        raw=p.read_bytes();assert hashlib.sha256(raw).hexdigest()==h;copy=snapshot/p.relative_to(ROOT);copy.parent.mkdir(parents=True,exist_ok=True);copy.write_bytes(raw);assert sha(copy)==h;inputs[str(copy)]=h
    assert all(sha(p)==h for p,h in originals.items())
    for directory in ['runtime']:
        inputs.update({str(p):sha(p)for p in (ROOT/directory).rglob('*')if p.is_file()and not any(n in {'build','target','__pycache__'}for n in p.relative_to(ROOT/directory).parts)})
    for n in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:inputs[str(ROOT/n)]=sha(ROOT/n)
    for p in (ROOT/'openspec/changes/add-selfhost-collection-intrinsics').rglob('*.md'):
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
    env.update(SENGOO_STDLIB=str(snapshot/'tools/stdlib'),SENGOO_RUNTIME=str(snapshot/'tools/stdlib/runtime.c'),SENGOO_CLANG=str(tools['clang']),SENGOO_MODULE_MAP=';'.join('selfhost_'+p.stem+'='+str(snapshot/p.relative_to(ROOT))for p in originals if p.parent==ROOT/'selfhost/src'))
    pure={k:v for k,v in env.items()if not k.upper().startswith('SENGOO_')};pure['PATH']=''
    report={'schema':1,'stage':'optional-old-consumers-and-aggregate-drop-cache-controls','result':'running','production_admitted':False,'m3_complete':False,'mir_schema':1,'host':platform.platform(),'source_revision':revision(),'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sources':inputs,'qualified_checkout_sources':{str(p):h for p,h in originals.items()},'tools':{n:{'path':str(p),'sha256':sha(p)}for n,p in tools.items()},'steps':[],'limits':['physical MIR/code generation only; source permission requires the separate fresh compiler pipeline','ordinary MIR/native entrypoints reject27 5008; internal physical mode alone grants no source authority','selected historical qualified C provider source/object chain is retained; C allocation pressure is not SG semantic policy','actual C Job-limited OOM and Owner1MiB tests do not establish M8/compiler-closure capacity','development compiler uses recorded C/Rust runtime; nested legacy source/native behavior gates do not qualify complete MSVC child-linker/SDK bootstrap provenance'],'sdk_environment':{n:env.get(n)for n in ['INCLUDE','LIB','LIBPATH','VCToolsInstallDir','WindowsSdkDir','WindowsSDKVersion','VSINSTALLDIR','VCINSTALLDIR']}}
    configs={p/'.cargo'/n for p in [ROOT,*ROOT.parents,snapshot,*snapshot.parents]for n in ['config','config.toml']};configs.update(Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))/n for n in ['config','config.toml']);report['cargo_config_candidates']={}
    for p in configs:
        raw=p.read_bytes()if p.is_file()else None;report['cargo_config_candidates'][str(p)]=None if raw is None else hashlib.sha256(raw).hexdigest()
        if raw is not None:assert 'include'not in tomllib.loads(raw.decode('utf8'))
    def save():(base/'evidence.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    def run(name,command,code=0,empty=False):
        done=subprocess.run(list(map(str,command)),cwd=ROOT,env=pure if empty else env,capture_output=True,timeout=900);stdout=base/(name+'.stdout.log');stderr=base/(name+'.stderr.log');stdout.write_bytes(done.stdout);stderr.write_bytes(done.stderr);report['steps'].append({'name':name,'command':list(map(str,command)),'exit_code':done.returncode,'expected_exit_code':code,'empty_path':empty,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)});save();print(name,done.returncode,flush=True);assert done.returncode==code,(name,done.returncode,done.stdout[-800:],done.stderr[-3000:]);return done
    artifacts={}
    try:
        # Revalidate the exact historical local qualification chain. It is not
        # substituted with freshly observed hashes of mutable current sources.
        qualified=ROOT/'target/selfhost/m3-collection-provider-ast-checkpoint';qp=qualified/'evidence.json';qraw=qp.read_bytes();qdigest=hashlib.sha256(qraw).hexdigest();assert qdigest=='a63152e01208dff39e1d5cbcbd326187901808c8f76fa0d3075a08e0a9fa1cd3';q=json.loads(qraw);assert q['result']=='passed'
        inputs[str(qp)]=qdigest
        for name,digest in q['sources'].items():assert sha(name)==digest;inputs[name]=digest
        for name,item in q['tools'].items():assert sha(item['path'])==item['sha256'];inputs[item['path']]=item['sha256']
        assert not any(q.get(k)for k in ['changed_sources','changed_tools','checkout_changes','changed_configs','changed_artifacts'])
        for relative,digest in q['artifacts'].items():p=qualified/relative;assert sha(p)==digest;inputs[str(p)]=digest
        providers=[];provider_dir=base/'qualified-provider';provider_dir.mkdir()
        for name in ['runtime.obj','runtime_string.obj','runtime_collections.obj']:
            original=qualified/'implementation/target/provider'/name;raw=original.read_bytes();digest=q['artifacts'][str(original.relative_to(qualified))];assert hashlib.sha256(raw).hexdigest()==digest;copy=provider_dir/name;copy.write_bytes(raw);assert sha(copy)==digest;inputs[str(copy)]=digest;providers.append(copy)
        report['qualified_provider']={'report':str(qp),'report_sha256':qdigest,'pinned_historical_qualification':True,'manifest_sha256':q['provider_manifest_sha256'],'object_sha256':q['provider_object_sha256'],'sources':q['sources'],'tools':q['tools'],'artifacts':q['artifacts']}
        for n in ['cargo','rustc']:run(n+'-version',[tools[n],'-vV'if n=='rustc'else'-V'])
        binary=base/'cache.exe';run('cache-build',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/optional_cache_drop.sg','--force-rebuild','-O','0','-o',binary]);obj=base/'cache.obj';run('cache-emit',[binary,obj],42,True)
        old_revision='5db2449';old_dir=base/'old-consumer';old_dir.mkdir();oldmap=env['SENGOO_MODULE_MAP'];pairs={k:v for k,v in(item.split('=',1)for item in oldmap.split(';'))};history={}
        for name in ['mir','mir_validate']:
            cmd=['git','show',old_revision+':selfhost/src/'+name+'.sg'];raw=subprocess.check_output(cmd,cwd=ROOT);p=old_dir/(name+'.sg');p.write_bytes(raw);inputs[str(p)]=sha(p);history[name]={'path':str(p),'sha256':sha(p),'command':cmd};pairs['selfhost_'+name]=str(p)
        env['SENGOO_MODULE_MAP']=';'.join(k+'='+v for k,v in pairs.items());old=old_dir/'boundary.exe';run('old-consumer-build',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/optional_old_boundary.sg','--force-rebuild','-O','0','-o',old]);r=run('old-consumer-rejection',[old],0,True);assert r.stdout.strip()==b'old opcode27: add_instruction=5002; inst_typecheck=5003'and not r.stderr;env['SENGOO_MODULE_MAP']=oldmap
        pairs={k:v for k,v in(item.split('=',1)for item in oldmap.split(';'))};cmd=['git','show',old_revision+':selfhost/src/native.sg'];raw=subprocess.check_output(cmd,cwd=ROOT);p=old_dir/'native.sg';p.write_bytes(raw);inputs[str(p)]=sha(p);history['native']={'path':str(p),'sha256':sha(p),'command':cmd};pairs['selfhost_native']=str(p);env['SENGOO_MODULE_MAP']=';'.join(k+'='+v for k,v in pairs.items());failed=old_dir/'cache.exe';run('old-cache-build',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/optional_cache_drop.sg','--force-rebuild','-O','0','-o',failed]);sentinel=base/'old-cache.obj';sentinel.write_bytes(b'old cache rejection must preserve this file');r=run('old-cache-rejection',[failed,sentinel],65,True);assert r.stdout.strip()==b'6301'and not r.stderr and sentinel.read_bytes()==b'old cache rejection must preserve this file';env['SENGOO_MODULE_MAP']=oldmap
        report['old_consumers']={'revision':subprocess.check_output(['git','rev-parse',old_revision],cwd=ROOT,text=True).strip(),'sources':history,'builder_code':5002,'instruction_code':5003,'layout_cache_code':6301}
        source=snapshot/'selfhost/tests/optional_cache_drop_probe.c';flags=['-std=c11','-O0','-I',qualified/'implementation/tools/stdlib'];dep=base/'before.d';run('cache-c-headers',[tools['clang'],*flags,'-M','-MT','optional_dependency','-MF',dep,source]);headers={str(p):sha(p)for p in dependencies(dep)};inputs.update(headers);cobj=base/'cache-probe.obj';after=base/'after.d';run('cache-c-build',[tools['clang'],*flags,'-MD','-MT','optional_dependency','-MF',after,'-c',source,'-o',cobj]);assert headers=={str(p):sha(p)for p in dependencies(after)};report['c_header_closure']=headers
        linker=tools['clang'].parent/'lld-link.exe';report['tools']['lld-link']={'path':str(linker),'sha256':sha(linker)};exe=base/'cache-probe.exe';linked=run('cache-c-link',[tools['clang'],'-fuse-ld=lld','-Wl,/verbose',cobj,obj,*providers,'-o',exe]);libs={}
        for m in re.finditer(rb'Reading ([^\r\n]+?\.lib)(?:\(|\r?$)',linked.stdout+linked.stderr,re.M):
            p=Path(m[1].decode('utf8')).resolve()
            if p.is_file():libs[str(p)]=sha(p)
        assert libs;inputs.update(libs);report['c_link_libraries']=libs
        run('cache-typed-drop-1000',[exe],42,True);run('cache-empty-drop-resource-negative',[exe,'empty'],78,True)
        for p in [binary,obj,old,failed,cobj,exe]:artifacts[str(p.relative_to(base))]=sha(p)
        report.update(result='passed',production_admitted=True,artifacts=artifacts,actual_typed_resource_scopes=1000,empty_destructor_is_resource_failure=True)
    except BaseException as error:report.update(result='failed',error=repr(error));raise
    finally:
        report.update(changed_sources=[n for n,h in inputs.items()if not Path(n).is_file()or sha(n)!=h],changed_tools=[n for n,value in report['tools'].items()if not Path(value['path']).is_file()or sha(value['path'])!=value['sha256']],checkout_changes=[str(p)for p,h in originals.items()if not p.is_file()or sha(p)!=h],changed_configs=[n for n,h in report['cargo_config_candidates'].items()if(sha(n)if Path(n).is_file()else None)!=h],changed_artifacts=[n for n,h in artifacts.items()if not(base/n).is_file()or sha(base/n)!=h],source_revision_end=revision(),completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
        if any(report[k]for k in ['changed_sources','changed_tools','checkout_changes','changed_configs','changed_artifacts']):report.update(result='failed',error='qualified input/output changed')
        save()
    assert report['result']=='passed';print(sha(base/'evidence.json'))
if __name__=='__main__':main()
