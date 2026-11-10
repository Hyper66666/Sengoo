"""Real admitted transaction and independent filesystem/marker/hash oracles."""
from __future__ import annotations
import argparse,datetime as dt,hashlib,json,os,platform,subprocess,tarfile,time
from pathlib import Path

def main():
    p=argparse.ArgumentParser();p.add_argument('--subject',type=Path,required=True);p.add_argument('--oracle',type=Path,required=True);p.add_argument('--cases',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False)
    sha=lambda file:hashlib.sha256(Path(file).read_bytes()).hexdigest()
    report={'schema':1,'stage':'development-admitted-package-transaction','result':'running','started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'host':platform.platform(),'subject':{'path':str(a.subject.resolve()),'sha256':sha(a.subject)},'oracle':{'path':str(a.oracle.resolve()),'sha256':sha(a.oracle)},'inputs':{},'steps':[],'cases':[],'differences':[]}
    env=dict(os.environ,PATH='')
    def run(name,command):
        command=list(map(str,command));start=time.monotonic();r=subprocess.run(command,capture_output=True,env=env,timeout=900)
        for suffix,raw in [('stdout',r.stdout),('stderr',r.stderr)]:(a.output/(name+'.'+suffix)).write_bytes(raw)
        report['steps'].append({'name':name,'command':command,'exit':r.returncode,'seconds':time.monotonic()-start,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()})
        if r.returncode or r.stderr:raise ValueError(name+' process exit/output differs')
        return json.loads(r.stdout)
    def tree(folder):return {file.relative_to(folder).as_posix():sha(file)for file in folder.rglob('*')if file.is_file()}
    try:
        records=json.loads((a.cases/'manifest.json').read_text(encoding='utf8'))['cases']
        for index,row in enumerate(records):
            if row['id']!=index:raise ValueError('fixture ID sequence differs')
            source=a.cases/row['archive'];metadata_file=a.cases/row['metadata']
            for file,pin in [(source,row['archive_sha256']),(metadata_file,row['metadata_sha256'])]:
                if sha(file)!=pin:raise ValueError('fixture source identity differs')
                report['inputs'][str(file.resolve())]=pin
            directory=a.output/str(index);directory.mkdir();outside=directory/'outside';outside.mkdir();(outside/'outside.keep').write_bytes(b'outside')
            if row['existing_target']:(directory/'final').mkdir();(directory/'final/existing.keep').write_bytes(b'existing')
            stats=run('transaction-'+str(index),[a.subject,directory,source,'private','final',row['cancel_after'],row['attack'],row['oom'],outside,row['expected_name'],row['expected_version'],row['expected_archive_sha256'],metadata_file,row['allow_yanked']])
            record={'id':index,'name':row['name'],'expected':row['expected'],'stats':stats};report['cases'].append(record);print(row['name'],stats['result'],flush=True)
            if stats['result']!=row['expected']or stats['allocations']!=stats['frees']or stats['live_bytes']!=0 or stats['handles_before']!=stats['handles_after']or stats['parent_identity_stable']is not True:raise ValueError('actual result/resource receipt differs '+row['name'])
            if row['name']=='cancel-after-write'and stats['write_calls']<1:raise ValueError('cancellation did not follow actual private writes')
            if(outside/'outside.keep').read_bytes()!=b'outside':raise ValueError('outside sentinel changed')
            if row['expected']==0:
                oracle=a.output/('oracle-'+str(index));oracle.mkdir()
                # All accepted fixtures are independently constructed safe
                # entries; Python's data filter remains enabled as well.
                with tarfile.open(source,'r:gz')as archive:archive.extractall(oracle,filter='data')
                metadata=run('metadata-'+str(index),[a.oracle,'--metadata',metadata_file])
                if metadata['ok']is not True:raise ValueError('reference metadata failed')
                (oracle/'.sgpm-registry-metadata.json').write_bytes(bytes.fromhex(metadata['encoded_hex']))
                (oracle/'.sgpm-archive.sha256').write_bytes((row['archive_sha256']+'\n').encode('ascii'))
                digest=run('content-'+str(index),[a.oracle,'--content-checksum',oracle])
                if digest['ok']is not True:raise ValueError('reference content checksum failed')
                (oracle/'.sgpm-content.sha256').write_bytes((digest['sha256']+'\n').encode('ascii'))
                actual,expected=tree(directory/'final'),tree(oracle)
                if actual!=expected:
                    report['differences'].append({'id':index,'name':row['name'],'subject':actual,'reference':expected});raise ValueError('actual published bytes differ')
                record['reference_tree']=expected;record['metadata_exact_bytes_rust_equal']=True;record['content_checksum_rust_equal']=True
                if(directory/'private').exists():raise ValueError('private root survived publication')
            elif row['expected']==-11:
                if not(directory/'private').exists():raise ValueError('unknown-object residue was removed')
            elif(directory/'private').exists():raise ValueError('session staging was not removed')
            if row['existing_target']:
                if(directory/'final/existing.keep').read_bytes()!=b'existing':raise ValueError('old target changed')
            elif row['attack']in ['target','replace']:
                if(directory/'final/race.keep').read_bytes()!=b'foreign':raise ValueError('racing target replaced')
            elif row['expected']<0 and(directory/'final').exists():raise ValueError('failure published a final tree')
        report['result']='passed'
    except BaseException as error:report['result']='failed';report['error']=repr(error);raise
    finally:
        report['completed_at']=dt.datetime.now(dt.timezone.utc).isoformat();(a.output/'evidence.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n',encoding='utf8')
if __name__=='__main__':main()
