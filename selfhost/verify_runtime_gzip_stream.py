"""Qualify fixed-state SG gzip framing continuation; old synchronous framing is unchanged."""
from __future__ import annotations
import argparse,datetime as dt,json,os,re,struct,subprocess,sys,time,zlib
from pathlib import Path
from verify_runtime_text import coff_image,dependencies,digest,exact_json,git_status,revision,sha,strict_json
from verify_runtime_deflate import metadata_proof
from verify_runtime_gzip import check_corpus
from linker_evidence import copy_linker
from runtime_evidence import c_link
ROOT=Path(__file__).resolve().parents[1]

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['text-evidence','gzip-evidence','stream-evidence','rust-evidence','output']:p.add_argument('--'+name,type=Path,required=True)
    a=p.parse_args();out=a.output.resolve()
    if not out.is_relative_to(ROOT/'target'):raise ValueError('output must be fresh target directory')
    out.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'stage':'sg-runtime-gzip-continuation','result':'running','revision_start':revision(),'git_status_start':git_status(),'host':__import__('platform').platform(),'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'sources':{},'external_sources':{},'tools':{},'artifacts':{},'steps':[],'old_synchronous_framing_migrated':False,'package_integration_complete':False,'remaining':['extract shared CRC owner and migrate old gzip export to this framing core with full regressions','incremental tar/private staging/full integrity validation/atomic publication','256MiB tar-content budget separate from decoded wire budget','performance/RSS/final SDK packaging/self-build qualification']}
    cache={}
    def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf8')
    def read(path,expected=None):
        path=Path(path).resolve()
        if path not in cache:cache[path]=path.read_bytes()
        data=cache[path];fingerprint=digest(data)
        if expected is not None and fingerprint!=expected:raise ValueError('qualified input identity differs '+str(path))
        group='sources'if path.is_relative_to(ROOT)else'external_sources';report[group][str(path.relative_to(ROOT))if group=='sources'else str(path)]=fingerprint
        return data
    def tool(name,identity):
        path=Path(identity['path']).resolve();read(path,identity['sha256']);report['tools'][name]={'path':str(path),'sha256':identity['sha256']};return path
    clean={k:v for k,v in os.environ.items()if not k.upper().startswith(('SENGOO_','SGPLATFORM_','PYTHON','RUSTC','RUSTDOC','CARGO_'))};pure=dict(clean,PATH='')
    def run(name,command,env=pure,expected=0,empty=False):
        command=list(map(str,command));start=time.monotonic();r=subprocess.run(command,cwd=out,env=env,capture_output=True,timeout=900)
        for suffix,data in [('stdout',r.stdout),('stderr',r.stderr)]:
            file=out/(name+'.'+suffix);file.write_bytes(data);report['artifacts'][str(file)]=digest(data)
        report['steps'].append({'name':name,'command':command,'exit':r.returncode,'expected_exit':expected,'empty_path':env.get('PATH')=='','seconds':time.monotonic()-start,'stdout_sha256':digest(r.stdout),'stderr_sha256':digest(r.stderr)});save();print(name,r.returncode,flush=True)
        if r.returncode!=expected or empty and(r.stdout or r.stderr):raise ValueError(name+' exit/output differs')
        return r
    save()
    try:
        text=strict_json(read(a.text_evidence));gzip=strict_json(read(a.gzip_evidence));stream=strict_json(read(a.stream_evidence));rust=strict_json(read(a.rust_evidence))
        for value in [text,gzip,stream,rust]:
            if type(value.get('schema'))is not int or value['schema']!=1 or value.get('result')!='passed'or any(v!=[]for k,v in value.items()if k.startswith('changed_')or k=='checkout_changes'):raise ValueError('parent report did not pass unchanged-input guards')
        if text['stage']!='sg-runtime-text-utf8'or gzip['stage']!='sg-runtime-gzip-framing'or stream['stage']!='sg-runtime-deflate-continuation'or rust['stage']!='independent-flate2-gzip-policy':raise ValueError('qualification stage differs')
        for group in ['sources','external_sources']:
            for name,fingerprint in text[group].items():read(ROOT/name if group=='sources'else Path(name),fingerprint)
        for name,identity in text['tools'].items():tool('text-producer-'+name,identity)
        native=tool('native',text['tools']['native']);formatter=tool('formatter',text['tools']['formatter']);clang=tool('clang',text['tools']['clang']);lld=tool('lld',text['tools']['clang-probe-linker']);kernel=tool('sdk-kernel32',text['tools']['sdk-kernel32']);objdump=tool('objdump',text['tools']['llvm-objdump-c-sdk-leaf-check']);objcopy=tool('objcopy',text['tools']['llvm-objcopy-c-sdk-metadata'])
        for name in ['native','formatter']:
            if text['artifacts'][Path(text['tools'][name]['path']).name]!=text['tools'][name]['sha256']:raise ValueError('actual compiler/formatter artifact relationship differs')
        pin=text['linker_qualification']['evidence'];parent=Path(pin['path']).resolve()
        if not parent.is_relative_to(ROOT/'target')or text['sources'].get(str(parent.relative_to(ROOT)))!=pin['sha256']:raise ValueError('declared linker report pin not bound')
        read(parent,pin['sha256']);linker,branch=copy_linker(ROOT,parent,out,report)
        if branch['binary_sha256']!=text['tools']['sg-linker']['sha256']:raise ValueError('historical linker binary differs')
        for name,identity in list(report['tools'].items()):tool(name,identity)
        for name,fingerprint in list(report['sources'].items()):read(ROOT/name,fingerprint)
        stack_origin=a.text_evidence.resolve().parent/'ntdll-chkstk.lib';stack_pin=text['stack_probe_provider']['import_library_sha256']
        if text['sources'].get(str(stack_origin.relative_to(ROOT)))!=stack_pin:raise ValueError('declared stack import library not source-bound')
        stack=out/'ntdll-chkstk.lib';stack.write_bytes(read(stack_origin,stack_pin));read(stack,stack_pin)
        python=tool('runner-python',{'path':sys.executable,'sha256':sha(Path(sys.executable))});tool('zlib-implementation',gzip['tools']['zlib-implementation'])
        for name in ['selfhost/verify_runtime_gzip_stream.py','selfhost/verify_runtime_gzip.py','selfhost/verify_runtime_deflate.py','selfhost/verify_runtime_text.py','selfhost/runtime_evidence.py','selfhost/linker_evidence.py']:read(ROOT/name)
        for name in ['proposal.md','design.md','specs/selfhost-deflate-core/spec.md']:read(ROOT/'openspec/changes/add-selfhost-deflate-core'/name)
        baseline=['selfhost/runtime/gzip.sg','selfhost/runtime/deflate.sg','selfhost/runtime/deflate_stream.sg','selfhost/runtime/deflate_stream_memory.c','selfhost/runtime/deflate_storage.c','selfhost/runtime/deflate_memory.c','selfhost/runtime/gzip_memory.c']
        for name in baseline:read(ROOT/name,gzip['sources'][str(Path(name))])
        core_pin=stream['fresh_rounds'][0]['sha256']['deflate_stream'];core_artifact=a.stream_evidence.resolve().parent/stream['fresh_rounds'][0]['round']/'deflate_stream.obj'
        if stream['artifacts'].get(str(core_artifact))!=core_pin:raise ValueError('raw continuation object is not artifact-bound')
        read(core_artifact,core_pin)
        if stream['sources'].get(str(Path('selfhost/runtime/deflate_stream.sg')))!=gzip['sources'][str(Path('selfhost/runtime/deflate_stream.sg'))]:raise ValueError('old gzip and raw continuation core sources differ')
        corpus_origin=a.gzip_evidence.resolve().parent/'corpus.bin';manifest_origin=corpus_origin.with_suffix('.bin.json')
        corpus=read(corpus_origin,gzip['sources'][str(corpus_origin.relative_to(ROOT))]);manifest=strict_json(read(manifest_origin,gzip['sources'][str(manifest_origin.relative_to(ROOT))]))
        for name,fingerprint in rust['sources'].items():read(name,fingerprint)
        for name,identity in rust['tools'].items():tool('rust-producer-'+name,identity)
        wire=rust['observation_stdout'];stdout=read(wire['path'],wire['sha256']);read(wire['corpus'],digest(corpus))
        executed=[r for r in rust['steps']if r['name']=='execute-all-fixtures']
        if len(executed)!=1 or executed[0]['exit']!=0 or executed[0]['command']!=[rust['tools']['flate2-oracle']['path'],wire['corpus']]or executed[0]['stdout_sha256']!=wire['sha256']:raise ValueError('Rust actual executable/stdout relationship differs')
        lines=stdout.decode().splitlines()
        if len(lines)!=8061 or len(rust['observations'])!=8061:raise ValueError('Rust observation IDs incomplete')
        for index,(line,o)in enumerate(zip(lines,rust['observations'])):
            fields=line.split('|')
            if len(fields)!=5 or fields[0]!=str(index)or any(v not in ['true','false']for v in fields[1:3])or fields[1:3]!=['true'if o['first_ok']else'false','true'if o['multi_ok']else'false']or fields[3:]!=[o['first_hex'],o['multi_hex']]:raise ValueError('Rust declared result differs from actual stdout')
        report['corpus']=check_corpus(corpus,manifest,rust);saved_corpus=out/'corpus.bin';saved_corpus.write_bytes(corpus);read(saved_corpus,digest(corpus))
        at=16;done=1469598103934665603;expected_runs=0;members=bytearray()
        for fixture in manifest['fixtures']:
            n,cap,expected,size,policy=struct.unpack_from('<IIqII',corpus,at);at+=24;source=corpus[at:at+n];at+=n+size
            schedules=list(range(3))+(list(range(3,n+2))if expected>=0 and n<=96 else [])
            for schedule in schedules:
                for byte in struct.pack('<QQq',fixture['id'],schedule,2 if expected>=0 else expected):done=((done^byte)*1099511628211)&((1<<64)-1)
            expected_runs+=len(schedules)
            count=0;used=0
            if expected>=0:
                while True:
                    decoder=zlib.decompressobj(31);decoder.decompress(source[used:]);decoder.flush()
                    if not decoder.eof:raise ValueError('positive member-count oracle truncated')
                    used=len(source)-len(decoder.unused_data);count+=1
                    if policy==0 or used==len(source):break
                if used!=expected>>27:raise ValueError('member-count wire oracle differs')
            members+=struct.pack('<I',count)
        if at!=len(corpus):raise ValueError('corpus trailing data')
        member_file=out/'members.bin';member_file.write_bytes(members);read(member_file);report['expected_runs']=expected_runs;report['expected_completed_fnv64']=format(done,'016x');report['positive_member_counts_independently_checked']=True
        snapshot=out/'inputs';snapshot.mkdir()
        names=['runtime/gzip_stream.sg','runtime/gzip_stream_memory.c','runtime/gzip.sg','runtime/deflate.sg','runtime/deflate_stream.sg','runtime/deflate_stream_memory.c','runtime/deflate_memory.c','runtime/gzip_memory.c','runtime/deflate_storage.c','tests/gzip_stream_probe.c','tests/gzip_stream_large_probe.c','tests/gzip_stream_noalloc.c','tests/gzip_stream_cases.py','tests/gzip_cases.py','tests/deflate_cases.py','tests/gzip_stream_fixture.c','tests/gzip_stream_smoke.sg']
        for name in names:
            data=read(ROOT/'selfhost'/name);saved=snapshot/Path(name).name;saved.write_bytes(data);read(saved,digest(data))
        for stem in ['gzip_stream','gzip_stream_smoke']:run('format-'+stem,[formatter,'--check',snapshot/(stem+'.sg')],empty=True)
        cenv=dict(clean,PATH=str(clang.parent)+os.pathsep+os.environ.get('PATH',''))
        def compile_c(name,source,target,extra=(),leaf=False):
            flags=['-std=c11','-O2','-g0','-fno-ident','-fno-addrsig',*extra]
            if leaf:flags+=['-fomit-frame-pointer','-fno-asynchronous-unwind-tables','-fno-unwind-tables','-fno-vectorize','-fno-slp-vectorize']
            before=target.with_suffix('.before.d');after=target.with_suffix('.d')
            run(name+'-headers',[clang,*flags,'-M','-MT','runtime_text_dependency','-MF',before,source],cenv);read(before);headers={p:digest(read(p))for p in dependencies(before)}
            run(name,[clang,*flags,'-MD','-MT','runtime_text_dependency','-MF',after,'-c',source,'-o',target],cenv);read(after)
            if dependencies(after)!=set(headers)or any(sha(p)!=value for p,value in headers.items()):raise ValueError('C actual header dependencies changed')
            report['artifacts'][str(target)]=sha(target);return target
        rounds=[];objects=[]
        for number,label in enumerate(['first fresh 世界 &','second fresh 世界 &']):
            folder=out/label;folder.mkdir();built={}
            (folder/'gzip.sg').write_bytes(read(snapshot/'gzip.sg'));read(folder/'gzip.sg')
            for stem in ['gzip_stream','deflate_stream','deflate','gzip_stream_smoke']:
                source=folder/(stem+'.sg');source.write_bytes(read(snapshot/(stem+'.sg')));read(source)
                obj=folder/(stem+'.obj');made=run(str(number)+'-emit-'+stem,[native,source,obj,'--wrap'])
                if made.stderr or strict_json(made.stdout)!={'schema_version':1,'ok':True,'phase':'object'}:raise ValueError('SG emit protocol differs')
                if stem=='deflate_stream'and sha(obj)!=core_pin:raise ValueError('emitted raw continuation core differs from qualified artifact')
                built[stem]=obj
            for stem in ['gzip_stream_memory','deflate_stream_memory','deflate_storage','deflate_memory','gzip_memory','gzip_stream_fixture']:
                source=folder/(stem+'.c');source.write_bytes(read(snapshot/(stem+'.c')));read(source);rawobj=compile_c(str(number)+'-'+stem,source,folder/(stem+'.raw.obj'),leaf=True)
                data=rawobj.read_bytes();parsed=coff_image(data);assembly=run(str(number)+'-'+stem+'-leaf',[objdump,'-d','--no-show-raw-insn',rawobj],cenv).stdout.decode().split('Disassembly of section .text:',1)[1]
                if re.search(r'%(?:rsp|rbp)|\b(?:call\w*|push\w*|pop\w*|enter|leave)\b',assembly):raise ValueError('C memory/constant test fixture is not leaf')
                removed=[n for n,v in parsed['sections'].items()if n in ['.data','.bss']and not v['data']and not v['relocations']];obj=folder/(stem+'.obj')
                run(str(number)+'-'+stem+'-metadata',[objcopy,*['--strip-symbol='+n for n in [*parsed['sections'],'.file','@feat.00']],*['--remove-section='+n for n in removed],rawobj,obj],cenv)
                normalized=bytearray(obj.read_bytes());normalized[4:8]=bytes(4);obj.write_bytes(normalized);report.setdefault('metadata_proofs',{})[label+' '+stem]=metadata_proof(data,bytes(normalized));built[stem]=obj
            image=folder/'gzip-stream.exe';run(str(number)+'-sg-link',[linker,'link',image,'main',*built.values(),kernel,stack],empty=True);run(str(number)+'-cold',[image],expected=42,empty=True)
            hashes={n:sha(p)for n,p in {**built,'image':image}.items()}
            if rounds and rounds[0]['sha256']!=hashes:raise ValueError('two fresh SG/CSDK/PE bytes differ')
            rounds.append({'round':label,'sha256':hashes,'empty_path_cold_exit':42});objects.append(built)
            report['artifacts'].update({str(p):sha(p)for p in [*built.values(),image]})
        report['fresh_rounds']=rounds;results={};large_results={}
        run('generate-wide',[python,'-B','-X','utf8',snapshot/'gzip_stream_cases.py','--output',out/'wide'])
        wide=strict_json(read(out/'wide/manifest.json'))
        if wide['zlib_implementation']['sha256']!=gzip['tools']['zlib-implementation']['sha256']:raise ValueError('wide generator oracle implementation differs')
        for row in wide['files']:read(row['path'],row['sha256'])
        if wide['files'][0]['output_bytes']!=68157440 or wide['files'][0]['members']!=65 or wide['files'][0]['python_gzip_equal']is not True:raise ValueError('independent wide output oracle differs')
        noalloc=compile_c('compile-noalloc-control',snapshot/'gzip_stream_noalloc.c',out/'noalloc.obj')
        noalloc_source=out/'noalloc-control-main.c';noalloc_source.write_text('#include <stdint.h>\nextern uint8_t *sengoo_ds_storage_alloc(int64_t);\nint main(void){sengoo_ds_storage_alloc(45056);return 0;}\n',encoding='ascii');read(noalloc_source)
        noalloc_main=compile_c('compile-noalloc-main',noalloc_source,out/'noalloc-main.obj');noalloc_image=out/'noalloc-control.exe'
        c_link(run,read,report,'link-noalloc-control',clang,[noalloc_main,noalloc],noalloc_image,cenv);report['artifacts'][str(noalloc_image)]=sha(noalloc_image)
        run('execute-noalloc-control',[noalloc_image],expected=99,empty=True)
        for mode in ['instrumented','production']:
            extra=['-DSENGOO_GZS_PRODUCTION']if mode=='production'else[]
            core=[objects[0][n]for n in ['gzip_stream','deflate_stream','deflate']]
            memory=[objects[0][n]for n in ['deflate_stream_memory','deflate_memory','gzip_memory','gzip_stream_memory']]if mode=='production'else[]
            provider=[noalloc]
            obj=compile_c('compile-'+mode,snapshot/'gzip_stream_probe.c',out/(mode+'.obj'),extra);image=out/(mode+'.exe')
            c_link(run,read,report,'link-'+mode,clang,[obj,*core,*memory,*provider,stack],image,cenv);report['artifacts'][str(image)]=sha(image)
            checked=run('corpus-'+mode,[image,saved_corpus,member_file]);value=strict_json(checked.stdout)
            if checked.stderr or set(value)!={'schema','ok','mode','fixtures','runs','calls','need_input','need_output','history_reads','crc_output_reads','state_bytes','completed_fnv64'}or value['ok']is not True or type(value['schema'])is not int or value['schema']!=1 or value['mode']!=mode or type(value['fixtures'])is not int or value['fixtures']!=8061 or type(value['runs'])is not int or value['runs']!=expected_runs or value['state_bytes']!=46080 or value['completed_fnv64']!=format(done,'016x')or any(type(value[k])is not int or value[k]<=0 for k in ['calls','need_input','need_output']):raise ValueError('corpus actual count/status/fixture completion differs')
            for field in ['history_reads','crc_output_reads']:
                if mode=='instrumented'and(type(value[field])is not int or value[field]<=0)or mode=='production'and value[field]is not None:raise ValueError('observed access counters differ')
            results[mode]=value
            if mode=='instrumented':
                failed=run('uninitialized-history-control',[image,'--uninitialized-control'],expected=1)
                if failed.stdout or b'reason=history-uninitialized'not in failed.stderr:raise ValueError('uninitialized history control did not reject')
                bad=bytearray(corpus);struct.pack_into('<q',bad,24,-1);poison=out/'wrong-oracle.bin';poison.write_bytes(bad);read(poison)
                failed=run('wrong-result-control',[image,poison,member_file],expected=1)
                if failed.stdout or b'fixture=0 reason=negative-status expected=-1 actual=2'not in failed.stderr:raise ValueError('wrong oracle did not reach actual decoder result')
            obj=compile_c('compile-wide-'+mode,snapshot/'gzip_stream_large_probe.c',out/('wide-'+mode+'.obj'),extra);image=out/('wide-'+mode+'.exe')
            c_link(run,read,report,'link-wide-'+mode,clang,[obj,*core,*memory,*provider,stack],image,cenv);report['artifacts'][str(image)]=sha(image)
            checked=run('wide-'+mode,[image,out/'wide']);value=strict_json(checked.stdout)
            if checked.stderr or set(value)!={'schema','ok','large_cases','cross_member_history_rejected','state_bytes','output_chunk_bytes','wide_output_bytes','actual_max_input_bytes','readonly_input_tailguard','output_tailguard','cases'}or value['ok']is not True or type(value['schema'])is not int or value['schema']!=1 or type(value['large_cases'])is not int or value['large_cases']!=6 or value['cross_member_history_rejected']is not True or value['state_bytes']!=46080 or value['output_chunk_bytes']!=32749 or value['wide_output_bytes']!=68157440 or value['actual_max_input_bytes']!=67108864 or value['readonly_input_tailguard']is not True or value['output_tailguard']is not True or len(value['cases'])!=6:raise ValueError('actual wide execution mode/stats differ')
            cases=value['cases'];wire_bytes=wide['files'][0]['input_bytes'];member_bytes=wide['files'][0]['member_bytes']
            for index,row in enumerate(cases):
                if set(row)!={'calls','decoded','consumed','members'}or any(type(v)is not int or v<0 for v in row.values())or row['calls']<1:raise ValueError('wide exact integer counters differ')
                if row['members']!=[65,64,1,1,64,1][index]:raise ValueError('wide member-budget oracle differs')
                if index==1:
                    # Failure publishes no result; a bounded correct prefix is
                    # allowed. Do not derive an exact prefix oracle from SG.
                    if not 67108864<=row['decoded']<=68157439 or not 0<=row['consumed']<=wire_bytes:raise ValueError('wide budget failure prefix exceeds its independent bounds')
                elif row['decoded']!=[68157440,0,1048576,1048576,68157440,67103726][index]or row['consumed']!=[wire_bytes,0,member_bytes,member_bytes,wire_bytes-1,67108864][index]:raise ValueError('wide byte-consumption oracle differs')
            large_results[mode]=value
        if results['production']!={**results['instrumented'],'mode':'production','history_reads':None,'crc_output_reads':None}or large_results['production']!=large_results['instrumented']:raise ValueError('instrumented and actual adapters disagree')
        report['probe_results']=results;report['wide_results']=large_results;report['wide_inputs']=wide;report['state_bytes']=46080;report['heap_allocations_in_new_algorithm']=0;report['no_allocation_control']={'actual_forced_allocation_exit':99,'all_corpus_and_wide_probes_link_same_trapping_provider':True,'no_decoder_allocation_call_reached':True};report['known_failure_prefix_compared']=True;report['old_production_source_pins']={name:gzip['sources'][str(Path(name))]for name in baseline};report['qualification_scope']='complete UTF8 tool source branch; actual old raw core source/object identity; immutable gzip corpus and actual Rust/Python comparison; current new framing-source execution, not requalification of changed historical child documentation';report['result']='passed'
    except BaseException as error:report['result']='failed';report['error']=repr(error);raise
    finally:
        for group in ['sources','external_sources']:
            report['changed_'+group]=[n for n,h in report[group].items()if not(ROOT/n if group=='sources'else Path(n)).is_file()or sha(ROOT/n if group=='sources'else Path(n))!=h]
        report['changed_tools']=[n for n,v in report['tools'].items()if not Path(v['path']).is_file()or sha(Path(v['path']))!=v['sha256']]
        report['changed_artifacts']=[n for n,h in report['artifacts'].items()if not Path(n).is_file()or sha(Path(n))!=h]
        if any(v for k,v in report.items()if k.startswith('changed_')):report['result']='failed'
        report['revision_end']=revision();report['git_status_end']=git_status();report['completed_at']=dt.datetime.now(dt.timezone.utc).isoformat();save()
    if report['result']!='passed':raise SystemExit(1)
if __name__=='__main__':main()
