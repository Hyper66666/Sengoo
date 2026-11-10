"""Independent COFF and PDB module-space checks for bounded SG debug linking."""
from __future__ import annotations
from pdb_type_hash_oracle import validate_tpi_hashes
import hashlib
import struct
from link_debug_gate import u16,u32,msf_streams
from link_multi_debug_gate import coff_fragments

def require(value,message):
    if not value: raise AssertionError(message)

def cstring(raw,at,end=None):
    if end is None:end=len(raw)
    require(0<=at<end<=len(raw),'string bounds');stop=raw.find(b'\0',at,end);require(stop>=at,'unterminated string');return raw[at:stop],stop+1

def type_records(raw,header):
    at=header;result=[]
    while at<len(raw):
        length=u16(raw,at);require(length>=2 and (length+2)%4==0 and at+length+2<=len(raw),'type record extent');result.append(raw[at:at+length+2]);at+=length+2
    require(at==len(raw),'type record tail');return result

def inspect_units(main_obj,worker_obj,pdb,source_paths,object_paths):
    objects=[main_obj,worker_obj];types=[];coff_helpers=[]
    for raw in objects:
        sections={raw[20+i*40:28+i*40].rstrip(b'\0'):(u32(raw,20+i*40+20),u32(raw,20+i*40+16)) for i in range(u16(raw,2))}
        at,size=sections[b'.debug$T'];require(u32(raw,at)==4,'COFF type signature');types.append(type_records(raw[at:at+size],4))
        table=u32(raw,8);count=u32(raw,12);strings=table+count*18;helpers=[]
        for i in range(count):
            row=table+i*18;name=raw[row:row+8]
            if name[:4]==b'\0'*4:name=cstring(raw,strings+u32(name,4))[0]
            else:name=name.rstrip(b'\0')
            if name==b'helper':helpers.append((u32(raw,row+8),u16(raw,row+12),raw[row+16]))
        require(len(helpers)==1 and helpers[0][2]==3,'each object owns one static helper');coff_helpers.append(helpers[0])
    require(len(types[0])>2 and len(types[1])>2 and types[0][2]!=types[1][2],'local type 4098 must differ across object namespaces')
    streams=msf_streams(pdb);require(len(streams)==13,'N+11 MSF streams');validate_tpi_hashes(streams);dbi=streams[3];require(u16(dbi,12)==10 and u16(dbi,16)==11 and u16(dbi,20)==9,'dynamic global/public/symbol stream indices')
    require(u32(streams[2],12)-u32(streams[2],8)==len(type_records(streams[2],56)),'global TPI extent')
    module_size,contrib_size,map_size,file_size=struct.unpack_from('<4I',dbi,24);at=64;modules=[]
    while at<64+module_size:
        name,next_at=cstring(dbi,at+64,64+module_size);obj,next_at=cstring(dbi,next_at,64+module_size);require(name==obj==str(object_paths[len(modules)]).encode(),'independent object module identity');modules.append({'stream':u16(dbi,at+34),'symbols':u32(dbi,at+36),'lines':u32(dbi,at+44),'files':u16(dbi,at+48),'contribution_module':u16(dbi,at+20)});at=(next_at+3)&~3
    require(at==64+module_size and len(modules)==2,'DBI exactly two modules');require([(x['stream'],x['files'],x['contribution_module']) for x in modules]==[(6,2,0),(7,2,1)],'separate streams/file counts/contribution owner')
    names=streams[5];require(u32(names,0)==0xEFFEEFFE and u32(names,4)==1,'/names identity');strings=names[12:12+u32(names,8)];expected=[str(p).encode() for p in source_paths];require(strings==b'\0'+b'\0'.join(expected)+b'\0','deduplicated global physical paths')
    name_offsets=[];offset=1
    for value in expected:name_offsets.append(offset);offset+=len(value)+1
    file_at=64+module_size+contrib_size+map_size;files=dbi[file_at:file_at+file_size];require(struct.unpack_from('<6H',files)==(2,4,0,1,2,2),'DBI module/file association arrays');require([u32(files,12+i*4) for i in range(4)]==[name_offsets[i]-1 for i in [0,1,2,1]],'shared file retained as contribution to both modules');require(files[28:28+len(strings)-1]==strings[1:],'DBI unique physical path buffer')
    optional=64+module_size+contrib_size+map_size+file_size+u32(dbi,40)+u32(dbi,52);require(u16(dbi,optional+5*2)==8,'dynamic section-header stream index')
    contributions=dbi[64+module_size:64+module_size+contrib_size];require(u32(contributions,0)==0xF12EBA2D and (len(contributions)-4)%28==0,'contribution format');owners=[u16(contributions,i+16) for i in range(4,len(contributions),28)];require(set(owners)=={0,1},'contributions from both modules')
    all_functions=[];module_helpers=[]
    for number,meta in enumerate(modules):
        data=streams[meta['stream']];require(u32(data,0)==4 and len(data)==meta['symbols']+meta['lines']+4 and u32(data,len(data)-4)==0,'module stream partition')
        at=4;functions=[];active=None
        while at<meta['symbols']:
            size=u16(data,at)+2;kind=u16(data,at+2);require(size>=4 and size%4==0 and at+size<=meta['symbols'],'module symbol extent')
            if kind in (0x110F,0x1110):
                name=cstring(data,at+39,at+size)[0];fn={'module':number,'name':name,'kind':kind,'record':at,'offset':u32(data,at+32),'segment':u16(data,at+36),'type':u32(data,at+28)};functions.append(fn);active=fn
            if kind==0x113E:
                require(active is not None,'local outside function');local=cstring(data,at+10,at+size)[0]
                if local==b'state':active['state_type']=u32(data,at+4)
            if kind==6:active=None
            at+=size
        require(at==meta['symbols'] and len(functions)==2,'two actual procedures per module');helpers=[x for x in functions if x['name']==b'helper'];require(len(helpers)==1 and helpers[0]['kind']==0x110F,'static local procedure kind');module_helpers+=helpers
        root=b'main' if number==0 else b'worker';roots=[x for x in functions if x['name']==root];require(len(roots)==1 and roots[0]['kind']==0x1110 and 'state_type'in roots[0],'global root with aggregate local');all_functions+=functions
        at=meta['symbols'];checksums=None;blocks=[]
        while at<meta['symbols']+meta['lines']:
            kind,length=struct.unpack_from('<II',data,at);payload=data[at+8:at+8+length]
            if kind==0xF4:require(checksums is None and length==80,'one local F4 table with two files');checksums=payload
            else:
                require(kind==0xF2 and length>=32,'C13 line fragment');file_ref=u32(payload,12);require(file_ref in (0,40),'local checksum byte offset');blocks.append(file_ref)
            at+=8+length
        require(checksums is not None and sorted(blocks)==[0,40],'both local physical files have function lines')
        for local,global_file in enumerate([0,1] if number==0 else [2,1]):
            row=checksums[local*40:local*40+40];require(u32(row,0)==name_offsets[global_file] and row[4:6]==b'\x20\x03' and row[6:38]==hashlib.sha256(source_paths[global_file].read_bytes()).digest() and row[38:]==b'\0\0','module F4 links global /names and physical SHA')
    roots=[x for x in all_functions if x['name']in (b'main',b'worker')];require(roots[0]['state_type']!=roots[1]['state_type'],'distinct module-local structures retain different global TPI IDs')
    tpi=type_records(streams[2],56)
    for fn,field_names in zip(roots,[(b'input',b'extra'),(b'count',b'step')]):
        record=tpi[fn['state_type']-4096];require(u16(record,2)==0x1505 and u16(record,4)==2 and u16(record,6)==0 and u16(record,20)==16,'global aggregate actual size/count');require(cstring(record,22)[0]==b'Local','global aggregate nominal name')
        fields=tpi[u32(record,8)-4096];require(u16(fields,2)==0x1203,'global aggregate field-list type');cursor=4
        for ordinal,name in enumerate(field_names):
            require(u16(fields,cursor)==0x150D and u16(fields,cursor+2)==3 and u32(fields,cursor+4)==19 and u16(fields,cursor+8)==ordinal*8,'actual i64 member type/offset');observed,cursor=cstring(fields,cursor+10);require(observed==name,'independently mapped member names')
            while cursor<len(fields)and fields[cursor]>=0xF0:cursor+=1
        require(cursor==len(fields),'complete aggregate field list')

    globals_data=streams[9];at=0;refs=[];publics=[]
    while at<len(globals_data):
        size=u16(globals_data,at)+2;kind=u16(globals_data,at+2)
        if kind==0x110E:publics.append(cstring(globals_data,at+14,at+size)[0])
        elif kind in (0x1125,0x1127):refs.append((cstring(globals_data,at+14,at+size)[0],kind,u16(globals_data,at+12),u32(globals_data,at+8)))
        else:raise AssertionError('unknown consolidated global record')
        at+=size
    require(sorted(publics)==[b'main',b'worker'],'static helpers never become publics');require(sorted((m,o)for n,k,m,o in refs if n==b'helper' and k==0x1127)==[(x['module']+1,x['record'])for x in module_helpers],'two same-name local procedure references keep module/symbol-offset identity')
    require(len(refs)==4,'all module procedure references');return {'modules':modules,'source_file_contributions':4,'unique_physical_files':3,'static_helpers':2,'publics':['main','worker'],'local_4098_conflicts':True,'local_4098_leaf_kinds':[u16(value[2],2)for value in types],'root_state_global_type_ids':[x['state_type']for x in roots]}

def patch_msf(pdb,stream,offset):
    raw=bytearray(pdb);size=u32(raw,32);directory_size=u32(raw,44);block_map=u32(raw,52);pages=(directory_size+size-1)//size;indices=[u32(raw,block_map*size+n*4)for n in range(pages)];directory=b''.join(raw[n*size:(n+1)*size]for n in indices)[:directory_size];count=u32(directory,0);sizes=[u32(directory,4+n*4)for n in range(count)];require(stream<count and offset<sizes[stream],'mutation logical stream bounds');at=4+4*count
    for n in range(stream):at+=4*((sizes[n]+size-1)//size)
    page=u32(directory,at+4*(offset//size));raw[page*size+offset%size]^=0x80;return bytes(raw)

def oracle_controls(main_obj,worker_obj,pdb,paths,objects):
    streams=msf_streams(pdb);dbi=streams[3];first_name,end=cstring(dbi,128,64+u32(dbi,24));_,end=cstring(dbi,end,64+u32(dbi,24));second=(end+3)&~3
    module=streams[7];at=u32(dbi,second+36);checksum=None
    while at<u32(dbi,second+36)+u32(dbi,second+44):
        kind,length=struct.unpack_from('<II',module,at)
        if kind==0xF4:checksum=at+8;break
        at+=length+8
    require(checksum is not None,'control source checksum')
    tpi=type_records(streams[2],56);aggregate=next(record for record in tpi if u16(record,2)==0x1505);field=u32(aggregate,8)-4096;type_control=56+sum(map(len,tpi[:field]))+8
    controls=[]
    for stream,offset,label in [(3,second+34,'duplicate-module-stream'),(7,checksum,'module-global-path-offset'),(9,12,'procedure-reference-module'),(2,type_control,'global-member-type')]:
        try:inspect_units(main_obj,worker_obj,patch_msf(pdb,stream,offset),paths,objects)
        except (AssertionError,IndexError,struct.error):controls.append(label)
        else:raise AssertionError('oracle negative control accepted: '+label)
    return controls
