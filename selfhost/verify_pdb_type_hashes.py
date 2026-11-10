"""Qualify deterministic SG TPI hash/index streams and real DbgHelp loaded-PDB identity."""
from __future__ import annotations
import ast,argparse,concurrent.futures,threading,datetime as dt,hashlib,json,os,platform,re,subprocess,sys,time,tomllib,struct,ctypes as c,uuid
from pathlib import Path
from pdb_type_hash_oracle import validate_tpi_hashes,records
from link_debug_gate import msf_streams
from link_multi_debug_gate import closure,rust_override
ROOT=Path(__file__).resolve().parents[1]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def revision():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
class Guid(c.Structure):_fields_=[('a',c.c_uint32),('b',c.c_uint16),('d',c.c_uint16),('e',c.c_ubyte*8)]
class Module(c.Structure):
 _fields_=[('SizeOfStruct',c.c_uint32),('BaseOfImage',c.c_uint64),('ImageSize',c.c_uint32),('TimeDateStamp',c.c_uint32),('CheckSum',c.c_uint32),('NumSyms',c.c_uint32),('SymType',c.c_uint32),('ModuleName',c.c_wchar*32),('ImageName',c.c_wchar*256),('LoadedImageName',c.c_wchar*256),('LoadedPdbName',c.c_wchar*256),('CVSig',c.c_uint32),('CVData',c.c_wchar*780),('PdbSig',c.c_uint32),('PdbSig70',Guid),('PdbAge',c.c_uint32),('PdbUnmatched',c.c_int),('DbgUnmatched',c.c_int),('LineNumbers',c.c_int),('GlobalSymbols',c.c_int),('TypeInfo',c.c_int),('SourceIndexed',c.c_int),('Publics',c.c_int),('MachineType',c.c_uint32),('Reserved',c.c_uint32)]
class Symbol(c.Structure):
 _fields_=[('SizeOfStruct',c.c_uint32),('TypeIndex',c.c_uint32),('Reserved',c.c_uint64*2),('Index',c.c_uint32),('Size',c.c_uint32),('ModBase',c.c_uint64),('Flags',c.c_uint32),('Value',c.c_uint64),('Address',c.c_uint64),('Register',c.c_uint32),('Scope',c.c_uint32),('Tag',c.c_uint32),('NameLen',c.c_uint32),('MaxNameLen',c.c_uint32),('Name',c.c_char*1)]
def dbghelp_probe(image,pdb,names):
 assert c.sizeof(Module)==3264
 kernel=c.WinDLL('kernel32',use_last_error=True);kernel.GetCurrentProcess.restype=c.c_void_p;process=kernel.GetCurrentProcess();dll=Path(os.environ['SystemRoot'])/'System32/dbghelp.dll';d=c.WinDLL(str(dll),use_last_error=True)
 d.SymInitializeW.argtypes=[c.c_void_p,c.c_wchar_p,c.c_int];d.SymInitializeW.restype=c.c_int;d.SymSetOptions.argtypes=[c.c_uint32];d.SymLoadModuleExW.argtypes=[c.c_void_p,c.c_void_p,c.c_wchar_p,c.c_wchar_p,c.c_uint64,c.c_uint32,c.c_void_p,c.c_uint32];d.SymLoadModuleExW.restype=c.c_uint64;d.SymFromName.argtypes=[c.c_void_p,c.c_char_p,c.POINTER(Symbol)];d.SymFromName.restype=c.c_int;d.SymGetModuleInfoW64.argtypes=[c.c_void_p,c.c_uint64,c.POINTER(Module)];d.SymGetModuleInfoW64.restype=c.c_int;d.SymCleanup.argtypes=[c.c_void_p];d.SymCleanup.restype=c.c_int
 r={'image':str(image),'image_sha256':sha(image),'expected_pdb':str(pdb),'expected_pdb_sha256':sha(pdb),'dbghelp':str(dll),'dbghelp_sha256':sha(dll),'symbols':[]};assert d.SymInitializeW(process,str(image.parent),0);d.SymSetOptions(4|16|2|1024)
 try:
  c.set_last_error(0);base=d.SymLoadModuleExW(process,None,str(image),'sgprobe',0,0,None,0);r.update(base=base,load_error=c.get_last_error())
  for name in names:
   buf=c.create_string_buffer(c.sizeof(Symbol)+2048);s=c.cast(buf,c.POINTER(Symbol));s.contents.SizeOfStruct=c.sizeof(Symbol);s.contents.MaxNameLen=2048;c.set_last_error(0);ok=d.SymFromName(process,('sgprobe!'+name).encode(),s);r['symbols'].append({'name':name,'ok':bool(ok),'error':c.get_last_error(),'rva':s.contents.Address-base if ok else None})
  m=Module();m.SizeOfStruct=c.sizeof(Module);c.set_last_error(0);ok=d.SymGetModuleInfoW64(process,base,c.byref(m));r['module']={'ok':bool(ok),'error':c.get_last_error()}
  if ok:
   r['module'].update(loaded_pdb=m.LoadedPdbName,guid=str(uuid.UUID(bytes_le=bytes(m.PdbSig70))),age=m.PdbAge,unmatched=bool(m.PdbUnmatched),dbg_unmatched=bool(m.DbgUnmatched))
   if m.LoadedPdbName:
    actual=Path(m.LoadedPdbName).resolve();raw=actual.read_bytes();info=msf_streams(raw)[1];r['module'].update(loaded_pdb_sha256=hashlib.sha256(raw).hexdigest(),path_matches=actual==pdb.resolve(),identity_matches=info[12:28]==bytes(m.PdbSig70)and struct.unpack_from('<I',info,8)[0]==m.PdbAge)
 finally:assert d.SymCleanup(process)
 return r

def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--output',type=Path,default=ROOT/'target/selfhost/m5-pdb-type-hash/checkpoint');p.add_argument('--seed',type=Path,default=ROOT/'target/agent-conditional-compiler/bin/sgc.exe');p.add_argument('--clang',type=Path,default=Path('C:/Users/admin/AppData/Local/Packages/OpenAI.Codex_2p2nqsd0c76g0/LocalCache/Local/SengooToolchain/clang+llvm-19.1.7-x86_64-pc-windows-msvc/bin/clang.exe'));p.add_argument('--formatter',type=Path,default=ROOT/'target/agent-formatter/debug/sgfmt.exe');p.add_argument('--lldb-python-home',type=Path,default=ROOT/'target/selfhost/m5-linker-dev/lldb-python310');p.add_argument('--focused-only',action='store_true');p.add_argument('--reuse-passed-from',type=Path);p.add_argument('--reuse-sha256');p.add_argument('--probe-image',type=Path);p.add_argument('--probe-pdb',type=Path);p.add_argument('--probe-names',default='worker,main,sengoo_deflate_fixture_length,sengoo_deflate_fixture_check');a=p.parse_args()
 if a.probe_image:
  print(json.dumps(dbghelp_probe(a.probe_image.resolve(),a.probe_pdb.resolve(),a.probe_names.split(',')),indent=2));return 0
 out=a.output.resolve()
 if not out.is_relative_to(ROOT/'target'):raise ValueError('output must remain under target')
 out.mkdir(parents=True,exist_ok=True);before={};origins={};snapshots={};tools={};configs={};report={'schema':1,'stage':'pdb-type-hash-index','result':'running','m5_complete':False,'public_only_lldb19_names_supported':False,'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'source_revision':revision(),'host':platform.platform(),'python':sys.version,'steps':[],'regressions':{}}
 lock_save=threading.Lock()
 def save():
  with lock_save:
   temp=out/'evidence.partial.json';temp.write_text(json.dumps(report,indent=2)+'\n',encoding='utf8');temp.replace(out/'evidence.json')
 save()
 try:
  lock=json.loads((ROOT/'selfhost/bootstrap.lock').read_bytes());rustbin=Path(os.environ.get('RUSTUP_HOME',str(Path.home()/'.rustup')))/'toolchains'/(lock['rust_toolchain']+'-x86_64-pc-windows-msvc')/'bin';selected={'seed':a.seed.resolve(),'clang':a.clang.resolve(),'formatter':a.formatter.resolve(),'python':Path(sys.executable),'rustc':rustbin/'rustc.exe','cargo':rustbin/'cargo.exe'};selected.update({name:a.clang.resolve().parent/exe for name,exe in [('lldb','lldb.exe'),('liblldb','liblldb.dll'),('pdbutil','llvm-pdbutil.exe'),('readobj','llvm-readobj.exe'),('lld','lld-link.exe')]})
  for path in [*a.lldb_python_home.glob('*.dll'),*a.lldb_python_home.glob('*.pyd'),a.lldb_python_home/'python310.zip']:selected['lldb-'+path.name]=path.resolve()
  for name,path in selected.items():
   digest=sha(path)
   expected=lock[name]['sha256']if name in ('seed','clang','formatter')else lock[name+'_sha256']if name in ('rustc','cargo')else None
   if expected and expected!=digest:raise RuntimeError(name+' identity mismatch')
   tools[name]={'path':str(path),'sha256':digest}
  tools['dbghelp']={'path':str(Path(os.environ['SystemRoot'])/'System32/dbghelp.dll'),'sha256':sha(Path(os.environ['SystemRoot'])/'System32/dbghelp.dll')};report['tools']=tools
  entries=[ROOT/'selfhost/tests/pdb_type_hashes.sg'];paths={}
  for entry in entries:paths.update(closure(entry))
  source=out/'implementation'
  for path,digest in paths.items():
   raw=path.read_bytes();assert hashlib.sha256(raw).hexdigest()==digest;dest=source/path.relative_to(ROOT);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(raw);origins[str(path)]=digest;snapshots[str(dest)]=sha(dest)
  for name in ['tools/stdlib','runtime','Cargo.toml','Cargo.lock','rust-toolchain.toml','selfhost/bootstrap.lock']:
   path=ROOT/name
   for file in path.rglob('*')if path.is_dir()else [path]:
    if not file.is_file()or any(x in {'target','build','__pycache__'}for x in file.relative_to(ROOT).parts):continue
    raw=file.read_bytes();dest=source/file.relative_to(ROOT);dest.parent.mkdir(parents=True,exist_ok=True);dest.write_bytes(raw);origins[str(file)]=hashlib.sha256(raw).hexdigest();snapshots[str(dest)]=sha(dest)
  python_pending=[Path(__file__).resolve(),*[ROOT/'selfhost'/name for name in ['link_debug_gate.py','link_native_debug_gate.py','mixeddebug_gate.py','link_units_gate.py']]];python_seen=set()
  while python_pending:
   path=python_pending.pop()
   if path in python_seen:continue
   python_seen.add(path);raw=path.read_bytes();before[str(path)]=hashlib.sha256(raw).hexdigest()
   for node in ast.walk(ast.parse(raw)):
    names=[node.module]if isinstance(node,ast.ImportFrom)and node.module else [alias.name for alias in node.names]if isinstance(node,ast.Import)else []
    for name in names:
     local=ROOT/'selfhost'/(name+'.py')
     if local.is_file():python_pending.append(local)
  for path in(ROOT/'openspec/changes/add-selfhost-pdb-type-hash-index').rglob('*.md'):
   if path.name!='tasks.md':before[str(path)]=sha(path)
  if any(sha(path)!=digest for path,digest in origins.items()):raise RuntimeError('mixed producer snapshot')
  report.update(inputs_before=before,producer_origins=origins,producer_snapshots=snapshots)
  env={k:v for k,v in os.environ.items()if not k.upper().startswith(('SENGOO_','PYTHON','LLDB_','SGPLATFORM_','SDL2_'))and not rust_override(k)};env.update(SENGOO_STDLIB=str(source/'tools/stdlib'),SENGOO_RUNTIME=str(source/'tools/stdlib/runtime.c'),SENGOO_CLANG=str(a.clang.resolve()),SENGOO_MODULE_MAP=';'.join('selfhost_'+Path(path).stem+'='+str(source/Path(path).relative_to(ROOT))for path in paths if Path(path).parent==ROOT/'selfhost/src'),CARGO=str(rustbin/'cargo.exe'),RUSTC=str(rustbin/'rustc.exe'),CARGO_BUILD_RUSTC=str(rustbin/'rustc.exe'),RUSTUP_TOOLCHAIN=rustbin.parent.name,RUSTC_WRAPPER='',RUSTC_WORKSPACE_WRAPPER='',CARGO_BUILD_RUSTC_WRAPPER='',CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='');env['PATH']=str(rustbin)+os.pathsep+str(a.clang.resolve().parent)+os.pathsep+env.get('PATH','')
  cargo_home=Path(env.get('CARGO_HOME',str(Path.home()/'.cargo')));candidates={parent/'.cargo'/name for parent in(ROOT,*ROOT.parents,source,*source.parents)for name in('config','config.toml')};candidates.update(cargo_home/name for name in('config','config.toml'))
  for path in candidates:
   raw=path.read_bytes()if path.is_file()else None;configs[str(path)]=hashlib.sha256(raw).hexdigest()if raw is not None else None
   if raw is not None and 'include'in tomllib.loads(raw.decode('utf8')):raise RuntimeError('unqualified included Cargo configuration')
  report['cargo_config_candidates']=configs
  def run(name,command,code=0):
   start=time.monotonic();done=subprocess.run(list(map(str,command)),cwd=ROOT,env=env,capture_output=True,timeout=1800);stdout=out/(name+'.stdout.log');stderr=out/(name+'.stderr.log');stdout.write_bytes(done.stdout);stderr.write_bytes(done.stderr);report['steps'].append({'name':name,'command':list(map(str,command)),'exit_code':done.returncode,'expected_exit_code':code,'elapsed_s':time.monotonic()-start,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)});save();print(name,done.returncode,flush=True)
   if done.returncode!=code:raise RuntimeError(name+' failed')
   return done
  for path in [ROOT/'selfhost/src'/name for name in ['link_debug.sg','link_pdb.sg','link_pdb_modules.sg']]+entries:run('format-'+path.stem,[a.formatter,'--check',path])
  binary=out/'focused.exe';run('build-focused',[a.seed,'--runtime-mode','source-development','build',source/'selfhost/tests/pdb_type_hashes.sg','--force-rebuild','-O','0','-o',binary]);r=run('first-call-resources',[binary,out/'types.msf'])
  if r.stdout.strip()!=b'PDB TPI hashes: three first-call resource scopes passed'or r.stderr:raise RuntimeError('focused marker mismatch')
  arrays=msf_streams((out/'types.msf').read_bytes());report['focused_types']=validate_tpi_hashes(arrays);controls=[]
  for label,target,at,value,width in [('missing',2,20,65535,2),('aux',2,22,11,2),('keysize',2,24,8,4),('buckets',2,28,4096,4),('hash-extent',2,36,0,4),('index-offset',2,40,0,4),('index-length',2,44,0,4),('adjustment',2,52,4,4),('hash-byte',11,0,0,4),('index-type',11,len(records(arrays[2]))*4,0,4),('index-record-offset',11,len(records(arrays[2]))*4+4,1,4)]:
   changed=arrays.copy();raw=bytearray(changed[target]);raw[at:at+width]=value.to_bytes(width,'little');changed[target]=raw
   try:validate_tpi_hashes(changed)
   except (AssertionError,ValueError,struct.error):controls.append(label)
   else:raise RuntimeError('oracle accepted '+label)
  report['oracle_rejections']=controls
  if not a.focused_only:
   common=['--seed',a.seed,'--clang',a.clang,'--formatter',a.formatter,'--lldb-python-home',a.lldb_python_home]
   reuse={}
   if a.reuse_passed_from:
    reuse_path=a.reuse_passed_from.resolve();reuse_raw=reuse_path.read_bytes()
    if not a.reuse_sha256 or hashlib.sha256(reuse_raw).hexdigest()!=a.reuse_sha256:raise RuntimeError('reused parent report pin mismatch')
    before[str(reuse_path)]=a.reuse_sha256;reuse_parent=json.loads(reuse_raw)
    if reuse_parent.get('stage')!='pdb-type-hash-index':raise RuntimeError('unrelated reused parent')
    reuse=reuse_parent.get('regressions',{});report['reuse_parent']={'report':str(reuse_path),'sha256':a.reuse_sha256,'result':reuse_parent['result'],'scope':'only independently passed children with all exact current sources/artifacts/tools/configs reverified; failed parent/native producer are not accepted'}
   def child(name,script,extra=()):
    if name in reuse:
     item=reuse[name];path=Path(item['report']);data=json.loads(path.read_bytes())
     if sha(path)!=item['sha256']or data.get('result')!='passed'or any(data.get(k)for k in ['changed_sources','changed_tools','changed_configs','checkout_changes']):raise RuntimeError('reused child failed or changed')
     verified={str(path):item['sha256']};python_pending=[ROOT/'selfhost'/script];python_seen=set();bound_python={}
     while python_pending:
      python_path=python_pending.pop()
      if python_path in python_seen:continue
      python_seen.add(python_path);python_raw=python_path.read_bytes();digest=reuse_parent['inputs_before'].get(str(python_path))
      if not digest or hashlib.sha256(python_raw).hexdigest()!=digest:raise RuntimeError('reused actual Python harness/import changed or unbound: '+str(python_path))
      copy=out/'reused-python'/name/python_path.relative_to(ROOT);copy.parent.mkdir(parents=True,exist_ok=True);copy.write_bytes(python_raw);snapshots[str(copy)]=digest;bound_python[str(python_path)]={'sha256':digest,'snapshot':str(copy)};verified[str(python_path)]=digest
      for node in ast.walk(ast.parse(python_raw)):
       names=[node.module]if isinstance(node,ast.ImportFrom)and node.module else [alias.name for alias in node.names]if isinstance(node,ast.Import)else []
       for module_name in names:
        local=ROOT/'selfhost'/(module_name+'.py')
        if local.is_file():python_pending.append(local)
     for source_name,digest in data['sources'].items():
      actual=ROOT/source_name
      if sha(actual)!=digest:raise RuntimeError('reused child source changed: '+source_name)
      verified[str(actual)]=digest
     for key in ['producer_origins','producer_source_origins','qualified_checkout_sources']:
      for source_name,digest in data.get(key,{}).items():
       actual=ROOT/source_name
       if sha(actual)!=digest:raise RuntimeError('reused producer source changed: '+source_name)
       verified[str(actual)]=digest
     for relative,digest in data['artifacts'].items():
      artifact=path.parent/relative
      if sha(artifact)!=digest:raise RuntimeError('reused artifact changed: '+relative)
      verified[str(artifact)]=digest
     for tool in data['tools'].values():
      if sha(tool['path'])!=tool['sha256']:raise RuntimeError('reused child tool changed')
      verified[tool['path']]=tool['sha256']
     for cfg,digest in data.get('cargo_config_candidates',{}).items():
      if(sha(cfg)if Path(cfg).is_file()else None)!=digest:raise RuntimeError('reused child Cargo config changed')
      configs[cfg]=digest
     before.update(verified);report['regressions'][name]={'report':str(path),'sha256':item['sha256'],'steps':len(data['steps']),'reused_independent_passed_child':True,'reverified_inputs':verified,'bound_actual_python_imports':bound_python,'python_snapshot_provenance':'reconstructed exact bytes matched to prior pre-run recorded input SHA; changed or unrecorded consumed imports are rejected'};save();print('reuse-'+name,'passed',flush=True);return path
    dest=out/name;r=run('regression-'+name,[sys.executable,ROOT/'selfhost'/script,*common,'--output',dest,*extra]);path=dest/'evidence.json';data=json.loads(path.read_bytes());assert data['result']=='passed'and not any(data.get(k)for k in ('changed_sources','changed_tools','changed_configs','checkout_changes'));report['regressions'][name]={'report':str(path),'sha256':sha(path),'steps':len(data['steps'])};save();return path
   with concurrent.futures.ThreadPoolExecutor(max_workers=3)as pool:
    futures=[pool.submit(child,'typed-source-step-unwind','link_debug_gate.py'),pool.submit(child,'native-typed','link_native_debug_gate.py'),pool.submit(child,'mixed','mixeddebug_gate.py')]
    results=[f.result()for f in futures]
   child('units','link_units_gate.py',['--native-evidence',results[1]])
   mixed=Path(report['regressions']['mixed']['report']).parent;pair=mixed/'fresh-a/program';pdb=pair.with_suffix('.pdb');image=pair.with_suffix('.exe');r=run('actual-dbghelp',[sys.executable,Path(__file__).resolve(),'--probe-image',image,'--probe-pdb',mixed/'fixed.pdb']);actual=json.loads(r.stdout);m=actual['module'];assert all(s['ok']for s in actual['symbols'])and m['ok']and m['path_matches']and m['identity_matches']and not m['unmatched']and not m['dbg_unmatched'];report['actual_dbghelp']=actual
   historical=ROOT/'target/selfhost/m5-mixeddebug-sdk-qualified';old_report=historical/'evidence.json';before[str(old_report)]=sha(old_report);assert sha(old_report)=='f0bcc7423d652813588ffe67f80eff9318a691b325e2d17e93252ce2cb1022e8';old_pdb=historical/'fresh-a/program.pdb';old_image=historical/'fresh-a/program.exe';before[str(old_pdb)]=sha(old_pdb);before[str(old_image)]=sha(old_image);old=msf_streams(old_pdb.read_bytes());new=msf_streams(pdb.read_bytes());assert old[2][56:]==new[2][56:]and old[2][8:20]==new[2][8:20];assert old[1][12:28]!=new[1][12:28];report['historical_type_records_and_ids_unchanged']=True;report['new_guid_differs_from_hashless_profile']=True
   r=run('historical-hashless-dbghelp',[sys.executable,Path(__file__).resolve(),'--probe-image',old_image,'--probe-pdb',historical/'fixed.pdb']);negative=json.loads(r.stdout);assert not negative['base']and all(not s['ok']for s in negative['symbols']);report['historical_hashless_control']=negative
  report['artifacts']={p.name:sha(p)for p in [binary,out/'types.msf']};report['result']='passed-focused-development'if a.focused_only else 'passed-pdb-type-hash-index'
 except BaseException as error:report.update(result='failed',failure=repr(error))
 finally:
  report.update(source_changes=[p for p,h in before.items()if sha(p)!=h],snapshot_changes=[p for p,h in snapshots.items()if sha(p)!=h],checkout_changes=[p for p,h in origins.items()if sha(p)!=h],tool_changes=[n for n,v in tools.items()if sha(v['path'])!=v['sha256']],cargo_config_changes=[p for p,h in configs.items()if(sha(p)if Path(p).is_file()else None)!=h],source_revision_after=revision(),finished_at=dt.datetime.now(dt.timezone.utc).isoformat())
  if any(report[k]for k in ['source_changes','snapshot_changes','tool_changes','cargo_config_changes']):report['result']='failed'
  save()
 print(json.dumps({k:report.get(k)for k in ['result','failure','source_changes','snapshot_changes','checkout_changes','tool_changes']}),flush=True);return 0 if report['result'].startswith('passed-')else 1
if __name__=='__main__':raise SystemExit(main())
