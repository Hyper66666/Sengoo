#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <psapi.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <wchar.h>
#include <winioctl.h>
#include <string.h>

extern int64_t sengoo_sg_package_extract(const uint8_t *,int64_t,int64_t,const uint8_t *,int64_t,const uint8_t *,int64_t,int64_t);
static uint64_t live_bytes,peak_bytes,allocations,frees;
static int64_t fail_allocation=-1;
static int argc_saved;
static wchar_t **argv_saved;
static uint64_t os_sessions,os_frees,os_region_bytes,attack_processes,readonly_files;
static uint64_t created_files,created_dirs,write_calls,closed_children,reopened_children,identity_rejections;
static int attack_exit=-1;
static int parent_identity_equal;
static int same_identity(const FILE_ID_INFO *a,const FILE_ID_INFO *b){
    if(a->VolumeSerialNumber!=b->VolumeSerialNumber)return 0;
    for(unsigned i=0;i<16;i++)if(a->FileId.Identifier[i]!=b->FileId.Identifier[i])return 0;return 1;
}
static int write_marker(const wchar_t *path){
    HANDLE file=CreateFileW(path,GENERIC_WRITE,0,NULL,CREATE_NEW,FILE_ATTRIBUTE_NORMAL,NULL);DWORD done;
    if(file==INVALID_HANDLE_VALUE)return 0;int ok=WriteFile(file,"foreign",7,&done,NULL)&&done==7;CloseHandle(file);return ok;
}
static int attack_main(int argc,wchar_t **argv){
    if(argc!=7)return 70;wchar_t path[32768];
    if(!wcscmp(argv[2],L"warmup"))return 0;
    if(!wcscmp(argv[2],L"unknown")){
        swprintf_s(path,32768,L"%ls\\%ls\\foreign.keep",argv[3],argv[4]);return write_marker(path)?0:71;
    }
    if(!wcscmp(argv[2],L"target")||!wcscmp(argv[2],L"replace")){
        if(!wcscmp(argv[2],L"replace")){
            swprintf_s(path,32768,L"%ls\\%ls\\Sengoo.toml",argv[3],argv[4]);
            if(!DeleteFileW(path)||!write_marker(path))return 72;
        }
        swprintf_s(path,32768,L"%ls\\%ls",argv[3],argv[5]);if(!CreateDirectoryW(path,NULL))return 73;
        swprintf_s(path,32768,L"%ls\\%ls\\race.keep",argv[3],argv[5]);return write_marker(path)?0:74;
    }
    if(!wcscmp(argv[2],L"reparse")){
        swprintf_s(path,32768,L"%ls\\%ls\\src",argv[3],argv[4]);if(!CreateDirectoryW(path,NULL))return 75;
        HANDLE dir=CreateFileW(path,GENERIC_WRITE,0,NULL,OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT,NULL);
        if(dir==INVALID_HANDLE_VALUE)return 76;
        struct {DWORD tag;WORD size,reserved;WORD suboff,sublen,printoff,printlen;WCHAR path[32768];} data;
        data.tag=IO_REPARSE_TAG_MOUNT_POINT;data.reserved=0;data.suboff=0;
        int n=swprintf_s(data.path,32768,L"\\??\\%ls",argv[6]);if(n<1)return 77;
        data.sublen=(WORD)(n*2);data.printoff=(WORD)((n+1)*2);data.printlen=0;data.path[n+1]=0;data.size=(WORD)(8+(n+2)*2);
        DWORD done;int ok=DeviceIoControl(dir,FSCTL_SET_REPARSE_POINT,&data,data.size+8,NULL,0,&done,NULL);CloseHandle(dir);return ok?0:78;
    }
    return 79;
}
static void attack_child(const wchar_t *mode){
    wchar_t exe[32768],cmd[32768];GetModuleFileNameW(NULL,exe,32768);
    const wchar_t *outside=argc_saved>8?argv_saved[8]:argv_saved[1];
    swprintf_s(cmd,32768,L"\"%ls\" --attack %ls \"%ls\" %ls %ls \"%ls\"",exe,mode,argv_saved[1],argv_saved[3],argv_saved[4],outside);
    STARTUPINFOW startup={sizeof(startup)};PROCESS_INFORMATION process;
    if(!CreateProcessW(exe,cmd,NULL,NULL,FALSE,CREATE_NO_WINDOW,NULL,NULL,&startup,&process))exit(94);
    if(WaitForSingleObject(process.hProcess,30000)!=WAIT_OBJECT_0)exit(95);DWORD result;GetExitCodeProcess(process.hProcess,&result);attack_exit=(int)result;
    CloseHandle(process.hThread);CloseHandle(process.hProcess);attack_processes++;
}
void sengoo_ps_observe(int phase,void *session,HANDLE *handle){
    const wchar_t *mode=argc_saved>6?argv_saved[6]:L"none";
    if(phase==0){MEMORY_BASIC_INFORMATION memory;if(!VirtualQuery(session,&memory,sizeof(memory))||memory.State!=MEM_COMMIT)exit(96);os_sessions++;os_region_bytes=memory.RegionSize;
        if(!wcscmp(mode,L"unknown")||!wcscmp(mode,L"reparse"))attack_child(mode);
    }else if(phase==1){created_files++;
        if(!wcscmp(mode,L"readonly")&&!readonly_files){HANDLE copy;
            if(!DuplicateHandle(GetCurrentProcess(),*handle,GetCurrentProcess(),&copy,FILE_GENERIC_READ|DELETE,FALSE,0))exit(97);
            if(!CloseHandle(*handle))exit(98);*handle=copy;readonly_files++;}
    }else if(phase==2){if(!wcscmp(mode,L"target")||!wcscmp(mode,L"replace"))attack_child(mode);}
    else if(phase==3){MEMORY_BASIC_INFORMATION memory;if(!VirtualQuery(session,&memory,sizeof(memory))||memory.State!=MEM_FREE)exit(99);os_frees++;}
    else if(phase==5)created_dirs++;else if(phase==6)write_calls++;else if(phase==7)closed_children++;
    else if(phase==8)reopened_children++;else if(phase==9)identity_rejections++;
}
int sengoo_ps_fail_session_alloc(void){return argc_saved>6&&!wcscmp(argv_saved[6],L"session-oom");}
static uint64_t verified_releases,memory_reads,memory_writes;
static struct{void *value;int64_t size;uint8_t *initialized;} allocated[8];
static const uint8_t *wire_input;
static int64_t wire_size;
static const uint8_t *arg_bytes[2];
static int64_t arg_sizes[2];
static uint8_t *checked(const uint8_t *base,int64_t index,int bytes,int writing){
    uintptr_t at=(uintptr_t)base;if(index<0||index>INT64_MAX/bytes)exit(81);uint64_t offset=(uint64_t)index*bytes;
    for(unsigned i=0;i<8;i++)if(allocated[i].value){uintptr_t start=(uintptr_t)allocated[i].value;
        if(at>=start&&at-start<=(uint64_t)allocated[i].size){
            uint64_t left=(uint64_t)allocated[i].size-(at-start);if(offset>left||(unsigned)bytes>left-offset)exit(82);
            uint64_t position=at-start+offset;
            if(writing){memset(allocated[i].initialized+position,1,bytes);memory_writes++;}
            else{for(int j=0;j<bytes;j++)if(!allocated[i].initialized[position+j])exit(83);memory_reads++;}
            return (uint8_t *)(at+offset);
        }
    }
    uintptr_t input=(uintptr_t)wire_input;
    if(!writing&&at>=input&&at-input<=(uint64_t)wire_size){uint64_t left=(uint64_t)wire_size-(at-input);
        if(offset<=left&&(unsigned)bytes<=left-offset){memory_reads++;return (uint8_t *)(at+offset);}}
    for(unsigned i=0;i<2;i++){uintptr_t begin=(uintptr_t)arg_bytes[i];if(!writing&&at>=begin&&at-begin<=(uint64_t)arg_sizes[i]){
        uint64_t left=(uint64_t)arg_sizes[i]-(at-begin);if(offset<=left&&(unsigned)bytes<=left-offset){memory_reads++;return (uint8_t *)(at+offset);}}}
    exit(84);
}
uint8_t sengoo_tar_read(const uint8_t *base,int64_t index){return *checked(base,index,1,0);}
uint8_t sengoo_tar_output_read(uint8_t *base,int64_t index){return *checked(base,index,1,0);}
void sengoo_tar_write(uint8_t *base,int64_t index,uint8_t value){*checked(base,index,1,1)=value;}
void sengoo_tar_observe_os_compare(void){}
uint8_t sengoo_ds_input_byte(const uint8_t *base,int64_t index){return *checked(base,index,1,0);}
uint8_t sengoo_ds_state_byte(uint8_t *base,int64_t index){return *checked(base,index,1,0);}
void sengoo_ds_state_write(uint8_t *base,int64_t index,uint8_t value){*checked(base,index,1,1)=value;}
void sengoo_ds_output_write(uint8_t *base,int64_t index,uint8_t value){*checked(base,index,1,1)=value;}
int64_t sengoo_ds_word_get(uint8_t *base,int64_t index){int64_t value;memcpy(&value,checked(base,index,8,0),8);return value;}
void sengoo_ds_word_put(uint8_t *base,int64_t index,int64_t value){memcpy(checked(base,index,8,1),&value,8);}
uint8_t sengoo_deflate_input_read(const uint8_t *base,int64_t index){return *checked(base,index,1,0);}
uint8_t sengoo_deflate_output_read(uint8_t *base,int64_t index){return *checked(base,index,1,0);}
void sengoo_deflate_output_write(uint8_t *base,int64_t index,uint8_t value){*checked(base,index,1,1)=value;}
uint8_t *sengoo_ds_storage_alloc(int64_t size){
    if(size<=0)return NULL;if(fail_allocation==(int64_t)allocations)return NULL;
    void *result=VirtualAlloc(NULL,(SIZE_T)size,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
    if(!result)return NULL;
    for(unsigned i=0;i<8;i++)if(!allocated[i].value){allocated[i].initialized=calloc((size_t)size,1);if(!allocated[i].initialized)exit(80);allocated[i].value=result;allocated[i].size=size;live_bytes+=size;if(live_bytes>peak_bytes)peak_bytes=live_bytes;allocations++;return result;}
    exit(91);
}
int64_t sengoo_ds_storage_present(uint8_t *value){return value!=NULL;}
void sengoo_ds_storage_free(uint8_t *value){
    for(unsigned i=0;i<8;i++)if(allocated[i].value==value){
        if(!VirtualFree(value,0,MEM_RELEASE))exit(92);MEMORY_BASIC_INFORMATION memory;
        if(!VirtualQuery(value,&memory,sizeof(memory))||memory.State!=MEM_FREE)exit(85);verified_releases++;
        live_bytes-=allocated[i].size;allocated[i].value=NULL;free(allocated[i].initialized);allocated[i].initialized=NULL;frees++;return;}
    exit(93);
}
int wmain(int argc,wchar_t **argv){
    if(argc==4&&!wcscmp(argv[1],L"--parent-identity")){
        HANDLE handles[2];FILE_ID_INFO ids[2];
        for(unsigned i=0;i<2;i++){handles[i]=CreateFileW(argv[i+2],GENERIC_READ,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,NULL,OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS,NULL);
            if(handles[i]==INVALID_HANDLE_VALUE||!GetFileInformationByHandleEx(handles[i],FileIdInfo,&ids[i],sizeof(ids[i])))return 61;}
        int equal=same_identity(&ids[0],&ids[1]);CloseHandle(handles[0]);CloseHandle(handles[1]);
        printf("{\"schema\":1,\"same_volume_and_file_id\":%s,\"volume\":%llu,\"file_id\":\"",equal?"true":"false",(unsigned long long)ids[0].VolumeSerialNumber);
        for(unsigned i=0;i<16;i++)printf("%02x",ids[0].FileId.Identifier[i]);puts("\"}");return equal?0:62;
    }
    if(argc==2&&!wcscmp(argv[1],L"--api-controls")){
        uint64_t reads=memory_reads,writes=memory_writes;unsigned cases=0;
        const int64_t bad[][6]={{-1,1,7,5,-1,0},{67108865,1,7,5,-1,0},{0,0,7,5,-1,0},{0,1,0,5,-1,0},{0,1,65,5,-1,0},{0,1,7,0,-1,0},{0,1,7,65,-1,0},{0,1,7,5,-2,0}};
        for(unsigned i=0;i<sizeof(bad)/sizeof(bad[0]);i++){
            if(sengoo_sg_package_extract(NULL,bad[i][0],bad[i][1],NULL,bad[i][2],NULL,bad[i][3],bad[i][4])!=-3)exit(86);cases++;
        }
        if(memory_reads!=reads||memory_writes!=writes||allocations||frees)exit(87);
        static const uint8_t stage[]="private",target[]="PRIVATE",device[]="CON";
        arg_bytes[0]=stage;arg_sizes[0]=7;arg_bytes[1]=target;arg_sizes[1]=7;
        if(sengoo_sg_package_extract(NULL,0,1,stage,7,target,7,-1)!=-3||allocations)exit(88);cases++;
        arg_bytes[0]=device;arg_sizes[0]=3;
        if(sengoo_sg_package_extract(NULL,0,1,device,3,target,7,-1)!=-3||allocations)exit(89);cases++;
        printf("{\"schema\":1,\"controls\":%u,\"bad_extent_zero_access\":true,\"same_stage_target_rejected\":true,\"dos_root_rejected\":true,\"allocations\":0}\n",cases);return 0;
    }
    if(argc>1&&!wcscmp(argv[1],L"--attack"))return attack_main(argc,argv);
    if(argc<6||argc>9)return 64;argc_saved=argc;argv_saved=argv;if(argc>7)fail_allocation=_wtoi64(argv[7]);
    HANDLE source=CreateFileW(argv[2],GENERIC_READ,FILE_SHARE_READ,NULL,OPEN_EXISTING,FILE_ATTRIBUTE_NORMAL,NULL);
    LARGE_INTEGER size;if(source==INVALID_HANDLE_VALUE||!GetFileSizeEx(source,&size)||size.QuadPart>67108864)return 65;
    HANDLE mapping=size.QuadPart?CreateFileMappingW(source,NULL,PAGE_READONLY,0,0,NULL):NULL;
    const uint8_t *bytes=mapping?MapViewOfFile(mapping,FILE_MAP_READ,0,0,0):NULL;
    if(size.QuadPart&&!bytes)return 66;
    HANDLE parent=CreateFileW(argv[1],GENERIC_READ|GENERIC_WRITE|DELETE,
        FILE_SHARE_READ|FILE_SHARE_WRITE,NULL,OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT,NULL);
    FILE_ATTRIBUTE_TAG_INFO tag;
    if(parent==INVALID_HANDLE_VALUE||!GetFileInformationByHandleEx(parent,FileAttributeTagInfo,&tag,sizeof(tag))||
       (tag.FileAttributes&FILE_ATTRIBUTE_REPARSE_POINT)||!(tag.FileAttributes&FILE_ATTRIBUTE_DIRECTORY))return 67;
    FILE_ID_INFO identity_before,identity_after;
    if(!GetFileInformationByHandleEx(parent,FileIdInfo,&identity_before,sizeof(identity_before)))return 60;
    char stage[128],target[128];
    int n=WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,argv[3],-1,stage,sizeof(stage),NULL,NULL);
    int m=WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,argv[4],-1,target,sizeof(target),NULL,NULL);
    if(n<2||m<2)return 68;
    DWORD before,after,warmup_before=0,warmup_after=0;
    if(argc>6&&(!wcscmp(argv[6],L"target")||!wcscmp(argv[6],L"replace")||!wcscmp(argv[6],L"unknown")||!wcscmp(argv[6],L"reparse"))){
        GetProcessHandleCount(GetCurrentProcess(),&warmup_before);attack_child(L"warmup");GetProcessHandleCount(GetCurrentProcess(),&warmup_after);}
    GetProcessHandleCount(GetCurrentProcess(),&before);
    wire_input=bytes;wire_size=size.QuadPart;arg_bytes[0]=(const uint8_t *)stage;arg_sizes[0]=n-1;arg_bytes[1]=(const uint8_t *)target;arg_sizes[1]=m-1;
    /* Stage/target are separate caller extents, so name reads are checked by
     * their fixed arrays rather than pretending they belong to gzip input. */
    int64_t result=sengoo_sg_package_extract(bytes,size.QuadPart,(int64_t)(uintptr_t)parent,(const uint8_t *)stage,n-1,(const uint8_t *)target,m-1,_wtoi64(argv[5]));
    if(!GetFileInformationByHandleEx(parent,FileIdInfo,&identity_after,sizeof(identity_after)))return 60;
    parent_identity_equal=same_identity(&identity_before,&identity_after);
    GetProcessHandleCount(GetCurrentProcess(),&after);
    PROCESS_MEMORY_COUNTERS_EX memory={sizeof(memory)};GetProcessMemoryInfo(GetCurrentProcess(),(PROCESS_MEMORY_COUNTERS *)&memory,sizeof(memory));
    printf("{\"parent_volume\":%llu,\"parent_file_id\":\"",(unsigned long long)identity_before.VolumeSerialNumber);
    for(unsigned i=0;i<16;i++)printf("%02x",identity_before.FileId.Identifier[i]);
    printf("\",\"parent_identity_stable\":%s,",parent_identity_equal?"true":"false");
    printf("\"schema\":1,\"result\":%lld,\"allocations\":%llu,\"frees\":%llu,\"live_bytes\":%llu,\"peak_sg_bytes\":%llu,\"handles_before\":%lu,\"handles_after\":%lu,\"peak_working_set\":%llu,\"os_sessions\":%llu,\"os_frees\":%llu,\"os_region_bytes\":%llu,\"attack_processes\":%llu,\"attack_exit\":%d,\"readonly_files\":%llu,\"warmup_handles_before\":%lu,\"warmup_handles_after\":%lu,\"created_files\":%llu,\"created_dirs\":%llu,\"write_calls\":%llu,\"closed_children\":%llu,\"reopened_children\":%llu,\"identity_rejections\":%llu,\"verified_releases\":%llu,\"memory_reads\":%llu,\"memory_writes\":%llu}\n",
        (long long)result,(unsigned long long)allocations,(unsigned long long)frees,(unsigned long long)live_bytes,(unsigned long long)peak_bytes,
        (unsigned long)before,(unsigned long)after,(unsigned long long)memory.PeakWorkingSetSize,
        (unsigned long long)os_sessions,(unsigned long long)os_frees,(unsigned long long)os_region_bytes,(unsigned long long)attack_processes,attack_exit,(unsigned long long)readonly_files,(unsigned long)warmup_before,(unsigned long)warmup_after,
        (unsigned long long)created_files,(unsigned long long)created_dirs,(unsigned long long)write_calls,(unsigned long long)closed_children,(unsigned long long)reopened_children,(unsigned long long)identity_rejections,
        (unsigned long long)verified_releases,(unsigned long long)memory_reads,(unsigned long long)memory_writes);
    CloseHandle(parent);if(bytes)UnmapViewOfFile(bytes);if(mapping)CloseHandle(mapping);CloseHandle(source);
    return live_bytes||allocations!=frees||before!=after||os_sessions!=os_frees||!parent_identity_equal?69:0;
}
