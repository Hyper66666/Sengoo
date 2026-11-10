"""Qualify canonical SG manifest/hash admission and real package import preview."""
from __future__ import annotations
import argparse,ctypes,datetime as dt,json,os,platform,re,shutil,struct,subprocess,sys,time
from pathlib import Path
from verify_runtime_text import strict_json,digest,sha,revision,git_status,dependencies
from runtime_evidence import c_link
from linker_evidence import load_linker
ROOT=Path(__file__).resolve().parents[1]

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['package-evidence','rust-evidence','output']:p.add_argument('--'+name,type=Path,required=True)
    args=p.parse_args();out=args.output.resolve()
    if not out.is_relative_to(ROOT/'target'):raise ValueError('output must be fresh target directory')
    out.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'stage':'sg-canonical-package-admission','result':'running','host':platform.platform(),'revision_start':revision(),'git_status_start':git_status(),'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'sources':{},'external_sources':{},'tools':{},'artifacts':{},'steps':[],'public_sgpm_replacement_complete':False,'remaining':['existing sgpm dependency graph/lockfile/network/cache operations','existing build/check/run/test/fmt/doc/scaffold/publish command contracts','performance/RSS/final SDK packaging']};cache={}
    def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf8')
    def read(path,pin=None):
        path=Path(path).resolve()
        if path not in cache:cache[path]=path.read_bytes()
        raw=cache[path];fingerprint=digest(raw)
        if pin is not None and fingerprint!=pin:raise ValueError('qualified input identity differs '+str(path))
        group='sources'if path.is_relative_to(ROOT)else'external_sources';report[group][str(path.relative_to(ROOT))if group=='sources'else str(path)]=fingerprint;return raw
    def tool(name,entry):
        path=Path(entry['path']).resolve();read(path,entry['sha256']);report['tools'][name]={'path':str(path),'sha256':entry['sha256']};return path
    pure={k:v for k,v in os.environ.items()if not k.upper().startswith(('SENGOO_','SGPLATFORM_','PYTHON','RUSTC','RUSTDOC','CARGO_'))};pure['PATH']=''
    def run(name,command,env=pure,expected=0,empty=False):
        command=list(map(str,command));start=time.monotonic();result=subprocess.run(command,cwd=out,env=env,capture_output=True,timeout=900)
        for suffix,raw in [('stdout',result.stdout),('stderr',result.stderr)]:
            file=out/(name+'.'+suffix);file.write_bytes(raw);report['artifacts'][str(file)]=digest(raw)
        report['steps'].append({'name':name,'command':command,'exit':result.returncode,'expected_exit':expected,'empty_path':env.get('PATH')=='','seconds':time.monotonic()-start,'stdout_sha256':digest(result.stdout),'stderr_sha256':digest(result.stderr)});save();print(name,result.returncode,flush=True)
        if result.returncode!=expected or empty and(result.stdout or result.stderr):raise ValueError(name+' exit/output differs')
        return result
    def qualify(path,stage):
        data=strict_json(read(path))
        if data.get('schema')!=1 or type(data['schema'])is not int or data.get('stage')!=stage or data.get('result')!='passed'or any(v!=[]for k,v in data.items()if k.startswith('changed_')or k=='checkout_changes'):raise ValueError('qualification stage/guards differ')
        return data
    save()
    try:
        package=qualify(args.package_evidence,'sg-runtime-package-stream-extraction');rust=qualify(args.rust_evidence,'independent-sgpm-manifest-content-policy')
        for group in ['sources','external_sources']:
            for name,pin in package[group].items():read(ROOT/name if group=='sources'else name,pin)
        for name,entry in package['tools'].items():tool('package-producer-'+name,entry)
        for name,pin in rust['sources'].items():read(name,pin)
        for name,entry in rust['tools'].items():tool('rust-producer-'+name,entry)
        for name,pin in rust['cargo_config_candidates'].items():
            path=Path(name)
            if(sha(path)if path.is_file()else None)!=pin:raise ValueError('Rust config candidate changed')
        oracle=tool('canonical-oracle',rust['oracle_binary']);native=tool('native',package['tools']['native']);formatter=tool('formatter',package['tools']['formatter']);clang=tool('clang',package['tools']['clang']);lld=tool('lld',package['tools']['lld']);linker=tool('sg-linker',package['tools']['sg-linker']);kernel=tool('sdk-kernel32',package['tools']['sdk-kernel32'])
        if package['linker_qualification']['binary']['sha256']!=package['tools']['sg-linker']['sha256']:raise ValueError('actual linker qualification relationship differs')
        parent=package['linker_qualification']['evidence'];parent_path=Path(parent['path']).resolve()
        if package['sources'].get(str(parent_path.relative_to(ROOT)))!=parent['sha256']:raise ValueError('actual linker declared parent pin differs')
        read(parent_path,parent['sha256'])
        original_linker=load_linker(ROOT,parent_path)
        if original_linker['binary_sha256']!=package['tools']['sg-linker']['sha256']:raise ValueError('historical linker branch resolved to a different binary')
        for name,pin in original_linker['consumed'].items():read(ROOT/name,pin)
        python=tool('python',{'path':sys.executable,'sha256':sha(Path(sys.executable))})
        git_path=shutil.which('git')
        if not git_path:raise ValueError('actual Git inventory tool unavailable')
        git=tool('git-inventory',{'path':git_path,'sha256':sha(Path(git_path))})
        for name in ['verify_package_admission.py','verify_runtime_text.py','runtime_evidence.py','linker_evidence.py','tests/package_manifest_rust_probe.py','tests/package_manifest_oracle.rs']:read(ROOT/'selfhost'/name)
        read(ROOT/'selfhost/src/link_hash.sg')
        snapshot=out/'inputs';snapshot.mkdir()
        stems=['package_stream','package_admission','manifest_toml','manifest_scalar','manifest_semver','manifest_schema','registry_metadata','sha256','tar_stream','gzip_stream','deflate_stream','deflate','utf8']
        extra=['tests/manifest_cases.py','tests/manifest_differential.py','tests/manifest_probe.c','tests/registry_metadata_cases.py','tests/package_manifest_fixtures.py','tests/sha256_probe.c','tests/package_snapshot_probe.c','tests/package_import_cases.py','tests/package_import_probe.c','tests/package_import_differential.py','tests/package_stream_probe.c','tests/package_stream_cases.py','tools/sgpm_preview.sg','runtime/package_admission_os.c','runtime/package_admission_cli.c','runtime/package_stream_os.c','runtime/package_stream_cli.c','runtime/tar_memory.c','runtime/tar_stream_memory.c','runtime/gzip_memory.c','runtime/gzip_stream_memory.c']
        for name in extra:
            saved=snapshot/Path(name).name;saved.write_bytes(read(ROOT/'selfhost'/name));read(saved)
        # Bind exact algorithm sources to the expanded full legacy regression.
        folder=args.package_evidence.resolve().parent/package['fresh_rounds'][0]['round'];pinned={}
        for stem in stems:
            file=folder/(stem+'.obj');pin=package['fresh_rounds'][0]['sha256'][stem]
            if package['artifacts'].get(str(file))!=pin:raise ValueError('expanded algorithm object not artifact-bound '+stem)
            pinned[stem]=read(file,pin)
            if stem in ['package_admission','manifest_toml','manifest_scalar','manifest_semver','manifest_schema','registry_metadata','sha256']:
                name=str(Path('selfhost/runtime')/(stem+'.sg'));read(ROOT/name,package['sources'][name])
        stack_origin=args.package_evidence.resolve().parent/'ntdll-chkstk.lib';stack_pin=package['sources'][str(stack_origin.relative_to(ROOT))];stack=out/'ntdll-chkstk.lib';stack.write_bytes(read(stack_origin,stack_pin));read(stack)
        run('format-sgpm-preview',[formatter,'--check',snapshot/'sgpm_preview.sg'],empty=True)
        cenv=dict(pure,PATH=str(clang.parent)+os.pathsep+os.environ.get('PATH',''))
        def compile_c(name,source,target,extra_flags=()):
            flags=['-std=c11','-O2','-g0','-fno-ident','-fno-addrsig','-fno-stack-protector',*extra_flags];before=target.with_suffix('.before.d');after=target.with_suffix('.d')
            run(name+'-headers',[clang,*flags,'-M','-MT','runtime_text_dependency','-MF',before,source],cenv);read(before);headers={p:digest(read(p))for p in dependencies(before)}
            run(name,[clang,*flags,'-MD','-MT','runtime_text_dependency','-MF',after,'-c',source,'-o',target],cenv);read(after)
            if dependencies(after)!=set(headers)or any(sha(path)!=pin for path,pin in headers.items()):raise ValueError('actual C/header closure changed')
            report['artifacts'][str(target)]=sha(target);return target
        rounds=[];built_rounds=[]
        sdkstems=['package_admission_os','package_admission_cli','deflate_stream_memory','deflate_memory','deflate_storage','gzip_memory','gzip_stream_memory','tar_memory','tar_stream_memory','memory_read']
        for number,label in enumerate(['first fresh 世界 &','second fresh 世界 &']):
            directory=out/label;directory.mkdir();built={}
            for stem,data in pinned.items():
                obj=directory/(stem+'.obj');obj.write_bytes(data);read(obj,digest(data));built[stem]=obj
            cli=directory/'sgpm_preview.obj';made=run(str(number)+'-emit-sgpm-preview',[native,snapshot/'sgpm_preview.sg',cli,'--wrap'])
            if made.stderr or strict_json(made.stdout)!={'schema_version':1,'ok':True,'phase':'object'}:raise ValueError('actual SG CLI object protocol differs')
            built['cli']=cli;objects=[];exports=set()
            for stem in sdkstems:
                source=snapshot/(stem+'.c')
                if not source.exists():source=ROOT/'selfhost/runtime'/(stem+'.c');read(source)
                rawobj=compile_c(str(number)+'-sdk-'+stem,source,directory/(stem+'.raw.obj'));raw=rawobj.read_bytes();normalized=bytearray(raw);normalized[4:8]=bytes(4);obj=directory/(stem+'.obj');obj.write_bytes(normalized)
                if raw[:4]!=normalized[:4]or raw[8:]!=normalized[8:]:raise ValueError('SDK timestamp-only normalization differs')
                report.setdefault('sdk_timestamp_normalization',[]).append({'object':str(obj),'raw_sha256':digest(raw),'raw_timestamp':struct.unpack_from('<I',raw,4)[0],'normalized_sha256':digest(normalized),'all_other_bytes_equal':True});objects.append(obj);built[stem+'-sdk']=obj
                text=read(source).decode()
                if stem=='package_admission_os':text+='\n'+read(snapshot/'package_stream_os.c').decode()
                if stem=='package_admission_cli':text+='\n'+read(snapshot/'package_stream_cli.c').decode()
                exports.update(re.findall(r'^\s*(?!static\b)[A-Za-z_][A-Za-z0-9_\s*]*\b(sengoo_[A-Za-z0-9_]+)\s*\([^;{}]*\)\s*\{',text,re.M))
            definition=directory/'provider.def';definition.write_text('LIBRARY sg-package-admission-dev\nEXPORTS\n'+'\n'.join(sorted(exports))+'\n');read(definition)
            dll=directory/'sg-package-admission-dev.dll';library=directory/'sg-package-admission-dev.lib'
            run(str(number)+'-sdk-dll',[lld,'/dll','/noentry','/nodefaultlib','/machine:x64','/Brepro','/out:'+str(dll),'/implib:'+str(library),'/def:'+str(definition),*objects,kernel],cenv);built['sdk-dll']=dll;built['sdk-import']=library
            image=directory/'sgpm-preview.exe';run(str(number)+'-sg-link',[linker,'link',image,'main',cli,*[built[n]for n in stems],library,stack],empty=True);built['image']=image
            fingerprints={name:sha(file)for name,file in built.items()}
            if rounds and rounds[0]['sha256']!=fingerprints:raise ValueError('two fresh product objects/DLL/import/PE bytes differ')
            rounds.append({'round':label,'sha256':fingerprints});built_rounds.append(built);report['artifacts'].update({str(file):sha(file)for file in built.values()})
        report['fresh_rounds']=rounds
        inspected=run('actual-admission-sdk-unwind-imports',[package['tools']['readobj']['path'],'--unwind','--coff-imports','--coff-exports',built_rounds[0]['sdk-dll']],cenv)
        if b'RuntimeFunction'not in inspected.stdout or b'ReadFile'not in inspected.stdout or b'SetFilePointerEx'not in inspected.stdout:raise ValueError('actual admission SDK unwind/imports differ')
        loaded=ctypes.WinDLL(str(built_rounds[0]['sdk-dll']))
        class RuntimeFunction(ctypes.Structure):_fields_=[('begin',ctypes.c_uint32),('end',ctypes.c_uint32),('unwind',ctypes.c_uint32)]
        lookup=ctypes.windll.ntdll.RtlLookupFunctionEntry;lookup.argtypes=[ctypes.c_uint64,ctypes.POINTER(ctypes.c_uint64),ctypes.c_void_p];lookup.restype=ctypes.POINTER(RuntimeFunction)
        unwind=[]
        for name in ['sengoo_pa_file_identity','sengoo_pa_file_read','sengoo_pa_open_attributes','sengoo_pa_cli_write','sengoo_pa_cli_input_identity']:
            pointer=ctypes.cast(getattr(loaded,name),ctypes.c_void_p).value;base=ctypes.c_uint64();entry=lookup(pointer,ctypes.byref(base),None)
            if not entry or not base.value+entry.contents.begin<=pointer<base.value+entry.contents.end:raise ValueError('actual new nonleaf admission SDK has no registered unwind '+name)
            unwind.append({'symbol':name,'rva':pointer-base.value,'begin':entry.contents.begin,'end':entry.contents.end,'unwind_rva':entry.contents.unwind})
        report['new_registered_sdk_unwind']=unwind
        kernel32=ctypes.WinDLL('kernel32',use_last_error=True);getmodule=kernel32.GetModuleHandleExW;getmodule.argtypes=[ctypes.c_uint32,ctypes.c_void_p,ctypes.POINTER(ctypes.c_void_p)];getmodule.restype=ctypes.c_int;filename=kernel32.GetModuleFileNameW;filename.argtypes=[ctypes.c_void_p,ctypes.c_wchar_p,ctypes.c_uint32];filename.restype=ctypes.c_uint32
        providers=[]
        for name in ['ReadFile','SetFilePointerEx','GetFileSizeEx','GetFileInformationByHandleEx','SetFileShortNameW','WriteFile']:
            pointer=ctypes.cast(getattr(kernel32,name),ctypes.c_void_p).value;handle=ctypes.c_void_p();buffer=ctypes.create_unicode_buffer(32768)
            if not getmodule(6,pointer,ctypes.byref(handle))or not filename(handle,buffer,len(buffer)):raise ValueError('actual admission OS provider unavailable '+name)
            path=Path(buffer.value).resolve();providers.append({'symbol':name,'path':str(path),'sha256':digest(read(path)),'rva':pointer-handle.value})
        report['new_actual_os_providers']=providers
        # Compile independent initialized-byte instrumentation, not algorithm C.
        common=[built_rounds[0][name]for name in ['manifest_toml','manifest_scalar','manifest_semver','manifest_schema','registry_metadata','utf8']]
        manifest_obj=compile_c('compile-manifest-probe',snapshot/'manifest_probe.c',out/'manifest-probe.obj',['-DSENGOO_MANIFEST_VALIDATE']);manifest_exe=out/'manifest-probe.exe';c_link(run,read,report,'link-manifest-probe',clang,[manifest_obj,*common,stack],manifest_exe,cenv)
        metadata_obj=compile_c('compile-metadata-probe',snapshot/'manifest_probe.c',out/'metadata-probe.obj',['-DSENGOO_REGISTRY_METADATA']);metadata_exe=out/'metadata-probe.exe';c_link(run,read,report,'link-metadata-probe',clang,[metadata_obj,*common,stack],metadata_exe,cenv)
        run('generate-constructed-manifests',[python,'-B','-X','utf8',snapshot/'manifest_cases.py','--output',out/'constructed'])
        run('generate-manifest-inventory',[python,'-B','-X','utf8',snapshot/'package_manifest_fixtures.py','--root',ROOT,'--git',git,'--constructed',out/'constructed','--output',out/'manifests'])
        fixtures=strict_json(read(out/'manifests/manifest.json'))
        if fixtures['constructed']!=154 or fixtures['repository']<42 or fixtures['canonical_embedded']<18 or fixtures['skipped_rust_literal_lines']:raise ValueError('canonical manifest fixture inventory differs')
        inventory=fixtures['git_inventory']
        if inventory['exit']!=0 or inventory['command']!=[str(git),'ls-files','-z','--','**/Sengoo.toml','Sengoo.toml']:raise ValueError('actual Git fixture inventory relationship differs')
        read(out/'manifests/git.stdout',inventory['stdout_sha256']);read(out/'manifests/git.stderr',inventory['stderr_sha256'])
        for row in fixtures['cases']:read(out/'manifests'/row['path'],row['sha256'])
        for name,pin in fixtures['sources'].items():read(name,pin)
        run('manifest-differential',[python,'-B','-X','utf8',snapshot/'manifest_differential.py','--subject',manifest_exe,'--oracle',oracle,'--cases',out/'manifests','--output',out/'manifest-results'])
        model=strict_json(read(out/'manifest-results/evidence.json'))
        if model['result']!='passed'or len(model['cases'])!=len(fixtures['cases'])or model['differences']:raise ValueError('canonical full manifest model differential differs')
        report['manifest_cases']={'count':len(model['cases']),'constructed':154,'repository':fixtures['repository'],'canonical_embedded':fixtures['canonical_embedded'],'full_field_models_equal':True}
        run('generate-metadata',[python,'-B','-X','utf8',snapshot/'registry_metadata_cases.py','--output',out/'metadata'])
        run('metadata-differential',[python,'-B','-X','utf8',snapshot/'manifest_differential.py','--metadata','--subject',metadata_exe,'--oracle',oracle,'--cases',out/'metadata','--output',out/'metadata-results'])
        model=strict_json(read(out/'metadata-results/evidence.json'))
        if model['result']!='passed'or len(model['cases'])!=58 or model['differences']:raise ValueError('actual metadata typed/serialized differential differs')
        report['registry_metadata_cases']={'count':58,'typed_models_and_exact_serde_json_bytes_equal':True}
        values=strict_json(read(out/'manifests/semver.json'))['values'];semver=[]
        for mode in ['--semver','--req']:
            for value in values:
                index=len(semver);file=out/('semver-'+str(index)+'.txt');file.write_bytes(value.encode());read(file)
                a=run('semver-subject-'+str(index),[manifest_exe,mode,file]);b=run('semver-oracle-'+str(index),[oracle,mode,value]);subject,reference=strict_json(a.stdout),strict_json(b.stdout)
                if a.stderr or b.stderr or type(subject['valid'])is not bool or type(reference['ok'])is not bool or subject['valid']!=reference['ok']:raise ValueError('actual SemVer differential differs')
                semver.append({'id':index,'mode':mode,'text':value,'accepted':subject['valid']})
        report['semver_cases']=semver
        sha_obj=compile_c('compile-sha256-probe',snapshot/'sha256_probe.c',out/'sha256-probe.obj');sha_exe=out/'sha256-probe.exe';c_link(run,read,report,'link-sha256-probe',clang,[sha_obj,built_rounds[0]['sha256'],stack],sha_exe,cenv)
        hash_expected=strict_json(read(out/'manifests/sha256.json'));read(out/'manifests/sha256.bin',hash_expected['sha256']);actual=run('sha256-vectors-and-cuts',[sha_exe,out/'manifests/sha256.bin']);value=strict_json(actual.stdout)
        if actual.stderr or value['schema']!=1 or value['cases']!=hash_expected['cases']or value['runs']!=hash_expected['runs']or any(value[k]is not True for k in ['invalid_api_zero_access','initialized_state_checks','disposed_zero']):raise ValueError('actual SHA256 known vector/split/resource controls differ')
        report['sha256_results']=value
        poison=bytearray(read(out/'manifests/sha256.bin'));poison[16]^=1;wrong=out/'wrong-sha256-oracle.bin';wrong.write_bytes(poison);read(wrong)
        rejected=run('wrong-sha256-oracle-control',[sha_exe,wrong],expected=1)
        if rejected.stdout or b'sha256-probe:fixture=0:actual-digest'not in rejected.stderr:raise ValueError('actual wrong SHA oracle control did not reject')
        probe=compile_c('compile-import-probe',snapshot/'package_import_probe.c',out/'import-probe.obj')
        osobj=compile_c('compile-import-os',snapshot/'package_admission_os.c',out/'import-os.obj',['-DSENGOO_PACKAGE_INSTRUMENTED'])
        tarobj=compile_c('compile-import-tar-os',snapshot/'tar_memory.c',out/'import-tar-os.obj',['-DSENGOO_TAR_INSTRUMENTED'])
        memory=[built_rounds[0][name+'-sdk']for name in ['tar_stream_memory','gzip_memory','gzip_stream_memory']]
        import_exe=out/'import-probe.exe';c_link(run,read,report,'link-import-probe',clang,[probe,osobj,tarobj,*memory,*[built_rounds[0][name]for name in stems],stack],import_exe,cenv)
        run('generate-imports',[python,'-B','-X','utf8',snapshot/'package_import_cases.py','--output',out/'imports','--large'])
        imports=strict_json(read(out/'imports/manifest.json'))
        if len(imports['cases'])!=46:raise ValueError('admission transaction fixture IDs incomplete')
        for row in imports['cases']:read(out/'imports'/row['archive'],row['archive_sha256']);read(out/'imports'/row['metadata'],row['metadata_sha256'])
        snapshot_obj=compile_c('compile-immutable-file-snapshot-probe',snapshot/'package_snapshot_probe.c',out/'snapshot-probe.obj');snapshot_exe=out/rounds[0]['round']/'snapshot-probe.exe'
        c_link(run,read,report,'link-immutable-file-snapshot-probe',clang,[snapshot_obj,built_rounds[0]['sdk-import']],snapshot_exe,cenv)
        observed=run('actual-immutable-file-snapshot',[snapshot_exe,out/'imports'/imports['cases'][0]['archive']]);identity=strict_json(observed.stdout)
        if observed.stderr or identity.get('write_denied')!=32 or identity.get('delete_denied')!=32 or identity.get('identity_stable')is not True or identity['handles_before']!=identity['handles_after']:raise ValueError('actual compressed FileSnapshot lock/identity control differs')
        report['immutable_input_snapshot']=identity
        run('actual-admitted-transactions',[python,'-B','-X','utf8',snapshot/'package_import_differential.py','--subject',import_exe,'--oracle',oracle,'--cases',out/'imports','--output',out/'import-results'])
        transactions=strict_json(read(out/'import-results/evidence.json'))
        if transactions['result']!='passed'or len(transactions['cases'])!=46 or transactions['differences']:raise ValueError('admitted transaction/actual content checksum differs')
        report['admitted_transactions']=transactions['cases']
        for number,built in enumerate(built_rounds):
            directory=out/('cli-'+str(number));directory.mkdir();row=imports['cases'][0]
            command=[built['image'],'import-archive',out/'imports'/row['archive'],directory,'private','final','demo','1.0.0',row['archive_sha256'],out/'imports'/row['metadata']]
            run('cli-import-'+str(number),command,empty=True);identity=run('cli-manifest-'+str(number),[built['image'],'manifest-info',directory/'final/Sengoo.toml'])
            if identity.stdout!=b'demo\t1.0.0\n'or identity.stderr:raise ValueError('actual SG CLI identity bytes differ')
            run('cli-existing-'+str(number),command,expected=78,empty=True)
            reference=run('cli-content-reference-'+str(number),[oracle,'--content-checksum',directory/'final']);checksum=strict_json(reference.stdout)
            if checksum.get('ok')is not True or(directory/'final/.sgpm-content.sha256').read_bytes()!=(checksum['sha256']+'\n').encode():raise ValueError('actual pure SG-linked product content checksum differs')
            for command in ['build','check','run','test','fmt','doc','tree','metadata','update','publish','new','init','cache','registry','unknown']:
                run('unsupported-'+str(number)+'-'+command,[built['image'],command],expected=69,empty=True)
        report['operation_matrix']={'sgpm-preview import-archive':'actual SG gzip/tar/full manifest/identity/checksums/markers/atomic publication','sgpm-preview manifest-info':'actual canonical full manifest validation, identity TSV only','existing sgpm operations':'explicit unsupported exit69; not replaced or delegated'}
        # Bind every nested command receipt and output, not just aggregate exit.
        for folder in ['manifest-results','metadata-results','import-results']:
            nested=strict_json(read(out/folder/'evidence.json'))
            for step in nested['steps']:
                if step['exit']!=0:raise ValueError('nested actual command failed')
                for suffix in ['stdout','stderr']:read(out/folder/(step['name']+'.'+suffix),step[suffix+'_sha256'])
        for path in out.rglob('*'):
            if path.is_file()and path.name!='evidence.json'and str(path)not in report['sources']and path.suffix not in ['.toml','.json']:
                report['artifacts'][str(path)]=sha(path)
        report['result']='passed'
    except BaseException as error:report['result']='failed';report['error']=repr(error);raise
    finally:
        report['changed_sources']=[name for name,pin in report['sources'].items()if not(ROOT/name).is_file()or sha(ROOT/name)!=pin]
        report['changed_external_sources']=[name for name,pin in report['external_sources'].items()if not Path(name).is_file()or sha(Path(name))!=pin]
        report['changed_tools']=[name for name,entry in report['tools'].items()if not Path(entry['path']).is_file()or sha(Path(entry['path']))!=entry['sha256']]
        report['revision_end']=revision();report['git_status_end']=git_status();report['completed_at']=dt.datetime.now(dt.timezone.utc).isoformat();report['initial_cached_reads']={str(path):1 for path in cache}
        if any(report[name]for name in ['changed_sources','changed_external_sources','changed_tools']):report['result']='failed'
        save()
        if report['result']!='passed'and not sys.exc_info()[0]:raise ValueError('terminal source/tool guard failed')
if __name__=='__main__':main()
