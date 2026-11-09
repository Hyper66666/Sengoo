"""Pinned offline flate2 policy oracle, independent of the SG implementation."""
from pathlib import Path
import argparse,datetime as dt,hashlib,io,json,os,platform,re,subprocess,sys,tarfile,time,tomllib

ROOT=Path(__file__).resolve().parents[2]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def main():
    p=argparse.ArgumentParser(description=__doc__)
    for name in ['cargo','rustc','corpus','output']:p.add_argument('--'+name,type=Path,required=True)
    a=p.parse_args();a.corpus=a.corpus.resolve();out=a.output.resolve()
    if not out.is_relative_to(ROOT/'target'):raise ValueError('output outside target')
    out.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'stage':'independent-flate2-gzip-policy','result':'running','started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'host':platform.platform(),'revision_start':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'sources':{},'tools':{},'steps':[]}
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
        read(Path(__file__));read(ROOT/'selfhost/tests/gzip_oracle.rs');read(ROOT/'Cargo.lock');read(a.corpus);manifest=json.loads(read(a.corpus.with_suffix('.bin.json')))
        report['compiler_version']=run('rustc-version',[rustc,'-vV']).stdout.decode();report['cargo_version']=run('cargo-version',[cargo,'-vV']).stdout.decode()
        sysroot=Path(run('rustc-sysroot',[rustc,'--print','sysroot']).stdout.decode().strip()).resolve()
        if sysroot!=cargo.parent.parent:raise ValueError('actual rustc sysroot differs')
        for path in (sysroot/'lib/rustlib/x86_64-pc-windows-msvc/lib').glob('*.rlib'):read(path)
        configs={parent/'.cargo'/name for parent in [out,*out.parents] for name in ['config','config.toml']}
        configs.update(Path(env.get('CARGO_HOME',str(Path.home()/'.cargo')))/name for name in ['config','config.toml'])
        report['cargo_config_candidates']={}
        for config in configs:
            report['cargo_config_candidates'][str(config)]=sha(config) if config.is_file() else None
            if config.is_file() and 'include' in tomllib.loads(read(config).decode()):raise ValueError('Cargo config include needs expanded qualification')
        project=out/'oracle';(project/'src').mkdir(parents=True)
        (project/'Cargo.toml').write_text('[package]\nname="sg-gzip-policy-oracle"\nversion="0.0.0"\nedition="2021"\n[dependencies]\nflate2="=1.1.9"\n[workspace]\n')
        (project/'src/main.rs').write_bytes(read(ROOT/'selfhost/tests/gzip_oracle.rs'))
        read(project/'Cargo.toml');read(project/'src/main.rs')
        run('generate-offline-lock',[cargo,'generate-lockfile','--offline','--manifest-path',project/'Cargo.toml'])
        repository_lock=tomllib.loads((ROOT/'Cargo.lock').read_text())
        pinned_names=['cfg-if','crc32fast','simd-adler32']
        for name in pinned_names:
            matches={row['version'] for row in repository_lock['package'] if row['name']==name}
            if len(matches)!=1:raise ValueError('ambiguous repository oracle dependency '+name)
            run('pin-'+name,[cargo,'update','--offline','--manifest-path',project/'Cargo.toml','-p',name,'--precise',next(iter(matches))])
        read(project/'Cargo.lock')
        metadata=run('locked-offline-metadata',[cargo,'metadata','--locked','--offline','--format-version','1','--manifest-path',project/'Cargo.toml'])
        packages=json.loads(metadata.stdout)['packages'];repo_lock=tomllib.loads((ROOT/'Cargo.lock').read_text());actual_lock=tomllib.loads((project/'Cargo.lock').read_text())
        report['packages']=[]
        for package in packages:
            directory=Path(package['manifest_path']).parent.resolve();files=[path for path in directory.rglob('*') if path.is_file() and 'target' not in path.relative_to(directory).parts]
            for path in files:read(path)
            identity={'name':package['name'],'version':package['version'],'manifest':package['manifest_path'],'files':len(files),'source':package['source']}
            if package['name']!='sg-gzip-policy-oracle':
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
        binary=tool('flate2-oracle',out/'rust-target/debug/sg-gzip-policy-oracle.exe')
        checked=run('execute-all-fixtures',[binary,a.corpus]);lines=checked.stdout.decode().splitlines()
        report['observation_stdout']={'path':str(out/'execute-all-fixtures.stdout.log'),'sha256':hashlib.sha256(checked.stdout).hexdigest(),'corpus':str(a.corpus),'manifest':str(a.corpus.with_suffix('.bin.json'))}
        if len(lines)!=manifest['records']:raise ValueError('Rust oracle missed fixture IDs')
        report['observations']=[]
        for index,line in enumerate(lines):
            fields=line.split('|')
            if len(fields)!=5 or fields[0]!=str(index) or any(value not in ['true','false'] for value in fields[1:3]) or any(re.fullmatch(r'(?:[0-9a-f]{2})*',value) is None for value in fields[3:]):raise ValueError('Rust result protocol differs')
            if fields[1]=='false' and fields[3] or fields[2]=='false' and fields[4]:raise ValueError('failed Rust result published bytes')
            report['observations'].append({'id':index,'name':manifest['fixtures'][index]['name'],'first_ok':fields[1]=='true','multi_ok':fields[2]=='true','first_hex':fields[3],'multi_hex':fields[4]})
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
