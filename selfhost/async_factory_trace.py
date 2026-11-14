"""Generate test-only stage observations from the actual captured driver bodies."""
from pathlib import Path
import re

def _function(path,name):
 text=Path(path).read_text('utf8');match=re.search(r'^def '+name+r'\(',text,re.M);assert match,name
 end=re.search(r'^def ',text[match.end():],re.M)
 return text[match.start():match.end()+end.start()if end else len(text)].rstrip()+'\n'

def trace_fragment(snapshot):
 snapshot=Path(snapshot);src=snapshot/'selfhost/src'
 native=_function(src/'native_driver.sg','sh_nc_file')
 lift=_function(src/'hir.sg','sh_hir_lift_factory_modules')
 lower=_function(src/'mir_lower.sg','sh_lower_factory_modules')
 native=native.replace('def sh_nc_file(', 'def factory_trace_file(').replace('sh_lower_factory_modules(', 'factory_trace_lower(')
 lift=lift.replace('def sh_hir_lift_factory_modules(', 'def factory_trace_lift(')
 lower=lower.replace('def sh_lower_factory_modules(', 'def factory_trace_lower(').replace('sh_hir_lift_factory_modules(', 'factory_trace_lift(')
 def insert(text,needle,label):
  assert text.count(needle)==1,(needle,text)
  return text.replace(needle,'    io_stderr_write("factory-stage:'+label+'\\n");\n'+needle)
 for needle,label in [('    let graph = sh_modules_load(', 'modules'),('    let analyzed = sh_modules_check(', 'semantics'),('    let front = &analyzed.front;', 'semantics-complete'),('    let physical = sh_module_mir_sources(', 'source-mapping'),('    let object = sh_native_object(', 'object')]:native=insert(native,needle,label)
 for needle,label in [('    if not sh_factory_modules_bound(', 'factory-source-bound'),('    let ownership = sh_check_ownership(', 'ownership'),('    let absent = sh_hir_origins_capture_absent(', 'module-witness'),('    sh_hir_lift_modules(', 'hir-lift')]:lift=insert(lift,needle,label)
 for needle,label in [('    let valid = sh_hir_validate_modules(', 'hir-validate'),('    sh_lower_parts(', 'mir-lower')]:lower=insert(lower,needle,label)
 return '\n// Test-only observations; the producer transformations change names and add stderr writes.\n'+native+'\n'+lift+'\n'+lower+'''
def factory_trace_main()->i64 {
 if args_len()!=3{return 64;};
 let input=sh_nc_argument(1);let output=sh_nc_argument(2);
 if not input.is_ok() or not output.is_ok(){return 64;};
 let result=factory_trace_file(input.value.as_str(),output.value.as_str(),true);
 if not result.is_ok(){sh_nc_error(result.error,0,0,"input-output");return 66;};
 result.value;
}
'''
