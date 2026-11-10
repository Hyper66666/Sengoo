#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
__declspec(dllimport) int64_t sengoo_ps_cli_count(void);
__declspec(dllimport) int64_t sengoo_ps_cli_open_input(int64_t);
__declspec(dllimport) const uint8_t *sengoo_ps_cli_input_bytes(int64_t);
__declspec(dllimport) int64_t sengoo_ps_cli_input_size(int64_t);
__declspec(dllimport) void sengoo_ps_cli_close_input(int64_t);
__declspec(dllimport) void sengoo_ps_cli_dispose(void);
__declspec(dllimport) int64_t sengoo_pa_cli_input_identity(int64_t,int64_t);
int wmain(int argc,wchar_t **argv){
    if(argc!=2||sengoo_ps_cli_count()!=1)return 64;
    DWORD before,after;GetProcessHandleCount(GetCurrentProcess(),&before);
    int64_t input=sengoo_ps_cli_open_input(0);if(!input)return 65;
    int64_t identity[4];for(int i=0;i<4;i++)identity[i]=sengoo_pa_cli_input_identity(input,i);
    if(identity[3]<1||identity[3]!=sengoo_ps_cli_input_size(input)||!sengoo_ps_cli_input_bytes(input))return 66;
    HANDLE write=CreateFileW(argv[1],GENERIC_WRITE,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,NULL,OPEN_EXISTING,0,NULL);
    DWORD write_error=GetLastError();if(write!=INVALID_HANDLE_VALUE){CloseHandle(write);return 67;}
    HANDLE remove=CreateFileW(argv[1],DELETE,FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,NULL,OPEN_EXISTING,0,NULL);
    DWORD delete_error=GetLastError();if(remove!=INVALID_HANDLE_VALUE){CloseHandle(remove);return 68;}
    if(write_error!=ERROR_SHARING_VIOLATION||delete_error!=ERROR_SHARING_VIOLATION)return 69;
    for(int i=0;i<4;i++)if(identity[i]!=sengoo_pa_cli_input_identity(input,i))return 70;
    sengoo_ps_cli_close_input(input);GetProcessHandleCount(GetCurrentProcess(),&after);sengoo_ps_cli_dispose();
    printf("{\"schema\":1,\"write_denied\":%lu,\"delete_denied\":%lu,\"identity_stable\":true,\"mapped_size\":%lld,\"handles_before\":%lu,\"handles_after\":%lu}\n",(unsigned long)write_error,(unsigned long)delete_error,(long long)identity[3],(unsigned long)before,(unsigned long)after);
    return before!=after?71:0;
}
