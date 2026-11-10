from pdb_type_hash_oracle import validate_tpi_hashes
"""Independent PDB byte oracle for mixed debug and explicitly nondebug modules."""
import hashlib,struct
from link_debug_gate import msf_streams,u16,u32
from link_units_oracle import cstring,require,patch_msf
from verify_runtime_text import coff_image

def inspect_mixed(pdb,sdk,source_paths):
 streams=msf_streams(pdb);require(len(streams)==14,'three modules plus TPI hash stream');validate_tpi_hashes(streams);dbi=streams[3];require((u16(dbi,12),u16(dbi,16),u16(dbi,20))==(11,12,10),'dynamic global/public/record indices');size,contributions,maps,files=struct.unpack_from('<4I',dbi,24);at=64;modules=[]
 while at<64+size:
  name,next_at=cstring(dbi,at+64,64+size);obj,next_at=cstring(dbi,next_at,64+size);require(name==obj==[b'sg-main.obj',b'sg-worker.obj',b'sdk-leaf.obj'][len(modules)],'actual declared module identity');modules.append({'stream':u16(dbi,at+34),'symbols':u32(dbi,at+36),'lines':u32(dbi,at+44),'files':u16(dbi,at+48),'owner':u16(dbi,at+20)});at=(next_at+3)&~3
 require(len(modules)==3 and [(x['stream'],x['files'],x['owner'])for x in modules]==[(6,2,0),(7,2,1),(8,0,2)],'three complete COFF modules / SDK filecount zero');plain=modules[2];require(plain['lines']==0,'no SDK line bytes');module=streams[8];require(len(module)==plain['symbols']+4 and u32(module,0)==4 and u32(module,len(module)-4)==0,'nondebug stream has only signature/symbols/end');at=4;kinds=[]
 while at<plain['symbols']:
  length=u16(module,at)+2;kinds.append(u16(module,at+2));require(length>=4 and length%4==0 and at+length<=plain['symbols'],'nondebug symbol extent');at+=length
 require(kinds==[0x1101,0x113c]and at==plain['symbols'],'nondebug module has object/compile records only, no invented procedure/local/source')
 names=streams[5];strings=names[12:12+u32(names,8)];paths=[str(p).encode()for p in source_paths];require(strings==b'\0'+b'\0'.join(paths)+b'\0','only real SG physical source names');file_at=64+size+contributions+maps;table=dbi[file_at:file_at+files];require(struct.unpack_from('<8H',table)==(3,4,0,1,2,2,2,0),'DBI preserves 2/2/0 source contributions');offsets=[];n=0
 for path in paths:offsets.append(n);n+=len(path)+1
 require([u32(table,16+i*4)for i in range(4)]==[offsets[i]for i in [0,1,2,1]],'no fake SDK source association');require(table[32:32+len(strings)-1]==strings[1:],'DBI physical name buffer')
 part=dbi[64+size:64+size+contributions];owners=[u16(part,i+16)for i in range(4,len(part),28)];require(set(owners)=={0,1,2}and owners.count(2)==3,'real SDK text/data/rdata contributions')
 source_checks=[]
 for index in [0,1]:
  meta=modules[index];data=streams[meta['stream']];at=meta['symbols'];linefiles=[];checksum=None
  while at<meta['symbols']+meta['lines']:
   kind,length=struct.unpack_from('<II',data,at);body=data[at+8:at+8+length]
   if kind==0xF4:require(checksum is None and length==80,'debug module independent F4');checksum=body
   else:require(kind==0xF2 and u32(body,12)in [0,40],'physical line references');linefiles.append(u32(body,12))
   at+=8+length
  require(checksum is not None and sorted(linefiles)==[0,40],'SG source/function coverage')
  for local,globalfile in enumerate([0,1]if index==0 else[2,1]):
   row=checksum[local*40:local*40+40];require(u32(row,0)==offsets[globalfile]+1 and row[4:6]==b'\x20\x03'and row[6:38]==hashlib.sha256(source_paths[globalfile].read_bytes()).digest()and row[38:40]==bytes(2),'SG file hashes and global names');source_checks.append(globalfile)
 symbols=streams[10];at=0;pubs=[];pub_records=[];refs=[]
 while at<len(symbols):
  length=u16(symbols,at)+2;kind=u16(symbols,at+2);require(length>=4 and length%4==0 and at+length<=len(symbols),'global record extent')
  if kind==0x110E:pubs.append((cstring(symbols,at+14,at+length)[0],u32(symbols,at+4),u16(symbols,at+12),u32(symbols,at+8)));pub_records.append(at)
  else:require(kind in [0x1125,0x1127],'only checked SG procedure references');refs.append((cstring(symbols,at+14,at+length)[0],kind,u16(symbols,at+12)))
  at+=length
 image=coff_image(sdk);external={s[0].encode()for s in image['symbols']if s[4]==2};static={s[0].encode()for s in image['symbols']if s[4]==3};require({n for n,_,_,_ in pubs}==external|{b'main',b'worker'},'all actual C externs only plus SG extern publics');require(not static&{n for n,_,_,_ in pubs}and b'helper'not in {n for n,_,_,_ in pubs},'static symbols excluded from publics');require(all(flags==2 and segment==1 for name,flags,segment,offset in pubs if name in external),'actual C code address publics');require(len(refs)==4 and {owner for _,_,owner in refs}=={1,2},'no invented SDK typed procedure reference');require(sorted((owner,kind)for name,kind,owner in refs if name==b'helper')==[(1,0x1127),(2,0x1127)],'same-named SG statics remain separate')
 contributions_by_segment={u16(part,i):u32(part,i+4)for i in range(4,len(part),28)if u16(part,i+16)==2};c_values={row[0].encode():(row[1],row[2])for row in image['symbols']if row[4]==2};section_ids={name:index+1 for index,name in enumerate(image['sections'])}
 for name,flags,segment,offset in pubs:
  if name in c_values:section,value=c_values[name];require(segment in contributions_by_segment and offset==contributions_by_segment[segment]+value,'C public exact bound COFF address')
 public_stream=streams[12];hash_size=u32(public_stream,0);address_size=u32(public_stream,4);require(len(public_stream)==28+hash_size+address_size and not any(public_stream[8:28]),'public stream header/extent');hashes=public_stream[28:28+hash_size];require((u32(hashes,0),u32(hashes,4),u32(hashes,8))==(0xffffffff,0xf12f091a,len(pubs)*8),'public GSI header');records=[(u32(hashes,16+i*8)-1,u32(hashes,20+i*8))for i in range(len(pubs))];require(sorted(offset for offset,refs in records)==sorted(pub_records)and all(refs==1 for offset,refs in records),'public hash record pointers cover all actual publics');bitmap=hashes[16+len(pubs)*8:16+len(pubs)*8+516];buckets=[i for i in range(4096)if bitmap[i//8]&(1<<(i%8))];require(not any(bitmap[512:]),'public hash reserved bitmap bits');bucket_offsets=[u32(hashes,16+len(pubs)*8+516+i*4)for i in range(len(buckets))];require(len(hashes)==16+len(pubs)*8+516+len(buckets)*4,'public hash bucket extent')
 def cvhash(text):
  value=0;at=0
  while at+4<=len(text):value^=int.from_bytes(text[at:at+4],'little');at+=4
  if at+2<=len(text):value^=int.from_bytes(text[at:at+2],'little');at+=2
  if at<len(text):value^=text[at]
  value|=0x20202020;value^=value>>11;return (value^(value>>16))%4096
 assigned={offset:cvhash(name)for offset,(name,_,_,_)in zip(pub_records,pubs)};require([assigned[offset]for offset,_ in records]==sorted(assigned.values()),'public hash sorted name buckets');require(buckets==sorted(set(assigned.values()))and bucket_offsets==[next(i for i,(offset,_)in enumerate(records)if assigned[offset]==bucket)*12 for bucket in buckets],'public hash bitmap/bucket-to-record associations');address_map=[u32(public_stream,28+hash_size+i*4)for i in range(address_size//4)];require(address_size==len(pubs)*4 and address_map==[offset for offset,_ in sorted(zip(pub_records,pubs),key=lambda pair:(pair[1][2],pair[1][3]))],'public address map covers/sorts exact section offsets')
 return {'modules':modules,'sdk_c13_bytes':0,'sdk_source_files':0,'source_contributions':4,'unique_sg_physical_sources':3,'sdk_real_contributions':3,'sdk_external_publics':sorted(n.decode()for n in external),'sdk_static_symbols_in_bound_identity_only':sorted(n.decode()for n in static),'sg_same_name_static_helpers':2}

def controls(pdb,sdk,paths):
 streams=msf_streams(pdb);dbi=streams[3];at=64
 for i in range(2):_,at=cstring(dbi,at+64,64+u32(dbi,24));_,at=cstring(dbi,at,64+u32(dbi,24));at=(at+3)&~3
 symbols=streams[10];public_at=0
 while u16(symbols,public_at+2)!=0x110E or not cstring(symbols,public_at+14,public_at+u16(symbols,public_at)+2)[0].startswith(b'sengoo_'):public_at+=u16(symbols,public_at)+2
 found=[]
 for stream,offset,label in [(3,at+48,'fake-sdk-source-count'),(3,at+44,'fake-sdk-lines'),(8,6,'fake-sdk-procedure-record'),(10,public_at+4,'wrong-public-code-flags')]:
  try:inspect_mixed(patch_msf(pdb,stream,offset),sdk,paths)
  except (AssertionError,IndexError,struct.error):found.append(label)
  else:raise AssertionError('mixed oracle negative accepted: '+label)
 return found
