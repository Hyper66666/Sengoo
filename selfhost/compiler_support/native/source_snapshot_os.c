#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stddef.h>
#include <limits.h>
#include <wchar.h>

/* Mechanical scratch allocation and byte movement; no source runtime policy. */
static void *ss_allocate(size_t length){return HeapAlloc(GetProcessHeap(),0,length);}
static void ss_release(void *value){if(value)HeapFree(GetProcessHeap(),0,value);}
static void ss_copy(void *to,const void *from,size_t length){volatile unsigned char *out=to;const volatile unsigned char *in=from;for(size_t i=0;i<length;i++)out[i]=in[i];}
static void ss_zero(void *to,int value,size_t length){volatile unsigned char *out=to;for(size_t i=0;i<length;i++)out[i]=(unsigned char)value;}
static const void *ss_find(const void *from,int value,size_t length){const volatile unsigned char *in=from;for(size_t i=0;i<length;i++)if(in[i]==(unsigned char)value)return (const unsigned char*)from+i;return NULL;}
static int ss_wide_equal(const wchar_t *left,const wchar_t *right,size_t length){for(size_t i=0;i<length;i++)if(left[i]!=right[i])return 1;return 0;}

/* Unsafe compiler-only OS ABI. SG owns lexical path policy, all source/count/
 * byte budgets, deduplication, complete reads, EOF/metadata comparisons and SHA.
 * No public snapshot owner registry, source contents or digest lives here. */
enum { SS_EXTENT=1024, SS_DIRECTORIES=64, SS_WIDE=32768 };
#define SS_MAGIC UINT64_C(0x5347534E41503031)
typedef struct {
    uint64_t magic;
    HANDLE file;
    HANDLE directories[SS_DIRECTORIES];
    uint64_t count;
} CompilerSnapshotState;
_Static_assert(sizeof(CompilerSnapshotState)==536,"Windows x64 snapshot ABI");
_Static_assert(_Alignof(CompilerSnapshotState)==8,"Windows x64 snapshot alignment");

static int64_t ss_error(DWORD error) {
    if(error==ERROR_FILE_NOT_FOUND||error==ERROR_PATH_NOT_FOUND)return -5;
    if(error==ERROR_ACCESS_DENIED||error==ERROR_SHARING_VIOLATION||error==ERROR_LOCK_VIOLATION)return -7;
    if(error==ERROR_NOT_ENOUGH_MEMORY||error==ERROR_OUTOFMEMORY)return -14;
    if(error==ERROR_NOT_SUPPORTED||error==ERROR_INVALID_FUNCTION)return -8;
    return -9;
}
static CompilerSnapshotState *ss_storage(int64_t address,int64_t extent) {
    if(!address||extent!=SS_EXTENT||((uint64_t)address&7))return NULL;
    return (CompilerSnapshotState*)(intptr_t)address;
}
static CompilerSnapshotState *ss_active(int64_t address,int64_t extent) {
    CompilerSnapshotState *state=ss_storage(address,extent);
    return state&&state->magic==SS_MAGIC&&state->count<=SS_DIRECTORIES?state:NULL;
}
static int ss_empty(const CompilerSnapshotState *state) {
    const unsigned char *bytes=(const unsigned char*)state;
    for(size_t i=0;i<SS_EXTENT;i++)if(bytes[i])return 0;
    return 1;
}
int64_t sengoo_compiler_snapshot_init(int64_t address,int64_t extent) {
    CompilerSnapshotState *state=ss_storage(address,extent);
    if(!state)return -2;
    /* Fresh zeroed caller storage, or an already initialized unused state. */
    if(state->magic==SS_MAGIC&&!state->file&&!state->count)return 0;
    if(!ss_empty(state))return -2;
    ss_zero(state,0,SS_EXTENT);state->magic=SS_MAGIC;return 0;
}
int64_t sengoo_compiler_snapshot_close(int64_t address,int64_t extent) {
    CompilerSnapshotState *state=ss_storage(address,extent);
    if(!state)return -2;
    if(ss_empty(state))return 0;
    if(!ss_active(address,extent))return -3;
    int64_t status=0;
    if(state->file&&state->file!=INVALID_HANDLE_VALUE&&!CloseHandle(state->file))status=-9;
    while(state->count)if(!CloseHandle(state->directories[--state->count]))status=-9;
    ss_zero(state,0,SS_EXTENT);return status;
}
/* UTF-8/UTF-16 conversions are Win32 marshalling. Embedded NUL cannot be passed
 * through a NUL-terminated OS path call, independent of SG's lexical policy. */
static int64_t ss_wide(int64_t address,int64_t length,wchar_t **out) {
    *out=NULL;
    if(!address||length<1||length>INT_MAX)return -2;
    const char *bytes=(const char*)(intptr_t)address;
    if(ss_find(bytes,0,(size_t)length))return -2;
    int count=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,bytes,(int)length,NULL,0);
    if(!count)return -20;
    if(count>=SS_WIDE-4)return -13;
    wchar_t *wide=ss_allocate(((size_t)count+1)*sizeof(wchar_t));
    if(!wide)return -14;
    if(!MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,bytes,(int)length,wide,count)){ss_release(wide);return -20;}
    wide[count]=0;
    for(int i=0;i<count;i++)if(wide[i]==L'/')wide[i]=L'\\';
    *out=wide;return count;
}
static int64_t ss_utf8(const wchar_t *wide,int length,int64_t output,int64_t capacity) {
    if(!output||capacity<0||capacity>INT_MAX)return -2;
    int count=WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,wide,length,NULL,0,NULL,NULL);
    if(!count)return -20;
    if(count>capacity)return -4;
    if(WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,wide,length,(char*)(intptr_t)output,count,NULL,NULL)!=count)return -20;
    return count; /* Exact content bytes; no trailing NUL is written. */
}
int64_t sengoo_compiler_snapshot_full_path(int64_t input,int64_t length,int64_t output,int64_t capacity) {
    wchar_t *wide=NULL;int64_t status=ss_wide(input,length,&wide);
    if(status<0)return status;
    wchar_t *full=ss_allocate(SS_WIDE*sizeof(wchar_t));
    if(!full){ss_release(wide);return -14;}
    DWORD count=GetFullPathNameW(wide,SS_WIDE,full,NULL);ss_release(wide);
    if(!count||count>=SS_WIDE){status=count?-13:ss_error(GetLastError());ss_release(full);return status;}
    status=ss_utf8(full,(int)count,output,capacity);ss_release(full);return status;
}
int64_t sengoo_compiler_snapshot_drive_type(int64_t input,int64_t length) {
    wchar_t *wide=NULL;int64_t status=ss_wide(input,length,&wide);
    if(status<0)return status;
    UINT kind=GetDriveTypeW(wide);ss_release(wide);return (int64_t)kind;
}
static int64_t ss_guard(CompilerSnapshotState *state,const wchar_t *path) {
    if(state->count>=SS_DIRECTORIES)return -13;
    HANDLE directory=CreateFileW(path,FILE_READ_ATTRIBUTES,FILE_SHARE_READ|FILE_SHARE_WRITE,NULL,OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT,NULL);
    if(directory==INVALID_HANDLE_VALUE)return ss_error(GetLastError());
    BY_HANDLE_FILE_INFORMATION info;
    if(!GetFileInformationByHandle(directory,&info)){int64_t status=ss_error(GetLastError());CloseHandle(directory);return status;}
    /* These are physical retained-open invariants, not source-language policy. */
    if(!(info.dwFileAttributes&FILE_ATTRIBUTE_DIRECTORY)||(info.dwFileAttributes&FILE_ATTRIBUTE_REPARSE_POINT)){CloseHandle(directory);return -2;}
    state->directories[state->count++]=directory;return 0;
}
int64_t sengoo_compiler_snapshot_open(int64_t address,int64_t extent,int64_t input,int64_t length) {
    CompilerSnapshotState *state=ss_active(address,extent);
    if(!state)return -3;
    if(state->file||state->count)return -2;
    wchar_t *normalized=NULL;int64_t count=ss_wide(input,length,&normalized);
    if(count<0)return count;
    /* SG has checked the normalized DOS path, components and fixed-drive policy.
     * Only this mechanical shape is needed to identify its root and suffix. */
    if(count<4||normalized[1]!=L':'||normalized[2]!=L'\\'){ss_release(normalized);return -2;}
    wchar_t root[8]={L'\\',L'\\',L'?',L'\\',normalized[0],L':',L'\\',0};
    int64_t status=ss_guard(state,root);wchar_t *pinned=NULL;size_t root_length=0;
    if(!status){
        wchar_t volume[128];DWORD size=GetFinalPathNameByHandleW(state->directories[0],volume,128,VOLUME_NAME_GUID);
        if(!size||size>=128)status=size?-13:ss_error(GetLastError());
        else if(ss_wide_equal(volume,L"\\\\?\\Volume{",11)||volume[size-1]!=L'\\')status=-8;
        else if((size_t)size+(size_t)count-3>=SS_WIDE)status=-13;
        else{
            pinned=ss_allocate(SS_WIDE*sizeof(wchar_t));
            if(!pinned)status=-14;
            else{ss_copy(pinned,volume,size*sizeof(wchar_t));ss_copy(pinned+size,normalized+3,((size_t)count-3+1)*sizeof(wchar_t));root_length=size;}
        }
    }
    ss_release(normalized);
    for(size_t i=root_length;!status&&pinned[i];i++)if(pinned[i]==L'\\'){
        pinned[i]=0;status=ss_guard(state,pinned);pinned[i]=L'\\';
    }
    if(!status){
        state->file=CreateFileW(pinned,GENERIC_READ,FILE_SHARE_READ,NULL,OPEN_EXISTING,FILE_FLAG_SEQUENTIAL_SCAN|FILE_FLAG_OPEN_REPARSE_POINT|FILE_FLAG_BACKUP_SEMANTICS,NULL);
        if(state->file==INVALID_HANDLE_VALUE){status=ss_error(GetLastError());state->file=NULL;}
    }
    ss_release(pinned);
    if(!status&&GetFileType(state->file)!=FILE_TYPE_DISK)status=-2;
    if(!status){
        BY_HANDLE_FILE_INFORMATION info;
        if(!GetFileInformationByHandle(state->file,&info))status=ss_error(GetLastError());
        else if(info.dwFileAttributes&(FILE_ATTRIBUTE_DIRECTORY|FILE_ATTRIBUTE_REPARSE_POINT))status=-2;
    }
    if(status){int64_t ignored=sengoo_compiler_snapshot_close(address,extent);(void)ignored;}
    return status;
}
int64_t sengoo_compiler_snapshot_read(int64_t address,int64_t extent,int64_t output,int64_t capacity) {
    CompilerSnapshotState *state=ss_active(address,extent);
    if(!state||!state->file)return -3;
    if(!output||capacity<1)return -2;
    if((uint64_t)capacity>UINT32_MAX)return -13;
    DWORD count=0;
    if(!ReadFile(state->file,(void*)(intptr_t)output,(DWORD)capacity,&count,NULL))return ss_error(GetLastError());
    return count;
}
int64_t sengoo_compiler_snapshot_metadata(int64_t address,int64_t extent,int64_t output,int64_t capacity) {
    CompilerSnapshotState *state=ss_active(address,extent);
    if(!state||!state->file)return -3;
    if(!output||capacity<0)return -2;
    if(capacity<80)return -4;
    BY_HANDLE_FILE_INFORMATION info;
    if(!GetFileInformationByHandle(state->file,&info))return ss_error(GetLastError());
    uint64_t words[10]={info.dwVolumeSerialNumber,info.nFileIndexHigh,info.nFileIndexLow,info.nFileSizeHigh,info.nFileSizeLow,info.dwFileAttributes,info.ftCreationTime.dwHighDateTime,info.ftCreationTime.dwLowDateTime,info.ftLastWriteTime.dwHighDateTime,info.ftLastWriteTime.dwLowDateTime};
    ss_copy((void*)(intptr_t)output,words,sizeof(words));return 0;
}
int64_t sengoo_compiler_snapshot_path(int64_t address,int64_t extent,int64_t kind,int64_t output,int64_t capacity) {
    CompilerSnapshotState *state=ss_active(address,extent);
    if(!state||!state->file)return -3;
    if(kind<0||kind>1)return -2;
    wchar_t *wide=ss_allocate(SS_WIDE*sizeof(wchar_t));
    if(!wide)return -14;
    DWORD count=GetFinalPathNameByHandleW(state->file,wide,SS_WIDE,kind?VOLUME_NAME_GUID:VOLUME_NAME_DOS);
    if(!count||count>=SS_WIDE){int64_t status=count?-13:ss_error(GetLastError());ss_release(wide);return status;}
    const wchar_t *start=wide;
    if(!kind&&count>=7&&!ss_wide_equal(wide,L"\\\\?\\",4)&&wide[5]==L':'){start+=4;count-=4;}
    int64_t status=ss_utf8(start,(int)count,output,capacity);ss_release(wide);return status;
}

/* Reposition the same retained handle; never reopen a path or release guards. */
int64_t sengoo_compiler_snapshot_seek(int64_t address,int64_t extent,int64_t offset) {
    CompilerSnapshotState *state=ss_active(address,extent);
    if(!state||!state->file)return -3;
    if(offset<0)return -2;
    LARGE_INTEGER position;position.QuadPart=offset;
    if(!SetFilePointerEx(state->file,position,NULL,FILE_BEGIN))return ss_error(GetLastError());
    return 0;
}
