#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>
extern int64_t sengoo_sg_deflate_stream_init(uint8_t *,int64_t,int64_t,int64_t,int64_t);
extern int64_t sengoo_sg_deflate_stream_step(uint8_t *,int64_t,const uint8_t *,int64_t,uint8_t *,int64_t,int64_t);
extern int64_t sengoo_sg_deflate_stream_query(uint8_t *,int64_t,int64_t);
extern int64_t sengoo_sg_deflate_stream_reset(uint8_t *,int64_t,int64_t,int64_t,int64_t);
extern int64_t sengoo_sg_deflate_stream_dispose(uint8_t *,int64_t);
static uint8_t *active_state,*active_output;
static const uint8_t *active_input;
static int64_t input_extent,output_extent,input_reads,output_writes;
static uint8_t word_initialized[5632],history_initialized[32768];
static uint64_t calls,records,need_inputs,need_outputs,history_reads;
static uint64_t state_reads,state_writes;
static unsigned fixture;
static void fail(const char *reason,int64_t expected,int64_t actual){fprintf(stderr,"fixture=%u reason=%s expected=%lld actual=%lld\n",fixture,reason,(long long)expected,(long long)actual);exit(1);}
#ifndef SENGOO_DS_PRODUCTION
uint8_t sengoo_ds_input_byte(const uint8_t *p,int64_t i){if(p!=active_input||i!=input_reads||i<0||i>=input_extent)fail("input-range",input_extent,i);++input_reads;return p[i];}
uint8_t sengoo_ds_state_byte(uint8_t *p,int64_t i){if(p!=active_state||i<10520||i>=43288||!history_initialized[i-10520])fail("history-uninitialized",1,i);++history_reads;return p[i];}
void sengoo_ds_state_write(uint8_t *p,int64_t i,uint8_t value){if(p!=active_state||i<10520||i>=43288)fail("history-write",1,i);history_initialized[i-10520]=1;p[i]=value;}
int64_t sengoo_ds_word_get(uint8_t *p,int64_t i){if(p!=active_state||i<0||i>=5632||!word_initialized[i])fail("state-word-read",5632,i);++state_reads;int64_t result;memcpy(&result,p+i*8,8);return result;}
void sengoo_ds_word_put(uint8_t *p,int64_t i,int64_t value){if(p!=active_state||i<0||i>=5632)fail("state-word-write",5632,i);++state_writes;word_initialized[i]=1;memcpy(p+i*8,&value,8);if(i*8>=10520&&i*8<43288)memset(history_initialized+i*8-10520,0,8);}
void sengoo_ds_output_write(uint8_t *p,int64_t i,uint8_t value){if(p!=active_output||i!=output_writes||i<0||i>=output_extent)fail("output-range",output_extent,i);++output_writes;p[i]=value;}
#endif
static uint64_t number(const uint8_t **p,const uint8_t *end,unsigned n){if(end-*p<(int64_t)n)fail("short-corpus",n,end-*p);uint64_t value=0;for(unsigned i=0;i<n;i++)value|=(uint64_t)*(*p)++<<(i*8);return value;}
static void run(const uint8_t *source,int64_t length,const uint8_t *plain,int64_t plain_length,int64_t expected,int64_t budget,int mode){
    uint8_t *allocation=VirtualAlloc(NULL,45056+8192,MEM_RESERVE,PAGE_NOACCESS);if(!allocation)fail("reserve",1,0);
    uint8_t *state=VirtualAlloc(allocation+4096,45056,MEM_COMMIT,PAGE_READWRITE);if(!state)fail("commit",1,0);active_state=state;memset(word_initialized,0,sizeof word_initialized);memset(history_initialized,0,sizeof history_initialized);
    if(sengoo_sg_deflate_stream_init(state,45056,1,67108864,budget))fail("init",0,1);
    if(sengoo_sg_deflate_stream_query(state,45056,6)!=45056)fail("state-size",45056,0);
    uint8_t *output_allocation=VirtualAlloc(NULL,12288,MEM_RESERVE,PAGE_NOACCESS);if(!output_allocation)fail("output-reserve",1,0);
    uint8_t *output_page=VirtualAlloc(output_allocation+4096,4096,MEM_COMMIT,PAGE_READWRITE);if(!output_page)fail("output-commit",1,0);
    int64_t offset=0,delivered=0,end=0;uint64_t random=0x345ab823+fixture;int64_t status=0;unsigned steps=0;
    while(steps++<10000000){
        if(offset==end&&end<length){random=random*6364136223846793005ULL+1;int64_t chunk=mode==0?1:mode==1?(int64_t)(random%73)+1:mode>=3&&offset==0?mode-2:length;end=offset+chunk;if(end>length)end=length;}
        int64_t capacity=mode==0?1:mode==1?(int64_t)(random%257)+1:257;uint8_t *output=output_page+4096-capacity;
        active_input=source+offset;input_extent=end-offset;input_reads=0;active_output=output;output_extent=capacity;output_writes=0;
        status=sengoo_sg_deflate_stream_step(state,45056,active_input,input_extent,output,capacity,end==length);++calls;
        int64_t used=sengoo_sg_deflate_stream_query(state,45056,0),made=sengoo_sg_deflate_stream_query(state,45056,1);
#ifdef SENGOO_DS_PRODUCTION
        input_reads=used;output_writes=made;
#endif
        if(used!=input_reads||made!=output_writes||used<0||used>input_extent||made<0||made>capacity)fail("call-counters",input_reads,used);
        if(expected>=0&&(made>plain_length-delivered||memcmp(output,plain+delivered,(size_t)made)))fail("output-oracle",0,1);
        offset+=used;delivered+=made;
        if(sengoo_sg_deflate_stream_query(state,45056,2)!=offset||sengoo_sg_deflate_stream_query(state,45056,3)!=delivered||sengoo_sg_deflate_stream_query(state,45056,4)!=status)fail("total-counters",offset,-1);
        if(status==0)++need_inputs;else if(status==1)++need_outputs;else break;
        if(status==0&&end==length)fail("finish-returned-need-input",-1,0);
        if(status==0&&used==0&&made==0&&input_extent>0)fail("stalled-input",1,0);
    }
    if(expected>=0){if(status!=2||offset!=length||delivered!=plain_length)fail("valid-terminal",2,status);}
    else if(status!=expected)fail("negative-terminal",expected,status);
    if(sengoo_sg_deflate_stream_dispose(state,45056))fail("dispose",0,1);
    if(sengoo_sg_deflate_stream_query(state,45056,4)!=-4)fail("disposed-query",-4,0);
    if(!VirtualFree(allocation,0,MEM_RELEASE)||!VirtualFree(output_allocation,0,MEM_RELEASE))fail("state-release",1,0);++records;
}
static int64_t step(uint8_t *state,const uint8_t *input,int64_t size,uint8_t *output,int64_t capacity,int64_t finish) {
    active_state=state;active_input=input;input_extent=size;input_reads=0;active_output=output;output_extent=capacity;output_writes=0;
    int64_t result=sengoo_sg_deflate_stream_step(state,45056,input,size,output,capacity,finish);++calls;
#ifdef SENGOO_DS_PRODUCTION
    if(result!=-3){input_reads=sengoo_sg_deflate_stream_query(state,45056,0);output_writes=sengoo_sg_deflate_stream_query(state,45056,1);}
#endif
    if(result!=-3&&(sengoo_sg_deflate_stream_query(state,45056,0)!=input_reads||sengoo_sg_deflate_stream_query(state,45056,1)!=output_writes))fail("protocol-call-counts",input_reads,-1);
    return result;
}
static void init(uint8_t *state,int exact){active_state=state;memset(word_initialized,0,sizeof word_initialized);memset(history_initialized,0,sizeof history_initialized);if(sengoo_sg_deflate_stream_init(state,45056,exact,67108864,536870912))fail("protocol-init",0,1);}
static void protocol(void){
    uint8_t *state=malloc(45056),*snapshot=malloc(45056);if(!state||!snapshot)fail("protocol-allocation",1,0);
    const uint8_t data[]={0x73,0x1c,0x05,0x00,0xff,0x42};uint8_t output[512];
    uint64_t reads_before=state_reads,writes_before=state_writes;
    if(sengoo_sg_deflate_stream_init(NULL,45055,0,0,0)!=-3||sengoo_sg_deflate_stream_init(NULL,45056,2,0,0)!=-3||sengoo_sg_deflate_stream_init(NULL,45056,0,-1,0)!=-3||sengoo_sg_deflate_stream_init(NULL,45056,0,67108865,0)!=-3||sengoo_sg_deflate_stream_init(NULL,45056,0,0,536870913)!=-3)fail("invalid-init",-3,0);
    if(sengoo_sg_deflate_stream_query(NULL,45056,7)!=-3||sengoo_sg_deflate_stream_query(NULL,45055,0)!=-3||sengoo_sg_deflate_stream_dispose(NULL,45055)!=-3)fail("invalid-query-dispose",-3,0);
    if(sengoo_sg_deflate_stream_step(NULL,45056,NULL,-1,NULL,0,0)!=-3||sengoo_sg_deflate_stream_step(NULL,45056,NULL,0,NULL,67108865,0)!=-3||sengoo_sg_deflate_stream_step(NULL,45056,NULL,0,NULL,0,2)!=-3)fail("invalid-step",-3,0);
    if(state_reads!=reads_before||state_writes!=writes_before)fail("invalid-state-access",0,1);
    init(state,0);for(int field=0;field<6;field++)if(sengoo_sg_deflate_stream_query(state,45056,field)!=0)fail("fresh-query",0,field);
    if(step(state,data,6,output,512,0)!=2||input_reads!=4||output_writes!=259)fail("prefix-boundary",4,input_reads);
    for(int i=0;i<259;i++)if(output[i]!=65)fail("prefix-bytes",65,output[i]);
    init(state,1);if(step(state,data,6,output,512,1)!=-1||input_reads!=4)fail("exact-tail",-1,0);
    init(state,1);if(step(state,data,4,output,512,0)!=0||output_writes!=259)fail("await-final",0,1);
    if(step(state,NULL,0,NULL,0,1)!=2||input_reads||output_writes)fail("final-empty",2,0);
    init(state,1);if(step(state,data,4,NULL,0,1)!=1||output_writes)fail("zero-cap-pending",1,0);
    int64_t consumed=input_reads;
    if(step(state,data+consumed,4-consumed,output,1,0)!=-3)fail("sticky-finish",-3,0);
    int64_t offset=consumed,total=0;
    while(1){int64_t result=step(state,data+offset,4-offset,output+total,1,1);offset+=input_reads;total+=output_writes;if(result==2)break;if(result!=1)fail("sticky-retry",1,result);}
    if(offset!=4||total!=259)fail("zero-cap-loss",259,total);
    init(state,1);if(step(state,data,3,output,1,0)!=1)fail("prepare-match-drain",1,0);
    if(input_reads!=3||output_writes!=1)fail("prepare-match-counts",3,input_reads);
    memcpy(snapshot,state,45056);uint8_t saved_words[5632],saved_history[32768];memcpy(saved_words,word_initialized,sizeof saved_words);memcpy(saved_history,history_initialized,sizeof saved_history);
    for(int copy=0;copy<2;copy++){
        uint8_t *selected=copy?snapshot:state;memcpy(word_initialized,saved_words,sizeof saved_words);memcpy(history_initialized,saved_history,sizeof saved_history);int64_t got=1;
        while(1){int64_t result=step(selected,NULL,0,output,7,0);got+=output_writes;for(int i=0;i<output_writes;i++)if(output[i]!=65)fail("drain-bytes",65,output[i]);if(result==0)break;if(result!=1)fail("drain-status",1,result);}
        if(got!=259)fail("drain-count",259,got);
        if(step(selected,data+3,1,output,512,1)!=2||output_writes)fail("snapshot-complete",2,0);
    }
    active_state=state;if(sengoo_sg_deflate_stream_reset(state,45056,1,67108864,536870912))fail("reset",0,1);
    if(sengoo_sg_deflate_stream_query(state,45056,2)!=0||sengoo_sg_deflate_stream_query(state,45056,3)!=0)fail("reset-count",0,1);
    const uint8_t empty[]={3,0};if(step(state,empty,2,NULL,0,1)!=2)fail("null-empty-output",2,0);
    if(sengoo_sg_deflate_stream_dispose(state,45056))fail("protocol-dispose",0,1);
    for(int i=0;i<45056;i++)if(state[i])fail("dispose-not-zero",0,state[i]);
    if(sengoo_sg_deflate_stream_query(state,45056,4)!=-4)fail("disposed-invalid",-4,0);
    free(state);free(snapshot);
}
int main(int argc,char **argv){
    SYSTEM_INFO system;GetSystemInfo(&system);if(system.dwPageSize!=4096)return 11;
    if(argc!=2)return 2;FILE *file=NULL;if(fopen_s(&file,argv[1],"rb")||!file)return 3;fseek(file,0,SEEK_END);long length=ftell(file);rewind(file);if(length<16)return 4;
    uint8_t *raw=malloc((size_t)length);if(!raw||fread(raw,1,length,file)!=(size_t)length)return 5;fclose(file);
    const uint8_t *at=raw,*end=raw+length;if(memcmp(at,"SGDFL1\0\0",8))return 6;at+=8;uint64_t count=number(&at,end,8);
    protocol();
    for(fixture=0;fixture<count;fixture++){
        int64_t n=number(&at,end,4),capacity=number(&at,end,4),expected=(int64_t)number(&at,end,8),plain_length=number(&at,end,4);
        if(end-at<n+plain_length)fail("corpus-record",n+plain_length,end-at);
        size_t rounded=((size_t)n+4095)/4096*4096;if(!rounded)rounded=4096;
        uint8_t *input_allocation=VirtualAlloc(NULL,rounded+8192,MEM_RESERVE,PAGE_NOACCESS);if(!input_allocation)fail("input-reserve",1,0);
        uint8_t *page=VirtualAlloc(input_allocation+4096,rounded,MEM_COMMIT,PAGE_READWRITE);if(!page)fail("input-commit",1,0);
        uint8_t *guarded=page+rounded-n;if(n)memcpy(guarded,at,(size_t)n);DWORD old;if(!VirtualProtect(page,rounded,PAGE_READONLY,&old))fail("input-readonly",1,0);
        if(expected!=-2){for(int mode=0;mode<3;mode++)run(guarded,n,at+n,plain_length,expected,67108864,mode);}
        else run(guarded,n,at+n,plain_length,expected,capacity,2);
        if(expected>=0&&n<=64){for(int cut=1;cut<n;cut++)run(guarded,n,at+n,plain_length,expected,67108864,cut+2);}
        if(!VirtualFree(input_allocation,0,MEM_RELEASE))fail("input-release",1,0);
        at+=n+plain_length;
    }
    if(at!=end)fail("corpus-tail",0,end-at);
#ifdef SENGOO_DS_PRODUCTION
    printf("{\"schema\":1,\"ok\":true,\"mode\":\"production\",\"fixtures\":%llu,\"runs\":%llu,\"calls\":%llu,\"need_input\":%llu,\"need_output\":%llu,\"history_reads\":null,\"state_bytes\":45056}\n",(unsigned long long)count,(unsigned long long)records,(unsigned long long)calls,(unsigned long long)need_inputs,(unsigned long long)need_outputs);
#else
    printf("{\"schema\":1,\"ok\":true,\"mode\":\"instrumented\",\"fixtures\":%llu,\"runs\":%llu,\"calls\":%llu,\"need_input\":%llu,\"need_output\":%llu,\"history_reads\":%llu,\"state_bytes\":45056}\n",(unsigned long long)count,(unsigned long long)records,(unsigned long long)calls,(unsigned long long)need_inputs,(unsigned long long)need_outputs,(unsigned long long)history_reads);
#endif
    free(raw);return 0;
}
