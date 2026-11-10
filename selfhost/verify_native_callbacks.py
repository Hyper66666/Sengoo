"""Qualify MIR28's physical C callback ABI; no collection/provider/Ref authority."""
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
    raw=path.read_text('utf8').replace('\\\n',' ');assert raw.startswith('callback_dependency:')
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
def inspect_object(raw):
    machine,count,stamp,symbol_at,symbol_count,optional,flags=struct.unpack_from('<HHIIIHH',raw)
    assert machine==0x8664 and stamp==0 and optional==0 and count>=3
    strings=raw[symbol_at+symbol_count*18:];assert len(strings)>=4 and int.from_bytes(strings[:4],'little')==len(strings)
    sections=[]
    for index in range(count):
        row=20+index*40;name=raw[row:row+8].rstrip(b'\0').decode('ascii');size,start=struct.unpack_from('<II',raw,row+16);reloc_at=struct.unpack_from('<I',raw,row+24)[0];relocs=struct.unpack_from('<H',raw,row+32)[0]
        assert start+size<=len(raw) and reloc_at+relocs*10<=len(raw)
        sections.append({'name':name,'size':size,'raw':raw[start:start+size],'relocations':[struct.unpack_from('<IIH',raw,reloc_at+i*10)for i in range(relocs)]})
    symbols={};index=0
    while index<symbol_count:
        at=symbol_at+index*18;name=raw[at:at+8]
        if name[:4]==b'\0'*4:
            offset=int.from_bytes(name[4:],'little');assert 4<=offset<len(strings);name=strings[offset:strings.index(0,offset)]
        else:name=name.rstrip(b'\0')
        value,section,ty,storage,aux=struct.unpack_from('<IhHBB',raw,at+8);assert aux==0
        symbols[name.decode('utf8')]={'value':value,'section':section,'type':ty,'storage':storage,'index':index};index+=1
    callbacks=['sg_cb_i64_move','sg_cb_i64_drop','sg_cb_owner_move','sg_cb_owner_drop']
    for name in [*callbacks,'sg_callbacks_probe']:assert symbols[name]['section']==1 and symbols[name]['type']==0x20
    text=sections[0]['raw'];begin=symbols['sg_callbacks_probe']['value'];end=len(text);leas=[];lea_symbols=[];producer_relocations=[]
    for at in range(begin,end-6):
        if text[at:at+3]==b'\x48\x8d\x05':
            matching=[(symbol,kind)for offset,symbol,kind in sections[0]['relocations']if offset==at+3]
            assert len(matching)==1 and matching[0][1]==4 and struct.unpack_from('<i',text,at+3)[0]==0
            symbol=next((name for name,value in symbols.items()if value['index']==matching[0][0]),None)
            assert symbol is not None
            leas.append(symbols[symbol]['value']);lea_symbols.append(symbol);producer_relocations.append(at+3)
    targets=[symbols[n]['value']for n in callbacks];assert lea_symbols==callbacks and leas==targets,(lea_symbols,leas,targets)
    for section in sections:
        for offset,symbol,kind in section['relocations']:assert symbol<symbol_count and offset+4<=section['size'] and kind in [3,4]
    return {'callbacks':{n:symbols[n]for n in callbacks},'actual_rip_relative_address_targets':leas,'function_address_relocation_offsets':producer_relocations,'function_address_symbols':lea_symbols,'section_names':[s['name']for s in sections],'all_callback_addresses_match_defined_symbols':True}

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for n in ['seed','clang','formatter','output']:parser.add_argument('--'+n,required=True,type=Path)
    args=parser.parse_args();base=args.output.resolve();assert base.is_relative_to(ROOT/'target');base.mkdir(parents=True,exist_ok=False)
    inputs={str(Path(__file__).resolve()):sha(__file__)}
    originals=closure([ROOT/'selfhost/tests/callbacks.sg',ROOT/'selfhost/tests/native.sg',ROOT/'selfhost/tests/abi.sg',ROOT/'selfhost/tests/mir.sg',ROOT/'selfhost/tests/callback_old_boundary.sg'])
    for directory in ['tools/stdlib']:
        originals.update({p:sha(p)for p in (ROOT/directory).rglob('*')if p.is_file()and not any(n in {'build','target','__pycache__'}for n in p.relative_to(ROOT/directory).parts)})
    for n in ['selfhost/tests/callback_probe.c','selfhost/tests/native_probe.c','selfhost/codegen_gate.py','selfhost/bootstrap.lock']:originals[ROOT/n]=sha(ROOT/n)
    snapshot=base/'implementation'
    for p,h in originals.items():
        raw=p.read_bytes();assert hashlib.sha256(raw).hexdigest()==h;copy=snapshot/p.relative_to(ROOT);copy.parent.mkdir(parents=True,exist_ok=True);copy.write_bytes(raw);assert sha(copy)==h;inputs[str(copy)]=h
    assert all(sha(p)==h for p,h in originals.items())
    for directory in ['runtime']:
        inputs.update({str(p):sha(p)for p in (ROOT/directory).rglob('*')if p.is_file()and not any(n in {'build','target','__pycache__'}for n in p.relative_to(ROOT/directory).parts)})
    for n in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:inputs[str(ROOT/n)]=sha(ROOT/n)
    for p in (ROOT/'openspec/changes/add-selfhost-native-backend').rglob('*.md'):
        if p.name!='tasks.md':
            raw=p.read_bytes();copy=snapshot/p.relative_to(ROOT);copy.parent.mkdir(parents=True,exist_ok=True);copy.write_bytes(raw);inputs[str(copy)]=sha(copy)
    tools={'seed':args.seed.resolve(),'clang':args.clang.resolve(),'formatter':args.formatter.resolve(),'python':Path(sys.executable)}
    for n in ['llvm-readobj','llvm-objdump','lld-link']:tools[n]=tools['clang'].parent/(n+'.exe')
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
    report={'schema':1,'stage':'mir28-physical-c-callback-erasure','result':'running','production_admitted':False,'m4_complete':False,'host':platform.platform(),'source_revision':revision(),'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sources':inputs,'qualified_checkout_sources':{str(p):h for p,h in originals.items()},'tools':{n:{'path':str(p),'sha256':sha(p)}for n,p in tools.items()},'steps':[],'limits':['physical raw C callback ABI only; no generated T/Drop/provider/constructor/Ref permission','MIR1 layout unchanged, 28 is an explicit consumer extension; 27 remains rejected','C independent probes and source-development C/Rust compiler runtime are recorded, not a completed native bootstrap'],'sdk_environment':{n:env.get(n)for n in ['INCLUDE','LIB','LIBPATH','VCToolsInstallDir','WindowsSdkDir','WindowsSDKVersion','VSINSTALLDIR','VCINSTALLDIR']}}
    configs={p/'.cargo'/n for p in [ROOT,*ROOT.parents]for n in ['config','config.toml']};configs.update(Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))/n for n in ['config','config.toml']);report['cargo_config_candidates']={}
    for p in configs:
        raw=p.read_bytes()if p.is_file()else None;report['cargo_config_candidates'][str(p)]=None if raw is None else hashlib.sha256(raw).hexdigest()
        if raw is not None:assert 'include'not in tomllib.loads(raw.decode('utf8'))
    def save():(base/'evidence.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    def run(name,command,code=0,empty=False):
        done=subprocess.run(list(map(str,command)),cwd=ROOT,env=pure if empty else env,capture_output=True,timeout=900);stdout=base/(name+'.stdout.log');stderr=base/(name+'.stderr.log');stdout.write_bytes(done.stdout);stderr.write_bytes(done.stderr);report['steps'].append({'name':name,'command':list(map(str,command)),'exit_code':done.returncode,'expected_exit_code':code,'empty_path':empty,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)});save();print(name,done.returncode,flush=True);assert done.returncode==code,(name,done.returncode,done.stdout[-800:],done.stderr[-3000:]);return done
    artifacts={}
    try:
        for n in ['cargo','rustc']:run(n+'-version',[tools[n],'-vV'])
        probe=base/'cargo-probe';probe.mkdir();(probe/'src').mkdir();(probe/'Cargo.toml').write_text('[package]\nname="callback_rustc_probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n',encoding='utf8');(probe/'src/lib.rs').write_text('pub fn answer()->i64{42}\n',encoding='utf8');(probe/'build.rs').write_text('fn main(){println!("cargo:warning=callback_rustc={}",std::env::var("RUSTC").unwrap());}\n',encoding='utf8')
        observed=run('cargo-actual-rustc',[tools['cargo'],'build','--offline','--verbose','--manifest-path',probe/'Cargo.toml','--target-dir',probe/'target']);combined=(observed.stdout+observed.stderr).decode('utf8');runs=[s for s in combined.splitlines()if s.lstrip().startswith('Running ')and str(tools['rustc']).casefold()in s.casefold()];assert len(runs)>=2 and 'callback_rustc='+str(tools['rustc'])in combined;report['cargo_actual_rustc_invocations']=runs
        for p in probe.rglob('*'):
            if p.is_file()and 'target'not in p.relative_to(probe).parts:inputs[str(p)]=sha(p)
        for n in ['callbacks','callback_old_boundary','native','abi','mir']:run('format-'+n,[tools['formatter'],'--check',snapshot/'selfhost/tests'/(n+'.sg')])
        for n in ['mir','mir_validate','native']:run('format-src-'+n,[tools['formatter'],'--check',snapshot/'selfhost/src'/(n+'.sg')])
        binary=base/'callbacks.exe';run('build-callbacks',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/callbacks.sg','--force-rebuild','-O','0','-o',binary])
        for flag in ['--fail-equality','--fail-truth']:
            r=run('assert-'+flag,[binary,flag],1,True);assert b'callbacks.sg'in r.stderr
        r=run('callbacks-first-resources',[binary],empty=True);assert r.stdout.strip()==b'callback MIR: 100 first-call success scopes and 26x5 failure scopes passed'and not r.stderr
        objects=[]
        for name in ['fresh-a','fresh-b']:
            directory=base/name;directory.mkdir();obj=directory/'callbacks.obj';r=run(name+'-emit',[binary,obj],empty=True);assert not r.stdout and not r.stderr;objects.append(obj);artifacts[str(obj.relative_to(base))]=sha(obj)
        assert objects[0].read_bytes()==objects[1].read_bytes();report['independent_object_inspection']=inspect_object(objects[0].read_bytes())
        original=objects[0].read_bytes();patched=bytearray(original);symbol_at,symbol_count=struct.unpack_from('<II',original,8);text_at=20;reloc_at=struct.unpack_from('<I',original,text_at+24)[0];relocs=struct.unpack_from('<H',original,text_at+32)[0];changed=next(reloc_at+i*10 for i in range(relocs)if struct.unpack_from('<I',original,reloc_at+i*10)[0]==report['independent_object_inspection']['function_address_relocation_offsets'][0]);struct.pack_into('<I',patched,changed+4,0)
        try:inspect_object(bytes(patched))
        except AssertionError:report['independent_object_inspection']['relocation_target_negative_control']=True
        else:raise AssertionError('independent callback relocation oracle did not reject mutation')
        run('llvm-object-inspection',[tools['llvm-readobj'],'--file-headers','--sections','--symbols','--relocations','--unwind',objects[0]])
        disasm=run('llvm-disassembly',[tools['llvm-objdump'],'-d',objects[0]]);assert b'sg_cb_owner_move'in disasm.stdout and b'sg_callbacks_probe'in disasm.stdout
        source=snapshot/'selfhost/tests/callback_probe.c';flags=['-std=c11','-Wall','-Wextra','-Werror','-Werror=date-time','-O0']
        before_dep=base/'before.d';run('c-before-headers',[tools['clang'],*flags,'-M','-MT','callback_dependency','-MF',before_dep,source]);headers={str(p):sha(p)for p in dependencies(before_dep)};inputs.update(headers)
        cobj=base/'callback_probe.obj';after_dep=base/'after.d';run('c-build',[tools['clang'],*flags,'-MD','-MT','callback_dependency','-MF',after_dep,'-c',source,'-o',cobj]);assert headers=={str(p):sha(p)for p in dependencies(after_dep)};report['c_header_closure']=headers
        exe=base/'callback_probe.exe';linked=run('c-link',[tools['clang'],'-fuse-ld=lld',cobj,objects[0],'-Wl,/verbose','-o',exe]);libraries={}
        for m in re.finditer(rb'Reading ([^\r\n]+?\.lib)(?:\(|\r?$)',linked.stdout+linked.stderr,re.M):
            p=Path(m[1].decode('utf8')).resolve()
            if p.is_file():libraries[str(p)]=sha(p)
        assert libraries,'lld verbose must disclose selected libraries';inputs.update(libraries);report['c_link_libraries']=libraries
        r=run('c-execute',[exe],42,True);assert r.stdout.strip()==b'C -> SG -> C -> SG callbacks: move8/owned16, exact release, aligned stack and data passed'and not r.stderr
        negatives=[];rejected=base/'rejected.obj';sentinel=b'previous callback object must survive'
        for mode in range(1,27):
            rejected.write_bytes(sentinel);r=run('reject-'+str(mode),[binary,rejected,chr(64+mode)],65,True);expected=5002 if mode==4 else 5004 if mode==5 else 5003 if mode in [3,9,19,24]else 5006;assert r.stdout.strip()==str(expected).encode()and not r.stderr and rejected.read_bytes()==sentinel;negatives.append({'case':mode,'diagnostic':expected,'first_call_resource_scopes':5})
        report['negative_cases']=negatives
        # Rebuild the old bounded consumers from the exact pre-extension Git revision.
        old_revision='f8019ce94db24b241071ee9ff0156a8b4a120e69';old_dir=base/'old-consumer';old_dir.mkdir();old_sources={};overrides={}
        for n in ['mir','mir_validate']:
            command=['git','show',old_revision+':selfhost/src/'+n+'.sg'];raw=subprocess.check_output(command,cwd=ROOT);dest=old_dir/(n+'.sg');dest.write_bytes(raw);inputs[str(dest)]=sha(dest);old_sources[n]={'command':command,'sha256':sha(dest),'path':str(dest)};overrides['selfhost_'+n]=dest
        oldmap=env['SENGOO_MODULE_MAP'];pairs={key:value for key,value in (item.split('=',1)for item in oldmap.split(';'))};pairs.update({k:str(v)for k,v in overrides.items()});env['SENGOO_MODULE_MAP']=';'.join(k+'='+v for k,v in pairs.items());old_binary=old_dir/'old-boundary.exe'
        run('build-old-consumers',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/callback_old_boundary.sg','--force-rebuild','-O','0','-o',old_binary]);env['SENGOO_MODULE_MAP']=oldmap
        r=run('old-consumer-rejection',[old_binary],empty=True);assert r.stdout.strip()==b'old add_instruction=5002; old inst_typecheck=5003'and not r.stderr
        report['old_consumer_baseline']={'revision':old_revision,'sources':old_sources,'builder_diagnostic':5002,'instruction_typecheck_diagnostic':5003};artifacts[str(old_binary.relative_to(base))]=sha(old_binary)
        # Existing native/ABI regression consumes only captured copies and the same pinned runtime/tool environment.
        regression=snapshot/'target/native-regression';run('existing-codegen-gate',[tools['python'],snapshot/'selfhost/codegen_gate.py','--seed',tools['seed'],'--clang',tools['clang'],'--formatter',tools['formatter'],'--output',regression])
        legacy=json.loads((regression/'evidence.json').read_text('utf8'));assert legacy['result']=='passed'and not legacy['changed_sources']and not legacy['changed_tools'];report['native_regression']={'report':str(regression/'evidence.json'),'sha256':sha(regression/'evidence.json'),'steps':len(legacy['steps'])}
        for n,h in legacy['artifacts'].items():assert sha(regression/n)==h;artifacts[str((regression/n).relative_to(base))]=h
        mir_binary=base/'mir-tests.exe';run('build-existing-mir',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/mir.sg','--force-rebuild','-O','0','-o',mir_binary]);r=run('existing-mir-tests',[mir_binary],empty=True);assert r.stdout.strip()==b'selfhost typed HIR/MIR: 10 roundtrip/lowering/cleanup and 30 CFG/SSA/ABI exact resource rounds passed'and not r.stderr
        for p in [binary,cobj,exe,mir_binary]:artifacts[str(p.relative_to(base))]=sha(p)
        report.update(result='passed',production_admitted=True,artifacts=artifacts,success_first_call_resource_scopes=100,rejection_first_call_resource_scopes=130,raw_object_deterministic=True)
    except BaseException as e:report.update(result='failed',error=repr(e));raise
    finally:
        report.update(changed_sources=[n for n,h in inputs.items()if not Path(n).is_file()or sha(n)!=h],changed_tools=[n for n,v in report['tools'].items()if not Path(v['path']).is_file()or sha(v['path'])!=v['sha256']],checkout_changes=[str(p)for p,h in originals.items()if not p.is_file()or sha(p)!=h],changed_configs=[n for n,h in report['cargo_config_candidates'].items()if(sha(n)if Path(n).is_file()else None)!=h],changed_artifacts=[n for n,h in artifacts.items()if not (base/n).is_file()or sha(base/n)!=h],source_revision_end=revision(),completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
        if any(report[k]for k in ['changed_sources','changed_tools','checkout_changes','changed_configs','changed_artifacts']):report.update(result='failed',error='qualified input/output changed')
        save()
    assert report['result']=='passed';print(sha(base/'evidence.json'))
if __name__=='__main__':main()
