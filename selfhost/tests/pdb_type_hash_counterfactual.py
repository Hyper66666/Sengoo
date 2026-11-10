"""Fresh-process DbgHelp causal control: change only TPI hash advertisement, not types or GUID."""
from __future__ import annotations
import argparse,platform,datetime as dt,hashlib,json,struct,subprocess,sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
sys.path.insert(0,str(ROOT/'selfhost'))
from link_debug_gate import msf_streams

def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def logical_patch(raw,stream,at,data):
 result=bytearray(raw);size=struct.unpack_from('<I',raw,32)[0];directory_size=struct.unpack_from('<I',raw,44)[0];blockmap=struct.unpack_from('<I',raw,52)[0];pages=(directory_size+size-1)//size;ids=struct.unpack_from('<'+'I'*pages,raw,blockmap*size);directory=b''.join(raw[i*size:(i+1)*size]for i in ids)[:directory_size];count=struct.unpack_from('<I',directory)[0];sizes=struct.unpack_from('<'+'I'*count,directory,4);pointer=4+4*count+4*sum((s+size-1)//size for s in sizes[:stream]);assert at+len(data)<=sizes[stream]
 for i,value in enumerate(data):
  block=struct.unpack_from('<I',directory,pointer+4*((at+i)//size))[0];result[block*size+(at+i)%size]=value
 return bytes(result)
def main():
 p=argparse.ArgumentParser(description=__doc__);p.add_argument('--producer-evidence',type=Path,required=True);p.add_argument('--output',type=Path,required=True);a=p.parse_args();out=a.output.resolve();assert out.is_relative_to(ROOT/'target');out.mkdir(parents=True,exist_ok=True);inputs={};report={'result':'running','stage':'pdb-type-hash-causal-control','m5_complete':False,'started_at':dt.datetime.now(dt.timezone.utc).isoformat(),'steps':[],'host':platform.platform(),'python':sys.version,'source_revision':subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()}
 def save():(out/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf8')
 save()
 try:
  source=a.producer_evidence.resolve();raw=source.read_bytes();inputs[str(source)]=hashlib.sha256(raw).hexdigest();qualified=json.loads(raw);assert qualified['result']=='passed-pdb-type-hash-index'and not any(qualified[k]for k in ['source_changes','snapshot_changes','tool_changes','cargo_config_changes'])
  for key in ['inputs_before','producer_snapshots']:
   for path,digest in qualified[key].items():assert sha(path)==digest;inputs[path]=digest
  for item in qualified['regressions'].values():
   child=Path(item['report']);assert sha(child)==item['sha256'];inputs[str(child)]=sha(child)
  mixed=Path(qualified['regressions']['mixed']['report']).parent;image=mixed/'fresh-a/program.exe';pdb=mixed/'fixed.pdb';original_image=image.read_bytes();original_pdb=pdb.read_bytes();inputs[str(image)]=sha(image);inputs[str(pdb)]=sha(pdb);inputs[str(Path(__file__).resolve())]=sha(__file__);streams=msf_streams(original_pdb);nohash=logical_patch(original_pdb,2,20,b'\xff\xff');nohash=logical_patch(nohash,2,32,b'\0'*24);changed=msf_streams(nohash)
  assert changed[2][56:]==streams[2][56:]and changed[1]==streams[1]and all(changed[i]==streams[i]for i in range(len(streams))if i!=2)
  results={}
  for name,stem,data in [('indexed','p',original_pdb),('nohash','n',nohash)]:
   ppath=out/(stem+'.p');ppath.write_bytes(data);pe=bytearray(original_image);start=pe.index(b'RSDS')+24;end=pe.index(0,start);new=ppath.as_posix().encode();assert len(new)<=end-start;pe[start:end+1]=new+b'\0'*(end-start+1-len(new));exe=out/(stem+'.exe');exe.write_bytes(pe);cmd=[sys.executable,ROOT/'selfhost/verify_pdb_type_hashes.py','--probe-image',exe,'--probe-pdb',ppath];done=subprocess.run(list(map(str,cmd)),cwd=ROOT,capture_output=True,timeout=60);(out/(name+'.stdout.log')).write_bytes(done.stdout);(out/(name+'.stderr.log')).write_bytes(done.stderr);assert done.returncode==0,done.stderr;result=json.loads(done.stdout);results[name]=result;report['steps'].append({'name':name,'command':list(map(str,cmd)),'exit_code':done.returncode,'stdout_sha256':sha(out/(name+'.stdout.log')),'stderr_sha256':sha(out/(name+'.stderr.log'))})
   assert result['dbghelp_sha256']==qualified['tools']['dbghelp']['sha256']
  dbi=streams[3];symbol_stream=streams[struct.unpack_from('<H',dbi,20)[0]];expected_rvas={};at=0
  while at<len(symbol_stream):
   size=struct.unpack_from('<H',symbol_stream,at)[0]+2
   if struct.unpack_from('<H',symbol_stream,at+2)[0]==0x110e:
    segment=struct.unpack_from('<H',symbol_stream,at+12)[0];offset=struct.unpack_from('<I',symbol_stream,at+8)[0];name=symbol_stream[at+14:symbol_stream.index(0,at+14,at+size)].decode('utf8');expected_rvas[name]=struct.unpack_from('<I',original_image,328+(segment-1)*40+12)[0]+offset
   at+=size
  good=results['indexed'];bad=results['nohash'];assert good['base']and all(s['ok']and s['rva']==expected_rvas[s['name']]for s in good['symbols']);report['expected_bound_public_rvas']=expected_rvas;m=good['module'];assert m['path_matches']and m['identity_matches']and not m['unmatched']and not m['dbg_unmatched'];assert not bad['base']and all(not s['ok']for s in bad['symbols']);report.update(result='passed-pdb-type-hash-causal-control',same_original_type_records_and_ids=True,same_guid_and_age=True,other_pdb_streams_identical=True,pe_changes_only_rsds_path=True,actual_results=results)
 except BaseException as error:report.update(result='failed',error=repr(error))
 finally:
  report['source_revision_after']=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip();report['inputs']=inputs;report['changed_inputs']=[p for p,h in inputs.items()if sha(p)!=h]
  if report['changed_inputs']:report['result']='failed'
  report['finished_at']=dt.datetime.now(dt.timezone.utc).isoformat();save()
 print(report['result']);return 0 if report['result']=='passed-pdb-type-hash-causal-control'else 1
if __name__=='__main__':raise SystemExit(main())
