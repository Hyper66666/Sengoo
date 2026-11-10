#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

extern int64_t sengoo_sg_tar_scan(const uint8_t *, int64_t, uint8_t *, int64_t, uint8_t *, int64_t);
static const uint8_t *active_input;
static uint8_t *active_table, *active_paths;
static uint8_t *table_written, *paths_written;
static int64_t active_input_length, active_table_length, active_paths_length;
static uint64_t reads, writes, calls, os_calls;
static uint64_t completed=1469598103934665603ULL;
void sengoo_tar_observe_os_compare(void) { ++os_calls; }
static unsigned fixture;
static void fail(const char *reason) {
    fprintf(stderr,"fixture=%u reason=%s\n",fixture,reason);exit(1);
}
#ifndef SENGOO_TAR_PRODUCTION
uint8_t sengoo_tar_read(const uint8_t *bytes, int64_t index) {
    if(bytes!=active_input || index<0 || index>=active_input_length)fail("input-bound");
    ++reads;return bytes[index];
}
uint8_t sengoo_tar_output_read(uint8_t *bytes, int64_t index) {
    int64_t length=bytes==active_table?active_table_length:bytes==active_paths?active_paths_length:-1;
    if(index<0 || index>=length)fail("output-read-bound");
    uint8_t *initialized=bytes==active_table?table_written:paths_written;
    if(!initialized[index])fail("output-uninitialized-read");
    ++reads;return bytes[index];
}
void sengoo_tar_write(uint8_t *bytes, int64_t index, uint8_t value) {
    int64_t length=bytes==active_table?active_table_length:bytes==active_paths?active_paths_length:-1;
    if(index<0 || index>=length)fail("output-write-bound");
    (bytes==active_table?table_written:paths_written)[index]=1;
    ++writes;bytes[index]=value;
}
#endif
typedef struct {uint8_t *allocation,*bytes;size_t total;int guard;} Arena;
static Arena arena(size_t length,int guard,int input) {
    Arena a={0};a.guard=guard;
    if(!guard){a.allocation=malloc(length+32);if(!a.allocation)fail("allocation");a.bytes=a.allocation+16;memset(a.allocation,0xa5,length+32);return a;}
    SYSTEM_INFO system;GetSystemInfo(&system);size_t page=system.dwPageSize;
    size_t accessible=((length?length:1)+page-1)/page*page;a.total=accessible+2*page;
    a.allocation=VirtualAlloc(NULL,a.total,MEM_RESERVE,PAGE_NOACCESS);if(!a.allocation)fail("reserve-guard");
    uint8_t *middle=VirtualAlloc(a.allocation+page,accessible,MEM_COMMIT,PAGE_READWRITE);if(!middle)fail("commit-guard");
    a.bytes=guard==1?middle:middle+accessible-length;
    if(!input&&length)memset(a.bytes,0xa5,length);return a;
}
static void release(Arena a){if(a.guard)VirtualFree(a.allocation,0,MEM_RELEASE);else free(a.allocation);}
static void readonly(Arena a) {
    SYSTEM_INFO system;GetSystemInfo(&system);DWORD old;
    if(!VirtualProtect(a.allocation+system.dwPageSize,a.total-2*system.dwPageSize,PAGE_READONLY,&old))fail("readonly-input");
}
static uint32_t get32(const uint8_t **at,const uint8_t *end) {
    uint32_t value=0;if(end-*at<4)fail("corpus-short");
    for(int i=0;i<4;i++)value|=(uint32_t)*(*at)++<<(8*i);return value;
}
static int64_t get64(const uint8_t **at,const uint8_t *end) {
    uint64_t value=0;if(end-*at<8)fail("corpus-short");
    for(int i=0;i<8;i++)value|=(uint64_t)*(*at)++<<(8*i);return (int64_t)value;
}
int main(int argc,char **argv) {
    if(argc==2&&!strcmp(argv[1],"--uninitialized-control")) {
#ifndef SENGOO_TAR_PRODUCTION
        uint8_t bytes[1]={0},initialized[1]={0};active_table=bytes;active_table_length=1;table_written=initialized;
        (void)sengoo_tar_output_read(bytes,0);return 9;
#else
        return 10;
#endif
    }
    if(argc!=2)return 2;FILE *file=fopen(argv[1],"rb");if(!file)return 3;
    if(fseek(file,0,SEEK_END))return 4;long size=ftell(file);if(size<12)return 5;rewind(file);
    uint8_t *data=malloc((size_t)size);if(!data)return 6;
    if(fread(data,1,(size_t)size,file)!=(size_t)size)return 7;fclose(file);
    const uint8_t *at=data,*end=data+size;if(memcmp(at,"SGTAR1\0\0",8))return 8;at+=8;
    uint32_t count=get32(&at,end);unsigned successes=0,rejections=0;
    for(fixture=0;fixture<count;fixture++) {
        uint32_t input=get32(&at,end),tc=get32(&at,end),pc=get32(&at,end);
        int64_t expected=get64(&at,end);uint32_t te=get32(&at,end),pe=get32(&at,end);
        if((uint64_t)(end-at)<(uint64_t)input+te+pe)fail("corpus-record-short");
        for(int guard=0;guard<3;guard++) {
        Arena in={0};if(guard){in=arena(input,guard,1);if(input)memcpy(in.bytes,at,input);readonly(in);}
        Arena table=arena(tc,guard,0),paths=arena(pc,guard,0);
        table_written=calloc(tc?tc:1,1);paths_written=calloc(pc?pc:1,1);if(!table_written||!paths_written)fail("allocation");
        active_input=guard?in.bytes:at;active_input_length=input;active_table=table.bytes;active_table_length=tc;active_paths=paths.bytes;active_paths_length=pc;
        int64_t actual=sengoo_sg_tar_scan(active_input,input,table.bytes,tc,paths.bytes,pc);++calls;
        if(actual!=expected){fprintf(stderr,"fixture=%u actual=%lld expected=%lld\n",fixture,(long long)actual,(long long)expected);fail("result-oracle");}
        for(unsigned n=0;n<8;n++){completed^=((uint64_t)fixture>>(n*8))&255;completed*=1099511628211ULL;}
        for(unsigned n=0;n<8;n++){completed^=((uint64_t)actual>>(n*8))&255;completed*=1099511628211ULL;}
        if(actual>=0){++successes;if(te>tc||pe>pc||memcmp(table.bytes,at+input,te)||memcmp(paths.bytes,at+input+te,pe))fail("metadata-oracle");}
        else{++rejections;for(unsigned n=0;n<32&&n<tc;n++)if(table.bytes[n]!=0xa5)fail("failed-header-published");}
        if(!guard)for(unsigned n=0;n<16;n++)if(table.allocation[n]!=0xa5||table.bytes[tc+n]!=0xa5||paths.allocation[n]!=0xa5||paths.bytes[pc+n]!=0xa5)fail("output-canary");
        if(guard&&input&&memcmp(in.bytes,at,input))fail("input-changed");
        release(table);release(paths);if(guard)release(in);free(table_written);free(paths_written);
        };
        at+=input+te+pe;
    }
    if(at!=end)fail("corpus-tail");
    active_input=NULL;active_input_length=0;active_table=NULL;active_table_length=0;active_paths=NULL;active_paths_length=0;
    uint64_t before_reads=reads,before_writes=writes,before_os=os_calls;
    const int64_t bad[][3]={{-1,0,0},{536870913,0,0},{0,-1,0},{0,480033,0},{0,0,-1},{0,0,16777217}};
    for(unsigned i=0;i<sizeof(bad)/sizeof(bad[0]);i++)if(sengoo_sg_tar_scan(NULL,bad[i][0],NULL,bad[i][1],NULL,bad[i][2])!=-3)fail("invalid-extent");
    if(reads!=before_reads||writes!=before_writes||os_calls!=before_os)fail("invalid-extent-access");
#ifdef SENGOO_TAR_PRODUCTION
    printf("{\"schema\":1,\"ok\":true,\"mode\":\"production\",\"fixtures\":%u,\"calls\":%llu,\"guard_calls\":%u,\"successes\":%u,\"rejections\":%u,\"invalid_extents\":6,\"reads\":null,\"writes\":null,\"failure_header_unchanged\":true,\"completed_fnv64\":\"%016llx\"}\n",count,(unsigned long long)calls,count*2,successes,rejections,(unsigned long long)completed);
#else
    printf("{\"schema\":1,\"ok\":true,\"mode\":\"instrumented\",\"fixtures\":%u,\"calls\":%llu,\"guard_calls\":%u,\"successes\":%u,\"rejections\":%u,\"invalid_extents\":6,\"reads\":%llu,\"writes\":%llu,\"failure_header_unchanged\":true,\"completed_fnv64\":\"%016llx\"}\n",count,(unsigned long long)calls,count*2,successes,rejections,(unsigned long long)reads,(unsigned long long)writes,(unsigned long long)completed);
#endif
    free(data);return 0;
}
