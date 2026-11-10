"""Derive streaming event fixtures from independently observed tar formats."""
import argparse,hashlib,json,struct
from pathlib import Path
def main():
    p=argparse.ArgumentParser();p.add_argument('--corpus',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False)
    raw=a.corpus.read_bytes();manifest=json.loads(a.corpus.with_suffix('.json').read_text(encoding='utf8'));assert raw[:8]==b'SGTAR1\0\0';at=12;rows=[];data=bytearray(b'SGTS1\0\0\0'+struct.pack('<I',manifest['records']))
    for fixture in manifest['fixtures']:
        n,tc,pc,expected,tlen,plen=struct.unpack_from('<IIIqII',raw,at);at+=28;source=raw[at:at+n];at+=n;table=raw[at:at+tlen];at+=tlen;paths=raw[at:at+plen];at+=plen
        if expected==-2:
            entries=fixture['declared_entries']if fixture['declared_entries']is not None else fixture['python']['entries'];expected=len(entries);table=bytearray();paths=bytearray();total=0
            for entry in entries:
                name='/'.join(x for x in entry['name'].replace('\\','/').split('/')if x not in ['','.']).encode('utf8')
                table+=struct.pack('<6Q',entry['data']-512,entry['data'],entry['size'],len(paths),len(name),int(entry['type']=='35'));paths+=name;total+=entry['size']
            last=entries[-1];end=last['data']+((last['size']+511)//512)*512+1024
            table=struct.pack('<4Q',1,expected,total,end)+table
        data+=struct.pack('<IqII',len(source),expected,len(table),len(paths))+source+table+paths
        schedules=3+max(0,len(source)-1)if expected>=0 and len(source)<=4096 else 3
        rows.append({'id':fixture['id'],'name':fixture['name'],'expected':expected,'input_bytes':len(source),'table_bytes':len(table),'paths_bytes':len(paths),'schedules':schedules,'source_sha256':hashlib.sha256(source).hexdigest(),'difference':'fixed internal arenas eliminate old caller table/path capacity negative'if fixture['expected']==-2 else None})
    assert at==len(raw)
    (a.output/'corpus.bin').write_bytes(data);(a.output/'manifest.json').write_text(json.dumps({'schema':1,'fixtures':rows,'records':len(rows),'source_corpus_sha256':hashlib.sha256(raw).hexdigest(),'sha256':hashlib.sha256(data).hexdigest()},indent=2)+'\n');print(json.dumps({'schema':1,'records':len(rows),'sha256':hashlib.sha256(data).hexdigest()}))
if __name__=='__main__':main()
