"""Execute the SG manifest model against the actual canonical Rust parser."""
from __future__ import annotations
import argparse,datetime as dt,hashlib,json,os,platform,subprocess,time
from pathlib import Path

def tree(document,node=0):
    item=document['nodes'][node];kind=item['kind']
    if kind==1:return bytes.fromhex(item['text']).decode('utf8')
    if kind==2:return item['integer']
    if kind==5:return item['boolean']
    if kind==6:return None
    if kind==4:raise ValueError('unadmitted scalar escaped canonical schema')
    value={}if kind==0 else[];child=item['first'];seen=set()
    while child>=0:
        if child in seen:raise ValueError('subject node cycle')
        seen.add(child);entry=document['nodes'][child];decoded=tree(document,child)
        if kind==0:
            key=bytes.fromhex(entry['key']).decode('utf8')
            if key in value:raise ValueError('subject duplicate object key')
            value[key]=decoded
        else:value.append(decoded)
        child=entry['next']
    return value

def rust_trim(text):
    return text.strip(' \t\n\r\v\f\u0085\u00a0\u1680\u2000\u2001\u2002\u2003\u2004\u2005\u2006\u2007\u2008\u2009\u200a\u2028\u2029\u202f\u205f\u3000')

def model(value):
    package=value['package'];dependencies={};registries={}
    for name,source in value.get('dependencies',{}).items():
        dep={'name':name,'package':None,'path':None,'git':None,'rev':None,'version_req':None,'registry':None}
        if type(source)is str:dep['version_req']=rust_trim(source)
        else:
            for key,text in source.items():dep['version_req'if key=='version'else key]=rust_trim(text)
        dependencies[name]=dep
    for name,source in value.get('registries',{}).items():registries[name]={'path':source.get('path'),'url':source.get('url'),'token_env':source.get('token_env')}
    return {'package':{'name':package['name'],'version':package['version'],'authors':package.get('authors',[]),'description':package.get('description'),'edition':package.get('edition')},'bin':None if 'bin'not in value else{'name':value['bin'].get('name'),'path':value['bin'].get('path','src/main.sg')},'lib':None if 'lib'not in value else{'path':value['lib'].get('path','src/lib.sg')},'test':value.get('test',[]),'registries':registries,'dependencies':dependencies}

def metadata_model(document):
    value={'checksum':None,'yank_reason':None};child=document['nodes'][0]['first'];seen=set()
    while child>=0:
        if child in seen:raise ValueError('metadata node cycle')
        seen.add(child);node=document['nodes'][child];key=bytes.fromhex(node['key']).decode('utf8')
        if key in ['checksum','yank_reason','yanked','features']:value[key]=tree(document,child)
        child=node['next']
    return value

def main():
    p=argparse.ArgumentParser();p.add_argument('--subject',type=Path,required=True);p.add_argument('--oracle',type=Path,required=True);p.add_argument('--cases',type=Path,required=True);p.add_argument('--output',type=Path,required=True);p.add_argument('--metadata',action='store_true');a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False)
    sha=lambda file:hashlib.sha256(Path(file).read_bytes()).hexdigest()
    report={'schema':1,'stage':'development-canonical-manifest-differential','result':'running','started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'host':platform.platform(),'subject':{'path':str(a.subject.resolve()),'sha256':sha(a.subject)},'oracle':{'path':str(a.oracle.resolve()),'sha256':sha(a.oracle)},'inputs':{},'steps':[],'cases':[],'differences':[]}
    env=dict(os.environ,PATH='')
    def run(name,command):
        command=list(map(str,command));start=time.monotonic();r=subprocess.run(command,capture_output=True,env=env,timeout=60)
        for suffix,raw in [('stdout',r.stdout),('stderr',r.stderr)]:(a.output/(name+'.'+suffix)).write_bytes(raw)
        report['steps'].append({'name':name,'command':command,'exit':r.returncode,'seconds':time.monotonic()-start,'stdout_sha256':hashlib.sha256(r.stdout).hexdigest(),'stderr_sha256':hashlib.sha256(r.stderr).hexdigest()})
        if r.returncode or r.stderr:raise ValueError(name+' process failed')
        return r.stdout
    try:
        records=json.loads((a.cases/'manifest.json').read_text())['cases']
        for row in records:
            file=a.cases/row['path'];assert sha(file)==row['sha256'];report['inputs'][str(file.resolve())]=row['sha256']
        if a.metadata:oracle=[json.loads(run('reference-'+str(row['id']),[a.oracle,'--metadata',a.cases/row['path']]))for row in records]
        else:
            raw=run('reference-all',[a.oracle,*[a.cases/x['path']for x in records]]);oracle=[json.loads(line)for line in raw.splitlines()]
        if len(oracle)!=len(records):raise ValueError('oracle result count differs')
        for index,row in enumerate(records):
            expected=oracle[index]
            if (not a.metadata and expected['id']!=index)or type(expected['ok'])is not bool:raise ValueError('oracle ID/status differs')
            subject=json.loads(run('subject-'+str(index),[a.subject,a.cases/row['path']]))
            if subject['schema']!=1 or type(subject['result'])is not int:raise ValueError('subject schema/result differs')
            record={'id':index,'name':row['name'],'reference_ok':expected['ok'],'subject_result':subject['result']}
            if (subject['result']==0)!=expected['ok']:report['differences'].append({**record,'reference':expected})
            elif subject['result']==0:
                actual=metadata_model(subject)if a.metadata else model(tree(subject));record['metadata'if a.metadata else'manifest']=actual
                if actual!=expected['value'if a.metadata else'manifest']:report['differences'].append({**record,'reference':expected})
                if a.metadata and subject['encoded_hex']!=expected['encoded_hex']:report['differences'].append({**record,'encoded_hex':subject['encoded_hex'],'reference':expected})
            report['cases'].append(record)
        report['result']='passed'if not report['differences']else'failed'
    finally:
        report['completed_at']=dt.datetime.now(dt.timezone.utc).isoformat();(a.output/'evidence.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n',encoding='utf8')
    print(json.dumps({'result':report['result'],'cases':len(report['cases']),'differences':len(report['differences'])}))
    if report['result']!='passed':raise SystemExit(1)
if __name__=='__main__':main()
