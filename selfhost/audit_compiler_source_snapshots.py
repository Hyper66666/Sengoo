"""Read-only integration audit; exits 2 while any required report is incomplete."""
from pathlib import Path
import argparse,datetime,hashlib,json,sys
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'selfhost'))
from compiler_inputs import compiler_closure

def sha(p):return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def need(value,message):
 if not value:raise AssertionError(message)
def path_at(name,base):
 p=Path(name);return p.resolve()if p.is_absolute()else(base/p).resolve()

def main():
 ap=argparse.ArgumentParser(description=__doc__)
 for name,default in [('formal','compiler-snapshot-formal01'),('ordinary','compiler-snapshot-ordinary-qualified2'),('provider','compiler-snapshot-provider-qualified3'),('hir','compiler-snapshot-hir-qualified1'),('flow','compiler-snapshot-flow-qualified2')]:ap.add_argument('--'+name,type=Path,default=ROOT/'target/selfhost'/default/'evidence.json')
 ap.add_argument('--output',type=Path,default=ROOT/'target/selfhost/compiler-snapshot-integration-audit.json')
 a=ap.parse_args();out=a.output.resolve();need(out.is_relative_to(ROOT/'target'),'output outside target')
 result={'result':'pending','started_at':datetime.datetime.now(datetime.timezone.utc).isoformat(),'verifier':{'path':str(Path(__file__).resolve()),'sha256':sha(__file__)},'reports':{},'checks':[],'audit_time_log_observations':[]}
 cache={};observed={};observed_configs={}
 def verify(p,h):
  p=Path(p).resolve();need(p.is_file(),'missing '+str(p));actual=sha(p);need(actual==h,'hash mismatch '+str(p));observed[str(p)]=h
 def maps(value,base):
  for n,h in value.items():need(isinstance(h,str)and len(h)==64,'invalid digest '+n);verify(path_at(n,base),h)
 def report(p,root=ROOT,expected=None):
  p=p.resolve()
  if str(p)in cache:
   data=cache[str(p)];need(expected is None or data['result']==expected,'unexpected cached result '+str(p));return data
  data=json.loads(p.read_bytes());need(str(data.get('result','')).startswith('passed'),'report not passed '+str(p));need(expected is None or data['result']==expected,'unexpected result '+str(p))
  for k,v in data.items():
   if k.startswith('changed_')or k in ['source_changes','snapshot_changes','tool_changes','checkout_changes','producer_checkout_changes','input_guards']:need(not v,'changed input '+k+' '+str(p))
  rev=data.get('source_revision',data.get('revision'));end=data.get('source_revision_end',data.get('source_revision_after',rev));need(end==rev,'revision changed '+str(p))
  stats={'path':str(p),'sha256':sha(p),'result':data['result'],'steps':len(data.get('steps',[])),'source_revision':rev}
  for k in ['sources','inputs_before','inputs_after','producer_origins','producer_snapshots','producer_source_origins','qualified_checkout_sources','inputs','fixture_sources']:
   if k in data:maps(data[k],root)
  if 'artifacts'in data:maps(data['artifacts'],p.parent)
  for name,row in data.get('tools',{}).items():verify(path_at(row.get('path',name),root),row['sha256'])
  for n,h in data.get('cargo_config_candidates',{}).items():
   q=path_at(n,root);need((sha(q)if q.is_file()else None)==h,'cargo config changed '+str(q));observed_configs[str(q)]=h
  for step in data.get('steps',[]):
   code=step.get('exit_code',step.get('exit'));expect=step.get('expected_exit_code',step.get('expected_exit',step.get('expected')))
   if expect is not None:need(code==expect,'command failed '+step['name'])
   else:need(code in (0,42),'unclassified legacy exit '+step['name'])
   if 'log'in step:verify(path_at(step['log'],root),step['log_sha256'])
   for stream in ['stdout','stderr']:
    h=step.get(stream+'_sha256')
    if h:
     candidates=[p.parent/(step['name']+'.'+stream+'.log'),p.parent/(step['name']+'.'+stream)]
     matches=[x for x in candidates if x.is_file()and sha(x)==h];need(matches,'missing log receipt '+str(p)+' '+step['name']+' '+stream);verify(matches[0],h)
    elif 'log'not in step:
     q=p.parent/(step['name']+'.'+stream+'.log');need(q.is_file(),'missing legacy log '+str(q));h=sha(q);verify(q,h)
     result['audit_time_log_observations'].append({'path':str(q),'sha256':h,'report':str(p),'step':step['name'],'scope':'Captured at audit time; original child report did not record this stream hash. Does not retroactively establish an original receipt.'})
  for key in ['child_reports','regressions']:
   for label,row in data.get(key,{}).items():
    child=Path(row.get('path',row.get('report')));verify(child,row['sha256']);child_root=(p.parent/'implementation')if key=='regressions'and data.get('stage')=='hir6-owning-origin-loan-candidates'else root
    report(child,child_root,row.get('result'))
  cache[str(p)]=data;result['reports'][str(p)]=stats;verify(p,stats['sha256']);return data
 try:
  required={n:getattr(a,n).resolve()for n in ['formal','ordinary','provider','hir','flow']}
  pending=[]
  for n,p in required.items():
   if not p.is_file():pending.append({'name':n,'path':str(p),'result':'missing'})
   else:
    state=json.loads(p.read_bytes()).get('result')
    if state=='running':pending.append({'name':n,'path':str(p),'result':state})
  if pending:result['pending']=pending;return 2
  formal=report(required['formal'],expected='passed-compiler-source-arena-boundary')
  ordinary=report(required['ordinary'],expected='passed-ordinary-lower-regressions');provider=report(required['provider'],expected='passed');hir=report(required['hir'],expected='passed');flow=report(required['flow'],expected='passed-fresh-scalar-flow-boundary')
  need(formal['m3_complete']is False and formal['m6_complete']is False,'overstated milestone')
  canonical={Path(n).resolve():h for n,h in formal['producer_origins'].items()}
  for origin,h in canonical.items():
   snap=required['formal'].parent/'implementation'/origin.relative_to(ROOT);need(formal['producer_snapshots'].get(str(snap))==h,'missing producer snapshot '+str(origin));verify(snap,h)
  core={ROOT/'selfhost/compiler_support/source_snapshot.sg',ROOT/'selfhost/compiler_support/native/source_snapshot_os.c',ROOT/'selfhost/compiler_support/native/source_snapshot_memory.c',ROOT/'selfhost/src/semantic_modules.sg',ROOT/'selfhost/src/hir_origins.sg',ROOT/'selfhost/src/semantic_origins.sg',ROOT/'selfhost/src/mir_module_sources.sg',ROOT/'selfhost/src/semantic_loan_liveness.sg'}
  all_shared=set()
  declarations={
   'ordinary':['selfhost/tests/borrow.sg','selfhost/tests/mir.sg','selfhost/tests/handle_disarm_hir.sg','selfhost/tools/native.sg'],
   'provider':['selfhost/tests/collection_provider.sg'],
   'flow':['selfhost/tests/collection_flow.sg','selfhost/tests/borrow.sg','selfhost/tests/mir.sg','selfhost/tests/handle_disarm_hir.sg','selfhost/tools/native.sg','selfhost/tools/semantic.sg','selfhost/tests/collection_cleanup.sg'],
   'hir':['selfhost/tests/hir_origins.sg','selfhost/tests/mir.sg','selfhost/tests/borrow.sg','selfhost/tests/handle_disarm_hir.sg','selfhost/tools/native.sg','selfhost/tools/semantic.sg']}
  for name,data in [('ordinary',ordinary),('provider',provider),('hir',hir),('flow',flow)]:
   origin={Path(n).resolve():h for n,h in data.get('qualified_checkout_sources',data.get('producer_origins',{})).items()};common=set(origin)&set(canonical);need(common,'empty common producer closure '+name)
   for p in common:need(origin[p]==canonical[p],'producer disagreement '+name+' '+str(p))
   # Every declared consumed file is already hash-rechecked above; list exact
   # shared coverage and extra test/feature inputs rather than call closures equal.
   shared_core=core&set(origin);all_shared|=shared_core
   expected_closure=compiler_closure(ROOT,[ROOT/n for n in declarations[name]])
   for p,h in expected_closure.items():
    need(origin.get(p)==h,'actual import closure input absent or stale '+name+' '+str(p))
    retained=required[name].parent/('retained-inputs'if name=='ordinary'else'implementation')/p.relative_to(ROOT)
    need(data['sources'].get(str(retained))==h,'actual retained closure unbound '+name+' '+str(p));verify(retained,h)
   need({ROOT/'selfhost/compiler_support/source_snapshot.sg',ROOT/'selfhost/compiler_support/native/source_snapshot_os.c'}<=shared_core,'snapshot producer absent '+name)
   result['checks'].append({'report':name,'same_bytes_shared_inputs':len(common),'snapshot_core_covered':[str(p.relative_to(ROOT))for p in sorted(shared_core)],'additional_inputs':[str(p.relative_to(ROOT))for p in sorted(set(origin)-set(canonical))]})
  conversion=provider.get('coff_timestamp_normalization');need(conversion,'reviewed provider conversion missing')
  for label in ['raw','normalized','qualified']:
   verify(conversion[label+'_object'],conversion[label+'_sha256'])
  raw=Path(conversion['raw_object']).read_bytes();normalized=Path(conversion['normalized_object']).read_bytes();qualified=Path(conversion['qualified_object']).read_bytes()
  need(normalized==qualified and raw[:4]==normalized[:4] and raw[8:]==normalized[8:] and len(raw)==len(normalized),'provider conversion exceeded timestamp')
  need(conversion['allowed_offsets']==[4,5,6,7] and conversion['non_timestamp_single_byte_rejections']==len(raw)-4 and conversion['length_rejections']==2,'provider conversion negative coverage')
  verify(conversion['parent_report'],conversion['parent_sha256'])
  need(flow['provider']['sha256']==sha(required['provider']) and Path(flow['provider']['report']).resolve()==required['provider'],'flow provider mismatch')
  need(flow['machine_source_calls']==7000 and flow['source_functions']==7 and flow['raw_objects_byte_equal'] is True,'flow execution evidence incomplete')
  need(core<=all_shared,'missing changed-consumer regression coverage')
  deriv=formal['fault_adapter_derivation'];verify(deriv['original'],deriv['original_sha256']);verify(deriv['wrapper'],deriv['wrapper_sha256']);verify(deriv['compiled'],deriv['compiled_sha256'])
  need(deriv['original_sha256']==canonical[ROOT/'selfhost/compiler_support/native/source_snapshot_os.c'],'fault original differs');need(deriv['wrapper_sha256']==deriv['compiled_sha256'],'fault wrapper differs')
  fault_root=required['formal'].parent/'fault-implementation';fault_expected=compiler_closure(ROOT,[ROOT/'selfhost/tests/compiler_source_snapshot_faults.sg'])
  for p,h in fault_expected.items():
   q=fault_root/p.relative_to(ROOT)
   if p==ROOT/'selfhost/compiler_support/native/source_snapshot_os.c':continue
   need(canonical.get(p)==h,'fault SG input outside canonical snapshot '+str(p));verify(q,h);need(formal['sources'].get(str(q))==h,'fault input unbound '+str(q))
  header_map=formal['c_headers'][deriv['compiled']];need(header_map.get(deriv['original'])==deriv['original_sha256'],'fault original absent from actual header closure')
  need(formal['fault_controls']['resource_scopes']==30,'fault scope count');need(set(formal['fault_controls']['admission_and_revalidation'])=={'early-eof','oversized-read-count','changed-second-metadata','fragmented-read-positive','missing-final-eof'},'fault cases missing')
  steps={s['name']:s for s in formal['steps']}
  for name in ['captured-os-oracle','compiler_source_snapshot_integrity-three-first-scopes','compiler_source_snapshot_faults-30-resource-scopes','whole-compiler-self-input']:need(name in steps,'required step absent '+name)
  osrow=formal['independent_os_oracle'];verify(osrow['path'],osrow['sha256']);osdata=report(Path(osrow['path']));need(len(osdata['cases'])>=9 and all(c['result']=='passed'and c['handles_before']==c['handles_after']for c in osdata['cases']),'OS cases/handles')
  need(formal['self_input_frontier']['phase']=='semantics'and formal['self_input_frontier']['ok']is False,'self-input result incorrect')
  # Recheck after audit so its own multi-report window cannot bless moved bytes.
  for p,h in observed.items():need(sha(p)==h,'changed during audit '+p)
  for p,h in observed_configs.items():need((sha(p)if Path(p).is_file()else None)==h,'cargo config changed during audit '+p)
  result.update(result='passed-snapshot-integration-audit',verified_files=len(observed),verified_config_candidates=len(observed_configs),scope='Same captured snapshot implementation, retained provenance, OS/fault/integrity checks and current ordinary/provider/HIR regressions; does not complete M3/M6/M8')
  return 0
 except BaseException as e:result.update(result='failed',error=repr(e));raise
 finally:
  result['completed_at']=datetime.datetime.now(datetime.timezone.utc).isoformat();out.parent.mkdir(parents=True,exist_ok=True);out.write_text(json.dumps(result,indent=2)+'\n');print(result['result'],str(out),flush=True)
if __name__=='__main__':raise SystemExit(main())
