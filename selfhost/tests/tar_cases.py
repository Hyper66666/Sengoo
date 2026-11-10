"""Rebuildable tar fixtures; Python tarfile is an independent format oracle."""
from __future__ import annotations
import argparse,ctypes,hashlib,io,json,random,struct,tarfile
from pathlib import Path

def header(name='file',size=0,kind=b'0',fmt=tarfile.GNU_FORMAT):
    value=tarfile.TarInfo(name);value.size=size;value.type=kind
    return value.tobuf(fmt,encoding='utf8')

def checksum(block):
    value=bytearray(block);value[148:156]=b' '*8
    value[148:156]=('%06o\0 ' % sum(value)).encode();return bytes(value)

def member(name='file',data=b'',kind=b'0',fmt=tarfile.GNU_FORMAT):
    return header(name,len(data),kind,fmt)+data+bytes(-len(data)%512)

def pax_record(key,value):
    body=key.encode()+b'='+value+b'\n';size=len(body)+2
    while True:
        candidate=len(str(size))+1+len(body)
        if candidate==size:return str(size).encode()+b' '+body
        size=candidate

def archive(entries,fmt):
    out=io.BytesIO()
    with tarfile.open(fileobj=out,mode='w',format=fmt,encoding='utf8') as made:
        for name,data,kind in entries:
            item=tarfile.TarInfo(name);item.type=kind;item.size=len(data)
            made.addfile(item,io.BytesIO(data))
    return out.getvalue()

def observe(source):
    try:
        with tarfile.open(fileobj=io.BytesIO(source),mode='r:',encoding='utf8',errors='strict') as value:
            rows=[]
            for item in value:
                content=value.extractfile(item).read() if item.isfile() else b''
                rows.append({'name':item.name,'type':item.type.hex(),'size':item.size,'header':item.offset,'data':item.offset_data,'content_sha256':hashlib.sha256(content).hexdigest()})
            return {'ok':True,'entries':rows}
    except (tarfile.TarError,UnicodeError,ValueError,OverflowError) as error:return {'ok':False,'error':type(error).__name__+': '+str(error)}

def main():
    p=argparse.ArgumentParser();p.add_argument('--output',type=Path,required=True);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False)
    rows=[]
    def add(name,source,expected=None,table_capacity=480032,path_capacity=1048576,declared_entries=None):
        observed=observe(source);entries=observed.get('entries',[]) if declared_entries is None else declared_entries
        if expected is None:
            if not observed['ok']:raise AssertionError((name,observed))
            expected=len(entries)
        paths=bytearray();record=bytearray();total=0
        if expected>=0:
            if expected!=len(entries):raise AssertionError(name)
            for item in entries:
                normalized='/'.join(part for part in item['name'].replace('\\','/').split('/') if part not in ['','.'])
                raw=normalized.encode('utf8');record+=struct.pack('<6Q',item['data']-512,item['data'],item['size'],len(paths),len(raw),int(item['type']=='35'));paths+=raw;total+=item['size']
            terminal=512
            if entries:
                last=entries[-1];terminal=last['data']+((last['size']+511)//512)*512+1024
            else:terminal=1024
            record=bytearray(struct.pack('<4Q',1,expected,total,terminal))+record
        rows.append({'id':len(rows),'name':name,'source':source,'expected':expected,'table_capacity':table_capacity,'path_capacity':path_capacity,'table':bytes(record),'paths':bytes(paths),'python':observed,'declared_entries':declared_entries})
    basic=[('Sengoo.toml',b'[package]\nname="demo"\n',b'0'),('src/',b'',b'5'),('src/main.sg',b'def main()->i64{42}',b'0')]
    for name,fmt in [('gnu',tarfile.GNU_FORMAT),('ustar',tarfile.USTAR_FORMAT),('pax',tarfile.PAX_FORMAT)]:add(name,archive(basic,fmt))
    add('ustar-prefix',archive([('a'*110+'/source.sg',b'prefix',b'0')],tarfile.USTAR_FORMAT))
    for name in ['nested/'+('long-'*35)+'source.sg','目录/é/𐐀.sg']:
        for label,fmt in [('gnu',tarfile.GNU_FORMAT),('pax',tarfile.PAX_FORMAT)]:add(label+'-long-'+str(len(rows)),archive([(name,b'hello',b'0')],fmt))
    add('old-v7',member('old',b'bytes',fmt=tarfile.USTAR_FORMAT)[:257]+bytes(255)+b'bytes'+bytes(507)+bytes(1024),-1)
    old=bytearray(member('old',b'bytes'));old[257:512]=bytes(255);old[:512]=checksum(old[:512]);add('old-v7-valid',bytes(old)+bytes(1024))
    add('normalize',member('./src//part\\file.sg',b'x')+bytes(1024))
    for name in ['../outside','safe/../bad','/absolute','\\server\\share','C:drive','dir/a:b','a\x00b','a.','a ','dir/NUL.txt','CON','prn','AUX','COM9','LPT1','COM¹','COM².txt','LPT³','a?b','a\x01b','.sgpm-archive.sha256','.SGPM-content.sha256','.sgpm-registry-metadata.json']:
        if '\x00' in name:
            source=member('placeholder',b'x');block=bytearray(source[:512]);block[:len(name)]=name.encode();source=checksum(block)+source[512:]
            # Header NUL terminates its name; this is a valid short name, not an embedded path byte.
            add('header-nul-terminates',source+bytes(1024));continue
        add('unsafe-'+name.encode().hex(),member(name,b'x')+bytes(1024),-4)
    for names in [('same','same'),('a','./a'),('a\\b','a/b'),('CASE','case'),('Ä','ä'),('file','file/child'),('file/child','file')]:
        add('collision-'+str(len(rows)),member(names[0],b'x')+member(names[1],b'y')+bytes(1024),-6)
    compare=ctypes.windll.kernel32.CompareStringOrdinal
    compare.argtypes=[ctypes.c_wchar_p,ctypes.c_int,ctypes.c_wchar_p,ctypes.c_int,ctypes.c_int];compare.restype=ctypes.c_int
    pairs=[('ASCII','ascii'),('Σ','σ'),('Σ','ς'),('I','i'),('I','ı'),('İ','i'),('ß','SS'),('K','k'),('𐐀','𐐨'),('Ä','ä'),('ẞ','ß'),('Ⱥ','ⱥ'),('Ɫ','ɫ')]
    rng=random.Random(0x7419503)
    alphabet=['A','a','Σ','σ','ς','I','i','ı','İ','ß','ẞ','K','k','𐐀','𐐨','Ä','ä','Д','д','é','É']
    for unused in range(100):
        left=''.join(rng.choice(alphabet) for unused in range(rng.randrange(1,8)));right=left.swapcase() if rng.randrange(2) else ''.join(rng.choice(alphabet) for unused in range(rng.randrange(1,8)))
        pairs.append((left,right))
    windows=[]
    for left,right in pairs:
        same=compare(left,-1,right,-1,True)==2
        windows.append({'left':left,'right':right,'equal':same})
        add('windows-whole-path-'+str(len(windows)),member(left,b'x')+member(right,b'y')+bytes(1024),-6 if same else 2)
    for left,right in [('Ⱥ','ⱥ'),('Ɫ','ɫ')]:
        assert len(left.encode())!=len(right.encode()) and compare(left,-1,right,-1,True)==2
        add('windows-variable-width-prefix-'+str(len(rows)),member(left,b'x')+member(right+'/child',b'y')+bytes(1024),-6)
        add('windows-variable-width-prefix-reverse-'+str(len(rows)),member(right+'/child',b'x')+member(left,b'y')+bytes(1024),-6)
    add('directories-may-parent',member('dir',b'',b'5')+member('dir/file',b'x')+bytes(1024))
    for kind in [b'1',b'2',b'3',b'4',b'6',b'7',b'S',b'g',b'K']:
        add('reject-type-'+kind.hex(),member('kind',b'',kind)+bytes(1024),-5)
    add('directory-content',member('dir',b'x',b'5')+bytes(1024),-5)
    plain=member('file',b'abc')+bytes(1024)
    for n in [0,1,511,512,513,1023,1024,1535,1536,2047]:add('truncated-'+str(n),plain[:n],-1)
    add('single-end-block',plain[:-512],-1)
    add('nonzero-tail',plain+b'x',-1)
    damaged=bytearray(plain);damaged[0]^=1;add('bad-checksum',bytes(damaged),-1)
    for field in [b'88888888888\0',b'77777777777\0',b'\xff'*12,b'\x80'+b'\x01'+bytes(10)]:
        damaged=bytearray(plain);damaged[124:136]=field;damaged[:512]=checksum(damaged[:512]);add('bad-number-'+field.hex(),bytes(damaged),-1 if field[0] in [56,255,128] else -7)
    binary=bytearray(plain);binary[124:136]=b'\x80'+bytes(10)+b'\x03';binary[:512]=checksum(binary[:512]);add('base256-positive',bytes(binary))
    pd=pax_record('path',b'pax/actual')+pax_record('size',b'3')
    add('pax-effective-size',member('PaxHeader',pd,b'x',tarfile.USTAR_FORMAT)+header('wrong',0)+b'abc'+bytes(509)+bytes(1024))
    content_hash=hashlib.sha256(b'abc').hexdigest()
    declaration=lambda name,data_offset:[{'name':name,'type':'30','size':3,'header':data_offset-512,'data':data_offset,'content_sha256':content_hash}]
    pd=pax_record('path',b'first')+pax_record('path',b'second')
    add('pax-duplicate-path-first',member('PaxHeader',pd,b'x',tarfile.USTAR_FORMAT)+member('fallback',b'abc')+bytes(1024),1,declared_entries=declaration('first',1536))
    pd=pax_record('path',b'pax-would-lose')
    add('gnu-name-precedes-pax',member('PaxHeader',pd,b'x',tarfile.USTAR_FORMAT)+member('././@LongLink',b'gnu-wins\0',b'L')+member('fallback',b'abc')+bytes(1024),1,declared_entries=declaration('gnu-wins',2560))
    for encoded in [b'bad\xc0\xaf',b'bad\xed\xa0\x80',b'bad\xf4\x90\x80\x80',b'bad\xe2\x82',b'bad\0rest']:
        add('invalid-long-utf8-'+encoded.hex(),member('././@LongLink',encoded+b'\0',b'L')+plain,-4)
    badck=bytearray(plain);badck[148:156]=b'\x80'+bytes(6)+b'\x01';add('base256-checksum-rejected',bytes(badck),-1)
    add('pax-bad-record',member('PaxHeader',b'99 path=file\n',b'x',tarfile.USTAR_FORMAT)+plain,-1)
    add('pax-bad-size',member('PaxHeader',pax_record('size',b'no'),b'x',tarfile.USTAR_FORMAT)+plain,-1)
    add('orphan-pax',member('PaxHeader',pax_record('path',b'x'),b'x',tarfile.USTAR_FORMAT)+bytes(1024),-1)
    add('orphan-longname',member('././@LongLink',b'long\0',b'L')+bytes(1024),-1)
    add('duplicate-longname',member('././@LongLink',b'one\0',b'L')+member('././@LongLink',b'two\0',b'L')+plain,-1)
    add('longname-state-cleared',member('././@LongLink',b'first-long\0',b'L')+member('fallback',b'a')+member('second',b'b')+bytes(1024))
    add('pax-state-cleared',member('PaxHeader',pax_record('path',b'first-pax'),b'x',tarfile.USTAR_FORMAT)+member('fallback',b'a')+member('second',b'b')+bytes(1024))
    path=('p'*100+'/')*648+'q'*87
    assert len(path)==65535
    add('path-limit-65535',member('././@LongLink',path.encode()+b'\0',b'L')+member('fallback',b'x')+bytes(1024))
    add('path-limit-65536',member('././@LongLink',path.encode()+b'x\0',b'L')+member('fallback',b'x')+bytes(1024),-4)
    def metadata_exact(size):
        padding=size-20
        while True:
            value=pax_record('comment',b'a'*padding)
            if len(value)==size:return value
            padding+=size-len(value)
    maximum=metadata_exact(1048576)
    add('metadata-member-exact-1MiB',member('PaxHeader',maximum,b'x',tarfile.USTAR_FORMAT)+member('file',b'x')+bytes(1024))
    add('metadata-member-over-1MiB',member('PaxHeader',maximum+b'x',b'x',tarfile.USTAR_FORMAT)+plain,-7)
    accumulated=b''.join(member('PaxHeader',maximum,b'x',tarfile.USTAR_FORMAT)+member('file'+str(i),b'') for i in range(16))
    add('metadata-aggregate-exact-16MiB',accumulated+bytes(1024))
    add('metadata-aggregate-over-16MiB',accumulated+member('PaxHeader',b'x',b'x',tarfile.USTAR_FORMAT)+plain,-7)
    def entry_name(index):
        digits='0123456789abcdefghijklmnopqrstuvwxyz';name=''
        for unused in range(3):name+=digits[index%36];index//=36
        return name+'x'
    many=b''.join(member(entry_name(index),b'') for index in range(10000))
    add('entry-count-exact-10000',many+bytes(1024))
    add('entry-count-over-10000',many+member(entry_name(10000),b'')+bytes(1024),-7)
    add('table-short',plain,-2,32)
    add('table-one-byte-short',plain,-2,79)
    add('table-exact',plain,1,80,4)
    add('path-short',plain,-2,480032,3)
    add('empty-tar',bytes(1024))
    output=bytearray(b'SGTAR1\0\0'+struct.pack('<I',len(rows)))
    manifest=[]
    for row in rows:
        output+=struct.pack('<IIIqII',len(row['source']),row['table_capacity'],row['path_capacity'],row['expected'],len(row['table']),len(row['paths']))+row['source']+row['table']+row['paths']
        manifest.append({k:v for k,v in row.items() if k not in ['source','table','paths']}|{'source_sha256':hashlib.sha256(row['source']).hexdigest(),'table_sha256':hashlib.sha256(row['table']).hexdigest(),'paths_sha256':hashlib.sha256(row['paths']).hexdigest()})
    (a.output/'corpus.bin').write_bytes(output);(a.output/'corpus.json').write_text(json.dumps({'schema':1,'records':len(rows),'corpus_sha256':hashlib.sha256(output).hexdigest(),'fixtures':manifest,'windows_whole_path_observations':windows},indent=2,ensure_ascii=False)+'\n',encoding='utf8')
    print(json.dumps({'schema':1,'records':len(rows),'sha256':hashlib.sha256(output).hexdigest()}))
if __name__=='__main__':main()
