#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

extern int64_t sengoo_sg_process_registry_new(int64_t,uint8_t*),sengoo_sg_process_registry_live(void);
extern uint8_t *sengoo_sg_process_registry_get(int64_t,int64_t),*sengoo_sg_process_registry_take(int64_t,int64_t);
extern int64_t sengoo_sg_process_vec_new(int64_t,int64_t,int64_t,int64_t),sengoo_sg_process_vec_push(int64_t,uint8_t*),sengoo_sg_process_vec_len(int64_t),sengoo_sg_process_vec_free(int64_t);
extern uint8_t *sengoo_sg_process_vec_get(int64_t,int64_t);
extern int64_t sengoo_sg_process_words_new(int64_t),sengoo_sg_process_words_set(int64_t,int64_t,int64_t),sengoo_sg_process_words_get(int64_t,int64_t,int64_t),sengoo_sg_process_words_free(int64_t);
extern int64_t sengoo_sg_collections_context_init(uint8_t*,int64_t,int64_t),sengoo_sg_collections_context_dispose(uint8_t*,int64_t);
extern int64_t sengoo_sg_collections_vec_new(uint8_t*,int64_t,int64_t,int64_t,int64_t,int64_t),sengoo_sg_collections_vec_free(uint8_t*,int64_t,int64_t);
extern uint8_t *sengoo_sg_collections_vec_get(uint8_t*,int64_t,int64_t,int64_t);
extern int64_t sengoo_opaque_handle_new(void*);extern void *sengoo_opaque_handle_get(int64_t),*sengoo_opaque_handle_take(int64_t);
extern uint8_t *sengoo_process_registry_address(void),*sengoo_process_vec_context_address(void);
static void *blocks[512];static int64_t sizes[512],live_bytes,peak_bytes,created,freed,attempts,fail_at;
static int64_t tokens,drops,reentered,completed[12],completed_count;
static void require(int good,int code){if(!good){fprintf(stderr,"process-check=%d\n",code);exit(code);}}
static void mark(int id){require(completed_count<12,70);completed[completed_count++]=id;}
uint8_t *sengoo_collections_allocate(int64_t bytes,int64_t align){
 ++attempts;if(fail_at&&attempts==fail_at)return NULL;void *p=VirtualAlloc(NULL,(SIZE_T)bytes,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);require(p&&((uintptr_t)p%(uintptr_t)align)==0,71);
 for(int i=0;i<512;i++)if(!blocks[i]){blocks[i]=p;sizes[i]=bytes;++created;live_bytes+=bytes;if(live_bytes>peak_bytes)peak_bytes=live_bytes;return p;}exit(72);
}
void sengoo_collections_release(uint8_t *p){if(!p)return;for(int i=0;i<512;i++)if(blocks[i]==p){require(VirtualFree(p,0,MEM_RELEASE)!=0,73);MEMORY_BASIC_INFORMATION info;require(VirtualQuery(p,&info,sizeof(info))==sizeof(info)&&info.State==MEM_FREE,74);blocks[i]=NULL;live_bytes-=sizes[i];++freed;return;}exit(75);}
static void move_value(void *to,void *from){memcpy(to,from,8);memset(from,0,8);}
static void drop_value(void *value){
 void *token;memcpy(&token,value,8);require(token!=NULL,76);require(HeapFree(GetProcessHeap(),0,token)!=0,77);tokens--;drops++;memset(value,0,8);
 int64_t words=sengoo_sg_process_words_new(2);require(words<0&&!sengoo_sg_process_words_set(words,0,19)&&!sengoo_sg_process_words_set(words,1,23),78);require(sengoo_sg_process_words_get(words,0,-1)+sengoo_sg_process_words_get(words,1,-1)==42,79);require(!sengoo_sg_process_words_free(words),80);++reentered;
}
static void value_new(uint8_t *out){void *p=HeapAlloc(GetProcessHeap(),0,8);require(p!=NULL,81);int64_t number=42;memcpy(p,&number,8);memcpy(out,&p,8);tokens++;}
static void rounds(void){
 int64_t stale=0;for(int round=0;round<100;round++){
  require(!sengoo_sg_process_registry_live()&&!live_bytes&&!tokens,82);
  int64_t vec=sengoo_sg_process_vec_new(8,8,(int64_t)(intptr_t)move_value,(int64_t)(intptr_t)drop_value),words=sengoo_sg_process_words_new(3);require(vec<0&&words<0&&vec!=words&&vec!=stale,83);
  require(sengoo_sg_process_vec_len(words)==-3&&sengoo_sg_process_words_get(vec,0,-17)==-17&&sengoo_sg_process_words_free(vec)==3,84);
  for(int i=0;i<12;i++){uint8_t value[8];value_new(value);require(!sengoo_sg_process_vec_push(vec,value),85);void *zero=NULL;require(!memcmp(value,&zero,8),86);}
  require(sengoo_sg_process_vec_len(vec)==12,87);for(int i=0;i<12;i++){void *token;memcpy(&token,sengoo_sg_process_vec_get(vec,i),8);require(*(int64_t*)token==42,88);}
  require(!sengoo_sg_process_words_set(words,0,40)&&!sengoo_sg_process_words_set(words,2,2)&&sengoo_sg_process_words_get(words,0,-1)+sengoo_sg_process_words_get(words,2,-1)==42,89);
  require(!sengoo_sg_process_vec_free(vec)&&!sengoo_sg_process_words_free(words),90);
  require(sengoo_sg_process_vec_free(vec)==3&&sengoo_sg_process_vec_len(vec)==-3&&!sengoo_sg_process_registry_live()&&!live_bytes&&!tokens,91);stale=vec;
 }
 require(drops==1200&&reentered==1200,92);mark(0);
}
static void failures(void){
 int64_t count=created;fail_at=attempts+1;require(!sengoo_sg_process_words_new(3)&&created==count&&!sengoo_sg_process_registry_live(),93);fail_at=0;mark(1);
 fail_at=attempts+1;require(!sengoo_sg_process_vec_new(8,8,(int64_t)(intptr_t)move_value,(int64_t)(intptr_t)drop_value)&&!live_bytes,94);fail_at=0;mark(2);
 int64_t vec=sengoo_sg_process_vec_new(8,8,(int64_t)(intptr_t)move_value,(int64_t)(intptr_t)drop_value);uint8_t value[8];value_new(value);fail_at=attempts+1;require(sengoo_sg_process_vec_push(vec,value)==14&&!tokens,95);fail_at=0;require(!sengoo_sg_process_vec_free(vec)&&!live_bytes,96);mark(3);
 int64_t all[128];for(int i=0;i<128;i++){all[i]=sengoo_sg_process_words_new(1);require(all[i]<0,97);}int64_t before=live_bytes;
 require(!sengoo_sg_process_words_new(2)&&!sengoo_sg_process_vec_new(8,8,(int64_t)(intptr_t)move_value,(int64_t)(intptr_t)drop_value)&&live_bytes==before,98);
 for(int i=0;i<128;i++)require(!sengoo_sg_process_words_free(all[i]),99);require(!live_bytes&&!sengoo_sg_process_registry_live(),100);mark(4);
}
static void mixed(void){
 uint8_t context[12352];require(!sengoo_sg_collections_context_init(context,sizeof(context),1),101);int64_t positive=sengoo_sg_collections_vec_new(context,sizeof(context),8,8,(int64_t)(intptr_t)move_value,(int64_t)(intptr_t)drop_value),opaque=sengoo_opaque_handle_new(context),negative=sengoo_sg_process_words_new(1);require(positive>0&&opaque>0&&negative<0,102);
 require(!sengoo_sg_process_registry_get(1,positive)&&!sengoo_sg_process_registry_get(2,opaque)&&!sengoo_opaque_handle_get(negative)&&!sengoo_sg_collections_vec_get(context,sizeof(context),negative,0),103);
 require(!sengoo_sg_collections_vec_free(context,sizeof(context),positive)&&sengoo_opaque_handle_take(opaque)==context&&!sengoo_sg_process_words_free(negative)&&!sengoo_sg_collections_context_dispose(context,sizeof(context)),104);mark(5);
 int64_t invalid[]={0,-1,-3,INT64_MIN,INT64_MAX,-4611686018427387904LL,-4611686018427388160LL,-4611686022722355329LL};for(size_t i=0;i<sizeof(invalid)/sizeof(invalid[0]);i++)require(!sengoo_sg_process_registry_get(1,invalid[i])&&!sengoo_sg_process_registry_take(2,invalid[i]),105);mark(6);
}
static DWORD WINAPI metadata_thread(void *argument){(void)argument;int64_t *payload=VirtualAlloc(NULL,16,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);require(payload!=NULL,113);payload[0]=1397184344;payload[1]=0;for(int i=0;i<2000;i++){int64_t h=sengoo_sg_process_registry_new(2,(uint8_t*)payload);require(h<0&&sengoo_sg_process_registry_get(2,h)==(uint8_t*)payload&&!sengoo_sg_process_registry_get(1,h),106);require(sengoo_sg_process_registry_take(2,h)==(uint8_t*)payload,107);}require(VirtualFree(payload,0,MEM_RELEASE),114);return 0;}
static void threads(void){HANDLE work[4];for(int i=0;i<4;i++){work[i]=CreateThread(NULL,0,metadata_thread,(void*)(intptr_t)i,0,NULL);require(work[i]!=NULL,108);}require(WaitForMultipleObjects(4,work,TRUE,60000)==WAIT_OBJECT_0,109);for(int i=0;i<4;i++){DWORD exit_code;require(GetExitCodeThread(work[i],&exit_code)&&exit_code==0&&CloseHandle(work[i]),110);}require(!sengoo_sg_process_registry_live(),111);mark(7);}
/* Controlled final test-only metadata setup: no production API resets generations. */
static void retirement(void){
 int64_t *root=(int64_t*)sengoo_process_registry_address();require(root[2]==0,115);
 for(int i=0;i<128;i++){require(root[8+i*4+1]==0&&root[8+i*4+2]==0,116);root[8+i*4]=16777215;}
 root[8+127*4]=16777214;int64_t payload[2]={1397184344,0};
 int64_t token=sengoo_sg_process_registry_new(2,(uint8_t*)payload);
 const int64_t last_token=-((INT64_C(1)<<62)+INT64_C(16777215)*256+128);
 require(token==last_token&&sengoo_sg_process_registry_get(2,token)==(uint8_t*)payload,117);
 require(!sengoo_sg_process_registry_get(2,last_token-1)&&!sengoo_sg_process_registry_take(2,last_token-1),120);
 require(sengoo_sg_process_registry_take(2,token)==(uint8_t*)payload&&!sengoo_sg_process_registry_get(2,token),118);
 require(!sengoo_sg_process_registry_new(2,(uint8_t*)payload)&&root[8+127*4]==16777215&&!root[2],119);mark(9);
}
int main(void){rounds();failures();mixed();threads();require(created==freed&&!live_bytes&&!tokens&&drops==1201&&reentered==1201,112);mark(8);retirement();
 printf("{\"rounds\":100,\"owning_drops\":1201,\"reentrant_callbacks\":1201,\"metadata_threads\":4,\"metadata_iterations\":8000,\"state_bytes\":16512,\"payload_live_bytes\":0,\"created\":%lld,\"freed\":%lld,\"payload_peak_bytes\":%lld,\"virtual_free_verified\":true,\"mixed_profiles_rejected\":true,\"payload_concurrency_qualified\":false,\"completed\":[",created,freed,peak_bytes);for(int i=0;i<completed_count;i++)printf(i?",%lld":"%lld",completed[i]);puts("]}");return 42;}
