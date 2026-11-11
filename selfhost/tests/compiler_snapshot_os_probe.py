import ctypes as C
from ctypes import wintypes as W
from pathlib import Path
import argparse, hashlib, json, os, struct, subprocess, sys, time, traceback

ROOT = Path(__file__).resolve().parents[2]
parser = argparse.ArgumentParser(description='Independently exercise compiler snapshot retained-file OS ABI, including absolute seek.')
parser.add_argument('--candidate', type=Path, required=True)
parser.add_argument('--dll', type=Path, required=True)
parser.add_argument('--dll-pin', required=True)
parser.add_argument('--output', type=Path, required=True)
args = parser.parse_args()
OUT = args.output.resolve()
if not OUT.is_relative_to(ROOT / 'target'):
    raise ValueError('probe output must remain below target/')
OUT.mkdir(exist_ok=False)
FIX = OUT / 'fixtures'
FIX.mkdir()
REPORT = {'stage': 'independent-compiler-snapshot-os-oracle', 'result': 'running',
          'started_at': time.time(), 'cases': [], 'commands': [], 'inputs': {},
          'scope': 'OS adapter only; SG policy, digest, budgets and graph qualification are separate'}
def sha(p): return hashlib.sha256(Path(p).read_bytes()).hexdigest()
def save(): (OUT/'evidence.json').write_text(json.dumps(REPORT, indent=2), encoding='utf8')
def check(value, label):
    if not value: raise AssertionError(label)
def case(name, action):
    before = handles()
    detail = action()
    after = handles()
    check(before == after, (name, 'handle leak', before, after))
    REPORT['cases'].append({'name': name, 'result': 'passed', 'handles_before': before,
                            'handles_after': after, 'details': detail})
    save()
    print(name, 'passed', flush=True)

k = C.WinDLL('kernel32', use_last_error=True)
def api(name, result, args):
    fn = getattr(k, name); fn.restype=result; fn.argtypes=args; return fn
CreateFile = api('CreateFileW', W.HANDLE, [W.LPCWSTR,W.DWORD,W.DWORD,C.c_void_p,W.DWORD,W.DWORD,W.HANDLE])
CloseHandle = api('CloseHandle', W.BOOL,[W.HANDLE])
GetCurrentProcess = api('GetCurrentProcess', W.HANDLE, [])
GetProcessHandleCount = api('GetProcessHandleCount', W.BOOL,[W.HANDLE,C.POINTER(W.DWORD)])
MoveFile = api('MoveFileExW', W.BOOL,[W.LPCWSTR,W.LPCWSTR,W.DWORD])
DeleteFile = api('DeleteFileW', W.BOOL,[W.LPCWSTR])
DeviceIoControl = api('DeviceIoControl', W.BOOL,[W.HANDLE,W.DWORD,C.c_void_p,W.DWORD,C.c_void_p,W.DWORD,C.POINTER(W.DWORD),C.c_void_p])
CreateSymbolicLink = api('CreateSymbolicLinkW', C.c_ubyte,[W.LPCWSTR,W.LPCWSTR,W.DWORD])
INVALID = C.c_void_p(-1).value
def handles():
    count=W.DWORD(); check(GetProcessHandleCount(GetCurrentProcess(), C.byref(count)), 'handle count')
    return count.value
def close(h): check(CloseHandle(h), 'Win32 CloseHandle')
def winopen(path, access=0x80000000, share=7, flags=0):
    return CreateFile(str(path), access, share, None, 3, flags, None)
def denied(action, label):
    C.set_last_error(0); value=action(); error=C.get_last_error()
    check(not value and error in (5,32,33), (label, value,error)); return error
def denied_open(path, access):
    C.set_last_error(0); h=winopen(path,access); error=C.get_last_error()
    if h!=INVALID: close(h)
    check(h==INVALID and error in (5,32,33), ('denied open',str(path),error))
    return error
class Info(C.Structure):
    _fields_=[('attrs',W.DWORD),('creation',W.FILETIME),('access',W.FILETIME),('write',W.FILETIME),
              ('volume',W.DWORD),('size_high',W.DWORD),('size_low',W.DWORD),('links',W.DWORD),
              ('index_high',W.DWORD),('index_low',W.DWORD)]
GetInfo=api('GetFileInformationByHandle',W.BOOL,[W.HANDLE,C.POINTER(Info)])
def independent_metadata(path):
    h=winopen(path); check(h!=INVALID,'independent metadata open')
    try:
        i=Info(); check(GetInfo(h,C.byref(i)),'independent metadata')
        return [i.volume,i.index_high,i.index_low,i.size_high,i.size_low,i.attrs,
                i.creation.dwHighDateTime,i.creation.dwLowDateTime,i.write.dwHighDateTime,i.write.dwLowDateTime]
    finally: close(h)

try:
    candidate=json.loads(args.candidate.read_text('utf8'))
    for item in candidate['sources']:
        check(sha(item['path'])==item['sha256'], 'candidate hash')
        REPORT['inputs'][item['path']]=item['sha256']
    dll=args.dll.resolve()
    check(sha(dll)==args.dll_pin,'DLL hash')
    REPORT['invocation'] = sys.argv
    for p in [dll,Path(__file__),args.candidate,Path(sys.executable),
              Path('C:/Windows/System32/kernel32.dll'),Path('C:/Windows/System32/ntdll.dll')]:
        REPORT['inputs'][str(p)]=sha(p)
    lib=C.WinDLL(str(dll))
    f={}
    for name,n in [('init',2),('close',2),('open',4),('read',4),('metadata',4),('path',5),('full_path',4),('drive_type',2),('seek',3)]:
        fn=getattr(lib,'sengoo_compiler_snapshot_'+name); fn.restype=C.c_int64;fn.argtypes=[C.c_int64]*n;f[name]=fn
    def call(name,*args): return f[name](*args)
    def raw(value): return C.create_string_buffer(value)
    def state():
        b=(C.c_uint64*128)(); check(call('init',C.addressof(b),1024)==0,'init'); return b
    def op(b,path):
        data=raw(str(path).encode('utf8'));return call('open',C.addressof(b),1024,C.addressof(data),len(data)-1)
    def cl(b): check(call('close',C.addressof(b),1024)==0,'close');check(bytes(b)==b'\0'*1024,'cleared')
    def metadata(b):
        data=(C.c_uint64*10)();check(call('metadata',C.addressof(b),1024,C.addressof(data),80)==0,'metadata')
        return list(data)
    def path_of(b,kind=0):
        data=C.create_string_buffer(131072);n=call('path',C.addressof(b),1024,kind,C.addressof(data),len(data))
        check(n>0,('path',n));return bytes(data)[:n].decode('utf8')
    def read(b,n):
        data=C.create_string_buffer(n+8);C.memset(data,0xA5,len(data)); got=call('read',C.addressof(b),1024,C.addressof(data),n)
        check(0<=got<=n,('read',got));check(bytes(data)[got:]==b'\xa5'*(n+8-got),'read bounds');return bytes(data)[:got]
    directory=FIX/'Unicode 空间';directory.mkdir()
    source=directory/'source.sg';payload=b'A\0B'+bytes(range(256))*16;source.write_bytes(payload)
    alias=directory/'hardlink.sg';os.link(source,alias)
    # Warm the loader and Python wrappers before the first measured resource scope.
    warm=state();check(op(warm,source)==0,'warm open');metadata(warm);path_of(warm);cl(warm);handles()

    def binary():
        b=state()
        try:
            check(op(b,source)==0,'open');check(metadata(b)==independent_metadata(source),'independent metadata equality')
            check(Path(path_of(b)).resolve()==source.resolve(),'DOS path')
            check(path_of(b,1).startswith('\\\\?\\Volume{'),'GUID path')
            chunks=[]
            for n in (1,2,17,4096,17): chunks.append(read(b,n))
            while True:
                data=read(b,13)
                if not data: break
                chunks.append(data)
            check(b''.join(chunks)==payload,'complete binary');check(read(b,1)==b'','repeated EOF')
            check(call('init',C.addressof(b),1024)==-2,'live init rejection')
            check(op(b,source)==-2,'live open rejection')
        finally: cl(b)
        check(call('close',C.addressof(b),1024)==0,'repeat close')
        return {'bytes':len(payload),'identity_and_times_independently_compared':True}
    case('binary_short_read_EOF_and_metadata',binary)

    def seeking():
        b=state();addr=C.addressof(b)
        check(call('seek',0,1024,0)==-3,'null seek')
        check(call('seek',addr,1024,0)==-3,'unopened seek')
        try:
            check(op(b,source)==0,'seek open')
            check(read(b,len(payload))==payload and read(b,1)==b'','initial EOF')
            witness=metadata(b)
            for _ in range(100):
                check(call('seek',addr,1024,0)==0,'rewind')
                check(read(b,3)==b'A\0B','read after rewind')
                check(call('seek',addr,1024,-1)==-2,'negative seek')
                check(call('seek',addr,1023,0)==-3,'invalid extent preserves position')
                check(call('seek',addr+1,1024,0)==-3,'invalid alignment preserves position')
                check(read(b,4)==payload[3:7],'failed seek preserves offset')
                check(call('seek',addr,1024,len(payload)-2)==0,'tail seek')
                check(read(b,8)==payload[-2:] and read(b,1)==b'','tail EOF')
                check(call('seek',addr,1024,len(payload)+100)==0,'past EOF seek')
                check(read(b,8)==b'','past EOF read')
                check(call('seek',addr,1024,1<<40)==0 and read(b,1)==b'','large offset EOF')
                check(call('seek',addr,1024,0)==0 and read(b,3)==b'A\0B','rewind after large offset')
                check(metadata(b)==witness,'seek preserves physical identity and metadata')
            check(call('seek',addr,1023,0)==-3,'bad extent seek')
            check(call('seek',addr+1,1024,0)==-3,'bad alignment seek')
        finally: cl(b)
        check(call('seek',addr,1024,0)==-3,'closed seek')
        return {'scopes':100,'same_retained_handle':True,'negative_seek_preserves_position':True}
    case('same_handle_seek_reread_and_invalid_offsets',seeking)


    def validation():
        b=state();addr=C.addressof(b);sent=C.create_string_buffer(b'Z'*160)
        try:
            check(op(b,source)==0,'open')
            checks=[('read',(addr,1024,C.addressof(sent),0),-2),('read',(addr,1024,C.addressof(sent),2**32),-13),
                    ('metadata',(addr,1024,C.addressof(sent),79),-4),('path',(addr,1024,0,C.addressof(sent),1),-4),
                    ('path',(addr,1024,2,C.addressof(sent),160),-2),('init',(addr,1023),-2),('init',(addr+1,1024),-2)]
            for name,args,expected in checks:
                check(call(name,*args)==expected, (name,expected));check(bytes(sent)==b'Z'*160+b'\0','output unchanged')
            text=raw(str(source).encode());check(call('full_path',C.addressof(text),len(text)-1,C.addressof(sent),1)==-4,'full path short')
            check(bytes(sent)==b'Z'*160+b'\0','full path output unchanged')
            for text,expected in [(b'\xff',-20),(b'D:\\a\0b',-2)]:
                data=raw(text);check(call('full_path',C.addressof(data),len(text),C.addressof(sent),160)==expected,'UTF8/NUL marshalling')
        finally: cl(b)
        check(call('read',addr,1024,C.addressof(sent),1)==-3,'read after close')
        return {'checks':len(checks)+5,'caller_buffers_unchanged':True}
    case('unsafe_ABI_bounds_and_no_partial_outputs',validation)

    def retained():
        one=state();two=state();errors=[]
        try:
            check(op(one,source)==0 and op(two,alias)==0,'two hardlink owners')
            check(metadata(one)==metadata(two),'hardlink identity')
            for p in (source,alias):
                errors.append(denied_open(p,0x40000000));errors.append(denied(lambda:DeleteFile(str(p)),'delete'))
            for p in (directory,FIX): errors.append(denied(lambda p=p:MoveFile(str(p),str(p)+'-moved',0),'ancestor rename'))
            sibling=directory/'sibling';sibling.write_bytes(b'allowed');check(sibling.read_bytes()==b'allowed','sibling write')
            cl(one);errors.append(denied_open(source,0x40000000))
        finally: cl(one);cl(two)
        writer=winopen(source,0x40000000);check(writer!=INVALID,'writer after close');close(writer)
        moved=directory.with_name('moved');check(MoveFile(str(directory),str(moved),0),'rename after close')
        check(MoveFile(str(moved),str(directory),0),'restore after close')
        return {'denied_errors':errors,'sibling_allowed':True,'post_close_write_and_rename':True}
    case('retained_files_hardlinks_and_ancestor_guards',retained)

    def rollback():
        statuses=[]
        for access in (0x40000000,0x10000):
            h=winopen(source,access);check(h!=INVALID,'preexisting writer/delete handle')
            b=state()
            try: statuses.append(op(b,source));check(statuses[-1]==-7,'sharing failure');check(bytes(b)==b'\0'*1024,'failure cleared')
            finally: cl(b);close(h)
        for path,expected in [(directory/'missing.sg',-5),(directory,-2),(directory/'missing'/'child.sg',-5)]:
            b=state()
            try: result=op(b,path);statuses.append(result);check(result==expected,('open failure',path,result));check(bytes(b)==b'\0'*1024,'partial guards cleared')
            finally: cl(b)
        return {'statuses':statuses}
    case('sharing_missing_and_directory_failure_rollback',rollback)

    def reparse():
        junction=FIX/'junction';junction.mkdir()
        substitute=('\\??\\'+str(directory)).encode('utf-16-le');printname=str(directory).encode('utf-16-le')
        names=substitute+b'\0\0'+printname+b'\0\0'
        data=struct.pack('<IHHHHHH',0xA0000003,8+len(names),0,0,len(substitute),len(substitute)+2,len(printname))+names
        h=winopen(junction,0x40000000,7,0x02200000);check(h!=INVALID,'junction open')
        try:
            blob=raw(data);count=W.DWORD();check(DeviceIoControl(h,0x000900A4,blob,len(data),None,0,C.byref(count),None),('junction create',C.get_last_error()))
        finally: close(h)
        for p in (junction/'source.sg',junction):
            b=state()
            try: check(op(b,p)==-2,'junction rejection');check(bytes(b)==b'\0'*1024,'junction failure cleared')
            finally: cl(b)
        symlink=FIX/'symbolic.sg';created=bool(CreateSymbolicLink(str(symlink),str(source),2));error=C.get_last_error()
        if created:
            b=state()
            try: check(op(b,symlink)==-2,'file symlink rejection')
            finally: cl(b)
        return {'junction_parent_and_final_rejected':True,'file_symlink_tested':created,'symlink_create_error':None if created else error}
    case('physical_reparse_rejection',reparse)

    def depths():
        cursor=FIX/'depth';cursor.mkdir();results=[]
        # Win32 guard count includes the drive root and all containing directories.
        for _ in range(70):
            target=cursor/'leaf.sg';target.write_bytes(b'x')
            depth=len(cursor.parts) # D:\ plus each relative directory
            if depth in (63,64,65):
                b=state()
                try:
                    result=op(b,target);check(result==(0 if depth<=64 else -13),('depth',depth,result))
                    if result==0: check(b[66]==depth,'actual guard count')
                    else: check(bytes(b)==b'\0'*1024,'depth rollback')
                    results.append({'ancestor_count':depth,'status':result})
                finally: cl(b)
            cursor=cursor/'d';cursor.mkdir()
        check(len(results)==3,'all boundary depths exercised');return results
    case('actual_63_64_65_ancestor_boundary',depths)

    def many():
        owners=[];before=handles();peak=before
        try:
            for _ in range(1025):
                b=state();owners.append(b);check(op(b,source)==0,'no global C owner cap')
            peak=handles();check(peak>before+1025,'real retained OS handles')
            check(read(owners[0],3)==b'A\0B' and read(owners[-1],3)==b'A\0B','first/last owners')
        finally:
            for b in owners: cl(b)
        return {'simultaneous_OS_states':1025,'peak_handles':peak,'baseline_handles':before,'no_SG_arena_capacity_claim':True}
    case('1025_independent_C_states_without_global_registry',many)

    def repeats():
        for i in range(100):
            before=handles();owners=[]
            try:
                for _ in range(33):
                    b=state();owners.append(b);check(op(b,source)==0,'scope open')
                fail=state();check(op(fail,directory/'absent')==-5,'scope failure');cl(fail)
            finally:
                for b in reversed(owners): cl(b)
            check(handles()==before,('scope cleanup',i))
        return {'scopes':100,'live_owners_per_scope':33,'partial_failure_per_scope':1}
    case('100_mixed_success_failure_scopes',repeats)
    REPORT['input_guards']=[p for p,h in REPORT['inputs'].items() if sha(p)!=h]
    check(not REPORT['input_guards'],'input changed')
    REPORT['result']='passed'
except BaseException as error:
    REPORT['result']='failed';REPORT['error']=repr(error);REPORT['traceback']=traceback.format_exc()
    raise
finally:
    REPORT['finished_at']=time.time();save()
