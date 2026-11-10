"""Independent chunked-gzip boundary inputs; no subject decoder/oracle feedback."""
from __future__ import annotations
import argparse,gzip,hashlib,json,struct,zlib
from pathlib import Path
from gzip_cases import member,decode
from deflate_cases import zlib_identity

def main():
    p=argparse.ArgumentParser();p.add_argument('--output',required=True,type=Path);a=p.parse_args();a.output.mkdir(parents=True,exist_ok=False)
    report={'schema':1,'zlib_implementation':zlib_identity(),'files':[],'scope':'bounded reusable chunks; output limit is cumulative, not a full output arena'}
    plain=bytes(index%251 for index in range(1048576));one=member(plain,31,b'EXTRA',b'name',b'comment')
    data=one*65
    parts=[];used=0;rolling=hashlib.sha256()
    for i in range(65):
        obj=zlib.decompressobj(31);decoded=obj.decompress(data[used:])+obj.flush()
        assert obj.eof and decoded==plain
        consumed=len(data)-used-len(obj.unused_data);assert consumed==len(one)
        rolling.update(decoded);used+=consumed
    assert used==len(data)
    output=a.output/'wide-members.gzip';output.write_bytes(data)
    report['files'].append({'name':'wide-members','path':str(output.resolve()),'sha256':hashlib.sha256(data).hexdigest(),'input_bytes':len(data),'member_bytes':len(one),'members':65,'output_bytes':68157440,'member_output':1048576,'output_sha256':rolling.hexdigest(),'pattern':'(global_index % 1048576) % 251','python_gzip_equal':hashlib.sha256(gzip.decompress(data)).hexdigest()==rolling.hexdigest()})
    first=member(b'A')
    # Fixed length=3/distance=1 before any literal: valid header/tree, invalid history.
    invalid_raw=bytes.fromhex('030200')
    try:zlib.decompress(invalid_raw,-15);raise AssertionError('invalid distance unexpectedly accepted')
    except zlib.error:pass
    bad=bytes([31,139,8,0,0,0,0,0,0,255])+invalid_raw+struct.pack('<II',zlib.crc32(b'AAA'),3)
    no_cross=first+bad
    assert decode(no_cross,2)[2] is not None
    cross=a.output/'cross-member-history.gzip';cross.write_bytes(no_cross)
    report['files'].append({'name':'cross-member-history','path':str(cross.resolve()),'sha256':hashlib.sha256(no_cross).hexdigest(),'input_bytes':len(no_cross),'expected':-1,'first_member_bytes':len(first),'first_member_output':1,'first_member_crc':zlib.crc32(b'A'),'reason':'each raw member starts with no initialized history'})
    count=67103726;plain=(bytes(range(251))*((count+250)//251))[:count]
    stream=bytearray(bytes([31,139,8,0])+bytes(6));at=0
    for index in range(1024):
        length=65531 if index<1023 else 65513
        stream+=bytes([int(index==1023)])+struct.pack('<HH',length,length^65535)+plain[at:at+length];at+=length
    stream+=struct.pack('<II',zlib.crc32(plain),len(plain))
    assert at==count and len(stream)==67108864 and gzip.decompress(stream)==plain
    maximum=a.output/'max-input.gzip';maximum.write_bytes(stream)
    report['files'].append({'name':'max-input','path':str(maximum.resolve()),'sha256':hashlib.sha256(stream).hexdigest(),'input_bytes':67108864,'members':1,'output_bytes':count,'output_sha256':hashlib.sha256(plain).hexdigest(),'pattern':'global_index % 251','python_gzip_equal':True})
    report['large_cases']=[{'name':'65MiB-output','policy':2,'output_limit':68157440,'expected':2,'members':65,'output_bytes':68157440},
                          {'name':'65MiB-minus-one-budget','policy':2,'output_limit':68157439,'expected':-2,'members':64},
                          {'name':'wide-first-member','policy':0,'output_limit':68157440,'expected':2,'members':1,'output_bytes':1048576},
                          {'name':'wide-strict-single-tail','policy':1,'output_limit':68157440,'expected':-1,'members':1,'output_bytes':1048576},
                          {'name':'compressed-budget-minus-one','policy':2,'output_limit':68157440,'input_limit':len(data)-1,'expected':-2,'members':64},
                          {'name':'actual-compressed-input-64MiB','policy':2,'input_limit':67108864,'output_limit':67103726,'expected':2,'members':1,'output_bytes':67103726}]
    (a.output/'manifest.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'schema':1,'files':3,'large_cases':6,'wide_output_bytes':68157440}))
if __name__=='__main__':main()
