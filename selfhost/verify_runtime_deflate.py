"""Source-bound qualification of SG raw DEFLATE; zlib and C are independent test consumers."""
from __future__ import annotations
import argparse,collections,ctypes,datetime as dt,hashlib,json,os,platform,re,struct,subprocess,sys,time,zlib
from pathlib import Path
from linker_evidence import copy_linker
from verify_runtime_text import coff_image,dependencies,digest,exact_json,git_status,revision,sha,strict_json
from runtime_evidence import bind_memory_providers,c_link,resource_receipt

ROOT=Path(__file__).resolve().parents[1]
CORPUS_SHA='6f6ae1cca20bf43074d80892afdcf6db6e813f22815bb9878fa19b91f1f59728'

def metadata_proof(before,after):
    old,new=coff_image(before),coff_image(after)
    removed={name for name,value in old['sections'].items() if name in {'.data','.bss'} and not value['data'] and not value['relocations']}
    wanted={name:value for name,value in old['sections'].items() if name not in removed}
    if not set(wanted)<={'.text','.data','.rdata'} or new['sections']!=wanted:raise ValueError('C leaf code/data/flags/relocations changed')
    olds={row[:5] for row in old['symbols']};news={row[:5] for row in new['symbols']}
    referenced={target for section in wanted.values() for _,_,target in section['relocations']}
    required={row[:5] for row in old['symbols'] if row[4]==2}
    if not required|referenced<=news or not news<=olds or any(row[5] or row[4] not in [2,3] for row in new['symbols']):raise ValueError('C leaf symbol/address changes')
    if any(row[:5] not in news and row[0] not in {*old['sections'],'.file','@feat.00'} for row in old['symbols']):raise ValueError('C normalization removed a non-metadata symbol')
    return {'raw_sha256':digest(before),'normalized_sha256':digest(after),'raw_timestamp':old['timestamp'],'normalized_timestamp':new['timestamp'],
            'removed_empty_sections':sorted(removed),'section_bytes_flags_relocations_equal':True,'all_external_and_referenced_symbol_addresses_equal':True,
            'sections':{name:{'sha256':digest(value['data']),'flags':value['flags'],'relocations':value['relocations']} for name,value in wanted.items()}}

def corpus_oracle(raw,manifest):
    if digest(raw)!=CORPUS_SHA or raw[:8]!=b'SGDFL1\0\0':raise ValueError('corpus bytes/header differ')
    count=struct.unpack_from('<Q',raw,8)[0]
    if count!=5290 or count!=manifest['records'] or len(manifest['fixtures'])!=count:raise ValueError('corpus record count differs')
    at=16;groups=collections.Counter();results=collections.Counter();guards=[];names=set()
    differences={row['name']:row for row in manifest['oracle_differences']}
    if set(differences)!={'empty-distance-hdist-31','empty-distance-hdist-32'}:raise ValueError('unexpected oracle difference set')
    for index,row in enumerate(manifest['fixtures']):
        length,capacity,expected,plain_length=struct.unpack_from('<IIqI',raw,at);at+=20
        source=raw[at:at+length];at+=length;plain=raw[at:at+plain_length];at+=plain_length
        if at>len(raw) or row['id']!=index or row['name'] in names or (length,capacity,expected,plain_length)!=(row['input_length'],row['capacity'],row['expected'],row['output_length']) or digest(source)!=row['input_sha256']:raise ValueError('fixture ID/bytes/oracle metadata differ')
        names.add(row['name']);groups[row['group']]+=1
        try:
            independent=zlib.decompressobj(-15);actual=independent.decompress(source)+independent.flush()
            valid=independent.eof and not independent.unused_data
        except zlib.error:actual=b'';valid=False
        if row['name'] in differences:
            if valid or expected!=0 or plain:raise ValueError('documented HDIST difference changed')
        elif expected>=0:
            if not valid or expected!=len(plain) or actual!=plain or capacity<expected:raise ValueError('positive zlib bytes differ')
        elif expected==-2:
            if not valid or actual!=plain or not 0<=capacity<len(plain):raise ValueError('capacity oracle is not a valid independently decoded source')
        elif expected!=-1 or valid:raise ValueError('malformed/trailing/truncated oracle differs')
        weight=1
        if length<=4096 and capacity<=4096 and (index<16 or length<=512):guards.append(index);weight+=2
        results['successes' if expected>=0 else 'malformed' if expected==-1 else 'capacity_failures']+=weight
    if at!=len(raw) or dict(groups)!=manifest['groups'] or guards!=manifest['guard_fixture_ids']:raise ValueError('corpus/group/guard inventory differs')
    for index,row in enumerate(manifest['fixtures']):
        if row['group'].startswith('malformed') and index not in guards:raise ValueError('named malformed fixture lacks head/tail hardware guards')
    return {'records':count,'calls':count+len(guards)*2,**results,'invalid_limits':8,'guard_cases':len(guards)*2,'guard_fixture_ids':guards,
            'groups':dict(groups),'independent_complete_parse_zlib_checked':True,'oracle_differences':manifest['oracle_differences']}

def main():
    parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--text-evidence',type=Path,required=True);parser.add_argument('--output',type=Path,required=True);args=parser.parse_args()
    out=args.output.resolve()
    if not out.is_relative_to(ROOT/'target'):raise ValueError('output must stay inside target/')
    out.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'stage':'sg-runtime-raw-deflate','result':'running','source_revision':revision(),'git_status_start':git_status(),'host':platform.platform(),
            'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'sources':{},'external_sources':{},'tools':{},'steps':[],'cases':[],
            'general_raw_decoder':True,'gzip_complete':False,'package_compatibility_complete':False,'final_sdk_complete':False,
            'limits':['independent zlib/Python/C verification; decompression policy is SG only','explicit development C leaf metadata compatibility and actual NTDLL stack provider inherited by exact qualification','no gzip/tar/package replacement, final SDK packaging, concurrency/performance or self-build claim']}
    cache={};checkout={}
    def save():(out/'evidence.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
    def read(path,expected=None,tool=False):
        path=Path(path).resolve()
        if path not in cache:cache[path]=path.read_bytes()
        raw=cache[path];observed=digest(raw)
        if expected is not None and observed!=expected:raise ValueError('consumed identity differs: '+str(path))
        if not tool:
            if path.is_relative_to(ROOT):
                report['sources'][str(path.relative_to(ROOT))]=observed
                if not path.is_relative_to(ROOT/'target'):checkout[path]=observed
            else:report['external_sources'][str(path)]=observed
        return raw
    def tool(name,path,expected=None):
        path=Path(path).resolve();raw=read(path,expected,True);report['tools'][name]={'path':str(path),'sha256':digest(raw)};return path
    def snap(path,saved,expected=None):
        raw=read(path,expected);saved.parent.mkdir(parents=True,exist_ok=True);saved.write_bytes(raw);read(saved,digest(raw));return saved
    clean={key:value for key,value in os.environ.items() if not key.upper().startswith(('SENGOO_','SGPLATFORM_','SDL2_','PYTHON','RUSTC','RUSTDOC','CARGO_')) and key.upper() not in ['RUSTFLAGS','RUSTUP_TOOLCHAIN']}
    pure=dict(clean,PATH='')
    def run(name,command,env=pure,expected=0,empty=False):
        command=list(map(str,command));start=time.monotonic();done=subprocess.run(command,cwd=out,env=env,capture_output=True,timeout=900)
        a,b=out/(name+'.stdout.log'),out/(name+'.stderr.log');a.write_bytes(done.stdout);b.write_bytes(done.stderr)
        report['steps'].append({'name':name,'command':command,'exit_code':done.returncode,'expected_exit_code':expected,'seconds':time.monotonic()-start,'empty_path':env.get('PATH')=='','stdout_sha256':sha(a),'stderr_sha256':sha(b)})
        save();print(name,done.returncode,flush=True)
        if done.returncode!=expected or empty and (done.stdout or done.stderr):raise AssertionError(name+' status/output differs')
        return done
    save()
    try:
        for name in ['selfhost/verify_runtime_deflate.py','selfhost/verify_runtime_text.py','selfhost/linker_evidence.py']:read(ROOT/name)
        python=tool('runner-python',sys.executable)
        report['python']={'version':sys.version,'zlib_build':zlib.ZLIB_VERSION,'zlib_runtime':zlib.ZLIB_RUNTIME_VERSION}
        windows=ctypes.WinDLL('kernel32.dll',use_last_error=True);windows.GetModuleFileNameW.argtypes=[ctypes.c_void_p,ctypes.c_wchar_p,ctypes.c_uint32];windows.GetModuleFileNameW.restype=ctypes.c_uint32
        actual=ctypes.create_unicode_buffer(32768);size=windows.GetModuleFileNameW(sys.dllhandle,actual,len(actual))
        if not 0<size<len(actual):raise ValueError('cannot resolve loaded Python runtime')
        loaded=tool('loaded-python-runtime',actual.value)
        if getattr(zlib,'__file__',None):zlib_binary=tool('zlib-implementation',zlib.__file__)
        elif zlib.__spec__.origin=='built-in' and 'zlib' in sys.builtin_module_names:zlib_binary=tool('zlib-implementation',loaded)
        else:raise ValueError('unqualified zlib implementation origin')
        report['zlib_implementation']={'origin':zlib.__spec__.origin,'path':str(zlib_binary),'sha256':report['tools']['zlib-implementation']['sha256'],'build_version':zlib.ZLIB_VERSION,'runtime_version':zlib.ZLIB_RUNTIME_VERSION}
        evidence=args.text_evidence.resolve()
        if not evidence.is_relative_to(ROOT/'target'):raise ValueError('text qualification is outside target/')
        prior=strict_json(read(evidence))
        if prior.get('schema')!=1 or prior.get('stage')!='sg-runtime-text-utf8' or prior.get('result')!='passed' or any(prior.get(key)!=[] for key in ['changed_sources','changed_external_sources','changed_tools','checkout_changes']):raise ValueError('UTF8 qualification did not pass unchanged-input guards')
        for name,expected in prior['sources'].items():snap(ROOT/name,out/'qualified-text-inputs'/name,expected)
        for name,expected in prior['external_sources'].items():read(Path(name),expected)
        for name,identity in prior['tools'].items():tool('text-producer-'+name,identity['path'],identity['sha256'])
        for name in ['native','formatter']:
            identity=prior['tools'][name];source=tool(name+'-qualified',identity['path'],identity['sha256']);binary=out/(name+'.exe');binary.write_bytes(read(source,identity['sha256'],True));tool(name,binary,identity['sha256'])
        native,formatter=out/'native.exe',out/'formatter.exe'
        linker_pin=prior['linker_qualification']['evidence'];linker_path=Path(linker_pin['path']).resolve()
        if not linker_path.is_relative_to(ROOT/'target') or prior['sources'].get(str(linker_path.relative_to(ROOT)))!=linker_pin['sha256']:raise ValueError('declared historical linker report pin is not bound by the text manifest')
        read(linker_path,linker_pin['sha256'])
        linker,linked=copy_linker(ROOT,linker_path,out,report)
        if linked['binary_sha256']!=prior['tools']['sg-linker']['sha256']:raise ValueError('resolved historical linker binary differs from the text qualification')
        for name,identity in list(report['tools'].items()):tool(name,identity['path'],identity['sha256'])
        for name,expected in list(report['sources'].items()):read(ROOT/name,expected)
        clang=tool('clang',prior['tools']['clang']['path'],prior['tools']['clang']['sha256'])
        extra={name:tool(name,prior['tools'][prior_name]['path'],prior['tools'][prior_name]['sha256']) for name,prior_name in [('objcopy','llvm-objcopy-c-sdk-metadata'),('objdump','llvm-objdump-c-sdk-leaf-check'),('readobj','llvm-readobj-independent-pe'),('lld','clang-probe-linker')]}
        kernel=tool('sdk-kernel32',linked['sdk']['path'],linked['sdk']['sha256'])
        stack=snap(evidence.parent/'ntdll-chkstk.lib',out/'ntdll-chkstk.lib',prior['stack_probe_provider']['import_library_sha256'])
        report['qualification']={'path':str(evidence),'sha256':digest(read(evidence)),'revision_start':prior['source_revision'],'revision_end':prior['source_revision_end'],'native_binary':report['tools']['native'],'formatter_preview':report['tools']['formatter'],'actual_ntdll':prior['stack_probe_provider'],'scope':'all prior consumed sources/external headers/tools guarded; actual emitter/preview binaries and stack SDK copied by exact digest; historical SG linker chain independently resolved'}
        cenv=dict(clean,PATH=str(clang.parent)+os.pathsep+clean.get('PATH',''),SOURCE_DATE_EPOCH='0')
        implementation=out/'implementation'
        names=['selfhost/runtime/deflate.sg','selfhost/runtime/deflate_stream.sg','selfhost/runtime/deflate_stream_memory.c','selfhost/runtime/deflate_storage.c','selfhost/runtime/deflate_memory.c','selfhost/tests/deflate_cases.py','selfhost/tests/deflate_probe.c','selfhost/tests/deflate_prefix_probe.c','selfhost/tests/deflate_resource_probe.c','selfhost/tests/deflate_prefix_resource_probe.c','selfhost/tests/deflate_resource_probe.h','selfhost/tests/deflate_fixture.c','selfhost/tests/deflate_smoke.sg','selfhost/runtime_evidence.py']
        for name in names:snap(ROOT/name,implementation/name)
        for path in (ROOT/'openspec/changes/add-selfhost-deflate-core').rglob('*.md'):
            if path.name!='tasks.md':snap(path,implementation/path.relative_to(ROOT))
        report['ledger_exclusion']='child tasks.md and unrelated concurrent edits are excluded from consumed inputs; Git dirty state is disclosed'
        for name in ['selfhost/runtime/deflate.sg','selfhost/runtime/deflate_stream.sg','selfhost/tests/deflate_smoke.sg']:run('format-'+Path(name).stem,[formatter,'--check',implementation/name],empty=True)
        report['actual_os_memory_providers']=bind_memory_providers(read)
        corpus_rows=[]
        for filename in ['corpus.bin','corpus-repeat.bin']:
            path=out/filename;generated=run('generate-'+filename,[python,'-B','-X','utf8',implementation/'selfhost/tests/deflate_cases.py','--output',path])
            summary=strict_json(generated.stdout);manifest=strict_json(read(path.with_suffix('.bin.json')))
            if generated.stderr or summary['records']!=5290 or summary['sha256']!=CORPUS_SHA or manifest['source_sha256']!=sha(implementation/'selfhost/tests/deflate_cases.py') or manifest['zlib_implementation']!=report['zlib_implementation'] or manifest['python_executable_sha256']!=report['tools']['runner-python']['sha256']:raise ValueError('generator source/tool/status differs')
            oracle=corpus_oracle(read(path,CORPUS_SHA),manifest);corpus_rows.append(oracle)
        if corpus_rows[0]!=corpus_rows[1]:raise AssertionError('two independently generated oracle inventories differ')
        report['corpus']=dict(corpus_rows[0],sha256=CORPUS_SHA,identical_regeneration=True)
        large=out/'large';run('generate-large',[python,'-B','-X','utf8',implementation/'selfhost/tests/deflate_cases.py','--large-dir',large])
        large_manifest=strict_json(read(large/'large.json'))
        if large_manifest['source_sha256']!=sha(implementation/'selfhost/tests/deflate_cases.py') or large_manifest['zlib_implementation']!=report['zlib_implementation']:raise ValueError('large generator source/tool differs')
        for row in large_manifest['files']:
            data=read(large/row['name'],row['sha256']);decoded=zlib.decompress(data,-15)
            if len(data)!=row['length'] or len(decoded)!=row['decoded_bytes'] or digest(decoded)!=row['decoded_sha256']:raise ValueError('actual large zlib oracle differs')
        report['large_inputs']=large_manifest
        def compile_c(label,source,target,sdk=False,production=False):
            flags=['-std=c11','-Wall','-Wextra','-Werror','-Werror=date-time','-g0','-fno-ident','-O2' if sdk else '-O0']
            if sdk:flags+=['-fno-addrsig','-fomit-frame-pointer','-fno-asynchronous-unwind-tables','-fno-unwind-tables','-fno-vectorize','-fno-slp-vectorize']
            if production:flags+=['-DSENGOO_DEFLATE_PRODUCTION_MEMORY']
            before,after=target.with_suffix('.before.d'),target.with_suffix('.d')
            run(label+'-headers',[clang,*flags,'-M','-MT','runtime_text_dependency','-MF',before,source],cenv)
            headers={path:digest(read(path)) for path in dependencies(before)}
            rawtarget=target.with_suffix('.raw.obj') if sdk else target
            run(label,[clang,*flags,'-MD','-MT','runtime_text_dependency','-MF',after,'-c',source,'-o',rawtarget],cenv)
            if dependencies(after)!=set(headers) or any(sha(path)!=expected for path,expected in headers.items()):raise ValueError('actual C header inputs changed')
            report.setdefault('c_header_closures',{})[label]={str(path):expected for path,expected in sorted(headers.items())}
            if sdk:
                raw=rawtarget.read_bytes();parsed=coff_image(raw);removed=[name for name,value in parsed['sections'].items() if name in ['.data','.bss'] and not value['data'] and not value['relocations']]
                disassembly=run(label+'-leaf',[extra['objdump'],'-d','--no-show-raw-insn',rawtarget],cenv).stdout.decode()
                if re.search(r'%(?:rsp|rbp)|\b(?:call\w*|push\w*|pop\w*|enter|leave)\b',disassembly.split('Disassembly of section .text:',1)[1]):raise ValueError('C memory/verification fixture is not a stack-free leaf')
                run(label+'-metadata',[extra['objcopy'],'--strip-unneeded','--strip-symbol=.file',*['--remove-section='+name for name in removed],rawtarget,target],cenv)
                normalized=bytearray(target.read_bytes());normalized[4:8]=bytes(4);target.write_bytes(normalized)
                proof=metadata_proof(raw,bytes(normalized));proof['timestamp_operation']='COFF header bytes4..7 explicitly zeroed; raw object retained'
                text=next(value for name,value in coff_image(normalized)['sections'].items() if name=='.text')
                changed=bytearray(normalized);text_at=struct.unpack_from('<I',changed,40)[0];changed[text_at]^=1
                try:metadata_proof(raw,bytes(changed))
                except ValueError:pass
                else:raise AssertionError('metadata oracle accepted a changed instruction')
                report.setdefault('c_metadata_proofs',{})[label]=proof
        first={};objects=[]
        for number,roundname in enumerate(['first fresh 世界 &','second fresh 世界 &']):
            directory=out/roundname;directory.mkdir();built={}
            for stem,original in [('deflate','selfhost/runtime/deflate.sg'),('deflate_stream','selfhost/runtime/deflate_stream.sg'),('deflate_smoke','selfhost/tests/deflate_smoke.sg')]:
                source=snap(implementation/original,directory/(stem+'.sg'));obj=directory/(stem+'.obj')
                done=run(str(number)+'-emit-'+stem,[native,source,obj,'--wrap']);exact_json(done.stdout,{'schema_version':1,'ok':True,'phase':'object'})
                if done.stderr:raise AssertionError('emitter stderr')
                built[stem]=obj
            for stem,original in [('deflate_memory','selfhost/runtime/deflate_memory.c'),('deflate_stream_memory','selfhost/runtime/deflate_stream_memory.c'),('deflate_storage','selfhost/runtime/deflate_storage.c'),('deflate_fixture','selfhost/tests/deflate_fixture.c')]:
                source=snap(implementation/original,directory/(stem+'.c'));obj=directory/(stem+'.obj');compile_c(str(number)+'-'+stem,source,obj,True);built[stem]=obj
            image=directory/'deflate.exe';run(str(number)+'-sg-link',[linker,'link',image,'main',*built.values(),kernel,stack])
            inspected=run(str(number)+'-inspect-imports',[extra['readobj'],'--coff-imports',image],cenv)
            if b'ntdll.dll' not in inspected.stdout.lower() or any(name not in inspected.stdout for name in [b'__chkstk',b'VirtualAlloc',b'VirtualFree']):raise AssertionError('actual memory/stack SDK import absent')
            if number==0:
                missing=directory/'missing-core.exe';rejected=run('missing-core-link',[linker,'link',missing,'main',*[p for n,p in built.items()if n!='deflate_stream'],kernel,stack],expected=65)
                if rejected.stdout.strip()!=b'7204'or rejected.stderr or missing.exists():raise AssertionError('missing shared core was not unresolved/unpublished')
                report['missing_core_negative']={'exit':65,'diagnostic':7204,'unresolved_shared_core':True,'image_published':False}
            run(str(number)+'-cold-execute',[image],expected=42,empty=True)
            identities={name:sha(path) for name,path in built.items()};identities['image']=sha(image)
            if number==0:first=identities
            elif identities!=first:raise AssertionError('two fresh untouched SG / normalized C / SG PE artifacts differ')
            report['cases'].append({'round':roundname,'sha256':identities,'cold_exit':42,'stdout_empty':True,'stderr_empty':True});objects.append(built)
        record_expected={key:value for key,value in report['corpus'].items() if key in ['records','calls','successes','malformed','capacity_failures','invalid_limits','guard_cases']}
        report['probe_results']={}
        prefix_obj,prefix_binary=out/'prefix-probe.obj',out/'prefix-probe.exe'
        compile_c('compile-prefix-probe',implementation/'selfhost/tests/deflate_prefix_resource_probe.c',prefix_obj)
        c_link(run,read,report,'link-prefix-probe',clang,[prefix_obj,objects[0]['deflate'],objects[0]['deflate_stream'],stack],prefix_binary,cenv)
        prefix=run('prefix-counts',[prefix_binary]);exact_json(prefix.stdout,{'schema':1,'ok':True,'prefix_cases':4,'legacy_exact_cases':3,'invalid_limits':8,'null_empty_output':True,'no_trailing_reads':True,'decoded_bytes_equal':True})
        report.setdefault('state_resource_receipts',{})['prefix']=resource_receipt(prefix.stderr,7)
        report['probe_results']['prefix-counts']=strict_json(prefix.stdout)
        oom=run('prefix-oom',[prefix_binary,'--oom-protocol']);exact_json(oom.stdout,{'schema':1,'ok':True,'oom':True,'no_raw_input_output_access':True});report['state_resource_receipts']['prefix-oom']=resource_receipt(oom.stderr,0,True)
        for mode in ['instrumented','production']:
            obj=out/('probe-'+mode+'.obj');binary=out/('probe-'+mode+'.exe');compile_c('compile-probe-'+mode,implementation/'selfhost/tests/deflate_resource_probe.c',obj,production=mode=='production')
            c_link(run,read,report,'link-probe-'+mode,clang,[obj,objects[0]['deflate'],objects[0]['deflate_stream'],*([objects[0]['deflate_stream_memory']]if mode=='production'else []),stack],binary,cenv)
            done=run(mode+'-corpus',[binary,out/'corpus.bin']);expected={'schema':1,'ok':True,**record_expected,'input_unchanged':True,'instrumented':mode=='instrumented','bounded_sequential_output':True if mode=='instrumented' else None,'known_failure_prefix_checked':mode=='instrumented','input_reads':4142320 if mode=='instrumented' else None,'history_reads':4802169 if mode=='instrumented' else None,'writes':8792668 if mode=='instrumented' else None}
            report['state_resource_receipts'][mode+'-corpus']=resource_receipt(done.stderr,10006)
            report['probe_results'][mode+'-corpus']=exact_json(done.stdout,expected)
            done=run(mode+'-64MiB',[binary,'--large',large]);expected={'schema':1,'ok':True,'large_cases':3,'successes':2,'capacity_failures':1,'input_limit':67108864,'output_limit':67108864,'readonly_input_tailguard':True,'output_tailguard':True,'instrumented':mode=='instrumented','complete_sequential_input_output':True if mode=='instrumented' else None,'known_failure_prefix_checked':mode=='instrumented','input_reads':67239330 if mode=='instrumented' else None,'history_reads':134217500 if mode=='instrumented' else None,'writes':201321248 if mode=='instrumented' else None}
            report['state_resource_receipts'][mode+'-64MiB']=resource_receipt(done.stderr,3)
            report['probe_results'][mode+'-64MiB']=exact_json(done.stdout,expected)
            oom=run(mode+'-oom',[binary,'--oom-protocol']);exact_json(oom.stdout,{'schema':1,'ok':True,'oom':True,'no_raw_input_output_access':True});report['state_resource_receipts'][mode+'-oom']=resource_receipt(oom.stderr,0,True)
        bad=bytearray(read(out/'corpus.bin'));struct.pack_into('<q',bad,24,-1);wrong=out/'wrong-empty-oracle.bin';wrong.write_bytes(bad)
        failed=run('wrong-result-oracle',[out/'probe-instrumented.exe',wrong],expected=1)
        if failed.stdout or b'reason=result-oracle expected=-1 actual=0' not in failed.stderr:raise AssertionError('wrong expected result did not reach algorithm oracle')
        report['wrong_oracle_control']={'actual_exit':1,'reason':'result-oracle','expected':-1,'actual':0,'only_first_expected_result_modified':True}
        for key,value in [('records',True),('instrumented',False),('known_failure_prefix_checked',False),('writes',None)]:
            expected=report['probe_results']['instrumented-corpus'];changed=dict(expected,**{key:value})
            try:exact_json(json.dumps(changed).encode(),expected)
            except ValueError:pass
            else:raise AssertionError('JSON exact-mode oracle accepted '+key)
        report['artifacts']={str(path.relative_to(out)):sha(path) for path in out.rglob('*') if path.is_file() and path.suffix in ['.obj','.exe','.lib']}
        report.update(result='passed',deterministic_sg_and_normalized_c_objects_and_images=True,raw_c_reproducibility_claimed=False,single_shared_deflate_core=True,one_shot_state_owner='SG fixed45056 request/dispose/free; OS memory-only primitive',deployment=['deflate.sg','deflate_stream.sg','deflate_stream_memory.c','deflate_storage.c','Kernel32','explicit NTDLL __chkstk'])
    except BaseException as error:report.update(result='failed',error=type(error).__name__+': '+str(error))
    finally:
        try:
            report['changed_sources']=[name for name,expected in report['sources'].items() if not (ROOT/name).is_file() or sha(ROOT/name)!=expected]
            report['changed_external_sources']=[name for name,expected in report['external_sources'].items() if not Path(name).is_file() or sha(Path(name))!=expected]
            report['changed_tools']=[name for name,identity in report['tools'].items() if not Path(identity['path']).is_file() or sha(Path(identity['path']))!=identity['sha256']]
            report['checkout_changes']=[str(path.relative_to(ROOT)) for path,expected in checkout.items() if not path.is_file() or sha(path)!=expected]
            report['initial_cached_reads']={str(path):1 for path in cache};report['read_policy']='cached first consumption and final independent SHA reread; historical linker helper separately validates full chain'
            report['source_revision_end']=revision();report['git_status_end']=git_status()
            if any(report[key] for key in ['changed_sources','changed_external_sources','changed_tools','checkout_changes']):report.update(result='failed',source_guard_failure='consumed closure changed')
        except BaseException as error:report.update(result='failed',finally_error=repr(error))
        report['completed_at']=dt.datetime.now(dt.timezone.utc).isoformat();save()
    print(json.dumps({key:report.get(key) for key in ['result','error','changed_sources','changed_external_sources','changed_tools','checkout_changes']}));return 0 if report['result']=='passed' else 1

if __name__=='__main__':raise SystemExit(main())
