/* Reuse only the independent memory instrumentation, not a decoder. */
#define main sh_deflate_unused_corpus_main
#include "deflate_probe.c"
#undef main
extern int64_t sengoo_sg_raw_inflate_prefix(const uint8_t *,int64_t,uint8_t *,int64_t);

int main(void) {
    const uint8_t empty[]={3,0};
    const uint8_t fixed[]={0x73,0x1c,0xf9,0,0};
    const uint8_t stored[]={1,3,0,252,255,65,66,67};
    const uint8_t *sources[]={empty,fixed,stored};
    const size_t lengths[]={sizeof empty,sizeof fixed,sizeof stored};
    const int64_t decoded[]={0,259,3};
    uint8_t input[32],output[300];
    for(unsigned test=0;test<3;++test) {
        record_id=test;memcpy(input,sources[test],lengths[test]);memset(input+lengths[test],0xff,3);
        active_input=input;input_extent=lengths[test]+3;active_output=output;output_extent=sizeof output;current_writes=current_input_reads=0;
        int64_t actual=sengoo_sg_raw_inflate_prefix(input,(int64_t)input_extent,output,(int64_t)output_extent);
        int64_t expected=((int64_t)lengths[test]<<27)|decoded[test];
        if(actual!=expected || current_input_reads!=lengths[test] || current_writes!=(uint64_t)decoded[test])fail("prefix-counts",expected,actual);
        for(int64_t index=0;index<decoded[test];++index)if(output[index]!=(test==1?65:(uint8_t)(65+index)))fail("prefix-bytes",65,output[index]);
        current_writes=current_input_reads=0;
        int64_t exact=sengoo_sg_raw_inflate(input,(int64_t)input_extent,output,(int64_t)output_extent);
        if(exact!=-1)fail("exact-legacy-trailing",-1,exact);
    }
    active_input=empty;input_extent=sizeof empty;active_output=NULL;output_extent=current_writes=current_input_reads=0;
    int64_t empty_count=sengoo_sg_raw_inflate_prefix(empty,sizeof empty,NULL,0);
    if(empty_count!=((int64_t)sizeof empty<<27)||current_writes!=0||current_input_reads!=sizeof empty)fail("prefix-null-empty-output",(int64_t)sizeof empty<<27,empty_count);
    void *unreadable=VirtualAlloc(NULL,4096,MEM_RESERVE|MEM_COMMIT,PAGE_NOACCESS);
    if(!unreadable)fail("prefix-limit-allocation",1,0);
    const int64_t limits[]={INT64_MIN,-1,67108865,INT64_MAX};
    active_input=unreadable;active_output=unreadable;input_extent=output_extent=current_writes=current_input_reads=0;
    for(unsigned i=0;i<4;++i)for(unsigned side=0;side<2;++side) {
        uint64_t before=input_reads+output_reads+output_writes;
        int64_t actual=sengoo_sg_raw_inflate_prefix(unreadable,side?7:limits[i],unreadable,side?limits[i]:7);
        if(actual!=-3 || input_reads+output_reads+output_writes!=before)fail("prefix-invalid-no-access",-3,actual);
    }
    if(!VirtualFree(unreadable,0,MEM_RELEASE))fail("prefix-limit-free",1,0);
    printf("{\"schema\":1,\"ok\":true,\"prefix_cases\":4,\"legacy_exact_cases\":3,\"invalid_limits\":8,\"null_empty_output\":true,\"no_trailing_reads\":true,\"decoded_bytes_equal\":true}\n");
    return 0;
}
