/* Reuse the qualified resource/OS-adversary harness; only caller arguments and
 * memory-input observations differ for the new admitted import entry point. */
#define sengoo_sg_package_extract sh_import_adapter
#define sengoo_tar_read sh_old_tar_read
#define sengoo_ps_observe sh_old_ps_observe
#define wmain sh_package_probe_main
#define SENGOO_PACKAGE_IMPORT_TEST
#include "package_stream_probe.c"
#undef wmain
#undef sengoo_tar_read
#undef sengoo_ps_observe
#undef sengoo_sg_package_extract

extern int64_t sengoo_sg_package_import(const uint8_t *,int64_t,int64_t,const uint8_t *,int64_t,const uint8_t *,int64_t,int64_t,uint8_t *);
extern uint8_t *sengoo_pa_arguments_new(const uint8_t *,int64_t,const uint8_t *,int64_t,const uint8_t *,int64_t,const uint8_t *,int64_t,int64_t);
extern void sengoo_pa_arguments_free(uint8_t *);
static uint8_t *arguments;
static const uint8_t *contract_bytes[4];static int64_t contract_sizes[4];
void sengoo_ps_observe(int phase,void *session,HANDLE *handle){
    sh_old_ps_observe(phase,session,handle);
    if(phase==1&&argc_saved>6&&!wcscmp(argv_saved[6],L"marker-readonly")&&created_files==4){
        HANDLE copy;if(!DuplicateHandle(GetCurrentProcess(),*handle,GetCurrentProcess(),&copy,FILE_GENERIC_READ|DELETE,FALSE,0))exit(102);
        if(!CloseHandle(*handle))exit(103);*handle=copy;readonly_files++;
    }
}
int64_t sh_import_adapter(const uint8_t *source,int64_t length,int64_t parent,const uint8_t *stage,int64_t stage_size,const uint8_t *target,int64_t target_size,int64_t cancel){
    return sengoo_sg_package_import(source,length,parent,stage,stage_size,target,target_size,cancel,arguments);
}
uint8_t sengoo_tar_read(const uint8_t *source,int64_t index){
    uintptr_t value=(uintptr_t)source;
    for(unsigned i=0;i<4;i++){uintptr_t base=(uintptr_t)contract_bytes[i];
        if(value>=base&&value-base<=(uint64_t)contract_sizes[i]){
            if(index<0||(uint64_t)index>=(uint64_t)contract_sizes[i]-(value-base))exit(101);memory_reads++;return source[index];}}
    return *checked(source,index,1,0);
}
uint8_t sengoo_runtime_memory_read_u8(const uint8_t *source,int64_t index){return sengoo_tar_read(source,index);}
int wmain(int argc,wchar_t **argv){
    if(argc>1&&(!wcscmp(argv[1],L"--attack")||!wcscmp(argv[1],L"--api-controls")||!wcscmp(argv[1],L"--parent-identity")))return sh_package_probe_main(argc,argv);
    if(argc!=14)return 64;
    uint8_t *text[3]={0};
    for(unsigned i=0;i<3;i++){
        int size=WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,argv[9+i],-1,NULL,0,NULL,NULL);if(size<1)return 65;
        text[i]=malloc(size);if(!text[i]||!WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,argv[9+i],-1,(char *)text[i],size,NULL,NULL))return 65;
        contract_bytes[i]=text[i];contract_sizes[i]=size-1;
    }
    HANDLE file=CreateFileW(argv[12],GENERIC_READ,FILE_SHARE_READ,NULL,OPEN_EXISTING,FILE_FLAG_OPEN_REPARSE_POINT,NULL);
    LARGE_INTEGER size;if(file==INVALID_HANDLE_VALUE||!GetFileSizeEx(file,&size)||size.QuadPart<1||size.QuadPart>1048576)return 66;
    HANDLE mapping=CreateFileMappingW(file,NULL,PAGE_READONLY,0,0,NULL);const uint8_t *bytes=mapping?MapViewOfFile(mapping,FILE_MAP_READ,0,0,0):NULL;if(!bytes)return 66;
    contract_bytes[3]=bytes;contract_sizes[3]=size.QuadPart;
    arguments=sengoo_pa_arguments_new(text[0],contract_sizes[0],text[1],contract_sizes[1],text[2],contract_sizes[2],bytes,size.QuadPart,_wtoi64(argv[13]));if(!arguments)return 67;
    int result=sh_package_probe_main(9,argv);
    sengoo_pa_arguments_free(arguments);UnmapViewOfFile(bytes);CloseHandle(mapping);CloseHandle(file);for(unsigned i=0;i<3;i++)free(text[i]);return result;
}
