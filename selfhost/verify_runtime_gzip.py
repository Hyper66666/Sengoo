"""Qualify SG gzip framing against new raw-prefix and independent pinned Rust/Python evidence."""
from __future__ import annotations
import argparse,collections,datetime as dt,gzip,hashlib,json,os,struct,subprocess,sys,time,zlib
from pathlib import Path
from linker_evidence import copy_linker
from verify_runtime_text import coff_image,dependencies,digest,exact_json,git_status,revision,sha,strict_json
from verify_runtime_deflate import metadata_proof
from runtime_evidence import bind_memory_providers,c_link,resource_receipt

ROOT=Path(__file__).resolve().parents[1]
CORPUS_SHA='539da730964da46cccae92e1adfe2e588acc67e192131b7b3a38921d2bd583a7'
def reference(source,mode):
    output=b'';used=0
    try:
        while True:
            decoder=zlib.decompressobj(31);part=decoder.decompress(source[used:])+decoder.flush()
            if not decoder.eof:return None,None
            used=len(source)-len(decoder.unused_data);output+=part
            if mode==0 or used==len(source):return output,used
            if mode==1:return None,None
    except zlib.error:return None,None

def check_corpus(raw,manifest,rust):
    if digest(raw)!=CORPUS_SHA or raw[:8]!=b'SGGZP1\0\0' or struct.unpack_from('<Q',raw,8)[0]!=8061 or manifest['records']!=8061:raise ValueError('gzip corpus identity/count differs')
    if len(manifest['fixtures'])!=8061 or len(rust['observations'])!=8061:raise ValueError('gzip/Rust fixture inventory differs')
    at=16;counts=collections.Counter();guards=[];differences=[];names=set();plain_facts={}
    exceptions={row['name'] for row in manifest['oracle_differences']}
    if exceptions!={'name-length-65536','comment-length-65536'}:raise ValueError('unexpected gzip policy difference')
    for index,(fixture,observed) in enumerate(zip(manifest['fixtures'],rust['observations'])):
        length,capacity,expected,plain_length,mode=struct.unpack_from('<IIqII',raw,at);at+=24
        source=raw[at:at+length];at+=length;plain=raw[at:at+plain_length];at+=plain_length
        if at>len(raw) or fixture['id']!=index or fixture['name'] in names or observed['id']!=index or observed['name']!=fixture['name'] or (length,capacity,expected,plain_length,mode)!=(fixture['input_length'],fixture['capacity'],fixture['expected'],fixture['output_length'],fixture['mode']) or digest(source)!=fixture['input_sha256']:raise ValueError('fixture ID/bytes/oracle fields differ')
        names.add(fixture['name']);value,used=reference(source,mode)
        plain_facts[fixture['name']]={'expected':expected,'mode':mode,'bytes':plain_length,'sha256':digest(plain)}
        if fixture['name'] in exceptions:
            if expected!=-1 or value!=b'':raise ValueError('metadata bound difference changed')
        elif expected>=0:
            if value!=plain or used is None or expected!=(used<<27)|len(value) or len(value)>capacity:raise ValueError('positive Python wire/output differs')
        elif expected==-2:
            if value!=plain or not capacity<len(value):raise ValueError('capacity fixture is not independently valid')
        elif expected!=-1 or value is not None:raise ValueError('malformed gzip oracle differs')
        for key,policy in [('first',0),('multi',2)]:
            value,used=reference(source,policy);actual_ok=observed[key+'_ok'];actual=bytes.fromhex(observed[key+'_hex'])
            if type(actual_ok) is not bool:raise ValueError('Rust result boolean type differs')
            if fixture['name'] in exceptions:
                if actual_ok:raise ValueError('Rust metadata limit unexpectedly accepts oversized field')
                differences.append({'id':index,'name':fixture['name'],'rust_policy':key,'python_accepts':True,'rust_accepts':False,'sg_accepts':False,'reason':'documented flate2 MAX_HEADER_BUF65535 compatibility'})
            elif actual_ok!=(value is not None) or actual_ok and actual!=value:raise ValueError('unclassified Rust/Python framing difference '+fixture['name']+' '+key)
        guarded=length<=131072 and capacity<=4096
        if guarded:guards.append(index)
        counts['successes' if expected>=0 else 'malformed' if expected==-1 else 'capacity_failures']+=3 if guarded else 1
    if at!=len(raw) or len(differences)!=4:raise ValueError('gzip corpus trailing bytes/difference count')
    python_gzip_differences=[]
    for row in manifest['python_gzip_observations']:
        name=row['name'];observed=row['observation'];selected=plain_facts[name]
        equivalent=(selected['expected']>=0)==observed['ok'] and (not observed['ok'] or (selected['bytes'],selected['sha256'])==(observed['bytes'],observed['sha256']))
        if equivalent:continue
        if name in exceptions:reason='flate2-compatible65535 metadata bound'
        elif name.startswith('empty-'):reason='selected API requires at least one member; Python gzip allows empty file'
        elif name=='corrupt-header-crc':reason='FHCRC validated by SG/zlib/flate2; Python gzip skips its value'
        elif name.startswith('reserved-bit-'):reason='reserved flags must reject; Python gzip is more tolerant'
        elif selected['mode']==0 and selected['expected']>=0:reason='first-member policy does not decode or validate later members/tails unlike Python gzip'
        elif selected['mode']==1:reason='strict-single requires immediate EOF unlike Python gzip multi/zero-pad tolerance'
        elif selected['mode']==2 and 'zero' in name:reason='strict concatenation requires another member; Python gzip tolerates zero padding'
        else:raise ValueError('unclassified Python gzip policy difference '+name)
        python_gzip_differences.append({'name':name,'mode':selected['mode'],'sg_expected':selected['expected'],'python_gzip':observed,'reason':reason})
    return {'records':8061,'calls':8061+len(guards)*2,**counts,'guard_cases':len(guards)*2,'invalid_limits_modes':10,'guard_fixture_ids':guards,'classified_rust_python_differences':differences,'classified_python_gzip_differences':python_gzip_differences,'every_fixture_compared_to_rust_first_and_multi':True}

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['raw-evidence','rust-evidence','output']:p.add_argument('--'+name,type=Path,required=True)
    a=p.parse_args();out=a.output.resolve()
    if not out.is_relative_to(ROOT/'target'):raise ValueError('output outside target')
    out.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'stage':'sg-runtime-gzip-framing','result':'running','source_revision':revision(),'git_status_start':git_status(),'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'host':__import__('platform').platform(),'sources':{},'external_sources':{},'tools':{},'steps':[],'cases':[],
            'package_integration_complete':False,'final_sdk_complete':False,'remaining':['actual tar entries/publication/checksum draining integration','sgpm output budget256MiB versus this bounded64MiB foundation; later streaming/chunked256MiB and10000-entry tar budget required','installed SDK/package/performance/self-build qualification']}
    cache={};checkout={}
    def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n')
    def read(path,expected=None,tool=False):
        path=Path(path).resolve()
        if path not in cache:cache[path]=path.read_bytes()
        raw=cache[path]
        if expected is not None and digest(raw)!=expected:raise ValueError('qualified input differs: '+str(path))
        if not tool:
            if path.is_relative_to(ROOT):
                report['sources'][str(path.relative_to(ROOT))]=digest(raw)
                if not path.is_relative_to(ROOT/'target'):checkout[path]=digest(raw)
            else:report['external_sources'][str(path)]=digest(raw)
        return raw
    def tool(name,path,expected=None):
        path=Path(path).resolve();raw=read(path,expected,True);report['tools'][name]={'path':str(path),'sha256':digest(raw)};return path
    def snap(path,saved,expected=None):
        raw=read(path,expected);saved.parent.mkdir(parents=True,exist_ok=True);saved.write_bytes(raw);read(saved,digest(raw));return saved
    clean={key:value for key,value in os.environ.items() if not key.upper().startswith(('SENGOO_','SGPLATFORM_','SDL2_','PYTHON','RUSTC','RUSTDOC','CARGO_')) and key.upper() not in ['RUSTFLAGS','RUSTUP_TOOLCHAIN']};pure=dict(clean,PATH='')
    def run(name,command,env=pure,expected=0,empty=False):
        command=list(map(str,command));start=time.monotonic();r=subprocess.run(command,cwd=out,env=env,capture_output=True,timeout=900)
        first,second=out/(name+'.stdout.log'),out/(name+'.stderr.log');first.write_bytes(r.stdout);second.write_bytes(r.stderr)
        report['steps'].append({'name':name,'command':command,'exit_code':r.returncode,'expected_exit_code':expected,'seconds':time.monotonic()-start,'empty_path':env.get('PATH')=='','stdout_sha256':sha(first),'stderr_sha256':sha(second)});save();print(name,r.returncode,flush=True)
        if r.returncode!=expected or empty and(r.stdout or r.stderr):raise AssertionError(name+' result/output differs')
        return r
    save()
    try:
        for name in ['selfhost/verify_runtime_gzip.py','selfhost/verify_runtime_deflate.py','selfhost/verify_runtime_text.py','selfhost/linker_evidence.py','tools/sgpm/src/resolver.rs']:read(ROOT/name)
        raw_path=a.raw_evidence.resolve();rust_path=a.rust_evidence.resolve()
        if not raw_path.is_relative_to(ROOT/'target') or not rust_path.is_relative_to(ROOT/'target'):raise ValueError('qualifications outside target')
        raw=strict_json(read(raw_path));rust=strict_json(read(rust_path))
        if type(raw.get('schema')) is not int or raw['schema']!=1 or raw.get('stage')!='sg-runtime-raw-deflate' or raw.get('result')!='passed' or any(raw.get(key)!=[] for key in ['changed_sources','changed_external_sources','changed_tools','checkout_changes']):raise ValueError('raw-prefix qualification is not passed/unchanged')
        exact_json(json.dumps(raw['probe_results']['prefix-counts']).encode(),{'schema':1,'ok':True,'prefix_cases':4,'legacy_exact_cases':3,'invalid_limits':8,'null_empty_output':True,'no_trailing_reads':True,'decoded_bytes_equal':True})
        if type(rust.get('schema')) is not int or rust['schema']!=1 or rust.get('stage')!='independent-flate2-gzip-policy' or rust.get('result')!='passed' or any(rust.get(key)!=[] for key in ['changed_sources','changed_tools','changed_cargo_configs']):raise ValueError('Rust policy oracle is not passed/unchanged')
        for name,expected in raw['sources'].items():read(ROOT/name,expected)
        for name,expected in raw['external_sources'].items():read(Path(name),expected)
        for name,identity in raw['tools'].items():tool('raw-producer-'+name,identity['path'],identity['sha256'])
        for name,expected in rust['sources'].items():read(Path(name),expected)
        for name,identity in rust['tools'].items():tool('rust-producer-'+name,identity['path'],identity['sha256'])
        observation=rust['observation_stdout'];observed_stdout=read(observation['path'],observation['sha256']);executions=[row for row in rust['steps'] if row['name']=='execute-all-fixtures']
        if len(executions)!=1 or executions[0]['exit']!=0 or executions[0]['stdout_sha256']!=observation['sha256'] or executions[0]['command']!=[rust['tools']['flate2-oracle']['path'],observation['corpus']]:raise ValueError('Rust observations are not bound to their actual executable command/stdout')
        corpus_manifest=Path(observation['manifest']).resolve()
        if str(corpus_manifest) not in rust['sources']:raise ValueError('Rust observations lack the pinned input manifest')
        rust_manifest=strict_json(read(corpus_manifest,rust['sources'][str(corpus_manifest)]));rows=observed_stdout.decode('utf-8').splitlines()
        if len(rows)!=rust_manifest['records'] or len(rust['observations'])!=len(rows):raise ValueError('Rust observed fixture IDs are incomplete')
        for index,line in enumerate(rows):
            fields=line.split('|')
            if len(fields)!=5 or fields[0]!=str(index) or any(value not in ['true','false'] for value in fields[1:3]) or any(__import__('re').fullmatch(r'(?:[0-9a-f]{2})*',value) is None for value in fields[3:]):raise ValueError('invalid exact Rust observation stdout')
            expected_observation={'id':index,'name':rust_manifest['fixtures'][index]['name'],'first_ok':fields[1]=='true','multi_ok':fields[2]=='true','first_hex':fields[3],'multi_hex':fields[4]}
            if rust['observations'][index]!=expected_observation:raise ValueError('declared Rust observation differs from executed stdout')
        for name,expected in rust['cargo_config_candidates'].items():
            if (sha(name) if Path(name).is_file() else None)!=expected:raise ValueError('Rust oracle Cargo config changed')
        report['cargo_config_candidates']=rust['cargo_config_candidates']
        for name in ['native','formatter']:
            identity=raw['tools'][name]
            if raw['artifacts'][name+'.exe']!=identity['sha256']:raise ValueError('raw emitter/formatter binary is not bound by artifact manifest')
            saved=out/(name+'.exe');saved.write_bytes(read(identity['path'],identity['sha256'],True));tool(name,saved,identity['sha256'])
        native,formatter=out/'native.exe',out/'formatter.exe'
        pin=raw['linker_qualification']['evidence'];linker_path=Path(pin['path']).resolve()
        if not linker_path.is_relative_to(ROOT/'target') or raw['sources'].get(str(linker_path.relative_to(ROOT)))!=pin['sha256']:raise ValueError('raw qualification does not bind declared linker parent')
        read(linker_path,pin['sha256']);linker,linked=copy_linker(ROOT,linker_path,out,report)
        if linked['binary_sha256']!=raw['tools']['sg-linker']['sha256']:raise ValueError('historical linker binary changed')
        for name,identity in list(report['tools'].items()):tool(name,identity['path'],identity['sha256'])
        for name,expected in list(report['sources'].items()):read(ROOT/name,expected)
        clang=tool('clang',raw['tools']['clang']['path'],raw['tools']['clang']['sha256'])
        extra={name:tool(name,raw['tools'][name]['path'],raw['tools'][name]['sha256']) for name in ['objcopy','objdump','readobj','lld']}
        kernel=tool('sdk-kernel32',linked['sdk']['path'],linked['sdk']['sha256'])
        stack_origin=raw_path.parent/'ntdll-chkstk.lib';stack_pin=raw['artifacts']['ntdll-chkstk.lib']
        if raw['sources'].get(str(stack_origin.relative_to(ROOT)))!=stack_pin:raise ValueError('raw stack import library is not bound')
        stack=snap(stack_origin,out/'ntdll-chkstk.lib',stack_pin)
        report['qualifications']={'raw':{'path':str(raw_path),'sha256':digest(read(raw_path)),'source_revision':raw['source_revision'],'source_revision_end':raw['source_revision_end']},'rust':{'path':str(rust_path),'sha256':digest(read(rust_path)),'packages':rust['packages'],'revision_start':rust['revision_start'],'revision_end':rust['revision_end']}}
        python=tool('runner-python',raw['tools']['runner-python']['path'],raw['tools']['runner-python']['sha256'])
        tool('zlib-implementation',raw['tools']['zlib-implementation']['path'],raw['tools']['zlib-implementation']['sha256']);report['zlib_implementation']=raw['zlib_implementation']
        cenv=dict(clean,PATH=str(clang.parent)+os.pathsep+clean.get('PATH',''),SOURCE_DATE_EPOCH='0')
        impl=out/'implementation';sources=['selfhost/runtime/deflate.sg','selfhost/runtime/deflate_stream.sg','selfhost/runtime/deflate_stream_memory.c','selfhost/runtime/deflate_storage.c','selfhost/runtime/deflate_memory.c','selfhost/runtime/gzip.sg','selfhost/runtime/gzip_memory.c','selfhost/tests/deflate_cases.py','selfhost/tests/deflate_probe.c','selfhost/tests/deflate_resource_probe.h','selfhost/tests/gzip_resource_probe.c','selfhost/tests/gzip_cases.py','selfhost/tests/gzip_probe.c','selfhost/tests/gzip_fixture.c','selfhost/tests/gzip_smoke.sg','selfhost/runtime_evidence.py']
        for name in sources:snap(ROOT/name,impl/name)
        for name in ['deflate','deflate_stream','gzip']:run('format-'+name,[formatter,'--check',impl/'selfhost/runtime'/(name+'.sg')],empty=True)
        report['actual_os_memory_providers']=bind_memory_providers(read)
        run('format-smoke',[formatter,'--check',impl/'selfhost/tests/gzip_smoke.sg'],empty=True)
        corpora=[]
        for name in ['corpus.bin','repeat.bin']:
            path=out/name;made=run('generate-'+name,[python,'-B','-X','utf8',impl/'selfhost/tests/gzip_cases.py','--output',path]);summary=strict_json(made.stdout);manifest=strict_json(read(path.with_suffix('.bin.json')))
            if made.stderr or summary['records']!=8061 or summary['sha256']!=CORPUS_SHA or manifest['source_sha256']!=sha(impl/'selfhost/tests/gzip_cases.py') or manifest['zlib_implementation']!=report['zlib_implementation']:raise ValueError('gzip generator source/tool identity differs')
            if not any(str(path).endswith('corpus-rust.bin') and value==CORPUS_SHA for path,value in rust['sources'].items()):raise ValueError('Rust oracle did not execute this exact generated corpus')
            checked=check_corpus(read(path,CORPUS_SHA),manifest,rust);corpora.append(checked)
        if corpora[0]!=corpora[1]:raise ValueError('gzip two generation inventories differ')
        report['corpus']=dict(corpora[0],sha256=CORPUS_SHA,identical_regeneration=True,groups=manifest['groups'],python_gzip_observations=manifest['python_gzip_observations'])
        large=out/'large';run('generate-large',[python,'-B','-X','utf8',impl/'selfhost/tests/gzip_cases.py','--large-dir',large]);large_manifest=strict_json(read(large/'large.json'))
        if large_manifest['source_sha256']!=sha(impl/'selfhost/tests/gzip_cases.py') or large_manifest['zlib_implementation']!=report['zlib_implementation']:raise ValueError('large gzip generator identity differs')
        for row in large_manifest['files']:
            data=read(large/row['name'],row['sha256']);plain,used=reference(data,2)
            if used!=len(data) or len(data)!=row['bytes'] or len(plain)!=row['decoded_bytes'] or digest(plain)!=row['decoded_sha256']:raise ValueError('large independent gzip oracle differs')
        report['large_inputs']=large_manifest
        def compile_c(label,source,target,sdk=False,production=False):
            flags=['-std=c11','-Wall','-Wextra','-Werror','-Werror=date-time','-g0','-fno-ident','-O2' if sdk else '-O0']
            if sdk:flags+=['-fno-addrsig','-fomit-frame-pointer','-fno-asynchronous-unwind-tables','-fno-unwind-tables','-fno-vectorize','-fno-slp-vectorize']
            if production:flags+=['-DSENGOO_GZIP_PRODUCTION_MEMORY']
            before,after=target.with_suffix('.before.d'),target.with_suffix('.d');run(label+'-headers',[clang,*flags,'-M','-MT','runtime_text_dependency','-MF',before,source],cenv);headers={path:digest(read(path)) for path in dependencies(before)}
            raw_target=target.with_suffix('.raw.obj') if sdk else target;run(label,[clang,*flags,'-MD','-MT','runtime_text_dependency','-MF',after,'-c',source,'-o',raw_target],cenv)
            if dependencies(after)!=set(headers) or any(sha(path)!=value for path,value in headers.items()):raise ValueError('actual C headers changed')
            report.setdefault('c_header_closures',{})[label]={str(path):value for path,value in headers.items()}
            if sdk:
                original=raw_target.read_bytes();parsed=coff_image(original);removed=[name for name,value in parsed['sections'].items() if name in ['.data','.bss'] and not value['data'] and not value['relocations']]
                assembly=run(label+'-leaf',[extra['objdump'],'-d','--no-show-raw-insn',raw_target],cenv).stdout.decode()
                if __import__('re').search(r'%(?:rsp|rbp)|\b(?:call\w*|push\w*|pop\w*|enter|leave)\b',assembly.split('Disassembly of section .text:',1)[1]):raise ValueError('C gzip memory/fixture is not stack-free leaf')
                run(label+'-metadata',[extra['objcopy'],'--strip-unneeded','--strip-symbol=.file',*['--remove-section='+name for name in removed],raw_target,target],cenv)
                normalized=bytearray(target.read_bytes());normalized[4:8]=bytes(4);target.write_bytes(normalized);report.setdefault('metadata_proofs',{})[label]=metadata_proof(original,bytes(normalized))
        first={};rounds=[]
        for index,name in enumerate(['first fresh 世界 &','second fresh 世界 &']):
            directory=out/name;directory.mkdir();objects={}
            for stem,relative in [('deflate','selfhost/runtime/deflate.sg'),('deflate_stream','selfhost/runtime/deflate_stream.sg'),('gzip','selfhost/runtime/gzip.sg'),('gzip_smoke','selfhost/tests/gzip_smoke.sg')]:
                source=snap(impl/relative,directory/(stem+'.sg'));obj=directory/(stem+'.obj');made=run(str(index)+'-emit-'+stem,[native,source,obj,'--wrap']);exact_json(made.stdout,{'schema_version':1,'ok':True,'phase':'object'})
                if made.stderr:raise AssertionError('gzip emitter stderr')
                objects[stem]=obj
                if stem in ['deflate','deflate_stream'] and sha(obj)!=raw['cases'][0]['sha256'][stem]:raise ValueError('gzip linked raw/core object differs from the full raw qualification')
            for stem,relative in [('deflate_memory','selfhost/runtime/deflate_memory.c'),('deflate_stream_memory','selfhost/runtime/deflate_stream_memory.c'),('deflate_storage','selfhost/runtime/deflate_storage.c'),('gzip_memory','selfhost/runtime/gzip_memory.c'),('gzip_fixture','selfhost/tests/gzip_fixture.c')]:
                source=snap(impl/relative,directory/(stem+'.c'));obj=directory/(stem+'.obj');compile_c(str(index)+'-'+stem,source,obj,True);objects[stem]=obj
            image=directory/'gzip.exe';run(str(index)+'-sg-link',[linker,'link',image,'main',*objects.values(),kernel,stack]);run(str(index)+'-cold-execute',[image],expected=42,empty=True)
            identity={key:sha(path) for key,path in objects.items()};identity['image']=sha(image)
            if index==0:first=identity
            elif first!=identity:raise AssertionError('fresh SG / normalized C / gzip PE bytes differ')
            report['cases'].append({'round':name,'sha256':identity,'cold_exit':42,'stdout_empty':True,'stderr_empty':True});rounds.append(objects)
        report['probe_results']={}
        for mode in ['instrumented','production']:
            obj=out/('probe-'+mode+'.obj');exe=out/('probe-'+mode+'.exe');compile_c('compile-'+mode,impl/'selfhost/tests/gzip_resource_probe.c',obj,production=mode=='production')
            c_link(run,read,report,'link-'+mode,clang,[obj,rounds[0]['deflate'],rounds[0]['deflate_stream'],rounds[0]['gzip'],*([rounds[0]['deflate_memory'],rounds[0]['deflate_stream_memory'],rounds[0]['gzip_memory']]if mode=='production'else []),stack],exe,cenv)
            done=run(mode+'-corpus',[exe,out/'corpus.bin']);expected={'schema':1,'ok':True,**{key:report['corpus'][key] for key in ['records','calls','successes','malformed','capacity_failures','guard_cases','invalid_limits_modes']},'null_empty_output':True,'input_unchanged':True,'instrumented':mode=='instrumented','sequential_bounded_access':True if mode=='instrumented' else None,'input_reads':2265606 if mode=='instrumented' else None,'initialized_output_reads':4754893 if mode=='instrumented' else None,'writes':3502474 if mode=='instrumented' else None}
            report.setdefault('state_resource_receipts',{})[mode+'-corpus']=resource_receipt(done.stderr,13692)
            report['probe_results'][mode+'-corpus']=exact_json(done.stdout,expected)
            done=run(mode+'-64MiB',[exe,'--large',large]);observed=strict_json(done.stdout)
            expected={'schema':1,'ok':True,'large_cases':4,'aggregate_members':64,'aggregate_decoded_bytes':67108864,'capacity_failure':True,'first_member_exact':True,'input_limit_bytes':67108864,'max_input_output_bytes':67103726,'readonly_input_tailguard':True,'output_tailguard':True,'instrumented':mode=='instrumented','complete_sequential_input_output':True if mode=='instrumented' else None,'input_reads':67244822 if mode=='instrumented' else None,'initialized_output_reads':336587438 if mode=='instrumented' else None,'writes':202369968 if mode=='instrumented' else None}
            report['state_resource_receipts'][mode+'-64MiB']=resource_receipt(done.stderr,130)
            if mode=='instrumented' and any(type(observed.get(key)) is not int or observed[key]<=67108864 for key in ['input_reads','initialized_output_reads','writes']):raise AssertionError('large gzip instrumentation missing')
            report['probe_results'][mode+'-64MiB']=exact_json(done.stdout,expected)
            oom=run(mode+'-oom',[exe,'--oom-protocol']);exact_json(oom.stdout,{'schema':1,'ok':True,'oom':True,'no_raw_input_output_access':True});report['state_resource_receipts'][mode+'-oom']=resource_receipt(oom.stderr,0,True)
        poisoned=bytearray(read(out/'corpus.bin'));struct.pack_into('<q',poisoned,24,-1);wrong=out/'wrong-empty-oracle.bin';wrong.write_bytes(poisoned);bad=run('wrong-result-oracle',[out/'probe-instrumented.exe',wrong],expected=1)
        if bad.stdout or b'reason=gzip-result-oracle expected=-1 actual=' not in bad.stderr:raise AssertionError('gzip wrong oracle did not reach decoder result')
        report['wrong_oracle_control']={'actual_exit':1,'reason':'gzip-result-oracle','only_first_expected_result_changed':True}
        report['artifacts']={str(path.relative_to(out)):sha(path) for path in out.rglob('*') if path.is_file() and path.suffix in ['.obj','.exe','.lib']};report.update(result='passed',deterministic_sg_and_normalized_c_objects_and_images=True,single_shared_deflate_core=True,raw_wrapper_qualification_sha256=digest(read(raw_path)))
    except BaseException as error:report.update(result='failed',error=repr(error))
    finally:
        try:
            report['changed_sources']=[name for name,value in report['sources'].items() if not(ROOT/name).is_file() or sha(ROOT/name)!=value]
            report['changed_external_sources']=[name for name,value in report['external_sources'].items() if not Path(name).is_file() or sha(name)!=value]
            report['changed_tools']=[name for name,value in report['tools'].items() if not Path(value['path']).is_file() or sha(value['path'])!=value['sha256']]
            report['checkout_changes']=[str(path.relative_to(ROOT)) for path,value in checkout.items() if not path.is_file() or sha(path)!=value]
            report['changed_cargo_configs']=[name for name,value in report.get('cargo_config_candidates',{}).items() if (sha(name) if Path(name).is_file() else None)!=value]
            report['source_revision_end']=revision();report['git_status_end']=git_status();report['initial_cached_reads']={str(path):1 for path in cache}
            if any(report[key] for key in ['changed_sources','changed_external_sources','changed_tools','checkout_changes','changed_cargo_configs']):report.update(result='failed',source_guard_failure='consumed gzip qualification changed')
        except BaseException as error:report.update(result='failed',finally_error=repr(error))
        report['completed_at']=dt.datetime.now(dt.timezone.utc).isoformat();save()
    print(json.dumps({key:report.get(key) for key in ['result','error','changed_sources','changed_external_sources','changed_tools','checkout_changes']}));return 0 if report['result']=='passed' else 1
if __name__=='__main__':raise SystemExit(main())
