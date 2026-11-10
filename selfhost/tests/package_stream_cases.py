"""Independent Python tar/gzip inputs for the SG byte-extraction transaction."""
from __future__ import annotations
import argparse,gzip,hashlib,io,json,tarfile,zlib
from pathlib import Path

def archive(entries,format=tarfile.GNU_FORMAT):
    output=io.BytesIO()
    with tarfile.open(fileobj=output,mode='w',format=format)as stream:
        for name,data,kind in entries:
            info=tarfile.TarInfo(name);info.type=kind;info.size=len(data)if kind==tarfile.REGTYPE else 0
            stream.addfile(info,io.BytesIO(data))
    return output.getvalue()

def header(name,size=0,kind=tarfile.REGTYPE):
    info=tarfile.TarInfo(name);info.size=size;info.type=kind
    return info.tobuf(tarfile.GNU_FORMAT,'utf8','strict')

def pax_padding(size):
    digits=1
    while True:
        actual=len(str(size))
        body=b'comment='+b'X'*(size-actual-1-8-1)
        result=str(size).encode()+b' '+body+b'\n'
        assert len(result)==size
        return result

def main():
    parser=argparse.ArgumentParser();parser.add_argument('--output',type=Path,required=True);parser.add_argument('--large',action='store_true');a=parser.parse_args();a.output.mkdir(parents=True,exist_ok=False)
    rows=[]
    def add(name,raw=None,wire=None,expected=0,cancel=-1,attack='none',oom=-1,existing=False):
        if wire is None:wire=gzip.compress(raw,mtime=0)
        output={};directories=[];oracle_error=None
        try:
            plain=gzip.decompress(wire)
            if expected==0:
                with tarfile.open(fileobj=io.BytesIO(plain),mode='r:')as tar:
                    for entry in tar:
                        if entry.isdir():directories.append(entry.name.rstrip('/'))
                        elif entry.isfile():output[entry.name]=hashlib.sha256(tar.extractfile(entry).read()).hexdigest()
                        else:raise AssertionError('positive special entry')
        except (OSError,EOFError,zlib.error,tarfile.TarError)as error:oracle_error=type(error).__name__+': '+str(error)
        if expected==0:assert oracle_error is None
        path=a.output/(name+'.tar.gz');path.write_bytes(wire)
        rows.append({'id':len(rows),'name':name,'path':path.name,'sha256':hashlib.sha256(wire).hexdigest(),'wire':len(wire),'raw_sha256':hashlib.sha256(raw).hexdigest()if raw is not None else None,'expected':expected,'cancel_after':cancel,'attack':attack,'oom':oom,'existing_target':existing,'files':output,'directories':directories,'python_error':oracle_error})
    entries=[('Sengoo.toml',b'[package]\nname="demo"\nversion="1.0.0"\n',tarfile.REGTYPE),('src/main.sg',b'def main()->i64 {42;}\n',tarfile.REGTYPE),('assets/中文.txt','hello中文'.encode(),tarfile.REGTYPE)]
    raw=archive(entries);wire=gzip.compress(raw,mtime=0)
    add('gnu',raw);add('ustar',archive(entries,tarfile.USTAR_FORMAT));add('pax',archive(entries+[('long/'+('中文'*90)+'.txt',b'long path',tarfile.REGTYPE)],tarfile.PAX_FORMAT))
    add('empty-input',wire=b'',expected=-1)
    add('binary-file',archive([('data.bin',bytes(range(256))*257,tarfile.REGTYPE)]))
    add('explicit-dir-after-implicit',archive(entries+[('src',b'',tarfile.DIRTYPE)]))
    add('concat-split',raw,wire=gzip.compress(raw[:777],mtime=0)+gzip.compress(raw[777:],mtime=0))
    add('empty-first-member',raw,wire=gzip.compress(b'',mtime=0)+wire)
    for name,index in [('crc',-8),('size',-4)]:
        bad=bytearray(wire);bad[index]^=1;add('bad-'+name,raw,bytes(bad),-1)
    bad=bytearray(gzip.compress(b'',mtime=0));bad[-8]^=1;add('bad-late-member',raw,wire+bad,-1)
    add('bad-trailer-truncated',wire=wire[:-1],expected=-1);add('bad-wire-tail',wire=wire+b'X',expected=-1)
    add('bad-tar-tail',raw+b'X',expected=-1);add('bad-tar-truncated',raw[:1536],expected=-1)
    for name,path in [('parent','../escape'),('absolute','/escape'),('ads','safe:stream'),('dos','CON'),('utf8-control','a\x01b')]:
        add('unsafe-'+name,archive([(path,b'bad',tarfile.REGTYPE)]),expected=-4)
    for name,paths in [('case',['value','VALUE']),('file-parent',['a','a/b']),('file-parent-reverse',['a/b','a'])]:
        add('conflict-'+name,archive([(x,b'x',tarfile.REGTYPE)for x in paths]),expected=-6)
    for name,kind in [('symlink',tarfile.SYMTYPE),('hardlink',tarfile.LNKTYPE),('device',tarfile.CHRTYPE)]:
        add('special-'+name,archive([('special',b'',kind)]),expected=-5)
    add('cancel-first',raw,expected=-9,cancel=0);add('cancel-after-write',raw,expected=-9,cancel=2)
    for index in range(4):add('oom-'+str(index),raw,expected=-8,oom=index)
    add('oom-session',raw,expected=-8,attack='session-oom')
    add('existing-final',raw,expected=-8,existing=True);add('rename-race',raw,expected=-8,attack='target')
    add('real-write-denied',raw,expected=-8,attack='readonly')
    add('unknown-cleanup',raw,expected=-11,attack='unknown',cancel=0)
    add('junction-parent',raw,expected=-11,attack='reparse')
    add('replaced-object',raw,expected=-11,attack='replace')
    if a.large:
        add('content-over-64MiB',archive([('large.bin',b'A'*(65*1024*1024),tarfile.REGTYPE)]))
        add('content-max-256MiB',header('large.bin',268435456)+b'A'*268435456+bytes(1024))
        add('content-over-256MiB',header('large.bin',268435457),expected=-7)
        alphabet='0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz_-'
        tenk=b''.join(header(alphabet[i%64]+str(i))for i in range(10000))
        add('entries-max-10000',tenk+bytes(1024));add('entries-over-10000',tenk+header('more'),expected=-7)
        meta=header('pax',1048576,tarfile.XHDTYPE)+pax_padding(1048576)
        add('metadata-max-single',meta+header('single')+bytes(1024))
        add('metadata-over-single',header('pax',1048577,tarfile.XHDTYPE),expected=-7)
        many=b''.join(meta+header('entry'+str(i))for i in range(16))
        add('metadata-max-aggregate',many+bytes(1024))
        add('metadata-over-aggregate',many+header('pax',1,tarfile.XHDTYPE),expected=-7)
    (a.output/'manifest.json').write_text(json.dumps({'schema':1,'scope':'byte extraction only; full resolver identity is separate','fixtures':rows},indent=2,ensure_ascii=False)+'\n',encoding='utf8')
    print(json.dumps({'schema':1,'fixtures':len(rows)}))
if __name__=='__main__':main()
