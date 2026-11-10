#include <stdint.h>
#include <string.h>

/* Test element callbacks and typed scalar observations only. */
static void fixture_move(void *target,void *source){memcpy(target,source,8);memset(source,0,8);}
static void fixture_drop(void *value){memset(value,0,8);}
int64_t sengoo_collections_fixture_move(void){return (int64_t)(intptr_t)fixture_move;}
int64_t sengoo_collections_fixture_drop(void){return (int64_t)(intptr_t)fixture_drop;}
void sengoo_collections_fixture_put(uint8_t *value,int64_t number){memcpy(value,&number,8);}
int64_t sengoo_collections_fixture_get(const uint8_t *value){int64_t number;memcpy(&number,value,8);return number;}

#ifdef SENGOO_COLLECTIONS_RESOURCE_PROBE
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <psapi.h>
#include <stdio.h>
#include <stdlib.h>
extern int64_t sengoo_sg_collections_context_init(uint8_t*,int64_t,int64_t);
extern int64_t sengoo_sg_collections_context_dispose(uint8_t*,int64_t);
extern int64_t sengoo_sg_collections_vec_new(uint8_t*,int64_t,int64_t,int64_t,int64_t,int64_t);
extern int64_t sengoo_sg_collections_vec_push(uint8_t*,int64_t,int64_t,uint8_t*);
static int64_t allocations,freed,attempts,live_bytes,peak_bytes,drop_count,live_tokens,fail_next;
static uint8_t *blocks[16];static int64_t sizes[16];
static void require(int good,int code){if(!good)exit(code);}
uint8_t *sengoo_collections_allocate(int64_t size,int64_t align){
 ++attempts;if(fail_next){fail_next=0;return NULL;}
 uint8_t *p=VirtualAlloc(NULL,(SIZE_T)size,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);require(p!=NULL&&((uintptr_t)p%(uintptr_t)align)==0,191);
 for(int i=0;i<16;i++)if(!blocks[i]){blocks[i]=p;sizes[i]=size;++allocations;live_bytes+=size;if(live_bytes>peak_bytes)peak_bytes=live_bytes;return p;}
 exit(192);
}
void sengoo_collections_release(uint8_t *p){
 if(!p)return;for(int i=0;i<16;i++)if(blocks[i]==p){require(VirtualFree(p,0,MEM_RELEASE)!=0,193);MEMORY_BASIC_INFORMATION info;require(VirtualQuery(p,&info,sizeof(info))==sizeof(info)&&info.State==MEM_FREE,194);blocks[i]=NULL;live_bytes-=sizes[i];++freed;return;}exit(195);
}
static void owning_move(void *to,void *from){memcpy(to,from,16);memset(from,0,16);}
static void owning_drop(void *value){void *p;memcpy(&p,value,8);require(p!=NULL,196);require(HeapFree(GetProcessHeap(),0,p)!=0,197);--live_tokens;++drop_count;memset(value,0,16);}
static void owning_value(uint8_t *out,int64_t value){void *p=HeapAlloc(GetProcessHeap(),0,16);require(p!=NULL,198);memcpy(p,&value,8);memcpy(out,&p,8);memcpy(out+8,&value,8);++live_tokens;}
int main(void){
 uint8_t *state=VirtualAlloc(NULL,12352,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);require(state!=NULL,199);
 for(int round=0;round<100;round++){
  require(!live_bytes&&!live_tokens&&allocations==freed,200);require(sengoo_sg_collections_context_init(state,12352,round+1)==0,201);
  int64_t handle=sengoo_sg_collections_vec_new(state,12352,16,16,(int64_t)(intptr_t)owning_move,(int64_t)(intptr_t)owning_drop);require(handle>0,202);
  __declspec(align(16)) uint8_t value[16];owning_value(value,42);fail_next=1;require(sengoo_sg_collections_vec_push(state,12352,handle,value)==14&&!live_tokens,203);
  for(int item=0;item<200;item++){owning_value(value,item);require(sengoo_sg_collections_vec_push(state,12352,handle,value)==0,204);uint8_t zero[16]={0};require(!memcmp(value,zero,16),205);}
  require(live_tokens==200,206);require(sengoo_sg_collections_context_dispose(state,12352)==0,207);
  require(!live_tokens&&!live_bytes&&allocations==freed&&drop_count==(round+1)*201,208);
  for(int byte=0;byte<12352;byte++)require(!state[byte],209);
 }
 require(VirtualFree(state,0,MEM_RELEASE)!=0,210);MEMORY_BASIC_INFORMATION info;require(VirtualQuery(state,&info,sizeof(info))==sizeof(info)&&info.State==MEM_FREE,211);
 PROCESS_MEMORY_COUNTERS counters;require(GetProcessMemoryInfo(GetCurrentProcess(),&counters,sizeof(counters))!=0,212);
 require(allocations==700&&freed==700&&attempts==800&&drop_count==20100&&peak_bytes==6144,213);
 printf("{\"scopes\":100,\"allocations\":700,\"freed\":700,\"attempts\":800,\"live_bytes\":0,\"payload_peak_bytes\":6144,\"owning_drops\":20100,\"state_bytes\":12352,\"disposed_zero\":true,\"virtual_free_verified\":true,\"working_set_peak_bytes\":%llu,\"rss_qualified\":false}\n",(unsigned long long)counters.PeakWorkingSetSize);return 42;
}
#endif
