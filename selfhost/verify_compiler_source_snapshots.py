"""Qualify actual SG compiler-owned sources and their whole-compiler frontier.

Ordinary compiler and collection authority requalification are separately bound
integration gates; this report does not close M3/M4/M6 or final SDK qualification.
"""
from __future__ import annotations
import argparse, datetime, hashlib, json, os, platform, re, shutil, subprocess, sys, time, tomllib
from pathlib import Path
from compiler_inputs import compiler_closure, module_map

ROOT=Path(__file__).resolve().parents[1]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def revision():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def rust_override(key):
    key=key.upper()
    return key.startswith(('RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','CARGO_BUILD_RUSTC','CARGO_BUILD_RUSTDOC','CARGO_PROFILE_')) or key in {'CARGO','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS','CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_RUSTDOCFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','CARGO_INCREMENTAL'} or key.startswith('CARGO_TARGET_') and key.endswith(('_RUSTFLAGS','_RUSTDOCFLAGS','_RUSTC'))
def dependencies(path):
    raw=Path(path).read_text('utf8').replace('\\\n',' ');assert raw.startswith('snapshot_dependency:');payload=raw.split(':',1)[1]
    words=[];word=[];i=0
    while i<len(payload):
        c=payload[i]
        if c=='\\' and i+1<len(payload) and payload[i+1] in ' \t#':word.append(payload[i+1]);i+=2
        elif c=='$' and i+1<len(payload) and payload[i+1]=='$':word.append('$');i+=2
        elif c.isspace():
            if word:words.append(''.join(word));word=[]
            i+=1
        else:word.append(c);i+=1
    if word:words.append(''.join(word))
    assert words
    return {str(Path(w).resolve()):sha(w)for w in words}

def main():
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--output',required=True,type=Path)
    ap.add_argument('--seed',required=True,type=Path)
    ap.add_argument('--clang',required=True,type=Path)
    ap.add_argument('--formatter',required=True,type=Path)
    ap.add_argument('--os-parent',type=Path,help='Optional supplementary historical OS evidence; current captured adapter is always exercised')
    ap.add_argument('--os-parent-pin')
    args=ap.parse_args();out=args.output.resolve();assert out.is_relative_to(ROOT/'target');out.mkdir(parents=True,exist_ok=False)
    sources={};origins={};snapshots={};tools={};configs={};artifacts={}
    report=dict(schema=1,stage='compiler-owned-retained-source-arena',result='running',source_revision=revision(),host=platform.platform(),started_at=datetime.datetime.now(datetime.timezone.utc).isoformat(),sources=sources,producer_origins=origins,producer_snapshots=snapshots,tools=tools,cargo_config_candidates=configs,artifacts=artifacts,steps=[],m3_complete=False,m6_complete=False,ordinary_and_collection_requalification_required=True)
    def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf8')
    def bind(path,h=None):
        path=Path(path).resolve();value=sha(path);assert h is None or value==h,str(path);sources[str(path)]=value;return value
    def run(name,command,env,expected=0,timeout=1800):
        start=time.monotonic();stdout=out/(name+'.stdout.log');stderr=out/(name+'.stderr.log')
        with stdout.open('wb')as o,stderr.open('wb')as e:done=subprocess.run(list(map(str,command)),cwd=out,env=env,stdout=o,stderr=e,timeout=timeout)
        artifacts[str(stdout.relative_to(out))]=sha(stdout);artifacts[str(stderr.relative_to(out))]=sha(stderr)
        report['steps'].append(dict(name=name,command=list(map(str,command)),exit_code=done.returncode,expected_exit_code=expected,elapsed_s=time.monotonic()-start,stdout_sha256=sha(stdout),stderr_sha256=sha(stderr)));save();print(name,done.returncode,flush=True)
        assert done.returncode==expected,(name,stdout.read_bytes()[-2500:],stderr.read_bytes()[-3000:])
        return stdout.read_bytes(),stderr.read_bytes()
    save()
    try:
        if args.os_parent is not None:
            assert args.os_parent_pin,'--os-parent requires --os-parent-pin'
            parent=args.os_parent.resolve();bind(parent,args.os_parent_pin);prior=json.loads(parent.read_bytes());assert prior['result'].startswith('passed') and not prior['input_guards']
            for n,h in prior['inputs'].items():bind(n,h)
            report['supplementary_historical_os_oracle']=dict(path=str(parent),sha256=args.os_parent_pin,scope=prior['scope'])
        else:assert args.os_parent_pin is None,'--os-parent-pin requires --os-parent'
        entries=[ROOT/'selfhost/tests/compiler_source_snapshots.sg',ROOT/'selfhost/tests/compiler_source_modules.sg',ROOT/'selfhost/tests/compiler_source_snapshot_integrity.sg',ROOT/'selfhost/tests/compiler_source_snapshot_faults.sg',ROOT/'selfhost/tools/native.sg']
        closure=compiler_closure(ROOT,entries);paths=set(closure)|{Path(__file__).resolve()}
        for folder in ['tools/stdlib','runtime']:
            paths.update(p for p in(ROOT/folder).rglob('*')if p.is_file()and not any(x in {'target','build','__pycache__'}for x in p.relative_to(ROOT/folder).parts))
        paths.update(ROOT/n for n in ['selfhost/bootstrap.lock','selfhost/reference-baseline.lock','selfhost/tests/compiler_source_snapshot_probe.c','selfhost/tests/compiler_snapshot_os_probe.py','Cargo.toml','Cargo.lock','rust-toolchain.toml'])
        paths.add(ROOT/'selfhost/tests/compiler_snapshot_faults.c')
        paths.update(p for p in (ROOT/'openspec/changes/add-selfhost-compiler-source-snapshots').rglob('*.md') if p.name not in {'tasks.md','checkpoint.md'})
        captured=out/'implementation'
        for p in sorted(paths):
            h=sha(p);dest=captured/p.relative_to(ROOT);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(p.read_bytes());assert sha(dest)==h;origins[str(p)]=h;snapshots[str(dest)]=h;bind(dest,h)
        report['actual_compiler_imports']={str(p.relative_to(ROOT)):h for p,h in closure.items()}
        test_package=captured/'selfhost/tests/Sengoo.toml';test_package.write_text('[package]\nname="compiler-source-snapshot-tests"\nversion="0.0.1"\nedition="2026"\n');bind(test_package)
        native=captured/'selfhost/tests/native/qualification_probe.c';native.parent.mkdir();native.write_bytes((captured/'selfhost/tests/compiler_source_snapshot_probe.c').read_bytes());bind(native)
        lock=json.loads((captured/'selfhost/bootstrap.lock').read_bytes())
        for n in ['seed','clang','formatter']:
            p=getattr(args,n).resolve();assert sha(p)==lock[n]['sha256'];tools[n]=dict(path=str(p),sha256=bind(p))
        tools['python']=dict(path=sys.executable,sha256=bind(sys.executable))
        env={k:v for k,v in os.environ.items()if not rust_override(k)and not k.upper().startswith(('SENGOO_','SGPLATFORM_','SDL2_','PYTHON','LLDB_'))and k.upper()not in {'CL','_CL_','LINK','_LINK_'}}
        env['PATH']=str(args.clang.resolve().parent)+os.pathsep+env.get('PATH','')
        rustup=Path(shutil.which('rustup',path=env['PATH'])).resolve();tools['rustup']=dict(path=str(rustup),sha256=bind(rustup))
        for n in ['rustc','cargo']:
            p=Path(subprocess.check_output([rustup,'which',n],env=env,text=True).strip()).resolve();assert sha(p)==lock[n+'_sha256'];tools[n]=dict(path=str(p),sha256=bind(p))
        env.update(RUSTC=tools['rustc']['path'],CARGO=tools['cargo']['path'],CARGO_BUILD_RUSTC=tools['rustc']['path'],RUSTUP_TOOLCHAIN=Path(tools['rustc']['path']).parent.parent.name,RUSTC_WRAPPER='',RUSTC_WORKSPACE_WRAPPER='',CARGO_BUILD_RUSTC_WRAPPER='',CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='',SENGOO_STDLIB=str(captured/'tools/stdlib'),SENGOO_RUNTIME=str(captured/'tools/stdlib/runtime.c'),SENGOO_CLANG=tools['clang']['path'],SENGOO_MODULE_MAP=module_map(ROOT,closure,captured))
        env['PATH']=str(Path(tools['cargo']['path']).parent)+os.pathsep+env['PATH'];pure=dict(env);pure['PATH']=''
        for n in ['kernel32.dll','ntdll.dll','KernelBase.dll']:
            p=Path(os.environ['SystemRoot'])/'System32'/n;tools[n]=dict(path=str(p),sha256=bind(p))
        candidates={d/'.cargo'/n for d in{ROOT,*ROOT.parents,out,*out.parents,captured}for n in['config','config.toml']};candidates.update(Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))/n for n in ['config','config.toml'])
        for p in candidates:
            configs[str(p)]=sha(p)if p.is_file()else None
            if p.is_file():assert 'include'not in tomllib.loads(p.read_text('utf8'));bind(p)
        probe=out/'cargo-probe';(probe/'src').mkdir(parents=True);(probe/'Cargo.toml').write_text('[package]\nname="compiler_snapshot_rustc_probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n');(probe/'src/lib.rs').write_text('pub fn answer()->i64{42}\n');(probe/'build.rs').write_text('fn main(){println!("cargo:warning=snapshot_actual_rustc={}",std::env::var("RUSTC").unwrap());}\n')
        for p in [probe/'Cargo.toml',probe/'src/lib.rs',probe/'build.rs']:bind(p)
        o,e=run('actual-rustc',[tools['cargo']['path'],'build','--offline','--verbose','--manifest-path',probe/'Cargo.toml','--target-dir',probe/'target'],env);text=(o+e).decode('utf8');assert 'snapshot_actual_rustc='+tools['rustc']['path']in text and sum(line.lstrip().startswith('Running ')and tools['rustc']['path'].casefold()in line.casefold()for line in text.splitlines())>=2
        report['c_headers']={}
        for p in [captured/'selfhost/compiler_support/native/source_snapshot_os.c',captured/'selfhost/compiler_support/native/source_snapshot_memory.c',native]:
            before=out/(p.stem+'-before.d');after=out/(p.stem+'-after.d');obj=out/(p.stem+'.obj');flags=['-std=c11','-O2','-Wall','-Wextra','-Werror','-Werror=date-time']
            run(p.stem+'-headers',[args.clang.resolve(),*flags,'-M','-MT','snapshot_dependency','-MF',before,p],env);headers=dependencies(before)
            for n,h in headers.items():bind(n,h)
            run(p.stem+'-header-compile',[args.clang.resolve(),*flags,'-MD','-MT','snapshot_dependency','-MF',after,'-c',p,'-o',obj],env);assert dependencies(after)==headers;artifacts[obj.name]=sha(obj);report['c_headers'][str(p)]=headers
        # The seed retains ordinary source-runtime linkage; bind its actual
        # clang-selected linker and conservative library search directories.
        o,e=run('actual-linker-selection',[args.clang.resolve(),'-###',out/'qualification_probe.obj','-o',out/'selection.exe'],env);words=re.findall(r'"([^"\r\n]+)"',(o+e).decode('utf8'));search=set()
        for word in words:
            if word.lower().endswith(('link.exe','lld-link.exe'))and Path(word).is_file():tools['system-linker']=dict(path=word,sha256=bind(word))
            if word.lower().startswith('-libpath:'):search.add(Path(word.split(':',1)[1]))
        assert 'system-linker'in tools and search
        report['conservative_system_libraries']={str(p.resolve()):bind(p)for d in search for p in d.glob('*.lib')}
        # Independently exercise the exact retained-handle adapter consumed by
        # this compiler build, using its just-compiled objects and header closure.
        exports=['init','open','close','read','metadata','path','full_path','drive_type','seek']
        definition=out/'snapshot-os.def';definition.write_text('LIBRARY compiler-snapshot-os\nEXPORTS\n'+'\n'.join('sengoo_compiler_snapshot_'+n for n in exports)+'\n');bind(definition)
        kernel_candidates=[p for d in sorted(search) for p in d.glob('kernel32.lib')];assert kernel_candidates
        kernel_library=kernel_candidates[0].resolve();bind(kernel_library)
        dll=out/'compiler-snapshot-os.dll';import_library=out/'compiler-snapshot-os.lib'
        run('captured-os-dll',[tools['system-linker']['path'],'/dll','/noentry','/nodefaultlib','/machine:x64','/Brepro','/out:'+str(dll),'/implib:'+str(import_library),'/def:'+str(definition),out/'source_snapshot_os.obj',out/'source_snapshot_memory.obj',kernel_library],env)
        artifacts[dll.name]=sha(dll);artifacts[import_library.name]=sha(import_library)
        candidate=out/'snapshot-os-candidate.json';candidate.write_text(json.dumps({'sources':[{'path':str(p),'sha256':sha(p)}for p in [captured/'selfhost/compiler_support/native/source_snapshot_os.c',captured/'selfhost/compiler_support/native/source_snapshot_memory.c']]},indent=2)+'\n');bind(candidate)
        (captured/'target').mkdir();os_output=captured/'target/independent-os'
        run('captured-os-oracle',[sys.executable,captured/'selfhost/tests/compiler_snapshot_os_probe.py','--candidate',candidate,'--dll',dll,'--dll-pin',sha(dll),'--output',os_output],pure)
        os_report=os_output/'evidence.json';oracle=json.loads(os_report.read_bytes());assert oracle['result']=='passed' and not oracle['input_guards']
        assert any(c['name']=='same_handle_seek_reread_and_invalid_offsets' and c['result']=='passed' for c in oracle['cases'])
        for n,h in oracle['inputs'].items():bind(n,h)
        bind(os_report);report['independent_os_oracle']=dict(path=str(os_report),sha256=sha(os_report),scope=oracle['scope'],cases=len(oracle['cases']),captured_adapter_sources=candidate.as_posix())
        fixtures=out/'fixtures';fixtures.mkdir();data=b'def x(){ 42; }\n';assert len(data)==15
        for i in range(1025):p=fixtures/('file-'+str(i)+'.sg');p.write_bytes(data);bind(p)
        (fixtures/'alias.sg').hardlink_to(fixtures/'file-0.sg');bind(fixtures/'alias.sg')
        for name,data in [('invalid.sg',b'\xff'),('payload-nul.sg',b'A\0B')]:p=fixtures/name;p.write_bytes(data);bind(p)
        graph=out/'graph';graph.mkdir();(graph/'stdlib').mkdir()
        for i in range(33):p=graph/('m'+str(i)+'.sg');p.write_text('def value_'+str(i)+'()->i64{42}\n');bind(p)
        body=''.join('import m'+str(i)+';\n'for i in range(33))+'def main()->i64{42}\n'
        for name,text in [('root.sg',body),('duplicate.sg',body+'import m0;\n')]:p=graph/name;p.write_text(text);bind(p)
        (graph/'invalid.sg').write_bytes(b'\xff');bind(graph/'invalid.sg')
        for name,directory in [('compiler_source_snapshots',fixtures),('compiler_source_modules',graph)]:
            entry=captured/'selfhost/tests'/(name+'.sg');run(name+'-format',[args.formatter.resolve(),'--check',entry],env)
            exe=out/(name+'.exe');run(name+'-build',[args.seed.resolve(),'--runtime-mode','source-development','build',entry,'--force-rebuild','-O','0','-o',exe],env);artifacts[exe.name]=sha(exe)
            o,e=run(name+'-three-first-scopes',[exe,directory],pure,42);assert not o and not e
        integrity=out/'integrity';integrity.mkdir();(integrity/'stdlib').mkdir()
        original=b'def main()->i64{42}\n';poison=b'def main()->i64{43}\n'
        assert len(original)==len(poison)==20
        poison_sha=hashlib.sha256(poison).hexdigest();assert poison_sha=='1771cef60c5239f5fb0e4be5b5c3ab216ece5738284b2bed9161b615546cdce4'
        fixture=integrity/'original.sg';fixture.write_bytes(original);bind(fixture)
        entry=captured/'selfhost/tests/compiler_source_snapshot_integrity.sg'
        assert poison_sha in entry.read_text('utf8')
        run('compiler_source_snapshot_integrity-format',[args.formatter.resolve(),'--check',entry],env)
        exe=out/'compiler_source_snapshot_integrity.exe';run('compiler_source_snapshot_integrity-build',[args.seed.resolve(),'--runtime-mode','source-development','build',entry,'--force-rebuild','-O','0','-o',exe],env);artifacts[exe.name]=sha(exe)
        o,e=run('compiler_source_snapshot_integrity-three-first-scopes',[exe,fixture,integrity/'stdlib'],pure,42);assert not o and not e
        report['integrity_controls']=dict(resource_scopes=3,matching_poison_digest=poison_sha,fixture_sha256=sha(fixture),physical_contents_identity_paths_metadata=True,malformed_source_copy_rejects_without_output_write=True,restored_source_reaccepted=True)
        # Test-only responses wrap the exact captured OS adapter. Keep the
        # unmodified compiler qualification snapshot and all SG policy intact.
        fault_root=out/'fault-implementation'
        fault_entry=ROOT/'selfhost/tests/compiler_source_snapshot_faults.sg'
        fault_closure=compiler_closure(ROOT,[fault_entry])
        for p in fault_closure:
            saved=captured/p.relative_to(ROOT);dest=fault_root/p.relative_to(ROOT)
            dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(saved.read_bytes());bind(dest,sha(saved))
        fault_os=fault_root/'selfhost/compiler_support/native/source_snapshot_os.c'
        original_os=fault_os.with_name('compiler_snapshot_os_original.inc')
        original_os.write_bytes(fault_os.read_bytes());bind(original_os)
        wrapper=captured/'selfhost/tests/compiler_snapshot_faults.c'
        fault_os.write_bytes(wrapper.read_bytes());sources[str(fault_os)]=sha(fault_os)
        report['fault_adapter_derivation']=dict(original=str(original_os),original_sha256=sha(original_os),wrapper=str(wrapper),wrapper_sha256=sha(wrapper),compiled=str(fault_os),compiled_sha256=sha(fault_os),scope='Test-only read/metadata responses; production SG and original retained-handle adapter unchanged')
        fault_package=fault_root/'selfhost/tests/Sengoo.toml';fault_package.write_bytes(test_package.read_bytes());bind(fault_package)
        fault_probe=fault_root/'selfhost/tests/native/qualification_probe.c';fault_probe.parent.mkdir();fault_probe.write_bytes(native.read_bytes());bind(fault_probe)
        before=out/'fault-before.d';after=out/'fault-after.d';obj=out/'fault-adapter.obj'
        run('fault-adapter-headers',[args.clang.resolve(),'-M','-MT','snapshot_dependency','-MF',before,fault_os],env);headers=dependencies(before)
        for n,h in headers.items():bind(n,h)
        run('fault-adapter-header-compile',[args.clang.resolve(),'-std=c11','-O2','-Wall','-Wextra','-Werror','-MD','-MT','snapshot_dependency','-MF',after,'-c',fault_os,'-o',obj],env);assert dependencies(after)==headers;artifacts[obj.name]=sha(obj);report['c_headers'][str(fault_os)]=headers
        fault_env=dict(env,SENGOO_MODULE_MAP=module_map(ROOT,fault_closure,fault_root))
        entry=fault_root/fault_entry.relative_to(ROOT);run('compiler_source_snapshot_faults-format',[args.formatter.resolve(),'--check',entry],env)
        exe=out/'compiler_source_snapshot_faults.exe';run('compiler_source_snapshot_faults-build',[args.seed.resolve(),'--runtime-mode','source-development','build',entry,'--force-rebuild','-O','0','-o',exe],fault_env);artifacts[exe.name]=sha(exe)
        o,e=run('compiler_source_snapshot_faults-30-resource-scopes',[exe,fixture],pure,42);assert not o and not e
        report['fault_controls']=dict(resource_scopes=30,admission_and_revalidation=['early-eof','oversized-read-count','changed-second-metadata','fragmented-read-positive','missing-final-eof'],restore_after_failure=True)
        native_entry=captured/'selfhost/tools/native.sg';compiler=out/'native.exe';run('real-compiler-build',[args.seed.resolve(),'--runtime-mode','source-development','build',native_entry,'--force-rebuild','-O','0','-o',compiler],env);artifacts[compiler.name]=sha(compiler)
        obj=out/'self-input.obj';o,e=run('whole-compiler-self-input',[compiler,native_entry,obj],pure,65,2400);diagnostic=json.loads(o);assert not e and not obj.exists() and diagnostic['ok']is False and diagnostic['phase']=='semantics';report['self_input_frontier']=diagnostic
        report.update(result='passed-compiler-source-arena-boundary',independent_capacity=1024,unique_overflow=1025,public_snapshot_limit_unchanged=32,caller_graph_limit_preserved=1000000,caller_bytes_preserved=1073741824,resource_scopes=10,source_hash_identity_comparisons=99,embedded_nul_hash_comparisons=3,limits=['Current actual compiler reaches its recorded semantic rejection; no native self-build success','Standalone arena plus real module graph scopes restore six resource observations including OS handles','Actual platform adapter has independent OS oracle; seed retains qualified development source runtime, system linker/library manifest is conservative and not final SDK qualification','Ordinary and collection/HIR authority regressions must be separately requalified against these exact producer bytes'])
    except BaseException as error:report.update(result='failed',failure=repr(error));raise
    finally:
        report.update(changed_sources=[p for p,h in sources.items()if not Path(p).is_file()or sha(p)!=h],changed_origins=[p for p,h in origins.items()if not Path(p).is_file()or sha(p)!=h],changed_snapshots=[p for p,h in snapshots.items()if sha(p)!=h],changed_tools=[n for n,v in tools.items()if sha(v['path'])!=v['sha256']],changed_configs=[p for p,h in configs.items()if(sha(p)if Path(p).is_file()else None)!=h],changed_artifacts=[n for n,h in artifacts.items()if sha(out/n)!=h],source_revision_end=revision(),completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
        if any(report[k]for k in ['changed_sources','changed_origins','changed_snapshots','changed_tools','changed_configs','changed_artifacts']):report['result']='failed'
        save();print(sha(out/'evidence.json'),flush=True)
    return 0 if report['result']=='passed-compiler-source-arena-boundary'else 1
if __name__=='__main__':raise SystemExit(main())
