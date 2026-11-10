"""Pinned offline tar policy oracle, independent of the SG implementation."""
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
    report={'schema':1,'stage':'independent-tar-package-policy','result':'running','started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'host':platform.platform(),'revision_start':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip(),'sources':{},'tools':{},'steps':[]}
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
        read(Path(__file__));read(ROOT/'selfhost/tests/tar_oracle.rs');read(ROOT/'Cargo.lock');read(a.corpus);manifest=json.loads(read(a.corpus.with_suffix('.json')))
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
        # xattr is an optional cfg(unix) extraction dependency, unused by this
        # Windows read-only parser probe. Do not fetch a foreign-host provider.
        (project/'Cargo.toml').write_text('[package]\nname="sg-tar-policy-oracle"\nversion="0.0.0"\nedition="2021"\n[dependencies]\ntar={version="=0.4.46",default-features=false}\n[workspace]\n')
        report['feature_scope']='Windows read-only tar parser; unused Unix xattr extraction feature disabled'
        (project/'src/main.rs').write_bytes(read(ROOT/'selfhost/tests/tar_oracle.rs'))
        read(project/'Cargo.toml');read(project/'src/main.rs')
        run('generate-offline-lock',[cargo,'generate-lockfile','--offline','--manifest-path',project/'Cargo.toml'])
        repository_lock=tomllib.loads((ROOT/'Cargo.lock').read_text())
        pinned_names=['cfg-if','libc','filetime']
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
            if package['name']!='sg-tar-policy-oracle':
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
        binary=tool('tar-oracle',out/'rust-target/debug/sg-tar-policy-oracle.exe')
        checked=run('execute-all-fixtures',[binary,a.corpus]);lines=checked.stdout.decode().splitlines()
        report['observation_stdout']={'path':str(out/'execute-all-fixtures.stdout.log'),'sha256':hashlib.sha256(checked.stdout).hexdigest(),'corpus':str(a.corpus),'manifest':str(a.corpus.with_suffix('.json'))}
        if len(lines)!=manifest['records']:raise ValueError('Rust oracle missed fixture IDs')
        report['observations']=[]
        for index,line in enumerate(lines):
            fields=line.split('|')
            if len(fields)!=3 or fields[0]!=str(index) or fields[1] not in ['true','false']:raise ValueError('Rust tar protocol differs')
            observation={'id':index,'name':manifest['fixtures'][index]['name'],'ok':fields[1]=='true'}
            if observation['ok']:
                entries=[]
                for value in fields[2].split(';') if fields[2] else []:
                    row=value.split(',')
                    if len(row)!=7 or any(re.fullmatch(r'(?:[0-9a-f]{2})*',row[i]) is None for i in [0,6]) or any(re.fullmatch(r'0|[1-9][0-9]*',row[i]) is None for i in range(1,6)):raise ValueError('Rust tar entry protocol differs')
                    entries.append({'path_hex':row[0],'kind':int(row[1]),'declared':int(row[2]),'effective':int(row[3]),'header':int(row[4]),'data':int(row[5]),'content_hex':row[6]})
                observation['entries']=entries
            else:
                if re.fullmatch(r'(?:[0-9a-f]{2})*',fields[2]) is None:raise ValueError('Rust tar error protocol differs')
                observation['error']=bytes.fromhex(fields[2]).decode('utf8')
            report['observations'].append(observation)
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
