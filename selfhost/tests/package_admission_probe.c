/* Constant fixture construction only; the parser, hashes and admission run SG. */
#include "../runtime/package_admission_os.c"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
extern int64_t sh_pa_preflight(uint8_t *);
extern int64_t sh_pa_archive_checksum(const uint8_t *,int64_t,uint8_t *,uint8_t *,uint8_t *);
extern int64_t sh_pa_root_manifest(int64_t,uint8_t *,uint8_t *);
extern int64_t sh_pa_read_manifest(int64_t,int64_t,uint8_t *,uint8_t *,uint8_t *);
extern int64_t sh_pa_targets(int64_t,uint8_t *,uint8_t *,uint8_t *,uint8_t *);
static void word(uint8_t *bytes,int64_t at,int64_t value){memcpy(bytes+at*8,&value,8);}
int main(int argc,char **argv){
    if(argc!=8)return 64;FILE *file=NULL;if(fopen_s(&file,argv[2],"rb")||!file)return 65;fseek(file,0,SEEK_END);long size=ftell(file);rewind(file);
    uint8_t *source=malloc(size+1);if(!source||fread(source,1,size,file)!=(size_t)size)return 66;fclose(file);
    wchar_t parent_text[32768];if(!MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,argv[1],-1,parent_text,32768))return 67;
    HANDLE parent=CreateFileW(parent_text,GENERIC_READ|GENERIC_WRITE|DELETE,FILE_SHARE_READ|FILE_SHARE_WRITE,NULL,OPEN_EXISTING,FILE_FLAG_BACKUP_SEMANTICS|FILE_FLAG_OPEN_REPARSE_POINT,NULL);if(parent==INVALID_HANDLE_VALUE)return 68;
    uint8_t *tar=VirtualAlloc(NULL,18438400,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE),*nodes=VirtualAlloc(NULL,1128584,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
    uint8_t *document=VirtualAlloc(NULL,8388608,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE),*buffer=VirtualAlloc(NULL,1048576,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE),*hash=VirtualAlloc(NULL,1024,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
    if(!tar||!nodes||!document||!buffer||!hash)return 69;
    DWORD before,after;GetProcessHandleCount(GetCurrentProcess(),&before);
    memcpy(tar+1661184,"Sengoo.tomlsrcmain.sg",21);
    int64_t session=sengoo_ps_session_new((int64_t)(uintptr_t)parent,(const uint8_t *)"private",7);if(!session)return 70;
    int64_t manifest=sengoo_ps_create_node(session,0,tar+1661184,11,0);
    int64_t directory=sengoo_ps_create_node(session,0,tar+1661195,3,1);
    int64_t main=sengoo_ps_create_node(session,directory,tar+1661198,7,0);
    if(manifest!=1||directory!=2||main!=3)return 71;
    for(long at=0;at<size;){long n=size-at;if(n>32768)n=32768;if(sengoo_ps_write_node(session,manifest,source+at,n))return 72;at+=n;}
    static const uint8_t main_text[]="def main()->i64{42;}\n";
    if(sengoo_ps_write_node(session,main,main_text,sizeof(main_text)-1))return 73;
    if(!strcmp(argv[7],"short")){ps_session *raw=(ps_session *)(uintptr_t)session;if(!SetFileShortNameW(raw->nodes[main].handle,L"START"))return 74;}
    word(nodes,0,4);word(nodes,4,0);word(nodes,5,1661184);word(nodes,6,11);word(nodes,7,0);
    word(nodes,8,0);word(nodes,9,1661195);word(nodes,10,3);word(nodes,11,1);
    word(nodes,12,2);word(nodes,13,1661198);word(nodes,14,7);word(nodes,15,0);
    const uint8_t metadata[]="{}";
    uint8_t *arguments=sengoo_pa_arguments_new((const uint8_t *)argv[3],strlen(argv[3]),(const uint8_t *)argv[4],strlen(argv[4]),(const uint8_t *)argv[5],strlen(argv[5]),metadata,2,0);if(!arguments)return 75;
    int64_t result=sh_pa_preflight(arguments);int64_t located=-1;
    if(!result)result=sh_pa_archive_checksum(source,size,arguments,hash,buffer);
    if(!result){located=sh_pa_root_manifest(session,tar,nodes);if(located<0)result=located;}
    if(!result)result=sh_pa_read_manifest(session,located,arguments,buffer,document);
    if(!result)result=sh_pa_targets(session,tar,nodes,document,buffer);
    sengoo_pa_arguments_free(arguments);int64_t cleanup=sengoo_ps_session_close(session);GetProcessHandleCount(GetCurrentProcess(),&after);
    printf("{\"schema\":1,\"result\":%lld,\"manifest_node\":%lld,\"cleanup\":%lld,\"handles_before\":%lu,\"handles_after\":%lu}\n",(long long)result,(long long)located,(long long)cleanup,(unsigned long)before,(unsigned long)after);
    CloseHandle(parent);VirtualFree(hash,0,MEM_RELEASE);VirtualFree(buffer,0,MEM_RELEASE);VirtualFree(document,0,MEM_RELEASE);VirtualFree(nodes,0,MEM_RELEASE);VirtualFree(tar,0,MEM_RELEASE);free(source);
    return cleanup||before!=after||result!=_strtoi64(argv[6],NULL,10)?76:0;
}
