"""Qualify the bounded AsyncFile factory, observer and print changes together.

Capture production bytes once. Test-only dispatch lives in a dedicated package;
this verifier makes no native AsyncFile execution or self-host fixed-point claim.
"""
from __future__ import annotations
import argparse,datetime,hashlib,importlib.util,json,os,platform,re,shutil,subprocess,sys,time,tomllib
from pathlib import Path
from compiler_inputs import compiler_closure,module_map
from async_factory_print_checks import collect_inputs,run_print_checks
from async_factory_policy_checks import run_policy_checks
from async_factory_trace import trace_fragment
from async_factory_source_checks import run_source_checks

ROOT=Path(__file__).resolve().parents[1]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def rust_override(key):
 key=key.upper()
 return key.startswith(('RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','CARGO_BUILD_RUSTC','CARGO_BUILD_RUSTDOC','CARGO_PROFILE_'))or key in {'CARGO','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS','CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_RUSTDOCFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','CARGO_INCREMENTAL'}or key.startswith('CARGO_TARGET_')and key.endswith(('_RUSTFLAGS','_RUSTDOCFLAGS','_RUSTC'))
def dependencies(path):
 raw=Path(path).read_text('utf8').replace('\\\n',' ');assert raw.startswith('factory_dependency:');words=[];word=[];i=0;payload=raw.split(':',1)[1]
 while i<len(payload):
  c=payload[i]
  if c=='\\'and i+1<len(payload)and payload[i+1]in' \t#':word.append(payload[i+1]);i+=2
  elif c=='$'and i+1<len(payload)and payload[i+1]=='$':word.append('$');i+=2
  elif c.isspace():
   if word:words.append(''.join(word));word=[]
   i+=1
  else:word.append(c);i+=1
 if word:words.append(''.join(word))
 return {str(Path(p).resolve()):sha(p)for p in words}

def main():
 ap=argparse.ArgumentParser(description=__doc__)
 for name in ['output','seed','clang','formatter','reference']:ap.add_argument('--'+name,type=Path,required=True)
 ap.add_argument('--provider-report',type=Path,default=ROOT/'target/selfhost/compiler-snapshot-provider-qualified3/evidence.json')
 ap.add_argument('--provider-pin',default='5986c47f3547447e99091d1ab7bac5e046412c94d5904e88a04bf8c34824b97a')
 ap.add_argument('--readiness-report',type=Path,default=ROOT/'target/selfhost/native-readiness-provider/qualified04/evidence.json')
 ap.add_argument('--readiness-pin',default='77c51f9c587dc8b72959c4b8b6e9cfa286d115a0500e2fffd3bc83dc6a76ab9e')
 a=ap.parse_args();out=a.output.resolve();assert out.is_relative_to(ROOT/'target');out.mkdir(parents=True,exist_ok=False)
 sources={};origins={};artifacts={};tools={};configs={};steps=[]
 report={'schema':1,'stage':'async-file-factory-hir7-observer-print','result':'running','host':platform.platform(),'source_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sources':sources,'producer_origins':origins,'artifacts':artifacts,'tools':tools,'cargo_configs':configs,'steps':steps,'m3_complete':False,'m6_complete':False,'native_async_file_support':False,'limits':['Source factory facts do not authorize native Future execution.','Pure HIR source anchors and module witness prove structural consistency; physical module consumption revalidates retained source handles.','Selected stdlib configuration remains a caller input; arbitrary simultaneous replacement of all trusted inputs is outside this gate.','Real case-sensitive directory execution unavailable on the development host; injected case-policy results are separate evidence.']}
 def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf8')
 def bind(path):p=Path(path).resolve();h=sha(p);assert str(p)not in sources or sources[str(p)]==h,str(p);sources[str(p)]=h;return h
 def artifact(path):p=Path(path).resolve();artifacts[str(p)]=sha(p)
 def run(name,command,expected=0,env=None,timeout=1800):
  command=list(map(str,command));stdout=out/(name+'.stdout');stderr=out/(name+'.stderr');started=time.monotonic();timed=False
  with stdout.open('wb')as o,stderr.open('wb')as e:
   try:p=subprocess.run(command,cwd=out,env=env,stdout=o,stderr=e,timeout=timeout)
   except subprocess.TimeoutExpired:timed=True;p=subprocess.CompletedProcess(command,None)
  artifact(stdout);artifact(stderr);steps.append({'name':name,'command':command,'exit_code':p.returncode,'expected_exit_code':expected,'timeout':timed,'elapsed_seconds':time.monotonic()-started,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)});save();print(name,p.returncode,'timeout'if timed else'',flush=True)
  p.stdout=stdout.read_bytes();p.stderr=stderr.read_bytes()
  if expected is not None:assert not timed and p.returncode==expected,(name,p.stdout[-1000:],p.stderr[-2000:])
  return p
 def regression(name):
  test=snapshot/'selfhost/tests'/(name+'.sg');exe=out/(name+'.exe');run(name+'-build',[a.seed,'--runtime-mode','source-development','build',test,'--force-rebuild','-O','0','-o',exe],env=env);artifact(exe)
  # Module fixture paths are repository-relative, so run through a cwd shim.
  oldcwd=os.getcwd();os.chdir(snapshot)
  try:
   done=subprocess.run([str(exe)],cwd=snapshot,env=pure,capture_output=True,timeout=1800)
  finally:os.chdir(oldcwd)
  o=out/(name+'-run.stdout');e=out/(name+'-run.stderr');o.write_bytes(done.stdout);e.write_bytes(done.stderr);artifact(o);artifact(e);steps.append({'name':name+'-run','command':[str(exe)],'cwd':str(snapshot),'exit_code':done.returncode,'expected_exit_code':0,'stdout_sha256':sha(o),'stderr_sha256':sha(e)});save();assert done.returncode==0,(name,done.stdout[-2000:],done.stderr[-2000:]);report['regressions'][name]=0
 save()
 try:
  report['qualified_dependencies']={}
  for label,path,pin in [('collections',a.provider_report.resolve(),a.provider_pin),('readiness',a.readiness_report.resolve(),a.readiness_pin)]:
   assert sha(path)==pin;bind(path);dep=json.loads(path.read_bytes());assert dep['result']=='passed'if label=='collections'else dep['result']=='passed-development-readiness-provider-lifecycle'
   for key in ['changed_sources','changed_artifacts','changed_configs','changed_tools']:assert not dep[key],(label,key)
   for p,h in dep['sources'].items():assert sha(p)==h;bind(p)
   for p,h in dep['artifacts'].items():q=Path(p);q=q if q.is_absolute()else path.parent/q;assert sha(q)==h;bind(q)
   for p,h in dep['cargo_config_candidates'].items():assert(sha(p)if Path(p).is_file()else None)==h;configs[p]=h
   if label=='readiness':assert not dep['final_sdk']and not dep['runtime_algorithms_ported']and not dep['sg_lowering_qualified']
   report['qualified_dependencies'][label]={'path':str(path),'sha256':pin,'result':dep['result'],'scope':'Rehashed immutable qualified source/artifact/config dependency; not a rerun or final runtime claim'}
  qualified_provider=json.loads(a.provider_report.read_bytes());provider_root=a.provider_report.resolve().parent/'implementation'
  entries=[ROOT/'selfhost/tests/async_factory/driver.sg',ROOT/'selfhost/tools/native.sg']+[ROOT/'selfhost/tests'/(n+'.sg')for n in ['semantic','semantic_modules','borrow','mir','hir_origins','hir_body_coverage']]
  closure=compiler_closure(ROOT,entries);paths=set(closure)
  paths.update(ROOT/'selfhost'/n for n in ['verify_async_factory.py','async_factory_print_checks.py','print_hir_fragment.sg','async_factory_policy_checks.py','async_factory_trace.py','async_factory_source_checks.py','policy_probe.sg','policy_wrapper.c','source_codegen_gate.py','bootstrap.lock','reference-baseline.lock','REFERENCE_GAPS.md','tests/native_source_probe.c','tests/async_factory_path_probe.c'])
  for folder in ['tools/stdlib','runtime','selfhost/tests/async_factory','selfhost/tests/semantic-fixtures']:
   paths.update(p for p in(ROOT/folder).rglob('*')if p.is_file()and not set(p.relative_to(ROOT/folder).parts)&{'target','build','__pycache__'})
  paths.update(ROOT/n for n in ['Cargo.toml','Cargo.lock','rust-toolchain.toml'])
  for child in ['add-selfhost-async-file-factory-contract','fix-selfhost-async-file-buffer-observer','add-selfhost-print-intrinsics']:
   paths.update((ROOT/'openspec/changes'/child).rglob('*.md'))
  snapshot=out/'implementation'
  for p in sorted(paths):
   h=sha(p);dest=snapshot/p.relative_to(ROOT);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(p.read_bytes());assert sha(dest)==h;origins[str(p)]=h;bind(dest)
  report['compiler_closure']={str(p.relative_to(ROOT)):h for p,h in closure.items()}
  lock=json.loads((snapshot/'selfhost/bootstrap.lock').read_bytes());reference=json.loads((snapshot/'selfhost/reference-baseline.lock').read_bytes())
  for name in ['seed','clang','formatter','reference']:
   p=getattr(a,name).resolve();expected=reference['tools']['sgc']['sha256']if name=='reference'else lock[name]['sha256']if name!='seed'else None
   if name=='seed':assert sha(p)in{lock['seed']['sha256'],reference['tools']['sgc']['sha256']}
   else:assert sha(p)==expected,name
   tools[name]={'path':str(p),'sha256':bind(p)}
  tools['python']={'path':sys.executable,'sha256':bind(sys.executable)}
  env={k:v for k,v in os.environ.items()if not rust_override(k)and not k.upper().startswith(('SENGOO_','SGPLATFORM_','SDL2_','PYTHON','LLDB_'))and k.upper()not in{'CL','_CL_','LINK','_LINK_'}}
  env['PATH']=str(a.clang.resolve().parent)+os.pathsep+env.get('PATH','');rustup=Path(shutil.which('rustup',path=env['PATH'])).resolve();bind(rustup)
  for name in ['cargo','rustc']:
   p=Path(subprocess.check_output([rustup,'which','--toolchain',lock['rust_toolchain'],name],cwd=ROOT,env=env,text=True).strip()).resolve();assert sha(p)==lock[name+'_sha256'];tools[name]={'path':str(p),'sha256':bind(p)}
  rustbin=Path(tools['rustc']['path']).parent
  env.update(RUSTC=tools['rustc']['path'],CARGO=tools['cargo']['path'],CARGO_BUILD_RUSTC=tools['rustc']['path'],RUSTUP_TOOLCHAIN=rustbin.parent.name,RUSTC_WRAPPER='',RUSTC_WORKSPACE_WRAPPER='',CARGO_BUILD_RUSTC_WRAPPER='',CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='')
  env['PATH']=str(rustbin)+os.pathsep+env['PATH'];env.update(SENGOO_STDLIB=str(snapshot/'tools/stdlib'),SENGOO_RUNTIME=str(snapshot/'tools/stdlib/runtime.c'),SENGOO_CLANG=str(a.clang.resolve()),SENGOO_MODULE_MAP=module_map(ROOT,closure,snapshot));pure=dict(env,PATH='')
  for name in ['kernel32.dll','ntdll.dll','KernelBase.dll']:bind(Path(os.environ['SystemRoot'])/'System32'/name)
  candidates={p/'.cargo'/name for p in{ROOT,*ROOT.parents,snapshot,*snapshot.parents,out}for name in['config','config.toml']};candidates.update(Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))/n for n in ['config','config.toml'])
  for p in candidates:
   value=sha(p)if p.is_file()else None;assert str(p)not in configs or configs[str(p)]==value,'conflicting configuration capture: '+str(p);configs[str(p)]=value
   if p.is_file():assert 'include'not in tomllib.loads(p.read_text('utf8'));bind(p)
  probe=out/'cargo-probe';(probe/'src').mkdir(parents=True);(probe/'Cargo.toml').write_text('[package]\nname="factory_actual_rustc"\nversion="0.0.0"\nedition="2021"\n[workspace]\n');(probe/'src/lib.rs').write_text('pub fn answer()->i64{42}\n');(probe/'build.rs').write_text('fn main(){println!("cargo:warning=factory_actual_rustc={}",std::env::var("RUSTC").unwrap());}\n')
  for p in [probe/'Cargo.toml',probe/'src/lib.rs',probe/'build.rs']:bind(p)
  actual=run('actual-rustc',[tools['cargo']['path'],'build','--offline','--verbose','--manifest-path',probe/'Cargo.toml','--target-dir',probe/'target'],env=env);assert ('factory_actual_rustc='+tools['rustc']['path']).encode()in actual.stdout+actual.stderr
  report['c_headers']={};c_sources=[snapshot/'selfhost/compiler_support/native'/n for n in ['source_snapshot_os.c','source_snapshot_memory.c']]+[snapshot/'selfhost/factory_support/native/factory_path_os.c',snapshot/'selfhost/tests/async_factory/native/resource_probe.c',snapshot/'selfhost/tests/async_factory_path_probe.c',snapshot/'tools/stdlib/runtime.c',snapshot/'tools/stdlib/runtime_string.c']
  for index,p in enumerate(c_sources):
   label='headers-'+str(index);before=out/(label+'.d');after=out/(label+'-compiled.d');obj=out/(label+'.obj');flags=['-std=c11','-O0','-Werror=date-time']
   run(label,[a.clang,*flags,'-M','-MT','factory_dependency','-MF',before,p],env=env);headers=dependencies(before)
   for n,h in headers.items():bind(n)
   run(label+'-compile',[a.clang,*flags,'-MD','-MT','factory_dependency','-MF',after,'-c',p,'-o',obj],env=env);assert dependencies(after)==headers;artifact(before);artifact(after);artifact(obj);report['c_headers'][str(p)]=headers
  selected=run('selected-linker',[a.clang,'-###',snapshot/'tools/stdlib/runtime.c','-o',out/'selection.exe'],env=env);words=re.findall(r'"([^"\r\n]+)"',(selected.stdout+selected.stderr).decode('utf8'));link_inputs={}
  for word in words:
   if word.lower().endswith(('link.exe','lld-link.exe'))and Path(word).is_file():link_inputs[str(Path(word).resolve())]=bind(word)
   if word.lower().startswith('-libpath:'):
    for p in Path(word.split(':',1)[1]).glob('*.lib'):link_inputs[str(p.resolve())]=bind(p)
  assert any(n.lower().endswith(('link.exe','lld-link.exe'))for n in link_inputs)and len(link_inputs)>10;report['link_inputs']=link_inputs
  fixtures=out/'fixtures';fixtures.mkdir()
  texts={'main42.sg':'import std::file;def main()->i64{42}\n','group.sg':'import std::{file};def main()->i64{42}\n','stream.sg':'import std::stream;def main()->i64{42}\n'}
  prefix='import std::ffi;import std::status;import std::result;import std::string;'
  texts.update({'same-bytes-nonstandard-module.sg':prefix+'import shadow;def main()->i64{42}\n','selection-realstd-shadow.sg':prefix+'import std::file;import shadow;def main()->i64{42}\n','selection-aliased-file.sg':prefix+'import alias_file;def main()->i64{42}\n','selection-aliased-stream.sg':prefix+'import alias_file;import alias_stream;def main()->i64{42}\n'})
  for name,text in texts.items():p=fixtures/name;p.write_text(text,encoding='utf8');bind(p)
  for name in ['stdlib','relocated standard 世界','stdlib-éя']:
   directory=fixtures/name;directory.mkdir()
   for source in(snapshot/'tools/stdlib').glob('*.sg'):target=directory/source.name;target.write_bytes(source.read_bytes());bind(target)
  shadow=fixtures/'shadow.sg';shadow.write_bytes((snapshot/'tools/stdlib/file.sg').read_bytes());bind(shadow)
  # Pin print component and every actual Rust/C source before compiling the producer.
  producer_inputs=collect_inputs(snapshot,ROOT,[a.reference.resolve(),a.clang.resolve()])
  for p,h in producer_inputs.items():assert bind(p)==h,'conflicting producer capture: '+p
  report['regressions']={}
  regression('mir')
  entry=snapshot/'selfhost/tests/async_factory/driver.sg';run('joint-format',[a.formatter,'--check',entry],env=env)
  generated=out/'generated-trace.sg';generated.write_text(trace_fragment(snapshot),encoding='utf8');run('trace-format',[a.formatter,'--write',generated],env=env);bind(generated)
  assert entry.read_text('utf8').rstrip().endswith(generated.read_text('utf8').rstrip()),'test trace is not derived from the captured driver/lift/lower bodies'
  binary=out/'joint-compiler.exe';run('joint-build',[a.seed,'--runtime-mode','source-development','build',entry,'--force-rebuild','-O','0','-o',binary],env=env);artifact(binary)
  for mode in ['factory-hir','source-selection','observer','identity']:
   command=[binary,'--'+mode,fixtures]
   if mode=='source-selection':command.append('fixtures/stdlib')
   run(mode,command,42,pure)
  source_result=run_source_checks(run,pure,snapshot,out/'source-controls',binary);report['source_report']={'path':str(out/'source-controls/evidence.json'),'sha256':bind(out/'source-controls/evidence.json'),'result':source_result['result']}
  policy=run_policy_checks(lambda n,c,e,expected=0,timeout=900:(lambda r:(r.stdout,r.stderr))(run('policy-'+n,c,expected,e,timeout)),env,pure,snapshot,out/'policy',a.seed,a.clang);report['policy_report']={'path':str(out/'policy/evidence.json'),'sha256':bind(out/'policy/evidence.json'),'result':policy['result']}
  path_exe=out/'path-strings.exe';run('path-string-build',[a.clang,snapshot/'selfhost/tests/async_factory_path_probe.c','-o',path_exe],env=env);run('path-string-resource-controls',[path_exe],env=pure);artifact(path_exe)
  printed=run_print_checks(run,env,pure,snapshot,out/'print',binary,reference=a.reference,clang=a.clang,producer_inputs=producer_inputs,actual_checkout=ROOT,link_inputs=link_inputs);report['print_report']={'path':str(out/'print/evidence.json'),'sha256':bind(out/'print/evidence.json'),'result':printed['result']}
  for name in ['semantic','semantic_modules','borrow','hir_origins']:
   regression(name)
  hb_source=out/'hir-body-source.sg';hb_source.write_text('import std::collections;import collection_runtime;def identity(value:i64)->i64{value} def construct()->Vec<i64>{vec_new()} def main()->i64{identity(42)}\n');bind(hb_source)
  hb=out/'hir-body-coverage.exe';run('hir-body-coverage-build',[a.seed,'--runtime-mode','source-development','build',snapshot/'selfhost/tests/hir_body_coverage.sg','--force-rebuild','-O','0','-o',hb],env=env);artifact(hb)
  covered=run('hir-body-coverage-three-resource-scopes',[hb,provider_root,qualified_provider['provider_manifest_sha256'],qualified_provider['provider_object_sha256'],hb_source],env=pure);assert covered.stdout.decode('utf8').splitlines()==['hir body coverage: three first-call resource scopes passed']and not covered.stderr;report['regressions']['hir_body_coverage']='current HIR7 test, current compiler closure, rehashed qualified provider; historical dedicated runner not used'
  native=binary
  frontier=run('native-std-file-frontier',[native,fixtures/'main42.sg',out/'std-file.obj'],None,pure);report['std_file_frontier']={'exit_code':frontier.returncode,'stdout':frontier.stdout.decode('utf8','replace'),'stderr':frontier.stderr.decode('utf8','replace')}
  frontier_json=json.loads(frontier.stdout);assert frontier.returncode==65 and not frontier.stderr and frontier_json['schema_version']==1 and frontier_json['ok']is False and frontier_json['phase']=='lowering'and frontier_json['code']==5008 and not(out/'std-file.obj').exists()
  traced=run('trace-std-file-equivalence',[native,'--trace-self-input',fixtures/'main42.sg',out/'traced-std-file.obj'],None,pure);assert traced.returncode==frontier.returncode and traced.stdout==frontier.stdout and not(out/'traced-std-file.obj').exists()
  markers=[b'factory-stage:'+p for p in [b'modules',b'semantics',b'semantics-complete',b'factory-source-bound',b'ownership',b'module-witness',b'hir-lift',b'hir-validate',b'mir-lower']];assert traced.stderr.splitlines()==markers
  full=run('full-compiler-self-input',[native,'--trace-self-input',snapshot/'selfhost/tools/native.sg',out/'self-input.obj'],None,pure,1800);report['full_self_input']={'test_only_stage_observation':True,'exit_code':full.returncode,'timed_out':full.returncode is None,'stdout':full.stdout.decode('utf8','replace'),'stderr':full.stderr.decode('utf8','replace')}
  observed=full.stderr.splitlines();all_markers=markers+[b'factory-stage:source-mapping',b'factory-stage:object'];assert observed==all_markers[:len(observed)]
  if full.returncode is not None:
   terminal=json.loads(full.stdout)
   if full.returncode==0:assert terminal=={'schema_version':1,'ok':True,'phase':'object'}and(out/'self-input.obj').is_file();artifact(out/'self-input.obj')
   else:assert full.returncode==65 and terminal['schema_version']==1 and terminal['ok']is False and terminal['phase']in{'modules','frontend','semantics','lowering','native'}and isinstance(terminal['code'],int)and terminal['code']>0 and not(out/'self-input.obj').exists()
  else:assert not(out/'self-input.obj').exists()
  report['result']='passed-bounded-contracts-with-recorded-native-frontier'
 except BaseException as e:report['result']='failed';report['error']=repr(e);raise
 finally:
  report['changed_sources']=[p for p,h in sources.items()if not Path(p).is_file()or sha(p)!=h];report['changed_origins']=[p for p,h in origins.items()if not Path(p).is_file()or sha(p)!=h];report['changed_artifacts']=[p for p,h in artifacts.items()if not Path(p).is_file()or sha(p)!=h];report['changed_configs']=[p for p,h in configs.items()if(sha(p)if Path(p).is_file()else None)!=h]
  if any(report[k]for k in ['changed_sources','changed_origins','changed_artifacts','changed_configs']):report['result']='failed'
  report['source_revision_end']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip();report['completed_at']=datetime.datetime.now(datetime.timezone.utc).isoformat();save()
 assert report['result'].startswith('passed'),report.get('error','input guard failure')
if __name__=='__main__':main()
