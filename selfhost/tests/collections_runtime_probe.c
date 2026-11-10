#include "runtime_shared.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <malloc.h>

extern long long sengoo_sg_collections_context_init(uint8_t*,long long,long long),sengoo_sg_collections_context_dispose(uint8_t*,long long),sengoo_sg_collections_vec_live(uint8_t*,long long);
extern long long sengoo_sg_collections_context_validate(uint8_t*,long long),sengoo_sg_collections_context_query(uint8_t*,long long,long long),sengoo_sg_collections_context_reset(uint8_t*,long long,long long);
extern long long sengoo_sg_collections_vec_new(uint8_t*,long long,long long,long long,long long,long long);
extern long long sengoo_sg_collections_vec_len(uint8_t*,long long,long long),sengoo_sg_collections_vec_push(uint8_t*,long long,long long,void*),sengoo_sg_collections_vec_set(uint8_t*,long long,long long,long long,void*);
extern long long sengoo_sg_collections_vec_insert(uint8_t*,long long,long long,long long,void*),sengoo_sg_collections_vec_pop(uint8_t*,long long,long long,void*),sengoo_sg_collections_vec_remove(uint8_t*,long long,long long,long long,void*);
extern long long sengoo_sg_collections_vec_clear(uint8_t*,long long,long long),sengoo_sg_collections_vec_free(uint8_t*,long long,long long);
extern void *sengoo_sg_collections_vec_get(uint8_t*,long long,long long,long long),*sengoo_sg_collections_vec_iter_next(uint8_t*,long long,long long);
extern long long sengoo_sg_collections_vec_iter_new(uint8_t*,long long,long long),sengoo_sg_collections_vec_iter_done(uint8_t*,long long,long long),sengoo_sg_collections_vec_iter_reset(uint8_t*,long long,long long),sengoo_sg_collections_vec_iter_index(uint8_t*,long long,long long),sengoo_sg_collections_vec_iter_free(uint8_t*,long long,long long);
extern long long sengoo_raw_vec_iter_done(long long),sengoo_raw_vec_iter_reset(long long),sengoo_raw_vec_iter_index(long long);
static uint8_t state[12352],other[12352],initialized[2][1544];
static long long moves[2],drops[2],sums[2],owned_live[2],metadata_accesses,opcount,allocations[2],frees[2],live_bytes[2],peak_bytes[2];
static void *alloc_ptr[2][128];static size_t alloc_size[2][128];static int fail_allocation[2];
static int mode; static size_t element_size;
static int completed[23],completed_count;
static void record(int id){if(id<0||id>=23||completed_count>=23)exit(185);for(int i=0;i<completed_count;i++)if(completed[i]==id)exit(186);completed[completed_count++]=id;}
static long long events[2][2048];static int event_count[2];
static void *reference_objects[512];static size_t reference_object_bytes[512];static int reference_object_count,reference_slot_fail,reference_iterator_leak_observed;
long long reference_opaque_handle_new(void*);
long long sengoo_opaque_handle_new(void *pointer){if(reference_slot_fail){reference_slot_fail=0;return 0;}return reference_opaque_handle_new(pointer);}
void *ref_calloc(size_t count,size_t size){void *p=calloc(count,size);if(p){for(int i=0;i<512;i++)if(!reference_objects[i]){reference_objects[i]=p;reference_object_bytes[i]=count*size;reference_object_count++;return p;}exit(89);}return p;}
void ref_object_free(void *p){if(!p)return;for(int i=0;i<512;i++)if(reference_objects[i]==p){reference_objects[i]=NULL;reference_object_count--;free(p);return;}free(p);}
static void check(int good,int code){if(!good){fprintf(stderr,"check=%d mode=%d op=%lld\n",code,mode,opcount);exit(code);}}
/* Verification-only byte bounds: not used to implement SG handle lookup. */
static int metadata(uint8_t *p,int64_t index,int write){check((p==state||p==other)&&index>=0&&index<1544,90);int at=p==other;check(write||initialized[at][index],147);if(write)initialized[at][index]=1;++metadata_accesses;return at;}
int64_t sengoo_collections_word(uint8_t *p,int64_t index){int64_t out;metadata(p,index,0);memcpy(&out,p+index*8,8);return out;}
void sengoo_collections_put(uint8_t *p,int64_t index,int64_t value){metadata(p,index,1);memcpy(p+index*8,&value,8);}
uint8_t *sengoo_collections_pointer(uint8_t *p,int64_t index){uint8_t *out;metadata(p,index,0);memcpy(&out,p+index*8,8);return out;}
void sengoo_collections_pointer_put(uint8_t *p,int64_t index,uint8_t *value){metadata(p,index,1);memcpy(p+index*8,&value,8);}
static void *allocate(int group,size_t size,size_t align){
 if(fail_allocation[group]){fail_allocation[group]=0;return NULL;}
 void *p=_aligned_malloc(size,align);check(p!=NULL,91);
 for(int i=0;i<128;i++)if(!alloc_ptr[group][i]){alloc_ptr[group][i]=p;alloc_size[group][i]=size;++allocations[group];live_bytes[group]+=(long long)size;if(live_bytes[group]>peak_bytes[group])peak_bytes[group]=live_bytes[group];return p;}
 exit(92);
}
static void release(int group,void *p){if(!p)return;for(int i=0;i<128;i++)if(alloc_ptr[group][i]==p){live_bytes[group]-=(long long)alloc_size[group][i];alloc_ptr[group][i]=NULL;++frees[group];_aligned_free(p);return;}exit(93);}
uint8_t *sengoo_collections_allocate(int64_t size,int64_t align){return allocate(0,(size_t)size,(size_t)align);}
void sengoo_collections_release(uint8_t *p){release(0,p);}
void *ref_alloc(size_t size,size_t align){return allocate(1,size,align);}
void ref_free(void *p){release(1,p);}
static long long value_of(const void *p){
 if(mode==2||mode==3){int64_t *token;memcpy(&token,p,8);check(token!=NULL,94);return *token;}
 uint64_t value=0;memcpy(&value,p,element_size<8?element_size:8);return (long long)value;
}
static void make(int group,void *p,long long value){memset(p,0,16);if(mode==2||mode==3){int64_t *token=malloc(8);check(token!=NULL,95);*token=value;memcpy(p,&token,8);if(mode==3)memcpy((uint8_t*)p+8,&value,8);++owned_live[group];}else memcpy(p,&value,element_size<8?element_size:8);}
static void move_value(int group,void *to,void *from){check(to!=from,96);check(event_count[group]<2048,148);events[group][event_count[group]++]=value_of(from)*2;memcpy(to,from,element_size);memset(from,0,element_size);++moves[group];}
static void drop_value(int group,void *value){long long number=value_of(value);check(event_count[group]<2048,149);events[group][event_count[group]++]=number*2+1;++drops[group];sums[group]+=number;if(mode==2||mode==3){void *token;memcpy(&token,value,8);free(token);--owned_live[group];}memset(value,0,element_size);}
static void move0(void *to,void *from){move_value(0,to,from);}static void move1(void *to,void *from){move_value(1,to,from);}
static void drop0(void *value){drop_value(0,value);}static void drop1(void *value){drop_value(1,value);}
static uint32_t rng=0x395e3a71U;
static uint32_t random_u32(void){rng^=rng<<13;rng^=rng>>17;rng^=rng<<5;return rng;}
static void parity(long long candidate,long long reference){
 long long length=sengoo_sg_collections_vec_len(state,sizeof(state),candidate);check(length==sengoo_raw_vec_len(reference),97);
 for(long long i=0;i<length;i++){void *a=sengoo_sg_collections_vec_get(state,sizeof(state),candidate,i),*b=sengoo_raw_vec_get(reference,i);check(value_of(a)==value_of(b),98);if(mode==3)check((uintptr_t)a%16==0&&(uintptr_t)b%16==0&&!memcmp((uint8_t*)a+8,(uint8_t*)b+8,8),150);}
 check(moves[0]==moves[1]&&drops[0]==drops[1]&&sums[0]==sums[1]&&owned_live[0]==owned_live[1],99);
 check(live_bytes[0]==live_bytes[1],100);
 check(event_count[0]==event_count[1]&&!memcmp(events[0],events[1],(size_t)event_count[0]*sizeof(long long)),151);event_count[0]=event_count[1]=0;
 uint8_t before[12352];memcpy(before,state,sizeof(state));check(sengoo_sg_collections_context_validate(state,sizeof(state))==0&&!memcmp(before,state,sizeof(state)),152);
}
static void scopes(void){
 for(mode=0;mode<5;mode++){
  element_size=mode==1?1:mode==3?16:mode==4?3:8;long long align=mode==3?16:mode==1||mode==4?1:8;
  memset(initialized,0,sizeof(initialized));memset(state,0xcc,sizeof(state));check(sengoo_sg_collections_context_init(state,sizeof(state),mode+1)==0,101);
  long long a=sengoo_sg_collections_vec_new(state,sizeof(state),element_size,align,(int64_t)(intptr_t)move0,(int64_t)(intptr_t)drop0);
  long long b=sengoo_raw_vec_new_parts(element_size,align,(void*)move1,(void*)drop1);
  check(a>0&&b>0,102);
  for(int k=0;k<10000;k++){
   ++opcount;uint32_t op=random_u32()%9;long long length=sengoo_sg_collections_vec_len(state,sizeof(state),a),index=(long long)(random_u32()%(uint32_t)(length+2));
   long long number=mode==1?(long long)(random_u32()&1):(long long)(random_u32()%1000+1);__declspec(align(16)) uint8_t left[16],right[16];
   if(length>64&&op==0)op=5;
   if(op==0||op==1||op==2){
    make(0,left,number);make(1,right,number);long long x,y;
    if(op==0){x=sengoo_sg_collections_vec_push(state,sizeof(state),a,left);y=sengoo_raw_vec_push(b,right);}else if(op==1){x=sengoo_sg_collections_vec_set(state,sizeof(state),a,index,left);y=sengoo_raw_vec_set(b,index,right);}else{x=sengoo_sg_collections_vec_insert(state,sizeof(state),a,index,left);y=sengoo_raw_vec_insert(b,index,right);}
    check(x==y,103);for(size_t z=0;z<element_size;z++)check(!left[z]&&!right[z],104);
   }else if(op==3||op==4){
    memset(left,0xcc,16);memset(right,0xcc,16);long long x=op==3?sengoo_sg_collections_vec_pop(state,sizeof(state),a,left):sengoo_sg_collections_vec_remove(state,sizeof(state),a,index,left);long long y=op==3?sengoo_raw_vec_pop(b,right):sengoo_raw_vec_remove(b,index,right);
    check(x==y,105);if(x==0){check(value_of(left)==value_of(right),106);drop0(left);drop1(right);}else for(int z=0;z<16;z++)check(left[z]==0xcc&&right[z]==0xcc,153);
   }else if(op==5){check(sengoo_sg_collections_vec_clear(state,sizeof(state),a)==sengoo_raw_vec_clear(b),107);
   }else if(op==6){
    long long x=sengoo_sg_collections_vec_iter_new(state,sizeof(state),a),y=sengoo_raw_vec_iter_new(b);check(x>0&&y>0,108);
    while(!sengoo_sg_collections_vec_iter_done(state,sizeof(state),x)){check(!sengoo_raw_vec_iter_done(y),109);check(value_of(sengoo_sg_collections_vec_iter_next(state,sizeof(state),x))==value_of(sengoo_raw_vec_iter_next(y)),110);check(sengoo_sg_collections_vec_iter_index(state,sizeof(state),x)==sengoo_raw_vec_iter_index(y),111);}
    check(sengoo_raw_vec_iter_done(y)&&!sengoo_sg_collections_vec_iter_next(state,sizeof(state),x)&&!sengoo_raw_vec_iter_next(y),112);
    check(sengoo_sg_collections_vec_iter_reset(state,sizeof(state),x)==sengoo_raw_vec_iter_reset(y),113);check(sengoo_sg_collections_vec_iter_index(state,sizeof(state),x)==0&&sengoo_raw_vec_iter_index(y)==0,114);
    check(sengoo_sg_collections_vec_iter_free(state,sizeof(state),x)==sengoo_raw_vec_iter_free(y),115);check(sengoo_sg_collections_vec_iter_done(state,sizeof(state),x)==1,116);
   }else if(op==7){
    void *x=sengoo_sg_collections_vec_get(state,sizeof(state),a,index),*y=sengoo_raw_vec_get(b,index);check((x==NULL)==(y==NULL),117);if(x)check(value_of(x)==value_of(y),118);
   }else{
    if(length){void *x=sengoo_sg_collections_vec_get(state,sizeof(state),a,0),*y=sengoo_raw_vec_get(b,0);check(sengoo_sg_collections_vec_set(state,sizeof(state),a,0,x)==0&&sengoo_raw_vec_set(b,0,y)==0,119);}
   }
   parity(a,b);
  }
  check(sengoo_sg_collections_vec_clear(state,sizeof(state),a)==0&&sengoo_raw_vec_clear(b)==0,120);
  /* Force next allocation after disposing old storage, and check ownership of
     the incoming value on reserve failure against the actual C algorithm. */
  check(sengoo_sg_collections_vec_free(state,sizeof(state),a)==0&&sengoo_raw_vec_free(b)==0,121);
  a=sengoo_sg_collections_vec_new(state,sizeof(state),element_size,align,(int64_t)(intptr_t)move0,(int64_t)(intptr_t)drop0);b=sengoo_raw_vec_new_parts(element_size,align,(void*)move1,(void*)drop1);
  __declspec(align(16)) uint8_t left[16],right[16];make(0,left,42);make(1,right,42);fail_allocation[0]=fail_allocation[1]=1;
  check(sengoo_sg_collections_vec_push(state,sizeof(state),a,left)==14&&sengoo_raw_vec_push(b,right)==14,122);parity(a,b);record(5+mode);
  for(int item=0;item<4;item++){make(0,left,item+1);make(1,right,item+1);check(!sengoo_sg_collections_vec_push(state,sizeof(state),a,left)&&!sengoo_raw_vec_push(b,right),180);parity(a,b);}
  void *old_a=sengoo_sg_collections_vec_get(state,sizeof(state),a,0),*old_b=sengoo_raw_vec_get(b,0);make(0,left,91);make(1,right,91);fail_allocation[0]=fail_allocation[1]=1;
  check(sengoo_sg_collections_vec_push(state,sizeof(state),a,left)==14&&sengoo_raw_vec_push(b,right)==14,181);check(sengoo_sg_collections_vec_get(state,sizeof(state),a,0)==old_a&&sengoo_raw_vec_get(b,0)==old_b,182);parity(a,b);record(10+mode);
  make(0,left,92);make(1,right,92);fail_allocation[0]=fail_allocation[1]=1;
  check(sengoo_sg_collections_vec_insert(state,sizeof(state),a,2,left)==14&&sengoo_raw_vec_insert(b,2,right)==14,183);parity(a,b);record(15+mode);
  check(sengoo_sg_collections_vec_free(state,sizeof(state),a)==0&&sengoo_raw_vec_free(b)==0,123);check(sengoo_sg_collections_vec_free(state,sizeof(state),a)==3,124);
  check(!owned_live[0]&&!owned_live[1]&&!live_bytes[0]&&!live_bytes[1],125);
  check(sengoo_sg_collections_context_dispose(state,sizeof(state))==0&&sengoo_sg_collections_vec_live(state,sizeof(state))==-1&&sengoo_opaque_live_handle_count()==0,126);event_count[0]=event_count[1]=0;record(mode);
 }
}
static void registry_cases(void){
 mode=0;element_size=8;check(sengoo_sg_collections_context_init(NULL,0,1)==2,127);long long reads=metadata_accesses;check(sengoo_sg_collections_context_init(NULL,12352,1)==2&&metadata_accesses==reads,128);
 check(sengoo_sg_collections_context_init(state,12351,1)==2&&metadata_accesses==reads,129);check(sengoo_sg_collections_context_init(state,sizeof(state),1)==0,130);
 long long handles[128];for(int i=0;i<128;i++){handles[i]=sengoo_sg_collections_vec_new(state,sizeof(state),8,8,(int64_t)(intptr_t)move0,(int64_t)(intptr_t)drop0);check(handles[i]>0,131);}
 check(sengoo_sg_collections_vec_live(state,sizeof(state))==128&&!sengoo_sg_collections_vec_new(state,sizeof(state),8,8,(int64_t)(intptr_t)move0,(int64_t)(intptr_t)drop0),132);
 check(!sengoo_sg_collections_vec_iter_new(state,sizeof(state),handles[0]),133);check(sengoo_sg_collections_vec_free(state,sizeof(state),handles[0])==0,134);
 long long replacement=sengoo_sg_collections_vec_iter_new(state,sizeof(state),handles[1]);check(replacement>0&&replacement!=handles[0]&&!sengoo_sg_collections_vec_get(state,sizeof(state),handles[0],0),135);
 check(sengoo_sg_collections_vec_free(state,sizeof(state),replacement)==3&&!sengoo_sg_collections_vec_get(state,sizeof(state),replacement,0),136);check(sengoo_sg_collections_vec_iter_done(state,sizeof(state),replacement)==1,137);
 check(sengoo_sg_collections_vec_iter_free(state,sizeof(state),replacement)==0&&sengoo_sg_collections_vec_iter_free(state,sizeof(state),replacement)==2,138);
 check(!sengoo_sg_collections_vec_get(state,sizeof(state),-1,0)&&!sengoo_sg_collections_vec_get(state,sizeof(state),INT64_MAX,0)&&!sengoo_sg_collections_vec_get(state,sizeof(state),4294967296LL,0),139);
 check(sengoo_sg_collections_vec_iter_free(state,sizeof(state),handles[1])==2,140);
 for(int i=1;i<128;i++)check(sengoo_sg_collections_vec_free(state,sizeof(state),handles[i])==0,141);
 check(sengoo_sg_collections_vec_live(state,sizeof(state))==0,142);
 long long vec=sengoo_sg_collections_vec_new(state,sizeof(state),8,8,(int64_t)(intptr_t)move0,(int64_t)(intptr_t)drop0),iter=sengoo_sg_collections_vec_iter_new(state,sizeof(state),vec);int64_t value=42;
 check(sengoo_sg_collections_vec_push(state,sizeof(state),vec,&value)==0&&value==0,143);check(sengoo_sg_collections_vec_free(state,sizeof(state),vec)==0&&sengoo_sg_collections_vec_iter_done(state,sizeof(state),iter)==1&&!sengoo_sg_collections_vec_iter_next(state,sizeof(state),iter),144);
 long long reference=sengoo_raw_vec_new_parts(8,8,(void*)move1,(void*)drop1),reference_iter=sengoo_raw_vec_iter_new(reference);value=42;
 check(reference>0&&reference_iter>0&&!sengoo_raw_vec_push(reference,&value)&&!value&&!sengoo_raw_vec_free(reference),188);
 check(sengoo_sg_collections_vec_iter_done(state,sizeof(state),iter)==sengoo_raw_vec_iter_done(reference_iter)&&!sengoo_raw_vec_iter_next(reference_iter),189);
 check(!sengoo_raw_vec_iter_free(reference_iter),190);
 check(sengoo_sg_collections_context_dispose(state,sizeof(state))==0&&!live_bytes[0]&&owned_live[0]==0,145);record(20);
}
static void lifecycle_cases(void){
 mode=0;element_size=8;event_count[0]=event_count[1]=0;
 long long old=metadata_accesses;uint8_t *bad=(uint8_t*)(uintptr_t)1;
 check(sengoo_sg_collections_context_init(bad,12351,7)==2&&sengoo_sg_collections_context_validate(bad,12351)==3&&sengoo_sg_collections_context_query(bad,12351,0)==-2&&sengoo_sg_collections_context_reset(bad,12351,8)==2&&sengoo_sg_collections_context_dispose(bad,12351)==2,154);
 check(!sengoo_sg_collections_vec_new(bad,12351,8,8,1,1)&&!sengoo_sg_collections_vec_get(bad,12351,1,0)&&sengoo_sg_collections_vec_push(bad,12351,1,bad)==2&&sengoo_sg_collections_vec_set(bad,12351,1,0,bad)==2&&sengoo_sg_collections_vec_insert(bad,12351,1,0,bad)==2&&sengoo_sg_collections_vec_pop(bad,12351,1,bad)==5&&sengoo_sg_collections_vec_remove(bad,12351,1,0,bad)==5,155);
 check(sengoo_sg_collections_vec_clear(bad,12351,1)==3&&sengoo_sg_collections_vec_free(bad,12351,1)==3&&!sengoo_sg_collections_vec_iter_new(bad,12351,1)&&!sengoo_sg_collections_vec_iter_next(bad,12351,1)&&sengoo_sg_collections_vec_iter_done(bad,12351,1)==1&&sengoo_sg_collections_vec_iter_free(bad,12351,1)==2,156);
 check(metadata_accesses==old,157);
 check(sengoo_sg_collections_context_init(state,12352,0)==2&&sengoo_sg_collections_context_init(state,12352,2147483648LL)==2&&metadata_accesses==old,158);
 memset(initialized,0,sizeof(initialized));check(sengoo_sg_collections_context_init(state,12352,700)==0&&sengoo_sg_collections_context_init(other,12352,701)==0,159);
 old=metadata_accesses;check(sengoo_sg_collections_context_query(bad,12352,99)==-2&&metadata_accesses==old,160);
 long long a=sengoo_sg_collections_vec_new(state,12352,8,8,(int64_t)(intptr_t)move0,(int64_t)(intptr_t)drop0),b=sengoo_sg_collections_vec_new(other,12352,8,8,(int64_t)(intptr_t)move0,(int64_t)(intptr_t)drop0);
 check(a>0&&b>0&&a!=b&&!sengoo_sg_collections_vec_get(state,12352,b,0)&&sengoo_sg_collections_vec_free(other,12352,a)==3,161);
 check(sengoo_sg_collections_context_reset(state,12352,702)==2,162);
 long long indices[]={0,1,2,3,4,8,9,10,12,13,14,15,16,17,18,19};
 long long values[]={1,129,129,0,1,16777216,3,1,1,1,0,3,0,0,1,-1};
 for(size_t i=0;i<sizeof(indices)/sizeof(indices[0]);i++){long long saved;memcpy(&saved,state+indices[i]*8,8);memcpy(state+indices[i]*8,&values[i],8);uint8_t before[12352];memcpy(before,state,sizeof(state));check(sengoo_sg_collections_context_validate(state,12352)!=0&&!memcmp(before,state,sizeof(state)),163);memcpy(state+indices[i]*8,&saved,8);}
 check(!sengoo_sg_collections_vec_new(state,12352,8,8,0,(int64_t)(intptr_t)drop0)&&!sengoo_sg_collections_vec_new(state,12352,8,8,(int64_t)(intptr_t)move0,0),164);
 check(sengoo_sg_collections_vec_free(state,12352,a)==0&&sengoo_sg_collections_vec_free(other,12352,b)==0,165);
 check(sengoo_sg_collections_context_reset(state,12352,700)==2&&sengoo_sg_collections_context_reset(state,12352,702)==0&&sengoo_sg_collections_vec_free(state,12352,a)==3,166);
 /* A max-generation free slot is retired rather than wrapped. */
 int64_t max=16777214;memcpy(state+8*8,&max,8);a=sengoo_sg_collections_vec_new(state,12352,8,8,(int64_t)(intptr_t)move0,(int64_t)(intptr_t)drop0);check(a%256==1&&(a/256)%16777216==16777215,167);check(sengoo_sg_collections_vec_free(state,12352,a)==0,168);
 b=sengoo_sg_collections_vec_new(state,12352,8,8,(int64_t)(intptr_t)move0,(int64_t)(intptr_t)drop0);check(b%256==2&&!sengoo_sg_collections_vec_get(state,12352,a,0)&&sengoo_sg_collections_vec_free(state,12352,b)==0,184);
 check(sengoo_sg_collections_context_dispose(state,12352)==0&&sengoo_sg_collections_context_dispose(state,12352)==3&&sengoo_sg_collections_context_dispose(other,12352)==0,169);
 /* Deliberately violate the unique-identity precondition and record the actual
    alias. This is not claimed as a rejected or safe input. */
 check(sengoo_sg_collections_context_init(state,12352,703)==0&&sengoo_sg_collections_context_init(other,12352,703)==0,170);
 a=sengoo_sg_collections_vec_new(state,12352,8,8,(int64_t)(intptr_t)move0,(int64_t)(intptr_t)drop0);b=sengoo_sg_collections_vec_new(other,12352,8,8,(int64_t)(intptr_t)move0,(int64_t)(intptr_t)drop0);check(a==b,171);
 check(sengoo_sg_collections_context_dispose(state,12352)==0&&sengoo_sg_collections_context_dispose(other,12352)==0,172);record(21);
}
static void reference_slot_failure(void){
 mode=0;element_size=8;check(reference_object_count==0,173);
 reference_slot_fail=1;check(!sengoo_raw_vec_new_parts(8,8,(void*)move1,(void*)drop1)&&reference_object_count==0,174);
 long long vector=sengoo_raw_vec_new_parts(8,8,(void*)move1,(void*)drop1);check(vector>0&&reference_object_count==1,175);
 reference_slot_fail=1;check(!sengoo_raw_vec_iter_new(vector)&&reference_object_count==2&&sengoo_opaque_live_handle_count()==1,176);
 void *backing=sengoo_opaque_handle_get(vector),*leak=NULL;for(int i=0;i<512;i++)if(reference_objects[i]&&reference_objects[i]!=backing){check(reference_object_bytes[i]==16,177);leak=reference_objects[i];}
 check(leak!=NULL,178);reference_iterator_leak_observed=1;ref_object_free(leak); /* fixture-only cleanup of proven leaked allocation */
 check(!sengoo_raw_vec_free(vector)&&!reference_object_count&&!sengoo_opaque_live_handle_count(),179);record(22);
}
int main(void){scopes();registry_cases();lifecycle_cases();reference_slot_failure();check(allocations[0]==frees[0]&&allocations[1]==frees[1],146);
 check(completed_count==23,187);
 printf("{\"operations\":%lld,\"families\":5,\"state_bytes\":12352,\"slots\":128,\"candidate_allocations\":%lld,\"reference_allocations\":%lld,\"candidate_peak_bytes\":%lld,\"reference_peak_bytes\":%lld,\"differential_callback_order_equal\":true,\"owning_8byte_and_16byte_elements\":true,\"zero_final_allocations\":true,\"oom_consumes_input_exactly_once\":true,\"generation_and_kind_rejection\":true,\"readonly_validation\":true,\"bad_extent_zero_access\":true,\"initialized_metadata_reads\":true,\"same_identity_alias_is_unsafe_precondition\":true,\"reference_iterator_slot_failure_leak_observed\":%s,\"actual_reference_runtime\":true,\"completed\":[",opcount,allocations[0],allocations[1],peak_bytes[0],peak_bytes[1],reference_iterator_leak_observed?"true":"false");
 for(int i=0;i<completed_count;i++)printf(i?",%d":"%d",completed[i]);puts("]}");return 42;}
