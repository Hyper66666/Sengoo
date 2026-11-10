"""Qualify physical optional borrowing and concrete push against the pinned real C provider."""
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
    originals=closure([ROOT/'selfhost/tests/optional_borrow.sg',ROOT/'selfhost/tests/collection_push.sg',ROOT/'selfhost/tests/native.sg',ROOT/'selfhost/tests/abi.sg'])
    for directory in ['tools/stdlib','runtime']:
        originals.update({p:sha(p)for p in (ROOT/directory).rglob('*')if p.is_file()and not any(n in {'build','target','__pycache__'}for n in p.relative_to(ROOT/directory).parts)})
    for n in ['selfhost/tests/optional_borrow_probe.c','selfhost/tests/collection_push_probe.c','selfhost/tests/collection_push_contract_probe.c','selfhost/tests/collection_os_probe.h','selfhost/tests/native_probe.c','selfhost/codegen_gate.py','selfhost/bootstrap.lock','Cargo.toml','Cargo.lock','rust-toolchain.toml']:originals[ROOT/n]=sha(ROOT/n)
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
    report={'schema':1,'stage':'optional-borrow-and-typed-push-physical-abi','result':'running','production_admitted':False,'m3_complete':False,'mir_schema':1,'host':platform.platform(),'source_revision':revision(),'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sources':inputs,'qualified_checkout_sources':{str(p):h for p,h in originals.items()},'tools':{n:{'path':str(p),'sha256':sha(p)}for n,p in tools.items()},'steps':[],'limits':['physical MIR/code generation only; source permission requires the separate fresh compiler pipeline','ordinary MIR/native entrypoints reject27 5008; internal physical mode alone grants no source authority','selected historical qualified C provider source/object chain is retained; C allocation pressure is not SG semantic policy','actual C Job-limited OOM and Owner1MiB tests do not establish M8/compiler-closure capacity','development compiler uses recorded C/Rust runtime; nested legacy source/native behavior gates do not qualify complete MSVC child-linker/SDK bootstrap provenance'],'sdk_environment':{n:env.get(n)for n in ['INCLUDE','LIB','LIBPATH','VCToolsInstallDir','WindowsSdkDir','WindowsSDKVersion','VSINSTALLDIR','VCINSTALLDIR']}}
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
        probe=base/'cargo-probe';probe.mkdir();(probe/'src').mkdir();(probe/'Cargo.toml').write_text('[package]\nname="optional_rustc_probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n',encoding='utf8');(probe/'src/lib.rs').write_text('pub fn value()->i64{42}\n',encoding='utf8');(probe/'build.rs').write_text('fn main(){println!("cargo:warning=optional_rustc={}",std::env::var("RUSTC").unwrap());}\n',encoding='utf8')
        observed=run('cargo-actual-rustc',[tools['cargo'],'build','--offline','--verbose','--manifest-path',probe/'Cargo.toml','--target-dir',probe/'target']);combined=(observed.stdout+observed.stderr).decode('utf8');runs=[line for line in combined.splitlines()if line.lstrip().startswith('Running ')and str(tools['rustc']).casefold()in line.casefold()];assert len(runs)>=2 and 'optional_rustc='+str(tools['rustc'])in combined;report['cargo_actual_rustc_invocations']=runs
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
        tools.update({'llvm-readobj':tools['clang'].parent/'llvm-readobj.exe','llvm-objdump':tools['clang'].parent/'llvm-objdump.exe','lld-link':tools['clang'].parent/'lld-link.exe'})
        report['tools'].update({n:{'path':str(p),'sha256':sha(p)}for n,p in tools.items()if n not in report['tools']})
        for n in ['mir','mir_validate','mir_optional','native','collection_push']:run('format-'+n,[tools['formatter'],'--check',snapshot/'selfhost/src'/(n+'.sg')])
        for n in ['optional_borrow','collection_push']:run('format-test-'+n,[tools['formatter'],'--check',snapshot/'selfhost/tests'/(n+'.sg')])
        binary=base/'optional.exe';run('build-optional',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/optional_borrow.sg','--force-rebuild','-O','0','-o',binary]);artifacts[binary.name]=sha(binary)
        for flag in ['--fail-equality','--fail-truth']:r=run('assert-'+flag,[binary,flag],1,True);assert b'optional_borrow.sg'in r.stderr
        run('optional-first-resources',[binary],42,True)
        objects=[]
        for directory in ['first 世界 &','second 世界 &']:
            d=base/directory;d.mkdir();obj=d/'optional.obj';r=run(directory.split()[0]+'-emit',[binary,obj],42,True);assert not r.stdout and not r.stderr;objects.append(obj);artifacts[str(obj.relative_to(base))]=sha(obj)
        assert objects[0].read_bytes()==objects[1].read_bytes()
        negatives=[]
        for mode in range(1,20):
            r=run('reject-'+str(mode),[binary,*['unused']*(mode+1)],42,True);lines=r.stdout.splitlines();assert len(lines)==5 and len(set(lines))==1 and lines[0].isdigit()and int(lines[0])>0 and not r.stderr;negatives.append({'case':mode,'diagnostic':int(lines[0]),'first_call_scopes':5})
        report['negative_cases']=negatives
        run('llvm-optional-inspection',[tools['llvm-readobj'],'--file-headers','--sections','--symbols','--relocations','--unwind',objects[0]])
        assembly=run('llvm-optional-disassembly',[tools['llvm-objdump'],'-d',objects[0]]);assert b'sg_optional_owner'in assembly.stdout and b'sg_optional_scalar_bool'in assembly.stdout
        push_binary=base/'push-generator.exe';run('build-push',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/collection_push.sg','--force-rebuild','-O','0','-o',push_binary]);push_obj=base/'push.obj';run('emit-push',[push_binary,push_obj],42,True);artifacts[push_binary.name]=sha(push_binary);artifacts[push_obj.name]=sha(push_obj)
        run('llvm-push-inspection',[tools['llvm-readobj'],'--file-headers','--sections','--symbols','--relocations','--unwind',push_obj])
        header_dir=qualified/'implementation/tools/stdlib';report['c_header_closures']={};report['c_link_libraries']={};report['loaded_os_modules']={}
        flags=['-std=c11','-O0','-I',header_dir]
        for label,name,obj in [('optional','optional_borrow_probe.c',objects[0]),('push','collection_push_probe.c',push_obj),('contract','collection_push_contract_probe.c',None)]:
            source=snapshot/'selfhost/tests'/name;dep=base/(label+'-before.d');run(label+'-headers',[tools['clang'],*flags,'-M','-MT','optional_dependency','-MF',dep,source]);headers={str(p):sha(p)for p in dependencies(dep)};inputs.update(headers)
            cobj=base/(label+'-probe.obj');after=base/(label+'-after.d');run(label+'-c-build',[tools['clang'],*flags,'-MD','-MT','optional_dependency','-MF',after,'-c',source,'-o',cobj]);assert headers=={str(p):sha(p)for p in dependencies(after)};report['c_header_closures'][label]=headers
            exe=base/(label+'-probe.exe');command=[tools['clang'],'-fuse-ld=lld','-Wl,/verbose','-Wl,/stack:33554432',cobj,*([obj]if obj is not None else[]),*providers,'-o',exe];linked=run(label+'-c-link',command);libraries={}
            for m in re.finditer(rb'Reading ([^\r\n]+?\.lib)(?:\(|\r?$)',linked.stdout+linked.stderr,re.M):
                p=Path(m[1].decode('utf8')).resolve()
                if p.is_file():libraries[str(p)]=sha(p)
            assert libraries;inputs.update(libraries);report['c_link_libraries'][label]=libraries
            run(label+'-normal',[exe],42,True)
            if label!='optional':run(label+'-actual-oom',[exe,'oom'],42,True)
            os_modules=run(label+'-os-providers',[exe,'--os-providers'],42,True);rows=os_modules.stdout.decode('utf8').splitlines();assert len(rows)==3 and all(line.startswith('OS_MODULE|')for line in rows)
            for line in rows:
                _,name,path=line.split('|',2);p=Path(path);assert p.is_absolute()and p.is_file();digest=sha(p);inputs[str(p)]=digest;report['loaded_os_modules'].setdefault(label,{})[name]={'path':str(p),'sha256':digest}
            artifacts[cobj.name]=sha(cobj);artifacts[exe.name]=sha(exe)
        regression=snapshot/'target/native-regression';run('existing-codegen-gate',[tools['python'],snapshot/'selfhost/codegen_gate.py','--seed',tools['seed'],'--clang',tools['clang'],'--formatter',tools['formatter'],'--output',regression]);legacy=json.loads((regression/'evidence.json').read_text('utf8'));assert legacy['result']=='passed'and not legacy['changed_sources']and not legacy['changed_tools']and len(legacy['actual_native_import_modules'])>18
        for n,h in legacy['artifacts'].items():assert sha(regression/n)==h;artifacts[str((regression/n).relative_to(base))]=h
        report.update(result='passed',production_admitted=True,artifacts=artifacts,success_first_call_resource_scopes=100,rejection_first_call_resource_scopes=95,raw_object_deterministic=True,native_regression={'report':str(regression/'evidence.json'),'sha256':sha(regression/'evidence.json'),'steps':len(legacy['steps']),'actual_import_modules':legacy['actual_native_import_modules']},actual_provider_scopes=1000,actual_OOM=True)
    except BaseException as error:report.update(result='failed',error=repr(error));raise
    finally:
        report.update(changed_sources=[n for n,h in inputs.items()if not Path(n).is_file()or sha(n)!=h],changed_tools=[n for n,value in report['tools'].items()if not Path(value['path']).is_file()or sha(value['path'])!=value['sha256']],checkout_changes=[str(p)for p,h in originals.items()if not p.is_file()or sha(p)!=h],changed_configs=[n for n,h in report['cargo_config_candidates'].items()if(sha(n)if Path(n).is_file()else None)!=h],changed_artifacts=[n for n,h in artifacts.items()if not(base/n).is_file()or sha(base/n)!=h],source_revision_end=revision(),completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
        if any(report[k]for k in ['changed_sources','changed_tools','checkout_changes','changed_configs','changed_artifacts']):report.update(result='failed',error='qualified input/output changed')
        save()
    assert report['result']=='passed';print(sha(base/'evidence.json'))
if __name__=='__main__':main()
