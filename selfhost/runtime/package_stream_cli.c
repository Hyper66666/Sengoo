#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>

/* Command-line and file/mapping handle adapters; no archive or path policy. */
typedef wchar_t **(WINAPI *ps_argv_fn)(const wchar_t *,int *);
typedef struct {int count;int offset[256],length[256];uint8_t text[131072];} ps_args;
static ps_args *args;
typedef struct {HANDLE file,mapping;const uint8_t *bytes;int64_t size;} ps_input;

int64_t sengoo_ps_cli_count(void){
    if(args)return args->count;
    HMODULE shell=LoadLibraryW(L"shell32.dll");if(!shell)return -1;
    ps_argv_fn parse=(ps_argv_fn)GetProcAddress(shell,"CommandLineToArgvW");int count=0;
    wchar_t **wide=parse?parse(GetCommandLineW(),&count):NULL;
    if(!wide||count<1||count>257){if(wide)LocalFree(wide);FreeLibrary(shell);return -1;}
    ps_args *copy=VirtualAlloc(NULL,sizeof(*copy),MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
    if(!copy){LocalFree(wide);FreeLibrary(shell);return -1;}
    int used=0;copy->count=count-1;
    for(int i=1;i<count;i++){
        int n=WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,wide[i],-1,(char *)copy->text+used,131072-used,NULL,NULL);
        if(n<1){VirtualFree(copy,0,MEM_RELEASE);LocalFree(wide);FreeLibrary(shell);return -1;}
        copy->offset[i-1]=used;copy->length[i-1]=n-1;used+=n;
    }
    LocalFree(wide);FreeLibrary(shell);args=copy;return copy->count;
}
const uint8_t *sengoo_ps_cli_arg(int64_t index){return args&&index>=0&&index<args->count?args->text+args->offset[index]:NULL;}
int64_t sengoo_ps_cli_length(int64_t index){return args&&index>=0&&index<args->count?args->length[index]:-1;}
static wchar_t *ps_wide(int64_t index){
    if(!args||index<0||index>=args->count)return NULL;
    wchar_t *wide=VirtualAlloc(NULL,65536,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);if(!wide)return NULL;
    int n=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,(char *)sengoo_ps_cli_arg(index),-1,wide,32768);
    if(n<1){VirtualFree(wide,0,MEM_RELEASE);return NULL;}return wide;
}
int64_t sengoo_ps_cli_open_input(int64_t index){
    wchar_t *wide=ps_wide(index);if(!wide)return 0;
    HANDLE file=CreateFileW(wide,GENERIC_READ,FILE_SHARE_READ,NULL,OPEN_EXISTING,FILE_FLAG_OPEN_REPARSE_POINT,NULL);VirtualFree(wide,0,MEM_RELEASE);
    LARGE_INTEGER size;FILE_ATTRIBUTE_TAG_INFO tag;
    if(file==INVALID_HANDLE_VALUE)return 0;
    if(!GetFileSizeEx(file,&size)||size.QuadPart<0||size.QuadPart>67108864||
       !GetFileInformationByHandleEx(file,FileAttributeTagInfo,&tag,sizeof(tag))||
       (tag.FileAttributes&(FILE_ATTRIBUTE_REPARSE_POINT|FILE_ATTRIBUTE_DIRECTORY))){CloseHandle(file);return 0;}
    ps_input *input=VirtualAlloc(NULL,sizeof(*input),MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);if(!input){CloseHandle(file);return 0;}
    input->file=file;input->size=size.QuadPart;
    if(input->size){input->mapping=CreateFileMappingW(file,NULL,PAGE_READONLY,0,0,NULL);
        if(input->mapping)input->bytes=MapViewOfFile(input->mapping,FILE_MAP_READ,0,0,0);
        if(!input->bytes){if(input->mapping)CloseHandle(input->mapping);CloseHandle(file);VirtualFree(input,0,MEM_RELEASE);return 0;}}
    return (int64_t)(uintptr_t)input;
}
const uint8_t *sengoo_ps_cli_input_bytes(int64_t value){return ((ps_input *)(uintptr_t)value)->bytes;}
int64_t sengoo_ps_cli_input_size(int64_t value){return ((ps_input *)(uintptr_t)value)->size;}
void sengoo_ps_cli_close_input(int64_t value){
    ps_input *input=(ps_input *)(uintptr_t)value;if(!input)return;
    if(input->bytes)UnmapViewOfFile(input->bytes);if(input->mapping)CloseHandle(input->mapping);CloseHandle(input->file);VirtualFree(input,0,MEM_RELEASE);
}
int64_t sengoo_ps_cli_open_parent(int64_t index){
    wchar_t *wide=ps_wide(index);if(!wide)return 0;
    HANDLE dir=CreateFileW(wide,GENERIC_READ|GENERIC_WRITE|DELETE,FILE_SHARE_READ|FILE_SHARE_WRITE,NULL,OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT,NULL);VirtualFree(wide,0,MEM_RELEASE);
    if(dir==INVALID_HANDLE_VALUE)return 0;FILE_ATTRIBUTE_TAG_INFO tag;
    if(!GetFileInformationByHandleEx(dir,FileAttributeTagInfo,&tag,sizeof(tag))||
       !(tag.FileAttributes&FILE_ATTRIBUTE_DIRECTORY)||(tag.FileAttributes&FILE_ATTRIBUTE_REPARSE_POINT)){CloseHandle(dir);return 0;}
    return (int64_t)(uintptr_t)dir;
}
void sengoo_ps_cli_close_parent(int64_t value){if(value)CloseHandle((HANDLE)(uintptr_t)value);}
void sengoo_ps_cli_dispose(void){if(args){VirtualFree(args,0,MEM_RELEASE);args=NULL;}}
