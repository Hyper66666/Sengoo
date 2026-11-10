"""Qualify independent SG COFF units with scoped symbols and merged PDB modules."""
from pathlib import Path
import sys,os,json,hashlib,re,subprocess,datetime,platform,shutil,tomllib
import argparse
from link_debug_gate import ROOT
from link_units_oracle import inspect_units, oracle_controls
from link_multi_debug_gate import closure, rust_override, coff_fragments
def coff_mutations(raw):
    rows={raw[20+i*40:28+i*40].rstrip(b'\0'):(int.from_bytes(raw[20+i*40+20:20+i*40+24],'little'),int.from_bytes(raw[20+i*40+16:20+i*40+20],'little'))for i in range(int.from_bytes(raw[2:4],'little'))}
    types,size=rows[b'.debug$T'];at=types+4
    while int.from_bytes(raw[at+2:at+4],'little')!=0x1203:at+=int.from_bytes(raw[at:at+2],'little')+2
    fragments=coff_fragments(raw);_,checksums,_=fragments[0xF4][0];_,lines,_=fragments[0xF2][0];_,symbols,symbol_size=fragments[0xF1][0];cursor=symbols
    while int.from_bytes(raw[cursor+2:cursor+4],'little')!=0x110F:cursor+=int.from_bytes(raw[cursor:cursor+2],'little')+2
    def change(offset,value,width):
        changed=bytearray(raw);changed[offset:offset+width]=value.to_bytes(width,'little');return bytes(changed)
    return [('forward-module-member-type',change(at+8,0x7FFFFFFF,4),7301),('unknown-module-type-leaf',change(at+2,0x7FFF,2),7303),('shared-source-checksum',change(checksums+46,raw[checksums+46]^128,1),7301),('static-procedure-mislabeled-global',change(cursor+2,0x1110,2),7331),('known-file-wrong-function',change(lines+12,40-int.from_bytes(raw[lines+12:lines+16],'little'),4),7301)]

parser=argparse.ArgumentParser(description="Qualify multiple independent SG COFF debug modules")
for name in ["seed","clang","formatter","lldb-python-home","output"]:parser.add_argument("--"+name,required=True,type=Path)
parser.add_argument('--native-evidence',type=Path,default=ROOT/'target/selfhost/m5-native-debug-multi-file-regression/evidence.json')
args=parser.parse_args();root=ROOT;base=args.output.resolve();assert base.is_relative_to(root/'target');base.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
entry=root/'selfhost/tests/link_native_debug_units.sg';originals=closure(entry);originals.update(closure(root/'selfhost/tests/link_debug_units.sg'));originals.update({p:sha(p) for p in (root/'tools/stdlib').rglob('*') if p.is_file() and not any(x in {'target','build','__pycache__'} for x in p.relative_to(root/'tools/stdlib').parts)})
inputs={str(Path(__file__).resolve()):sha(Path(__file__)),str(root/'selfhost/link_units_oracle.py'):sha(root/'selfhost/link_units_oracle.py'),str(root/'selfhost/link_multi_debug_gate.py'):sha(root/'selfhost/link_multi_debug_gate.py'),str(root/'selfhost/link_debug_gate.py'):sha(root/'selfhost/link_debug_gate.py'),str(root/'selfhost/bootstrap.lock'):sha(root/'selfhost/bootstrap.lock')}
snapshot=base/'implementation'
for p,digest in originals.items():
 raw=p.read_bytes();assert hashlib.sha256(raw).hexdigest()==digest;copy=snapshot/p.relative_to(root);copy.parent.mkdir(parents=True,exist_ok=True);copy.write_bytes(raw);assert sha(copy)==digest;inputs[str(copy)]=digest
assert all(sha(p)==h for p,h in originals.items())
for p in (root/'runtime').rglob('*'):
 if p.is_file() and not any(x in {'target','build','__pycache__'} for x in p.relative_to(root/'runtime').parts):inputs[str(p)]=sha(p)
for name in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:inputs[str(root/name)]=sha(root/name)
for p in sorted((root/'openspec/changes/add-selfhost-linker-debug').rglob('*.md')):
 raw=p.read_bytes();copy=snapshot/p.relative_to(root);copy.parent.mkdir(parents=True,exist_ok=True);copy.write_bytes(raw);inputs[str(copy)]=hashlib.sha256(raw).hexdigest()

llvm=args.clang.resolve().parent;py=args.lldb_python_home.resolve()
tools={'seed':args.seed.resolve(),'formatter':args.formatter.resolve(),'clang':args.clang.resolve(),'readobj':llvm/'llvm-readobj.exe','pdbutil':llvm/'llvm-pdbutil.exe','lldb':llvm/'lldb.exe','liblldb':llvm/'liblldb.dll','python':Path(sys.executable)}
for p in [*py.glob('*.dll'),*py.glob('*.pyd'),py/'python310.zip']:tools['lldb-'+p.name]=p
build={k:v for k,v in os.environ.items() if not k.upper().startswith(('SENGOO_','SGPLATFORM_','SDL2_','PYTHON','LLDB_')) and not rust_override(k)};build['PATH']=str(llvm)+os.pathsep+build.get('PATH','')
for name in ['cargo','rustc','rustup']:tools[name]=Path(shutil.which(name,path=build['PATH'])).resolve()
for name in ['cargo','rustc']:tools[name+'-selected']=Path(subprocess.check_output([tools['rustup'],'which',name],env=build,cwd=root,text=True).strip()).resolve()
assert tools['cargo-selected'].parent==tools['rustc-selected'].parent
build.update(CARGO=str(tools['cargo-selected']),RUSTC=str(tools['rustc-selected']),RUSTC_WRAPPER='',RUSTC_WORKSPACE_WRAPPER='',CARGO_BUILD_RUSTC=str(tools['rustc-selected']),CARGO_BUILD_RUSTC_WRAPPER='',CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='',RUSTUP_TOOLCHAIN=tools['rustc-selected'].parent.parent.name);build['PATH']=str(tools['cargo-selected'].parent)+os.pathsep+build['PATH']
build.update(SENGOO_STDLIB=str(snapshot/'tools/stdlib'),SENGOO_RUNTIME=str(snapshot/'tools/stdlib/runtime.c'),SENGOO_CLANG=str(tools['clang']),SENGOO_MODULE_MAP=';'.join('selfhost_'+p.stem+'='+str(snapshot/p.relative_to(root)) for p in originals if p.parent==root/'selfhost/src'))
pure={k:v for k,v in build.items() if not k.upper().startswith('SENGOO_')};pure['PATH']=''
debug=dict(pure,PYTHONHOME=str(py),PYTHONPATH=str(py/'python310.zip')+os.pathsep+str(py),LLDB_USE_NATIVE_PDB_READER='1');debug['PATH']=str(py)+os.pathsep+str(llvm)
report={'schema':1,'stage':'independent-sg-coff-debug-modules','result':'running','production_admitted':False,'host':platform.platform(),'source_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sources':inputs,'producer_origins':{str(p):h for p,h in originals.items()},'qualified_checkout_sources':{str(p):h for p,h in originals.items()},'tools':{k:{'path':str(v),'sha256':sha(v)} for k,v in tools.items()},'steps':[],'limits':['bounded all-debug SG inputs: every actual COFF object must supply one supported debug unit; nondebug SDK/C objects and archive-unit admission remain open','two independent semantic/MIR/native/object compilations; native instruction/address facts are unmodified','same-named helper visibility is explicitly classified as COFF storage3 by this test caller before emission; automatic compiler internalization is not qualified','two fresh artifact directories share identical actual source inputs, logical module names and RSDS path; different source checkout directories/path remapping and M6/M8 reproduction remain open','product object/link/debug read/write commands use empty PATH; SG seed development C/Rust runtime remains recorded']}
configs={p/'.cargo'/n for p in [root,*root.parents] for n in ['config','config.toml']};configs.update(Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))/n for n in ['config','config.toml']);report['cargo_config_candidates']={}
for p in configs:
 raw=p.read_bytes() if p.is_file() else None;report['cargo_config_candidates'][str(p)]=None if raw is None else hashlib.sha256(raw).hexdigest()
 if raw is not None:assert 'include' not in tomllib.loads(raw.decode('utf-8'))
def run(name,command,mode='build',code=0):
 done=subprocess.run(list(map(str,command)),cwd=root,env=pure if mode=='pure' else debug if mode=='debug' else build,capture_output=True,timeout=900);(base/(name+'.stdout.log')).write_bytes(done.stdout);(base/(name+'.stderr.log')).write_bytes(done.stderr);report['steps'].append({'name':name,'command':list(map(str,command)),'exit_code':done.returncode,'expected_exit_code':code,'empty_path':mode=='pure'});print(name,done.returncode,flush=True);assert done.returncode==code,(name,done.returncode,done.stderr);return done.stdout+done.stderr
try:
 lock=json.loads((root/'selfhost/bootstrap.lock').read_text());assert report['tools']['seed']['sha256']==lock['seed']['sha256'] and report['tools']['clang']['sha256']==lock['clang']['sha256'] and report['tools']['formatter']['sha256']==lock['formatter']['sha256']
 for name in ['cargo','rustc']:report['tools'][name+'-selected']['version']=run(name+'-version',[tools[name+'-selected'],'-vV']).decode('utf-8')
 probe=base/'rust-tool-probe';(probe/'src').mkdir(parents=True);(probe/'Cargo.toml').write_text('[package]\nname="debug-units-rust-tool"\nversion="0.0.0"\nedition="2021"\n[workspace]\n',encoding='utf-8');(probe/'build.rs').write_text('fn main(){println!("cargo:warning=units_rustc={}",std::env::var("RUSTC").unwrap());}\n',encoding='utf-8');(probe/'src/lib.rs').write_text('pub fn probe()->u64{42}\n',encoding='utf-8')
 for path in [probe/'Cargo.toml',probe/'build.rs',probe/'src/lib.rs']:inputs[str(path)]=sha(path)
 observed=run('cargo-actual-rustc',[tools['cargo-selected'],'build','--offline','--verbose','--manifest-path',probe/'Cargo.toml','--target-dir',probe/'target']).decode('utf-8');compiler_runs=[line for line in observed.splitlines()if line.lstrip().startswith('Running ') and str(tools['rustc-selected']).casefold()in line.casefold()];assert len(compiler_runs)>=2 and 'units_rustc='+str(tools['rustc-selected'])in observed;report['cargo_actual_rustc_invocations']=compiler_runs

 for path in [p for p in originals if p.suffix=='.sg' and (p.name in {'link_debug_units.sg','link_native_debug_units.sg'} or p.stem.startswith('link_'))]:run('format-'+path.stem,[tools['formatter'],'--check',snapshot/path.relative_to(root)])
 fast=base/'unit-model.exe';run('build-unit-model',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/link_debug_units.sg','--force-rebuild','-O','0','-o',fast])
 prior=args.native_evidence.resolve();raw=prior.read_bytes();qualified=json.loads(raw);assert qualified['result']=='passed' and not qualified['changed_sources'] and not qualified['changed_tools'];inputs[str(prior)]=hashlib.sha256(raw).hexdigest()
 for name,digest in qualified['sources'].items():assert sha(root/name)==digest;inputs[str(root/name)]=digest
 report['qualifications']={'legacy_native_objects':{'report':str(prior),'sha256':hashlib.sha256(raw).hexdigest(),'sources':qualified['sources'],'artifacts':qualified['artifacts']}}
 fixtures=[('multiple-functions','link_native_debug_source.sg'),('multiple-returns','link_debug_ranges_source.sg'),('floating-parameters','link_native_debug_types_source.sg'),('mixed-scalar-parameters','link_native_debug_scalars_source.sg'),('readonly-reference','link_native_debug_refs_source.sg'),('mutable-reference','link_native_debug_mut_ref_source.sg'),('raw-void-pointer','link_native_debug_raw_ptr_source.sg'),('aggregate-locals','link_native_debug_aggregate_source.sg'),('tagged-enum-local','link_native_debug_enum_source.sg')]
 single=[]
 for name,source_name in fixtures:
  obj=prior.parent/(name+'.obj');assert sha(obj)==qualified['artifacts'][obj.name];inputs[str(obj)]=sha(obj);source=root/'selfhost/tests'/source_name;assert sha(source)==qualified['sources'][str(source.relative_to(root))];inputs[str(source)]=sha(source);command=[fast,obj,source.as_posix(),base/(name+'.exe'),base/(name+'.pdb')];assert run('n1-exact-bytes-'+name,command,'pure').strip()==b'one-unit exact legacy bytes and first-call resources: ok';run('n1-execute-'+name,[base/(name+'.exe')],'pure',42);single.append({'name':name,'same_path_old_and_new_raw_pe_pdb_equal':True,'first_call_resource_scopes':5})
 negatives=[];sentinel=b'preserve previous module PE/PDB'
 for n in range(1,23):
  image=base/'reject-single.exe';pdb=base/'reject-single.pdb';image.write_bytes(sentinel);pdb.write_bytes(sentinel);code=7301 if n==21 else 7331
  out=run('reject-n1-state-'+str(n),[fast,prior.parent/'multiple-functions.obj',(root/'selfhost/tests/link_native_debug_source.sg').as_posix(),image,pdb,chr(64+n)],'pure',65);assert out.strip()==str(code).encode() and image.read_bytes()==pdb.read_bytes()==sentinel;negatives.append({'scope':'one-unit','case':n,'diagnostic':code,'first_call_resources':5})
 binary=base/'native-units.exe';run('build-independent-native-units',[tools['seed'],'--runtime-mode','source-development','build',snapshot/entry.relative_to(root),'--force-rebuild','-O','0','-o',binary])
 for mode in ['--fail-equality','--fail-truth']:
  failed=run('assertion'+mode,[binary,mode],'pure',1);assert b'link_native_debug_units.sg' in failed
 variants={
 'parameter':{'common.sg':'pub def helper()->i64 {\n return 2;\n}\n','main.sg':'import common;\nextern "C" { fn worker(value:i64)->i64; }\nstruct Local { input:i64, extra:i64 }\ndef main()->i64 {\n let state:Local=Local{input:38,extra:2};\n let input:i64=state.input+helper();\n let answer:i64=worker(input);\n return answer;\n}\n','worker.sg':'import common;\nstruct Local { count:i64, step:i64 }\npub def worker(value:i64)->i64 {\n let state:Local=Local{count:value,step:helper()};\n let answer:i64=state.count+state.step;\n return answer;\n}\n'},
 'type-4098':{'common.sg':'pub def helper()->i64 {\n return 2;\n}\n','main.sg':'import common;\nextern "C" { fn worker()->i64; }\nstruct Local { input:i64, extra:i64 }\ndef main()->i64 {\n let state:Local=Local{input:38,extra:2};\n let input:i64=state.input+helper();\n let answer:i64=worker()+input-40;\n return answer;\n}\n','worker.sg':'import common;\nstruct Local { count:i64, step:i64 }\npub def worker()->i64 {\n let state:Local=Local{count:40,step:helper()};\n let answer:i64=state.count+state.step;\n return answer;\n}\n'}}
 inspections={};positive={}
 for variant,files in variants.items():
  source=base/('sources 涓栫晫 & '+variant);source.mkdir();paths=[source/name for name in ['main.sg','common.sg','worker.sg']]
  for name,text in files.items():p=source/name;p.write_text(text,encoding='utf-8',newline='\n');inputs[str(p)]=sha(p)
  pinned=base/('fixed-'+variant+'.pdb');fresh=[]
  for round in ['fresh-a','fresh-b']:
   directory=base/(variant+'-'+round);directory.mkdir();obj=[directory/'main.obj',directory/'worker.obj'];image=directory/'program.exe';pdb=directory/'program.pdb';command=[binary,paths[0],paths[2],*obj,image,pdb,'@',pinned.as_posix()];assert run(variant+'-'+round+'-source-coff-link-pdb',command,'pure').strip()==b'independent COFF modules: ok';run(variant+'-'+round+'-execute',[image],'pure',42);fresh.append([p.read_bytes()for p in [*obj,image,pdb]])
  assert fresh[0]==fresh[1];pinned.write_bytes(fresh[0][3]);directory=base/(variant+'-fresh-a');obj=[directory/'main.obj',directory/'worker.obj'];image=directory/'program.exe';pdb=directory/'program.pdb'
  for name,path in zip(['main','worker'],obj):run(variant+'-inspect-'+name,[tools['readobj'],'--codeview','--symbols','--relocations',path])
  run(variant+'-inspect-pdb',[tools['pdbutil'],'dump','--modules','--files','-l','--types','--symbols','--globals','--publics',pdb]);inspections[variant]=inspect_units(fresh[0][0],fresh[0][1],fresh[0][3],paths,['sg-main.obj','sg-worker.obj']);inspections[variant]['oracle_negative_controls']=oracle_controls(fresh[0][0],fresh[0][1],fresh[0][3],paths,['sg-main.obj','sg-worker.obj']);positive[variant]={'source_paths':[str(p)for p in paths],'logical_module_names':['sg-main.obj','sg-worker.obj'],'fixed_rsds_path':pinned.as_posix(),'fresh_artifact_directories':[str(base/(variant+'-'+r))for r in ['fresh-a','fresh-b']],'obj_pe_pdb_raw_bytes_identical':True,'artifacts':{str(p.relative_to(base)):sha(p)for p in [*obj,image,pdb,pinned]}}
  if variant=='type-4098':assert inspections[variant]['local_4098_leaf_kinds']==[0x1203,0x1203]
  if variant=='parameter':
   frame='settings set frame-format "SG_PHYSICAL ${line.file.fullpath}:${line.number}\\n"';commands=[frame,'breakpoint set --file "'+paths[0].as_posix()+'" --line 7','run','frame variable input state','thread step-in','thread step-over','frame variable value state','thread step-over','frame variable answer','thread step-out','thread step-over','frame variable answer','continue'];live=run('live-cross-unit-step',[tools['lldb'],'--batch','--no-lldbinit',*[v for c in commands for v in ['-o',c]],image],'debug')
   for value in [b'input = 40',b'input = 38, extra = 2',b'value = 40',b'count = 40, step = 2',b'answer = 42',b'exited with status = 42']:assert value in live,value
   for path,line in [(paths[0],7),(paths[2],4),(paths[2],5),(paths[2],6),(paths[0],8)]:assert ('SG_PHYSICAL '+str(path)+':'+str(line)).encode() in live
   commands=['settings set frame-format "SG_PHYSICAL ${line.file.fullpath}:${line.number} function=${function.name}\\n"','breakpoint set --file "'+paths[1].as_posix()+'" --line 2','run','bt','continue','bt','continue'];helper=run('live-two-same-name-static-helpers',[tools['lldb'],'--batch','--no-lldbinit',*[v for c in commands for v in ['-o',c]],image],'debug');assert b'locations = 2' in helper or b'2 locations' in helper;assert b'function=helper' in helper and b'function=main' in helper and b'function=worker' in helper and b'exited with status = 42'in helper
   for label,changed,code in coff_mutations(fresh[0][1]):
    malformed=base/(label+'.obj');malformed.write_bytes(changed);inputs[str(malformed)]=sha(malformed);rejected_image=base/'rejected.exe';rejected_pdb=base/'rejected.pdb';rejected_image.write_bytes(sentinel);rejected_pdb.write_bytes(sentinel);out=run('reject-coff-'+label,[binary,paths[0],paths[2],obj[0],malformed,rejected_image,rejected_pdb,'read',pinned.as_posix()],'pure',65);assert out.strip()==str(code).encode() and rejected_image.read_bytes()==rejected_pdb.read_bytes()==sentinel;negatives.append({'scope':'input-coff','name':label,'diagnostic':code,'first_call_resources':5})
   for n in range(1,26):
    rejected_image=base/'rejected.exe';rejected_pdb=base/'rejected.pdb';rejected_image.write_bytes(sentinel);rejected_pdb.write_bytes(sentinel);code=7301 if n==3 else 7333 if n==4 else 7331;out=run('reject-multi-state-'+str(n),[binary,paths[0],paths[2],base/'negative-main.obj',base/'negative-worker.obj',rejected_image,rejected_pdb,chr(64+n),pinned.as_posix()],'pure',65);assert out.strip()==str(code).encode() and rejected_image.read_bytes()==rejected_pdb.read_bytes()==sentinel;negatives.append({'scope':'multi-unit','case':n,'diagnostic':code,'first_call_resources':5})
 report.update(result='passed',production_admitted=True,single_unit_compatibility=single,independent_byte_inspection=inspections,positive_cases=positive,negative_cases=negatives,artifacts={binary.name:sha(binary),fast.name:sha(fast)})

except BaseException as error:report.update(result='failed',error=repr(error));raise
finally:
 report['changed_sources']=[n for n,h in inputs.items() if not Path(n).is_file() or sha(Path(n))!=h];report['changed_tools']=[n for n,v in report['tools'].items() if sha(Path(v['path']))!=v['sha256']];report['changed_configs']=[n for n,h in report['cargo_config_candidates'].items() if (sha(Path(n)) if Path(n).is_file() else None)!=h];report['checkout_changes']=[str(p) for p,h in originals.items() if not p.is_file() or sha(p)!=h];report.update(source_revision_end=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat());
 if report['changed_sources'] or report['changed_tools'] or report['changed_configs'] or report['checkout_changes']:report.update(result='failed',error='probe input changed')
 (base/'evidence.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');assert not report['changed_sources'] and not report['changed_tools'] and not report['changed_configs'] and not report['checkout_changes']
print(sha(base/'evidence.json'))
