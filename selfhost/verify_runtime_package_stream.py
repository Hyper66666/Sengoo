"""Source-bound development qualification of SG incremental tar and extraction.

This is byte extraction, not sgpm manifest/registry admission or M8 qualification.
"""
from __future__ import annotations
import argparse,ctypes,datetime as dt,hashlib,json,os,platform,re,struct,subprocess,sys,time
from pathlib import Path
from verify_runtime_text import strict_json,digest,sha,revision,git_status,dependencies
from linker_evidence import copy_linker
from runtime_evidence import c_link
ROOT=Path(__file__).resolve().parents[1]

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['text-evidence','gzip-stream-evidence','tar-evidence','rust-tar-evidence','output']:
        parser.add_argument('--'+name,type=Path,required=True)
    args=parser.parse_args();out=args.output.resolve()
    if not out.is_relative_to(ROOT/'target'):raise ValueError('output must be fresh target directory')
    out.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'stage':'sg-runtime-package-stream-extraction','result':'running','host':platform.platform(),'revision_start':revision(),'git_status_start':git_status(),'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'sources':{},'external_sources':{},'tools':{},'artifacts':{},'steps':[],'package_resolver_integration_complete':False,'performance_qualified':False,'remaining':['full Sengoo.toml/package identity/registry admission before publication','public sgpm command integration','same-user adversarial private-parent mutation is outside the admitted-parent model','streaming source-file reads instead of a bounded readonly compressed mapping','M8 performance/RSS/final SG SDK packaging']}
    cache={}
    def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf8')
    def read(path,pin=None):
        path=Path(path).resolve()
        if path not in cache:cache[path]=path.read_bytes()
        raw=cache[path];fingerprint=digest(raw)
        if pin is not None and pin!=fingerprint:raise ValueError('qualified input identity differs: '+str(path))
        group='sources'if path.is_relative_to(ROOT)else'external_sources'
        report[group][str(path.relative_to(ROOT))if group=='sources'else str(path)]=fingerprint
        return raw
    def tool(name,entry):
        path=Path(entry['path']).resolve();read(path,entry['sha256']);report['tools'][name]={'path':str(path),'sha256':entry['sha256']};return path
    pure={k:v for k,v in os.environ.items()if not k.upper().startswith(('SENGOO_','SGPLATFORM_','PYTHON','RUSTC','RUSTDOC','CARGO_'))};pure['PATH']=''
    def run(name,command,env=pure,expected=0,empty=False):
        command=list(map(str,command));start=time.monotonic();result=subprocess.run(command,cwd=out,env=env,capture_output=True,timeout=900)
        for suffix,raw in [('stdout',result.stdout),('stderr',result.stderr)]:
            path=out/(name+'.'+suffix);path.write_bytes(raw);report['artifacts'][str(path)]=digest(raw)
        report['steps'].append({'name':name,'command':command,'exit':result.returncode,'expected_exit':expected,'empty_path':env.get('PATH')=='','seconds':time.monotonic()-start,'stdout_sha256':digest(result.stdout),'stderr_sha256':digest(result.stderr)})
        save();print(name,result.returncode,flush=True)
        if result.returncode!=expected or empty and(result.stdout or result.stderr):raise ValueError(name+' exit/output differs')
        return result
    def passed(path,stage):
        value=strict_json(read(path))
        if type(value.get('schema'))is not int or value['schema']!=1 or value.get('stage')!=stage or value.get('result')!='passed' or any(v!=[]for k,v in value.items()if k.startswith('changed_')or k=='checkout_changes'):raise ValueError('qualification stage/guards differ')
        return value
    save()
    try:
        text=passed(args.text_evidence,'sg-runtime-text-utf8');gzip=passed(args.gzip_stream_evidence,'sg-runtime-gzip-continuation');tar=passed(args.tar_evidence,'sg-runtime-tar-scan');rust=passed(args.rust_tar_evidence,'independent-tar-package-policy')
        for group in ['sources','external_sources']:
            for name,pin in text[group].items():read(ROOT/name if group=='sources'else name,pin)
        for name,entry in text['tools'].items():tool('text-producer-'+name,entry)
        native=tool('native',text['tools']['native']);formatter=tool('formatter',text['tools']['formatter']);clang=tool('clang',text['tools']['clang']);lld=tool('lld',text['tools']['clang-probe-linker']);kernel=tool('sdk-kernel32',text['tools']['sdk-kernel32']);objdump=tool('objdump',text['tools']['llvm-objdump-c-sdk-leaf-check']);readobj=tool('readobj',text['tools']['llvm-readobj-independent-pe'])
        for name in ['native','formatter']:
            if text['artifacts'].get(Path(text['tools'][name]['path']).name)!=text['tools'][name]['sha256']:raise ValueError('compiler/formatter artifact relationship differs')
        parent=text['linker_qualification']['evidence'];path=Path(parent['path']).resolve()
        if not path.is_relative_to(ROOT/'target')or text['sources'].get(str(path.relative_to(ROOT)))!=parent['sha256']:raise ValueError('declared linker report not source-bound')
        read(path,parent['sha256']);linker,branch=copy_linker(ROOT,path,out,report)
        if branch['binary_sha256']!=text['tools']['sg-linker']['sha256']:raise ValueError('linker binary differs')
        for name,entry in list(report['tools'].items()):tool(name,entry)
        for name,pin in list(report['sources'].items()):read(ROOT/name,pin)
        stack_source=args.text_evidence.resolve().parent/'ntdll-chkstk.lib';pin=text['stack_probe_provider']['import_library_sha256']
        if text['sources'].get(str(stack_source.relative_to(ROOT)))!=pin:raise ValueError('stack import relationship differs')
        stack=out/'ntdll-chkstk.lib';stack.write_bytes(read(stack_source,pin));read(stack,pin)
        python=tool('python',{'path':sys.executable,'sha256':sha(Path(sys.executable))})
        import tarfile,gzip as python_gzip,zlib
        for module in [tarfile,python_gzip]:read(module.__file__)
        if getattr(zlib,'__file__',None):zlib_path=Path(zlib.__file__)
        elif zlib.__spec__.origin=='built-in':
            filename=ctypes.windll.kernel32.GetModuleFileNameW;filename.argtypes=[ctypes.c_void_p,ctypes.c_wchar_p,ctypes.c_uint32];filename.restype=ctypes.c_uint32
            buffer=ctypes.create_unicode_buffer(32768)
            if not filename(ctypes.c_void_p(sys.dllhandle),buffer,len(buffer)):raise ValueError('builtin zlib Python DLL identity unavailable')
            zlib_path=Path(buffer.value)
        else:raise ValueError('zlib implementation identity unavailable')
        tool('zlib-implementation',{'path':str(zlib_path),'sha256':sha(zlib_path)})
        if sha(zlib_path)!=gzip['tools']['zlib-implementation']['sha256']:raise ValueError('independent zlib implementation differs from admitted producer')
        report['oracle_versions']={'python':sys.version,'zlib_compile':zlib.ZLIB_VERSION,'zlib_runtime':zlib.ZLIB_RUNTIME_VERSION}
        for name in ['verify_runtime_package_stream.py','verify_runtime_text.py','linker_evidence.py','runtime_evidence.py']:read(ROOT/'selfhost'/name)
        for name in ['proposal.md','design.md','specs/selfhost-package-archive/spec.md']:read(ROOT/'openspec/changes/add-selfhost-package-archive'/name)
        baseline=['tar.sg','tar_memory.c','gzip_stream.sg','gzip_stream_memory.c','gzip.sg','deflate.sg','deflate_stream.sg','deflate_stream_memory.c','deflate_memory.c','deflate_storage.c','gzip_memory.c']
        for name in baseline:
            origin=tar if name in ['tar.sg','tar_memory.c']else gzip
            read(ROOT/'selfhost/runtime'/name,origin['sources'][str(Path('selfhost/runtime')/name)])
        frozen={}
        for name in ['gzip_stream','deflate_stream','deflate']:
            folder=args.gzip_stream_evidence.resolve().parent/gzip['fresh_rounds'][0]['round'];path=folder/(name+'.obj');pin=gzip['fresh_rounds'][0]['sha256'][name]
            if gzip['artifacts'].get(str(path))!=pin:raise ValueError('gzip core object not artifact-bound')
            frozen[name]=read(path,pin)
        # Independent Rust observations remain immutable historical execution.
        # Bind their complete source/compiler closure, executable and actual log.
        for name,pin in rust['sources'].items():read(name,pin)
        for name,entry in rust['tools'].items():tool('rust-tar-'+name,entry)
        obs=rust['observation_stdout'];actual=read(obs['path'],obs['sha256']);executed=[x for x in rust['steps']if x['name']=='execute-all-fixtures']
        if len(executed)!=1 or executed[0]['exit']!=0 or executed[0]['command']!=[rust['tools']['tar-oracle']['path'],obs['corpus']]or executed[0]['stdout_sha256']!=obs['sha256']:raise ValueError('Rust actual observation relationship differs')
        if len(actual.splitlines())!=216 or len(rust['observations'])!=216:raise ValueError('Rust observation IDs incomplete')
        for index,(line,row)in enumerate(zip(actual.decode().splitlines(),rust['observations'])):
            fields=line.split('|')
            if len(fields)!=3 or fields[0]!=str(index)or fields[1]not in ['true','false']or type(row['ok'])is not bool or row['id']!=index or (fields[1]=='true')!=row['ok']:raise ValueError('Rust stdout ID/status differs')
            if row['ok']:
                entries=[]
                for item in fields[2].split(';')if fields[2]else []:
                    values=item.split(',')
                    if len(values)!=7 or not re.fullmatch('(?:[0-9a-f]{2})*',values[0])or not re.fullmatch('(?:[0-9a-f]{2})*',values[6])or any(not re.fullmatch('[0-9]+',v)for v in values[1:6]):raise ValueError('Rust entry stdout encoding differs')
                    entries.append(dict(zip(['path_hex','kind','declared','effective','header','data','content_hex'],[values[0],*map(int,values[1:6]),values[6]])))
                if entries!=row['entries']:raise ValueError('Rust actual entry bytes differ from declared observations')
            elif not re.fullmatch('(?:[0-9a-f]{2})*',fields[2])or bytes.fromhex(fields[2]).decode()!=row['error']:raise ValueError('Rust error observation differs')
        report['rust_tar_qualification']={'evidence':str(args.rust_tar_evidence.resolve()),'sha256':sha(args.rust_tar_evidence),'actual_stdout_sha256':obs['sha256'],'historical_observation_not_reexecuted':True}
        tar_corpus=args.tar_evidence.resolve().parent/'corpus/corpus.bin';tar_manifest=tar_corpus.with_suffix('.json')
        read(tar_corpus,tar['sources'][str(tar_corpus.relative_to(ROOT))]);read(tar_manifest,tar['sources'][str(tar_manifest.relative_to(ROOT))])
        if digest(read(obs['corpus']))!=digest(read(tar_corpus)):raise ValueError('Rust corpus differs from scanner corpus')
        snapshot=out/'inputs';snapshot.mkdir()
        names=['runtime/tar_stream.sg','runtime/tar_stream_memory.c','runtime/package_stream.sg','runtime/package_stream_os.c','runtime/package_stream_cli.c','tools/package_stream.sg','tests/tar_stream_cases.py','tests/tar_stream_probe.c','tests/package_stream_cases.py','tests/package_stream_probe.c','tests/package_stream_unwind_probe.c']+['runtime/'+n for n in baseline]
        for name in names:
            saved=snapshot/('package_cli.sg'if name=='tools/package_stream.sg'else Path(name).name);saved.write_bytes(read(ROOT/'selfhost'/name));read(saved)
        for name in ['tar_stream','package_stream','package_cli']:
            run('format-'+name,[formatter,'--check',snapshot/(name+'.sg')],empty=True)
        for name in ['tar.sg','gzip.sg']:read(snapshot/name)
        run('generate-tar-events',[python,'-B','-X','utf8',snapshot/'tar_stream_cases.py','--corpus',tar_corpus,'--output',out/'tar-events'])
        events=strict_json(read(out/'tar-events/manifest.json'));event_bytes=read(out/'tar-events/corpus.bin',events['sha256'])
        if events['records']!=216 or [x['id']for x in events['fixtures']]!=list(range(216)):raise ValueError('tar event IDs incomplete')
        report['tar_event_oracle']={'fixtures':216,'runs':sum(x['schedules']for x in events['fixtures']),'source_corpus_sha256':events['source_corpus_sha256'],'fixed_arena_differences':[x for x in events['fixtures']if x['difference']]}
        run('generate-package-cases',[python,'-B','-X','utf8',snapshot/'package_stream_cases.py','--output',out/'package-cases','--large'])
        cases=strict_json(read(out/'package-cases/manifest.json'))
        if len(cases['fixtures'])!=48 or [x['id']for x in cases['fixtures']]!=list(range(48)):raise ValueError('package fixture IDs incomplete')
        for row in cases['fixtures']:read(out/'package-cases'/row['path'],row['sha256'])
        cenv=dict(pure,PATH=str(clang.parent)+os.pathsep+os.environ.get('PATH',''))
        def compile_c(name,source,target,extra=()):
            flags=['-std=c11','-O2','-g0','-fno-ident','-fno-addrsig','-fno-stack-protector',*extra];before=target.with_suffix('.before.d');after=target.with_suffix('.d')
            run(name+'-headers',[clang,*flags,'-M','-MT','runtime_text_dependency','-MF',before,source],cenv);read(before);headers={p:digest(read(p))for p in dependencies(before)}
            run(name,[clang,*flags,'-MD','-MT','runtime_text_dependency','-MF',after,'-c',source,'-o',target],cenv);read(after)
            if dependencies(after)!=set(headers)or any(sha(p)!=pin for p,pin in headers.items()):raise ValueError('C actual header dependencies changed')
            report['artifacts'][str(target)]=sha(target);return target
        rounds=[];built_rounds=[]
        exports=set()
        cstems=['package_stream_os','package_stream_cli','deflate_stream_memory','deflate_memory','deflate_storage','gzip_memory','gzip_stream_memory','tar_memory','tar_stream_memory']
        for stem in cstems:exports.update(re.findall(r'^\s*(?!static\b)[A-Za-z_][A-Za-z0-9_\s*]*\b(sengoo_[A-Za-z0-9_]+)\s*\([^;{}]*\)\s*\{',read(snapshot/(stem+'.c')).decode(),re.M))
        for number,label in enumerate(['first fresh 世界 &','second fresh 世界 &']):
            folder=out/label;folder.mkdir();built={}
            (folder/'tar.sg').write_bytes(read(snapshot/'tar.sg'));read(folder/'tar.sg')
            for stem in ['tar_stream','package_stream','package_cli']:
                source=folder/(stem+'.sg');source.write_bytes(read(snapshot/(stem+'.sg')));read(source);obj=folder/(stem+'.obj')
                result=run(str(number)+'-emit-'+stem,[native,source,obj,'--wrap'])
                if result.stderr or strict_json(result.stdout)!={'schema_version':1,'ok':True,'phase':'object'}:raise ValueError('actual SG object protocol differs')
                built[stem]=obj
            for stem,data in frozen.items():built[stem]=folder/(stem+'.obj');built[stem].write_bytes(data);read(built[stem],digest(data))
            objects=[]
            for stem in cstems:
                raw=compile_c(str(number)+'-c-'+stem,snapshot/(stem+'.c'),folder/(stem+'.raw.obj'));data=raw.read_bytes();normalized=bytearray(data);normalized[4:8]=bytes(4)
                obj=folder/(stem+'.obj');obj.write_bytes(normalized)
                if data[:4]!=normalized[:4]or data[8:]!=normalized[8:]:raise ValueError('C timestamp-only normalization differs')
                report.setdefault('timestamp_only_c_metadata',[]).append({'object':str(obj),'raw_sha256':digest(data),'raw_timestamp':struct.unpack_from('<I',data,4)[0],'normalized_sha256':digest(normalized),'only_bytes_4_to_7_changed':True,'unwind_aux_symbols_relocations_preserved':True});objects.append(obj);built[stem+'-sdk']=obj
            definition=folder/'provider.def';definition.write_text('LIBRARY sg-package-dev\nEXPORTS\n'+'\n'.join(sorted(exports))+'\n',encoding='ascii');read(definition)
            dll=folder/'sg-package-dev.dll';lib=folder/'sg-package-dev.lib'
            run(str(number)+'-sdk-dll',[lld,'/dll','/noentry','/nodefaultlib','/machine:x64','/Brepro','/out:'+str(dll),'/implib:'+str(lib),'/def:'+str(definition),*objects,kernel],cenv)
            built['sdk-dll']=dll;built['sdk-import']=lib
            inspected=run(str(number)+'-sdk-unwind',[readobj,'--unwind','--coff-imports','--coff-exports',dll],cenv)
            if b'RuntimeFunction'not in inspected.stdout or b'NtCreateFile'in inspected.stdout:raise ValueError('SDK real unwind/import shape differs')
            image=folder/'sg-package-extract-preview.exe';run(str(number)+'-sg-link',[linker,'link',image,'main',*[built[n]for n in ['package_cli','package_stream','tar_stream','gzip_stream','deflate_stream','deflate']],lib,stack],empty=True);built['image']=image
            hashes={name:sha(path)for name,path in built.items()}
            if rounds and rounds[0]['sha256']!=hashes:raise ValueError('two fresh SG/CSDK/DLL/importlib/PE bytes differ')
            rounds.append({'round':label,'sha256':hashes});built_rounds.append(built);report['artifacts'].update({str(path):sha(path)for path in built.values()})
        report['fresh_rounds']=rounds
        # Preserve and execute a real SDK stack frame, not just a .pdata label.
        for name,entry in tar['tools'].items():
            if name.startswith('debug-dependency-')or name in ['lldb','liblldb']:tool(name,entry)
        debug_python=Path(tar['tools']['debug-dependency-python310.dll']['path']).parent
        for path in [*debug_python.glob('*.pyd'),*debug_python.glob('*.dll'),debug_python/'python310.zip']:read(path)
        debugobj=compile_c('compile-sdk-unwind-probe',snapshot/'package_stream_unwind_probe.c',out/'unwind-probe.obj',['-O0','-g','-gcodeview'])
        folder=out/rounds[0]['round'];debugexe=folder/'unwind.exe'
        c_link(run,read,report,'link-sdk-unwind-probe',clang,[debugobj,built_rounds[0]['sdk-import'],'-Wl,/debug,/pdb:'+str(folder/'unwind.pdb')],debugexe,cenv)
        debugenv=dict(pure,PYTHONHOME=str(debug_python),PYTHONPATH=str(debug_python/'python310.zip')+os.pathsep+str(debug_python),LLDB_USE_NATIVE_PDB_READER='1',PATH=str(debug_python)+os.pathsep+str(clang.parent))
        active=run('actual-active-sdk-unwind',[report['tools']['lldb']['path'],'--batch','-o','breakpoint set --shlib sg-package-dev.dll -n sengoo_ps_component_equal','-o','run','-o','thread step-inst -c 5','-o','thread backtrace','-o','disassemble -f','-o','continue',debugexe],debugenv)
        for expected in ['sengoo_ps_component_equal + 12','package_unwind_caller','`main','exited with status = 42','subq   $0x438, %rsp']:
            if expected not in active.stdout.decode():raise ValueError('actual SDK body unwind missing '+expected)
        loaded=ctypes.WinDLL(str(built_rounds[0]['sdk-dll']))
        class RuntimeFunction(ctypes.Structure):_fields_=[('begin',ctypes.c_uint32),('end',ctypes.c_uint32),('unwind',ctypes.c_uint32)]
        lookup=ctypes.windll.ntdll.RtlLookupFunctionEntry;lookup.argtypes=[ctypes.c_uint64,ctypes.POINTER(ctypes.c_uint64),ctypes.c_void_p];lookup.restype=ctypes.POINTER(RuntimeFunction)
        registered=[]
        for name in ['sengoo_ps_component_equal','sengoo_ps_session_new','sengoo_ps_create_node','sengoo_ps_write_node','sengoo_ps_flush_node','sengoo_ps_publish','sengoo_ps_session_close','sengoo_ps_cli_count','sengoo_ps_cli_open_input','sengoo_ps_cli_open_parent']:
            pointer=ctypes.cast(getattr(loaded,name),ctypes.c_void_p).value;base=ctypes.c_uint64();entry=lookup(pointer,ctypes.byref(base),None)
            if not entry or not base.value+entry.contents.begin<=pointer<base.value+entry.contents.end:raise ValueError('actual SDK nonleaf export has no registered unwind '+name)
            registered.append({'symbol':name,'rva':pointer-base.value,'begin':entry.contents.begin,'end':entry.contents.end,'unwind_rva':entry.contents.unwind,'unwind_header':ctypes.string_at(base.value+entry.contents.unwind,4).hex()})
        report['registered_sdk_unwind']=registered;report['live_sdk_body_backtrace_verified']=True
        kernel32=ctypes.WinDLL('kernel32',use_last_error=True);ntdll=ctypes.WinDLL('ntdll');shell=ctypes.WinDLL('shell32')
        getmodule=kernel32.GetModuleHandleExW;getmodule.argtypes=[ctypes.c_uint32,ctypes.c_void_p,ctypes.POINTER(ctypes.c_void_p)];getmodule.restype=ctypes.c_int
        filename=kernel32.GetModuleFileNameW;filename.argtypes=[ctypes.c_void_p,ctypes.c_wchar_p,ctypes.c_uint32];filename.restype=ctypes.c_uint32
        providers=[]
        for module,symbols in [(kernel32,['VirtualAlloc','VirtualFree','VirtualQuery','MultiByteToWideChar','WideCharToMultiByte','CompareStringOrdinal','CreateFileW','WriteFile','FlushFileBuffers','GetFileInformationByHandleEx','SetFileInformationByHandle','CloseHandle','CreateFileMappingW','MapViewOfFile','UnmapViewOfFile','GetFileSizeEx','GetModuleHandleW','GetProcAddress','LoadLibraryW','FreeLibrary','GetCommandLineW','LocalFree']),(ntdll,['NtCreateFile','NtSetInformationFile','RtlLookupFunctionEntry']),(shell,['CommandLineToArgvW'])]:
            for name in symbols:
                pointer=ctypes.cast(getattr(module,name),ctypes.c_void_p).value;handle=ctypes.c_void_p();buffer=ctypes.create_unicode_buffer(32768)
                if not getmodule(6,pointer,ctypes.byref(handle))or not filename(handle,buffer,len(buffer)):raise ValueError('actual OS provider unavailable '+name)
                path=Path(buffer.value).resolve();providers.append({'symbol':name,'path':str(path),'sha256':digest(read(path)),'rva':pointer-handle.value})
        report['actual_os_providers']=providers
        report['artifacts'].update({str(path):sha(path)for path in [debugexe,folder/'unwind.pdb']})
        # Instrumented parser: all historical forms plus byte initialization,
        # state/input tail guards and exact event acknowledgements.
        tprobe=compile_c('compile-tar-probe',snapshot/'tar_stream_probe.c',out/'tar-probe.obj')
        tmem=compile_c('compile-tar-os',snapshot/'tar_memory.c',out/'tar-os.obj',['-DSENGOO_TAR_INSTRUMENTED'])
        c_link(run,read,report,'link-tar-probe',clang,[tprobe,tmem,built_rounds[0]['tar_stream'],stack],out/'tar-probe.exe',cenv)
        result=run('tar-events',[out/'tar-probe.exe',out/'tar-events/corpus.bin']);stats=strict_json(result.stdout)
        integer_fields=['schema','fixtures','runs','calls','entry_events','data_events','reads','writes','state_bytes','protocols'];boolean_fields=['ok','readonly_input','input_tailguard','state_tailguard','initialized_byte_checks']
        if result.stderr or set(stats)!=set(integer_fields+boolean_fields)or any(type(stats[k])is not int or stats[k]<1 for k in integer_fields)or stats['schema']!=1 or stats['fixtures']!=216 or stats['runs']!=report['tar_event_oracle']['runs']or stats['protocols']!=8 or stats['state_bytes']!=18438400 or any(stats[k]is not True for k in boolean_fields):raise ValueError('tar event/protocol completion differs')
        report['tar_event_results']=stats
        control=run('uninitialized-state-control',[out/'tar-probe.exe','--uninitialized-control'],expected=1)
        if control.stdout or b'reason=state-uninitialized'not in control.stderr:raise ValueError('uninitialized workspace control did not reach actual adapter')
        bad=bytearray(event_bytes);struct.pack_into('<q',bad,16,4);wrong=out/'wrong-event-oracle.bin';wrong.write_bytes(bad);read(wrong)
        control=run('wrong-event-oracle-control',[out/'tar-probe.exe',wrong],expected=1)
        if control.stdout or b'fixture=0 reason=valid-terminal expected=4 actual=2'not in control.stderr:raise ValueError('wrong event oracle did not reach actual scanner')
        pprobe=compile_c('compile-package-probe',snapshot/'package_stream_probe.c',out/'package-probe.obj')
        pos=compile_c('compile-package-os',snapshot/'package_stream_os.c',out/'package-os.obj',['-DSENGOO_PACKAGE_INSTRUMENTED'])
        pmem=[built_rounds[0][stem+'-sdk']for stem in ['gzip_memory','gzip_stream_memory','tar_stream_memory']]
        core=[built_rounds[0][name]for name in ['package_stream','tar_stream','gzip_stream','deflate_stream','deflate']]
        c_link(run,read,report,'link-package-probe',clang,[pprobe,pos,tmem,*pmem,*core,stack],out/'package-probe.exe',cenv)
        controls=run('package-api-controls',[out/'package-probe.exe','--api-controls']);value=strict_json(controls.stdout)
        expected={'schema':1,'controls':10,'bad_extent_zero_access':True,'same_stage_target_rejected':True,'dos_root_rejected':True,'allocations':0}
        if controls.stderr or set(value)!=set(expected)or any(type(value[k])is not type(v)or value[k]!=v for k,v in expected.items()):raise ValueError('package actual API controls differ')
        report['api_controls']=value
        results=[]
        for row in cases['fixtures']:
            directory=out/'transactions'/str(row['id']);directory.mkdir(parents=True);outside=directory/'outside';outside.mkdir();sentinel=outside/'outside.keep';sentinel.write_bytes(b'outside');read(sentinel)
            if row['existing_target']:(directory/'final').mkdir();(directory/'final/existing.keep').write_bytes(b'existing')
            result=run('case-'+str(row['id']),[out/'package-probe.exe',directory,out/'package-cases'/row['path'],'private','final',row['cancel_after'],row['attack'],row['oom'],outside]);value=strict_json(result.stdout)
            fields=['schema','result','allocations','frees','live_bytes','peak_sg_bytes','handles_before','handles_after','peak_working_set','os_sessions','os_frees','os_region_bytes','attack_processes','attack_exit','readonly_files','warmup_handles_before','warmup_handles_after','created_files','created_dirs','write_calls','closed_children','reopened_children','identity_rejections','verified_releases','memory_reads','memory_writes']
            if result.stderr or set(value)!=set(fields+['parent_volume','parent_file_id','parent_identity_stable'])or any(type(value[k])is not int for k in fields+['parent_volume'])or type(value['parent_file_id'])is not str or not re.fullmatch('[0-9a-f]{32}',value['parent_file_id'])or value['parent_identity_stable']is not True or value['schema']!=1 or value['result']!=row['expected']or value['live_bytes']!=0 or value['allocations']!=value['frees']or value['verified_releases']!=value['frees']or value['handles_before']!=value['handles_after']or value['os_sessions']!=value['os_frees']:raise ValueError('transaction result/resource receipt differs '+row['name'])
            if sentinel.read_bytes()!=b'outside':raise ValueError('outside sentinel modified')
            if row['expected']==0:
                actual={p.relative_to(directory/'final').as_posix():sha(p)for p in(directory/'final').rglob('*')if p.is_file()}
                if actual!=row['files']or(directory/'private').exists():raise ValueError('published tree differs')
                for path in(directory/'final').rglob('*'):
                    if path.is_file():report['artifacts'][str(path)]=sha(path)
            elif row['expected']==-11:
                if not(directory/'private').exists()or value['attack_exit']!=0:raise ValueError('unknown/replaced object cleanup control differs')
                if row['attack']=='unknown'and(directory/'private/foreign.keep').read_bytes()!=b'foreign':raise ValueError('unknown child deleted')
                if row['attack']=='replace'and(directory/'private/Sengoo.toml').read_bytes()!=b'foreign':raise ValueError('replacement identity was deleted')
            elif(directory/'private').exists():raise ValueError('owned private staging not removed')
            if row['existing_target']:
                if(directory/'final/existing.keep').read_bytes()!=b'existing':raise ValueError('old target modified')
            elif row['attack']in ['target','replace']:
                if(directory/'final/race.keep').read_bytes()!=b'foreign':raise ValueError('racing target replaced')
            elif row['expected']!=0 and(directory/'final').exists():raise ValueError('failure published a partial final')
            if row['name']in ['bad-crc','bad-size','bad-late-member','cancel-after-write']and value['write_calls']<1:raise ValueError('late-error fixture did not write real private content')
            if row['attack']=='replace'and value['identity_rejections']<1:raise ValueError('replacement did not reach actual file-ID rejection')
            results.append({'id':row['id'],'name':row['name'],'expected':row['expected'],'stats':value})
        report['transaction_results']=results
        # Host names are narrow ASCII components under one admitted parent
        # handle, rather than two unrelated host paths. Exercise real plain
        # ASCII 8.3 aliases and an intermediate parent junction.
        host=out/'host-identities';host.mkdir();existing=host/'ExistingFinalDirectory';existing.mkdir();(existing/'existing.keep').write_bytes(b'existing')
        create=kernel32.CreateFileW;create.argtypes=[ctypes.c_wchar_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p,ctypes.c_uint32,ctypes.c_uint32,ctypes.c_void_p];create.restype=ctypes.c_void_p
        close=kernel32.CloseHandle;close.argtypes=[ctypes.c_void_p];close.restype=ctypes.c_int
        handle=create(str(existing),0x10100,7,None,3,0x02000000,None)
        setshort=kernel32.SetFileShortNameW;setshort.argtypes=[ctypes.c_void_p,ctypes.c_wchar_p];setshort.restype=ctypes.c_int
        if handle==ctypes.c_void_p(-1).value or not setshort(handle,'ALIAS'):raise ValueError('required actual short-name control unavailable '+str(ctypes.get_last_error()))
        close(handle)
        same=run('actual-shortname-file-id',[out/'package-probe.exe','--parent-identity',existing,host/'ALIAS']);identity=strict_json(same.stdout)
        if same.stderr or identity.get('same_volume_and_file_id')is not True:raise ValueError('short name did not resolve to actual same OS object')
        sample=out/'package-cases'/cases['fixtures'][0]['path'];image=built_rounds[0]['image']
        run('actual-shortname-stage-alias',[image,sample,host,'ALIAS','ExistingFinalDirectory'],expected=78,empty=True)
        if(existing/'existing.keep').read_bytes()!=b'existing'or sorted(p.name for p in existing.iterdir())!=['existing.keep']:raise ValueError('short-name existing target modified')
        run('same-root-name-case',[image,sample,host,'private','PRIVATE'],expected=73,empty=True)
        if(host/'private').exists():raise ValueError('same stage/final name allocated private root')
        junction=out/'parent-junction';junction.mkdir();wide='\\??\\'+str(host);encoded=wide.encode('utf-16-le')+bytes(4)
        reparse=struct.pack('<IHHHHHH',0xa0000003,8+len(encoded),0,0,len(wide.encode('utf-16-le')),len(wide.encode('utf-16-le'))+2,0)+encoded
        handle=create(str(junction),0x40000000,0,None,3,0x02000000|0x00200000,None)
        ioctl=kernel32.DeviceIoControl;ioctl.argtypes=[ctypes.c_void_p,ctypes.c_uint32,ctypes.c_void_p,ctypes.c_uint32,ctypes.c_void_p,ctypes.c_uint32,ctypes.POINTER(ctypes.c_uint32),ctypes.c_void_p];ioctl.restype=ctypes.c_int
        size=ctypes.c_uint32();buffer=ctypes.create_string_buffer(reparse)
        if handle==ctypes.c_void_p(-1).value or not ioctl(handle,0x000900a4,buffer,len(reparse),None,0,ctypes.byref(size),None):raise ValueError('required actual junction control unavailable')
        close(handle)
        run('junction-parent-leaf',[image,sample,junction,'private','final'],expected=66,empty=True)
        child=host/'physical';child.mkdir();same=run('actual-junction-parent-file-id',[out/'package-probe.exe','--parent-identity',child,junction/'physical']);physical=strict_json(same.stdout)
        if same.stderr or physical.get('same_volume_and_file_id')is not True:raise ValueError('intermediate junction did not bind actual same parent')
        run('junction-intermediate-same-root-name',[image,sample,junction/'physical','private','PRIVATE'],expected=73,empty=True)
        if list(child.iterdir()):raise ValueError('aliased parent same-name control wrote private files')
        report['host_parent_identity_controls']={'shortname_same_file_id':identity,'intermediate_junction_same_parent':physical,'ascii_single_components_only':True,'single_parent_handle':True,'existing_object_not_reused':True,'junction_leaf_rejected':True}
        # Actual no-CRT product path, built solely by the qualified SG linker.
        for number,built in enumerate(built_rounds):
            directory=out/('cli-'+str(number));directory.mkdir();row=cases['fixtures'][0]
            run('cli-cold-'+str(number),[built['image'],out/'package-cases'/row['path'],directory,'private','final'],empty=True)
            if {p.relative_to(directory/'final').as_posix():sha(p)for p in(directory/'final').rglob('*')if p.is_file()}!=row['files']:raise ValueError('actual SG-linked command tree differs')
            run('cli-existing-'+str(number),[built['image'],out/'package-cases'/row['path'],directory,'private','final'],expected=78,empty=True)
        report['fixed_sg_workspaces']=19565824;report['c_session_extent_is_additional']=True;report['bounded_compressed_readonly_mapping']=67108864;report['content_budget']=268435456;report['tar_wire_budget']=536870912;report['entry_budget']=10000;report['result']='passed'
    except BaseException as error:
        report['result']='failed';report['error']=repr(error);raise
    finally:
        report['changed_sources']=[name for name,pin in report['sources'].items()if not(ROOT/name).is_file()or sha(ROOT/name)!=pin]
        report['changed_external_sources']=[name for name,pin in report['external_sources'].items()if not Path(name).is_file()or sha(Path(name))!=pin]
        report['changed_tools']=[name for name,entry in report['tools'].items()if not Path(entry['path']).is_file()or sha(Path(entry['path']))!=entry['sha256']]
        report['revision_end']=revision();report['git_status_end']=git_status();report['completed_at']=dt.datetime.now(dt.timezone.utc).isoformat();report['initial_cached_reads']={str(path):1 for path in cache}
        for name in ['changed_sources','changed_external_sources','changed_tools']:
            if report[name]:report['result']='failed'
        save()
        if report['result']!='passed'and not sys.exc_info()[0]:raise ValueError('final source/tool guard failed')
if __name__=='__main__':main()
