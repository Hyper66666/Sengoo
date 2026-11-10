"""Qualify the independent CFG last-reachable-use overlay."""
from __future__ import annotations
import argparse,ctypes,datetime as dt,hashlib,json,os,platform,re,subprocess,sys,time,tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
FIXTURE='import std::collections;\nextern "C"{fn opaque()->i64;}\ndef last_use_overreject()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let last=&iterator;values.clear();0}\ndef iterator_later()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();values.clear();let last=&iterator;0}\ndef element_last()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let item:Option<&i64> =raw_vec_iter_next(iterator.handle);let last=&item;values.clear();0}\ndef element_later()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let item:Option<&i64> =raw_vec_iter_next(iterator.handle);values.clear();let last=&item;0}\ndef branch_last(flag:bool)->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();if flag{let left=&iterator;();}else{let right=&iterator;();};values.clear();0}\ndef branch_join(flag:bool)->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();if flag{values.clear();();};let last=&iterator;0}\ndef branch_return(flag:bool)->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();if flag{values.clear();return 0;};let last=&iterator;0}\ndef return_join(flag:bool)->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();if flag{values.clear();();}else{return 0;};let last=&iterator;0}\ndef loop_break()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();loop{let last=&iterator;values.clear();break;};0}\ndef break_later()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();loop{values.clear();break;};let last=&iterator;0}\ndef loop_backedge(flag:bool)->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();while flag{let last=&iterator;values.clear();};0}\ndef continue_backedge(flag:bool)->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();while flag{let last=&iterator;values.clear();continue;};0}\ndef continue_unreachable(flag:bool)->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();while flag{continue;values.clear();let unreachable=&iterator;};values.clear();0}\ndef use_it(iterator:&VecIter<i64>)->bool{true}\ndef short_circuit(flag:bool)->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();values.clear();flag and use_it(&iterator);0}\ndef unknown_source()->i64{let fake:VecIter<i64> =VecIter{handle:opaque(),marker:0};let item:Option<&i64> =raw_vec_iter_next(fake.handle);0}\ndef unsupported_match(flag:bool)->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();match flag{true=>0,false=>0};values.clear();0}\ndef conditional(iterator:&RawVecIter<i64>)->i64{let item=iterator.next();0}\nstruct Owner{handle:i64}impl Drop for Owner{def drop(&mut self){}}\ndef owner_last()->i64{let values:Vec<Owner> =vec_new();let iterator=values.iter();let item=iterator.next();let last=&item;values.clear();0}\ndef owner_later()->i64{let values:Vec<Owner> =vec_new();let iterator=values.iter();let item=iterator.next();values.clear();let last=&item;0}\ndef copy_owner()->i64{let values:Vec<Owner> =vec_new();let iterator=values.iter();let item=iterator.next();let owned=*item.value;0}\ndef only_second(first:&Vec<i64>,second:&Vec<i64>)->i64{second.clear();0}\ndef all_parameters_good()->i64{let first:Vec<i64> =vec_new();let second:Vec<i64> =vec_new();let iterator=first.iter();only_second(&first,&second);let last=&iterator;0}\ndef all_parameters_bad()->i64{let first:Vec<i64> =vec_new();let second:Vec<i64> =vec_new();let iterator=second.iter();only_second(&first,&second);let last=&iterator;0}\nstruct View{values:&Vec<i64>}\ndef unknown_container()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let last=&iterator;let view=View{values:&values};0}\ndef branch_container(flag:bool)->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();if flag{();};let view=View{values:&values};values.clear();0}\ndef copied_tag()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let item:Option<&i64> =raw_vec_iter_next(iterator.handle);let tag=item.is_some;values.clear();if tag{0}else{0}}\ndef mutation_then_fresh()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let last=&iterator;values.clear();let fresh=values.iter();let value=fresh.next();0}\ndef multiple_mutations()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let last=&iterator;values.clear();values.clear();0}\ndef main()->i64{42}\n'

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def revision():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def rust_override(name):
 key=name.upper();return key.startswith(('RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','CARGO_BUILD_RUSTC','CARGO_BUILD_RUSTDOC','CARGO_PROFILE_'))or key in {'CARGO','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS','CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_RUSTDOCFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','CARGO_INCREMENTAL'}or key.startswith('CARGO_TARGET_')and key.endswith(('_RUSTFLAGS','_RUSTDOCFLAGS','_RUSTC'))
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,default=ROOT/'target/selfhost/m3-iterator-nll/checkpoint');parser.add_argument('--seed',type=Path,default=ROOT/'target/agent-conditional-compiler/bin/sgc.exe');parser.add_argument('--clang',type=Path,default=Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe'));args=parser.parse_args();out=args.output.resolve()
 if not out.is_relative_to(ROOT/'target'):raise ValueError('output must remain under target')
 out.mkdir(parents=True,exist_ok=True);report={'schema':1,'stage':'independent-iterator-cfg-nll','result':'running','m3_complete':False,'hir6_implemented':False,'origin_schema':1,'loan_schema':1,'native_reference_authority':False,'scope':'root-module CFG backward liveness overlay; mandatory validated candidate inputs; no native references or HIR6','host':{'platform':platform.platform(),'python':platform.python_version(),'python_sha256':sha(sys.executable)},'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'steps':[]};before={};origins={};snapshots={};tools={};configs={}
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
  report['source_revision']=revision();lock=json.loads((ROOT/'selfhost/bootstrap.lock').read_text(encoding='utf-8'));rustbin=Path(os.environ.get('RUSTUP_HOME',str(Path.home()/'.rustup')))/'toolchains'/(lock['rust_toolchain']+'-x86_64-pc-windows-msvc')/'bin'
  tools={'seed':(args.seed.resolve(),lock['seed']['sha256']),'clang':(args.clang.resolve(),lock['clang']['sha256']),'rustc':(rustbin/'rustc.exe',lock['rustc_sha256']),'cargo':(rustbin/'cargo.exe',lock['cargo_sha256'])}
  for name,(path,digest)in tools.items():
   if sha(path)!=digest:raise RuntimeError(name+' identity mismatch')
  report['tools']={name:{'path':str(path),'sha256':digest}for name,(path,digest)in tools.items()}
  entry=ROOT/'selfhost/tests/iterator_loan_liveness.sg';pending=[entry];closure=set()
  while pending:
   path=pending.pop()
   if path in closure:continue
   closure.add(path)
   for name in re.findall(r'^\s*import\s+selfhost_([a-z0-9_]+)\s*;',path.read_text(encoding='utf-8'),re.M):pending.append(ROOT/'selfhost/src'/(name+'.sg'))
  paths=set(closure)|{Path(__file__).resolve(),ROOT/'selfhost/bootstrap.lock',ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'rust-toolchain.toml'}
  for directory in ['tools/stdlib','runtime']:
   paths.update(p for p in(ROOT/directory).rglob('*')if p.is_file()and not any(part in {'target','build','__pycache__'}for part in p.relative_to(ROOT/directory).parts))
  paths.update(p for p in(ROOT/'openspec/changes/add-selfhost-iterator-origins').rglob('*.md')if p.name!='tasks.md')
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
  binary=out/'nll.exe';binary.unlink(missing_ok=True);run('nll-tests-build',[args.seed.resolve(),'--runtime-mode','source-development','build',snapshot/entry.relative_to(ROOT),'--force-rebuild','-O','0','-o',binary]);snapshots[str(binary)]=sha(binary)
  collections=snapshot/'tools/stdlib/collections.sg';digest=sha(collections);option=snapshot/'tools/stdlib/option.sg';option_sha=sha(option)
  source=out/'source.sg';source.write_text(FIXTURE,encoding='utf-8');snapshots[str(source)]=sha(source)
  r=run('source-cfg-nll-three-first-call-resource-scopes',[binary,source,snapshot/'tools/stdlib',collections,digest,option,option_sha])
  lines=r.stdout.decode('utf-8').splitlines()
  if len(lines)!=7 or lines[0]!='nll_counts' or lines[-1]!='iterator NLL: CFG and three exact resource scopes passed' or not all(v.isdecimal()for v in lines[1:6])or r.stderr:raise RuntimeError('NLL resource/oracle marker mismatch')
  report['actual_table_counts']=dict(zip(['input_words','tokens','events','edges','supersessions'],map(int,lines[1:6])));report['input_hardcap_words']=16000000;report['m8_memory_qualified']=False
  report['artifacts']={'test_executable':sha(binary)};report['factory_count']=3;report['resource_scopes']=3;report['result']='passed-independent-cfg-nll'
 except BaseException as e:report.update(result='failed',failure=type(e).__name__+': '+str(e))
 finally:
  try:
   report['source_changes']=[p for p,h in before.items()if sha(p)!=h];report['snapshot_changes']=[p for p,h in snapshots.items()if sha(p)!=h];report['checkout_changes']=[p for p,h in origins.items()if sha(p)!=h];report['tool_changes']=[n for n,(p,h)in tools.items()if sha(p)!=h];report['cargo_config_changes']=[p for p,h in configs.items()if(sha(p)if Path(p).is_file()else None)!=h];report['source_revision_after']=revision()
   if report['source_changes']or report['snapshot_changes']or report['tool_changes']or report['cargo_config_changes']:report['result']='failed'
  except BaseException as e:report.update(result='failed',finally_failure=str(e))
  report['finished_at']=dt.datetime.now(dt.timezone.utc).isoformat();save()
 print(json.dumps({k:report.get(k)for k in ['result','failure','source_changes','snapshot_changes','checkout_changes','tool_changes']}),flush=True)
 return 0 if report['result']=='passed-independent-cfg-nll'else 1
if __name__=='__main__':raise SystemExit(main())
