"""Qualify the independent, partial canonical iterator-origin analysis layer."""
from __future__ import annotations
import argparse,datetime as dt,hashlib,json,os,platform,re,subprocess,sys,time,tomllib
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
FIXTURE="""import std::collections;
extern "C"{fn opaque()->i64;fn unknown_raw(handle:i64)->i64;}
struct Owner{handle:i64} impl Drop for Owner{def drop(&mut self){}}
def positive()->i64{let values:Vec<i64> = vec_new();let iterator=values.iter();let moved=iterator;let alias=&moved;let dead=&iterator;let raw=moved.handle;let fake:VecIter<i64> = VecIter{handle:raw,marker:0};let bad_alias=&fake;0}
def forward(values:&Vec<i64>)->VecIter<i64>{values.iter()}
def forward2(values:&Vec<i64>)->VecIter<i64>{forward(values)}
def generic<T>(values:&Vec<T>)->RawVecIter<T>{values.iter()}
def caller()->i64{let values:Vec<i64> = vec_new();let iterator=forward2(&values);0}
def unknown_caller()->i64{let values:Vec<i64> = Vec{handle:opaque(),marker:0};let iterator=forward2(&values);0}
def forged(iterator:RawVecIter<Owner>)->i64{let fake:VecIter<i64> = VecIter{handle:iterator.handle,marker:0};0}
def owned_caller()->i64{let values:Vec<Owner> = vec_new();let iterator=values.iter();0}
def branch_boundary(flag:bool)->i64{let values:Vec<i64> = vec_new();if flag{let unused=1;};let iterator=values.iter();0}
def loop_boundary()->i64{let values:Vec<i64> = vec_new();while false{0;};let iterator=values.iter();0}
def unknown_effect()->i64{let values:Vec<i64> = vec_new();unknown_raw(values.handle);let iterator=values.iter();0}
def escape()->VecIter<i64>{let values:Vec<i64> = vec_new();values.iter()}
def needs(value:&Vec<i64>)->i64{let iterator=value.iter();0}
def needs2(left:&Vec<i64>,right:&Vec<i64>)->VecIter<i64>{let other=right.iter();left.iter()}
def scalar_good()->i64{let values:Vec<i64> = vec_new();needs(&values);0}
def scalar_bad()->i64{let values:Vec<i64> = Vec{handle:opaque(),marker:0};needs(&values);0}
def scalar_conditional(values:&Vec<i64>)->i64{needs(values)}
def two_bad()->i64{let good:Vec<i64> = vec_new();let bad:Vec<i64> = Vec{handle:opaque(),marker:0};let iterator=needs2(&good,&bad);0}
def two_good()->i64{let left:Vec<i64> = vec_new();let right:Vec<i64> = vec_new();let iterator=needs2(&left,&right);0}
def two_pending(right:&Vec<i64>)->i64{let left:Vec<i64> = vec_new();let iterator=needs2(&left,right);0}
def zeros()->i64{let z0:Vec<i64> = Vec{handle:0,marker:0};let z1:Vec<bool> = Vec{handle:0,marker:0};let z2:Vec<Owner> = Vec{handle:0,marker:0};let z3:Vec<i64> = Vec{handle:0,marker:0};let z4:Vec<i64> = Vec{handle:0,marker:0};let z5:Vec<i64> = Vec{handle:0,marker:0};let z6:Vec<i64> = Vec{handle:0,marker:0};let z7:Vec<i64> = Vec{handle:0,marker:0};0}
def main()->i64{42}
"""

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def revision():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def rust_override(name):
 key=name.upper();return key.startswith(('RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','CARGO_BUILD_RUSTC','CARGO_BUILD_RUSTDOC','CARGO_PROFILE_'))or key in {'CARGO','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS','CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_RUSTDOCFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','CARGO_INCREMENTAL'}or key.startswith('CARGO_TARGET_')and key.endswith(('_RUSTFLAGS','_RUSTDOCFLAGS','_RUSTC'))
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--output',type=Path,default=ROOT/'target/selfhost/m3-iterator-origins/checkpoint');parser.add_argument('--seed',type=Path,default=ROOT/'target/agent-conditional-compiler/bin/sgc.exe');parser.add_argument('--clang',type=Path,default=Path('C:/Users/admin/AppData/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe'));args=parser.parse_args();out=args.output.resolve()
 if not out.is_relative_to(ROOT/'target'):raise ValueError('output must remain under target')
 out.mkdir(parents=True,exist_ok=True);report={'schema':1,'stage':'independent-iterator-origins','result':'running','m3_complete':False,'hir6_implemented':False,'origin_schema':1,'scope':'partial root-module straight-line origin dataflow; conditional parameter/return obligations; no native borrow authorization','host':{'platform':platform.platform(),'python':platform.python_version(),'python_sha256':sha(sys.executable)},'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'steps':[]};before={};origins={};snapshots={};tools={};configs={}
 def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf-8')
 save()
 try:
  report['source_revision']=revision();lock=json.loads((ROOT/'selfhost/bootstrap.lock').read_text(encoding='utf-8'));rustbin=Path(os.environ.get('RUSTUP_HOME',str(Path.home()/'.rustup')))/'toolchains'/(lock['rust_toolchain']+'-x86_64-pc-windows-msvc')/'bin'
  tools={'seed':(args.seed.resolve(),lock['seed']['sha256']),'clang':(args.clang.resolve(),lock['clang']['sha256']),'rustc':(rustbin/'rustc.exe',lock['rustc_sha256']),'cargo':(rustbin/'cargo.exe',lock['cargo_sha256'])}
  for name,(path,digest)in tools.items():
   if sha(path)!=digest:raise RuntimeError(name+' identity mismatch')
  report['tools']={name:{'path':str(path),'sha256':digest}for name,(path,digest)in tools.items()}
  entry=ROOT/'selfhost/tests/iterator_origins.sg';pending=[entry];closure=set()
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
  binary=out/'origins.exe';binary.unlink(missing_ok=True);run('origin-tests-build',[args.seed.resolve(),'--runtime-mode','source-development','build',snapshot/entry.relative_to(ROOT),'--force-rebuild','-O','0','-o',binary])
  fixtures={'source':FIXTURE,'shadow-constructor':'import std::collections;def vec_new<T>()->Vec<T>{Vec{handle:1,marker:0}}def main()->i64{42}', 'shadow-type':'import std::collections;struct Vec<T>{handle:i64,marker:i64}def main()->i64{42}'}
  collections=snapshot/'tools/stdlib/collections.sg';digest=sha(collections)
  for name,text in fixtures.items():
   source=out/(name+'.sg');source.write_text(text,encoding='utf-8');snapshots[str(source)]=sha(source)
   r=run(name+'-five-first-call-resource-scopes',[binary,source,snapshot/'tools/stdlib',collections,digest,*(['shadow']if name!='source'else[])])
   marker=b'iterator origins: five exact resource scopes passed'if name=='source'else b'iterator origin shadow: rejected'
   if r.stdout.strip()!=marker or r.stderr:raise RuntimeError(name+' resource/oracle marker mismatch')
  report['artifacts']={'test_executable':sha(binary)};report['factory_count']=3;report['resource_scopes']=15;report['result']='passed-independent-partial-origin-layer'
 except BaseException as e:report.update(result='failed',failure=type(e).__name__+': '+str(e))
 finally:
  try:
   report['source_changes']=[p for p,h in before.items()if sha(p)!=h];report['snapshot_changes']=[p for p,h in snapshots.items()if sha(p)!=h];report['checkout_changes']=[p for p,h in origins.items()if sha(p)!=h];report['tool_changes']=[n for n,(p,h)in tools.items()if sha(p)!=h];report['cargo_config_changes']=[p for p,h in configs.items()if(sha(p)if Path(p).is_file()else None)!=h];report['source_revision_after']=revision()
   if report['source_changes']or report['snapshot_changes']or report['tool_changes']or report['cargo_config_changes']:report['result']='failed'
  except BaseException as e:report.update(result='failed',finally_failure=str(e))
  report['finished_at']=dt.datetime.now(dt.timezone.utc).isoformat();save()
 print(json.dumps({k:report.get(k)for k in ['result','failure','source_changes','snapshot_changes','checkout_changes','tool_changes']}),flush=True)
 return 0 if report['result']=='passed-independent-partial-origin-layer'else 1
if __name__=='__main__':raise SystemExit(main())
