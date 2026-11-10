"""Shared qualification helpers for raw/gzip/continuation memory-only deployment."""
from pathlib import Path
import ctypes,re
from verify_runtime_text import strict_json,digest

def c_link(run,read,report,name,clang,objects,output,environment):
    if 'actual_clang_linker'not in report:
        driver=run(name+'-driver',[clang,'-###','-fuse-ld=lld',*objects,'-o',output],environment)
        selected=re.findall(r'^ "([^"]*lld-link(?:\.exe)?)"',driver.stderr.decode(),re.M)
        if len(selected)!=1 or Path(selected[0].replace('\\\\','\\')).with_suffix('.exe').resolve()!=Path(report['tools']['lld']['path']).resolve():raise ValueError('actual clang linker differs from qualified binary')
        report['actual_clang_linker']=report['tools']['lld']
    result=run(name,[clang,'-fuse-ld=lld',*objects,'-Wl,/verbose','-o',output],environment)
    known={};libraries={}
    for line in(result.stdout+result.stderr).decode().splitlines():
        match=re.search(r'Reading (.+?\.lib)(?:\(|$)',line,re.I)
        if not match:continue
        path=Path(match.group(1))
        if path.is_absolute():path=path.resolve();known.setdefault(path.name.casefold(),set()).add(path)
        else:
            choices=known.get(path.name.casefold(),set())
            if len(choices)!=1:raise ValueError('C library member has no unique physical archive '+str(path))
            path=next(iter(choices))
        if str(path)not in libraries:libraries[str(path)]=digest(read(path))
    if not libraries:raise ValueError('actual C link trace has no archive inputs')
    report.setdefault('actual_c_link_libraries',{})[name]=libraries
    return result

def resource_receipt(raw,allocations,oom=False):
    value=strict_json(raw)
    expected={'schema':1,'live_bytes':0,'created':allocations,'freed':allocations,'peak_bytes':45056 if allocations else 0,'attempts':1 if oom else allocations,'disposed_zero':True,'virtual_release_verified':True}
    if set(value)!=set(expected)or any(type(value[k])is not type(v)or value[k]!=v for k,v in expected.items()):raise ValueError('actual state-allocation/release receipt differs')
    return value

def bind_memory_providers(read):
    kernel=ctypes.WinDLL('kernel32',use_last_error=True)
    module=kernel.GetModuleHandleExW;module.argtypes=[ctypes.c_uint32,ctypes.c_void_p,ctypes.POINTER(ctypes.c_void_p)];module.restype=ctypes.c_int
    filename=kernel.GetModuleFileNameW;filename.argtypes=[ctypes.c_void_p,ctypes.c_wchar_p,ctypes.c_uint32];filename.restype=ctypes.c_uint32
    rows=[]
    for symbol in ['VirtualAlloc','VirtualFree','VirtualQuery']:
        address=ctypes.cast(getattr(kernel,symbol),ctypes.c_void_p).value;handle=ctypes.c_void_p();name=ctypes.create_unicode_buffer(32768)
        if not module(6,address,ctypes.byref(handle))or not filename(handle,name,len(name)):raise ValueError('actual OS memory provider unavailable')
        path=Path(name.value).resolve();rows.append({'symbol':symbol,'path':str(path),'sha256':digest(read(path)),'rva':address-handle.value})
    return rows
