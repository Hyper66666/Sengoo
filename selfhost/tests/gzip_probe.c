/* Expected-byte/extent consumer only. SG implements all framing and CRC policy. */
#define SENGOO_DEFLATE_PRODUCTION_MEMORY
#define main sh_gzip_unused_raw_probe_main
#include "deflate_probe.c"
#undef main
extern int64_t sengoo_sg_gzip_decode(const uint8_t *,int64_t,uint8_t *,int64_t,int64_t);

#ifndef SENGOO_GZIP_PRODUCTION_MEMORY
static uint64_t gzip_offset(const uint8_t *pointer,int64_t index,const uint8_t *base,uint64_t extent,int one_past) {
    uintptr_t start=(uintptr_t)base,supplied=(uintptr_t)pointer;
    if(supplied<start||supplied-start>extent||index<0||(uint64_t)index>extent-(supplied-start)||(!one_past&&(uint64_t)index==extent-(supplied-start)))fail("gzip-pointer-range",(int64_t)extent,index);
    return (uint64_t)(supplied-start)+(uint64_t)index;
}
uint8_t sengoo_deflate_input_read(const uint8_t *source,int64_t index) {
    uint64_t at=gzip_offset(source,index,active_input,input_extent,0);
    if(at!=current_input_reads)fail("gzip-input-sequence",(int64_t)current_input_reads,(int64_t)at);
    ++input_reads;++current_input_reads;return source[index];
}
uint8_t sengoo_deflate_output_read(uint8_t *target,int64_t index) {
    uint64_t at=gzip_offset(target,index,active_output,output_extent,0);
    if(at>=current_writes)fail("gzip-uninitialized-output",(int64_t)current_writes,(int64_t)at);
    ++output_reads;return target[index];
}
void sengoo_deflate_output_write(uint8_t *target,int64_t index,uint8_t value) {
    uint64_t at=gzip_offset(target,index,active_output,output_extent,0);
    if(at!=current_writes)fail("gzip-nonsequential-write",(int64_t)current_writes,(int64_t)at);
    ++output_writes;++current_writes;target[index]=value;
}
const uint8_t *sengoo_gzip_input_offset(const uint8_t *source,int64_t index) {
    (void)gzip_offset(source,index,active_input,input_extent,1);return index==0?source:source+index;
}
uint8_t *sengoo_gzip_output_offset(uint8_t *target,int64_t index) {
    (void)gzip_offset(target,index,active_output,output_extent,1);return index==0?target:target+index;
}
#endif
static void gzip_check(const uint8_t *source,uint32_t length,uint8_t *target,uint32_t capacity,int64_t expected,const uint8_t *plain,uint32_t plain_length,int64_t mode) {
    active_input=source;input_extent=length;active_output=target;output_extent=capacity;current_input_reads=current_writes=0;
    int64_t actual=sengoo_sg_gzip_decode(source,length,target,capacity,mode);
    if(actual!=expected)fail("gzip-result-oracle",expected,actual);
    if(expected>=0) {
        if((expected&134217727)!=plain_length || ((uint64_t)expected>>27)>length || (plain_length&&memcmp(target,plain,plain_length)))fail("gzip-byte-count-oracle",expected,actual);
#ifndef SENGOO_GZIP_PRODUCTION_MEMORY
        if(current_writes!=plain_length||current_input_reads!=((uint64_t)expected>>27))fail("gzip-consumed-count",expected,(int64_t)current_input_reads);
#endif
        ++successes;
    } else {
        if(expected==-1)++malformed;else if(expected==-2)++capacities;else fail("gzip-expected-error",-1,expected);
#ifndef SENGOO_GZIP_PRODUCTION_MEMORY
        if(plain_length && (current_writes>plain_length||memcmp(target,plain,(size_t)current_writes)))fail("gzip-known-prefix",0,1);
#endif
    }
    ++passed;
}
static void gzip_guard(const uint8_t *source,uint32_t length,const uint8_t *plain,uint32_t capacity,int64_t expected,uint32_t plain_length,int64_t mode) {
    SYSTEM_INFO info;GetSystemInfo(&info);size_t page=info.dwPageSize;
    size_t input_round=((size_t)length+page-1)/page*page,output_round=((size_t)capacity+page-1)/page*page;
    if(!input_round)input_round=page;if(!output_round)output_round=page;
    for(unsigned side=0;side<2;++side) {
        uint8_t *a=VirtualAlloc(NULL,input_round+2*page,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);
        uint8_t *b=VirtualAlloc(NULL,output_round+2*page,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);DWORD old;
        if(!a||!b||!VirtualProtect(a+page,input_round,PAGE_READWRITE,&old)||!VirtualProtect(b+page,output_round,PAGE_READWRITE,&old))fail("gzip-guard-allocation",1,0);
        uint8_t *input=side?a+page:a+page+input_round-length,*output=side?b+page:b+page+output_round-capacity;
        memcpy(input,source,length);if(!VirtualProtect(a+page,input_round,PAGE_READONLY,&old))fail("gzip-readonly",1,0);
        gzip_check(input,length,output,capacity,expected,plain,plain_length,mode);
        if(!VirtualFree(a,0,MEM_RELEASE)||!VirtualFree(b,0,MEM_RELEASE))fail("gzip-guard-free",1,0);
        ++guard_cases;
    }
}
static int gzip_large(const char *directory) {
    SYSTEM_INFO info;GetSystemInfo(&info);size_t page=info.dwPageSize;
    for(unsigned test=0;test<4;++test) {
        record_id=test;char path[32768];
        if(snprintf(path,sizeof path,"%s/%s",directory,test==3?"large-input.gzip":"large-concat.gzip")<0)fail("gzip-large-path",1,0);
        FILE *file=NULL;if(fopen_s(&file,path,"rb")||!file)fail("gzip-large-open",1,0);
        if(_fseeki64(file,0,SEEK_END))fail("gzip-large-seek",1,0);
        int64_t length=_ftelli64(file);if(length<1||length>67108864||_fseeki64(file,0,SEEK_SET))fail("gzip-large-length",1,length);
        size_t rounded=((size_t)length+page-1)/page*page;DWORD old;
        uint8_t *a=VirtualAlloc(NULL,rounded+2*page,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);
        if(!a||!VirtualProtect(a+page,rounded,PAGE_READWRITE,&old))fail("gzip-large-input-allocation",1,0);
        uint8_t *input=a+page+rounded-(size_t)length;exact(file,input,(size_t)length);
        if(fgetc(file)!=EOF||fclose(file)||!VirtualProtect(a+page,rounded,PAGE_READONLY,&old))fail("gzip-large-input-readonly",1,0);
        uint64_t cap=test==1?67108863:test==2?1048576:67108864;size_t out_round=((size_t)cap+page-1)/page*page;
        uint8_t *b=VirtualAlloc(NULL,out_round+2*page,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);
        if(!b||!VirtualProtect(b+page,out_round,PAGE_READWRITE,&old))fail("gzip-large-output-allocation",1,0);
        uint8_t *output=b+page+out_round-(size_t)cap;
        active_input=input;input_extent=(uint64_t)length;active_output=output;output_extent=cap;current_writes=current_input_reads=0;
        int64_t used=test==2?length/64:length,count=test==3?67103726:test==2?1048576:67108864;
        int64_t expected=test==1?-2:(used<<27)|count;
        int64_t actual=sengoo_sg_gzip_decode(input,length,output,(int64_t)cap,test==3?1:test==2?0:2);
        if(actual!=expected)fail("gzip-large-result",expected,actual);
        uint64_t check_bytes=expected>=0?(uint64_t)count:current_writes;
        for(uint64_t i=0;i<check_bytes;++i)if(output[i]!=(test==3?(uint8_t)(i%251):65))fail("gzip-large-bytes",test==3?(int64_t)(i%251):65,output[i]);
#ifndef SENGOO_GZIP_PRODUCTION_MEMORY
        if(expected>=0&&(current_input_reads!=(uint64_t)used||current_writes!=(uint64_t)count))fail("gzip-large-consumption",used,(int64_t)current_input_reads);
#endif
        if(!VirtualFree(a,0,MEM_RELEASE)||!VirtualFree(b,0,MEM_RELEASE))fail("gzip-large-free",1,0);
    }
    printf("{\"schema\":1,\"ok\":true,\"large_cases\":4,\"aggregate_members\":64,\"aggregate_decoded_bytes\":67108864,\"capacity_failure\":true,\"first_member_exact\":true,\"input_limit_bytes\":67108864,\"max_input_output_bytes\":67103726,\"readonly_input_tailguard\":true,\"output_tailguard\":true");
#ifdef SENGOO_GZIP_PRODUCTION_MEMORY
    printf(",\"instrumented\":false,\"complete_sequential_input_output\":null,\"input_reads\":null,\"initialized_output_reads\":null,\"writes\":null}\n");
#else
    printf(",\"instrumented\":true,\"complete_sequential_input_output\":true,\"input_reads\":%" PRIu64 ",\"initialized_output_reads\":%" PRIu64 ",\"writes\":%" PRIu64 "}\n",input_reads,output_reads,output_writes);
#endif
    return 0;
}
#ifndef SENGOO_GZIP_PROBE_MAIN
#define SENGOO_GZIP_PROBE_MAIN main
#endif
int SENGOO_GZIP_PROBE_MAIN(int argc,char **argv) {
    if(argc==3&&!strcmp(argv[1],"--large"))return gzip_large(argv[2]);
    if(argc!=2)return 64;
    FILE *file=NULL;if(fopen_s(&file,argv[1],"rb")||!file)fail("gzip-corpus-open",1,0);
    uint8_t header[24];exact(file,header,16);if(memcmp(header,"SGGZP1\0\0",8))fail("gzip-corpus-magic",1,0);
    uint64_t records=little(header+8,8);if(!records||records>100000)fail("gzip-record-count",1,(int64_t)records);
    for(record_id=0;record_id<records;++record_id) {
        exact(file,header,24);uint32_t length=(uint32_t)little(header,4),capacity=(uint32_t)little(header+4,4),plain_length=(uint32_t)little(header+16,4),mode=(uint32_t)little(header+20,4);int64_t expected=(int64_t)little(header+8,8);
        if(length>1048576||capacity>67108864||plain_length>1048576||mode>2||(expected>=0&&plain_length>capacity))fail("gzip-corpus-limits",1,0);
        uint8_t *input=malloc((size_t)length+32),*saved=malloc((size_t)length+1),*plain=malloc((size_t)plain_length+1),*output=malloc((size_t)capacity+32);
        if(!input||!saved||!plain||!output)fail("gzip-allocation",1,0);
        memset(input,0x5a,(size_t)length+32);memset(output,0xa5,(size_t)capacity+32);exact(file,input+16,length);memcpy(saved,input+16,length);exact(file,plain,plain_length);
        gzip_check(input+16,length,output+16,capacity,expected,plain,plain_length,mode);
        if(length&&memcmp(input+16,saved,length))fail("gzip-input-modified",0,1);
        for(unsigned i=0;i<16;++i)if(input[i]!=0x5a||input[length+16+i]!=0x5a||output[i]!=0xa5||output[capacity+16+i]!=0xa5)fail("gzip-canary",0,1);
        if(length<=131072&&capacity<=4096)gzip_guard(input+16,length,plain,capacity,expected,plain_length,mode);
        free(input);free(saved);free(plain);free(output);
    }
    if(fgetc(file)!=EOF||ferror(file)||fclose(file))fail("gzip-corpus-trailing",0,1);
    void *unreadable=VirtualAlloc(NULL,4096,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);if(!unreadable)fail("gzip-limit-allocate",1,0);
    active_input=unreadable;active_output=unreadable;input_extent=output_extent=current_writes=current_input_reads=0;
    int64_t limits[]={INT64_MIN,-1,67108865,INT64_MAX};
    for(unsigned i=0;i<4;++i)for(unsigned side=0;side<2;++side) {
        uint64_t before=input_reads+output_reads+output_writes;int64_t value=sengoo_sg_gzip_decode(unreadable,side?7:limits[i],unreadable,side?limits[i]:7,0);
        if(value!=-3||input_reads+output_reads+output_writes!=before)fail("gzip-invalid-limits",-3,value);++invalid_limits;
    }
    for(int64_t mode=-1;mode<=3;mode+=4){int64_t value=sengoo_sg_gzip_decode(unreadable,7,unreadable,7,mode);if(value!=-3)fail("gzip-invalid-mode",-3,value);++invalid_limits;}
    if(!VirtualFree(unreadable,0,MEM_RELEASE))fail("gzip-limit-free",1,0);
    const uint8_t empty[]={31,139,8,0,0,0,0,0,0,255,3,0,0,0,0,0,0,0,0,0};
    active_input=empty;input_extent=sizeof empty;active_output=NULL;output_extent=current_writes=current_input_reads=0;
    int64_t null_result=sengoo_sg_gzip_decode(empty,sizeof empty,NULL,0,0);if(null_result!=((int64_t)sizeof empty<<27))fail("gzip-null-empty",(int64_t)sizeof empty<<27,null_result);
    printf("{\"schema\":1,\"ok\":true,\"records\":%" PRIu64 ",\"calls\":%" PRIu64 ",\"successes\":%" PRIu64 ",\"malformed\":%" PRIu64 ",\"capacity_failures\":%" PRIu64 ",\"guard_cases\":%" PRIu64 ",\"invalid_limits_modes\":%" PRIu64 ",\"null_empty_output\":true,\"input_unchanged\":true",records,passed,successes,malformed,capacities,guard_cases,invalid_limits);
#ifdef SENGOO_GZIP_PRODUCTION_MEMORY
    printf(",\"instrumented\":false,\"sequential_bounded_access\":null,\"input_reads\":null,\"initialized_output_reads\":null,\"writes\":null}\n");
#else
    printf(",\"instrumented\":true,\"sequential_bounded_access\":true,\"input_reads\":%" PRIu64 ",\"initialized_output_reads\":%" PRIu64 ",\"writes\":%" PRIu64 "}\n",input_reads,output_reads,output_writes);
#endif
    return 0;
}
