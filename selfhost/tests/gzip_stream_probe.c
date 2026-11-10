#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>
extern int64_t sengoo_sg_gzip_stream_init(uint8_t *,int64_t,int64_t,int64_t,int64_t);
extern int64_t sengoo_sg_gzip_stream_step(uint8_t *,int64_t,const uint8_t *,int64_t,uint8_t *,int64_t,int64_t);
extern int64_t sengoo_sg_gzip_stream_query(uint8_t *,int64_t,int64_t);
extern int64_t sengoo_sg_gzip_stream_reset(uint8_t *,int64_t,int64_t,int64_t,int64_t);
extern int64_t sengoo_sg_gzip_stream_dispose(uint8_t *,int64_t);
static uint8_t *state_owner,*output_owner;
static const uint8_t *input_owner;
static int64_t input_extent,output_extent,current_reads,current_writes;
static uint8_t initialized[46080],history_initialized[32768];
static uint64_t runs,calls,need_inputs,need_outputs,history_reads,crc_output_reads;
static uint64_t state_reads,state_writes,offset_calls;
static uint64_t completed=1469598103934665603ULL;
static unsigned fixture;
static void fail(const char *reason,int64_t expected,int64_t actual){fprintf(stderr,"fixture=%u reason=%s expected=%lld actual=%lld\n",fixture,reason,(long long)expected,(long long)actual);exit(1);}
#ifndef SENGOO_GZS_PRODUCTION
static int64_t input_index(const uint8_t *p,int64_t index,int access){
    uintptr_t base=(uintptr_t)input_owner,at=(uintptr_t)p;
    if(at<base||at-base>(uint64_t)input_extent||index<0||(uint64_t)index>(uint64_t)input_extent-(at-base)||access&&(uint64_t)index==(uint64_t)input_extent-(at-base))fail("input-range",input_extent,index);
    return (int64_t)(at-base)+index;
}
static int64_t output_index(uint8_t *p,int64_t index,int access){
    uintptr_t base=(uintptr_t)output_owner,at=(uintptr_t)p;
    if(at<base||at-base>(uint64_t)output_extent||index<0||(uint64_t)index>(uint64_t)output_extent-(at-base)||access&&(uint64_t)index==(uint64_t)output_extent-(at-base))fail("output-range",output_extent,index);
    return (int64_t)(at-base)+index;
}
static int64_t state_index(uint8_t *p,int64_t index,int width){
    if(p!=state_owner&&p!=state_owner+1024)fail("state-owner",1,0);
    int64_t begin=p-state_owner;
    if(index<0||index>(46080-begin-width)/width)fail("state-range",46080,index);
    return begin+index*width;
}
uint8_t sengoo_ds_input_byte(const uint8_t *p,int64_t index){int64_t at=input_index(p,index,1);if(at!=current_reads)fail("raw-input-sequence",current_reads,at);++current_reads;return p[index];}
uint8_t sengoo_deflate_input_read(const uint8_t *p,int64_t index){return sengoo_ds_input_byte(p,index);}
void sengoo_ds_output_write(uint8_t *p,int64_t index,uint8_t value){int64_t at=output_index(p,index,1);if(at!=current_writes)fail("output-sequence",current_writes,at);++current_writes;p[index]=value;}
void sengoo_deflate_output_write(uint8_t *p,int64_t index,uint8_t value){sengoo_ds_output_write(p,index,value);}
uint8_t sengoo_deflate_output_read(uint8_t *p,int64_t index){int64_t at=output_index(p,index,1);if(at>=current_writes)fail("crc-uninitialized",current_writes,at);++crc_output_reads;return p[index];}
int64_t sengoo_ds_word_get(uint8_t *p,int64_t index){int64_t at=state_index(p,index,8);for(int i=0;i<8;i++)if(!initialized[at+i])fail("state-uninitialized",1,at+i);++state_reads;int64_t result;memcpy(&result,p+index*8,8);return result;}
void sengoo_ds_word_put(uint8_t *p,int64_t index,int64_t value){int64_t at=state_index(p,index,8);memset(initialized+at,1,8);memcpy(p+index*8,&value,8);++state_writes;if(at>=11544&&at<44312)memset(history_initialized+at-11544,0,8);}
uint8_t sengoo_ds_state_byte(uint8_t *p,int64_t index){int64_t at=state_index(p,index,1);if(at<11544||at>=44312||!history_initialized[at-11544])fail("history-uninitialized",1,at);++history_reads;return p[index];}
void sengoo_ds_state_write(uint8_t *p,int64_t index,uint8_t value){int64_t at=state_index(p,index,1);if(at<11544||at>=44312)fail("history-write",1,at);history_initialized[at-11544]=1;p[index]=value;}
const uint8_t *sengoo_gzip_input_offset(const uint8_t *p,int64_t index){(void)input_index(p,index,0);++offset_calls;return index?p+index:p;}
uint8_t *sengoo_gzip_output_offset(uint8_t *p,int64_t index){(void)output_index(p,index,0);++offset_calls;return index?p+index:p;}
uint8_t *sengoo_gzs_state_offset(uint8_t *p,int64_t index){if(p!=state_owner||index!=1024)fail("substate-offset",1024,index);++offset_calls;return p+index;}
#endif
static uint64_t number(const uint8_t **p,const uint8_t *end,unsigned n){if(end-*p<(int64_t)n)fail("corpus-short",n,end-*p);uint64_t result=0;for(unsigned i=0;i<n;i++)result|=(uint64_t)*(*p)++<<(8*i);return result;}
static void check(const uint8_t *input,int64_t length,const uint8_t *plain,int64_t size,int64_t expected,int64_t budget,int64_t policy,int schedule,int64_t expected_members){
    uint8_t *allocation=VirtualAlloc(NULL,57344,MEM_RESERVE,PAGE_NOACCESS);if(!allocation)fail("state-reserve",1,0);
    uint8_t *state_page=VirtualAlloc(allocation+4096,49152,MEM_COMMIT,PAGE_READWRITE);if(!state_page)fail("state-commit",1,0);
    uint8_t *state=schedule%2?state_page:state_page+49152-46080;state_owner=state;memset(initialized,0,sizeof initialized);memset(history_initialized,0,sizeof history_initialized);
    uint8_t *out_allocation=VirtualAlloc(NULL,12288,MEM_RESERVE,PAGE_NOACCESS);if(!out_allocation)fail("out-reserve",1,0);
    uint8_t *page=VirtualAlloc(out_allocation+4096,4096,MEM_COMMIT,PAGE_READWRITE);if(!page)fail("out-commit",1,0);
    if(sengoo_sg_gzip_stream_init(state,46080,policy,67108864,budget))fail("init",0,1);
    int64_t used=0,made=0,end=0,status=0;unsigned steps=0;uint64_t random=0x1952369+fixture;
    while(steps++<10000000){
        if(used==end&&end<length){random=random*6364136223846793005ULL+1;int64_t chunk=schedule==0?1:schedule==1?(int64_t)(random%79)+1:schedule>=3&&used==0?schedule-2:length;end=used+chunk;if(end>length)end=length;}
        int64_t capacity=schedule==0?1:schedule==1?(int64_t)(random%257)+1:257;uint8_t *output=page+4096-capacity;
        input_owner=input+used;input_extent=end-used;current_reads=0;output_owner=output;output_extent=capacity;current_writes=0;
        status=sengoo_sg_gzip_stream_step(state,46080,input_owner,input_extent,output,capacity,end==length);++calls;
        int64_t consumed=sengoo_sg_gzip_stream_query(state,46080,0),produced=sengoo_sg_gzip_stream_query(state,46080,1);
#ifdef SENGOO_GZS_PRODUCTION
        current_reads=consumed;current_writes=produced;
#endif
        if(consumed!=current_reads||produced!=current_writes||consumed<0||consumed>input_extent||produced<0||produced>capacity)fail("call-counters",current_reads,consumed);
        if((expected>=0||size>0)&&(produced>size-made||memcmp(output,plain+made,(size_t)produced)))fail("output-oracle",size,made+produced);
        used+=consumed;made+=produced;
        if(sengoo_sg_gzip_stream_query(state,46080,2)!=used||sengoo_sg_gzip_stream_query(state,46080,3)!=made||sengoo_sg_gzip_stream_query(state,46080,4)!=status)fail("cumulative-counts",used,-1);
        if(status==0)++need_inputs;else if(status==1)++need_outputs;else break;
        if(status==0&&end==length)fail("finish-need-input",-1,0);
        if(status==0&&consumed==0&&produced==0&&input_extent>0)fail("input-stall",1,0);
    }
    if(expected>=0){if(status!=2||made!=(expected&134217727)||used!=(expected>>27))fail("success-counts",expected,(used<<27)|made);}
    else if(status!=expected)fail("negative-status",expected,status);
    if(expected>=0&&expected_members>=0&&sengoo_sg_gzip_stream_query(state,46080,7)!=expected_members)fail("validated-member-count",expected_members,sengoo_sg_gzip_stream_query(state,46080,7));
    int64_t expected_status=expected>=0?2:expected;
    for(unsigned n=0;n<8;n++){completed^=((uint64_t)fixture>>(n*8))&255;completed*=1099511628211ULL;}
    for(unsigned n=0;n<8;n++){completed^=((uint64_t)schedule>>(n*8))&255;completed*=1099511628211ULL;}
    for(unsigned n=0;n<8;n++){completed^=((uint64_t)expected_status>>(n*8))&255;completed*=1099511628211ULL;}
    if(sengoo_sg_gzip_stream_dispose(state,46080))fail("dispose",0,1);
    for(int i=0;i<46080;i++)if(state[i])fail("dispose-erasure",0,state[i]);
    if(sengoo_sg_gzip_stream_query(state,46080,4)!=-4)fail("disposed-query",-4,0);
    VirtualFree(allocation,0,MEM_RELEASE);VirtualFree(out_allocation,0,MEM_RELEASE);++runs;
}
static int64_t step(uint8_t *state,const uint8_t *source,int64_t n,uint8_t *output,int64_t capacity,int64_t finish){
    state_owner=state;input_owner=source;input_extent=n;current_reads=0;output_owner=output;output_extent=capacity;current_writes=0;
    int64_t result=sengoo_sg_gzip_stream_step(state,46080,source,n,output,capacity,finish);++calls;
#ifdef SENGOO_GZS_PRODUCTION
    if(result!=-3){current_reads=sengoo_sg_gzip_stream_query(state,46080,0);current_writes=sengoo_sg_gzip_stream_query(state,46080,1);}
#endif
    if(result!=-3&&(sengoo_sg_gzip_stream_query(state,46080,0)!=current_reads||sengoo_sg_gzip_stream_query(state,46080,1)!=current_writes))fail("protocol-counters",current_reads,-1);
    return result;
}
static void init(uint8_t *state,int mode){state_owner=state;memset(initialized,0,sizeof initialized);memset(history_initialized,0,sizeof history_initialized);if(sengoo_sg_gzip_stream_init(state,46080,mode,67108864,536870912))fail("protocol-init",0,1);}
static void protocol(void){
    const uint8_t empty[]={31,139,8,0,0,0,0,0,0,255,3,0,0,0,0,0,0,0,0,0};
    const uint8_t repeated[]={31,139,8,0,0,0,0,0,0,255,115,28,5,0,191,174,228,79,3,1,0,0};
    uint8_t *state=malloc(46080),*snapshot=malloc(46080);if(!state||!snapshot)fail("protocol-allocation",1,0);
    uint8_t output[300],twice[40];memcpy(twice,empty,20);memcpy(twice+20,empty,20);
    uint64_t before_reads=state_reads,before_writes=state_writes,before_offsets=offset_calls;
    if(sengoo_sg_gzip_stream_init(NULL,46079,0,0,0)!=-3||sengoo_sg_gzip_stream_init(NULL,46080,3,0,0)!=-3||sengoo_sg_gzip_stream_init(NULL,46080,0,-1,0)!=-3||sengoo_sg_gzip_stream_init(NULL,46080,0,67108865,0)!=-3||sengoo_sg_gzip_stream_init(NULL,46080,0,0,536870913)!=-3)fail("bad-init",-3,0);
    if(sengoo_sg_gzip_stream_query(NULL,46080,8)!=-3||sengoo_sg_gzip_stream_query(NULL,46079,0)!=-3||sengoo_sg_gzip_stream_dispose(NULL,46079)!=-3)fail("bad-query-dispose",-3,0);
    if(sengoo_sg_gzip_stream_step(NULL,46080,NULL,-1,NULL,0,0)!=-3||sengoo_sg_gzip_stream_step(NULL,46080,NULL,0,NULL,67108865,0)!=-3||sengoo_sg_gzip_stream_step(NULL,46080,NULL,0,NULL,0,2)!=-3)fail("bad-step",-3,0);
    if(state_reads!=before_reads||state_writes!=before_writes||offset_calls!=before_offsets)fail("bad-api-access",0,1);
    init(state,0);for(int field=0;field<8;field++)if(field!=6&&sengoo_sg_gzip_stream_query(state,46080,field)!=0)fail("fresh-query",0,field);
    if(step(state,twice,40,NULL,0,0)!=2||current_reads!=20||current_writes||sengoo_sg_gzip_stream_query(state,46080,7)!=1)fail("first-member-boundary",20,current_reads);
    init(state,1);if(step(state,twice,40,NULL,0,1)!=-1||current_reads!=20)fail("strict-tail",-1,0);
    init(state,2);if(step(state,empty,20,NULL,0,0)!=0||sengoo_sg_gzip_stream_query(state,46080,7)!=1)fail("concat-await",0,1);
    if(step(state,empty,20,NULL,0,1)!=2||sengoo_sg_gzip_stream_query(state,46080,7)!=2)fail("concat-members",2,0);
    init(state,1);if(step(state,empty,19,NULL,0,0)!=0||sengoo_sg_gzip_stream_query(state,46080,7)!=0)fail("partial-footer-not-member",0,1);
    if(step(state,empty+19,1,NULL,0,1)!=2||sengoo_sg_gzip_stream_query(state,46080,7)!=1)fail("footer-complete",1,0);
    for(int mode=0;mode<3;mode++){init(state,mode);if(step(state,NULL,0,NULL,0,1)!=-1)fail("empty-stream-reject",-1,mode);}
    init(state,0);if(step(state,repeated,sizeof repeated,NULL,0,1)!=1||current_writes)fail("zero-cap-pending",1,0);
    int64_t used=current_reads;
    if(step(state,repeated+used,sizeof repeated-used,output,1,0)!=-3)fail("sticky-finish",-3,0);
    int64_t got=0;while(1){int64_t result=step(state,repeated+used,sizeof repeated-used,output+got,1,1);used+=current_reads;got+=current_writes;if(result==2)break;if(result!=1)fail("finish-retry",1,result);}
    if(got!=259||used!=sizeof repeated)fail("zero-cap-loss",259,got);for(int i=0;i<259;i++)if(output[i]!=65)fail("zero-cap-bytes",65,output[i]);
    init(state,0);if(step(state,repeated,13,output,1,0)!=1||current_reads!=13||current_writes!=1)fail("drain-prepare",13,current_reads);
    memcpy(snapshot,state,46080);uint8_t initialized_copy[46080],history_copy[32768];memcpy(initialized_copy,initialized,sizeof initialized);memcpy(history_copy,history_initialized,sizeof history_initialized);
    for(int copy=0;copy<2;copy++){
        uint8_t *selected=copy?snapshot:state;memcpy(initialized,initialized_copy,sizeof initialized);memcpy(history_initialized,history_copy,sizeof history_initialized);got=1;
        while(1){int64_t result=step(selected,NULL,0,output,7,0);got+=current_writes;if(result==0)break;if(result!=1)fail("drain-status",1,result);}
        if(got!=259)fail("empty-input-match-drain",259,got);
        if(step(selected,repeated+13,sizeof repeated-13,output,300,1)!=2||current_writes)fail("snapshot-final",2,0);
    }
    state_owner=state;if(sengoo_sg_gzip_stream_reset(state,46080,0,67108864,536870912))fail("reset",0,1);
    for(int field=0;field<8;field++)if(field!=6&&sengoo_sg_gzip_stream_query(state,46080,field)!=0)fail("reset-query",0,field);
    if(sengoo_sg_gzip_stream_dispose(state,46080))fail("dispose",0,1);for(int i=0;i<46080;i++)if(state[i])fail("erase",0,state[i]);
    if(sengoo_sg_gzip_stream_query(state,46080,4)!=-4)fail("disposed-query",-4,0);free(state);free(snapshot);
}
int main(int argc,char **argv){
#ifndef SENGOO_GZS_PRODUCTION
    if(argc==2&&!strcmp(argv[1],"--uninitialized-control")){
        uint8_t *state=malloc(46080);if(!state)return 11;init(state,0);(void)sengoo_ds_state_byte(state+1024,10520);return 12;
    }
#endif
    if(argc!=2&&argc!=3)return 2;FILE *file=NULL;if(fopen_s(&file,argv[1],"rb")||!file)return 3;fseek(file,0,SEEK_END);long length=ftell(file);rewind(file);if(length<16)return 4;
    uint8_t *raw=malloc((size_t)length);if(!raw||fread(raw,1,length,file)!=(size_t)length)return 5;fclose(file);
    const uint8_t *at=raw,*end=raw+length;if(memcmp(at,"SGGZP1\0\0",8))return 6;at+=8;uint64_t count=number(&at,end,8);
    uint32_t *member_counts=NULL;
    if(argc==3){if(fopen_s(&file,argv[2],"rb")||!file)return 7;member_counts=malloc((size_t)count*4);if(!member_counts||fread(member_counts,4,(size_t)count,file)!=count||fgetc(file)!=EOF)return 8;fclose(file);}
    protocol();
    for(fixture=0;fixture<count;fixture++){
        int64_t n=number(&at,end,4),capacity=number(&at,end,4),expected=(int64_t)number(&at,end,8),size=number(&at,end,4),mode=number(&at,end,4);
        if(end-at<n+size)fail("corpus-record",n+size,end-at);
        size_t rounded=((size_t)n+4095)/4096*4096;if(!rounded)rounded=4096;
        uint8_t *allocation=VirtualAlloc(NULL,rounded+8192,MEM_RESERVE,PAGE_NOACCESS);if(!allocation)fail("input-reserve",1,0);
        uint8_t *page=VirtualAlloc(allocation+4096,rounded,MEM_COMMIT,PAGE_READWRITE);if(!page)fail("input-commit",1,0);
        uint8_t *source=page+rounded-n;if(n)memcpy(source,at,(size_t)n);DWORD old;if(!VirtualProtect(page,rounded,PAGE_READONLY,&old))fail("input-readonly",1,0);
        int64_t expected_members=member_counts?member_counts[fixture]:-1;
        for(int schedule=0;schedule<3;schedule++)check(source,n,at+n,size,expected,expected==-2?capacity:67108864,mode,schedule,expected_members);
        if(expected>=0&&n<=96)for(int cut=1;cut<n;cut++)check(source,n,at+n,size,expected,67108864,mode,cut+2,expected_members);
        VirtualFree(allocation,0,MEM_RELEASE);at+=n+size;
    }
    if(at!=end)fail("corpus-tail",0,end-at);
#ifdef SENGOO_GZS_PRODUCTION
    printf("{\"schema\":1,\"ok\":true,\"mode\":\"production\",\"fixtures\":%llu,\"runs\":%llu,\"calls\":%llu,\"need_input\":%llu,\"need_output\":%llu,\"history_reads\":null,\"crc_output_reads\":null,\"state_bytes\":46080,\"completed_fnv64\":\"%016llx\"}\n",(unsigned long long)count,(unsigned long long)runs,(unsigned long long)calls,(unsigned long long)need_inputs,(unsigned long long)need_outputs,(unsigned long long)completed);
#else
    printf("{\"schema\":1,\"ok\":true,\"mode\":\"instrumented\",\"fixtures\":%llu,\"runs\":%llu,\"calls\":%llu,\"need_input\":%llu,\"need_output\":%llu,\"history_reads\":%llu,\"crc_output_reads\":%llu,\"state_bytes\":46080,\"completed_fnv64\":\"%016llx\"}\n",(unsigned long long)count,(unsigned long long)runs,(unsigned long long)calls,(unsigned long long)need_inputs,(unsigned long long)need_outputs,(unsigned long long)history_reads,(unsigned long long)crc_output_reads,(unsigned long long)completed);
#endif
    free(member_counts);free(raw);return 0;
}
