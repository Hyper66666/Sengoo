"""RFC1952 constructed headers and independent Python zlib/gzip framing comparisons."""
from __future__ import annotations
import argparse,gzip,hashlib,json,random,struct,sys,zlib
from pathlib import Path
from deflate_cases import zlib_identity

def member(plain,flags=0,extra=b'',name=b'',comment=b'',level=6):
    header=bytes([31,139,8,flags])+struct.pack('<I',0x36292026)+bytes([2,255])
    if flags&4:header+=struct.pack('<H',len(extra))+extra
    if flags&8:header+=name+b'\0'
    if flags&16:header+=comment+b'\0'
    if flags&2:header+=struct.pack('<H',zlib.crc32(header)&65535)
    raw=zlib.compressobj(level,zlib.DEFLATED,-15);body=raw.compress(plain)+raw.flush()
    return header+body+struct.pack('<II',zlib.crc32(plain),len(plain)&0xffffffff)

def decode(source,mode):
    out=b'';consumed=0
    try:
        while True:
            subject=zlib.decompressobj(31);decoded=subject.decompress(source[consumed:])+subject.flush()
            if not subject.eof:return None,None,'truncated'
            used=len(source)-consumed-len(subject.unused_data)
            out+=decoded;consumed+=used
            if mode==0 or consumed==len(source):return out,consumed,None
            if mode==1:return None,None,'trailing-first-member'
    except zlib.error as error:return None,None,str(error)

def generate(path):
    rows=[];names=set();differences=[]
    def add(name,source,mode=0,plain=None,expected=None,capacity=None,group='constructed',difference=None):
        if name in names:raise AssertionError(name)
        names.add(name);actual,used,error=decode(source,mode)
        if plain is None:plain=actual or b''
        if expected is None:expected=-1 if error else (used<<27)|len(actual)
        if capacity is None:capacity=len(plain) if expected>=0 else 4096
        if difference:differences.append({'name':name,'reason':difference,'python_result':None if error else {'consumed':used,'decoded':len(actual)},'expected':expected})
        elif expected>=0 and (error or actual!=plain or expected!=(used<<27)|len(plain)):raise AssertionError((name,error))
        elif expected==-1 and not error:raise AssertionError('false malformed '+name)
        elif expected==-2 and (error or actual!=plain or not 0<=capacity<len(actual)):raise AssertionError('false capacity '+name)
        rows.append((name,bytes(source),mode,capacity,expected,plain,group))
    rng=random.Random(0x19522026)
    texts=[b'',b'A',b'hello gzip',bytes(range(256)),b'AB'*3000,('世界😀\0gzip\n'*200).encode(),bytes(rng.randrange(256) for _ in range(4096))]
    for i,plain in enumerate(texts):
        for level in [0,1,6,9]:
            data=gzip.compress(plain,compresslevel=level,mtime=0)
            for mode in range(3):add(f'python-{i}-{level}-{mode}',data,mode,plain,group='python-gzip')
    for flags in range(32):
        plain=b'optional-fields-'+bytes([flags]);data=member(plain,flags,b'\0\xffXY\x01\0Z',b'filename\xff',b'comment\xfe')
        for mode in range(3):add(f'flags-{flags}-mode-{mode}',data,mode,plain,group='optional-flags')
    single=member(b'first',31,b'EX',b'a',b'b');other=member(b'second',2)
    for tail,label in [(b'junk','garbage'),(b'\0','zero'),(bytes(32),'zeros'),(other,'concat'),(other[:-1],'bad-next')]:
        for mode in range(3):add(f'tail-{label}-{mode}',single+tail,mode,group='tail-policy')
    for n in [1,2,5,20]:
        data=b''.join(member(bytes([i]),i%32,b'',b'x',b'y') for i in range(n))
        for mode in range(3):add(f'concat-{n}-{mode}',data,mode,group='members')
    for field in ['name','comment']:
        flag=8 if field=='name' else 16
        for n in [65534,65535,65536]:
            data=member(b'',flag,name=b'N'*n if field=='name' else b'',comment=b'C'*n if field=='comment' else b'')
            add(f'{field}-length-{n}',data,0,b'',expected=-1 if n>65535 else None,capacity=0,group='header-limit',difference='flate2 MAX_HEADER_BUF accepts at most65535 non-NUL bytes; Python zlib allows longer' if n>65535 else None)
    add('extra-length-65535',member(b'',4,bytes(i%251 for i in range(65535))),plain=b'',capacity=0,group='header-limit')
    for name,data in [('empty',b''),('magic',b'not gzip!!'),('method',bytes([31,139,7,0])+bytes(6)),
                      ('reserved-flags',bytes([31,139,8,32])+bytes(6)),('unterminated-name',bytes([31,139,8,8])+bytes(6)+b'name'),
                      ('unterminated-comment',bytes([31,139,8,16])+bytes(6)+b'comment'),
                      ('truncated-extra',bytes([31,139,8,4])+bytes(6)+b'\xff\xffABC')]:
        for mode in range(3):add(name+'-'+str(mode),data,mode,expected=-1,group='malformed-header')
    for bit in [5,6,7]:add('reserved-bit-'+str(bit),member(b'answer',1<<bit),expected=-1,group='malformed-header')
    for where,label in [(-8,'crc32'),(-4,'isize'),(16,'header-crc')]:
        data=bytearray(member(b'checksum-data',2));at=10 if label=='header-crc' else len(data)+where;data[at]^=1
        add('corrupt-'+label,data,expected=-1,plain=b'checksum-data' if label!='header-crc' else b'',group='malformed-integrity')
    base=list(rows)
    for name,data,mode,capacity,expected,plain,group in base:
        if expected>=0 and len(data)<=96:
            for cut in range(len(data)):
                add(name+'-cut-'+str(cut),data[:cut],mode,capacity=max(64,len(plain)),group='truncation-policy')
        if expected>=0 and plain and group in ['optional-flags','members']:
            add(name+'-capacity',data,mode,plain,-2,len(plain)-1,'capacity')
    mutation_pool=[row for row in base if row[-1]!='header-limit']
    for i in range(160):
        name,data,_,_,_,_,_=rng.choice(mutation_pool);changed=bytearray(data)
        if changed:changed[rng.randrange(len(changed))]^=1<<rng.randrange(8)
        for mode in [0,2]:
            plain,used,error=decode(changed,mode)
            if plain is not None and len(plain)>1048576:continue
            # A malformed stream may emit a long prefix before its eventual
            # checksum/tree failure. Give it a format-derived upper bound:
            # at most258 output bytes per consumed bit (a deliberately loose
            # bound), so capacity cannot preempt the intended malformed error.
            capacity=max(4096,len(changed)*8*258) if error else len(plain)
            if capacity>67108864:raise AssertionError('mutation proof extent exceeds API bound')
            add(f'mutate-{i}-{mode}',changed,mode,plain,expected=-1 if error else None,capacity=capacity,group='mutation')
    path.parent.mkdir(parents=True,exist_ok=True)
    with path.open('wb') as stream:
        stream.write(b'SGGZP1\0\0'+struct.pack('<Q',len(rows)))
        for _,data,mode,capacity,expected,plain,_ in rows:
            stream.write(struct.pack('<IIqII',len(data),capacity,expected,len(plain),mode));stream.write(data);stream.write(plain)
    import collections
    python_gzip=[]
    for name,data,mode,capacity,expected,plain,group in base:
        if group in ['tail-policy','members','header-limit','malformed-header','malformed-integrity']:
            try:
                output=gzip.decompress(data);observation={'ok':True,'bytes':len(output),'sha256':hashlib.sha256(output).hexdigest()}
            except (OSError,EOFError,zlib.error) as error:observation={'ok':False,'error_type':type(error).__name__}
            python_gzip.append({'name':name,'observation':observation})
    report={'schema':1,'records':len(rows),'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'groups':dict(collections.Counter(row[-1] for row in rows)),
            'source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'zlib_implementation':zlib_identity(),'python':sys.version,
            'fixtures':[{'id':i,'name':row[0],'input_length':len(row[1]),'input_sha256':hashlib.sha256(row[1]).hexdigest(),'mode':row[2],'capacity':row[3],'expected':row[4],'output_length':len(row[5]),'group':row[6]} for i,row in enumerate(rows)],
            'python_gzip_observations':python_gzip,'oracle_differences':differences,'policy':'first/strictsingle/strictconcat via independent zlib eof+unused_data; empty input is invalid in every mode; metadata limit is explicit flate2 compatibility'}
    path.with_suffix(path.suffix+'.json').write_text(json.dumps(report,indent=2)+'\n');return report

def large_inputs(directory):
    directory.mkdir(parents=True,exist_ok=True)
    one=member(b'A'*1048576,2);joined=one*64
    plain=gzip.decompress(joined)
    assert len(plain)==67108864 and plain==b'A'*67108864
    (directory/'large-concat.gzip').write_bytes(joined)
    rows=[{'name':'large-concat.gzip','sha256':hashlib.sha256(joined).hexdigest(),'bytes':len(joined),'members':64,'first_member_bytes':len(one),'first_decoded_bytes':1048576,'decoded_bytes':len(plain),'decoded_sha256':hashlib.sha256(plain).hexdigest()}]
    count=67103726;plain=(bytes(range(251))*((count+250)//251))[:count]
    stream=bytearray(bytes([31,139,8,0])+bytes(6))
    at=0
    for index in range(1024):
        length=65531 if index<1023 else 65513
        stream+=bytes([int(index==1023)])+struct.pack('<HH',length,length^65535)+plain[at:at+length];at+=length
    stream+=struct.pack('<II',zlib.crc32(plain),len(plain))
    assert at==count and len(stream)==67108864 and gzip.decompress(stream)==plain
    (directory/'large-input.gzip').write_bytes(stream)
    rows.append({'name':'large-input.gzip','sha256':hashlib.sha256(stream).hexdigest(),'bytes':len(stream),'members':1,'decoded_bytes':count,'decoded_sha256':hashlib.sha256(plain).hexdigest(),'output_pattern':'index modulo251'})
    result={'schema':1,'files':rows,'zlib_implementation':zlib_identity(),'source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    (directory/'large.json').write_text(json.dumps(result,indent=2)+'\n');return result

if __name__=='__main__':
    parser=argparse.ArgumentParser();group=parser.add_mutually_exclusive_group(required=True);group.add_argument('--output',type=Path);group.add_argument('--large-dir',type=Path);args=parser.parse_args()
    if args.large_dir:print(json.dumps(large_inputs(args.large_dir.resolve())))
    else:
        d=generate(args.output.resolve());print(json.dumps({key:d[key] for key in ['records','sha256','groups','oracle_differences']}))
