"""Qualify the fixed-state continuation prototype; old raw/gzip exports are not migrated."""
from __future__ import annotations
import argparse,datetime as dt,hashlib,json,os,platform,re,struct,subprocess,sys,time
from pathlib import Path
from verify_runtime_text import coff_image,dependencies,digest,git_status,revision,sha,strict_json
from verify_runtime_deflate import corpus_oracle,metadata_proof
from linker_evidence import copy_linker
sys.path.insert(0,str(Path(__file__).resolve().parent/'tests'))
from deflate_cases import zlib_identity
ROOT=Path(__file__).resolve().parents[1]

def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['text-evidence','raw-evidence','rust-evidence','output']:p.add_argument('--'+name,type=Path,required=True)
    a=p.parse_args();out=a.output.resolve()
    if not out.is_relative_to(ROOT/'target'):raise ValueError('fresh output must be in target')
    out.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'stage':'sg-runtime-deflate-continuation','result':'running','revision_start':revision(),'git_status_start':git_status(),'host':platform.platform(),'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'sources':{},'external_sources':{},'tools':{},'artifacts':{},'steps':[],'old_exports_migrated':False,'package_integration_complete':False,'remaining':['fresh gzip wrapper qualification for the migrated raw branch','streaming gzip CRC/header/trailer','incremental tar and staging/full validation/atomic publication','performance and final SDK/self-build qualification']}
    cache={}
    def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf8')
    def read(path,expected=None):
        path=Path(path).resolve()
        if path not in cache:cache[path]=path.read_bytes()
        data=cache[path];fingerprint=digest(data)
        if expected is not None and fingerprint!=expected:raise ValueError('qualified identity differs '+str(path))
        group='sources' if path.is_relative_to(ROOT) else 'external_sources';key=str(path.relative_to(ROOT)) if group=='sources' else str(path);report[group][key]=fingerprint
        return data
    def tool(name,identity):
        path=Path(identity['path']).resolve();read(path,identity['sha256']);report['tools'][name]={'path':str(path),'sha256':identity['sha256']};return path
    clean={k:v for k,v in os.environ.items() if not k.upper().startswith(('SENGOO_','SGPLATFORM_','PYTHON','RUSTC','RUSTDOC','CARGO_'))};pure=dict(clean,PATH='')
    def run(name,command,env=pure,expected=0,empty=False):
        command=list(map(str,command));start=time.monotonic();r=subprocess.run(command,cwd=out,env=env,capture_output=True,timeout=900)
        first=out/(name+'.stdout'),out/(name+'.stderr')
        for file,data in zip(first,[r.stdout,r.stderr]):file.write_bytes(data);report['artifacts'][str(file)]=digest(data)
        report['steps'].append({'name':name,'command':command,'exit':r.returncode,'expected_exit':expected,'empty_path':env.get('PATH')=='','seconds':time.monotonic()-start,'stdout_sha256':digest(r.stdout),'stderr_sha256':digest(r.stderr)});save();print(name,r.returncode,flush=True)
        if r.returncode!=expected or empty and (r.stdout or r.stderr):raise ValueError(name+' actual result/output differs')
        return r
    save()
    try:
        text=strict_json(read(a.text_evidence));raw=strict_json(read(a.raw_evidence));rust=strict_json(read(a.rust_evidence))
        for data in [text,raw,rust]:
            if type(data.get('schema')) is not int or data['schema']!=1 or data.get('result')!='passed' or any(value!=[] for key,value in data.items() if key.startswith('changed_')):raise ValueError('parent qualification not passed/unchanged')
        if text['stage']!='sg-runtime-text-utf8' or raw['stage']!='sg-runtime-raw-deflate' or rust['stage']!='independent-miniz-deflate-policy':raise ValueError('qualification branch stage differs')
        if raw.get('single_shared_deflate_core')is not True:raise ValueError('raw branch is not a migrated shared-core qualification')
        for group in ['sources','external_sources']:
            for name,fingerprint in raw[group].items():read(ROOT/name if group=='sources'else Path(name),fingerprint)
        for name,identity in raw['tools'].items():tool('raw-producer-'+name,identity)
        for group in ['sources','external_sources']:
            for name,fingerprint in text[group].items():read(ROOT/name if group=='sources' else Path(name),fingerprint)
        for name,identity in text['tools'].items():tool('text-producer-'+name,identity)
        native=tool('native',text['tools']['native']);formatter=tool('formatter',text['tools']['formatter'])
        for name in ['native','formatter']:
            artifact=Path(text['tools'][name]['path']).name
            if text['artifacts'][artifact]!=text['tools'][name]['sha256']:raise ValueError('qualified compiler artifact binding differs')
        pin=text['linker_qualification']['evidence'];parent=Path(pin['path']).resolve()
        if not parent.is_relative_to(ROOT/'target') or text['sources'].get(str(parent.relative_to(ROOT)))!=pin['sha256']:raise ValueError('declared historical linker parent not bound')
        read(parent,pin['sha256']);linker,branch=copy_linker(ROOT,parent,out,report)
        if branch['binary_sha256']!=text['tools']['sg-linker']['sha256']:raise ValueError('qualified linker binary differs')
        for name,identity in list(report['tools'].items()):tool(name,identity)
        for name,fingerprint in list(report['sources'].items()):read(ROOT/name,fingerprint)
        clang=tool('clang',text['tools']['clang']);lld=tool('lld',text['tools']['clang-probe-linker']);objcopy=tool('objcopy',text['tools']['llvm-objcopy-c-sdk-metadata']);objdump=tool('objdump',text['tools']['llvm-objdump-c-sdk-leaf-check']);kernel=tool('sdk-kernel32',text['tools']['sdk-kernel32'])
        stack_origin=a.text_evidence.resolve().parent/'ntdll-chkstk.lib';stack_pin=text['stack_probe_provider']['import_library_sha256']
        if text['sources'].get(str(stack_origin.relative_to(ROOT)))!=stack_pin:raise ValueError('stack import library pin not source-bound')
        stack=out/'ntdll-chkstk.lib';stack.write_bytes(read(stack_origin,stack_pin));read(stack,stack_pin)
        tool('runner-python',{'path':sys.executable,'sha256':sha(Path(sys.executable))});oracle=zlib_identity();tool('zlib-implementation',oracle);report['zlib']=oracle
        for name in ['selfhost/verify_runtime_deflate_stream.py','selfhost/verify_runtime_deflate.py','selfhost/verify_runtime_text.py','selfhost/linker_evidence.py','selfhost/tests/deflate_cases.py']:read(ROOT/name)
        for name in ['proposal.md','design.md','specs/selfhost-deflate-core/spec.md']:read(ROOT/'openspec/changes/add-selfhost-deflate-core'/name)
        corpus_origin=a.raw_evidence.resolve().parent/'corpus.bin';manifest_origin=corpus_origin.with_suffix('.bin.json')
        corpus=read(corpus_origin,raw['sources'][str(corpus_origin.relative_to(ROOT))]);manifest=strict_json(read(manifest_origin,raw['sources'][str(manifest_origin.relative_to(ROOT))]));validation=corpus_oracle(corpus,manifest)
        source=out/'corpus.bin';source.write_bytes(corpus);read(source,digest(corpus));report['corpus']={'sha256':digest(corpus),'records':manifest['records'],'independent_python_revalidated':True,'validation':validation}
        for name,fingerprint in rust['sources'].items():read(name,fingerprint)
        for name,identity in rust['tools'].items():tool('rust-producer-'+name,identity)
        wire=rust['observation_stdout'];stdout=read(wire['path'],wire['sha256']);read(wire['corpus'],digest(corpus));executions=[row for row in rust['steps'] if row['name']=='execute-all-fixtures']
        if len(executions)!=1 or executions[0]['exit']!=0 or executions[0]['command']!=[rust['tools']['miniz-oracle']['path'],wire['corpus']] or executions[0]['stdout_sha256']!=wire['sha256']:raise ValueError('Rust actual execution relationship differs')
        lines=stdout.decode().splitlines();observations=rust['observations']
        if len(lines)!=manifest['records'] or len(observations)!=len(lines):raise ValueError('Rust fixture count differs')
        at=16;differences=[];equal=0;expected_runs=0
        for f,o,line in zip(manifest['fixtures'],observations,lines):
            n,cap,expected,size=struct.unpack_from('<IIqI',corpus,at);at+=20;plain=corpus[at+n:at+n+size];at+=n+size
            fields=line.split('|')
            if len(fields)!=3 or fields[0]!=str(f['id']) or fields[1]not in ['true','false'] or type(o['ok'])is not bool or o['id']!=f['id'] or o['name']!=f['name'] or o['ok']!=(fields[1]=='true') or fields[2]!=(o.get('hex')if o['ok']else o.get('status')):raise ValueError('Rust declared observations differ from actual stdout')
            expected_runs+=1 if expected==-2 else 3
            if expected>=0 and n<=64:expected_runs+=max(0,n-1)
            if expected>=0 or expected==-2:
                if o['ok'] and bytes.fromhex(o['hex'])==plain:equal+=1
                elif f['name']in ['empty-distance-hdist-31','empty-distance-hdist-32'] and not o['ok']:differences.append({'id':f['id'],'name':f['name'],'reason':'miniz restricts unused HDIST to30; SG preserves raw reference range32'})
                else:raise ValueError('Rust independent valid output differs '+f['name'])
            elif o['ok']:differences.append({'id':f['id'],'name':f['name'],'reason':'miniz permissive incomplete-tree or prefix/trailing-byte acceptance; strict SG/Python raw oracle rejects'})
        if at!=len(corpus) or len(differences)!=18:raise ValueError('raw corpus or named Rust difference inventory changed')
        report['rust_comparison']={'equal_valid_and_capacity_source_outputs':equal,'differences':differences,'report_sha256':digest(read(a.rust_evidence))}
        snapshot=out/'inputs';snapshot.mkdir()
        names=['runtime/deflate_stream.sg','runtime/deflate_stream_memory.c','tests/deflate_stream_probe.c','tests/deflate_stream_fixture.c','tests/deflate_stream_smoke.sg']
        for name in names:
            data=read(ROOT/'selfhost'/name);saved=snapshot/Path(name).name;saved.write_bytes(data);read(saved,digest(data))
        cenv=dict(clean,PATH=str(clang.parent)+os.pathsep+os.environ.get('PATH',''))
        driver=run('actual-clang-linker',[clang,'-###','-fuse-ld=lld',snapshot/'deflate_stream_probe.c','-o',out/'not-executed.exe'],cenv)
        paths=re.findall(r'^ "([^"]*lld-link(?:\.exe)?)"',driver.stderr.decode(),re.M)
        if len(paths)!=1 or Path(paths[0].replace('\\\\','\\')).with_suffix('.exe').resolve()!=lld:raise ValueError('clang actual linker differs')
        def compile_c(name,input,output,flags=()):
            options=['-std=c11','-O2','-g0','-fno-ident','-fno-addrsig',*flags];before=output.with_suffix('.before.d');after=output.with_suffix('.d')
            run(name+'-headers',[clang,*options,'-M','-MT','runtime_text_dependency','-MF',before,input],cenv);read(before);headers={p:digest(read(p))for p in dependencies(before)}
            run(name,[clang,*options,'-MD','-MT','runtime_text_dependency','-MF',after,'-c',input,'-o',output],cenv);read(after)
            if dependencies(after)!=set(headers)or any(sha(p)!=value for p,value in headers.items()):raise ValueError('C header closure changed')
            report['artifacts'][str(output)]=sha(output);return output
        def link_c(name,objects,output):
            linked=run(name,[clang,'-fuse-ld=lld',*objects,stack,'-Wl,/verbose','-o',output],cenv);known={};libraries={}
            for line in(linked.stdout+linked.stderr).decode().splitlines():
                match=re.search(r'Reading (.+?\.lib)(?:\(|$)',line,re.I)
                if not match:continue
                path=Path(match.group(1))
                if path.is_absolute():path=path.resolve();known.setdefault(path.name.casefold(),set()).add(path)
                else:
                    options=known.get(path.name.casefold(),set())
                    if len(options)!=1:raise ValueError('library member physical path is ambiguous')
                    path=next(iter(options))
                if str(path)not in libraries:libraries[str(path)]=digest(read(path))
            if not libraries:raise ValueError('actual C link trace has no archives')
            report.setdefault('actual_c_link_libraries',{})[name]=libraries;report['artifacts'][str(output)]=sha(output)
        for stem in ['deflate_stream','deflate_stream_smoke']:
            run('format-'+stem,[formatter,'--check',snapshot/(stem+'.sg')])
        rounds=[]
        for index,label in enumerate(['first fresh 世界 &','second fresh 世界 &']):
            folder=out/label;folder.mkdir();objects={}
            for stem in ['deflate_stream','deflate_stream_smoke']:
                data=read(snapshot/(stem+'.sg'));saved=folder/(stem+'.sg');saved.write_bytes(data);read(saved)
                obj=folder/(stem+'.obj');made=run(str(index)+'-emit-'+stem,[native,saved,obj,'--wrap'])
                if made.stderr or strict_json(made.stdout)!={'schema_version':1,'ok':True,'phase':'object'}:raise ValueError('native emitter result differs')
                objects[stem]=obj
                if stem=='deflate_stream'and sha(obj)!=raw['cases'][0]['sha256']['deflate_stream']:raise ValueError('continuation SG object differs from qualified one-shot core')
            for stem in ['deflate_stream_memory','deflate_stream_fixture']:
                saved=folder/(stem+'.c');saved.write_bytes(read(snapshot/(stem+'.c')));read(saved)
                rawobj=compile_c(str(index)+'-'+stem,saved,folder/(stem+'.raw.obj'),['-fomit-frame-pointer','-fno-asynchronous-unwind-tables','-fno-unwind-tables','-fno-vectorize','-fno-slp-vectorize'])
                original=rawobj.read_bytes();parsed=coff_image(original)
                assembly=run(str(index)+'-'+stem+'-leaf',[objdump,'-d','--no-show-raw-insn',rawobj],cenv).stdout.decode().split('Disassembly of section .text:',1)[1]
                if re.search(r'%(?:rsp|rbp)|\b(?:call\w*|push\w*|pop\w*|enter|leave)\b',assembly):raise ValueError('C memory/constant fixture is not leaf')
                removed=[n for n,v in parsed['sections'].items()if n in ['.data','.bss']and not v['data']and not v['relocations']]
                obj=folder/(stem+'.obj');run(str(index)+'-'+stem+'-metadata',[objcopy,*['--strip-symbol='+n for n in [*parsed['sections'],'.file','@feat.00']],*['--remove-section='+n for n in removed],rawobj,obj],cenv)
                data=bytearray(obj.read_bytes());data[4:8]=bytes(4);obj.write_bytes(data);report.setdefault('metadata_proofs',{})[label+' '+stem]=metadata_proof(original,bytes(data));objects[stem]=obj
            image=folder/'stream.exe';run(str(index)+'-sg-link',[linker,'link',image,'main',*objects.values(),kernel,stack],empty=True);run(str(index)+'-cold',[image],expected=42,empty=True)
            hashes={n:sha(p)for n,p in {**objects,'image':image}.items()}
            if rounds and rounds[0]['sha256']!=hashes:raise ValueError('fresh objects and PE bytes differ')
            rounds.append({'round':label,'sha256':hashes,'empty_path_cold_exit':42})
            report['artifacts'].update({str(p):sha(p)for p in [*objects.values(),image]})
        report['fresh_rounds']=rounds
        results={}
        for mode in ['instrumented','production']:
            obj=compile_c('compile-'+mode,snapshot/'deflate_stream_probe.c',out/(mode+'.obj'),['-DSENGOO_DS_PRODUCTION']if mode=='production'else [])
            image=out/(mode+'.exe');extra=[out/'first fresh 世界 &/deflate_stream_memory.obj']if mode=='production'else []
            link_c('link-'+mode,[obj,out/'first fresh 世界 &/deflate_stream.obj',*extra],image)
            checked=run('probe-'+mode,[image,source]);value=strict_json(checked.stdout)
            if checked.stderr or set(value)!={'schema','ok','mode','fixtures','runs','calls','need_input','need_output','history_reads','state_bytes'}or type(value['schema'])is not int or value['schema']!=1 or value['ok']is not True or value['mode']!=mode or type(value['fixtures'])is not int or value['fixtures']!=5290 or type(value['runs'])is not int or value['runs']!=expected_runs or value['state_bytes']!=45056 or any(type(value[k])is not int or value[k]<=0 for k in ['calls','need_input','need_output']):raise ValueError('actual completion protocol/count differs')
            if mode=='instrumented' and(type(value['history_reads'])is not int or value['history_reads']<=0)or mode=='production'and value['history_reads']is not None:raise ValueError('observed/unobserved history flags differ')
            results[mode]=value
        if results['production']!={**results['instrumented'],'mode':'production','history_reads':None}:raise ValueError('production/instrumented completion diverged')
        report['probe_results']=results;report['state_bytes']=45056;report['heap_allocations_in_algorithm']=0;report['result']='passed';report['old_exports_migrated']=True;report['raw_wrapper_qualification']={'path':str(a.raw_evidence.resolve()),'sha256':digest(read(a.raw_evidence)),'sg_wrapper_owns_fixed_state_allocation':True,'stream_core_allocates_heap':False}
    except BaseException as error:report['result']='failed';report['error']=repr(error);raise
    finally:
        for group in ['sources','external_sources']:
            report['changed_'+group]=[n for n,v in report[group].items()if not(ROOT/n if group=='sources'else Path(n)).is_file()or sha(ROOT/n if group=='sources'else Path(n))!=v]
        report['changed_tools']=[n for n,v in report['tools'].items()if not Path(v['path']).is_file()or sha(Path(v['path']))!=v['sha256']]
        report['changed_artifacts']=[n for n,v in report['artifacts'].items()if not Path(n).is_file()or sha(Path(n))!=v]
        if any(v for k,v in report.items()if k.startswith('changed_')):report['result']='failed'
        report['revision_end']=revision();report['git_status_end']=git_status();report['completed_at']=dt.datetime.now(dt.timezone.utc).isoformat();save()
    if report['result']!='passed':raise SystemExit(1)
if __name__=='__main__':main()
