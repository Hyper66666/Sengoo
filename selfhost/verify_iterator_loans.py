"""Qualify the independent, partial backing-loan/effects source analysis layer."""
from __future__ import annotations
import argparse,datetime as dt,hashlib,json,os,platform,re,subprocess,sys,time,tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
FIXTURE="""import std::collections;
extern "C"{fn opaque()->i64;fn unknown_raw(handle:i64)->i64;}
struct Owner{handle:i64}impl Drop for Owner{def drop(&mut self){}}
def raw<T>(values:&Vec<T>)->RawVecIter<T>{values.iter()}
def borrow<T>(iterator:&RawVecIter<T>)->Option<&T>{iterator.next()}
def observe(values:&Vec<i64>)->i64{0}
def needs_iterator(iterator:&RawVecIter<i64>)->i64{let item=iterator.next();0}
def needs_two(first:&RawVecIter<i64>,second:&RawVecIter<i64>)->i64{let a=first.next();let b=second.next();0}
def mutate(values:&Vec<i64>)->i64{values.clear();0}
def mutate2(values:&Vec<i64>)->i64{mutate(values)}
def only_second(first:&Vec<i64>,second:&Vec<i64>)->i64{second.clear();0}
def consume(values:Vec<i64>)->i64{0}
def dispose(values:Vec<i64>){}
def consume_iterator<T>(iterator:RawVecIter<T>){}
def raw_identity(value:i64)->i64{value}
def make()->Vec<i64>{vec_new()}
def origin_i64()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let moved=iterator;let alias=&moved;0}
def origin_bool()->i64{let values:Vec<bool> =vec_new();let iterator=values.iter();0}
def origin_owner()->i64{let values:Vec<Owner> =vec_new();let iterator=values.iter();0}
def borrow_owner()->i64{let values:Vec<Owner> =vec_new();let iterator=raw(&values);let item=iterator.next();let alias=&item;0}
def borrow_bool()->i64{let values:Vec<bool> =vec_new();let iterator=raw(&values);let item=iterator.next();0}
def repeated()->i64{let values:Vec<i64> =vec_new();let iterator=raw(&values);let first=iterator.next();let second=iterator.next();0}
def scope_release()->i64{let values:Vec<i64> =vec_new();{let iterator=values.iter();0;};values.clear();0}
def direct_borrow()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let item:Option<&i64> =raw_vec_iter_next(iterator.handle);0}
def free_borrow_conflict()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let item:Option<&i64> =raw_vec_iter_next(iterator.handle);iterator.free();0}
def raw_unproven(handle:i64)->Option<&i64>{raw_vec_iter_next(handle)}
def free_release()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();iterator.free();values.clear();0}
def observe_causal()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();observe(&values);let alias=&iterator;0}
def nested_value()->i64{let values:Vec<Owner> =vec_new();let item={let iterator=raw(&values);iterator.next()};let alias=&item;0}
def all_parameters_good()->i64{let first:Vec<i64> =vec_new();let second:Vec<i64> =vec_new();let iterator=first.iter();only_second(&first,&second);0}
def returned_parameter()->i64{let values:Vec<i64> =vec_new();let iterator=raw(&values);let item=borrow(&iterator);0}
def required_good()->i64{let values:Vec<i64> =vec_new();let iterator=raw(&values);needs_iterator(&iterator);0}
def required_bad()->i64{let fake:RawVecIter<i64> =RawVecIter{handle:opaque(),marker:0};needs_iterator(&fake);0}
def required_two_bad()->i64{let values:Vec<i64> =vec_new();let first=raw(&values);let fake:RawVecIter<i64> =RawVecIter{handle:opaque(),marker:0};needs_two(&first,&fake);0}
def last_use_overreject()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let last=&iterator;values.clear();0}
def push_conflict()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();values.push(1);0}
def set_conflict()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();values.set(0,1);0}
def insert_conflict()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();values.insert(0,1);0}
def clear_conflict()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();values.clear();0}
def free_conflict()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();values.free();0}
def drop_conflict()->i64{let mut values:Vec<i64> =vec_new();let iterator=values.iter();dispose(values);0}
def transfer_conflict()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();consume(values);0}
def raw_escape()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let alias=&values;let handle=alias.handle;unknown_raw(handle);0}
def raw_repack()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let handle=values.handle;let fake:Vec<i64> =Vec{handle:handle,marker:0};fake.clear();0}
def callee_effect()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();mutate2(&values);0}
def all_parameters_bad()->i64{let first:Vec<i64> =vec_new();let second:Vec<i64> =vec_new();let iterator=second.iter();only_second(&first,&second);0}
def unknown_effect(callback:fn(&Vec<i64>)->i64)->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();callback(&values);0}
def drop_borrowed_iterator()->i64{let values:Vec<Owner> =vec_new();let mut iterator=raw(&values);let item=iterator.next();consume_iterator(iterator);0}
def copy_owner()->i64{let values:Vec<Owner> =vec_new();let iterator=raw(&values);let item=iterator.next();let owned=*item.value;0}
def raw_identity_escape()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let alias=&values;let h=raw_identity(alias.handle);unknown_raw(h);0}
def temporary_backing()->i64{let iterator=make().iter();0}
def escaped_backing()->i64{let item={let values:Vec<Owner> =vec_new();let iterator=raw(&values);iterator.next()};let alias=&item;0}
def cfg_boundary(flag:bool)->i64{let values:Vec<i64> =vec_new();if flag{();};let iterator=values.iter();0}
struct VecView{values:&Vec<i64>}
def obscure(values:&Vec<i64>)->i64{unknown_raw((values.handle+0) as i64);0}
def pass_vec(values:&Vec<i64>)->&Vec<i64>{values}
def through_alias(values:&Vec<i64>)->i64{let alias=pass_vec(values);alias.clear();0}
def wrapped_escape()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let view=VecView{values:&values};view.values.clear();0}
def assigned_escape()->i64{let values:Vec<i64> =vec_new();let other:Vec<i64> =vec_new();let iterator=values.iter();let mut alias=&other;alias=&values;alias.clear();0}
def grouped_raw_escape()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let alias=&values;unknown_raw((alias.handle));0}
def cast_raw_escape()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let alias=&values;unknown_raw(alias.handle as i64);0}
def arithmetic_raw_escape()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();let alias=&values;unknown_raw(alias.handle+0);0}
def obscured_callee_effect()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();obscure(&values);0}
def returned_alias_effect()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();through_alias(&values);0}
def main()->i64{42}
"""

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def revision():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def rust_override(name):
 key=name.upper();return key.startswith(('RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','CARGO_BUILD_RUSTC','CARGO_BUILD_RUSTDOC','CARGO_PROFILE_'))or key in {'CARGO','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS','CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_RUSTDOCFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','CARGO_INCREMENTAL'}or key.startswith('CARGO_TARGET_')and key.endswith(('_RUSTFLAGS','_RUSTDOCFLAGS','_RUSTC'))
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,default=ROOT/'target/selfhost/m3-iterator-loans/checkpoint');parser.add_argument('--seed',type=Path,default=ROOT/'target/agent-conditional-compiler/bin/sgc.exe');parser.add_argument('--clang',type=Path,default=Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe'));parser.add_argument('--case',choices=['source','release-shadow','typed-negative','typed-drop']);args=parser.parse_args();out=args.output.resolve()
 if not out.is_relative_to(ROOT/'target'):raise ValueError('output must remain under target')
 out.mkdir(parents=True,exist_ok=True);report={'schema':1,'stage':'independent-iterator-backing-loans','result':'running','m3_complete':False,'hir6_implemented':False,'origin_schema':1,'loan_schema':1,'native_reference_authority':False,'scope':'partial root-module source backing leases and effect summaries; scope-end conservative; no native references or HIR6','host':{'platform':platform.platform(),'python':platform.python_version(),'python_sha256':sha(sys.executable)},'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'steps':[],'selected_development_case':args.case};before={};origins={};snapshots={};tools={};configs={}
 def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
 save()
 try:
  report['source_revision']=revision();lock=json.loads((ROOT/'selfhost/bootstrap.lock').read_text(encoding='utf-8'));rustbin=Path(os.environ.get('RUSTUP_HOME',str(Path.home()/'.rustup')))/'toolchains'/(lock['rust_toolchain']+'-x86_64-pc-windows-msvc')/'bin'
  tools={'seed':(args.seed.resolve(),lock['seed']['sha256']),'clang':(args.clang.resolve(),lock['clang']['sha256']),'rustc':(rustbin/'rustc.exe',lock['rustc_sha256']),'cargo':(rustbin/'cargo.exe',lock['cargo_sha256'])}
  for name,(path,digest)in tools.items():
   if sha(path)!=digest:raise RuntimeError(name+' identity mismatch')
  report['tools']={name:{'path':str(path),'sha256':digest}for name,(path,digest)in tools.items()}
  entry=ROOT/'selfhost/tests/iterator_loans.sg';pending=[entry];closure=set()
  while pending:
   path=pending.pop()
   if path in closure:continue
   closure.add(path)
   for name in re.findall(r'^\s*import\s+selfhost_([a-z0-9_]+)\s*;',path.read_text(encoding='utf-8'),re.M):pending.append(ROOT/'selfhost/src'/(name+'.sg'))
  paths=set(closure)|{Path(__file__).resolve(),ROOT/'selfhost/bootstrap.lock',ROOT/'Cargo.toml',ROOT/'Cargo.lock',ROOT/'rust-toolchain.toml'}
  for directory in ['tools/stdlib','runtime']:
   paths.update(p for p in(ROOT/directory).rglob('*')if p.is_file()and not any(part in {'target','build','__pycache__'}for part in p.relative_to(ROOT/directory).parts))
  paths.update(p for p in(ROOT/'openspec/changes/add-selfhost-iterator-origins').rglob('*.md')if p.name!='tasks.md')
  before={str(p):sha(p)for p in sorted(paths)};report['inputs_before']=before
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
  def run(name,command,expected=0):
   started=time.perf_counter();r=subprocess.run(list(map(str,command)),cwd=ROOT,env=env,capture_output=True,timeout=900);stdout=out/(name+'.stdout.log');stderr=out/(name+'.stderr.log');stdout.write_bytes(r.stdout);stderr.write_bytes(r.stderr);report['steps'].append({'name':name,'command':list(map(str,command)),'exit':r.returncode,'expected_exit':expected,'elapsed_s':time.perf_counter()-started,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)});save();print(name,r.returncode,flush=True)
   if r.returncode!=expected:raise RuntimeError(name+' failed')
   return r
  for name,(path,digest)in tools.items():report['tools'][name]['version']=run(name+'-version',[path,'-vV'if name=='rustc'else '--version']).stdout.decode('utf-8').strip()
  binary=out/'loans.exe';binary.unlink(missing_ok=True);run('loan-tests-build',[args.seed.resolve(),'--runtime-mode','source-development','build',snapshot/entry.relative_to(ROOT),'--force-rebuild','-O','0','-o',binary]);snapshots[str(binary)]=sha(binary)
  fixtures={'typed-drop':'import std::collections;def main()->i64{let mut values:Vec<i64> =vec_new();values.drop();42}','typed-negative':'import std::collections;extern "C"{fn unknown_ref(values:&Vec<i64>)->i64;}def main()->i64{42}','source':FIXTURE,'release-shadow':'import std::collections;extern "C"{fn sengoo_raw_vec_iter_free(handle:i64)->i64;}def shadow_release()->i64{let values:Vec<i64> =vec_new();let iterator=values.iter();iterator.free();values.clear();0}def main()->i64{42}'}
  collections=snapshot/'tools/stdlib/collections.sg';digest=sha(collections);option=snapshot/'tools/stdlib/option.sg';option_sha=sha(option)
  for name,text in fixtures.items():
   if args.case is not None and name!=args.case:continue
   source=out/(name+'.sg');source.write_text(text,encoding='utf-8');snapshots[str(source)]=sha(source)
   r=run(name+'-three-first-call-resource-scopes',[binary,source,snapshot/'tools/stdlib',collections,digest,option,option_sha,*(['typed-drop']if name=='typed-drop'else['typed']if name=='typed-negative'else['shadow']if name!='source'else[])])
   marker=b'iterator loan typed rejection: 5123'if name=='typed-drop'else b'iterator loan typed rejection: 5122'if name=='typed-negative'else b'iterator backing loans: three exact resource scopes passed'if name=='source'else b'iterator loan shadow: rejected'
   if r.stdout.strip()!=marker or r.stderr:raise RuntimeError(name+' resource/oracle marker mismatch')
  report['artifacts']={'test_executable':sha(binary)};report['factory_count']=3;report['resource_scopes']=3 if args.case else 12;report['result']='passed-development-selected-case'if args.case else 'passed-independent-partial-backing-loans'
 except BaseException as e:report.update(result='failed',failure=type(e).__name__+': '+str(e))
 finally:
  try:
   report['source_changes']=[p for p,h in before.items()if sha(p)!=h];report['snapshot_changes']=[p for p,h in snapshots.items()if sha(p)!=h];report['checkout_changes']=[p for p,h in origins.items()if sha(p)!=h];report['tool_changes']=[n for n,(p,h)in tools.items()if sha(p)!=h];report['cargo_config_changes']=[p for p,h in configs.items()if(sha(p)if Path(p).is_file()else None)!=h];report['source_revision_after']=revision()
   if report['source_changes']or report['snapshot_changes']or report['tool_changes']or report['cargo_config_changes']:report['result']='failed'
  except BaseException as e:report.update(result='failed',finally_failure=str(e))
  report['finished_at']=dt.datetime.now(dt.timezone.utc).isoformat();save()
 print(json.dumps({k:report.get(k)for k in ['result','failure','source_changes','snapshot_changes','checkout_changes','tool_changes']}),flush=True)
 return 0 if report['result']in {'passed-independent-partial-backing-loans','passed-development-selected-case'}else 1
if __name__=='__main__':raise SystemExit(main())
