"""Recheck source-bound raw-next machine cleanup and its exact old-MIR leak control.

The retained private lower is a test harness, not a promoted public source API.
"""
from __future__ import annotations
import argparse,datetime,hashlib,json,os,platform,re,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
def sha(path):return hashlib.sha256(Path(path).read_bytes()).hexdigest()
def revision():return subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
def dependencies(path):
    raw=path.read_text('utf8').replace('\\\n',' ');assert raw.startswith('optional_dependency:')
    payload=raw.split(':',1)[1];words=[];word=[];at=0
    while at<len(payload):
        c=payload[at]
        if c=='\\' and at+1<len(payload) and payload[at+1] in ' \t#':word.append(payload[at+1]);at+=2
        elif c=='$' and at+1<len(payload) and payload[at+1]=='$':word.append('$');at+=2
        elif c.isspace():
            if word:words.append(''.join(word));word=[]
            at+=1
        else:word.append(c);at+=1
    if word:words.append(''.join(word))
    paths={Path(w).resolve()for w in words};assert paths and all(p.is_file()for p in paths)
    return paths
def main():
 parser=argparse.ArgumentParser(description=__doc__);parser.add_argument('--producer',required=True,type=Path);parser.add_argument('--producer-sha256',required=True);parser.add_argument('--semantic',required=True,type=Path);parser.add_argument('--semantic-sha256',required=True);parser.add_argument('--output',required=True,type=Path);args=parser.parse_args();base=args.output.resolve();assert base.is_relative_to(ROOT/'target');base.mkdir(parents=True,exist_ok=False)
 inputs={str(Path(__file__).resolve()):sha(__file__)};outputs={};tools={};configs={};report={'schema':1,'stage':'direct-raw-next-machine-resources','result':'running','host':platform.platform(),'source_revision':revision(),'started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'sources':inputs,'artifacts':outputs,'tools':tools,'steps':[],'public_source_api_qualified':False,'native_reference_authority':False,'limits':['private fully source-bound lower/test harness is not a public source authority entry','constructor/factory/cleanup test hooks retain actual source/provider/job checks but complete generic/nativeSDK integration remains open','HIR6 complete new NLL/handles/effects/authority wire is not implemented','normal source/runtime C/Rust dependencies and recorded native development tools remain; M6/M8/compiler selfbuild are not qualified']}
 def save():(base/'evidence.json').write_text(json.dumps(report,indent=2)+'\n',encoding='utf8')
 def bind(path,digest):
  path=Path(path).resolve();raw=path.read_bytes();assert hashlib.sha256(raw).hexdigest()==digest,(path,digest);inputs[str(path)]=digest;return raw
 def copied(path,digest,name):
  raw=bind(path,digest);destination=base/name;destination.parent.mkdir(parents=True,exist_ok=True);destination.write_bytes(raw);assert sha(destination)==digest;inputs[str(destination)]=digest;return destination
 def manifest(path,digest,expected_result):
  path=Path(path).resolve();raw=bind(path,digest);data=json.loads(raw);assert data['result']==expected_result
  for key in ['changed_sources','changed_tools','checkout_changes','changed_configs','changed_artifacts']:assert data.get(key)==[],(key,data.get(key))
  for p,h in data['sources'].items():bind(p,h)
  # Original sources are historical provenance, bound to their actually consumed
  # immutable counterpart. Mutable checkout bytes are not silently refreshed.
  snapshot=path.parent/'implementation'
  for p,h in data['qualified_checkout_sources'].items():
   original=Path(p);saved=snapshot/original.relative_to(ROOT);assert data['sources'].get(str(saved))==h;(bind(saved,h))
  for name,item in data['tools'].items():bind(item['path'],item['sha256'])
  for name,h in data['artifacts'].items():bind(path.parent/name,h)
  for step in data['steps']:
   assert step['exit_code']==step['expected_exit_code']
   for channel in ['stdout','stderr']:bind(path.parent/(step['name']+'.'+channel+'.log'),step[channel+'_sha256'])
  for p,h in data['cargo_config_candidates'].items():
   actual=sha(p)if Path(p).is_file()else None;assert actual==h;configs[p]=h
  return data
 try:
  producer_path=args.producer.resolve();producer=manifest(producer_path,args.producer_sha256,'passed-private-development');semantic=manifest(args.semantic.resolve(),args.semantic_sha256,'passed');assert not producer['formal_qualification'] and semantic['counts']['iterator_reuse_observations']==2
  report['producer']={'report':str(producer_path),'sha256':args.producer_sha256,'formal_qualification':False,'scope':'fully source-bound private test harness artifact'};report['semantic']={'report':str(args.semantic.resolve()),'sha256':args.semantic_sha256}
  for name in ['semantic_collection_raw_next_observation','semantic_collection_effect_ownership','semantic_collection_effects','borrow']:
   path=ROOT/'selfhost/src'/(name+'.sg');digest=sha(path);assert producer['qualified_checkout_sources'][str(path)]==semantic['qualified_checkout_sources'][str(path)]==digest;bind(path,digest)
  # Both parents must bind the same genuinely qualified historical SDK branch.
  q=producer['qualified_provider'];assert q['report_sha256']==semantic['qualified_provider']['report_sha256']=='a63152e01208dff39e1d5cbcbd326187901808c8f76fa0d3075a08e0a9fa1cd3';bind(q['report'],q['report_sha256']);qbase=Path(q['report']).parent;qs=json.loads(Path(q['report']).read_bytes());assert qs['result']=='passed'
  for p,h in q['sources'].items():bind(p,h)
  for name,item in q['tools'].items():bind(item['path'],item['sha256'])
  for name,h in q['artifacts'].items():bind(qbase/name,h)
  report['historical_provider']=q
  env={k:v for k,v in os.environ.items()if not k.upper().startswith(('SENGOO_','CL','_CL','LINK','_LINK','LLDB_'))};clang=Path(producer['tools']['clang']['path']);env['PATH']=str(clang.parent)+os.pathsep+env.get('PATH','');pure=dict(env);pure['PATH']='';tools.update({'clang':producer['tools']['clang'],'lld-link':{'path':str(clang.parent/'lld-link.exe'),'sha256':sha(clang.parent/'lld-link.exe')}})
  def run(name,command,expected=0,empty=False):
   done=subprocess.run(list(map(str,command)),cwd=ROOT,env=pure if empty else env,capture_output=True,timeout=900);stdout=base/(name+'.stdout.log');stderr=base/(name+'.stderr.log');stdout.write_bytes(done.stdout);stderr.write_bytes(done.stderr);report['steps'].append({'name':name,'command':list(map(str,command)),'exit_code':done.returncode,'expected_exit_code':expected,'empty_path':empty,'stdout_sha256':sha(stdout),'stderr_sha256':sha(stderr)});save();print(name,done.returncode,flush=True);assert done.returncode==expected,(done.stdout[-500:],done.stderr[-2000:]);return done
  run('clang-version',[clang,'--version'])
  generator=copied(producer_path.parent/'flow.exe',producer['artifacts']['flow.exe'],'flow.exe');source_original=producer_path.parent/'source 世界 &/source.sg';source=copied(source_original,producer['sources'][str(source_original)],'physical source 世界 &/source.sg');provider=qbase/'implementation';col=provider/'tools/stdlib/collections.sg';option=provider/'tools/stdlib/option.sg';bind(col,sha(col));bind(option,sha(option))
  generated=base/'source.obj';run('fresh-source-from-independent-pins',[generator,provider,q['manifest_sha256'],q['object_sha256'],source,generated,sha(col),sha(option)],42,True);outputs[generated.name]=sha(generated);assert sha(generated)==producer['artifacts']['source.obj'];report['same_source_object_sha256']=sha(generated)
  # The old MIR control is an immutable causal input, never a source authority.
  control_dir=ROOT/'target/selfhost/m3-source-flow-next';control=control_dir/'edge-resource-control.json';raw=bind(control,'0d17e34e17644695f6e2b28cd2aba21769ffcf34bf23caeed68d32935dda23dc');c=json.loads(raw);assert c['formal']==False and c['steps'][-1]['exit']==81
  for step in c['steps']:
   assert step['exit']==step['expected']
   for channel in ['stdout','stderr']:bind(control_dir/(step['name']+'.'+channel),step[channel+'_sha256'])
  freeze_path=control_dir/'private-freeze.json';freeze=json.loads(bind(freeze_path,'2775b243e07782bf13f36dc852ab5732caf4e1533dd7a1fdd1aba7e4883afcae'))
  for relative,h in freeze['control_retained_artifacts'].items():bind(control_dir/relative,h)
  old_obj=copied(control_dir/'edge-only.obj',freeze['control_retained_artifacts']['edge-only.obj'],'old-edge-only.obj')
  probe_original=ROOT/'selfhost/tests/collection_raw_next_observation_probe.c';probe=copied(probe_original,sha(probe_original),'probe.c');report['qualified_checkout_sources']={str(probe_original):sha(probe_original)};headers={};libs={}
  for name,item in tools.items():bind(item['path'],item['sha256'])
  providers=[provider/'target/provider'/(n+'.obj')for n in ['runtime','runtime_string','runtime_collections']]
  for label,obj,defines,code in [('old-projection',old_obj,['-DOBSERVATION_OLD_MIR'],81),('new-observation',generated,[],42)]:
   flags=['-std=c11','-Wall','-Wextra','-Werror','-O0',*defines,'-I',provider/'tools/stdlib'];before=base/(label+'-before.d');run(label+'-headers',[clang,*flags,'-M','-MT','optional_dependency','-MF',before,probe]);captured={str(p):sha(p)for p in dependencies(before)}
   for p,h in captured.items():bind(p,h)
   after=base/(label+'-after.d');cobj=base/(label+'.obj');run(label+'-c-build',[clang,*flags,'-MD','-MT','optional_dependency','-MF',after,'-c',probe,'-o',cobj]);assert captured=={str(p):sha(p)for p in dependencies(after)};headers[label]=captured;outputs[cobj.name]=sha(cobj)
   exe=base/(label+'.exe');linked=run(label+'-link',[clang,'-fuse-ld=lld','-Wl,/verbose',cobj,obj,*providers,'-o',exe]);selected={}
   for match in re.finditer(rb'Reading ([^\r\n]+?\.lib)(?:\(|\r?$)',linked.stdout+linked.stderr,re.M):
    path=Path(match[1].decode('utf8')).resolve()
    if path.is_file():selected[str(path)]=sha(path)
   assert selected
   for p,h in selected.items():bind(p,h)
   libs[label]=selected;outputs[exe.name]=sha(exe);observed=run(label+'-actual-resources',[exe],code,True);assert not observed.stdout and not observed.stderr
  report.update(result='passed-private-harness-machine-resources',c_header_closures=headers,c_link_libraries=libs,cargo_config_candidates=configs,actual_resource_scopes=7000,old_projection_expected_exit=81,new_observation_expected_exit=42)
 except BaseException as error:report.update(result='failed',error=repr(error));raise
 finally:
  report.update(changed_sources=[p for p,h in inputs.items()if not Path(p).is_file()or sha(p)!=h],changed_artifacts=[p for p,h in outputs.items()if not(base/p).is_file()or sha(base/p)!=h],changed_tools=[n for n,item in tools.items()if sha(item['path'])!=item['sha256']],changed_configs=[p for p,h in configs.items()if(sha(p)if Path(p).is_file()else None)!=h],source_revision_end=revision(),completed_at=datetime.datetime.now(datetime.timezone.utc).isoformat())
  if any(report[k]for k in ['changed_sources','changed_artifacts','changed_tools','changed_configs']):report['result']='failed'
  save()
 assert report['result']=='passed-private-harness-machine-resources';print(sha(base/'evidence.json'))
if __name__=='__main__':main()
