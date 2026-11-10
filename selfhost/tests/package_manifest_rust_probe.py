"""Pinned canonical manifest/metadata/content oracle; no SG implementation provider."""
from pathlib import Path
import argparse,datetime as dt,hashlib,io,json,os,platform,re,subprocess,sys,tarfile,time,tomllib

ROOT=Path(__file__).resolve().parents[2]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['cargo','rustc','output']:p.add_argument('--'+name,type=Path,required=True)
    a=p.parse_args();out=a.output.resolve()
    if not out.is_relative_to(ROOT/'target'):raise ValueError('output outside target')
    out.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'stage':'independent-sgpm-manifest-content-policy','result':'running','started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'host':platform.platform(),'revision_start':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'sources':{},'tools':{},'steps':[]}
    def read(path):
        path=Path(path).resolve();raw=path.read_bytes();report['sources'][str(path)]=hashlib.sha256(raw).hexdigest();return raw
    def tool(name,path):
        path=Path(path).resolve();report['tools'][name]={'path':str(path),'sha256':sha(path)};return path
    cargo,rustc=tool('cargo',a.cargo),tool('rustc',a.rustc)
    if cargo.parent!=rustc.parent:raise ValueError('Rust tools are not from one selected toolchain')
    env={key:value for key,value in os.environ.items() if not key.upper().startswith(('RUSTC','RUSTDOC','CARGO_BUILD_','CARGO_PROFILE_','CARGO_TARGET_')) and key.upper() not in ['RUSTFLAGS','CARGO_ENCODED_RUSTFLAGS','RUSTUP_TOOLCHAIN','CARGO_INCREMENTAL']}
    env.update(CARGO=str(cargo),RUSTC=str(rustc),CARGO_BUILD_RUSTC=str(rustc),RUSTUP_TOOLCHAIN=cargo.parent.parent.name,RUSTC_WRAPPER='',RUSTC_WORKSPACE_WRAPPER='',CARGO_BUILD_RUSTC_WRAPPER='',CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER='')
    env['PATH']=str(cargo.parent)+os.pathsep+env.get('PATH','')
    def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n')
    def run(name,cmd):
        cmd=list(map(str,cmd));start=time.monotonic();r=subprocess.run(cmd,cwd=out,env=env,capture_output=True,timeout=600)
        (out/(name+'.stdout.log')).write_bytes(r.stdout);(out/(name+'.stderr.log')).write_bytes(r.stderr);report['steps'].append({'name':name,'command':cmd,'exit':r.returncode,'seconds':time.monotonic()-start,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()});save();print(name,r.returncode,flush=True)
        if r.returncode:raise RuntimeError(name+' failed')
        return r
    save()
    try:
        read(Path(__file__));read(ROOT/'selfhost/tests/package_manifest_oracle.rs');read(ROOT/'Cargo.lock')
        report['compiler_version']=run('rustc-version',[rustc,'-vV']).stdout.decode();report['cargo_version']=run('cargo-version',[cargo,'-vV']).stdout.decode()
        sysroot=Path(run('rustc-sysroot',[rustc,'--print','sysroot']).stdout.decode().strip()).resolve()
        if sysroot!=cargo.parent.parent:raise ValueError('actual rustc sysroot differs')
        for directory in [sysroot/'bin',sysroot/'lib']:
            for path in directory.glob('*.dll'):read(path)
        for path in (sysroot/'lib/rustlib/x86_64-pc-windows-msvc/lib').glob('*.rlib'):read(path)
        configs={parent/'.cargo'/name for parent in [out,*out.parents] for name in ['config','config.toml']}
        configs.update(Path(env.get('CARGO_HOME',str(Path.home()/'.cargo')))/name for name in ['config','config.toml'])
        report['cargo_config_candidates']={}
        for config in configs:
            report['cargo_config_candidates'][str(config)]=sha(config) if config.is_file() else None
            if config.is_file() and 'include' in tomllib.loads(read(config).decode()):raise ValueError('Cargo config include needs expanded qualification')
        project=out/'oracle';(project/'src').mkdir(parents=True)
        repository=tomllib.loads(read(ROOT/'Cargo.lock').decode())
        dependencies=['miette','serde','serde_json','toml','semver','sha2','walkdir']
        selected={}
        for name in dependencies:
            rows=[row for row in repository['package']if row['name']==name]
            if len(rows)!=1:raise ValueError('canonical direct dependency ambiguous '+name)
            selected[name]=rows[0]['version']
        lines=['[package]','name="sg-manifest-policy-oracle"','version="0.0.0"','edition="2021"','[workspace]','[dependencies]']
        for name,version in selected.items():lines.append(name+'={version="='+version+'",features=["derive"]}'if name=='serde'else name+'="='+version+'"')
        (project/'Cargo.toml').write_text('\n'.join(lines)+'\n')
        (project/'src/main.rs').write_bytes(read(ROOT/'selfhost/tests/package_manifest_oracle.rs'))
        (project/'src/canonical_manifest.rs').write_bytes(read(ROOT/'tools/sgpm/src/manifest.rs'))
        resolver=read(ROOT/'tools/sgpm/src/resolver.rs').decode()
        first=resolver.index('pub struct RegistryPackageMetadata');begin=resolver.rfind('#[derive',0,first);end=resolver.index('\n}',first)+2
        (project/'src/canonical_metadata.rs').write_text('use serde::{Serialize,Deserialize};\n'+resolver[begin:end]+'\n')
        begin=resolver.index('fn package_content_checksum(');end=resolver.index('\nfn create_remote_registry_staging_dir',begin)
        (project/'src/canonical_content.rs').write_text('use std::fs;\nuse std::path::Path;\nuse miette::{Result,Context,IntoDiagnostic};\nuse walkdir::WalkDir;\nuse sha2::{Digest,Sha256};\n'+resolver[begin:end]+'\npub fn checksum(path:&Path)->Result<String>{package_content_checksum(path,&[".sgpm-archive.sha256",".sgpm-content.sha256"])}\n')
        read(project/'Cargo.toml')
        for source in (project/'src').glob('*.rs'):read(source)
        report['canonical_sources']={'manifest':'tools/sgpm/src/manifest.rs','metadata_and_content':'tools/sgpm/src/resolver.rs','extraction':'full manifest module; exact metadata struct; exact package_content_checksum function, imports/wrapper only'}
        run('generate-offline-lock',[cargo,'generate-lockfile','--offline','--manifest-path',project/'Cargo.toml'])
        actual=tomllib.loads((project/'Cargo.lock').read_text())
        pin_names=sorted({row['name']for row in actual['package']if row['name']!='sg-manifest-policy-oracle'})
        for name in pin_names:
            matches=[row for row in repository['package']if row['name']==name]
            current=[row for row in tomllib.loads((project/'Cargo.lock').read_text())['package']if row['name']==name]
            if len(current)!=1:raise ValueError('oracle transitive package ambiguous '+name)
            if len(matches)>1:
                matches=[row for row in matches if row['version']==current[0]['version']and row.get('source')==current[0].get('source')and row.get('checksum')==current[0].get('checksum')]
            if len(matches)!=1:raise ValueError('canonical transitive dependency ambiguous '+name)
            if current[0]['version']!=matches[0]['version']:run('pin-'+name,[cargo,'update','--offline','--manifest-path',project/'Cargo.toml','-p',name,'--precise',matches[0]['version']])
        read(project/'Cargo.lock')
        metadata=run('locked-offline-metadata',[cargo,'metadata','--locked','--offline','--format-version','1','--manifest-path',project/'Cargo.toml'])
        packages=json.loads(metadata.stdout)['packages'];repo_lock=tomllib.loads((ROOT/'Cargo.lock').read_text());actual_lock=tomllib.loads((project/'Cargo.lock').read_text())
        report['packages']=[]
        for package in packages:
            directory=Path(package['manifest_path']).parent.resolve();files=[path for path in directory.rglob('*') if path.is_file() and 'target' not in path.relative_to(directory).parts]
            for path in files:read(path)
            identity={'name':package['name'],'version':package['version'],'manifest':package['manifest_path'],'files':len(files),'source':package['source']}
            if package['name']!='sg-manifest-policy-oracle':
                repository=[row for row in repo_lock['package'] if (row['name'],row['version'],row.get('source'))==(package['name'],package['version'],package['source'])]
                selected=[row for row in actual_lock['package'] if (row['name'],row['version'],row.get('source'))==(package['name'],package['version'],package['source'])]
                if len(repository)!=1 or len(selected)!=1 or repository[0]['checksum']!=selected[0]['checksum']:raise ValueError('ambiguous or mismatched repository dependency identity: '+package['name'])
                archive=directory.parents[2]/'cache'/directory.parent.name/(package['name']+'-'+package['version']+'.crate')
                archive_bytes=read(archive)
                if hashlib.sha256(archive_bytes).hexdigest()!=selected[0]['checksum']:raise ValueError('cached crate archive checksum differs')
                verified=0
                with tarfile.open(fileobj=io.BytesIO(archive_bytes),mode='r:gz') as packed:
                    for member in packed:
                        if not member.isfile():continue
                        prefix=package['name']+'-'+package['version']+'/'
                        if not member.name.startswith(prefix):raise ValueError('crate archive member prefix differs')
                        path=(directory/member.name[len(prefix):]).resolve()
                        if not path.is_relative_to(directory) or read(path)!=packed.extractfile(member).read():raise ValueError('cached source differs from pinned crate archive: '+member.name)
                        verified+=1
                identity.update(checksum=selected[0]['checksum'],source_archive_files_verified=verified,crate_archive=str(archive),crate_archive_verified=True,
                                source_verification='every regular archive member equals the actual cached source; no archive extraction or source writes')
            report['packages'].append(identity)
        built=run('locked-offline-build',[cargo,'build','--locked','--offline','--verbose','--manifest-path',project/'Cargo.toml','--target-dir',out/'rust-target'])
        if not re.search(r'Running.*'+re.escape(str(rustc)).replace('\\\\','[\\\\/]'),built.stderr.decode(),re.I):
            if str(rustc) not in built.stderr.decode():raise ValueError('Cargo did not visibly execute the pinned rustc')
        binary=tool('package-oracle',out/'rust-target/debug/sg-manifest-policy-oracle.exe')
        checked=run('execute-version-control',[binary,'--semver','1.2.3'])
        if json.loads(checked.stdout)!={'ok':True}or checked.stderr:raise ValueError('actual canonical oracle control failed')
        report['oracle_binary']=report['tools']['package-oracle']
        report['result']='passed'
    except BaseException as error:report.update(result='failed',error=repr(error))
    finally:
        report['changed_sources']=[name for name,expected in report['sources'].items() if not Path(name).is_file() or sha(name)!=expected]
        report['changed_tools']=[name for name,row in report['tools'].items() if sha(row['path'])!=row['sha256']]
        report['changed_cargo_configs']=[name for name,expected in report.get('cargo_config_candidates',{}).items() if (sha(name) if Path(name).is_file() else None)!=expected]
        if report['changed_sources'] or report['changed_tools'] or report['changed_cargo_configs']:report['result']='failed'
        report['revision_end']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip();report['completed_at']=dt.datetime.now(dt.timezone.utc).isoformat();save()
    print(report['result'],report.get('error'));return 0 if report['result']=='passed' else 1
if __name__=='__main__':raise SystemExit(main())
