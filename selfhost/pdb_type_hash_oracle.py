"""Independent LLVM-compatible TPI hash/index oracle; no product type-record rewriting."""
from __future__ import annotations
import struct,zlib

def u16(data,at):return struct.unpack_from('<H',data,at)[0]
def u32(data,at):return struct.unpack_from('<I',data,at)[0]
def need(ok,message):
    if not ok:raise AssertionError(message)
def hash_string(data):
    value=0;at=0
    while at+4<=len(data):value^=u32(data,at);at+=4
    if at+2<=len(data):value^=u16(data,at);at+=2
    if at<len(data):value^=data[at]
    value|=0x20202020;value^=value>>11
    return (value^(value>>16))&0xffffffff

def text(data,at):
    end=data.find(b'\0',at);need(end>=at,'TPI UDT name terminator');return data[at:end],end+1

def records(tpi):
    need(len(tpi)>=56 and u32(tpi,0)==20040203 and u32(tpi,4)==56,'TPI header')
    need(u32(tpi,8)==4096 and 0<=u32(tpi,12)-4096<=4096,'TPI type index bounds')
    raw=tpi[56:];need(len(raw)==u32(tpi,16) and len(raw)<=1048576,'TPI type record extent')
    result=[];at=0
    while at<len(raw):
        need(at+4<=len(raw),'TPI record prefix');size=u16(raw,at)+2;need(size>=4 and size%4==0 and at+size<=len(raw),'TPI record extent/alignment');rec=raw[at:at+size];kind=u16(rec,2)
        if kind in (0x1504,0x1505,0x1519,0x1506,0x1507):
            properties=u16(rec,6)
            if kind==0x1507:name_at=16
            else:
                numeric=20 if kind in (0x1504,0x1505,0x1519)else 12;leaf=u16(rec,numeric);need(leaf<0x8000 or leaf==0x8004,'TPI supported numeric form');name_at=numeric+(2 if leaf<0x8000 else 6)
            name,next_at=text(rec,name_at);need(name,'TPI UDT empty name');unique=text(rec,next_at)[0]if properties&0x200 else b'';anonymous=name in (b'<unnamed-tag>',b'__unnamed')or name.endswith((b'::<unnamed-tag>',b'::__unnamed'));forward=properties&0x80;scoped=properties&0x100
            if not forward and not scoped and not(properties&0x200 and anonymous):value=hash_string(name)
            elif not forward and properties&0x200 and not anonymous:value=hash_string(unique)
            else:value=(zlib.crc32(rec,0xffffffff)^0xffffffff)&0xffffffff
        elif kind in (0x1606,0x1607):value=hash_string(rec[4:8])
        else:value=(zlib.crc32(rec,0xffffffff)^0xffffffff)&0xffffffff
        result.append({'type_index':4096+len(result),'offset':at,'length':size,'kind':kind,'hash':value%262143});at+=size
    need(len(result)==u32(tpi,12)-4096,'TPI declared type count');return result

def validate_tpi_hashes(streams):
    tpi=streams[2];items=records(tpi);index=u16(tpi,20)
    need(index==len(streams)-1 and index>=11 and u16(tpi,22)==0xffff,'required last TPI hash stream')
    need(u32(tpi,24)==4 and u32(tpi,28)==262143,'TPI hash key/bucket contract')
    expected_hashes=b''.join(struct.pack('<I',item['hash'])for item in items);pairs=[]
    for item in items:
        if not pairs or (item['offset']+item['length'])//8192>item['offset']//8192:pairs.append((item['type_index'],item['offset']))
    expected_offsets=b''.join(struct.pack('<II',*pair)for pair in pairs)
    need(tuple(u32(tpi,at)for at in range(32,56,4))==(0,len(expected_hashes),len(expected_hashes),len(expected_offsets),len(expected_hashes),0),'TPI exact hash/index/adjustment ranges')
    need(streams[index]==expected_hashes+expected_offsets,'TPI exact hash and index-offset bytes')
    return {'stream_index':index,'record_count':len(items),'hash_bytes':len(expected_hashes),'index_offsets':[list(pair)for pair in pairs]}
