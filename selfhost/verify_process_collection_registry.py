"""Qualify an explicit process-profile SG registry and source-bound Windows storage DLL."""
from __future__ import annotations
import argparse,ctypes,datetime as dt,json,os,platform,re,struct,subprocess,sys,time
from pathlib import Path
from verify_runtime_text import coff_image,dependencies,digest,exact_json,git_status,revision,sha,strict_json
from verify_runtime_deflate import metadata_proof
from runtime_evidence import bind_memory_providers,c_link
from linker_evidence import copy_linker

ROOT=Path(__file__).resolve().parents[1]
OWNED=['selfhost/runtime/collections_process_registry.sg','selfhost/runtime/collections_process_vec.sg','selfhost/runtime/collections_process_storage.c',
       'selfhost/tests/collections_process_probe.c','selfhost/tests/collections_process_smoke.sg','selfhost/tests/collections_process_fixture.c']
CORE=['selfhost/runtime/collections_registry.sg','selfhost/runtime/collections_vec.sg','selfhost/runtime/collections_memory.c','selfhost/tests/collections_runtime_fixture.c']

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--core-evidence',type=Path,required=True);parser.add_argument('--text-evidence',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    parser.add_argument('--os-rehost',action='store_true',help='Explicitly requalify updated System32 kernel32/ntdll providers by replaying all core/text commands')
    args=parser.parse_args();out=args.output.resolve()
    if not out.is_relative_to(ROOT/'target'):raise ValueError('output must be fresh target directory')
    out.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'stage':'sg-process-collection-registry','result':'running','revision_start':revision(),'git_status_start':git_status(),
            'host':platform.platform(),'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'sources':{},'external_sources':{},'tools':{},'artifacts':{},'steps':[],
            'namespace2_admitted':False,'global_registry_replaced':False,'self_build_complete':False,
            'remaining':['all other global/TLS runtime families and complete provider migration','provider/catalog and typed source-lowering integration','installed SDK packaging and self-build','whole-program memory/performance qualification']}
    cache={};checkout={}
    def save():(out/'evidence.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    def read(path,expected=None):
        path=Path(path).resolve()
        if path not in cache:cache[path]=path.read_bytes()
        raw=cache[path];fingerprint=digest(raw)
        if expected is not None and fingerprint!=expected:raise ValueError('consumed identity differs '+str(path))
        group='sources'if path.is_relative_to(ROOT)else'external_sources';report[group][str(path.relative_to(ROOT))if group=='sources'else str(path)]=fingerprint
        if path.is_relative_to(ROOT)and not path.is_relative_to(ROOT/'target'):checkout[path]=fingerprint
        return raw
    def historical_read(path,pin):
        path=Path(path).resolve();actual=sha(path)
        if actual!=pin:
            allowed=[(Path(os.environ['SystemRoot'])/'System32'/name).resolve()for name in ['kernel32.dll','ntdll.dll']]
            if not args.os_rehost or path not in allowed:raise ValueError('historical consumed identity differs '+str(path))
            row={'path':str(path),'historical_sha256':pin,'current_sha256':actual,'historical_bytes_retained_by_parent':False}
            changes=report.setdefault('os_rehost',{'historical_OS_closure_currently_reverified':False,'changed_providers':[],'required':'all 55 text and 45 context-core commands replayed in fresh directories before stage qualification'})['changed_providers']
            if row not in changes:changes.append(row)
        return read(path,actual)
    def tool(name,identity):
        path=Path(identity['path']).resolve();raw=historical_read(path,identity['sha256']);report['tools'][name]={'path':str(path),'sha256':digest(raw)};return path
    def snap(source,target):
        raw=read(source);target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(raw);read(target,digest(raw));return target
    clean={k:v for k,v in os.environ.items()if not k.upper().startswith(('SENGOO_','SGPLATFORM_','PYTHON','RUSTC','RUSTDOC','CARGO_','SDL2_'))}
    pure=dict(clean,PATH='')
    def run(name,command,env=pure,expected=0,empty=False):
        command=list(map(str,command));start=time.monotonic();done=subprocess.run(command,cwd=out,env=env,capture_output=True,timeout=600)
        for suffix,data in [('stdout',done.stdout),('stderr',done.stderr)]:
            path=out/(name+'.'+suffix);path.write_bytes(data);report['artifacts'][str(path)]=digest(data)
        report['steps'].append({'name':name,'command':command,'exit':done.returncode,'expected_exit':expected,'empty_path':env.get('PATH')=='','seconds':time.monotonic()-start,'stdout_sha256':digest(done.stdout),'stderr_sha256':digest(done.stderr)})
        save();print(name,done.returncode,flush=True)
        if done.returncode!=expected or empty and(done.stdout or done.stderr):raise ValueError(name+' exit/output differs '+done.stderr[-2000:].decode(errors='replace'))
        return done
    save()
    try:
        core=strict_json(read(args.core_evidence))
        if core.get('result')!='passed' or core.get('stage')!='sg-explicit-context-collections-runtime' or any(v!=[] for k,v in core.items() if k.startswith('changed_') or k=='checkout_changes'):raise ValueError('core qualification is not passed unchanged')
        for group in ['sources','external_sources','artifacts']:
            for name,pin in core[group].items():historical_read(ROOT/name if group=='sources' else Path(name),pin)
        for name,identity in core['tools'].items():tool('core-'+name,identity)
        for step in core['steps']:
            if step['exit']!=step['expected_exit']:raise ValueError('core command failed')
            for stream in ['stdout','stderr']:read(args.core_evidence.resolve().parent/(step['name']+'.'+stream),step[stream+'_sha256'])
        for name in CORE:
            if core['sources'].get(str(Path(name)))!=sha(ROOT/name):raise ValueError('qualified core input differs '+name)
        report['core_qualification']={'path':str(args.core_evidence.resolve()),'sha256':sha(args.core_evidence),'commands':len(core['steps']),'scope':'all source/artifact/compiler and command receipts reverified; OS provider changes require explicit full command replay; current reused core bytes identical'}
        parent=strict_json(read(args.text_evidence));
        if parent.get('schema')!=1 or parent.get('result')!='passed'or parent.get('stage')!='sg-runtime-text-utf8'or any(v!=[]for k,v in parent.items()if k.startswith('changed_')or k=='checkout_changes'):raise ValueError('text qualification is not passed/unchanged')
        for group in ['sources','external_sources']:
            for name,pin in parent[group].items():historical_read(ROOT/name if group=='sources'else Path(name),pin)
        for name,identity in parent['tools'].items():tool('producer-'+name,identity)
        native=tool('native',parent['tools']['native']);formatter=tool('formatter',parent['tools']['formatter']);clang=tool('clang',parent['tools']['clang']);lld=tool('lld',parent['tools']['clang-probe-linker']);kernel=tool('kernel32',parent['tools']['sdk-kernel32']);objcopy=tool('objcopy',parent['tools']['llvm-objcopy-c-sdk-metadata']);objdump=tool('objdump',parent['tools']['llvm-objdump-c-sdk-leaf-check']);readobj=tool('readobj',parent['tools']['llvm-readobj-independent-pe'])
        for name in ['native','formatter']:
            if parent['artifacts'].get(Path(parent['tools'][name]['path']).name)!=parent['tools'][name]['sha256']:raise ValueError('qualified compiler artifact relationship differs')
        pin=parent['linker_qualification']['evidence'];path=Path(pin['path']).resolve()
        if parent['sources'].get(str(path.relative_to(ROOT)))!=pin['sha256']:raise ValueError('declared linker parent not pinned')
        read(path,pin['sha256'])
        if args.os_rehost:
            # Resolve the already copied, source-bound historical branch without
            # asserting that its old OS DLL bytes still exist on this machine.
            qualified=core['linker_qualification'];chain=qualified['chain'];foundation=None
            for index,row in enumerate(chain):
                prior=strict_json(read(row['path'],row['sha256']))
                if prior.get('result')!='passed' or prior['stage']!=row['stage'] or len(prior['steps'])!=row['steps'] or any(v!=[] for k,v in prior.items()if k.startswith('changed_')):raise ValueError('historical linker report differs')
                if index+1<len(chain):
                    declared=prior['linker_qualification']['evidence'] if prior['stage']=='sg-native-module-graph' else prior['qualifications']['linker']
                    following=chain[index+1]
                    if Path(declared['path']).resolve()!=Path(following['path']).resolve() or declared['sha256']!=following['sha256'] or prior['sources'].get(str(Path(following['path']).relative_to(ROOT)))!=following['sha256']:raise ValueError('historical linker chain edge differs')
                else:foundation=prior
            if not foundation or foundation['stage']!='sg-coff-pe-linker-foundation' or set(foundation['sources'])!=set(qualified['sources']):raise ValueError('historical linker foundation sources differ')
            copied={}
            for name,row in qualified['sources'].items():
                if row['sha256']!=foundation['sources'][name] or core['sources'].get(row['snapshot'])!=row['sha256']:raise ValueError('historical linker snapshot relation differs')
                target=out/'linker-implementation'/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(read(ROOT/row['snapshot'],row['sha256']));read(target,row['sha256']);copied[name]={'snapshot':str(target.relative_to(ROOT)),'sha256':row['sha256']}
            for name,identity in foundation['tools'].items():tool('rehost-linker-producer-'+name,identity)
            binary=qualified['binary']
            if binary['sha256']!=foundation['artifacts']['sg-linker.exe'] or binary['sha256']!=parent['tools']['sg-linker']['sha256']:raise ValueError('historical linker binary relation differs')
            linker=out/'sg-linker.exe';linker.write_bytes(read(binary['path'],binary['sha256']));read(linker,binary['sha256']);report['tools']['sg-linker']={'path':str(linker),'sha256':binary['sha256']}
            report['linker_qualification']={'historical':qualified,'current_copied_sources':copied,'scope':'unchanged historical algorithm/compiler bytes; historical OS bytes not reverified; current host qualified only by fresh rehost and current consumer gates'}
        else:
            linker,branch=copy_linker(ROOT,path,out,report)
            if branch['binary_sha256']!=parent['tools']['sg-linker']['sha256']:raise ValueError('qualified linker binary differs')
        for name,identity in list(report['tools'].items()):tool(name,identity)
        for name,pin in list(report['sources'].items()):read(ROOT/name,pin)
        stack_origin=args.text_evidence.resolve().parent/'ntdll-chkstk.lib';stack_pin=parent['stack_probe_provider']['import_library_sha256']
        if parent['sources'].get(str(stack_origin.relative_to(ROOT)))!=stack_pin:raise ValueError('stack import library not source-pinned')
        stack=out/'ntdll-chkstk.lib';stack.write_bytes(read(stack_origin,stack_pin));read(stack,stack_pin)
        tool('runner-python',{'path':sys.executable,'sha256':sha(Path(sys.executable))});report['python']=sys.version
        report['os_memory_providers']=bind_memory_providers(read)
        helpers=['selfhost/verify_process_collection_registry.py','selfhost/verify_runtime_text.py','selfhost/verify_runtime_deflate.py','selfhost/runtime_evidence.py','selfhost/linker_evidence.py']
        for name in helpers:read(ROOT/name)
        impl=out/'implementation'
        for name in OWNED+CORE:snap(ROOT/name,impl/name)
        change=ROOT/'openspec/changes/add-selfhost-process-collection-registry'
        for path in sorted(change.rglob('*.md')):
            if path.name!='tasks.md':snap(path,impl/path.relative_to(ROOT))
        for name in ['runtime.c','runtime_string.c','runtime_collections.c','runtime_shared.h']:snap(ROOT/'tools/stdlib'/name,impl/'tools/stdlib'/name)
        baseline=['selfhost/sdk/collections_x64_abi1/manifest.json','selfhost/sdk/collections_x64_abi1/prelude.sg','tools/stdlib/collections.sg']
        report['baseline_untouched']={name:digest(read(ROOT/name))for name in baseline}
        cenv=dict(clean,PATH=str(clang.parent)+os.pathsep+clean.get('PATH',''),SOURCE_DATE_EPOCH='0')
        def compile_c(name,source,obj,flags=(),sdk=False):
            dep=obj.with_suffix('.d');raw=obj.with_suffix('.raw.obj')if sdk else obj
            command=[clang,'-std=c11','-Werror=date-time','-fno-addrsig','-O2'if sdk else'-O0','-I',impl/'tools/stdlib',*flags,'-MD','-MT','runtime_text_dependency','-MF',dep,'-c',source,'-o',raw]
            run(name,command,cenv)
            for path in dependencies(dep):read(path)
            if sdk:
                before=read(raw);image=coff_image(before)
                if any(section in image['sections']for section in ['.pdata','.xdata']):raise ValueError('SDK function is nonleaf; real unwind cannot be stripped')
                dis=run(name+'-disassembly',[objdump,'-d',raw],cenv)
                if re.search(rb'\bcall[q]?\b',dis.stdout):raise ValueError('C SDK leaf contains a call')
                run(name+'-normalize',[objcopy,'--strip-unneeded','--strip-symbol=.file','--remove-section=.bss','--remove-section=.data',raw,obj],cenv)
                after=bytearray(obj.read_bytes());after[4:8]=bytes(4);obj.write_bytes(after)
                proof=metadata_proof(before,bytes(after));report.setdefault('c_sdk_metadata',[]).append({'name':name,**proof})
            read(obj);return obj
        if args.os_rehost:
            replay_reports=[]
            for label,origin,data,count in [('text',args.text_evidence.resolve().parent,parent,55),('core',args.core_evidence.resolve().parent,core,45)]:
                if len(data['steps'])!=count:raise ValueError('rehost exact historical command inventory differs')
                directory=out/('rehost-'+label);directory.mkdir()
                def relocate(value):return str(value).replace(str(origin),str(directory))
                def outputs(command):
                    if '-###'in command:return []
                    result=[]
                    for flag in ['-o','-MF','-l','--output']:
                        if flag in command:result.append(Path(command[command.index(flag)+1]))
                    if Path(command[0]).name=='native.exe' and len(command)>2:result.append(Path(command[2]))
                    if len(command)>2 and command[1]=='link':result.append(Path(command[2]))
                    if '--strip-unneeded'in command:result.append(Path(command[-1]))
                    return result
                produced={path.resolve() for step in data['steps'] for path in outputs(step['command'])}
                comparable={path.resolve() for step in data['steps'] if Path(step['command'][0]).name=='native.exe' or '--strip-unneeded'in step['command'] or len(step['command'])>1 and step['command'][1]=='link' for path in outputs(step['command'])}
                originals={}
                for group in ['sources','artifacts']:
                    for key,pin in data[group].items():
                        source=ROOT/key if group=='sources' else Path(key)if Path(key).is_absolute()else origin/key
                        if not source.resolve().is_relative_to(origin):continue
                        raw=read(source,pin);originals[str(source.resolve())]=pin
                        if source.resolve() in produced:continue
                        target=Path(relocate(source));target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(raw)
                for identity in data['tools'].values():
                    source=Path(identity['path']).resolve()
                    if source.is_relative_to(origin):
                        target=Path(relocate(source));target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(read(source,identity['sha256']))
                start=len(report['steps']);generated=[]
                for step in data['steps']:
                    expected=step.get('expected_exit',step.get('expected_exit_code'))
                    actual_old=step.get('exit',step.get('exit_code'))
                    if expected is None or actual_old!=expected:raise ValueError('historical command expected result unavailable')
                    command=[relocate(arg)for arg in step['command']];targets=[Path(relocate(p))for p in outputs(step['command'])]
                    for target in targets:
                        if not target.resolve().is_relative_to(directory):raise ValueError('rehost output escapes fresh directory')
                        target.parent.mkdir(parents=True,exist_ok=True)
                        if target.exists():target.unlink()
                    result=run('rehost-'+label+'-'+step['name'],command,pure if step.get('empty_path')else cenv,expected=expected)
                    if '--strip-unneeded'in command:
                        target=Path(command[-1]);raw=bytearray(target.read_bytes());raw[4:8]=bytes(4);target.write_bytes(raw)
                    if 'format-preview'in step['name']:
                        if label=='text':formatted=directory/'format-preview'/(step['name'].removeprefix('format-preview-')+'.sg')
                        else:formatted=directory/(step['name'].removesuffix('-format-preview')+'-formatted.sg')
                        formatted.parent.mkdir(parents=True,exist_ok=True);formatted.write_bytes(result.stdout)
                    for target in targets:
                        if not target.is_file():raise ValueError('rehost command did not produce its fresh output '+str(target))
                        generated.append({'command':step['name'],'path':str(target),'sha256':sha(target)})
                    if 'execute'in step['name'] and result.stdout:
                        observed=strict_json(result.stdout);old_stream=origin/(step['name']+'.stdout')
                        old=strict_json(read(old_stream,step['stdout_sha256']))
                        if isinstance(observed,dict) and 'working_set_peak_bytes'in observed:
                            if type(observed['working_set_peak_bytes'])is not int or observed['working_set_peak_bytes']<=0:raise ValueError('new working-set observation unavailable')
                            observed=dict(observed);old=dict(old);observed.pop('working_set_peak_bytes');old.pop('working_set_peak_bytes')
                        if observed!=old:raise ValueError('rehost executed resource receipt differs '+step['name'])
                equal=[]
                for source,pin in originals.items():
                    original=Path(source)
                    if original in comparable:
                        target=Path(relocate(original));actual=sha(target)
                        if actual!=pin:raise ValueError('fresh rehost executable/normalized object differs '+str(target))
                        equal.append({'path':str(target),'sha256':actual})
                replay_reports.append({'parent':str(origin/'evidence.json'),'parent_sha256':sha(origin/'evidence.json'),'commands':len(report['steps'])-start,'generated_outputs':generated,'equal_products':equal,'scope':'all historical commands replayed with only output-root relocation; outputs absent before producer execution; generated normalized C COFF timestamps zeroed as in original verifier; fresh exact resource receipts, variable observed RSS retained'})
            report.setdefault('os_rehost',{})['replayed']=replay_reports
        # These unsafe C ABI sources use the qualified SG formatter preview.
        for name in ['collections_process_registry','collections_process_vec','collections_process_smoke']:
            path=impl/('selfhost/tests/'+name+'.sg' if name.endswith('smoke') else 'selfhost/runtime/'+name+'.sg')
            preview=run(name+'-format-preview',[formatter,path]);formatted=out/(name+'-formatted.sg');formatted.write_bytes(preview.stdout)
            if preview.stdout!=read(path):raise ValueError('owned SG source is not formatted '+name)
            run(name+'-format-idempotent',[formatter,formatted,'--check'])
        provider=[]
        for name in ['runtime','runtime_string','runtime_collections']:
            provider.append(compile_c(name+'-reference-compile',impl/'tools/stdlib'/(name+'.c'),out/(name+'.obj')))
        instrumented=compile_c('instrumented-memory',impl/'selfhost/runtime/collections_memory.c',out/'instrumented-memory.obj',['-DSENGOO_COLLECTIONS_TEST_ALLOCATOR'])
        probe=compile_c('resource-thread-reentry-probe',impl/'selfhost/tests/collections_process_probe.c',out/'probe.obj')
        probe_storage=compile_c('probe-storage',impl/'selfhost/runtime/collections_process_storage.c',out/'probe-storage.obj')
        fresh=[]; built_rounds=[]
        for number,directory_name in enumerate(['first fresh 世界 &','second fresh 世界 &']):
            directory=out/directory_name;directory.mkdir();objects={};built={}
            for stem,relative in [('registry','selfhost/runtime/collections_registry.sg'),('vec','selfhost/runtime/collections_vec.sg'),('process_registry','selfhost/runtime/collections_process_registry.sg'),('process_vec','selfhost/runtime/collections_process_vec.sg'),('smoke','selfhost/tests/collections_process_smoke.sg')]:
                source=snap(impl/relative,directory/(stem+'.sg'));obj=directory/(stem+'.obj')
                emitted=run(str(number)+'-emit-'+stem,[native,source,obj,'--wrap']);exact_json(emitted.stdout,{'schema_version':1,'ok':True,'phase':'object'})
                if emitted.stderr:raise ValueError('native emission stderr')
                objects[stem]=obj;built[stem]=obj
            if number==0:
                exe=out/'process-resources.exe';c_link(run,read,report,'resource-thread-reentry-link',clang,[*[objects[n] for n in ['registry','vec','process_registry','process_vec']],instrumented,probe_storage,probe,*provider],exe,cenv)
                actual=run('resource-thread-reentry-execute',[exe],expected=42);stats=strict_json(actual.stdout)
                required={'rounds':100,'owning_drops':1201,'reentrant_callbacks':1201,'metadata_threads':4,'metadata_iterations':8000,'state_bytes':16512,'payload_live_bytes':0,'created':1833,'freed':1833,'payload_peak_bytes':3104,'virtual_free_verified':True,'mixed_profiles_rejected':True,'payload_concurrency_qualified':False,'completed':list(range(10))}
                if stats!=required or any(type(stats[k])is not type(v) for k,v in required.items()) or actual.stderr:raise ValueError('actual resource/thread/reentry/completion receipt differs')
                report['actual_os_resources']=stats
            for stem,relative in [('memory','selfhost/runtime/collections_memory.c'),('fixture','selfhost/tests/collections_process_fixture.c')]:
                # Preserve include-relative fixture closure in each fresh directory.
                if stem=='fixture':snap(impl/'selfhost/tests/collections_runtime_fixture.c',directory/'collections_runtime_fixture.c')
                source=snap(impl/relative,directory/(stem+'.c'));objects[stem]=compile_c(str(number)+'-compile-'+stem,source,directory/(stem+'.obj'),sdk=True);built[stem]=objects[stem]
            storage_source=snap(impl/'selfhost/runtime/collections_process_storage.c',directory/'storage.c')
            raw=compile_c(str(number)+'-compile-storage',storage_source,directory/'storage.raw.obj')
            before=read(raw);normalized=bytearray(before);normalized[4:8]=bytes(4);storage=directory/'storage.obj';storage.write_bytes(normalized);read(storage)
            if before[:4]!=normalized[:4] or before[8:]!=normalized[8:]:raise ValueError('state SDK timestamp normalization differs')
            report.setdefault('storage_timestamp_normalization',[]).append({'raw':str(raw),'raw_sha256':digest(before),'normalized':str(storage),'normalized_sha256':digest(normalized),'only_bytes_4_to_7_zeroed':True})
            built['storage']=storage
            dll=directory/'sengoo-process-state.dll';library=directory/'sengoo-process-state.lib'
            run(str(number)+'-storage-dll',[lld,'/dll','/noentry','/nodefaultlib','/machine:x64','/Brepro','/out:'+str(dll),'/implib:'+str(library),storage,kernel],cenv)
            read(dll);read(library);built['storage-dll']=dll;built['storage-import']=library
            if number==0:
                dll_probe=directory/'process-resources.exe'
                c_link(run,read,report,'actual-storage-dll-resource-link',clang,[*[objects[n]for n in ['registry','vec','process_registry','process_vec']],instrumented,probe,library,*provider],dll_probe,cenv)
                dll_observed=run('actual-storage-dll-resource-execute',[dll_probe],expected=42)
                if strict_json(dll_observed.stdout)!=report['actual_os_resources'] or dll_observed.stderr:raise ValueError('actual storage DLL resource/thread/reentry/retirement result differs')
                report['actual_storage_dll_resources']=strict_json(dll_observed.stdout)
            state_info=run(str(number)+'-storage-inspection',[readobj,'--sections','--unwind','--coff-imports','--coff-exports',dll],cenv)
            if any(name not in state_info.stdout for name in [b'RuntimeFunction',b'AcquireSRWLockExclusive',b'ReleaseSRWLockExclusive',b'sengoo_process_registry_address',b'sengoo_process_vec_context_address']):raise ValueError('real storage DLL imports/exports/unwind unavailable')
            raw_info=run(str(number)+'-storage-object-inspection',[readobj,'--sections','--symbols','--unwind',raw],cenv)
            if any(name not in raw_info.stdout for name in [b'.bss',b'.pdata',b'.xdata']):raise ValueError('real storage object state/unwind was removed')
            image=directory/'collections-process.exe'
            run(str(number)+'-sg-link',[linker,'link',image,'main',*objects.values(),library,kernel,stack])
            run(str(number)+'-sg-execute',[image],expected=42,empty=True)
            inspect=run(str(number)+'-independent-inspection',[readobj,'--file-headers','--coff-imports','--unwind',image],cenv)
            if any(name not in inspect.stdout for name in [b'sengoo-process-state.dll',b'VirtualAlloc',b'VirtualFree',b'__chkstk']):raise ValueError('actual state/memory/stack imports absent')
            built['image']=image;values={name:sha(path)for name,path in built.items()}
            if fresh and values!=fresh[0]['sha256']:raise ValueError('two fresh SG/C/DLL/import/PE bytes differ')
            fresh.append({'directory':directory_name,'sha256':values,'execution_exit':42});built_rounds.append(built)
        report['fresh_rounds']=fresh
        kernel32=ctypes.WinDLL('kernel32',use_last_error=True)
        filename=kernel32.GetModuleFileNameW;filename.argtypes=[ctypes.c_void_p,ctypes.c_wchar_p,ctypes.c_uint32];filename.restype=ctypes.c_uint32
        getmodule=kernel32.GetModuleHandleExW;getmodule.argtypes=[ctypes.c_uint32,ctypes.c_void_p,ctypes.POINTER(ctypes.c_void_p)];getmodule.restype=ctypes.c_int
        class RuntimeFunction(ctypes.Structure):_fields_=[('begin',ctypes.c_uint32),('end',ctypes.c_uint32),('unwind',ctypes.c_uint32)]
        lookup=ctypes.WinDLL('ntdll').RtlLookupFunctionEntry;lookup.argtypes=[ctypes.c_uint64,ctypes.POINTER(ctypes.c_uint64),ctypes.c_void_p];lookup.restype=ctypes.POINTER(RuntimeFunction)
        loaded=ctypes.WinDLL(str(built_rounds[0]['storage-dll']));actual_name=ctypes.create_unicode_buffer(32768)
        if not filename(loaded._handle,actual_name,len(actual_name)) or Path(actual_name.value).resolve()!=built_rounds[0]['storage-dll'].resolve():raise ValueError('actual loaded storage DLL path differs')
        report['actual_loaded_storage']={'path':actual_name.value,'sha256':digest(read(actual_name.value)),'state_sizes':[4160,12352],'zero_before_SG_initialization':True,'unwind':[]}
        for symbol,size in [('sengoo_process_registry_address',4160),('sengoo_process_vec_context_address',12352)]:
            function=getattr(loaded,symbol);function.restype=ctypes.c_void_p;address=function()
            if not address or ctypes.string_at(address,size)!=bytes(size):raise ValueError('actual OS zero storage differs')
        for symbol in ['sengoo_process_registry_lock','sengoo_process_registry_unlock']:
            function=getattr(loaded,symbol);function.restype=None;function.argtypes=[]
            pointer=ctypes.cast(function,ctypes.c_void_p).value;base=ctypes.c_uint64();entry=lookup(pointer,ctypes.byref(base),None)
            if not entry or base.value!=loaded._handle or not base.value+entry.contents.begin<=pointer<base.value+entry.contents.end:raise ValueError('actual nonleaf SDK has no registered unwind '+symbol)
            report['actual_loaded_storage']['unwind'].append({'symbol':symbol,'rva':pointer-base.value,'begin':entry.contents.begin,'end':entry.contents.end,'unwind_rva':entry.contents.unwind});function()
        providers=[]
        for symbol in ['AcquireSRWLockExclusive','ReleaseSRWLockExclusive','CreateThread','WaitForMultipleObjects','VirtualQuery']:
            pointer=ctypes.cast(getattr(kernel32,symbol),ctypes.c_void_p).value;handle=ctypes.c_void_p();buffer=ctypes.create_unicode_buffer(32768)
            if not getmodule(6,pointer,ctypes.byref(handle))or not filename(handle,buffer,len(buffer)):raise ValueError('actual OS provider unavailable '+symbol)
            path=Path(buffer.value).resolve();providers.append({'symbol':symbol,'path':str(path),'sha256':digest(read(path)),'rva':pointer-handle.value})
        report['actual_os_synchronization_providers']=providers
        report['deployment']={'sg_objects':['registry','vec','process_registry','process_vec','smoke'],'C_storage':'two zero static roots and SRWLOCK/address leaves only','state_DLL':'unstripped real BSS and unwind; timestamp-only input normalization','remaining_memory_fixture_C':'qualified leaf metadata normalization preserving code/data/relocations/symbol addresses','payload_concurrency_qualified':False,'final_SDK_qualified':False}
        report.update(result='passed',two_fresh_objects_and_images_equal=True)
    except BaseException as error:
        report['error']=str(error);report['result']='failed';raise
    finally:
        report['changed_sources']=[name for name,pin in report['sources'].items()if sha(ROOT/name)!=pin]
        report['changed_external_sources']=[name for name,pin in report['external_sources'].items()if sha(name)!=pin]
        report['changed_tools']=[name for name,row in report['tools'].items()if sha(row['path'])!=row['sha256']]
        report['checkout_changes']=[str(path.relative_to(ROOT))for path,pin in checkout.items()if sha(path)!=pin]
        if any(report[key]for key in ['changed_sources','changed_external_sources','changed_tools','checkout_changes']):report['result']='failed'
        report['revision_end']=revision();report['git_status_end']=git_status();report['completed_at']=dt.datetime.now(dt.timezone.utc).isoformat()
        for path in sorted(out.rglob('*')):
            if path.is_file()and path!=out/'evidence.json':report['artifacts'][str(path)]=sha(path)
        save();print(json.dumps({'result':report['result'],'commands':len(report['steps']),'evidence_sha256':sha(out/'evidence.json')}),flush=True)
    if report['result']!='passed':raise SystemExit(1)

if __name__=='__main__':main()
