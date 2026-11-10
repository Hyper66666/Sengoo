/* Independent expected-byte consumer and authorized-range instrumentation.
 * No compression/decompression algorithm is implemented here. */
#include <stdint.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

extern int64_t sengoo_sg_raw_inflate(const uint8_t *, int64_t, uint8_t *, int64_t);
static const uint8_t *active_input;
static uint8_t *active_output;
static uint64_t input_extent,output_extent,input_reads,output_reads,output_writes,current_writes,current_input_reads;
static uint64_t record_id,passed,successes,malformed,capacities,invalid_limits,guard_cases;

static void fail(const char *reason,int64_t expected,int64_t actual) {
    fprintf(stderr,"deflate failure case=%" PRIu64 " reason=%s expected=%" PRId64 " actual=%" PRId64 "\n",record_id,reason,expected,actual);
    exit(1);
}
#ifndef SENGOO_DEFLATE_PRODUCTION_MEMORY
uint8_t sengoo_deflate_input_read(const uint8_t *source,int64_t index) {
    if(source!=active_input || index<0 || (uint64_t)index>=input_extent || (uint64_t)index!=current_input_reads)fail("input-range-or-sequence",(int64_t)input_extent,index);
    ++input_reads;++current_input_reads;return source[index];
}
uint8_t sengoo_deflate_output_read(uint8_t *target,int64_t index) {
    if(target!=active_output || index<0 || (uint64_t)index>=current_writes)fail("history-range",(int64_t)current_writes,index);
    ++output_reads;return target[index];
}
void sengoo_deflate_output_write(uint8_t *target,int64_t index,uint8_t value) {
    if(target!=active_output || index<0 || (uint64_t)index>=output_extent || (uint64_t)index!=current_writes)fail("output-range-or-sequence",(int64_t)output_extent,index);
    ++output_writes;++current_writes;target[index]=value;
}
#endif

static void check(const uint8_t *source,uint64_t input_length,uint8_t *target,uint64_t capacity,int64_t expected,const uint8_t *plain,uint64_t plain_length) {
    active_input=source;active_output=target;input_extent=input_length;output_extent=capacity;current_writes=current_input_reads=0;
    uint64_t initial_reads=input_reads;
    int64_t actual=sengoo_sg_raw_inflate(source,(int64_t)input_length,target,(int64_t)capacity);
    if(actual!=expected)fail("result-oracle",expected,actual);
    if(expected>=0) {
        if((uint64_t)expected!=plain_length || (plain_length && memcmp(target,plain,(size_t)plain_length)))fail("decoded-byte-oracle",expected,actual);
#ifndef SENGOO_DEFLATE_PRODUCTION_MEMORY
        if(current_writes!=plain_length)fail("successful-write-count",(int64_t)plain_length,(int64_t)current_writes);
        if(input_reads-initial_reads!=input_length)fail("successful-exact-input",(int64_t)input_length,(int64_t)(input_reads-initial_reads));
#endif
        ++successes;
    } else {
        if(expected==-1)++malformed;else if(expected==-2)++capacities;else fail("unexpected-negative",-1,expected);
#ifndef SENGOO_DEFLATE_PRODUCTION_MEMORY
        if(plain_length && (current_writes>plain_length || memcmp(target,plain,(size_t)current_writes)))fail("failure-prefix",0,1);
#endif
    }
#ifdef SENGOO_DEFLATE_PRODUCTION_MEMORY
    (void)initial_reads;
#endif
    ++passed;
}
static void exact(FILE *file,void *data,size_t size) {if(size && fread(data,1,size,file)!=size)fail("truncated-corpus",(int64_t)size,0);}
static uint64_t little(const uint8_t *data,unsigned width) {uint64_t value=0;for(unsigned i=0;i<width;++i)value|=(uint64_t)data[i]<<(i*8);return value;}

static void guards(const uint8_t *input,uint32_t length,const uint8_t *plain,uint32_t capacity,int64_t expected,uint32_t plain_length) {
    SYSTEM_INFO info;GetSystemInfo(&info);size_t page=info.dwPageSize;
    if(length>page || capacity>page)return;
    for(unsigned side=0;side<2;++side) {
        uint8_t *a=VirtualAlloc(NULL,page*3,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);
        uint8_t *b=VirtualAlloc(NULL,page*3,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);DWORD old;
        if(!a||!b || !VirtualProtect(a+page,page,PAGE_READWRITE,&old) || !VirtualProtect(b+page,page,PAGE_READWRITE,&old))fail("guard-allocation",1,0);
        uint8_t *source=side?a+page:a+2*page-length;
        uint8_t *target=side?b+page:b+2*page-capacity;
        memcpy(source,input,length);memset(b+page,0xa5,page);
        if(!VirtualProtect(a+page,page,PAGE_READONLY,&old))fail("guard-readonly",1,0);
        check(source,length,target,capacity,expected,plain,plain_length);
        if(length && memcmp(source,input,length))fail("guard-input-modified",0,1);
        if(!VirtualFree(a,0,MEM_RELEASE)||!VirtualFree(b,0,MEM_RELEASE))fail("guard-free",1,0);
        ++guard_cases;
    }
}

static int large(const char *directory) {
    SYSTEM_INFO info;GetSystemInfo(&info);size_t page=info.dwPageSize;
    for(unsigned test=0;test<3;++test) {
        record_id=test;
        char path[32768];const char *name=test==2?"large-input.deflate":"large-output.deflate";
        if(snprintf(path,sizeof path,"%s/%s",directory,name)<0)fail("large-path",1,0);
        FILE *file=NULL;if(fopen_s(&file,path,"rb")||!file)fail("large-open",1,0);
        if(_fseeki64(file,0,SEEK_END))fail("large-seek",0,1);
        int64_t length=_ftelli64(file);if(length<=0||length>67108864||_fseeki64(file,0,SEEK_SET))fail("large-input-size",1,length);
        size_t rounded=((size_t)length+page-1)/page*page;
        uint8_t *a=VirtualAlloc(NULL,rounded+2*page,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);DWORD old;
        if(!a||!VirtualProtect(a+page,rounded,PAGE_READWRITE,&old))fail("large-input-allocation",1,0);
        uint8_t *input=a+page+rounded-(size_t)length;exact(file,input,(size_t)length);
        if(fgetc(file)!=EOF||fclose(file)||!VirtualProtect(a+page,rounded,PAGE_READONLY,&old))fail("large-input-readonly",1,0);
        uint64_t capacity=test==1?67108863:67108864;
        size_t output_rounded=((size_t)capacity+page-1)/page*page;
        uint8_t *b=VirtualAlloc(NULL,output_rounded+2*page,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);
        if(!b||!VirtualProtect(b+page,output_rounded,PAGE_READWRITE,&old))fail("large-output-allocation",1,0);
        uint8_t *output=b+page+output_rounded-(size_t)capacity;
        active_input=input;active_output=output;input_extent=(uint64_t)length;output_extent=capacity;current_writes=current_input_reads=0;
        int64_t expected=test==1?-2:test==2?67103744:67108864;
        int64_t result=sengoo_sg_raw_inflate(input,length,output,(int64_t)capacity);
        if(result!=expected)fail("large-result",expected,result);
        uint64_t checked=expected>=0?(uint64_t)expected:current_writes;
        for(uint64_t i=0;i<checked;++i)if(output[i]!=(test==2?(uint8_t)(i%251):65))fail("large-decoded-byte",test==2?(int64_t)(i%251):65,output[i]);
#ifndef SENGOO_DEFLATE_PRODUCTION_MEMORY
        if(expected>=0 && (current_writes!=(uint64_t)expected || current_input_reads!=(uint64_t)length))fail("large-complete-coverage",expected,(int64_t)current_writes);
#endif
        if(expected>=0)++successes;else ++capacities;
        ++passed;++guard_cases;
        if(!VirtualFree(a,0,MEM_RELEASE)||!VirtualFree(b,0,MEM_RELEASE))fail("large-free",1,0);
    }
    printf("{\"schema\":1,\"ok\":true,\"large_cases\":3,\"successes\":2,\"capacity_failures\":1,\"input_limit\":67108864,\"output_limit\":67108864,\"readonly_input_tailguard\":true,\"output_tailguard\":true");
#ifdef SENGOO_DEFLATE_PRODUCTION_MEMORY
    printf(",\"instrumented\":false,\"complete_sequential_input_output\":null,\"known_failure_prefix_checked\":false,\"input_reads\":null,\"history_reads\":null,\"writes\":null}\n");
#else
    printf(",\"instrumented\":true,\"complete_sequential_input_output\":true,\"known_failure_prefix_checked\":true,\"input_reads\":%" PRIu64 ",\"history_reads\":%" PRIu64 ",\"writes\":%" PRIu64 "}\n",input_reads,output_reads,output_writes);
#endif
    return 0;
}

#ifndef SENGOO_RAW_PROBE_MAIN
#define SENGOO_RAW_PROBE_MAIN main
#endif
int SENGOO_RAW_PROBE_MAIN(int argc,char **argv) {
    if(argc==3&&!strcmp(argv[1],"--large"))return large(argv[2]);
    if(argc!=2)return 64;
    FILE *file=NULL;if(fopen_s(&file,argv[1],"rb")||!file)fail("open-corpus",1,0);
    uint8_t header[20];exact(file,header,16);
    if(memcmp(header,"SGDFL1\0\0",8))fail("corpus-magic",1,0);
    uint64_t records=little(header+8,8);if(!records||records>100000)fail("corpus-count",1,(int64_t)records);
    for(record_id=0;record_id<records;++record_id) {
        exact(file,header,20);uint32_t length=(uint32_t)little(header,4),capacity=(uint32_t)little(header+4,4),plain_length=(uint32_t)little(header+16,4);
        int64_t expected=(int64_t)little(header+8,8);
        if(length>1048576||capacity>1048576||plain_length>1048576 || (expected>=0 && ((uint64_t)expected>capacity || (uint64_t)expected!=plain_length)))fail("corpus-limits",1,0);
        uint8_t *input=malloc((size_t)length+32),*saved=malloc((size_t)length+1),*plain=malloc((size_t)plain_length+1),*output=malloc((size_t)capacity+32);
        if(!input||!saved||!plain||!output)fail("allocation",1,0);
        memset(input,0x5a,(size_t)length+32);memset(output,0xa5,(size_t)capacity+32);
        exact(file,input+16,length);memcpy(saved,input+16,length);exact(file,plain,plain_length);
        check(input+16,length,output+16,capacity,expected,plain,plain_length);
        if(length && memcmp(input+16,saved,length))fail("input-modified",0,1);
        for(unsigned i=0;i<16;++i)if(input[i]!=0x5a||input[length+16+i]!=0x5a||output[i]!=0xa5||output[capacity+16+i]!=0xa5)fail("canary",0,1);
        if(record_id<16 || (length<=512 && capacity<=4096))guards(input+16,length,plain,capacity,expected,plain_length);
        free(input);free(saved);free(plain);free(output);
    }
    if(fgetc(file)!=EOF||ferror(file)||fclose(file))fail("corpus-trailing",0,1);
    void *unreadable=VirtualAlloc(NULL,4096,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);
    if(!unreadable)fail("limit-guard-allocation",1,0);
    const int64_t limits[]={INT64_MIN,-1,67108865,INT64_MAX};
    active_input=unreadable;active_output=unreadable;input_extent=output_extent=current_writes=0;
    for(unsigned i=0;i<sizeof limits/sizeof limits[0];++i)for(unsigned side=0;side<2;++side) {
        uint64_t before=input_reads+output_reads+output_writes;
        int64_t result=sengoo_sg_raw_inflate(unreadable,side?7:limits[i],unreadable,side?limits[i]:7);
        if(result!=-3 || input_reads+output_reads+output_writes!=before)fail("invalid-limits-no-access",-3,result);
        ++invalid_limits;
    }
    if(!VirtualFree(unreadable,0,MEM_RELEASE))fail("limit-guard-free",1,0);
    printf("{\"schema\":1,\"ok\":true,\"records\":%" PRIu64 ",\"calls\":%" PRIu64 ",\"successes\":%" PRIu64 ",\"malformed\":%" PRIu64 ",\"capacity_failures\":%" PRIu64 ",\"invalid_limits\":%" PRIu64 ",\"guard_cases\":%" PRIu64 ",\"input_unchanged\":true",records,passed,successes,malformed,capacities,invalid_limits,guard_cases);
#ifdef SENGOO_DEFLATE_PRODUCTION_MEMORY
    printf(",\"instrumented\":false,\"bounded_sequential_output\":null,\"known_failure_prefix_checked\":false,\"input_reads\":null,\"history_reads\":null,\"writes\":null}\n");
#else
    printf(",\"instrumented\":true,\"bounded_sequential_output\":true,\"known_failure_prefix_checked\":true,\"input_reads\":%" PRIu64 ",\"history_reads\":%" PRIu64 ",\"writes\":%" PRIu64 "}\n",input_reads,output_reads,output_writes);
#endif
    return 0;
}
