#define main sh_gzs_unused_corpus_main
#include "gzip_stream_probe.c"
#undef main
static uint8_t *load(const char *directory,const char *name,int64_t *length,uint8_t **allocation){
    char path[32768];if(snprintf(path,sizeof path,"%s/%s",directory,name)<0)fail("large-path",1,0);
    FILE *file=NULL;if(fopen_s(&file,path,"rb")||!file)fail("large-open",1,0);fseek(file,0,SEEK_END);long n=ftell(file);rewind(file);if(n<1||n>67108864)fail("large-length",1,n);
    size_t rounded=((size_t)n+4095)/4096*4096;*allocation=VirtualAlloc(NULL,rounded+8192,MEM_RESERVE,PAGE_NOACCESS);if(!*allocation)fail("large-reserve",1,0);
    uint8_t *page=VirtualAlloc(*allocation+4096,rounded,MEM_COMMIT,PAGE_READWRITE);if(!page)fail("large-commit",1,0);uint8_t *bytes=page+rounded-n;
    if(fread(bytes,1,n,file)!=(size_t)n||fgetc(file)!=EOF)fail("large-file-read",n,0);fclose(file);DWORD old;if(!VirtualProtect(page,rounded,PAGE_READONLY,&old))fail("large-readonly",1,0);*length=n;return bytes;
}
int main(int argc,char **argv){
    if(argc!=2)return 2;int64_t length;uint8_t *allocation;const uint8_t *input=load(argv[1],"wide-members.gzip",&length,&allocation);
    uint8_t *state=malloc(46080);if(!state)fail("wide-state",1,0);
    uint8_t *out_allocation=VirtualAlloc(NULL,40960,MEM_RESERVE,PAGE_NOACCESS);if(!out_allocation)fail("wide-out-reserve",1,0);
    uint8_t *page=VirtualAlloc(out_allocation+4096,32768,MEM_COMMIT,PAGE_READWRITE);if(!page)fail("wide-out-commit",1,0);
    uint64_t case_calls[6];int64_t decoded[6],consumed[6],members[6];
    for(int which=0;which<5;which++){
        fixture=which;state_owner=state;memset(initialized,0,sizeof initialized);memset(history_initialized,0,sizeof history_initialized);
        int policy=which==2?0:which==3?1:2;int64_t limit=which==1?68157439:68157440;
        if(sengoo_sg_gzip_stream_init(state,46080,policy,which==4?length-1:67108864,limit))fail("wide-init",0,1);
        int64_t at=0,produced=0,end=0;uint64_t count=0;int64_t status;
        while(1){
            if(at==end&&end<length){end=at+113;if(end>length)end=length;}
            int64_t capacity=32749;uint8_t *output=page+32768-capacity;
            status=step(state,input+at,end-at,output,capacity,end==length);++count;
            for(int64_t i=0;i<current_writes;i++)if(output[i]!=(uint8_t)(((produced+i)%1048576)%251))fail("wide-output-pattern",(produced+i)%251,output[i]);
            at+=current_reads;produced+=current_writes;
            if(status!=0&&status!=1)break;
            if(status==0&&end==length)fail("wide-final-need-input",-1,0);
        }
        int64_t expected=which==1||which==4?-2:which==3?-1:2;
        if(status!=expected)fail("wide-status",expected,status);
        int64_t expected_members=which==1||which==4?64:which==2||which==3?1:65;
        int64_t seen=sengoo_sg_gzip_stream_query(state,46080,7);if(seen!=expected_members)fail("wide-members",expected_members,seen);
        if((which==0&&produced!=68157440)||((which==2||which==3)&&produced!=1048576))fail("wide-output-count",68157440,produced);
        if(which==0&&at!=length)fail("wide-exact-consumed",length,at);
        if((which==2||which==3)&&at!=length/65)fail("wide-first-wire",length/65,at);
        if(which==4&&at!=length-1)fail("wide-input-budget",length-1,at);
        decoded[which]=produced;consumed[which]=at;members[which]=seen;case_calls[which]=count;
        if(sengoo_sg_gzip_stream_dispose(state,46080))fail("wide-dispose",0,1);
    }
    VirtualFree(allocation,0,MEM_RELEASE);
    int64_t cross_length;const uint8_t *cross=load(argv[1],"cross-member-history.gzip",&cross_length,&allocation);
    init(state,2);int64_t bad=step(state,cross,cross_length,page,32768,1);
    if(bad!=-1||current_writes!=1||page[0]!=65||sengoo_sg_gzip_stream_query(state,46080,7)!=1)fail("member-history-reset",-1,bad);
    VirtualFree(allocation,0,MEM_RELEASE);
    const uint8_t *maximum=load(argv[1],"max-input.gzip",&length,&allocation);if(length!=67108864)fail("actual-max-input",67108864,length);
    state_owner=state;memset(initialized,0,sizeof initialized);memset(history_initialized,0,sizeof history_initialized);
    if(sengoo_sg_gzip_stream_init(state,46080,2,67108864,67103726))fail("maximum-init",0,1);
    int64_t at=0,produced=0,end=0,status;uint64_t count=0;
    while(1){
        if(at==end&&end<length){end=at+113;if(end>length)end=length;}
        uint8_t *output=page+32768-32749;status=step(state,maximum+at,end-at,output,32749,end==length);++count;
        for(int64_t i=0;i<current_writes;i++)if(output[i]!=(uint8_t)((produced+i)%251))fail("max-input-pattern",(produced+i)%251,output[i]);
        at+=current_reads;produced+=current_writes;if(status!=0&&status!=1)break;
        if(status==0&&end==length)fail("max-input-final",2,status);
    }
    if(status!=2||at!=length||produced!=67103726||sengoo_sg_gzip_stream_query(state,46080,7)!=1)fail("max-input-oracle",67103726,produced);
    case_calls[5]=count;decoded[5]=produced;consumed[5]=at;members[5]=1;
    if(sengoo_sg_gzip_stream_dispose(state,46080))fail("max-dispose",0,1);for(int i=0;i<46080;i++)if(state[i])fail("max-erase",0,state[i]);
    printf("{\"schema\":1,\"ok\":true,\"large_cases\":6,\"cross_member_history_rejected\":true,\"state_bytes\":46080,\"output_chunk_bytes\":32749,\"wide_output_bytes\":68157440,\"actual_max_input_bytes\":67108864,\"readonly_input_tailguard\":true,\"output_tailguard\":true,\"cases\":[");
    for(int i=0;i<6;i++)printf("%s{\"calls\":%llu,\"decoded\":%lld,\"consumed\":%lld,\"members\":%lld}",i?",":"",(unsigned long long)case_calls[i],(long long)decoded[i],(long long)consumed[i],(long long)members[i]);puts("]}");
    VirtualFree(allocation,0,MEM_RELEASE);VirtualFree(out_allocation,0,MEM_RELEASE);free(state);return 0;
}
