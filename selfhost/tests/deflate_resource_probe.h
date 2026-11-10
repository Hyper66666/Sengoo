/* Include after the original validator in the same translation unit. */
#define sengoo_ds_storage_alloc sh_resource_provider_alloc
#define sengoo_ds_storage_present sh_resource_provider_present
#define sengoo_ds_storage_free sh_resource_provider_free
#include "../runtime/deflate_storage.c"
#undef sengoo_ds_storage_alloc
#undef sengoo_ds_storage_present
#undef sengoo_ds_storage_free

#if defined(SENGOO_RESOURCE_GZIP)
#if !defined(SENGOO_GZIP_PRODUCTION_MEMORY)
#define SENGOO_RESOURCE_INSTRUMENTED
#endif
#elif !defined(SENGOO_DEFLATE_PRODUCTION_MEMORY)
#define SENGOO_RESOURCE_INSTRUMENTED
#endif
static uint8_t *sh_state_owner;
static uint8_t sh_history_initialized[32768];
static uint64_t sh_state_live,sh_state_created,sh_state_freed,sh_state_peak,sh_state_attempts;
static int sh_fail_next;
uint8_t *sengoo_ds_storage_alloc(int64_t length) {
    ++sh_state_attempts;
    if(length!=45056||sh_state_live)fail("state-allocation-shape",45056,length);
    if(sh_fail_next){sh_fail_next=0;return NULL;}
    uint8_t *state=sh_resource_provider_alloc(length);if(!state)return NULL;
    MEMORY_BASIC_INFORMATION info={0};
    if(!VirtualQuery(state,&info,sizeof info)||info.State!=MEM_COMMIT||info.RegionSize!=45056)fail("actual-state-commit",45056,(int64_t)info.RegionSize);
    sh_state_owner=state;sh_state_live=45056;++sh_state_created;if(sh_state_live>sh_state_peak)sh_state_peak=sh_state_live;
    memset(sh_history_initialized,0,sizeof sh_history_initialized);return state;
}
int64_t sengoo_ds_storage_present(uint8_t *state){return sh_resource_provider_present(state);}
void sengoo_ds_storage_free(uint8_t *state) {
    if(state!=sh_state_owner||sh_state_live!=45056)fail("state-free-owner",45056,(int64_t)sh_state_live);
    for(int i=0;i<45056;i++)if(state[i])fail("state-dispose-zero",0,state[i]);
    sh_resource_provider_free(state);MEMORY_BASIC_INFORMATION info={0};
    if(!VirtualQuery(state,&info,sizeof info)||info.State!=MEM_FREE)fail("actual-state-release",MEM_FREE,(int64_t)info.State);
    sh_state_owner=NULL;sh_state_live=0;++sh_state_freed;
}
#ifdef SENGOO_RESOURCE_INSTRUMENTED
uint8_t sengoo_ds_input_byte(const uint8_t *source,int64_t index){return sengoo_deflate_input_read(source,index);}
void sengoo_ds_output_write(uint8_t *target,int64_t index,uint8_t value){sengoo_deflate_output_write(target,index,value);}
uint8_t sengoo_ds_state_byte(uint8_t *state,int64_t index){
    if(state!=sh_state_owner||index<10520||index>=43288||!sh_history_initialized[index-10520])fail("state-uninitialized-history",1,index);
    ++output_reads;return state[index];
}
void sengoo_ds_state_write(uint8_t *state,int64_t index,uint8_t value){
    if(state!=sh_state_owner||index<10520||index>=43288)fail("state-history-write",1,index);
    sh_history_initialized[index-10520]=1;state[index]=value;
}
int64_t sengoo_ds_word_get(uint8_t *state,int64_t index){
    if(state!=sh_state_owner||index<0||index>=5632)fail("state-word-read",5632,index);
    int64_t value;memcpy(&value,state+index*8,8);return value;
}
void sengoo_ds_word_put(uint8_t *state,int64_t index,int64_t value){
    if(state!=sh_state_owner||index<0||index>=5632)fail("state-word-write",5632,index);memcpy(state+index*8,&value,8);
}
#endif
static void sh_resource_receipt(void){
    if(sh_state_live||sh_state_created!=sh_state_freed||sh_state_peak!=(sh_state_created?45056:0))fail("final-state-resources",(int64_t)sh_state_created,(int64_t)sh_state_freed);
    fprintf(stderr,"{\"schema\":1,\"live_bytes\":%llu,\"created\":%llu,\"freed\":%llu,\"peak_bytes\":%llu,\"attempts\":%llu,\"disposed_zero\":true,\"virtual_release_verified\":true}\n",(unsigned long long)sh_state_live,(unsigned long long)sh_state_created,(unsigned long long)sh_state_freed,(unsigned long long)sh_state_peak,(unsigned long long)sh_state_attempts);
}
static int sh_resource_oom(void){
    uint64_t before=input_reads+output_reads+output_writes;const uint8_t input[]={0};uint8_t output[]={123};
    (void)before;
    if(sengoo_sg_raw_inflate(input,-1,output,1)!=-3||sh_state_attempts)fail("invalid-limit-allocated",0,(int64_t)sh_state_attempts);
    sh_fail_next=1;
#if defined(SENGOO_RESOURCE_GZIP)
    const uint8_t gzip_empty[]={31,139,8,0,0,0,0,0,0,255,3,0,0,0,0,0,0,0,0,0};
    active_input=gzip_empty;input_extent=sizeof gzip_empty;active_output=NULL;output_extent=current_writes=current_input_reads=0;
    int64_t result=sengoo_sg_gzip_decode(gzip_empty,sizeof gzip_empty,NULL,0,0);
#ifdef SENGOO_RESOURCE_INSTRUMENTED
    if(current_input_reads!=10||output_writes)fail("gzip-oom-header-only",10,(int64_t)current_input_reads);
#endif
#elif defined(SENGOO_RESOURCE_PREFIX)
    int64_t result=sengoo_sg_raw_inflate_prefix(input,1,output,1);
    if(input_reads+output_reads+output_writes!=before||output[0]!=123)fail("prefix-oom-access",0,1);
#else
    int64_t result=sengoo_sg_raw_inflate(input,1,output,1);
    if(input_reads+output_reads+output_writes!=before||output[0]!=123)fail("raw-oom-access",0,1);
#endif
    if(result!=-2||sh_state_created||sh_state_freed||sh_state_live||sh_state_attempts!=1)fail("oom-result-resources",-2,result);
    sh_resource_receipt();puts("{\"schema\":1,\"ok\":true,\"oom\":true,\"no_raw_input_output_access\":true}");return 0;
}
