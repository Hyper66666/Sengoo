#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>
extern int64_t sengoo_sg_tar_stream_init(uint8_t *,int64_t);
extern int64_t sengoo_sg_tar_stream_step(uint8_t *,int64_t,const uint8_t *,int64_t,int64_t);
extern int64_t sengoo_sg_tar_stream_query(uint8_t *,int64_t,int64_t);
extern int64_t sengoo_sg_tar_stream_ack(uint8_t *,int64_t,int64_t);
extern int64_t sengoo_sg_tar_stream_dispose(uint8_t *,int64_t);
static uint8_t *owner,*initialized;
static uint8_t *owner_allocation;
static uint8_t generation;
static const uint8_t *input;
static int64_t input_size;
static unsigned fixture;
void sengoo_tar_observe_os_compare(void){}
static uint64_t calls,runs,entry_events,data_events,reads,writes;
static void fail(const char *reason,int64_t expected,int64_t actual){fprintf(stderr,"fixture=%u reason=%s expected=%lld actual=%lld\n",fixture,reason,(long long)expected,(long long)actual);exit(1);}
static int64_t range(const uint8_t *p,int64_t index,int bytes){uintptr_t b=(uintptr_t)owner,a=(uintptr_t)p;if(a<b||a-b>18438400||18438400-(a-b)<(unsigned)bytes||index<0||(uint64_t)index>(18438400-(a-b)-bytes)/(unsigned)bytes)fail("state-bound",18438400,index);return (int64_t)(a-b)+index*bytes;}
uint8_t sengoo_tar_read(const uint8_t *p,int64_t index){
    uintptr_t b=(uintptr_t)input,a=(uintptr_t)p;
    if(a>=b&&a-b<=(uint64_t)input_size){if(index<0||(uint64_t)index>=(uint64_t)input_size-(a-b))fail("input-bound",input_size,index);++reads;return p[index];}
    int64_t at=range(p,index,1);if(initialized[at]!=generation)fail("state-uninitialized",1,at);++reads;return p[index];
}
uint8_t sengoo_tar_output_read(uint8_t *p,int64_t index){int64_t at=range(p,index,1);if(initialized[at]!=generation)fail("state-uninitialized",1,at);++reads;return p[index];}
void sengoo_tar_write(uint8_t *p,int64_t index,uint8_t value){int64_t at=range(p,index,1);initialized[at]=generation;++writes;p[index]=value;}
int64_t sengoo_ds_word_get(uint8_t *p,int64_t index){int64_t at=range(p,index,8);for(int i=0;i<8;i++)if(initialized[at+i]!=generation)fail("word-uninitialized",1,at+i);int64_t value;memcpy(&value,p+index*8,8);++reads;return value;}
void sengoo_ds_word_put(uint8_t *p,int64_t index,int64_t value){int64_t at=range(p,index,8);memset(initialized+at,generation,8);memcpy(p+index*8,&value,8);++writes;}
uint8_t *sengoo_ts_mut_offset(uint8_t *p,int64_t offset){(void)range(p,offset,1);return p+offset;}
const uint8_t *sengoo_ts_const_offset(uint8_t *p,int64_t offset){(void)range(p,offset,1);return p+offset;}
static uint64_t word(const uint8_t *p){uint64_t result=0;for(unsigned i=0;i<8;i++)result|=(uint64_t)p[i]<<(8*i);return result;}
static uint64_t number(const uint8_t **p,const uint8_t *end,unsigned n){if(end-*p<(int64_t)n)fail("corpus-short",n,end-*p);uint64_t value=0;for(unsigned i=0;i<n;i++)value|=(uint64_t)*(*p)++<<(8*i);return value;}
static unsigned protocols;
static void protocol(const uint8_t *source,int64_t length){
    ++generation;if(!generation){memset(initialized,0,18438400);generation=1;}
    uint64_t r=reads,w=writes;
    if(sengoo_sg_tar_stream_init(NULL,0)!=-3||sengoo_sg_tar_stream_step(NULL,0,NULL,0,0)!=-3||sengoo_sg_tar_stream_query(NULL,0,0)!=-3||sengoo_sg_tar_stream_dispose(NULL,0)!=-3||sengoo_sg_tar_stream_ack(NULL,0,0)!=-3||reads!=r||writes!=w)fail("invalid-extent-access",0,1);protocols++;
    if(sengoo_sg_tar_stream_init(owner,18438400))fail("protocol-init",0,1);
    r=reads;w=writes;
    if(sengoo_sg_tar_stream_query(owner,18438400,15)!=-3||sengoo_sg_tar_stream_step(owner,18438400,NULL,-1,0)!=-3||sengoo_sg_tar_stream_step(owner,18438400,NULL,0,2)!=-3||reads!=r||writes!=w)fail("invalid-field-access",0,1);protocols++;
    input=NULL;input_size=0;if(sengoo_sg_tar_stream_step(owner,18438400,NULL,0,0)!=0)fail("zero-input",0,1);protocols++;
    input=source;input_size=length;
    if(sengoo_sg_tar_stream_step(owner,18438400,source,length,1)!=3)fail("first-entry",3,0);
    int64_t used=sengoo_sg_tar_stream_query(owner,18438400,0);r=reads;w=writes;
    if(sengoo_sg_tar_stream_step(owner,18438400,NULL,0,1)!=-3||sengoo_sg_tar_stream_query(owner,18438400,0)!=used)fail("unacked-event",-3,0);protocols++;
    if(sengoo_sg_tar_stream_ack(owner,18438400,-8)!=-8||sengoo_sg_tar_stream_step(owner,18438400,NULL,0,1)!=-8)fail("io-latched",-8,0);protocols++;
    if(sengoo_sg_tar_stream_init(owner,18438400)||sengoo_sg_tar_stream_step(owner,18438400,source,1,1)!=-1||sengoo_sg_tar_stream_step(owner,18438400,NULL,0,0)!=-3)fail("sticky-finish",-3,0);protocols++;
    if(sengoo_sg_tar_stream_init(owner,18438400)||sengoo_sg_tar_stream_step(owner,18438400,source,length,0)!=3||sengoo_sg_tar_stream_ack(owner,18438400,-9)!=-9||sengoo_sg_tar_stream_step(owner,18438400,NULL,0,0)!=-9)fail("cancel-latched",-9,0);protocols++;
    if(sengoo_sg_tar_stream_dispose(owner,18438400)||sengoo_sg_tar_stream_query(owner,18438400,0)!=-10)fail("disposed-query",-10,0);protocols++;
}
static void check(const uint8_t *source,int64_t length,int64_t expected,const uint8_t *table,int64_t table_size,const uint8_t *paths,int64_t path_size,int schedule){
    ++generation;if(!generation){memset(initialized,0,18438400);generation=1;};if(sengoo_sg_tar_stream_init(owner,18438400))fail("init",0,1);
    int64_t used=0,end=0,entries=0,body=0,entry_size=0,status=0;uint64_t random=0x7612abc3+fixture;unsigned attempts=0;
    while(attempts++<30000000){
        if(used==end&&end<length){random=random*6364136223846793005ULL+1;int64_t chunk=schedule==0&&length<131072?1:schedule==1?(int64_t)(random%997)+1:schedule>=3&&used==0?schedule-2:32749;end=used+chunk;if(end>length)end=length;}
        input=source+used;input_size=end-used;int64_t previous=used;
        status=sengoo_sg_tar_stream_step(owner,18438400,input,input_size,end==length);++calls;int64_t consumed=sengoo_sg_tar_stream_query(owner,18438400,0);
        if(consumed<0||consumed>input_size)fail("consumed-bound",input_size,consumed);used+=consumed;
        if(sengoo_sg_tar_stream_query(owner,18438400,1)!=used)fail("wire-count",used,sengoo_sg_tar_stream_query(owner,18438400,1));
        if(status==3){
            ++entry_events;body=0;entry_size=sengoo_sg_tar_stream_query(owner,18438400,10);
            if(expected>=0){
                if(entries>=expected||table_size<32+(entries+1)*48)fail("entry-count",expected,entries);
                const uint8_t *row=table+32+entries*48;int64_t start=sengoo_sg_tar_stream_query(owner,18438400,7),n=sengoo_sg_tar_stream_query(owner,18438400,8),kind=sengoo_sg_tar_stream_query(owner,18438400,9);
                if(start<1661184||n<1||n>65535||start+n>18438400||n!=(int64_t)word(row+32)||kind!=(int64_t)word(row+40)||entry_size!=(int64_t)word(row+16)||used!=(int64_t)word(row+8)||memcmp(owner+start,paths+word(row+24),(size_t)n))fail("entry-metadata",entries,0);
            }
            ++entries;if(sengoo_sg_tar_stream_ack(owner,18438400,0))fail("entry-ack",0,1);
        } else if(status==4){
            ++data_events;int64_t at=sengoo_sg_tar_stream_query(owner,18438400,11),n=sengoo_sg_tar_stream_query(owner,18438400,12);
            if(at<0||n<1||at+n>input_size||body>entry_size-n)fail("data-event-bound",entry_size,body+n);
            if(expected>=0){const uint8_t *row=table+32+(entries-1)*48;if(memcmp(input+at,source+word(row+8)+body,(size_t)n))fail("data-event-bytes",0,1);}
            body+=n;if(sengoo_sg_tar_stream_ack(owner,18438400,0))fail("data-ack",0,1);
        } else if(status==0){if(end==length||used==previous&&input_size>0)fail("need-input-stall",1,0);}
        else break;
    }
    if(expected>=0){if(status!=2||entries!=expected||used!=length||sengoo_sg_tar_stream_query(owner,18438400,2)!=(int64_t)word(table+16))fail("valid-terminal",expected,status);}
    else if(status!=expected)fail("negative-terminal",expected,status);
    if(sengoo_sg_tar_stream_dispose(owner,18438400)||sengoo_sg_tar_stream_query(owner,18438400,0)!=-10)fail("dispose",-10,0);++runs;(void)path_size;
}
int main(int argc,char **argv){
    if(argc==2&&!strcmp(argv[1],"--uninitialized-control")){
        owner=VirtualAlloc(NULL,18438400,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);initialized=calloc(18438400,1);generation=1;
        if(!owner||!initialized)return 6;sengoo_tar_output_read(owner,1024);return 99;
    }
    if(argc!=2)return 2;FILE *file=NULL;if(fopen_s(&file,argv[1],"rb")||!file)return 3;fseek(file,0,SEEK_END);long length=ftell(file);rewind(file);uint8_t *raw=malloc(length);if(!raw||fread(raw,1,length,file)!=(size_t)length)return 4;fclose(file);
    const uint8_t *at=raw,*end=raw+length;if(memcmp(at,"SGTS1\0\0\0",8))return 5;at+=8;uint64_t count=number(&at,end,4);
    SIZE_T padded=(18438400+4095)&~(SIZE_T)4095;
    owner_allocation=VirtualAlloc(NULL,padded+8192,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);initialized=calloc(18438400,1);DWORD old;
    if(!owner_allocation||!initialized||!VirtualProtect(owner_allocation+4096,padded,PAGE_READWRITE,&old))return 6;owner=owner_allocation+4096+padded-18438400;
    for(fixture=0;fixture<count;fixture++){
        int64_t n=number(&at,end,4),expected=(int64_t)number(&at,end,8),table_size=number(&at,end,4),path_size=number(&at,end,4);if(end-at<n+table_size+path_size)fail("corpus-record",n,end-at);
        const uint8_t *table=at+n,*paths=table+table_size;
        SIZE_T rounded=((SIZE_T)n+4095)&~(SIZE_T)4095;uint8_t *allocation=VirtualAlloc(NULL,rounded+8192,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);
        if(!allocation||rounded&&!VirtualProtect(allocation+4096,rounded,PAGE_READWRITE,&old))return 7;
        uint8_t *guarded=allocation+4096+rounded-n;if(n){memcpy(guarded,at,n);if(!VirtualProtect(allocation+4096,rounded,PAGE_READONLY,&old))return 8;}
        if(fixture==0)protocol(guarded,n);
        for(int schedule=0;schedule<3;schedule++)check(guarded,n,expected,table,table_size,paths,path_size,schedule);
        if(expected>=0&&n<=4096)for(int cut=1;cut<n;cut++)check(guarded,n,expected,table,table_size,paths,path_size,cut+2);
        if(n&&memcmp(guarded,at,n))fail("readonly-input-changed",0,1);VirtualFree(allocation,0,MEM_RELEASE);
        at+=n+table_size+path_size;
    }
    if(at!=end)fail("corpus-tail",0,end-at);
    printf("{\"schema\":1,\"ok\":true,\"fixtures\":%llu,\"runs\":%llu,\"calls\":%llu,\"entry_events\":%llu,\"data_events\":%llu,\"reads\":%llu,\"writes\":%llu,\"state_bytes\":18438400,\"protocols\":%u,\"readonly_input\":true,\"input_tailguard\":true,\"state_tailguard\":true,\"initialized_byte_checks\":true}\n",(unsigned long long)count,(unsigned long long)runs,(unsigned long long)calls,(unsigned long long)entry_events,(unsigned long long)data_events,(unsigned long long)reads,(unsigned long long)writes,protocols);
    VirtualFree(owner_allocation,0,MEM_RELEASE);free(initialized);free(raw);return 0;
}
