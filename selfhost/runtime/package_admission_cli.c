#include "package_stream_cli.c"
int64_t sengoo_pa_cli_write(const uint8_t *source,int64_t length,int64_t error){
    if(length<0||length>1048576||(error!=0&&error!=1))return -8;
    HANDLE handle=GetStdHandle(error?STD_ERROR_HANDLE:STD_OUTPUT_HANDLE);
    while(length){DWORD done;if(!WriteFile(handle,source,(DWORD)length,&done,NULL)||!done)return -8;source+=done;length-=done;}return 0;
}
int64_t sengoo_pa_cli_input_identity(int64_t value,int64_t field){
    if(!value||field<0||field>3)return 0;ps_input *input=(ps_input *)(uintptr_t)value;
    FILE_ID_INFO identity;LARGE_INTEGER size;
    if(!GetFileInformationByHandleEx(input->file,FileIdInfo,&identity,sizeof(identity))||!GetFileSizeEx(input->file,&size))return 0;
    if(field==0)return (int64_t)identity.VolumeSerialNumber;
    if(field==3)return size.QuadPart;
    uint64_t result=0;int begin=field==1?0:8;for(int i=0;i<8;i++)result|=(uint64_t)identity.FileId.Identifier[begin+i]<<(8*i);return (int64_t)result;
}
