"""Independent zlib corpus plus explicit RFC 1951 bit streams; no decoder implementation."""
from __future__ import annotations
import argparse,ctypes,hashlib,json,platform,random,struct,sys,zlib
from pathlib import Path

def zlib_identity():
    origin=zlib.__spec__.origin
    if getattr(zlib,'__file__',None):path=Path(zlib.__file__).resolve()
    elif origin=='built-in' and 'zlib' in sys.builtin_module_names:
        windows=ctypes.WinDLL('kernel32.dll',use_last_error=True);windows.GetModuleFileNameW.argtypes=[ctypes.c_void_p,ctypes.c_wchar_p,ctypes.c_uint32];windows.GetModuleFileNameW.restype=ctypes.c_uint32
        name=ctypes.create_unicode_buffer(32768);size=windows.GetModuleFileNameW(sys.dllhandle,name,len(name))
        if not 0<size<len(name):raise ValueError('cannot identify the Python DLL containing builtin zlib')
        path=Path(name.value).resolve()
    else:raise ValueError('unqualified zlib implementation origin')
    return {'origin':origin,'path':str(path),'sha256':hashlib.sha256(path.read_bytes()).hexdigest(),'build_version':zlib.ZLIB_VERSION,'runtime_version':zlib.ZLIB_RUNTIME_VERSION}

class Bits:
    def __init__(self): self.bits=[]
    def number(self,value,width): self.bits.extend((value>>i)&1 for i in range(width))
    def code(self,value,width): self.bits.extend((value>>i)&1 for i in reversed(range(width)))
    def bytes(self):
        result=bytearray((len(self.bits)+7)//8)
        for i,bit in enumerate(self.bits):result[i//8]|=bit<<(i%8)
        return bytes(result)
    def stored(self,data,final=0):
        self.number(final,1);self.number(0,2)
        while len(self.bits)%8:self.number(0,1)
        for value in struct.pack('<HH',len(data),len(data)^65535)+data:self.number(value,8)

def canonical(lengths):
    counts=[0]*16
    for width in lengths: counts[width]+=1
    counts[0]=0;code=0;next_codes=[0]*16
    for width in range(1,16):code=(code+counts[width-1])*2;next_codes[width]=code
    result=[]
    for width in lengths:
        result.append((next_codes[width],width) if width else None)
        if width:next_codes[width]+=1
    return result

FIXED=canonical([8]*144+[9]*112+[7]*24+[8]*8)
DISTANCE=canonical([5]*32)
ORDER=[16,17,18,0,8,7,9,6,10,5,11,4,12,3,13,2,14,1,15]

def fixed(symbols,final=1,prefix=None):
    bits=prefix or Bits();bits.number(final,1);bits.number(1,2)
    for symbol in symbols:
        if type(symbol) is int:bits.code(*FIXED[symbol])
        else:
            kind,value,width=symbol
            if kind=='distance':bits.code(*DISTANCE[value])
            else:bits.number(value,width)
    return bits

def dynamic(literal,distance,sequence=None,cl=None,final=1):
    cl=cl or [4]*13+[5]*6  # complete alphabet: 13/16 + 6/32 = 1
    bits=Bits();bits.number(final,1);bits.number(2,2)
    bits.number(len(literal)-257,5);bits.number(len(distance)-1,5);bits.number(15,4)
    for symbol in ORDER:bits.number(cl[symbol],3)
    codes=canonical(cl)
    for item in (sequence if sequence is not None else literal+distance):
        if type(item) is int:bits.code(*codes[item])
        else:bits.number(item[0],item[1])
    return bits

def oracle(data):
    try:
        subject=zlib.decompressobj(-15);plain=subject.decompress(data);plain+=subject.flush()
    except zlib.error as error:return None,str(error)
    if not subject.eof:return None,'truncated'
    if subject.unused_data:return None,'trailing-full-bytes'
    return plain,None

def generate(output):
    cases=[];names=set();ledger=[]
    def add(name,data,plain=None,expected=None,capacity=None,group='constructed',oracle_exception=None):
        if name in names:raise AssertionError('duplicate fixture '+name)
        names.add(name);actual,error=oracle(data)
        if expected is None:expected=len(actual) if error is None else -1
        if plain is None:plain=actual or b''
        if capacity is None:capacity=len(plain) if expected>=0 else max(len(plain),65536)
        if oracle_exception:
            ledger.append({'name':name,'zlib_error':error,'sg_expected':expected,'reason':oracle_exception,'bytes':data.hex()})
        elif expected>=0 and (error is not None or actual!=plain):raise AssertionError((name,error))
        elif expected==-1 and error is None:raise AssertionError('malformed expectation accepted by zlib '+name)
        elif expected==-2 and (error is not None or actual!=plain or not 0<=capacity<len(actual)):raise AssertionError('invalid capacity fixture '+name)
        cases.append((name,data,capacity,expected,plain,group))
    rng=random.Random(0x19512026)
    texts=[b'',b'A',b'hello raw DEFLATE',bytes(range(256)),b'AB'*2048,b'A'*8192,
           ('世界😀\0RFC1951\n'*512).encode(),bytes(rng.randrange(256) for _ in range(8192)),
           b''.join(bytes([i%17])*((i%31)+1) for i in range(4000))]
    for i,plain in enumerate(texts):
        for level in [0,1,6,9]:
            for strategy in [zlib.Z_DEFAULT_STRATEGY,zlib.Z_FIXED,zlib.Z_HUFFMAN_ONLY,zlib.Z_RLE]:
                compressor=zlib.compressobj(level,zlib.DEFLATED,-15,8,strategy)
                data=compressor.compress(plain)+compressor.flush()
                add(f'zlib-{i}-{level}-{strategy}',data,plain,group='zlib-compressor')
    for i in range(80):
        n=rng.randrange(1025);plain=bytes(rng.randrange(256 if i%2 else 8) for _ in range(n))
        compressor=zlib.compressobj(rng.randrange(1,10),zlib.DEFLATED,-15)
        add('random-'+str(i),compressor.compress(plain)+compressor.flush(),plain,group='zlib-random')
    compressor=zlib.compressobj(6,zlib.DEFLATED,-15)
    first=b'cross-block-history-'*3000;second=first[-32768:]*2
    multiblock=compressor.compress(first)+compressor.flush(zlib.Z_SYNC_FLUSH)+compressor.compress(second)+compressor.flush()
    add('zlib-sync-cross-block',multiblock,first+second,group='cross-block')
    for code in [284,285]:
        for extra in ([30,31] if code==284 else [0]):
            seq=[65,code]
            if code==284:seq.append(('extra',extra,5))
            seq.extend([('distance',0,0),256])
            plain=b'A'*(1+(227+extra if code==284 else 258))
            add(f'length-{code}-extra-{extra}',fixed(seq).bytes(),plain,group='length-compatibility')
    for code,base,extra in [(257,3,0),(265,11,1),(269,19,2),(273,35,3),(277,67,4),(281,131,5)]:
        value=1 if extra else 0
        seq=[65,66,code]
        if extra:seq.append(('extra',value,extra))
        seq += [('distance',1,0),256]
        add('overlap-length-'+str(code),fixed(seq).bytes(),(b'AB'*200)[:2+base+value],group='overlap-extra-bits')
    # Every defined length and distance symbol, with deliberately non-palindromic extras.
    lb=[3,4,5,6,7,8,9,10,11,13,15,17,19,23,27,31,35,43,51,59,67,83,99,115,131,163,195,227,258]
    le=[0]*8+[1]*4+[2]*4+[3]*4+[4]*4+[5]*4+[0]
    db=[1,2,3,4,5,7,9,13,17,25,33,49,65,97,129,193,257,385,513,769,1025,1537,2049,3073,4097,6145,8193,12289,16385,24577]
    de=[0]*4+[i for i in range(1,14) for _ in range(2)]
    history=bytes(i%251 for i in range(32768))
    for index in range(30):
        bits=Bits();bits.stored(history)
        l=index%29;lextra=1 if le[l] else 0;dextra=(1<<de[index])-1 if de[index] else 0
        back=db[index]+dextra;length=lb[l]+lextra
        seq=[257+l]
        if le[l]:seq.append(('extra',lextra,le[l]))
        seq.append(('distance',index,0))
        if de[index]:seq.append(('extra',dextra,de[index]))
        seq.append(256);data=fixed(seq,prefix=bits).bytes()
        wanted=bytearray(history)
        for _ in range(length):wanted.append(wanted[-back])
        add('all-distance-'+str(index),data,bytes(wanted),group='every-distance-cross-block')
    for n in [1,30,31,32]:
        lit=[0]*256+[1];dist=[0]*n;bits=dynamic(lit,dist);bits.code(0,1)
        add('empty-distance-hdist-'+str(n),bits.bytes(),b'',expected=0,capacity=0,
            oracle_exception='RFC1951 dynamic header permits 1..32 distance lengths; unused 30/31 are never decoded; zlib rejects count>30' if n>30 else None)
    # Repeated zero lengths, including repeat16 across the literal/distance split.
    lit=[0]*256+[1];dist=[0]*7
    sequence=[18,(127,7),18,(107,7),1,0,16,(3,2)] # 138+118+1+1+6 = 264
    bits=dynamic(lit,dist,sequence);bits.code(0,1)
    add('repeat18-repeat16-zero',bits.bytes(),b'',expected=0,capacity=0)
    lit=[0]*286;lit[256]=1;dist=[0]*4
    bits=dynamic(lit,dist,[0]*256+[1]+[0]*27+[16,(3,2)]);bits.code(0,1)
    add('repeat16-cross-literal-distance-zero',bits.bytes(),b'',expected=0,capacity=0)
    lit=[0]*258
    for symbol in [65,255,256,257]:lit[symbol]=2
    dist=[2]*4
    bits=dynamic(lit,dist,[0]*65+[2]+[0]*189+[2]+[16,(3,2)])
    bits.code(0,2);bits.code(2,2)
    add('repeat16-cross-literal-distance-nonzero',bits.bytes(),b'A',group='constructed')
    lit=[0]*257
    for symbol in range(14):lit[symbol]=symbol+1
    lit[14]=lit[256]=15
    bits=dynamic(lit,[0]);codes=canonical(lit)
    for symbol in [0,7,14,256]:bits.code(*codes[symbol])
    add('maximum-code-width15',bits.bytes(),bytes([0,7,14]),group='constructed')
    # General explicit repeated code lengths from zeros, exercising 16/17/18 independently.
    for repeat,width,count,extra in [(16,2,3,0),(17,3,3,0),(18,7,11,0)]:
        lit=[0]*256+[1];dist=[0]
        sequence=[0]* (256-count)+[repeat,(extra,width)]+[1,0]
        if repeat==16 and 256-count==0:raise AssertionError()
        bits=dynamic(lit,dist,sequence);bits.code(0,1)
        add('repeat-'+str(repeat),bits.bytes(),b'',expected=0,capacity=0)
    malformed=[('reserved-block',b'\x07'),('stored-complement',b'\x01\x01\x00\xff\xffA'),
               ('stored-truncated',b'\x01\x02\x00\xfd\xffA'),
               ('reserved-literal286',fixed([286,256]).bytes()),('reserved-literal287',fixed([287,256]).bytes()),
               ('distance-before-history',fixed([257,('distance',0,0),256]).bytes()),
               ('reserved-distance30',fixed([65,257,('distance',30,0),256]).bytes()),
               ('reserved-distance31',fixed([65,257,('distance',31,0),256]).bytes())]
    for name,data in malformed:add(name,data,expected=-1,capacity=4096,group='malformed')
    for name,cl in [('oversubscribed-cl',[1]*19),('incomplete-cl',[0]*18+[1]),('empty-cl',[0]*19)]:
        # Only the header is needed: the subject must reject the code-length tree.
        add(name,dynamic([0]*256+[1],[0],[],cl).bytes(),expected=-1,capacity=4096,group='malformed-tree')
    for name,sequence in [('repeat16-no-previous',[16,(0,2)]),('repeat18-overrun',[18,(127,7)]*3),
                          ('repeat17-overrun',[0]*257+[17,(7,3)])]:
        add(name,dynamic([0]*256+[1],[0],sequence).bytes(),expected=-1,capacity=4096,group='malformed-repeat')
    for name,lit,dist in [('missing-eob',[1,1]+[0]*255,[0]),('oversubscribed-lit',[1]*257,[0]),
                          ('incomplete-lit',[0]*256+[2],[0]),('incomplete-distance',[0]*256+[1],[2]),
                          ('oversubscribed-distance',[0]*256+[1],[1,1,1])]:
        add(name,dynamic(lit,dist).bytes(),expected=-1,capacity=4096,group='malformed-tree')
    lit=[0]*258;lit[256]=lit[257]=1;bits=dynamic(lit,[0]);bits.code(1,1)
    add('empty-distance-used',bits.bytes(),expected=-1,capacity=4096,group='malformed-distance')
    bases=list(cases)
    for name,data,cap,result,plain,group in bases:
        if result>=0 and len(data)<=160:
            for cut in range(len(data)):
                add(name+'-trunc-'+str(cut),data[:cut],expected=-1,capacity=64 if len(plain)<=64 else max(cap,65536),group='truncation')
        if result>0 and group in ['constructed','overlap-extra-bits','length-compatibility','every-distance-cross-block']:
            for cap in sorted({0,max(0,result-1),result//2}):add(name+'-cap-'+str(cap),data,plain,-2,cap,'capacity')
    small=fixed([65,66,67,256]).bytes()
    for padding in [0x80,0x40,0xc0]:
        changed=small[:-1]+bytes([small[-1]|padding]);actual,error=oracle(changed)
        if error is None:add('unused-final-padding-'+str(padding),changed,actual,group='padding')
    for suffix in [b'\0',b'\xff',b'\x03\0']:
        add('trailing-'+suffix.hex(),small+suffix,expected=-1,group='exact-eof')
    for i in range(200):
        data=bytearray(rng.choice(bases)[1])
        if data:
            at=rng.randrange(len(data));data[at]^=1<<rng.randrange(8)
        actual,error=oracle(bytes(data))
        if error is None and len(actual)>1048576:continue
        add('mutation-'+str(i),bytes(data),actual,expected=len(actual) if error is None else -1,group='mutated-zlib-oracle')
    output.parent.mkdir(parents=True,exist_ok=True)
    with output.open('wb') as stream:
        stream.write(b'SGDFL1\0\0'+struct.pack('<Q',len(cases)))
        for name,data,cap,expected,plain,group in cases:
            stream.write(struct.pack('<IIqI',len(data),cap,expected,len(plain)))
            stream.write(data);stream.write(plain)
    import collections
    report={'schema':1,'source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),'python':sys.version,
            'python_executable_sha256':hashlib.sha256(Path(sys.executable).read_bytes()).hexdigest(),
            'zlib_implementation':zlib_identity(),'platform':platform.platform(),'records':len(cases),
            'sha256':hashlib.sha256(output.read_bytes()).hexdigest(),'groups':dict(collections.Counter(c[5] for c in cases)),
            'fixtures':[{'id':i,'name':c[0],'input_sha256':hashlib.sha256(c[1]).hexdigest(),'input_length':len(c[1]),'capacity':c[2],'expected':c[3],'output_length':len(c[4]),'group':c[5]} for i,c in enumerate(cases)],
            'guard_page_size':4096,'guard_fixture_ids':[i for i,c in enumerate(cases) if len(c[1])<=4096 and c[2]<=4096 and (i<16 or len(c[1])<=512)],
            'oracle_differences':ledger,'scope':'raw exact EOF and final byte padding; zlib is an independent test oracle, never the SG implementation'}
    output.with_suffix(output.suffix+'.json').write_text(json.dumps(report,indent=2)+'\n')
    return report

def large_inputs(directory):
    directory.mkdir(parents=True,exist_ok=True);limit=67108864
    original=b'A'*limit
    compressor=zlib.compressobj(6,zlib.DEFLATED,-15)
    compressed=compressor.compress(original)+compressor.flush()
    assert oracle(compressed)==(original,None)
    (directory/'large-output.deflate').write_bytes(compressed)
    rows=[{'name':'large-output.deflate','length':len(compressed),'sha256':hashlib.sha256(compressed).hexdigest(),'decoded_bytes':limit,'decoded_sha256':hashlib.sha256(original).hexdigest(),'capacity_failure':limit-1}]
    decoded=(bytes(range(251))*((67103744+250)//251))[:67103744]
    raw=bytearray()
    for index in range(1024):
        raw+=bytes([int(index==1023)])+struct.pack('<HH',65531,65531^65535)+decoded[index*65531:(index+1)*65531]
    assert len(raw)==limit and oracle(raw)==(decoded,None)
    (directory/'large-input.deflate').write_bytes(raw)
    rows.append({'name':'large-input.deflate','length':len(raw),'sha256':hashlib.sha256(raw).hexdigest(),'decoded_bytes':len(decoded),'decoded_sha256':hashlib.sha256(decoded).hexdigest(),'stored_blocks':1024,'payload_per_block':65531,'byte_pattern':'decoded index modulo 251'})
    result={'schema':1,'files':rows,'zlib_implementation':zlib_identity(),'source_sha256':hashlib.sha256(Path(__file__).read_bytes()).hexdigest()}
    (directory/'large.json').write_text(json.dumps(result,indent=2)+'\n')
    return result

if __name__=='__main__':
    parser=argparse.ArgumentParser();group=parser.add_mutually_exclusive_group(required=True);group.add_argument('--output',type=Path);group.add_argument('--large-dir',type=Path);args=parser.parse_args()
    if args.large_dir:print(json.dumps(large_inputs(args.large_dir.resolve())))
    else:
        d=generate(args.output.resolve());print(json.dumps({k:d[k] for k in ['records','sha256','groups','oracle_differences']}))
