/* Same handle resource implementation, with additional byte read/identity
 * operations. Archive policy, manifest, hashing and markers stay in SG. */
#include "package_stream_os.c"
typedef struct {
    const uint8_t *bytes[4];
    int64_t lengths[4];
    int64_t allow_yanked;
} pa_arguments;
uint8_t *sengoo_pa_null(void){return NULL;}

uint8_t *sengoo_pa_arguments_new(const uint8_t *name,int64_t name_size,
    const uint8_t *version,int64_t version_size,const uint8_t *checksum,int64_t checksum_size,
    const uint8_t *metadata,int64_t metadata_size,int64_t allow_yanked){
    pa_arguments *value=VirtualAlloc(NULL,sizeof(*value),MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
    if(!value)return NULL;
    value->bytes[0]=name;value->lengths[0]=name_size;
    value->bytes[1]=version;value->lengths[1]=version_size;
    value->bytes[2]=checksum;value->lengths[2]=checksum_size;
    value->bytes[3]=metadata;value->lengths[3]=metadata_size;
    value->allow_yanked=allow_yanked;return (uint8_t *)value;
}
void sengoo_pa_arguments_free(uint8_t *value){if(value)VirtualFree(value,0,MEM_RELEASE);}
const uint8_t *sengoo_pa_argument(uint8_t *value,int64_t index){
    return value&&index>=0&&index<4?((pa_arguments *)value)->bytes[index]:NULL;
}
int64_t sengoo_pa_argument_length(uint8_t *value,int64_t index){
    return value&&index>=0&&index<4?((pa_arguments *)value)->lengths[index]:-1;
}
int64_t sengoo_pa_allow_yanked(uint8_t *value){return value?((pa_arguments *)value)->allow_yanked:-1;}
static ps_node *pa_file(int64_t value,int64_t id){
    ps_session *session=(ps_session *)(uintptr_t)value;
    if(!session||session->magic!=PS_MAGIC||session->published||id<1||id>=session->count||
        session->nodes[id].directory||!session->nodes[id].handle)return NULL;
    return &session->nodes[id];
}
int64_t sengoo_pa_file_size(int64_t session,int64_t id){
    ps_node *node=pa_file(session,id);LARGE_INTEGER size;
    if(!node||!GetFileSizeEx(node->handle,&size)||size.QuadPart<0)return -8;return size.QuadPart;
}
int64_t sengoo_pa_file_identity(int64_t session,int64_t id){
    ps_node *node=pa_file(session,id);FILE_ID_INFO identity;FILE_ATTRIBUTE_TAG_INFO tag;
    if(!node||!GetFileInformationByHandleEx(node->handle,FileIdInfo,&identity,sizeof(identity))||
       !GetFileInformationByHandleEx(node->handle,FileAttributeTagInfo,&tag,sizeof(tag))||
       (tag.FileAttributes&(FILE_ATTRIBUTE_REPARSE_POINT|FILE_ATTRIBUTE_DIRECTORY)))return -8;
    return ps_same_id(&identity,&node->identity)?1:-8;
}
int64_t sengoo_pa_file_read(int64_t session,int64_t id,int64_t offset,uint8_t *target,int64_t capacity){
    if(offset<0||capacity<0||capacity>32768)return -8;
    ps_node *node=pa_file(session,id);if(!node)return -8;if(!capacity)return 0;
    LARGE_INTEGER at;at.QuadPart=offset;DWORD read;
    if(!SetFilePointerEx(node->handle,at,NULL,FILE_BEGIN)||!ReadFile(node->handle,target,(DWORD)capacity,&read,NULL))return -8;
#ifdef SENGOO_PACKAGE_INSTRUMENTED
    extern void sengoo_pa_observe_read(uint8_t *,int64_t);
    sengoo_pa_observe_read(target,read);
#endif
    return read;
}
int64_t sengoo_pa_open_attributes(int64_t value,int64_t parent,const uint8_t *name,int64_t length){
    ps_session *session=(ps_session *)(uintptr_t)value;WCHAR wide[256];
    if(!session||session->magic!=PS_MAGIC||session->published||parent<0||parent>=session->count||
       !session->nodes[parent].directory||!session->nodes[parent].handle||length<1||length>1020)return 0;
    int count=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,(const char *)name,(int)length,wide,256);if(count<1||count>255)return 0;
    UNICODE_STRING text={(USHORT)(count*2),(USHORT)(count*2),wide};
    OBJECT_ATTRIBUTES attrs={sizeof(attrs),session->nodes[parent].handle,&text,OBJ_CASE_INSENSITIVE|OBJ_DONT_REPARSE,NULL,NULL};
    IO_STATUS_BLOCK ios;HANDLE handle=NULL;ps_nt_create open=(ps_nt_create)GetProcAddress(GetModuleHandleW(L"ntdll.dll"),"NtCreateFile");
    if(!open||open(&handle,FILE_READ_ATTRIBUTES|SYNCHRONIZE,&attrs,&ios,NULL,0,
        FILE_SHARE_READ|FILE_SHARE_WRITE|FILE_SHARE_DELETE,1,0x00200000|0x20,NULL,0)<0)return 0;
    FILE_ATTRIBUTE_TAG_INFO tag;if(!GetFileInformationByHandleEx(handle,FileAttributeTagInfo,&tag,sizeof(tag))||
       (tag.FileAttributes&FILE_ATTRIBUTE_REPARSE_POINT)){CloseHandle(handle);return 0;}
    return (int64_t)(uintptr_t)handle;
}
int64_t sengoo_pa_attributes_match(int64_t value,int64_t id,int64_t attribute_handle){
    ps_session *session=(ps_session *)(uintptr_t)value;FILE_ID_INFO identity;
    if(!session||session->magic!=PS_MAGIC||id<1||id>=session->count||!attribute_handle||
       !GetFileInformationByHandleEx((HANDLE)(uintptr_t)attribute_handle,FileIdInfo,&identity,sizeof(identity)))return 0;
    return ps_same_id(&identity,&session->nodes[id].identity);
}
void sengoo_pa_close_attributes(int64_t handle){if(handle)CloseHandle((HANDLE)(uintptr_t)handle);}
