#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <winternl.h>
#include <stdint.h>
#include <stddef.h>

/* Memory and OS resource operations only. SG supplies admitted components and
 * owns framing, path policy and transaction sequencing. The unsafe caller keeps
 * component byte extents stable until session_close. */
#ifndef OBJ_DONT_REPARSE
#define OBJ_DONT_REPARSE 0x1000L
#endif
#define PS_MAX_NODES 32768
#define PS_MAGIC UINT64_C(0x534750534f533031)
typedef NTSTATUS (NTAPI *ps_nt_create)(PHANDLE,ACCESS_MASK,POBJECT_ATTRIBUTES,
    PIO_STATUS_BLOCK,PLARGE_INTEGER,ULONG,ULONG,ULONG,ULONG,PVOID,ULONG);
typedef NTSTATUS (NTAPI *ps_nt_set)(HANDLE,PIO_STATUS_BLOCK,PVOID,ULONG,FILE_INFORMATION_CLASS);
typedef struct {
    HANDLE handle;
    FILE_ID_INFO identity;
    const uint8_t *name;
    int64_t length,parent;
    int directory;
} ps_node;
typedef struct {
    uint64_t magic;
    HANDLE parent;
    int64_t count,published;
    ps_node nodes[PS_MAX_NODES];
} ps_session;
#ifdef SENGOO_PACKAGE_INSTRUMENTED
extern void sengoo_ps_observe(int phase,void *session,HANDLE *handle);
extern int sengoo_ps_fail_session_alloc(void);
#define PS_OBSERVE(phase,session,handle) sengoo_ps_observe(phase,session,handle)
#else
#define PS_OBSERVE(phase,session,handle) ((void)0)
#endif

uint8_t *sengoo_ps_mut_offset(uint8_t *source,int64_t offset){return offset?source+offset:source;}
const uint8_t *sengoo_ps_const_offset(uint8_t *source,int64_t offset){return offset?source+offset:source;}
const uint8_t *sengoo_ps_input_offset(const uint8_t *source,int64_t offset){return offset?source+offset:source;}
int64_t sengoo_ps_component_equal(const uint8_t *a,int64_t n,const uint8_t *b,int64_t m){
    WCHAR left[256],right[256];
    if(n<1||m<1||n>1020||m>1020)return 0;
    int x=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,(const char *)a,(int)n,left,256);
    int y=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,(const char *)b,(int)m,right,256);
    return x>0&&y>0&&CompareStringOrdinal(left,x,right,y,TRUE)==CSTR_EQUAL;
}
static HANDLE ps_open(HANDLE parent,const uint8_t *name,int64_t length,int directory,int create){
    WCHAR wide[256];
    if(length<1||length>1020)return INVALID_HANDLE_VALUE;
    int count=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,(const char *)name,(int)length,wide,256);
    if(count<1||count>255)return INVALID_HANDLE_VALUE;
    UNICODE_STRING text={(USHORT)(count*2),(USHORT)(count*2),wide};
    OBJECT_ATTRIBUTES attrs={sizeof(attrs),parent,&text,OBJ_CASE_INSENSITIVE|OBJ_DONT_REPARSE,NULL,NULL};
    IO_STATUS_BLOCK ios;HANDLE result=NULL;
    ps_nt_create open=(ps_nt_create)GetProcAddress(GetModuleHandleW(L"ntdll.dll"),"NtCreateFile");
    if(!open)return INVALID_HANDLE_VALUE;
    NTSTATUS status=open(&result,GENERIC_READ|GENERIC_WRITE|DELETE|SYNCHRONIZE,&attrs,&ios,NULL,
        FILE_ATTRIBUTE_NORMAL,0,create?2:1,0x00200000|0x20|(directory?1:0x40),NULL,0);
    if(status<0)return INVALID_HANDLE_VALUE;
    FILE_ATTRIBUTE_TAG_INFO tag;
    if(!GetFileInformationByHandleEx(result,FileAttributeTagInfo,&tag,sizeof(tag))||
       (tag.FileAttributes&FILE_ATTRIBUTE_REPARSE_POINT)){
        CloseHandle(result);return INVALID_HANDLE_VALUE;
    }
    return result;
}
int64_t sengoo_ps_session_new(int64_t parent,const uint8_t *name,int64_t length){
#ifdef SENGOO_PACKAGE_INSTRUMENTED
    if(sengoo_ps_fail_session_alloc())return 0;
#endif
    ps_session *session=VirtualAlloc(NULL,sizeof(*session),MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
    if(!session)return 0;
    HANDLE root=ps_open((HANDLE)(uintptr_t)parent,name,length,1,1);
    if(root==INVALID_HANDLE_VALUE){VirtualFree(session,0,MEM_RELEASE);return 0;}
    session->magic=PS_MAGIC;session->parent=(HANDLE)(uintptr_t)parent;session->count=1;
    session->nodes[0].handle=root;session->nodes[0].directory=1;
    PS_OBSERVE(0,session,&session->nodes[0].handle);return (int64_t)(uintptr_t)session;
}
int64_t sengoo_ps_create_node(int64_t value,int64_t parent,const uint8_t *name,int64_t length,int64_t kind){
    ps_session *session=(ps_session *)(uintptr_t)value;
    if(!session||session->magic!=PS_MAGIC||session->published||parent<0||parent>=session->count||
       !session->nodes[parent].directory||(kind!=0&&kind!=1)||session->count==PS_MAX_NODES)return -8;
    HANDLE handle=ps_open(session->nodes[parent].handle,name,length,(int)kind,1);
    if(handle==INVALID_HANDLE_VALUE)return -8;
    int64_t result=session->count;ps_node *node=&session->nodes[result];
    if(!GetFileInformationByHandleEx(handle,FileIdInfo,&node->identity,sizeof(node->identity))){
        FILE_DISPOSITION_INFO remove={TRUE};SetFileInformationByHandle(handle,FileDispositionInfo,&remove,sizeof(remove));CloseHandle(handle);return -8;
    }
    node->handle=handle;node->directory=(int)kind;node->parent=parent;node->name=name;node->length=length;session->count++;
    PS_OBSERVE(kind?5:1,session,&node->handle);return result;
}
int64_t sengoo_ps_write_node(int64_t value,int64_t node,const uint8_t *source,int64_t length){
    ps_session *session=(ps_session *)(uintptr_t)value;
    if(!session||session->magic!=PS_MAGIC||session->published||node<1||node>=session->count||
       session->nodes[node].directory||length<0||length>32768)return -8;
    while(length){DWORD written=0;if(!WriteFile(session->nodes[node].handle,source,(DWORD)length,&written,NULL)||!written)return -8;
        source+=written;length-=written;}
    PS_OBSERVE(6,session,&session->nodes[node].handle);
    return 0;
}
int64_t sengoo_ps_flush_node(int64_t value,int64_t node){
    ps_session *session=(ps_session *)(uintptr_t)value;
    if(!session||session->magic!=PS_MAGIC||node<1||node>=session->count)return -8;
    return session->nodes[node].directory||FlushFileBuffers(session->nodes[node].handle)?0:-8;
}
int64_t sengoo_ps_publish(int64_t value,const uint8_t *target,int64_t length){
    ps_session *session=(ps_session *)(uintptr_t)value;
    if(!session||session->magic!=PS_MAGIC||session->published||length<1||length>64)return -8;
    struct {BOOL replace;HANDLE root;DWORD bytes;WCHAR name[128];} info;
    int count=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,(const char *)target,(int)length,info.name,128);
    if(count<1)return -8;
    /* Flush is requested by SG. Release all child handles before root rename. */
    for(int64_t i=session->count;i>1;){--i;if(session->nodes[i].handle){
        PS_OBSERVE(7,session,&session->nodes[i].handle);
        if(!CloseHandle(session->nodes[i].handle))return -8;session->nodes[i].handle=NULL;}}
    PS_OBSERVE(2,session,&session->nodes[0].handle);
    info.replace=FALSE;info.root=session->parent;info.bytes=count*2;IO_STATUS_BLOCK ios;
    ps_nt_set set=(ps_nt_set)GetProcAddress(GetModuleHandleW(L"ntdll.dll"),"NtSetInformationFile");
    if(!set||set(session->nodes[0].handle,&ios,&info,
        (ULONG)(offsetof(FILE_RENAME_INFO,FileName)+info.bytes),(FILE_INFORMATION_CLASS)10)<0)return -8;
    session->published=1;return 0;
}
static int ps_same_id(const FILE_ID_INFO *a,const FILE_ID_INFO *b){
    if(a->VolumeSerialNumber!=b->VolumeSerialNumber)return 0;
    for(int i=0;i<16;i++)if(a->FileId.Identifier[i]!=b->FileId.Identifier[i])return 0;return 1;
}
int64_t sengoo_ps_session_close(int64_t value){
    ps_session *session=(ps_session *)(uintptr_t)value;
    if(!session||session->magic!=PS_MAGIC)return -8;int64_t result=0;
    if(!session->published){
        /* A failed rename may have closed child handles. Reopen only recorded
         * objects, without reparses, and require the original OS identity. */
        for(int64_t i=1;i<session->count;i++)if(!session->nodes[i].handle){
            ps_node *node=&session->nodes[i];HANDLE parent=session->nodes[node->parent].handle;
            if(!parent){result=-8;continue;}
            HANDLE handle=ps_open(parent,node->name,node->length,node->directory,0);
            if(handle==INVALID_HANDLE_VALUE){result=-8;continue;}FILE_ID_INFO identity;
            if(!GetFileInformationByHandleEx(handle,FileIdInfo,&identity,sizeof(identity))||!ps_same_id(&identity,&node->identity)){
                PS_OBSERVE(9,session,&handle);
                CloseHandle(handle);result=-8;continue;}
            PS_OBSERVE(8,session,&handle);
            node->handle=handle;
        }
    }
    for(int64_t i=session->count;i>0;){--i;HANDLE handle=session->nodes[i].handle;if(!handle)continue;
        if(!session->published){FILE_DISPOSITION_INFO info={TRUE};
            if(!SetFileInformationByHandle(handle,FileDispositionInfo,&info,sizeof(info)))result=-8;}
        if(!CloseHandle(handle))result=-8;
    }
    session->magic=0;if(!VirtualFree(session,0,MEM_RELEASE))result=-8;
    else PS_OBSERVE(3,session,NULL);return result;
}
