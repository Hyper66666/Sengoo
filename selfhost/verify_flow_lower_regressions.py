"""Retain exact source/tool/header inputs for unchanged ordinary lower regressions.

Legacy direct clang linking remains a behavior regression, not final SDK qualification.
"""
from __future__ import annotations
import argparse,concurrent.futures,datetime,hashlib,json,os,platform,re,subprocess,sys,tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def revision():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def rust_override(key):
    key=key.upper()
    return key.startswith(('RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','CARGO_BUILD_RUSTC','CARGO_BUILD_RUSTDOC','CARGO_PROFILE_')) or key in {'CARGO','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS','CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_RUSTDOCFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','CARGO_INCREMENTAL'} or key.startswith('CARGO_TARGET_') and key.endswith(('_RUSTFLAGS','_RUSTDOCFLAGS','_RUSTC'))
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
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',required=True,type=Path);args=parser.parse_args();base=args.output.resolve();assert base.is_relative_to(ROOT/'target');base.mkdir(parents=True,exist_ok=False);inputs={};originals={};tools={};configs={};artifacts={}
 report={'schema':1,'stage':'ordinary-lower-regressions-after-internal-flow-port','result':'running','source_revision':revision(),'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'host':platform.platform(),'sources':inputs,'qualified_checkout_sources':originals,'tools':tools,'cargo_config_candidates':configs,'steps':[],'child_reports':{},'limits':['verify_mir and verify_projection execute original live paths, guarded byte-identical to the retained raw source archive; they are not falsely described as consuming the archive','verify_handle_disarm/source_codegen_gate additionally execute their actual immutable producer snapshots','legacy zero-handle reference Drop policy and other declared reference gaps remain visible','existing direct clang links use its selected system linker; its path/hash and library-search-directory conservative manifests are retained; no final complete SDK/linker bootstrap qualification','these ordinary behavior regressions do not qualify source Ref authority, generic native selfbuild, M6 or M8']}
 def save():(base/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf8')
 def bind(p,h=None):
  p=Path(p).resolve();raw=p.read_bytes();actual=hashlib.sha256(raw).hexdigest();assert h is None or actual==h;inputs[str(p)]=actual;return raw
 def step(name,command,env,expected=0):
  done=subprocess.run(list(map(str,command)),cwd=ROOT,env=env,capture_output=True,timeout=3600);out=base/(name+'.stdout.log');err=base/(name+'.stderr.log');out.write_bytes(done.stdout);err.write_bytes(done.stderr);row={'name':name,'command':list(map(str,command)),'exit_code':done.returncode,'expected_exit_code':expected,'stdout_sha256':sha(out),'stderr_sha256':sha(err)};report['steps'].append(row);save();print(name,done.returncode,flush=True);assert done.returncode==expected,(name,done.stdout[-1000:],done.stderr[-2000:]);return done
 try:
  lock=json.loads((ROOT/'selfhost/bootstrap.lock').read_bytes());reference=json.loads((ROOT/'selfhost/reference-baseline.lock').read_bytes());clang=Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe');seed=ROOT/'target/agent-conditional-compiler/bin/sgc.exe';ref=ROOT/'target/selfhost/seed-reference/release/sgc.exe';fmt=ROOT/'target/agent-formatter/debug/sgfmt.exe';rustbin=Path(os.environ.get('RUSTUP_HOME',str(Path.home()/'.rustup')))/'toolchains'/(lock['rust_toolchain']+'-x86_64-pc-windows-msvc')/'bin'
  for n,p,h in [('seed',seed,lock['seed']['sha256']),('clang',clang,lock['clang']['sha256']),('reference',ref,reference['tools']['sgc']['sha256']),('formatter',fmt,lock['formatter']['sha256']),('cargo',rustbin/'cargo.exe',lock['cargo_sha256']),('rustc',rustbin/'rustc.exe',lock['rustc_sha256']),('python',Path(sys.executable),sha(sys.executable))]:bind(p,h);tools[n]={'path':str(p),'sha256':h}
  entries=[ROOT/n for n in ['selfhost/tests/borrow.sg','selfhost/tests/mir.sg','selfhost/tests/handle_disarm_hir.sg','selfhost/tools/native.sg']];seen=set();pending=entries[:]
  while pending:
   p=pending.pop()
   if p in seen:continue
   assert p.is_file(),p;seen.add(p)
   for name in re.findall(r'import\s+selfhost_([A-Za-z0-9_]+)\s*;',p.read_text('utf8')):pending.append(ROOT/'selfhost/src'/(name+'.sg'))
  paths=seen|{Path(__file__).resolve()}|{ROOT/n for n in ['selfhost/verify_mir.py','selfhost/verify_projection.py','selfhost/verify_handle_disarm.py','selfhost/source_codegen_gate.py','selfhost/bootstrap.lock','selfhost/reference-baseline.lock','selfhost/REFERENCE_GAPS.md','selfhost/tests/handle_disarm.sg','selfhost/tests/handle_disarm_probe.c','selfhost/tests/projection_probe.c','selfhost/tests/native_source_probe.c','Cargo.toml','Cargo.lock','rust-toolchain.toml']}
  for directory in ['tools/stdlib','runtime']:paths.update(p for p in(ROOT/directory).rglob('*')if p.is_file()and not any(x in {'build','target','__pycache__'}for x in p.relative_to(ROOT/directory).parts))
  paths.update(p for p in(ROOT/'openspec/changes/add-selfhost-handle-disarm').rglob('*.md')if p.name!='tasks.md')
  archive=base/'retained-inputs'
  for p in sorted(paths):
   raw=bind(p);h=hashlib.sha256(raw).hexdigest();originals[str(p)]=h;copy=archive/p.relative_to(ROOT);copy.parent.mkdir(parents=True,exist_ok=True);copy.write_bytes(raw);assert sha(copy)==h;inputs[str(copy)]=h
  assert all(sha(p)==h for p,h in originals.items());report['actual_sg_import_closure']={str(p.relative_to(ROOT)):originals[str(p)]for p in sorted(seen)}
  env={k:v for k,v in os.environ.items()if not rust_override(k)and not k.upper().startswith(('SENGOO_','SGPLATFORM_','SDL2_'))and k.upper()not in {'CL','_CL_','LINK','_LINK_'}};env.update(RUSTC=str(rustbin/'rustc.exe'),CARGO=str(rustbin/'cargo.exe'),CARGO_BUILD_RUSTC=str(rustbin/'rustc.exe'),RUSTUP_TOOLCHAIN=rustbin.parent.name,RUSTC_WRAPPER='',RUSTC_WORKSPACE_WRAPPER='',CARGO_BUILD_RUSTC_WRAPPER='',CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='');env['PATH']=str(rustbin)+os.pathsep+str(clang.parent)+os.pathsep+env.get('PATH','')
  report['explicit_compiler_environment']={k:env[k]for k in ['RUSTC','CARGO','CARGO_BUILD_RUSTC','RUSTUP_TOOLCHAIN','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','CARGO_BUILD_RUSTC_WRAPPER','CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER']}
  candidates={p/'.cargo'/n for p in [ROOT,*ROOT.parents,archive,*archive.parents]for n in ['config','config.toml']};candidates.update(Path(env.get('CARGO_HOME',str(Path.home()/'.cargo')))/n for n in ['config','config.toml'])
  for p in sorted(candidates):
   raw=p.read_bytes()if p.is_file()else None;configs[str(p)]=None if raw is None else hashlib.sha256(raw).hexdigest()
   if raw is not None:assert 'include'not in tomllib.loads(raw.decode('utf8'));inputs[str(p)]=configs[str(p)]
  for name in ['cargo','rustc','clang','seed','reference','formatter']:report['tools'][name]['version']=step(name+'-version',[tools[name]['path'],'-vV'if name=='rustc'else'--version'],env).stdout.decode('utf8','replace').strip()
  probe=base/'cargo-probe';(probe/'src').mkdir(parents=True);(probe/'Cargo.toml').write_text('[package]\nname="flow_lower_rustc_probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n',encoding='utf8');(probe/'src/lib.rs').write_text('pub fn answer()->i64{42}\n',encoding='utf8');(probe/'build.rs').write_text('fn main(){println!("cargo:warning=flow_lower_rustc={}",std::env::var("RUSTC").unwrap());}\n',encoding='utf8');observed=step('cargo-actual-compiler',[rustbin/'cargo.exe','build','--offline','--verbose','--manifest-path',probe/'Cargo.toml','--target-dir',probe/'target'],env);text=(observed.stdout+observed.stderr).decode('utf8');assert 'flow_lower_rustc='+str(rustbin/'rustc.exe')in text and sum(line.lstrip().startswith('Running ')and str(rustbin/'rustc.exe').casefold()in line.casefold()for line in text.splitlines())>=2
  for p in [probe/'Cargo.toml',probe/'Cargo.lock',probe/'build.rs',probe/'src/lib.rs']:bind(p)
  headers={}
  for name in ['projection_probe','handle_disarm_probe','native_source_probe']:
   source=ROOT/'selfhost/tests'/(name+'.c');before=base/(name+'-before.d');after=base/(name+'-after.d');step(name+'-headers',[clang,'-M','-MT','optional_dependency','-MF',before,source],env);captured={str(p):sha(p)for p in dependencies(before)}
   for p,h in captured.items():bind(p,h)
   obj=base/(name+'.obj');step(name+'-header-compile-check',[clang,'-MD','-MT','optional_dependency','-MF',after,'-c',source,'-o',obj],env);assert captured=={str(p):sha(p)for p in dependencies(after)};artifacts[obj.name]=sha(obj);headers[name]=captured
  report['c_header_closures']=headers
  selected=step('clang-system-link-selection',[clang,'-###',base/'projection_probe.obj','-o',base/'link-selection.exe'],env);diagnostic=(selected.stdout+selected.stderr).decode('utf8');report['system_link_selection']=diagnostic
  words=re.findall(r'"([^"\r\n]+)"',diagnostic);search=set()
  for word in words:
   if word.lower().endswith(('link.exe','lld-link.exe'))and Path(word).is_file():tools['system-linker']={'path':word,'sha256':sha(word)};bind(word)
   if word.lower().startswith('-libpath:'):search.add(Path(word.split(':',1)[1]))
  assert 'system-linker'in tools and search
  libraries={str(p.resolve()):sha(p)for d in search for p in d.glob('*.lib')};assert libraries
  for p,h in libraries.items():bind(p,h)
  report['conservative_system_library_search_manifest']=libraries
  save()
  jobs=[('mir','verify_mir.py',[]),('projection','verify_projection.py',['--reference',str(ref)]),('disarm','verify_handle_disarm.py',['--reference',str(ref)]),('source','source_codegen_gate.py',['--formatter',str(fmt),'--reference',str(ref)])]
  def run_child(job):
   name,script,extra=job;out=base/name;command=[sys.executable,ROOT/'selfhost'/script,'--seed',seed,'--clang',clang,'--output',out,*extra];done=subprocess.run(list(map(str,command)),cwd=ROOT,env=env,capture_output=True,timeout=3600);stdout=base/(name+'.stdout.log');stderr=base/(name+'.stderr.log');stdout.write_bytes(done.stdout);stderr.write_bytes(done.stderr);return name,command,done,out,stdout,stderr
  with concurrent.futures.ThreadPoolExecutor(max_workers=4)as pool:
   futures=[pool.submit(run_child,job)for job in jobs]
   for future in concurrent.futures.as_completed(futures):
    name,command,done,out,stdout,stderr=future.result();report['steps'].append({'name':name,'command':list(map(str,command)),'exit_code':done.returncode,'expected_exit_code':0,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)});child=out/'evidence.json';data=json.loads(child.read_bytes());report['child_reports'][name]={'path':str(child),'sha256':sha(child),'result':data['result'],'steps':len(data['steps'])};print(name,done.returncode,data['result'],flush=True);save();assert done.returncode==0
    for p in out.rglob('*'):
     if p.is_file()and(p.suffix in {'.exe','.obj','.ll'}or p.name.endswith(('.stdout.log','.stderr.log','.stdout','.stderr'))):artifacts[str(p.relative_to(base))]=sha(p)
    for key in ['source_changes','snapshot_changes','checkout_changes','tool_changes','changed_cargo_configs','changed_sources','changed_tools','changed_configs','changed_artifacts','changed_checkout_sources']:assert not data.get(key),(name,key,data.get(key))
    if name=='source':assert len(data['steps'])==346 and len(data['cases'])==54
  report.update(result='passed-ordinary-lower-regressions',artifacts=artifacts)
 except BaseException as error:report.update(result='failed',error=repr(error));raise
 finally:
  report.update(changed_sources=[p for p,h in inputs.items()if not Path(p).is_file()or sha(p)!=h],checkout_changes=[p for p,h in originals.items()if not Path(p).is_file()or sha(p)!=h],changed_tools=[n for n,item in tools.items()if not Path(item['path']).is_file()or sha(item['path'])!=item['sha256']],changed_configs=[p for p,h in configs.items()if(sha(p)if Path(p).is_file()else None)!=h],changed_artifacts=[p for p,h in artifacts.items()if not(base/p).is_file()or sha(base/p)!=h],source_revision_end=revision(),completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
  if any(report[k]for k in ['changed_sources','checkout_changes','changed_tools','changed_configs','changed_artifacts']):report['result']='failed'
  save()
 assert report['result']=='passed-ordinary-lower-regressions';print(sha(base/'evidence.json'))
if __name__=='__main__':main()
