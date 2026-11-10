"""Qualify fresh source effect proofs and selected ownership without granting native Ref authority."""
from __future__ import annotations
import argparse, datetime, hashlib, json, os, platform, re, shutil, struct, subprocess, sys, tomllib
from pathlib import Path

ROOT=Path(__file__).resolve().parents[1]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def revision():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def rust_override(key):
    key=key.upper()
    return key.startswith(('RUSTC','RUSTDOC','RUSTUP_TOOLCHAIN','CARGO_BUILD_RUSTC','CARGO_BUILD_RUSTDOC','CARGO_PROFILE_')) or key in {'CARGO','RUSTFLAGS','RUSTDOCFLAGS','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS','CARGO_BUILD_RUSTFLAGS','CARGO_BUILD_RUSTDOCFLAGS','CARGO_BUILD_TARGET','CARGO_TARGET_DIR','CARGO_INCREMENTAL'} or key.startswith('CARGO_TARGET_') and key.endswith(('_RUSTFLAGS','_RUSTDOCFLAGS','_RUSTC'))
def closure(entries):
    result={};pending=list(entries)
    while pending:
        path=pending.pop()
        if path in result:continue
        result[path]=sha(path)
        pending += [ROOT/'selfhost/src'/(n+'.sg') for n in re.findall(r'import\s+selfhost_([A-Za-z0-9_]+)\s*;',path.read_text('utf8'))]
    return result
def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for n in ['seed','clang','formatter','output']:parser.add_argument('--'+n,required=True,type=Path)
    args=parser.parse_args();base=args.output.resolve();assert base.is_relative_to(ROOT/'target');base.mkdir(parents=True,exist_ok=False)
    inputs={str(Path(__file__).resolve()):sha(__file__)}
    originals=closure([ROOT/'selfhost/tests/collection_raw_next_observation.sg'])
    for directory in ['tools/stdlib','runtime']:
        originals.update({p:sha(p)for p in (ROOT/directory).rglob('*')if p.is_file()and not any(n in {'build','target','__pycache__'}for n in p.relative_to(ROOT/directory).parts)})
    for n in ['selfhost/tests/collection_raw_next_observation_source.sg','selfhost/bootstrap.lock','Cargo.toml','Cargo.lock','rust-toolchain.toml']:originals[ROOT/n]=sha(ROOT/n)
    snapshot=base/'implementation'
    for p,h in originals.items():
        raw=p.read_bytes();assert hashlib.sha256(raw).hexdigest()==h;copy=snapshot/p.relative_to(ROOT);copy.parent.mkdir(parents=True,exist_ok=True);copy.write_bytes(raw);assert sha(copy)==h;inputs[str(copy)]=h
    assert all(sha(p)==h for p,h in originals.items())
    for directory in ['runtime']:
        inputs.update({str(p):sha(p)for p in (ROOT/directory).rglob('*')if p.is_file()and not any(n in {'build','target','__pycache__'}for n in p.relative_to(ROOT/directory).parts)})
    for n in ['Cargo.toml','Cargo.lock','rust-toolchain.toml']:inputs[str(ROOT/n)]=sha(ROOT/n)
    for p in (ROOT/'openspec/changes/add-selfhost-collection-intrinsics/specs/selfhost-collection-raw-next-observation').rglob('*.md'):
        if p.name!='tasks.md':
            raw=p.read_bytes();copy=snapshot/p.relative_to(ROOT);copy.parent.mkdir(parents=True,exist_ok=True);copy.write_bytes(raw);inputs[str(copy)]=sha(copy)
    tools={'seed':args.seed.resolve(),'clang':args.clang.resolve(),'formatter':args.formatter.resolve(),'python':Path(sys.executable)}
    lock=json.loads((ROOT/'selfhost/bootstrap.lock').read_text('utf8'))
    for n in ['seed','clang','formatter']:assert sha(tools[n])==lock[n]['sha256']
    env={k:v for k,v in os.environ.items()if not rust_override(k)and not k.upper().startswith(('SENGOO_','SGPLATFORM_','SDL2_','PYTHON','LLDB_'))and k.upper()not in {'CL','_CL_','LINK','_LINK_'}}
    env['PATH']=str(tools['clang'].parent)+os.pathsep+env.get('PATH','')
    tools['rustup']=Path(shutil.which('rustup',path=env['PATH'])).resolve()
    for n in ['cargo','rustc']:tools[n]=Path(subprocess.check_output([tools['rustup'],'which',n],env=env,text=True).strip()).resolve()
    assert tools['cargo'].parent==tools['rustc'].parent
    env.update(CARGO=str(tools['cargo']),RUSTC=str(tools['rustc']),CARGO_BUILD_RUSTC=str(tools['rustc']),RUSTUP_TOOLCHAIN=tools['rustc'].parent.parent.name,RUSTC_WRAPPER='',RUSTC_WORKSPACE_WRAPPER='',CARGO_BUILD_RUSTC_WRAPPER='',CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='')
    env['PATH']=str(tools['cargo'].parent)+os.pathsep+env['PATH']
    env.update(SENGOO_STDLIB=str(snapshot/'tools/stdlib'),SENGOO_RUNTIME=str(snapshot/'tools/stdlib/runtime.c'),SENGOO_CLANG=str(tools['clang']),SENGOO_LINKER='lld',SENGOO_MODULE_MAP=';'.join('selfhost_'+p.stem+'='+str(snapshot/p.relative_to(ROOT))for p in originals if p.parent==ROOT/'selfhost/src'))
    pure={k:v for k,v in env.items()if not k.upper().startswith('SENGOO_')};pure['PATH']=''
    report={'schema':1,'stage':'direct-raw-next-source-observation','result':'running','production_admitted':False,'m3_complete':False,'mir_schema':1,'host':platform.platform(),'source_revision':revision(),'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sources':inputs,'qualified_checkout_sources':{str(p):h for p,h in originals.items()},'tools':{n:{'path':str(p),'sha256':sha(p)}for n,p in tools.items()},'steps':[],'limits':['schema1 source candidates remain unlowerable1; a faithful replay is not native Ref/constructor authority','read-only scope is seven explicit actual root functions; public source/native permission and complete HIR new-facts persistence remain open','qualified historical C collection provider contract is pinned; existing C algorithms remain a development SDK dependency','physical MIR27 and push runtime execution is separately qualified by ed3aaa4; this gate tests source semantic admission/rejection and exact ownership','seed compiler uses recorded C/Rust development runtimes; requested lld selection does not independently qualify its complete nested SDK child-link closure','M3 core diagnostics, generic source jobs, Some source native emission, M6/M8/bootstrap cutover remain open'],'sdk_environment':{n:env.get(n)for n in ['INCLUDE','LIB','LIBPATH','VCToolsInstallDir','WindowsSdkDir','WindowsSDKVersion','VSINSTALLDIR','VCINSTALLDIR']}}
    configs={p/'.cargo'/n for p in [ROOT,*ROOT.parents,snapshot,*snapshot.parents]for n in ['config','config.toml']};configs.update(Path(os.environ.get('CARGO_HOME',str(Path.home()/'.cargo')))/n for n in ['config','config.toml']);report['cargo_config_candidates']={}
    for p in configs:
        raw=p.read_bytes()if p.is_file()else None;report['cargo_config_candidates'][str(p)]=None if raw is None else hashlib.sha256(raw).hexdigest()
        if raw is not None:assert 'include'not in tomllib.loads(raw.decode('utf8'))
    def save():(base/'evidence.json').write_text(json.dumps(report,ensure_ascii=False,indent=2)+'\n',encoding='utf8')
    def run(name,command,code=0,empty=False):
        done=subprocess.run(list(map(str,command)),cwd=ROOT,env=pure if empty else env,capture_output=True,timeout=900);stdout=base/(name+'.stdout.log');stderr=base/(name+'.stderr.log');stdout.write_bytes(done.stdout);stderr.write_bytes(done.stderr);report['steps'].append({'name':name,'command':list(map(str,command)),'exit_code':done.returncode,'expected_exit_code':code,'empty_path':empty,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)});save();print(name,done.returncode,flush=True);assert done.returncode==code,(name,done.returncode,done.stdout[-800:],done.stderr[-3000:]);return done
    artifacts={}
    try:
        # Revalidate the exact historical local qualification chain. It is not
        # substituted with freshly observed hashes of mutable current sources.
        qualified=ROOT/'target/selfhost/m3-collection-provider-ast-checkpoint';qp=qualified/'evidence.json';qraw=qp.read_bytes();qdigest=hashlib.sha256(qraw).hexdigest();assert qdigest=='a63152e01208dff39e1d5cbcbd326187901808c8f76fa0d3075a08e0a9fa1cd3';q=json.loads(qraw);assert q['result']=='passed'
        inputs[str(qp)]=qdigest
        immutable_values=set(q['sources'].values());transformed=[]
        for name,digest in q['qualified_checkout_sources'].items():
            if digest in immutable_values:continue
            assert name.replace('\\','/').endswith('/selfhost/sdk/collections_x64_abi1/manifest.json'),name
            assert sha(name)==digest;inputs[name]=digest;transformed.append({'path':name,'sha256':digest,'reason':'historical manifest template transformed by its qualified inventory step'})
        report['historical_provider_origin_bindings']={'originals':q['qualified_checkout_sources'],'consumed_immutable_sources':q['sources'],'transformed_originals':transformed}
        for name,digest in q['sources'].items():assert sha(name)==digest;inputs[name]=digest
        for name,item in q['tools'].items():assert sha(item['path'])==item['sha256'];inputs[item['path']]=item['sha256']
        assert not any(q.get(k)for k in ['changed_sources','changed_tools','checkout_changes','changed_configs','changed_artifacts'])
        for relative,digest in q['artifacts'].items():p=qualified/relative;assert sha(p)==digest;inputs[str(p)]=digest
        report['qualified_provider']={'report':str(qp),'report_sha256':qdigest,'pinned_historical_qualification':True,'manifest_sha256':q['provider_manifest_sha256'],'object_sha256':q['provider_object_sha256'],'sources':q['sources'],'tools':q['tools'],'artifacts':q['artifacts']}
        for n in ['cargo','rustc']:run(n+'-version',[tools[n],'-vV'if n=='rustc'else'-V'])
        # Cargo's actual compiler selection is verified, not inferred from a shim/sysroot.
        probe=base/'cargo-probe';probe.mkdir();(probe/'src').mkdir();(probe/'Cargo.toml').write_text('[package]\nname="effects_rustc_probe"\nversion="0.0.0"\nedition="2021"\n[workspace]\n',encoding='utf8');(probe/'src/lib.rs').write_text('pub fn value()->i64{42}\n',encoding='utf8');(probe/'build.rs').write_text('fn main(){println!("cargo:warning=effects_rustc={}",std::env::var("RUSTC").unwrap());}\n',encoding='utf8')
        observed=run('cargo-actual-rustc',[tools['cargo'],'build','--offline','--verbose','--manifest-path',probe/'Cargo.toml','--target-dir',probe/'target']);combined=(observed.stdout+observed.stderr).decode('utf8');runs=[line for line in combined.splitlines()if line.lstrip().startswith('Running ')and str(tools['rustc']).casefold()in line.casefold()];assert len(runs)>=2 and 'effects_rustc='+str(tools['rustc'])in combined;report['cargo_actual_rustc_invocations']=runs
        for p in [probe/'Cargo.toml',probe/'Cargo.lock',probe/'build.rs',probe/'src/lib.rs']:inputs[str(p)]=sha(p)
        for name in ['semantic_collection_raw_next_observation','semantic_collection_effect_ownership']:run('format-'+name,[tools['formatter'],'--check',snapshot/'selfhost/src'/(name+'.sg')])
        run('format-tests',[tools['formatter'],'--check',snapshot/'selfhost/tests/collection_raw_next_observation.sg'])
        binary=base/'effects.exe';run('effects-build',[tools['seed'],'--runtime-mode','source-development','build',snapshot/'selfhost/tests/collection_raw_next_observation.sg','--force-rebuild','-O','0','-o',binary]);artifacts[binary.name]=sha(binary)
        for flag in ['--fail-equality','--fail-truth']:
            result=run('assert-'+flag,[binary,flag],1,True);assert b'collection_raw_next_observation.sg'in result.stderr
        source=base/'physical source 世界 &'/ 'source.sg';source.parent.mkdir();raw=(snapshot/'selfhost/tests/collection_raw_next_observation_source.sg').read_bytes();source.write_bytes(raw);assert sha(source)==hashlib.sha256(raw).hexdigest();inputs[str(source)]=sha(source)
        result=run('source-first-success-rejection-and-poison-scopes',[binary,qualified/'implementation',q['provider_manifest_sha256'],q['provider_object_sha256'],source,sha(qualified/'implementation/tools/stdlib/collections.sg'),sha(qualified/'implementation/tools/stdlib/option.sg')],42,True)
        lines=result.stdout.decode('utf8').splitlines();assert not result.stderr and len(lines)==25 and lines[-1]=='raw next observation: three first-call resource scopes passed',lines
        expected=['negative_mutable','negative_unguarded','negative_invalidated','negative_unknown','negative_push_reuse','negative_fake','negative_cross_t','negative_family','negative_escape'];negative={}
        for index,name in enumerate(expected):assert lines[index*2]==name and lines[index*2+1].isdecimal()and int(lines[index*2+1])>0;negative[name]=int(lines[index*2+1])
        assert lines[18]=='raw_observation_counts'and all(v.isdecimal()for v in lines[19:24]);counts=dict(zip(['accepted_functions','direct_observations','iterator_reuse_observations','iterator_cleanup_actions','candidate_poisons'],map(int,lines[19:24])))
        assert counts['accepted_functions']==7 and counts['direct_observations']>=4 and counts['iterator_reuse_observations']==2 and counts['iterator_cleanup_actions']==1 and counts['candidate_poisons']==19,counts
        assert negative['negative_push_reuse']==6101 and negative['negative_unguarded']==6124,negative
        report.update(counts=counts,negative_source_cases=negative,resource_scopes=3,resource_counters=['String','Buffer','opaque','file snapshot handles','file snapshot bytes'],source_fixture={'path':str(source),'sha256':sha(source),'physical_source_bytes':len(raw),'rule':'fixed actual source sites; independent collection/Option pins passed from the retained provider branch'})
        report.update(result='passed',production_admitted=True,artifacts=artifacts,native_reference_authority=False,m3_complete=False)
    except BaseException as error:report.update(result='failed',error=repr(error));raise
    finally:
        report.update(changed_sources=[n for n,h in inputs.items()if not Path(n).is_file()or sha(n)!=h],changed_tools=[n for n,value in report['tools'].items()if not Path(value['path']).is_file()or sha(value['path'])!=value['sha256']],checkout_changes=[str(p)for p,h in originals.items()if not p.is_file()or sha(p)!=h],changed_configs=[n for n,h in report['cargo_config_candidates'].items()if(sha(n)if Path(n).is_file()else None)!=h],changed_artifacts=[n for n,h in artifacts.items()if not(base/n).is_file()or sha(base/n)!=h],source_revision_end=revision(),completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
        if any(report[k]for k in ['changed_sources','changed_tools','checkout_changes','changed_configs','changed_artifacts']):report.update(result='failed',error='qualified input/output changed')
        save()
    assert report['result']=='passed';print(sha(base/'evidence.json'))
if __name__=='__main__':main()
