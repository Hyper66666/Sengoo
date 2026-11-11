"""Qualify independent SG COFF units with scoped symbols and merged PDB modules."""
from pathlib import Path
import sys,os,json,hashlib,re,subprocess,datetime,platform,shutil,tomllib
import argparse
from link_debug_gate import ROOT, msf_streams, u16, u32
from link_units_oracle import cstring
import struct
from mixeddebug_sdk import qualify
from mixeddebug_oracle import inspect_mixed, controls
from link_multi_debug_gate import closure, rust_override, coff_fragments
parser=argparse.ArgumentParser(description="Qualify mixed SG debug units and source-locked nondebug C leaf")
for name in ["seed","clang","formatter","lldb-python-home","output"]:parser.add_argument("--"+name,required=True,type=Path)
args=parser.parse_args();root=ROOT;base=args.output.resolve();assert base.is_relative_to(root/'target');base.mkdir(parents=True,exist_ok=False)
sha=lambda p:hashlib.sha256(p.read_bytes()).hexdigest()
entry=root/'selfhost/tests/mixeddebug.sg';originals=closure(entry);originals.update({p:sha(p) for p in (root/'tools/stdlib').rglob('*') if p.is_file() and not any(x in {'target','build','__pycache__'} for x in p.relative_to(root/'tools/stdlib').parts)})
inputs={str(Path(__file__).resolve()):sha(Path(__file__)),**{str(root/name):sha(root/name)for name in ['selfhost/mixeddebug_sdk.py','selfhost/mixeddebug_oracle.py','selfhost/link_units_oracle.py','selfhost/verify_runtime_text.py','selfhost/linker_evidence.py']},str(root/'selfhost/link_multi_debug_gate.py'):sha(root/'selfhost/link_multi_debug_gate.py'),str(root/'selfhost/link_debug_gate.py'):sha(root/'selfhost/link_debug_gate.py'),str(root/'selfhost/bootstrap.lock'):sha(root/'selfhost/bootstrap.lock')}
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
tools={'seed':args.seed.resolve(),'formatter':args.formatter.resolve(),'clang':args.clang.resolve(),'readobj':llvm/'llvm-readobj.exe','pdbutil':llvm/'llvm-pdbutil.exe','lldb':llvm/'lldb.exe','liblldb':llvm/'liblldb.dll','lld':llvm/'lld-link.exe','python':Path(sys.executable)}
for p in [*py.glob('*.dll'),*py.glob('*.pyd'),py/'python310.zip']:tools['lldb-'+p.name]=p
build={k:v for k,v in os.environ.items() if not k.upper().startswith(('SENGOO_','SGPLATFORM_','SDL2_','PYTHON','LLDB_')) and not rust_override(k)};build['PATH']=str(llvm)+os.pathsep+build.get('PATH','')
for name in ['cargo','rustc','rustup']:tools[name]=Path(shutil.which(name,path=build['PATH'])).resolve()
for name in ['cargo','rustc']:tools[name+'-selected']=Path(subprocess.check_output([tools['rustup'],'which',name],env=build,cwd=root,text=True).strip()).resolve()
assert tools['cargo-selected'].parent==tools['rustc-selected'].parent
build.update(CARGO=str(tools['cargo-selected']),RUSTC=str(tools['rustc-selected']),RUSTC_WRAPPER='',RUSTC_WORKSPACE_WRAPPER='',CARGO_BUILD_RUSTC=str(tools['rustc-selected']),CARGO_BUILD_RUSTC_WRAPPER='',CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='',RUSTUP_TOOLCHAIN=tools['rustc-selected'].parent.parent.name);build['PATH']=str(tools['cargo-selected'].parent)+os.pathsep+build['PATH']
build.update(SENGOO_STDLIB=str(snapshot/'tools/stdlib'),SENGOO_RUNTIME=str(snapshot/'tools/stdlib/runtime.c'),SENGOO_CLANG=str(tools['clang']),SENGOO_MODULE_MAP=';'.join('selfhost_'+p.stem+'='+str(snapshot/p.relative_to(root)) for p in originals if p.parent==root/'selfhost/src'))
pure={k:v for k,v in build.items() if not k.upper().startswith('SENGOO_')};pure['PATH']=''
debug=dict(pure,PYTHONHOME=str(py),PYTHONPATH=str(py/'python310.zip')+os.pathsep+str(py),LLDB_USE_NATIVE_PDB_READER='1');debug['PATH']=str(py)+os.pathsep+str(llvm)
report={'schema':1,'stage':'mixed-sg-debug-and-source-locked-nondebug-coff','result':'running','production_admitted':False,'host':platform.platform(),'source_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sources':inputs,'producer_origins':{str(p):h for p,h in originals.items()},'qualified_checkout_sources':{str(p):h for p,h in originals.items()},'tools':{k:{'path':str(v),'sha256':sha(v)} for k,v in tools.items()},'steps':[],'limits':['at least one debugged SG unit plus explicitly declared nondebug objects with no .debug-prefixed sections; general MSVC auxiliary/COMDAT/debug/archive input support remains open','two independent semantic/MIR/native/object compilations; native instruction/address facts are unmodified','same-named helper visibility is explicitly classified as COFF storage3 by this test caller before emission; automatic compiler internalization is not qualified','two fresh artifact directories share identical actual source inputs, logical module names and RSDS path; different source checkout directories/path remapping and M6/M8 reproduction remain open','product object/link/debug read/write commands use empty PATH; SG seed development C/Rust runtime remains recorded']}
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

 debugger_source=root/'target/selfhost/m5-linker-dev/llvmorg-19.1.7-SymbolFileNativePDB.cpp';debugger_raw=debugger_source.read_bytes();assert hashlib.sha256(debugger_raw).hexdigest()=='7434e4c38399d0564aa7d17c583fe89c2d05ff3457b98395d066dacc4a99e71e';assert b'void SymbolFileNativePDB::AddSymbols(Symtab &symtab) {}'in debugger_raw and b'if (match.second.kind() != S_PROCREF && match.second.kind() != S_LPROCREF)'in debugger_raw;inputs[str(debugger_source)]=hashlib.sha256(debugger_raw).hexdigest();report['debugger_primary_source']={'url':'https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-19.1.7/lldb/source/Plugins/SymbolFile/NativePDB/SymbolFileNativePDB.cpp','sha256':hashlib.sha256(debugger_raw).hexdigest(),'tag':'llvmorg-19.1.7','actual_lldb_version':run('lldb-version',[tools['lldb'],'--version'],'debug').decode('utf-8')}
 sdk,qualification=qualify(root,base,inputs,report['tools'],run);report['sdk_qualification']=qualification
 for path in [p for p in originals if p.suffix=='.sg'and (p.name=='mixeddebug.sg'or p.stem.startswith('link_'))]:run('format-'+path.stem,[tools['formatter'],'--check',snapshot/path.relative_to(root)])
 binary=base/'mixeddebug.exe';run('build-mixed-native-debug',[tools['seed'],'--runtime-mode','source-development','build',snapshot/entry.relative_to(root),'--force-rebuild','-O','0','-o',binary])
 for mode in ['--fail-equality','--fail-truth']:assert b'mixeddebug.sg'in run('assertion'+mode,[binary,mode],'pure',1)
 source=base/'sources 世界 &';source.mkdir();fixtures={'main.sg':'import common;\nextern "C" { fn worker(value:i64)->i64; }\nstruct Local { input:i64, extra:i64 }\ndef main()->i64 {\n let state:Local=Local{input:38,extra:2};\n let input:i64=state.input+helper();\n let answer:i64=worker(input);\n return answer;\n}\n','common.sg':'pub def helper()->i64 {\n return 2;\n}\n','worker.sg':'import common;\nextern "C" {fn sengoo_deflate_fixture_length(kind:i64)->i64; fn sengoo_deflate_fixture_check(kind:i64,count:i64)->i64;}\nstruct Local { count:i64, step:i64 }\npub def worker(value:i64)->i64 {\n let state:Local=Local{count:value+helper()-2,step:sengoo_deflate_fixture_length(0)-6+sengoo_deflate_fixture_check(0,3)};\n let answer:i64=state.count+state.step;\n return answer;\n}\n'}
 for name,text in fixtures.items():path=source/name;path.write_text(text,encoding='utf-8',newline='\n');inputs[str(path)]=sha(path)
 paths=[source/name for name in ['main.sg','common.sg','worker.sg']];fixed=base/'fixed.pdb';fresh=[];artifacts={}
 for label in ['fresh-a','fresh-b']:
  directory=base/label;directory.mkdir();obj=[directory/'main.obj',directory/'worker.obj'];image=directory/'program.exe';pdb=directory/'program.pdb';command=[binary,paths[0],paths[2],sdk,*obj,image,pdb,'@',fixed.as_posix()];assert run(label+'-sg-source-object-link-pdb',command,'pure').strip()==b'mixed SG debug and source-locked C leaf: ok';run(label+'-execute',[image],'pure',42);fresh.append([path.read_bytes()for path in [*obj,image,pdb]]);artifacts.update({str(path.relative_to(base)):sha(path)for path in [*obj,image,pdb]})
 assert fresh[0]==fresh[1];fixed.write_bytes(fresh[0][3]);artifacts[str(fixed.relative_to(base))]=sha(fixed);directory=base/'fresh-a';obj=[directory/'main.obj',directory/'worker.obj'];image=directory/'program.exe';pdb=directory/'program.pdb'
 inspection=inspect_mixed(fresh[0][3],sdk.read_bytes(),paths);inspection['negative_controls']=controls(fresh[0][3],sdk.read_bytes(),paths)
 run('inspect-c-sdk-sections-symbols',[tools['readobj'],'--sections','--symbols','--relocations',sdk]);run('inspect-mixed-pdb',[tools['pdbutil'],'dump','--modules','--files','-l','--symbols','--globals','--publics',pdb]);run('inspect-imports',[tools['readobj'],'--coff-imports',image])
 frame='settings set frame-format "SG_PHYSICAL ${line.file.fullpath}:${line.number} function=${function.name}\\n"';commands=[frame,'breakpoint set --file "'+paths[0].as_posix()+'" --line 7','run','frame variable input state','thread step-in','thread step-over','frame variable value state','thread step-over','frame variable answer','thread step-out','thread step-over','frame variable answer','continue'];live=run('live-mixed-cross-debug-units',[tools['lldb'],'--batch','--no-lldbinit',*[x for c in commands for x in ['-o',c]],image],'debug')
 for value in [b'input = 40',b'input = 38, extra = 2',b'value = 40',b'count = 40, step = 2',b'answer = 42',b'exited with status = 42']:assert value in live,value
 for path,line in [(paths[0],7),(paths[2],5),(paths[2],6),(paths[2],7),(paths[0],8)]:assert ('SG_PHYSICAL '+str(path)+':'+str(line)).encode()in live
 streams=msf_streams(pdb.read_bytes());records=streams[10];at=0
 while at<len(records):
  length=u16(records,at)+2
  if u16(records,at+2)==0x110E and cstring(records,at+14,at+length)[0]==b'sengoo_deflate_fixture_length':break
  at+=length
 segment=u16(records,at+12);offset=u32(records,at+8);raw_image=image.read_bytes();file_address=struct.unpack_from('<Q',raw_image,112)[0]+u32(raw_image,328+(segment-1)*40+12)+offset
 named=run('sdk-name-pending-in-pinned-lldb',[tools['lldb'],'--batch','--no-lldbinit','-o','breakpoint set --name sengoo_deflate_fixture_length','-o','run',image],'debug');assert b'Breakpoint 1: no locations (pending).'in named and b'exited with status = 42'in named
 ccommands=['breakpoint set --shlib program.exe --address '+hex(file_address),'run','register read rip rcx','image list -o -f','frame info','process kill'];clive=run('live-c-leaf-address-no-invented-source',[tools['lldb'],'--batch','--no-lldbinit',*[x for c in ccommands for x in ['-o',c]],image],'debug');assert b'stop reason = breakpoint 1.1'in clive and b'rcx = 0x0000000000000000'in clive
 registers=re.search(rb'\brip = (0x[0-9A-Fa-f]+)',clive);slide=re.search(rb'\[\s*0\]\s+(0x[0-9A-Fa-f]+)',clive);framepc=re.search(rb'frame #0: (0x[0-9A-Fa-f]+) program.exe\r?\r?\n',clive);assert registers and slide and framepc;assert int(registers[1],16)==int(slide[1],16)+file_address==int(framepc[1],16);inspection['actual_sdk_breakpoint']={'public_file_address':hex(file_address),'module_slide':hex(int(slide[1],16)),'rip':hex(int(registers[1],16)),'argument_rcx':0,'no_source_frame':True,'explicit_process_kill':True};assert b'.sg:'not in clive and b'(lldb) process kill'in clive
 independent=base/'independent';independent.mkdir();main_c=independent/'main.c';main_c.write_text('extern long long sengoo_deflate_fixture_length(long long);\nint main(void){return (int)sengoo_deflate_fixture_length(0)+34;}\n',encoding='utf-8');inputs[str(main_c)]=sha(main_c);run('oracle-clang-debug-main',[tools['clang'],'-g','-gcodeview','-O0','-Werror','-c',main_c,'-o',independent/'main.obj']);run('oracle-lld-mixed-pdb',[tools['lld'],'/entry:main','/subsystem:console','/nodefaultlib','/debug','/out:'+str(independent/'program.exe'),'/pdb:'+str(independent/'program.pdb'),independent/'main.obj',base/'sdk/historical.raw.obj']);run('oracle-lld-execute',[independent/'program.exe'],'pure',42);independent_name=run('oracle-lld-public-name-pending',[tools['lldb'],'--batch','--no-lldbinit','-o','breakpoint set --name sengoo_deflate_fixture_length','-o','run',independent/'program.exe'],'debug');assert b'Breakpoint 1: no locations (pending).'in independent_name and b'exited with status = 42'in independent_name
 for path in independent.glob('*'):artifacts[str(path.relative_to(base))]=sha(path)
 report['sdk_public_debugger_scope']={'qualified':'actual public RIP = module slide + independent public file address, rcx=0, no source frame; process explicitly killed after the stop; separate full product and SG source debugger runs exit42','open':'LLDB19 nativePDB public-name lookup absent (AddSymbols is empty; FindFunctions consumes only PROCREF/LPROCREF), C typed/source debugging is not fabricated','primary_reference':'https://raw.githubusercontent.com/llvm/llvm-project/llvmorg-19.1.7/lldb/source/Plugins/SymbolFile/NativePDB/SymbolFileNativePDB.cpp','public_file_address':hex(file_address),'address_stop_kills_process':True}

 commands=[frame,'breakpoint set --file "'+paths[1].as_posix()+'" --line 2','run','bt','continue','bt','continue'];static=run('live-both-static-helpers',[tools['lldb'],'--batch','--no-lldbinit',*[x for c in commands for x in ['-o',c]],image],'debug');assert b'2 locations'in static and b'function=main'in static and b'function=worker'in static and b'exited with status = 42'in static
 negatives=[];sentinel=b'previous mixed artifact must survive'
 def reject(label,sdk_object,mode,expected):
  outimage=base/'rejected.exe';outpdb=base/'rejected.pdb';outimage.write_bytes(sentinel);outpdb.write_bytes(sentinel);out=run('reject-'+label,[binary,paths[0],paths[2],sdk_object,base/'reject-main.obj',base/'reject-worker.obj',outimage,outpdb,mode,fixed.as_posix()],'pure',65);assert out.strip()==str(expected).encode()and outimage.read_bytes()==outpdb.read_bytes()==sentinel,(label,out);negatives.append({'name':label,'code':expected,'first_call_resource_scopes':5})
 for n in range(1,22):reject('state-'+str(n),sdk,chr(64+n),7333 if n in [2,19]else 7303 if n==3 else 7331)
 # Standard section/file auxiliaries now pass COFF admission. The unchanged raw
 # clang object still carries the absolute @feat.00 symbol, which the existing
 # no-debug PDB public catalog rejects at its independent 7333 boundary.
 reject('raw-clang-absolute-public',base/'sdk/historical.raw.obj','@',7333)
 raw=sdk.read_bytes();count=int.from_bytes(raw[2:4],'little');section=next(20+n*40 for n in range(count)if raw[20+n*40:28+n*40].rstrip(b'\0')==b'.rdata')
 for name,encoded in [('unknown-debug',b'.debugX'),('single-debug-S',b'.debug$S'),('single-debug-T',b'.debug$T')]:
  changed=bytearray(raw);changed[section:section+8]=encoded.ljust(8,b'\0');changed[section+36:section+40]=int.from_bytes(fresh[0][0][20+3*40+36:20+3*40+40],'little').to_bytes(4,'little') if name!='unknown-debug'else raw[section+36:section+40];path=base/(name+'.obj');path.write_bytes(changed);inputs[str(path)]=sha(path);reject(name,path,'@',7333)
 report.update(result='passed',production_admitted=True,sdk_qualification=qualification,independent_byte_inspection=inspection,negative_cases=negatives,logical_module_names=['sg-main.obj','sg-worker.obj','sdk-leaf.obj'],fixed_rsds_path=fixed.as_posix(),fresh_artifact_directories=[str(base/'fresh-a'),str(base/'fresh-b')],identical_inputs_options_obj_pe_pdb_bytes=True,artifacts={binary.name:sha(binary),**artifacts})

except BaseException as error:report.update(result='failed',error=repr(error));raise
finally:
 report['changed_sources']=[n for n,h in inputs.items() if not Path(n).is_file() or sha(Path(n))!=h];report['changed_tools']=[n for n,v in report['tools'].items() if sha(Path(v['path']))!=v['sha256']];report['changed_configs']=[n for n,h in report['cargo_config_candidates'].items() if (sha(Path(n)) if Path(n).is_file() else None)!=h];report['checkout_changes']=[str(p) for p,h in originals.items() if not p.is_file() or sha(p)!=h];report.update(source_revision_end=subprocess.check_output(['git','rev-parse','HEAD'],cwd=root,text=True).strip(),completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat());
 if report['changed_sources'] or report['changed_tools'] or report['changed_configs'] or report['checkout_changes']:report.update(result='failed',error='probe input changed')
 (base/'evidence.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');assert not report['changed_sources'] and not report['changed_tools'] and not report['changed_configs'] and not report['checkout_changes']
print(sha(base/'evidence.json'))
