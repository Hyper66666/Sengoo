"""Qualify the explicit-context SG registry and RawVec without replacing global runtime providers."""
from __future__ import annotations
import argparse,datetime as dt,json,os,platform,re,struct,subprocess,sys,time
from pathlib import Path
from verify_runtime_text import coff_image,dependencies,digest,exact_json,git_status,revision,sha,strict_json
from verify_runtime_deflate import metadata_proof
from runtime_evidence import bind_memory_providers,c_link
from linker_evidence import copy_linker

ROOT=Path(__file__).resolve().parents[1]
OWNED=['selfhost/runtime/collections_registry.sg','selfhost/runtime/collections_vec.sg','selfhost/runtime/collections_memory.c',
       'selfhost/tests/collections_runtime_probe.c','selfhost/tests/collections_runtime_smoke.sg','selfhost/tests/collections_runtime_fixture.c']

def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--text-evidence',type=Path,required=True);parser.add_argument('--output',type=Path,required=True)
    args=parser.parse_args();out=args.output.resolve()
    if not out.is_relative_to(ROOT/'target'):raise ValueError('output must be fresh target directory')
    out.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'stage':'sg-explicit-context-collections-runtime','result':'running','revision_start':revision(),'git_status_start':git_status(),
            'host':platform.platform(),'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'sources':{},'external_sources':{},'tools':{},'artifacts':{},'steps':[],
            'namespace2_admitted':False,'global_registry_replaced':False,'self_build_complete':False,
            'remaining':['shared process-global registry and other handle families','provider/catalog and typed source-lowering integration','installed SDK packaging and self-build','whole-program memory/performance qualification']}
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
    def tool(name,identity):
        path=Path(identity['path']).resolve();read(path,identity['sha256']);report['tools'][name]={'path':str(path),'sha256':identity['sha256']};return path
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
        parent=strict_json(read(args.text_evidence));
        if parent.get('schema')!=1 or parent.get('result')!='passed'or parent.get('stage')!='sg-runtime-text-utf8'or any(v!=[]for k,v in parent.items()if k.startswith('changed_')or k=='checkout_changes'):raise ValueError('text qualification is not passed/unchanged')
        for group in ['sources','external_sources']:
            for name,pin in parent[group].items():read(ROOT/name if group=='sources'else Path(name),pin)
        for name,identity in parent['tools'].items():tool('producer-'+name,identity)
        native=tool('native',parent['tools']['native']);formatter=tool('formatter',parent['tools']['formatter']);clang=tool('clang',parent['tools']['clang']);lld=tool('lld',parent['tools']['clang-probe-linker']);kernel=tool('kernel32',parent['tools']['sdk-kernel32']);objcopy=tool('objcopy',parent['tools']['llvm-objcopy-c-sdk-metadata']);objdump=tool('objdump',parent['tools']['llvm-objdump-c-sdk-leaf-check']);readobj=tool('readobj',parent['tools']['llvm-readobj-independent-pe'])
        for name in ['native','formatter']:
            if parent['artifacts'].get(Path(parent['tools'][name]['path']).name)!=parent['tools'][name]['sha256']:raise ValueError('qualified compiler artifact relationship differs')
        pin=parent['linker_qualification']['evidence'];path=Path(pin['path']).resolve()
        if parent['sources'].get(str(path.relative_to(ROOT)))!=pin['sha256']:raise ValueError('declared linker parent not pinned')
        read(path,pin['sha256']);linker,branch=copy_linker(ROOT,path,out,report)
        if branch['binary_sha256']!=parent['tools']['sg-linker']['sha256']:raise ValueError('qualified linker binary differs')
        for name,identity in list(report['tools'].items()):tool(name,identity)
        for name,pin in list(report['sources'].items()):read(ROOT/name,pin)
        stack_origin=args.text_evidence.resolve().parent/'ntdll-chkstk.lib';stack_pin=parent['stack_probe_provider']['import_library_sha256']
        if parent['sources'].get(str(stack_origin.relative_to(ROOT)))!=stack_pin:raise ValueError('stack import library not source-pinned')
        stack=out/'ntdll-chkstk.lib';stack.write_bytes(read(stack_origin,stack_pin));read(stack,stack_pin)
        tool('runner-python',{'path':sys.executable,'sha256':sha(Path(sys.executable))});report['python']=sys.version
        report['os_memory_providers']=bind_memory_providers(read)
        helpers=['selfhost/verify_runtime_collections.py','selfhost/verify_runtime_text.py','selfhost/verify_runtime_deflate.py','selfhost/runtime_evidence.py','selfhost/linker_evidence.py']
        for name in helpers:read(ROOT/name)
        impl=out/'implementation'
        for name in OWNED:snap(ROOT/name,impl/name)
        change=ROOT/'openspec/changes/add-selfhost-collections-runtime-core'
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
        # Formatter remains an explicit qualified preview. No Rust formatter is modified.
        for name in ['collections_registry','collections_vec','collections_runtime_smoke']:
            path=impl/('selfhost/tests/'+name+'.sg'if name.endswith('smoke')else'selfhost/runtime/'+name+'.sg')
            preview=run(name+'-format-preview',[formatter,path]);formatted=out/(name+'-formatted.sg');formatted.write_bytes(preview.stdout)
            if preview.stdout!=read(path):raise ValueError('owned SG source is not formatted '+name)
            run(name+'-format-idempotent',[formatter,formatted,'--check'])
        provider=[]
        for name,flags in [('runtime',['-Dsengoo_opaque_handle_new=reference_opaque_handle_new']),('runtime_string',[]),('runtime_collections',['-D_aligned_malloc=ref_alloc','-D_aligned_free=ref_free','-Dcalloc=ref_calloc','-Dfree=ref_object_free'])]:
            provider.append(compile_c(name+'-reference-compile',impl/'tools/stdlib'/(name+'.c'),out/(name+'.obj'),flags))
        test_memory=compile_c('instrumented-memory',impl/'selfhost/runtime/collections_memory.c',out/'instrumented-memory.obj',['-DSENGOO_COLLECTIONS_TEST_METADATA','-DSENGOO_COLLECTIONS_TEST_ALLOCATOR'])
        test=compile_c('differential-probe',impl/'selfhost/tests/collections_runtime_probe.c',out/'probe.obj')
        resource_memory=compile_c('real-memory-primitives',impl/'selfhost/runtime/collections_memory.c',out/'real-memory-primitives.obj',['-DSENGOO_COLLECTIONS_TEST_ALLOCATOR'])
        resource_probe=compile_c('real-memory-resource-probe',impl/'selfhost/tests/collections_runtime_fixture.c',out/'resource-probe.obj',['-DSENGOO_COLLECTIONS_RESOURCE_PROBE'])
        fresh=[]
        for number,directory_name in enumerate(['first fresh 世界 &','second fresh 世界 &']):
            directory=out/directory_name;directory.mkdir();objects={}
            for stem,relative in [('registry','selfhost/runtime/collections_registry.sg'),('vec','selfhost/runtime/collections_vec.sg'),('smoke','selfhost/tests/collections_runtime_smoke.sg')]:
                source=snap(impl/relative,directory/(stem+'.sg'));obj=directory/(stem+'.obj')
                emitted=run(str(number)+'-emit-'+stem,[native,source,obj,'--wrap']);exact_json(emitted.stdout,{'schema_version':1,'ok':True,'phase':'object'})
                if emitted.stderr:raise ValueError('native emission stderr')
                objects[stem]=obj
            if number==0:
                exe=out/'differential.exe';c_link(run,read,report,'differential-link',clang,[objects['registry'],objects['vec'],test_memory,test,*provider],exe,cenv)
                actual=run('differential-execute',[exe],expected=42);stats=strict_json(actual.stdout)
                required={'operations':50000,'families':5,'state_bytes':12352,'slots':128,'differential_callback_order_equal':True,'owning_8byte_and_16byte_elements':True,'zero_final_allocations':True,'oom_consumes_input_exactly_once':True,'generation_and_kind_rejection':True,'readonly_validation':True,'bad_extent_zero_access':True,'initialized_metadata_reads':True,'same_identity_alias_is_unsafe_precondition':True,'reference_iterator_slot_failure_leak_observed':True,'actual_reference_runtime':True}
                numeric=['candidate_allocations','reference_allocations','candidate_peak_bytes','reference_peak_bytes']
                expected_ids=[value for mode in range(5)for value in [5+mode,10+mode,15+mode,mode]]+[20,21,22]
                if set(stats)!=set(required)|set(numeric)|{'completed'}or any(type(stats[k])is not type(v)or stats[k]!=v for k,v in required.items())or any(type(stats[k])is not int or stats[k]<1 for k in numeric)or type(stats['completed'])is not list or any(type(value)is not int for value in stats['completed'])or stats['completed']!=expected_ids:raise ValueError('differential fixture receipt/actual completion IDs differ')
                if actual.stderr:raise ValueError('differential stderr')
                report['differential']=stats
                resource_exe=out/'resources.exe';c_link(run,read,report,'real-memory-resource-link',clang,[objects['registry'],objects['vec'],resource_memory,resource_probe],resource_exe,cenv)
                observed=run('real-memory-resource-execute',[resource_exe],expected=42);resources=strict_json(observed.stdout)
                resource_expected={'scopes':100,'allocations':700,'freed':700,'attempts':800,'live_bytes':0,'payload_peak_bytes':6144,'owning_drops':20100,'state_bytes':12352,'disposed_zero':True,'virtual_free_verified':True,'rss_qualified':False}
                if set(resources)!=set(resource_expected)|{'working_set_peak_bytes'}or any(type(resources[k])is not type(value)or resources[k]!=value for k,value in resource_expected.items())or type(resources['working_set_peak_bytes'])is not int or resources['working_set_peak_bytes']<1 or observed.stderr:raise ValueError('actual OS resource observation differs')
                report['actual_os_resources']=resources
            for stem,relative in [('memory','selfhost/runtime/collections_memory.c'),('fixture','selfhost/tests/collections_runtime_fixture.c')]:
                source=snap(impl/relative,directory/(stem+'.c'));objects[stem]=compile_c(str(number)+'-compile-'+stem,source,directory/(stem+'.obj'),sdk=True)
            image=directory/'collections.exe'
            if number==0:
                frames={}
                for stem in ['registry','vec','smoke']:
                    inspected=run(stem+'-native-frame-inspection',[readobj,'--unwind',objects[stem]],cenv)
                    found={match[1]:int(match[2])for match in re.finditer(r'StartAddress: ([^ ]+) .*?ALLOC_LARGE size=(\d+)',inspected.stdout.decode(),re.S)}
                    if not found:raise ValueError('native function stack frames unavailable '+stem)
                    frames.update(found)
                report['native_frames']={'functions':frames,'maximum_single_frame_bytes':max(frames.values()),'caller_state_bytes':12352,'rss_qualified':False}
            run(str(number)+'-sg-link',[linker,'link',image,'main',*objects.values(),kernel,stack])
            run(str(number)+'-sg-execute',[image],expected=42,empty=True)
            inspect=run(str(number)+'-independent-inspection',[readobj,'--file-headers','--coff-imports','--unwind',image],cenv)
            if b'VirtualAlloc'not in inspect.stdout or b'VirtualFree'not in inspect.stdout:raise ValueError('actual OS memory imports absent')
            values={name:sha(path)for name,path in objects.items()};values['image']=sha(image)
            if fresh and values!=fresh[0]['sha256']:raise ValueError('two fresh SG/normalized C objects or PE bytes differ')
            fresh.append({'directory':directory_name,'sha256':values,'execution_exit':42})
        report['fresh_rounds']=fresh
        report['deployment']={'sg_objects':['registry.obj','vec.obj','smoke.obj'],'c_objects':['memory.obj','fixture.obj'],'os_libraries':[str(kernel),str(stack)],'c_sdk_normalization':'leaf-only metadata removal plus timestamp zero, verified code/data/relocations/symbol addresses'}
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
