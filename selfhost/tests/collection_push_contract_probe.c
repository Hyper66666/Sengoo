#include "collection_os_probe.h"
#include "runtime_shared.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <windows.h>
extern long long sengoo_ffi_buffer_new(long long);
extern long long sengoo_ffi_buffer_free(long long);
extern long long sengoo_buffer_live_handle_count(void);
typedef struct {int64_t handle;int64_t value;} Owned;
static int created,dropped,moved;
static size_t object_bytes=sizeof(Owned);
static void drop_owned(void *input){Owned *p=input;if(!p->handle||sengoo_ffi_buffer_free(p->handle))__builtin_trap();++dropped;memset(input,0,object_bytes);}
static void move_owned(void *to,void *from){if(((uintptr_t)to&7)||((uintptr_t)from&7))__builtin_trap();memcpy(to,from,object_bytes);memset(from,0,object_bytes);++moved;}
static Owned make(int64_t value){Owned x={sengoo_ffi_buffer_new(64),value};if(!x.handle)__builtin_trap();++created;return x;}
static int ordinary(void){
 long long buffers=sengoo_buffer_live_handle_count(),opaque=sengoo_opaque_live_handle_count();
 for(int i=0;i<1000;++i){
  long long vector=sengoo_raw_vec_new_parts(sizeof(Owned),8,(void*)move_owned,(void*)drop_owned);if(!vector)return 70;
  Owned a=make(40);int d=dropped;if(sengoo_raw_vec_push(vector,&a)||a.handle||dropped!=d||sengoo_raw_vec_len(vector)!=1)return 71;
  Owned bad=make(2),saved=bad;if(sengoo_raw_vec_push(0,&bad)!=SENGOO_STATUS_INVALID_ARGUMENT||memcmp(&bad,&saved,sizeof bad)||dropped!=d)return 72;drop_owned(&bad);
  Owned negative=make(3);saved=negative;if(sengoo_raw_vec_push(-3,&negative)!=SENGOO_STATUS_INVALID_ARGUMENT||memcmp(&negative,&saved,sizeof negative))return 73;drop_owned(&negative);
  if(((Owned*)sengoo_raw_vec_get(vector,0))->value!=40||sengoo_raw_vec_free(vector)||created!=dropped)return 74;
  if(sengoo_buffer_live_handle_count()!=buffers||sengoo_opaque_live_handle_count()!=opaque)return 75;
 }
 printf("push_status|success=0_consumed_move|invalid=2_unconsumed|created=%d|dropped=%d|moved=%d\n",created,dropped,moved);return 42;
}
static int oom(void){
 object_bytes=1024*1024;void *value=VirtualAlloc(NULL,object_bytes,MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);if(!value)return 76;
 HANDLE job=CreateJobObjectW(NULL,NULL);if(!job)return 77;JOBOBJECT_EXTENDED_LIMIT_INFORMATION policy;memset(&policy,0,sizeof policy);policy.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_PROCESS_MEMORY;policy.ProcessMemoryLimit=48*1024*1024;
 if(!SetInformationJobObject(job,JobObjectExtendedLimitInformation,&policy,sizeof policy)||!AssignProcessToJobObject(job,GetCurrentProcess()))return 78;
 long long buffers=sengoo_buffer_live_handle_count(),opaque=sengoo_opaque_live_handle_count();long long vector=sengoo_raw_vec_new_parts(object_bytes,8,(void*)move_owned,(void*)drop_owned);if(!vector)return 79;int observed=0;
 for(int i=0;i<128;++i){memset(value,0,object_bytes);*(Owned*)value=make(100+i);int before=dropped;long long count=sengoo_raw_vec_len(vector);long long status=sengoo_raw_vec_push(vector,value);
  if(status==SENGOO_STATUS_OUT_OF_MEMORY){if(dropped!=before+1||((Owned*)value)->handle||sengoo_raw_vec_len(vector)!=count)return 80;for(long long at=0;at<count;++at){Owned *kept=sengoo_raw_vec_get(vector,at);if(!kept||!kept->handle||kept->value!=100+at)return 81;}observed=1;break;}
  if(status||((Owned*)value)->handle||dropped!=before)return 82;
 }
 if(!observed||sengoo_raw_vec_free(vector)||created!=dropped||sengoo_buffer_live_handle_count()!=buffers||sengoo_opaque_live_handle_count()!=opaque)return 83;
 printf("push_oom|status=14_consumed_drop|owner_preserved=1|source_cleared=1|created=%d|dropped=%d|moved=%d\n",created,dropped,moved);VirtualFree(value,0,MEM_RELEASE);CloseHandle(job);return 42;
}
int main(int argc,char **argv){if(argc>1&&strcmp(argv[1],"--os-providers")==0)return sh_collection_os_modules();return argc>1?oom():ordinary();}
