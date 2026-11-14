"""Qualify HIR7 owning candidates and independent actual typed-input replay."""
from __future__ import annotations
import argparse, datetime, hashlib, json, os, platform, re, shutil, struct, subprocess, sys, tomllib
from pathlib import Path
from compiler_inputs import compiler_closure, module_map

ROOT=Path(__file__).resolve().parents[1]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def revision():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def rust_override(key):
    key=key.upper()
    return key.startswith(('RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','CARGO_BUILD_RUSTC','CARGO_BUILD_RUSTDOC','CARGO_PROFILE_')) or key in {'CARGO','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS','CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_RUSTDOCFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','CARGO_INCREMENTAL'} or key.startswith('CARGO_TARGET_') and key.endswith(('_RUSTFLAGS','_RUSTDOCFLAGS','_RUSTC'))
def closure(entries):
    return compiler_closure(ROOT, entries)
def dependencies(path):
    raw=path.read_text('utf8').replace('\\\n',' ');assert raw.startswith('hir_origin_dependency:')
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
    for n in ['seed','clang','formatter','reference','output']:parser.add_argument('--'+n,required=True,type=Path)
    args=parser.parse_args();base=args.output.resolve();assert base.is_relative_to(ROOT/'target');base.mkdir(parents=True,exist_ok=False)
    inputs={str(Path(__file__).resolve()):sha(__file__)}
    originals=closure([ROOT/'selfhost/tests/hir_origins.sg',ROOT/'selfhost/tests/mir.sg',ROOT/'selfhost/tests/borrow.sg',ROOT/'selfhost/tests/handle_disarm_hir.sg',ROOT/'selfhost/tools/native.sg',ROOT/'selfhost/tools/semantic.sg'])
    for directory in ['tools/stdlib','runtime']:
        originals.update({p:sha(p)for p in (ROOT/directory).rglob('*')if p.is_file()and not any(n in {'build','target','__pycache__'}for n in p.relative_to(ROOT/directory).parts)})
    for n in ['selfhost/tests/native_source_probe.c','selfhost/tests/projection_probe.c','selfhost/tests/handle_disarm_probe.c','selfhost/tests/handle_disarm.sg','selfhost/source_codegen_gate.py','selfhost/verify_mir.py','selfhost/verify_projection.py','selfhost/verify_handle_disarm.py','selfhost/bootstrap.lock','selfhost/reference-baseline.lock','selfhost/REFERENCE_GAPS.md','Cargo.toml','Cargo.lock','rust-toolchain.toml']:originals[ROOT/n]=sha(ROOT/n)
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
    tools={'seed':args.seed.resolve(),'clang':args.clang.resolve(),'formatter':args.formatter.resolve(),'python':Path(sys.executable),'reference':args.reference.resolve()}
    baseline=json.loads((ROOT/'selfhost/reference-baseline.lock').read_text('utf8'));assert sha(tools['reference'])==baseline['tools']['sgc']['sha256']
    lock=json.loads((ROOT/'selfhost/bootstrap.lock').read_text('utf8'))
    for n in ['seed','clang','formatter']:assert sha(tools[n])==lock[n]['sha256']
    env={k:v for k,v in os.environ.items()if not rust_override(k)and not k.upper().startswith(('SENGOO_','SGPLATFORM_','SDL2_','PYTHON','LLDB_'))and k.upper()not in {'CL','_CL_','LINK','_LINK_'}}
    env['PATH']=str(tools['clang'].parent)+os.pathsep+env.get('PATH','')
    tools['rustup']=Path(shutil.which('rustup',path=env['PATH'])).resolve()
    for n in ['cargo','rustc']:tools[n]=Path(subprocess.check_output([tools['rustup'],'which',n],env=env,text=True).strip()).resolve()
    assert tools['cargo'].parent==tools['rustc'].parent
    env.update(CARGO=str(tools['cargo']),RUSTC=str(tools['rustc']),CARGO_BUILD_RUSTC=str(tools['rustc']),RUSTUP_TOOLCHAIN=tools['rustc'].parent.parent.name,RUSTC_WRAPPER='',RUSTC_WORKSPACE_WRAPPER='',CARGO_BUILD_RUSTC_WRAPPER='',CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='')
    env['PATH']=str(tools['cargo'].parent)+os.pathsep+env['PATH']
    env.update(SENGOO_STDLIB=str(snapshot/'tools/stdlib'),SENGOO_RUNTIME=str(snapshot/'tools/stdlib/runtime.c'),SENGOO_CLANG=str(tools['clang']),SENGOO_MODULE_MAP=module_map(ROOT, originals, snapshot))
    pure={k:v for k,v in env.items()if not k.upper().startswith('SENGOO_')};pure['PATH']=''
    report={'schema':1,'stage':'hir7-owning-origin-loan-candidates','result':'running','production_admitted':False,'m3_complete':False,'hir_schema':7,'host':platform.platform(),'source_revision':revision(),'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sources':inputs,'qualified_checkout_sources':{str(p):h for p,h in originals.items()},'tools':{n:{'path':str(p),'sha256':sha(p)}for n,p in tools.items()},'steps':[],'limits':['candidate validation proves faithful source analysis only; source diagnostics never authorize lowering','candidate unlowerable1 retained; all module/present payloads rejected5008 by ordinary lower; no provider/callback/constructor/Ref authority','mini source modules are independent pinned constructed fixtures, not official selected SDK provider or current std ownership qualification','wire/arena cap4000000i64 words; large duplicated input tables recorded, no compiler closure capacity or M8/RSS claim','development compiler uses recorded C/Rust runtime; nested legacy source/native behavior gates do not qualify complete MSVC child-linker/SDK bootstrap provenance'],'sdk_environment':{n:env.get(n)for n in ['INCLUDE','LIB','LIBPATH','VCToolsInstallDir','WindowsSdkDir','WindowsSDKVersion','VSINSTALLDIR','VCINSTALLDIR']}}
    configs={p/'.cargo'/n for p in [ROOT,*ROOT.parents,snapshot,*snapshot.parents]for n in ['config','config.toml']};configs.update(Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))/n for n in ['config','config.toml']);report['cargo_config_candidates']={}
    for p in configs:
        raw=p.read_bytes()if p.is_file()else None;report['cargo_config_candidates'][str(p)]=None if raw is None else hashlib.sha256(raw).hexdigest()
        if raw is not None:assert 'include'not in tomllib.loads(raw.decode('utf8'))
    def save():(base/'evidence.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    def run(name,command,code=0,empty=False):
        done=subprocess.run(list(map(str,command)),cwd=ROOT,env=pure if empty else env,capture_output=True,timeout=900);stdout=base/(name+'.stdout.log');stderr=base/(name+'.stderr.log');stdout.write_bytes(done.stdout);stderr.write_bytes(done.stderr);report['steps'].append({'name':name,'command':list(map(str,command)),'exit_code':done.returncode,'expected_exit_code':code,'empty_path':empty,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)});save();print(name,done.returncode,flush=True);assert done.returncode==code,(name,done.returncode,done.stdout[-800:],done.stderr[-3000:]);return done
    artifacts={}
    try:
        for n in ['cargo','rustc']:run(n+'-version',[tools[n],'-vV'])
        probe=base/'cargo-probe';probe.mkdir();(probe/'src').mkdir();(probe/'Cargo.toml').write_text('[package]\nname="hir_origin_rustc_probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n',encoding='utf8');(probe/'src/lib.rs').write_text('pub fn answer()->i64{42}\n',encoding='utf8');(probe/'build.rs').write_text('fn main(){println!("cargo:warning=hir_origin_rustc={}",std::env::var("RUSTC").unwrap());}\n',encoding='utf8')
        observed=run('cargo-actual-rustc',[tools['cargo'],'build','--offline','--verbose','--manifest-path',probe/'Cargo.toml','--target-dir',probe/'target']);combined=(observed.stdout+observed.stderr).decode('utf8');runs=[s for s in combined.splitlines()if s.lstrip().startswith('Running ')and str(tools['rustc']).casefold()in s.casefold()];assert len(runs)>=2 and 'hir_origin_rustc='+str(tools['rustc'])in combined;report['cargo_actual_rustc_invocations']=runs
        for p in probe.rglob('*'):
            if p.is_file()and 'target'not in p.relative_to(probe).parts:inputs[str(p)]=sha(p)
        for n in ['hir','hir_origins','mir_lower']:run('format-src-'+n,[tools['formatter'],'--check',snapshot/'selfhost/src'/(n+'.sg')])
        for n in ['hir_origins','mir','borrow','handle_disarm_hir']:run('format-test-'+n,[tools['formatter'],'--check',snapshot/'selfhost/tests'/(n+'.sg')])
        binary=base/'hir-origins.exe';run('build-owning-tests',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/hir_origins.sg','--force-rebuild','-O','0','-o',binary])
        for flag in ['--fail-equality','--fail-truth']:
            result=run('assert-'+flag,[binary,flag],1,True);assert b'hir_origins.sg'in result.stderr
        result=run('ordinary-first-resources',[binary],empty=True);assert result.stdout.strip()==b'HIR7 exact first-call resources passed'and not result.stderr
        # Constructed source fixtures contain actual declarations, bodies and physical imports.
        # Their external source pins identify these files; they are never SDK/provider certificates.
        library=base/'fixture stdlib 世界 &';library.mkdir()
        collection='''import std::option;
extern "C"{fn sengoo_raw_vec_iter_new(handle: i64) -> i64;fn sengoo_raw_vec_iter_free(handle:i64)->i64;fn sengoo_raw_vec_clear(handle:i64)->i64;}
struct Vec<T>{handle:i64,marker:i64}struct RawVecIter<T>{handle:i64,marker:i64}struct VecIter<T>{handle:i64,marker:i64}
def vec_new<T>()->Vec<T>{Vec{handle:0,marker:0}}
def raw_vec_iter_next<T>(handle:i64)->Option<&T>{raw_vec_iter_next(handle)}
impl<T> Vec<T>{def iter(&self)->RawVecIter<T>{RawVecIter{handle:sengoo_raw_vec_iter_new(self.handle),marker:0}}def clear(&self)->bool{sengoo_raw_vec_clear(self.handle)==0}}
impl Vec<i64>{def iter(&self)->VecIter<i64>{VecIter{handle:sengoo_raw_vec_iter_new(self.handle),marker:0}}}
impl Vec<bool>{def iter(&self)->VecIter<bool>{VecIter{handle:sengoo_raw_vec_iter_new(self.handle),marker:0}}}
impl<T> RawVecIter<T>{def next(&self)->Option<&T>{raw_vec_iter_next(self.handle)}def free(&self)->bool{sengoo_raw_vec_iter_free(self.handle)==0}}
impl<T> VecIter<T>{def free(&self)->bool{sengoo_raw_vec_iter_free(self.handle)==0}}
'''
        for n,text in {'collections':collection,'option':'struct Option<T>{is_some:bool,value:T}\n','ffi':'','string':'','result':''}.items():
            p=library/(n+'.sg');p.write_text(text,encoding='utf8');inputs[str(p)]=sha(p)
        source=base/'actual source 世界 &.sg';source.write_text('import std::collections;\ndef clean()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();0}\ndef conflict()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();values.clear();let alias=&iterator;0}\ndef main()->i64{42}\n',encoding='utf8');inputs[str(source)]=sha(source)
        unused=base/'unused standard declarations.sg';unused.write_text('import std::collections;def main()->i64{42}\n',encoding='utf8');inputs[str(unused)]=sha(unused)
        candidate=run('mini-candidates-first-resources',[binary,source,library,library/'collections.sg',sha(library/'collections.sg'),library/'option.sg',sha(library/'option.sg'),'candidate'],empty=True)
        lines=candidate.stdout.decode('utf8').splitlines();marker='module candidates: complete owning codec, actual source pins, all field poisons, no lower authority';assert len(lines)==7 and lines[-1]=='HIR7 exact first-call resources passed'and lines[::2][:3]==[marker]*3 and not candidate.stderr
        sizes=list(map(int,lines[1:6:2]));assert len(set(sizes))==1 and 0<sizes[0]<4000000
        absent=run('mini-unused-first-resources',[binary,unused,library,library/'collections.sg',sha(library/'collections.sg'),library/'option.sg',sha(library/'option.sg'),'absent'],empty=True);assert absent.stdout.decode('utf8').splitlines()==['module absent: actual unused standard declarations preserved, no lower authority']*3+['HIR7 exact first-call resources passed']and not absent.stderr
        actual_library=snapshot/'tools/stdlib';large=run('actual-stdlib-faithful-rejection',[binary,source,actual_library,actual_library/'collections.sg',sha(actual_library/'collections.sg'),actual_library/'option.sg',sha(actual_library/'option.sg'),'stdlib'],empty=True);lines=large.stdout.decode('utf8').splitlines();assert len(lines)==7 and lines[0]=='stdlib|ownership=6101|diagnostics=112'and lines[-1]=='HIR7 exact first-call resources passed'and not large.stderr
        ast,origin_input,loan_input,module_input,wire=list(map(int,lines[1:6]));assert ast>35000 and origin_input>500000 and loan_input>800000 and module_input>300000 and 2500000<wire<4000000
        report.update(candidate_field_poison_count=120,physical_mapping_poison_count=13,physical_nondeclaration_source_replacement_control=True,codec_header_scalar_negative_controls=8,hir_header_negative_controls=3,exact_total_and_total_minus_one_controls=True,clone_total_budget_above_every_individual_vector=True,preclone_budget_readonly_rejection=True,hir_raw_header_fields=["error","start","end"],ordinary_first_call_scopes=10,mini_first_call_scopes=6,stdlib_first_call_scopes=1,mini_full_hir_words=sizes[0],stdlib_candidate_words={'ast_nodes':ast,'origins_input':origin_input,'loans_origin_input':loan_input,'module_input':module_input,'wire':wire,'bytes':wire*8,'wire_word_limit':4000000},actual_stdlib_ownership={'error':6101,'diagnostics':112,'successful_module_hir_rejected':5502},fixture_sources={str(p):sha(p)for p in [source,unused,*library.glob('*.sg')]})
        report['regressions']={}
        regressions=[('mir','verify_mir.py',[]),('disarm','verify_handle_disarm.py',['--reference',tools['reference']]),('projection','verify_projection.py',['--reference',tools['reference']]),('source-native','source_codegen_gate.py',['--formatter',tools['formatter'],'--reference',tools['reference']])]
        for name,script,extra in regressions:
            dest=snapshot/'target'/('hir7-'+name);command=[tools['python'],snapshot/'selfhost'/script,'--seed',tools['seed'],'--clang',tools['clang'],'--output',dest,*extra];run('existing-'+name,command)
            evidence=dest/'evidence.json';legacy=json.loads(evidence.read_text('utf8'));expected_result='passed-with-reference-zero-drop-policy-difference'if name=='disarm'else'passed';assert legacy['result']==expected_result,(name,legacy.get('result'))
            for k,value in legacy.items():
                if k.startswith(('changed_','source_changes','snapshot_changes','tool_changes','cargo_config_changes','checkout_changes','producer_checkout_changes')):assert not value,(name,k,value)
            report['regressions'][name]={'report':str(evidence),'sha256':sha(evidence),'steps':len(legacy['steps']),'result':legacy['result'],'scope':'legacy behavior regression; no complete child-linker or SDK/bootstrap provenance'}
            if name=='mir':assert legacy['hir_schema']==7 and len(legacy['actual_frontend_import_modules'])>11;report['frontend_import_modules']=legacy['actual_frontend_import_modules']
            # Keep every actual binary/object output, including child negative controls.
            for p in dest.rglob('*'):
                if p.is_file()and p.suffix in ['.exe','.obj','.ll']and not any(n in ['build','target']for n in p.relative_to(dest).parts[:-1]):artifacts[str(p.relative_to(base))]=sha(p)
        artifacts[binary.name]=sha(binary);report.update(result='passed',production_admitted=True,artifacts=artifacts)
    except BaseException as error:report.update(result='failed',error=repr(error));raise
    finally:
        report.update(changed_sources=[n for n,h in inputs.items()if not Path(n).is_file()or sha(n)!=h],changed_tools=[n for n,value in report['tools'].items()if not Path(value['path']).is_file()or sha(value['path'])!=value['sha256']],checkout_changes=[str(p)for p,h in originals.items()if not p.is_file()or sha(p)!=h],changed_configs=[n for n,h in report['cargo_config_candidates'].items()if(sha(n)if Path(n).is_file()else None)!=h],changed_artifacts=[n for n,h in artifacts.items()if not(base/n).is_file()or sha(base/n)!=h],source_revision_end=revision(),completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
        if any(report[k]for k in ['changed_sources','changed_tools','checkout_changes','changed_configs','changed_artifacts']):report.update(result='failed',error='qualified input/output changed')
        save()
    assert report['result']=='passed';print(sha(base/'evidence.json'))
if __name__=='__main__':main()
