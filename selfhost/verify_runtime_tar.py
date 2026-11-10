"""Source-bound development proof for the SG tar scanner (not package qualification)."""
from __future__ import annotations
import argparse,ctypes,datetime as dt,hashlib,json,os,platform,re,struct,subprocess,sys,tarfile,time
from pathlib import Path
from verify_runtime_text import strict_json,revision,git_status,digest,sha,dependencies,coff_image
from verify_runtime_deflate import metadata_proof
from linker_evidence import copy_linker

ROOT=Path(__file__).resolve().parents[1]
def main():
    p=argparse.ArgumentParser(description=__doc__);p.add_argument('--gzip-evidence',required=True,type=Path);p.add_argument('--output',required=True,type=Path);mode=p.add_mutually_exclusive_group(required=True);mode.add_argument('--development',action='store_true');mode.add_argument('--formal',action='store_true');p.add_argument('--rust-evidence',type=Path);p.add_argument('--debugger-evidence',type=Path);a=p.parse_args()
    if a.formal and (not a.rust_evidence or not a.debugger_evidence):raise ValueError('formal requires Rust and debugger qualification evidence')
    out=a.output.resolve()
    if not out.is_relative_to(ROOT/'target'):raise ValueError('output must be fresh target directory')
    out.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'stage':'sg-runtime-tar-scan' if a.formal else 'sg-tar-development','result':'running','started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'host':platform.platform(),'revision_start':revision(),'git_status_start':git_status(),'sources':{},'external_sources':{},'tools':{},'steps':[],'package_integration_complete':False,'formal_qualification_complete':False,'remaining':['gzip streaming256MiB/staging/manifest/publication','installed development SDK replacement/packaging/performance qualification']}
    cache={}
    def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf8')
    def read(path,expected=None):
        path=Path(path).resolve()
        if path not in cache:cache[path]=path.read_bytes()
        raw=cache[path];fingerprint=digest(raw)
        if expected is not None and fingerprint!=expected:raise ValueError('identity differs: '+str(path))
        if path.is_relative_to(ROOT):report['sources'][str(path.relative_to(ROOT))]=fingerprint
        else:report['external_sources'][str(path)]=fingerprint
        return raw
    def tool(name,identity):
        path=Path(identity['path']).resolve();read(path,identity['sha256']);report['tools'][name]=identity;return path
    clean={k:v for k,v in os.environ.items() if not k.upper().startswith(('SENGOO_','SGPLATFORM_','PYTHON','RUSTC','RUSTDOC','CARGO_'))};pure=dict(clean,PATH='')
    def run(name,args,env=pure,expected=0):
        start=time.monotonic();r=subprocess.run(list(map(str,args)),cwd=out,env=env,capture_output=True,timeout=900)
        first=out/(name+'.stdout');second=out/(name+'.stderr');first.write_bytes(r.stdout);second.write_bytes(r.stderr)
        report.setdefault('artifacts',{}).update({str(first):sha(first),str(second):sha(second)})
        report['steps'].append({'name':name,'command':list(map(str,args)),'exit':r.returncode,'expected_exit':expected,'seconds':time.monotonic()-start,'stdout_sha256':sha(first),'stderr_sha256':sha(second)});save();print(name,r.returncode,flush=True)
        if r.returncode!=expected:raise AssertionError(name+': '+r.stdout.decode(errors='replace')+' '+r.stderr.decode(errors='replace'))
        return r
    save()
    try:
        prior=strict_json(read(a.gzip_evidence))
        if type(prior.get('schema')) is not int or prior['schema']!=1 or prior.get('result')!='passed' or prior.get('stage')!='sg-runtime-gzip-framing' or any(prior.get(key)!=[] for key in ['changed_sources','changed_external_sources','changed_tools','checkout_changes','changed_cargo_configs']):raise ValueError('gzip evidence not passed/unchanged')
        native=tool('native',prior['tools']['native']);clang=tool('clang',prior['tools']['clang']);lld=tool('lld',prior['tools']['lld']);tool('python',{'path':sys.executable,'sha256':sha(Path(sys.executable))})
        if a.formal:
            read(Path(tarfile.__file__))
            for group in ['sources','external_sources']:
                for name,expected in prior[group].items():read(Path(name) if group=='external_sources' else ROOT/name,expected)
            for name,identity in prior['tools'].items():tool('gzip-producer-'+name,identity)
            read(ROOT/'selfhost/verify_runtime_text.py');read(ROOT/'selfhost/verify_runtime_deflate.py');read(ROOT/'selfhost/linker_evidence.py')
            if a.rust_evidence:
                qualified=strict_json(read(a.rust_evidence))
                if qualified.get('result')!='passed' or any(qualified.get(key)!=[] for key in ['changed_sources','changed_tools','changed_cargo_configs']):raise ValueError('Rust evidence not passed unchanged')
                for name,expected in qualified['sources'].items():read(name,expected)
                for name,identity in qualified['tools'].items():tool('rust-preflight-'+name,identity)
            debug=strict_json(read(a.debugger_evidence))
            if debug.get('result')!='passed':raise ValueError('debugger source qualification not passed')
            debug_python=Path(debug['tools']['debug-dependency-python310.dll']['path']).resolve().parent
            for name,identity in debug['tools'].items():
                if name.startswith('debug-dependency-'):tool(name,identity)
            for path in [*debug_python.glob('*.pyd'),*debug_python.glob('*.dll'),debug_python/'python310.zip']:read(path)
            lldb=tool('lldb',{'path':str(clang.parent/'lldb.exe'),'sha256':sha(clang.parent/'lldb.exe')})
            tool('liblldb',{'path':str(clang.parent/'liblldb.dll'),'sha256':sha(clang.parent/'liblldb.dll')})
            for name in ['proposal.md','design.md','specs/selfhost-package-archive/spec.md']:read(ROOT/'openspec/changes/add-selfhost-package-archive'/name)
            for name,expected in prior['artifacts'].items():read(a.gzip_evidence.resolve().parent/name,expected)
            for name in ['native','formatter']:
                if prior['artifacts'].get(name+'.exe')!=prior['tools'][name]['sha256']:raise ValueError('qualified binary artifact relationship differs')
            pin=prior['linker_qualification']['evidence'];parent=Path(pin['path']).resolve()
            if not parent.is_relative_to(ROOT/'target') or prior['sources'].get(str(parent.relative_to(ROOT)))!=pin['sha256']:raise ValueError('gzip declared linker parent pin is not bound')
            read(parent,pin['sha256']);linker,branch=copy_linker(ROOT,parent,out,report)
            if branch['binary_sha256']!=prior['tools']['sg-linker']['sha256']:raise ValueError('historical SG linker artifact differs')
            for name,identity in list(report['tools'].items()):tool(name,identity)
            for name,expected in list(report['sources'].items()):read(ROOT/name,expected)
            formatter=tool('formatter',prior['tools']['formatter']);kernel=tool('sdk-kernel32',prior['tools']['sdk-kernel32'])
            objcopy=tool('objcopy',prior['tools']['objcopy']);objdump=tool('objdump',prior['tools']['objdump']);readobj=tool('readobj',prior['tools']['readobj'])
        stack=Path(prior['steps'][next(i for i,r in enumerate(prior['steps']) if r['name']=='link-instrumented')]['command'][-3])
        # Pin the declared import library rather than discovering one by basename alone.
        if stack.name!='ntdll-chkstk.lib':raise ValueError('stack qualification relationship differs')
        stack_raw=read(stack,prior['sources'][str(stack.relative_to(ROOT))]);(out/'ntdll-chkstk.lib').write_bytes(stack_raw)
        snapshot=out/'inputs';snapshot.mkdir()
        names=['runtime/tar.sg','runtime/tar_memory.c','tests/tar_probe.c','tests/tar_cases.py','tests/tar_large_probe.c','tests/tar_large_cases.py','verify_runtime_tar.py']
        if a.formal:names+=['runtime/tar_sdk.c','tests/tar_unwind_probe.c','tests/tar_fixture.c','tests/tar_fixture.py','tests/tar_smoke.sg']
        for name in names:
            path=ROOT/'selfhost'/name;raw=read(path);target=snapshot/Path(name).name;target.write_bytes(raw);read(target,digest(raw))
        run('generate-corpus',[sys.executable,'-B','-X','utf8',snapshot/'tar_cases.py','--output',out/'corpus'])
        manifest=strict_json(read(out/'corpus/corpus.json'));read(out/'corpus/corpus.bin',manifest['corpus_sha256'])
        report['corpus']={'path':str(out/'corpus/corpus.bin'),'sha256':manifest['corpus_sha256'],'records':manifest['records']}
        made=run('emit-tar',[native,snapshot/'tar.sg',out/'tar.obj','--wrap'])
        if strict_json(made.stdout)!={'schema_version':1,'ok':True,'phase':'object'}:raise ValueError('emit result differs')
        cenv=dict(clean,PATH=str(clang.parent)+os.pathsep+os.environ.get('PATH',''))
        def compile_c(name,source,target,extra=(),leaf=False):
            flags=['-std=c11','-O2','-g0','-fno-ident','-fno-addrsig',*extra]
            if leaf:flags+=['-fomit-frame-pointer','-fno-asynchronous-unwind-tables','-fno-unwind-tables','-fno-vectorize','-fno-slp-vectorize']
            before=target.with_suffix('.before.d');after=target.with_suffix('.d')
            run(name+'-headers',[clang,*flags,'-M','-MT','runtime_text_dependency','-MF',before,source],cenv)
            read(before)
            headers={path:digest(read(path)) for path in dependencies(before)}
            run(name,[clang,*flags,'-MD','-MT','runtime_text_dependency','-MF',after,'-c',source,'-o',target],cenv)
            read(after);report.setdefault('artifacts',{})[str(target)]=sha(target)
            if set(headers)!=dependencies(after) or any(sha(path)!=value for path,value in headers.items()):raise ValueError('C actual header dependencies changed')
            report.setdefault('c_headers',{})[name]={str(path):value for path,value in headers.items()}
            return target
        def link_c(name,objects,target,extra=(),nostdlib=False):
            flags=['-fuse-ld=lld',*(['-nostdlib'] if nostdlib else [])]
            made=run(name,[clang,*flags,*objects,*extra,'-Wl,/verbose','-o',target],cenv)
            output=(made.stdout+made.stderr).decode(errors='strict');libraries={};known={}
            for line in output.splitlines():
                match=re.search(r'Reading (.+?\.lib)(?:\(|$)',line,re.I)
                if match:
                    declared=Path(match.group(1))
                    if declared.is_absolute():
                        path=declared.resolve();known.setdefault(path.name.casefold(),set()).add(path)
                    else:
                        candidates=known.get(declared.name.casefold(),set())
                        if len(candidates)!=1:raise ValueError('library member has no unique prior physical archive: '+str(declared))
                        path=next(iter(candidates))
                    if not path.is_file():raise ValueError('actual linked library not found: '+str(path))
                    libraries[str(path)]={'path':str(path),'sha256':digest(read(path))}
            if not libraries:raise ValueError('actual C linker provided no library input trace')
            report.setdefault('actual_c_link_libraries',{})[name]=list(libraries.values());report.setdefault('artifacts',{})[str(target)]=sha(target)
            return target
        driver=run('verify-driver-linker',[clang,'-###','-fuse-ld=lld','-O2',snapshot/'tar_probe.c','-o',out/'not-executed.exe'],cenv)
        selected=re.findall(r'^ "([^"]*lld-link(?:\.exe)?)"',driver.stderr.decode(),re.M)
        if len(selected)!=1 or Path(selected[0].replace('\\\\','\\')).with_suffix('.exe').resolve()!=lld:raise ValueError('clang selected a different actual linker')
        if a.formal:
            instrumented_probe=compile_c('instrumented-probe',snapshot/'tar_probe.c',out/'probe.obj',['-DSENGOO_TAR_INSTRUMENTED'])
            instrumented_memory=compile_c('instrumented-memory',snapshot/'tar_memory.c',out/'instrumented-memory.obj',['-DSENGOO_TAR_INSTRUMENTED'])
            link_c('link-probe',[instrumented_probe,instrumented_memory,out/'tar.obj',out/'ntdll-chkstk.lib'],out/'probe.exe')
        else:run('link-probe',[clang,'-fuse-ld=lld','-O2','-DSENGOO_TAR_INSTRUMENTED',snapshot/'tar_probe.c',snapshot/'tar_memory.c',out/'tar.obj',out/'ntdll-chkstk.lib','-o',out/'probe.exe'],cenv)
        actual=run('execute-probe',[out/'probe.exe',out/'corpus/corpus.bin']);observed=strict_json(actual.stdout)
        successes=sum(row['expected']>=0 for row in manifest['fixtures']);rejections=manifest['records']-successes
        if actual.stderr or observed.get('ok') is not True or observed.get('mode')!='instrumented' or observed.get('fixtures')!=manifest['records'] or observed.get('calls')!=manifest['records']*3 or observed.get('guard_calls')!=manifest['records']*2 or observed.get('successes')!=successes*3 or observed.get('rejections')!=rejections*3 or observed.get('invalid_extents')!=6 or observed.get('failure_header_unchanged') is not True:raise ValueError('completed fixture count or oracle differs')
        done=1469598103934665603
        for fixture in manifest['fixtures']:
            for guard in range(3):
                for byte in struct.pack('<Qq',fixture['id'],fixture['expected']):done=((done^byte)*1099511628211)&((1<<64)-1)
        if observed.get('completed_fnv64')!=format(done,'016x'):raise ValueError('fixture ID/result completion digest differs')
        report['probe']=observed;report.setdefault('artifacts',{}).update({str(out/name):sha(out/name) for name in ['tar.obj','probe.exe','ntdll-chkstk.lib']});report['result']='passed'
        if a.formal:
            production_probe=compile_c('production-probe',snapshot/'tar_probe.c',out/'production-probe.obj',['-DSENGOO_TAR_PRODUCTION'])
            production_memory=compile_c('production-memory',snapshot/'tar_memory.c',out/'production-memory.obj')
            link_c('link-production-probe',[production_probe,production_memory,out/'tar.obj',out/'ntdll-chkstk.lib'],out/'production.exe')
        else:run('link-production-probe',[clang,'-fuse-ld=lld','-O2','-DSENGOO_TAR_PRODUCTION',snapshot/'tar_probe.c',snapshot/'tar_memory.c',out/'tar.obj',out/'ntdll-chkstk.lib','-o',out/'production.exe'],cenv)
        product=run('execute-production-probe',[out/'production.exe',out/'corpus/corpus.bin']);production=strict_json(product.stdout)
        if product.stderr or production!={**observed,'mode':'production','reads':None,'writes':None}:raise ValueError('production guard/corpus results differ')
        report['production_probe']=production;report['artifacts'][str(out/'production.exe')]=sha(out/'production.exe')
        control=run('uninitialized-output-control',[out/'probe.exe','--uninitialized-control'],expected=1)
        if control.stdout or b'reason=output-uninitialized-read' not in control.stderr:raise ValueError('uninitialized output read control did not reject')
        corpus=bytearray((out/'corpus/corpus.bin').read_bytes());struct.pack_into('<q',corpus,12+12,-1);wrong=out/'wrong-oracle.bin';wrong.write_bytes(corpus);read(wrong)
        control=run('wrong-oracle-control',[out/'probe.exe',wrong],expected=1)
        if control.stdout or b'fixture=0 reason=result-oracle' not in control.stderr:raise ValueError('wrong expected tar result did not reject')
        run('generate-large-headers',[sys.executable,'-B','-X','utf8',snapshot/'tar_large_cases.py','--output',out/'large'])
        large=strict_json(read(out/'large/manifest.json'))
        for row in large['headers']:read(row['path'],row['sha256'])
        if a.formal:
            large_probe=compile_c('large-probe',snapshot/'tar_large_probe.c',out/'large-probe.obj')
            link_c('link-large-probe',[large_probe,production_memory,out/'tar.obj',out/'ntdll-chkstk.lib'],out/'large.exe')
        else:run('link-large-probe',[clang,'-fuse-ld=lld','-O2',snapshot/'tar_large_probe.c',snapshot/'tar_memory.c',out/'tar.obj',out/'ntdll-chkstk.lib','-o',out/'large.exe'],cenv)
        actual_large=run('execute-large-probe',[out/'large.exe',out/'large'])
        expected_large={'schema':1,'ok':True,'cases':3,'content_limit':268435456,'raw_extent_limit':536870912,'readonly_input':True,'tail_guards':True,'all_input_bytes_observed':False}
        if actual_large.stderr or strict_json(actual_large.stdout)!=expected_large:raise ValueError('actual tar large extent/content boundary differs')
        report['large_probe']=expected_large;report['artifacts'][str(out/'large.exe')]=sha(out/'large.exe')
        if a.rust_evidence:
            rust=strict_json(read(a.rust_evidence))
            if rust.get('stage')!='independent-tar-package-policy' or rust.get('result')!='passed' or any(rust.get(key)!=[] for key in ['changed_sources','changed_tools','changed_cargo_configs']):raise ValueError('Rust tar evidence not passed/unchanged')
            for name,fingerprint in rust['sources'].items():read(name,fingerprint)
            for name,identity in rust['tools'].items():tool('rust-'+name,identity)
            wire=rust['observation_stdout'];stdout=read(wire['path'],wire['sha256']);input_corpus=read(wire['corpus'],manifest['corpus_sha256'])
            executions=[row for row in rust['steps'] if row['name']=='execute-all-fixtures']
            if len(executions)!=1 or executions[0]['exit']!=0 or executions[0]['command']!=[rust['tools']['tar-oracle']['path'],wire['corpus']] or executions[0]['stdout_sha256']!=wire['sha256']:raise ValueError('Rust observations lack exact executed command')
            lines=stdout.decode('utf8').splitlines()
            if len(lines)!=manifest['records'] or len(rust['observations'])!=len(lines):raise ValueError('Rust tar fixture IDs differ')
            differences=[];positives=0
            for fixture,observation,line in zip(manifest['fixtures'],rust['observations'],lines):
                identifier,success,payload=line.split('|');entries=observation.get('entries',[])
                if identifier!=str(fixture['id']) or observation['id']!=fixture['id'] or observation['name']!=fixture['name'] or type(observation['ok']) is not bool or success not in ['true','false'] or observation['ok']!=(success=='true'):raise ValueError('Rust fixture record differs')
                if observation['ok']:
                    reconstructed=';'.join(','.join([row['path_hex'],str(row['kind']),str(row['declared']),str(row['effective']),str(row['header']),str(row['data']),row['content_hex']]) for row in entries)
                    if payload!=reconstructed:raise ValueError('Rust declared entries differ from actual stdout')
                elif bytes.fromhex(payload).decode('utf8')!=observation['error']:raise ValueError('Rust declared error differs from actual stdout')
                if fixture['expected']>=0:
                    expected=fixture['declared_entries'] if fixture['declared_entries'] is not None else fixture['python']['entries']
                    if not observation['ok'] or len(entries)!=len(expected):raise ValueError('Rust rejected SG positive '+fixture['name'])
                    normalize=lambda name:'/'.join(part for part in name.replace('\\','/').split('/') if part not in ['','.'])
                    for x,y in zip(expected,entries):
                        if normalize(x['name'])!=normalize(bytes.fromhex(y['path_hex']).decode('utf8')) or x['size']!=y['effective'] or x['data']!=y['data'] or x['content_sha256']!=digest(bytes.fromhex(y['content_hex'])):raise ValueError('Rust positive entry metadata differs '+fixture['name'])
                    positives+=1
                elif observation['ok']:
                    reason={-1:'strict framing/numeric/PAX validation',-2:'caller arena capacity',-4:'strict Windows path safety',-5:'package file/directory type policy',-6:'normalized identity/file-prefix conflict',-7:'resource budget'}[fixture['expected']]
                    differences.append({'id':fixture['id'],'name':fixture['name'],'sg':fixture['expected'],'rust_tar_parser_accepts':True,'reason':reason})
            report['rust_comparison']={'positive_metadata_equal':positives,'parser_vs_package_profile_differences':differences,'feature_scope':rust['feature_scope']}
        if a.formal:
            for source in [snapshot/'tar.sg',snapshot/'tar_smoke.sg']:
                run('format-check-'+source.stem,[formatter,'--check',source])
                preview=run('format-preview-'+source.stem,[formatter,source])
                saved=out/(source.stem+'.preview.sg');saved.write_bytes(preview.stdout);read(saved)
                run('format-preview-check-'+source.stem,[formatter,'--check',saved])
            rebuilt=out/'fixture-rebuilt.c';run('rebuild-constant-fixture',[sys.executable,'-B','-X','utf8',snapshot/'tar_fixture.py','--corpus',out/'corpus/corpus.bin','--output',rebuilt])
            if read(rebuilt)!=read(snapshot/'tar_fixture.c'):raise ValueError('committed fixture bytes do not match independent corpus')
            rounds=[];providers=[]
            for index,label in enumerate(['first fresh 世界 &','second fresh 世界 &']):
                directory=out/label;directory.mkdir()
                for name in ['tar.sg','tar_smoke.sg','tar_memory.c','tar_sdk.c','tar_fixture.c','tar_unwind_probe.c']:
                    saved=directory/name;saved.write_bytes(read(snapshot/name));read(saved)
                objects={}
                for stem in ['tar','tar_smoke']:
                    made=run(str(index)+'-emit-'+stem,[native,directory/(stem+'.sg'),directory/(stem+'.obj'),'--wrap'])
                    if made.stderr or strict_json(made.stdout)!={'schema_version':1,'ok':True,'phase':'object'}:raise ValueError('fresh SG tar object result differs')
                    objects[stem]=directory/(stem+'.obj')
                raw_sdk=compile_c(str(index)+'-sdk-object',directory/'tar_sdk.c',directory/'tar_sdk.raw.obj')
                raw_sdk_bytes=raw_sdk.read_bytes();normalized_sdk=bytearray(raw_sdk_bytes);normalized_sdk[4:8]=bytes(4)
                if len(raw_sdk_bytes)<20 or struct.unpack_from('<H',raw_sdk_bytes)[0]!=0x8664 or struct.unpack_from('<H',raw_sdk_bytes,16)[0]!=0 or bytes(normalized_sdk[:4])+raw_sdk_bytes[4:8]+bytes(normalized_sdk[8:])!=raw_sdk_bytes:raise ValueError('SDK timestamp normalization changed real unwind/code/data')
                section_count=struct.unpack_from('<H',raw_sdk_bytes,2)[0]
                if len(raw_sdk_bytes)<20+section_count*40:raise ValueError('SDK section headers truncated')
                section_names={raw_sdk_bytes[20+n*40:28+n*40].rstrip(b'\0') for n in range(section_count)}
                sdkobj=directory/'tar_sdk.obj';sdkobj.write_bytes(normalized_sdk)
                if not {b'.pdata',b'.xdata'}<=section_names:raise ValueError('SDK nonleaf unwind sections absent')
                report.setdefault('sdk_timestamp_proofs',{})[label]={'raw_sha256':digest(raw_sdk_bytes),'raw_timestamp':struct.unpack_from('<I',raw_sdk_bytes,4)[0],'normalized_sha256':digest(bytes(normalized_sdk)),'normalized_timestamp':0,'only_coff_header_bytes4_to7_changed':True,'all_sections_symbols_aux_relocations_equal':True,'unwind_preserved':True}
                dll=directory/'tar_sdk.dll';importlib=directory/'tar_sdk.lib'
                link_c(str(index)+'-sdk-link',[sdkobj,kernel],dll,['-shared','-Wl,/nodefaultlib,/noentry,/Brepro,/timestamp:0,/implib:'+str(importlib)],True)
                inspection=run(str(index)+'-inspect-sdk',[readobj,'--unwind','--coff-imports','--coff-exports',dll],cenv)
                inspected=inspection.stdout.decode()
                if 'ALLOC_SMALL size=56' not in inspected or 'CompareStringOrdinal' not in inspected or 'sengoo_tar_windows_scalar_equal' not in inspected:raise ValueError('SDK lost real unwind/import/export facts')
                rawfixture=compile_c(str(index)+'-fixture',directory/'tar_fixture.c',directory/'fixture.raw.obj',leaf=True)
                original=rawfixture.read_bytes();parsed=coff_image(original)
                assembly=run(str(index)+'-fixture-leaf',[objdump,'-d','--no-show-raw-insn',rawfixture],cenv).stdout.decode().split('Disassembly of section .text:',1)[1]
                if re.search(r'%(?:rsp|rbp)|\b(?:call\w*|push\w*|pop\w*|enter|leave)\b',assembly):raise ValueError('test C fixture is not a stack-free leaf')
                removed=[name for name,value in parsed['sections'].items() if name in ['.data','.bss'] and not value['data'] and not value['relocations']]
                fixture=directory/'fixture.obj'
                run(str(index)+'-fixture-metadata',[objcopy,*['--strip-symbol='+name for name in [*parsed['sections'],'.file','@feat.00']],*['--remove-section='+name for name in removed],rawfixture,fixture],cenv)
                normalized=bytearray(fixture.read_bytes());normalized[4:8]=bytes(4);fixture.write_bytes(normalized)
                report.setdefault('metadata_proofs',{})[label]=metadata_proof(original,bytes(normalized));objects['fixture']=fixture
                image=directory/'tar.exe';linked=run(str(index)+'-sg-link',[linker,'link',image,'main',*objects.values(),importlib,kernel,out/'ntdll-chkstk.lib'])
                if linked.stdout or linked.stderr:raise ValueError('SG linker printed unexpected output')
                cold=run(str(index)+'-cold',[image],expected=42)
                if cold.stdout or cold.stderr:raise ValueError('cold tar image output must be empty')
                fingerprints={name:sha(path) for name,path in {**objects,'image':image,'sdk_object':sdkobj,'sdk_dll':dll,'sdk_importlib':importlib}.items()}
                if rounds and rounds[0]['sha256']!=fingerprints:raise ValueError('two fresh SG/CSDK/PE artifacts differ')
                rounds.append({'round':label,'sha256':fingerprints,'empty_path_cold_exit':42,'output_empty':True});providers.append((directory,dll,importlib))
                for path in [*objects.values(),image,sdkobj,dll,importlib]:report['artifacts'][str(path)]=sha(path)
            report['fresh_rounds']=rounds
            directory,dll,importlib=providers[0]
            debugobj=compile_c('unwind-probe',directory/'tar_unwind_probe.c',out/'unwind-probe.obj',['-O0','-g','-gcodeview'])
            debugexe=link_c('link-unwind-probe',[debugobj,importlib],directory/'unwind.exe',['-Wl,/debug,/pdb:'+str(directory/'unwind.pdb')])
            debugenv=dict(clean,PYTHONHOME=str(debug_python),PYTHONPATH=str(debug_python/'python310.zip')+os.pathsep+str(debug_python),LLDB_USE_NATIVE_PDB_READER='1',PATH=str(debug_python)+os.pathsep+str(clang.parent))
            debugged=run('actual-active-sdk-unwind',[lldb,'--batch','-o','breakpoint set --shlib tar_sdk.dll -n sengoo_tar_windows_scalar_equal','-o','run','-o','thread step-inst','-o','thread backtrace','-o','disassemble -f','-o','continue',debugexe],debugenv)
            text=debugged.stdout.decode()
            for required in ['sengoo_tar_windows_scalar_equal + 4','tar_unwind_caller','`main','exited with status = 42','subq   $0x38, %rsp']:
                if required not in text:raise ValueError('actual active-frame unwind did not observe '+required)
            loaded=ctypes.WinDLL(str(dll));scalar=loaded.sengoo_tar_windows_scalar_equal;scalar.argtypes=[ctypes.c_int64,ctypes.c_int64];scalar.restype=ctypes.c_int64
            if scalar(65,97)!=1:raise ValueError('loaded SDK scalar comparison differs')
            class RuntimeFunction(ctypes.Structure):_fields_=[('begin',ctypes.c_uint32),('end',ctypes.c_uint32),('unwind',ctypes.c_uint32)]
            lookup=ctypes.windll.ntdll.RtlLookupFunctionEntry;lookup.argtypes=[ctypes.c_uint64,ctypes.POINTER(ctypes.c_uint64),ctypes.c_void_p];lookup.restype=ctypes.POINTER(RuntimeFunction)
            base=ctypes.c_uint64();address=ctypes.cast(scalar,ctypes.c_void_p).value;entry=lookup(address,ctypes.byref(base),None)
            if not entry or not base.value+entry.contents.begin<=address<base.value+entry.contents.end:raise ValueError('Windows did not register SDK unwind range')
            report['loaded_sdk_unwind']={'export_rva':address-base.value,'begin':entry.contents.begin,'end':entry.contents.end,'unwind_rva':entry.contents.unwind,'unwind_header':ctypes.string_at(base.value+entry.contents.unwind,4).hex(),'body_backtrace_verified':True}
            kernel32=ctypes.WinDLL('kernel32',use_last_error=True)
            getmodule=kernel32.GetModuleHandleExW;getmodule.argtypes=[ctypes.c_uint32,ctypes.c_void_p,ctypes.POINTER(ctypes.c_void_p)];getmodule.restype=ctypes.c_int
            filename=kernel32.GetModuleFileNameW;filename.argtypes=[ctypes.c_void_p,ctypes.c_wchar_p,ctypes.c_uint32];filename.restype=ctypes.c_uint32
            identities=[]
            for name,fn in [('CompareStringOrdinal',kernel32.CompareStringOrdinal),('RtlLookupFunctionEntry',lookup)]:
                pointer=ctypes.cast(fn,ctypes.c_void_p).value;module=ctypes.c_void_p();buffer=ctypes.create_unicode_buffer(32768)
                if not getmodule(6,pointer,ctypes.byref(module)) or not filename(module,buffer,len(buffer)):raise ValueError('actual OS provider module identity unavailable')
                module_path=Path(buffer.value).resolve();identities.append({'symbol':name,'module':str(module_path),'sha256':digest(read(module_path)),'rva':pointer-module.value})
            report['actual_os_providers']=identities
            for path in [debugexe,directory/'unwind.pdb',debugobj]:report['artifacts'][str(path)]=sha(path)
            report['formal_qualification_complete']=True
    except Exception as error:report['result']='failed';report['error']=str(error);raise
    finally:
        report['changed_sources']=[name for name,value in report['sources'].items() if not (ROOT/name).is_file() or sha(ROOT/name)!=value]
        report['changed_external_sources']=[name for name,value in report['external_sources'].items() if not Path(name).is_file() or sha(Path(name))!=value]
        report['changed_tools']=[name for name,value in report['tools'].items() if not Path(value['path']).is_file() or sha(Path(value['path']))!=value['sha256']]
        if report['changed_sources'] or report['changed_external_sources'] or report['changed_tools']:report['result']='failed'
        report['revision_end']=revision();report['git_status_end']=git_status();report['completed_at']=dt.datetime.now(dt.timezone.utc).isoformat();save()
    if report['result']!='passed':raise SystemExit(1)
if __name__=='__main__':main()
