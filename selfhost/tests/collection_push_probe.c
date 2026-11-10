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
typedef struct {int64_t handle;int64_t value;} Owner;
typedef struct {Owner owner;unsigned char padding[1048560];} Large;
extern _Bool sg_push_i64(int64_t,int64_t);
extern _Bool sg_push_bool(int64_t,_Bool);
extern _Bool sg_push_owner(int64_t,Owner);
extern _Bool sg_push_large(int64_t,Large);
static int created,dropped,moved;static size_t width=16;
void optional_owner_release(Owner *p){if(!p->handle||sengoo_ffi_buffer_free(p->handle))__builtin_trap();++dropped;memset(p,0,16);}
void optional_owner_release_large(Large *p){if(!p->owner.handle||sengoo_ffi_buffer_free(p->owner.handle))__builtin_trap();++dropped;memset(p,0,sizeof *p);}
static void move_owner(void *to,void *from){memcpy(to,from,width);memset(from,0,width);++moved;}
static void drop_owner(void *from){if(width==16)optional_owner_release(from);else optional_owner_release_large(from);}
static Owner make(int64_t value){Owner p={sengoo_ffi_buffer_new(64),value};if(!p.handle)__builtin_trap();++created;return p;}
static void move_i64(void *to,void *from){memcpy(to,from,8);memset(from,0,8);}
static void move_bool(void *to,void *from){memcpy(to,from,1);memset(from,0,1);}
static void scalar_drop(void *from){(void)from;}
static int ordinary(void){long long buffers=sengoo_buffer_live_handle_count(),opaque=sengoo_opaque_live_handle_count();
 for(int round=0;round<1000;++round){long long vi=sengoo_raw_vec_new_parts(8,8,(void*)move_i64,(void*)scalar_drop),vb=sengoo_raw_vec_new_parts(1,1,(void*)move_bool,(void*)scalar_drop),vo=sengoo_raw_vec_new_parts(16,8,(void*)move_owner,(void*)drop_owner);if(!vi||!vb||!vo)return 70;
  if(!sg_push_i64(vi,42)||*(int64_t*)sengoo_raw_vec_get(vi,0)!=42||!sg_push_bool(vb,1)||*(_Bool*)sengoo_raw_vec_get(vb,0)!=1)return 71;
  Owner actual=make(40);if(!sg_push_owner(vo,actual))return 72;memset(&actual,0,sizeof actual);if(((Owner*)sengoo_raw_vec_get(vo,0))->value!=40)return 73;
  Owner invalid=make(2);int before=dropped;if(sg_push_owner(0,invalid)||dropped!=before+1)return 74;memset(&invalid,0,sizeof invalid);
  Owner negative=make(3);before=dropped;if(sg_push_owner(-3,negative)||dropped!=before+1)return 75;memset(&negative,0,sizeof negative);
  if(sengoo_raw_vec_free(vi)||sengoo_raw_vec_free(vb)||sengoo_raw_vec_free(vo)||created!=dropped||sengoo_buffer_live_handle_count()!=buffers||sengoo_opaque_live_handle_count()!=opaque)return 76;
 }
 printf("SG_push|created=%d|dropped=%d|moved=%d|invalid_typed_drop_once=1\n",created,dropped,moved);return 42;
}
static int oom(void){width=sizeof(Large);Large *input=VirtualAlloc(NULL,width,MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);if(!input)return 77;
 HANDLE job=CreateJobObjectW(NULL,NULL);JOBOBJECT_EXTENDED_LIMIT_INFORMATION policy;memset(&policy,0,sizeof policy);policy.BasicLimitInformation.LimitFlags=JOB_OBJECT_LIMIT_PROCESS_MEMORY;policy.ProcessMemoryLimit=48*1024*1024;if(!job||!SetInformationJobObject(job,JobObjectExtendedLimitInformation,&policy,sizeof policy)||!AssignProcessToJobObject(job,GetCurrentProcess()))return 78;
 long long buffers=sengoo_buffer_live_handle_count(),opaque=sengoo_opaque_live_handle_count(),vector=sengoo_raw_vec_new_parts(width,8,(void*)move_owner,(void*)drop_owner);if(!vector)return 79;int failure=0;
 for(int round=0;round<128;++round){memset(input,0,width);input->owner=make(100+round);int before=dropped;long long count=sengoo_raw_vec_len(vector);_Bool okay=sg_push_large(vector,*input);memset(input,0,width);
  if(!okay){if(dropped!=before+1||sengoo_raw_vec_len(vector)!=count)return 80;for(long long at=0;at<count;++at){Large *stored=sengoo_raw_vec_get(vector,at);if(!stored||!stored->owner.handle||stored->owner.value!=100+at)return 81;}failure=1;break;}
  if(dropped!=before)return 82;
 }
 if(!failure||sengoo_raw_vec_free(vector)||created!=dropped||sengoo_buffer_live_handle_count()!=buffers||sengoo_opaque_live_handle_count()!=opaque)return 83;
 printf("SG_push_OOM|created=%d|dropped=%d|moved=%d|no_second_drop=1\n",created,dropped,moved);VirtualFree(input,0,MEM_RELEASE);CloseHandle(job);return 42;
}
int main(int argc,char **argv){if(argc>1&&strcmp(argv[1],"--os-providers")==0)return sh_collection_os_modules();return argc>1?oom():ordinary();}
