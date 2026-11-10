"""Qualify HIR6 deferred trait body coverage and readonly resolved callable identity against the full actual standard module graph."""
from __future__ import annotations
import argparse,ctypes,datetime as dt,hashlib,json,os,platform,re,subprocess,sys,time,tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
FIXTURE='import std::collections;import collection_runtime;def identity(value:i64)->i64{value} def construct()->Vec<i64>{vec_new()} def main()->i64{identity(42)}\n'

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def revision():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def rust_override(name):
 key=name.upper();return key.startswith(('RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','CARGO_BUILD_RUSTC','CARGO_BUILD_RUSTDOC','CARGO_PROFILE_'))or key in {'CARGO','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS','CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_RUSTDOCFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','CARGO_INCREMENTAL'}or key.startswith('CARGO_TARGET_')and key.endswith(('_RUSTFLAGS','_RUSTDOCFLAGS','_RUSTC'))
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,default=ROOT/'target/selfhost/m3-hir-body-coverage/checkpoint');parser.add_argument('--seed',type=Path,default=ROOT/'target/agent-conditional-compiler/bin/sgc.exe');parser.add_argument('--clang',type=Path,default=Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe'));args=parser.parse_args();out=args.output.resolve()
 if not out.is_relative_to(ROOT/'target'):raise ValueError('output must remain under target')
 out.mkdir(parents=True,exist_ok=True);report={'schema':1,'stage':'hir6-full-stdlib-body-coverage','result':'running','m3_complete':False,'hir6_implemented':True,'deferred_executable_authority':False,'origin_schema':1,'loan_schema':1,'native_reference_authority':False,'scope':'HIR6 raw full-module candidate serialization with deferred trait bodies, exact ordinary callable identity and no native default-body permission','host':{'platform':platform.platform(),'python':platform.python_version(),'python_sha256':sha(sys.executable)},'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'steps':[]};before={};origins={};snapshots={};tools={};configs={}
 def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
 save()
 try:
  qualified_path=ROOT/'target/selfhost/m3-iterator-loans/escaped-forms-final/evidence.json'
  qualified_raw=qualified_path.read_bytes();qualified_digest=hashlib.sha256(qualified_raw).hexdigest()
  if qualified_digest!='befc520f624e043664d9b48b5245861f9f4a25feec4d92af772ebf8eadb1a8db':raise RuntimeError('qualified loan report identity mismatch')
  qualified=json.loads(qualified_raw)
  if qualified.get('result')!='passed-independent-partial-backing-loans' or any(qualified.get(k)for k in ['source_changes','snapshot_changes','tool_changes','cargo_config_changes']):raise RuntimeError('loan report not qualified')
  dependency=ROOT/'selfhost/src/semantic_loans.sg';dependency_sha='a9c350d7aac79c0c0e647624711f748d401b9b2693cdf2c7835be8fed2798f7e'
  if qualified['inputs_before'].get(str(dependency))!=dependency_sha or sha(dependency)!=dependency_sha:raise RuntimeError('qualified loan source mismatch')
  before[str(qualified_path)]=qualified_digest
  report['qualified_loan_dependency']={'report':str(qualified_path),'report_sha256':qualified_digest,'source':str(dependency),'source_sha256':dependency_sha,'authority':'standalone candidate source qualification only; no runtime Ref authority'}
  nll_report=ROOT/'target/selfhost/m3-iterator-nll/checkpoint-boundaries/evidence.json'
  nll_digest=sha(nll_report)
  if nll_digest!='d06e4eb949c56b2389ddbb39f059745ccc0bcc8aca7b7e08aef7a84b2b4f0d08':raise RuntimeError('qualified NLL report identity mismatch')
  nll_qualified=json.loads(nll_report.read_bytes())
  nll_source=ROOT/'selfhost/src/semantic_loan_liveness.sg'
  if nll_qualified['result']!='passed-independent-cfg-nll' or nll_qualified['inputs_before'].get(str(nll_source))!=sha(nll_source):raise RuntimeError('qualified NLL source mismatch')
  before[str(nll_report)]=nll_digest
  provider_report=ROOT/'target/selfhost/m3-collection-provider-ast-checkpoint/evidence.json'
  provider_digest=sha(provider_report)
  if provider_digest!='a63152e01208dff39e1d5cbcbd326187901808c8f76fa0d3075a08e0a9fa1cd3':raise RuntimeError('qualified provider report identity mismatch')
  qualified_provider=json.loads(provider_report.read_bytes())
  if qualified_provider.get('result')!='passed' or any(qualified_provider.get(k)for k in ['changed_sources','checkout_changes','changed_tools','changed_configs','changed_artifacts']):raise RuntimeError('provider report not qualified')
  provider_root=provider_report.parent/'implementation'
  for source,digest in qualified_provider['sources'].items():
   if sha(source)!=digest:raise RuntimeError('qualified provider source changed: '+source)
   before[source]=digest
  for relative,digest in qualified_provider['artifacts'].items():
   artifact=provider_report.parent/relative
   if sha(artifact)!=digest:raise RuntimeError('qualified provider artifact changed: '+relative)
   before[str(artifact)]=digest
  before[str(provider_report)]=provider_digest
  provider_manifest=provider_root/'selfhost/sdk/collections_x64_abi1/manifest.json'
  if sha(provider_manifest)!=qualified_provider['provider_manifest_sha256']:raise RuntimeError('provider manifest pin mismatch')
  provider_source=ROOT/'selfhost/src/collection_provider.sg'
  if qualified_provider['qualified_checkout_sources'].get(str(provider_source))!=sha(provider_source):raise RuntimeError('qualified provider SG implementation mismatch')
  provider_origin=ROOT/'selfhost/src/semantic_origins.sg'
  if qualified_provider['qualified_checkout_sources'].get(str(provider_origin))!=sha(provider_origin):raise RuntimeError('qualified origin implementation mismatch')
  report['qualified_provider']={'report':str(provider_report),'report_sha256':provider_digest,'root':str(provider_root),'manifest_sha256':qualified_provider['provider_manifest_sha256'],'object_sha256':qualified_provider['provider_object_sha256']}
  qualified_handles=ROOT/'target/selfhost/m3-collection-handles/checkpoint/evidence.json'
  if sha(qualified_handles)!='62f9421e7e7d7405bd1fbfa797e15f65f86b5e1d4f24136066596192ead40140':raise RuntimeError('qualified handle report mismatch')
  old_handles=json.loads(qualified_handles.read_bytes())
  if old_handles['result']!='passed-independent-collection-handles' or any(old_handles.get(k)for k in ['source_changes','snapshot_changes','tool_changes','cargo_config_changes']):raise RuntimeError('handle report failed')
  dependency=ROOT/'selfhost/src/semantic_collection_handles.sg'
  if old_handles['inputs_before'].get(str(dependency))!=sha(dependency):raise RuntimeError('qualified handle implementation changed')
  if sha(ROOT/'selfhost/src/borrow.sg')!='990fb05324a641ecd2ebee7cd696d3224e55f8992df5bdd4118718969bdff45c':raise RuntimeError('legacy ownership baseline mismatch')
  before[str(qualified_handles)]=sha(qualified_handles)
  report['qualified_handles']={'report':str(qualified_handles),'report_sha256':sha(qualified_handles),'source':str(dependency),'source_sha256':sha(dependency),'legacy_ownership_sha256':sha(ROOT/'selfhost/src/borrow.sg')}
  report['source_revision']=revision();lock=json.loads((ROOT/'selfhost/bootstrap.lock').read_text(encoding='utf-8'));rustbin=Path(os.environ.get('RUSTUP_HOME',str(Path.home()/'.rustup')))/'toolchains'/(lock['rust_toolchain']+'-x86_64-pc-windows-msvc')/'bin'
  tools={'seed':(args.seed.resolve(),lock['seed']['sha256']),'clang':(args.clang.resolve(),lock['clang']['sha256']),'rustc':(rustbin/'rustc.exe',lock['rustc_sha256']),'cargo':(rustbin/'cargo.exe',lock['cargo_sha256'])}
  for name,(path,digest)in tools.items():
   if sha(path)!=digest:raise RuntimeError(name+' identity mismatch')
  report['tools']={name:{'path':str(path),'sha256':digest}for name,(path,digest)in tools.items()}
  ownership_report=ROOT/'target/selfhost/m3-collection-borrow/checkpoint/evidence.json'
  if sha(ownership_report)!='ac61c469da579ee17f8cd9ed9685c75dfd854c932c923329491dfe950b16369d':raise RuntimeError('qualified fresh ownership report identity mismatch')
  ownership=json.loads(ownership_report.read_bytes());ownership_source=ROOT/'selfhost/src/semantic_collection_borrow.sg'
  if ownership['result']!='passed-fresh-collection-ownership' or ownership['inputs_before'][str(ownership_source)]!=sha(ownership_source)or any(ownership.get(k)for k in ['source_changes','snapshot_changes','checkout_changes','tool_changes','cargo_config_changes']):raise RuntimeError('qualified fresh ownership source mismatch')
  before[str(ownership_report)]=sha(ownership_report);report['qualified_actual_ownership']={'report':str(ownership_report),'sha256':sha(ownership_report),'source_sha256':sha(ownership_source)}
  entry=ROOT/'selfhost/tests/hir_body_coverage.sg';pending=[entry];closure=set()
  while pending:
   path=pending.pop()
   if path in closure:continue
   closure.add(path)
   for name in re.findall(r'^\s*import\s+selfhost_([a-z0-9_]+)\s*;',path.read_text(encoding='utf-8'),re.M):pending.append(ROOT/'selfhost/src'/(name+'.sg'))
  paths=set(closure)|{Path(__file__).resolve(),ROOT/'selfhost/bootstrap.lock',ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'rust-toolchain.toml'}
  for directory in ['tools/stdlib','runtime']:
   paths.update(p for p in(ROOT/directory).rglob('*')if p.is_file()and not any(part in {'target','build','__pycache__'}for part in p.relative_to(ROOT/directory).parts))
  paths.update(p for p in(ROOT/'openspec/changes/add-selfhost-iterator-origins').rglob('*.md')if p.name!='tasks.md')
  paths.add(ROOT/'openspec/changes/add-selfhost-collection-intrinsics/hir6-body-coverage.md');paths.add(ROOT/'openspec/changes/add-selfhost-collection-intrinsics/specs/selfhost-hir-body-coverage/spec.md')
  before.update({str(p):sha(p)for p in sorted(paths)});report['inputs_before']=before
  snapshot=out/'implementation';snapshot.mkdir(exist_ok=True)
  for p in sorted(closure|{p for p in paths if p.is_relative_to(ROOT/'tools/stdlib')}):
   data=p.read_bytes();dest=snapshot/p.relative_to(ROOT);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(data);origins[str(p)]=hashlib.sha256(data).hexdigest();snapshots[str(dest)]=sha(dest)
  if any(sha(p)!=digest for p,digest in origins.items()):raise RuntimeError('mixed producer snapshot')
  report['producer_origins']=origins;report['producer_snapshots']=snapshots
  env={k:v for k,v in os.environ.items()if not k.upper().startswith(('SENGOO_','SDL2_','SGPLATFORM_'))and not rust_override(k)};env.update(SENGOO_STDLIB=str(snapshot/'tools/stdlib'),SENGOO_RUNTIME=str(snapshot/'tools/stdlib/runtime.c'),SENGOO_CLANG=str(args.clang.resolve()),SENGOO_MODULE_MAP=';'.join('selfhost_'+p.stem+'='+str(snapshot/p.relative_to(ROOT))for p in sorted(closure)if p.parent==ROOT/'selfhost/src'),RUSTC=str(rustbin/'rustc.exe'),CARGO_BUILD_RUSTC=str(rustbin/'rustc.exe'),CARGO=str(rustbin/'cargo.exe'),RUSTUP_TOOLCHAIN=rustbin.parent.name,RUSTC_WRAPPER='',RUSTC_WORKSPACE_WRAPPER='',CARGO_BUILD_RUSTC_WRAPPER='',CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='');env['PATH']=str(rustbin)+os.pathsep+str(args.clang.resolve().parent)+os.pathsep+env.get('PATH','')
  cargo_home=Path(env.get('CARGO_HOME',str(Path.home()/'.cargo')));candidates={parent/'.cargo'/name for parent in(ROOT,*ROOT.parents)for name in('config','config.toml')};candidates.update(cargo_home/name for name in('config','config.toml'))
  for path in sorted(candidates):
   raw=path.read_bytes()if path.is_file()else None;configs[str(path)]=None if raw is None else hashlib.sha256(raw).hexdigest()
   if raw is not None and 'include'in tomllib.loads(raw.decode('utf-8')):raise RuntimeError('Cargo include needs separate manifest')
  report['cargo_config_candidates']=configs
  class MemoryCounters(ctypes.Structure):
   _fields_=[('cb',ctypes.c_uint32),('PageFaultCount',ctypes.c_uint32),*[(n,ctypes.c_size_t)for n in ['PeakWorkingSetSize','WorkingSetSize','QuotaPeakPagedPoolUsage','QuotaPagedPoolUsage','QuotaPeakNonPagedPoolUsage','QuotaNonPagedPoolUsage','PagefileUsage','PeakPagefileUsage']]]
  get_memory=ctypes.WinDLL('psapi',use_last_error=True).GetProcessMemoryInfo;get_memory.argtypes=[ctypes.c_void_p,ctypes.POINTER(MemoryCounters),ctypes.c_uint32];get_memory.restype=ctypes.c_int
  def run(name,command,expected=0):
   started=time.perf_counter();stdout=out/(name+'.stdout.log');stderr=out/(name+'.stderr.log');peak=0
   with stdout.open('wb')as output,stderr.open('wb')as errors:
    process=subprocess.Popen(list(map(str,command)),cwd=ROOT,env=env,stdout=output,stderr=errors)
    while process.poll()is None:
     counters=MemoryCounters();counters.cb=ctypes.sizeof(counters)
     if get_memory(int(process._handle),ctypes.byref(counters),counters.cb):peak=max(peak,int(counters.PeakWorkingSetSize))
     if time.perf_counter()-started>900:process.kill();process.wait();raise RuntimeError(name+' timeout')
     time.sleep(.02)
    result=process.returncode
   r=subprocess.CompletedProcess(list(map(str,command)),result,stdout.read_bytes(),stderr.read_bytes())
   report['steps'].append({'name':name,'command':list(map(str,command)),'exit':r.returncode,'expected_exit':expected,'elapsed_s':time.perf_counter()-started,'observed_peak_working_set_bytes':peak,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)});save();print(name,r.returncode,flush=True)
   if r.returncode!=expected:raise RuntimeError(name+' failed')
   return r
  for name,(path,digest)in tools.items():report['tools'][name]['version']=run(name+'-version',[path,'-vV'if name=='rustc'else '--version']).stdout.decode('utf-8').strip()
  binary=out/'borrow.exe';binary.unlink(missing_ok=True);run('borrow-tests-build',[args.seed.resolve(),'--runtime-mode','source-development','build',snapshot/entry.relative_to(ROOT),'--force-rebuild','-O','0','-o',binary]);snapshots[str(binary)]=sha(binary)
  source=out/'source.sg';source.write_text(FIXTURE,encoding='utf-8');snapshots[str(source)]=sha(source)
  r=run('full-stdlib-hir6-three-first-call-scopes',[binary,provider_root,qualified_provider['provider_manifest_sha256'],qualified_provider['provider_object_sha256'],source])
  if r.stdout.decode('utf-8').splitlines()!=['hir body coverage: three first-call resource scopes passed']or r.stderr:raise RuntimeError('full HIR6 resource/oracle marker mismatch')
  report['production_legacy_ownership_unchanged']=True;report['runtime_reference_authority']=False;report['wire_schema']=6;report['deferred_trait_default_functions']=10;report['deferred_trait_default_calls']=4;report['resolved_equivalent_call_signatures']=2;report['artifacts']={'test_executable':sha(binary)};report['resource_scopes']=3;report['result']='passed-hir6-full-stdlib-body-coverage'
 except BaseException as e:report.update(result='failed',failure=type(e).__name__+': '+str(e))
 finally:
  try:
   report['source_changes']=[p for p,h in before.items()if sha(p)!=h];report['snapshot_changes']=[p for p,h in snapshots.items()if sha(p)!=h];report['checkout_changes']=[p for p,h in origins.items()if sha(p)!=h];report['tool_changes']=[n for n,(p,h)in tools.items()if sha(p)!=h];report['cargo_config_changes']=[p for p,h in configs.items()if(sha(p)if Path(p).is_file()else None)!=h];report['source_revision_after']=revision()
   if report['source_changes']or report['snapshot_changes']or report['tool_changes']or report['cargo_config_changes']:report['result']='failed'
  except BaseException as e:report.update(result='failed',finally_failure=str(e))
  report['finished_at']=dt.datetime.now(dt.timezone.utc).isoformat();save()
 print(json.dumps({k:report.get(k)for k in ['result','failure','source_changes','snapshot_changes','checkout_changes','tool_changes']}),flush=True)
 return 0 if report['result']=='passed-hir6-full-stdlib-body-coverage'else 1
if __name__=='__main__':raise SystemExit(main())
