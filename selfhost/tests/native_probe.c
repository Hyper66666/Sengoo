#include <stdint.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
typedef int64_t (*binary_fn)(int64_t,int64_t);
extern int64_t sg_native_add(int64_t,int64_t);
extern int64_t sg_native_call6(int64_t);
extern int64_t sg_native_loop(int64_t);
extern int64_t sg_native_indirect(binary_fn,int64_t,int64_t);
extern binary_fn sg_native_address(void);
extern int64_t sg_native_memory(int8_t *);
extern int64_t sg_native_local(void);
extern int64_t sg_native_swap(int64_t);
extern int64_t sg_native_mul(int64_t,int64_t);
extern int64_t sg_native_sub(int64_t,int64_t);
extern int64_t sg_native_shift(int64_t,int64_t);
extern bool sg_native_less(int64_t,int64_t);
extern uint64_t sg_native_uadd(uint64_t,uint64_t);
extern uint64_t sg_native_umul(uint64_t,uint64_t);
static int checks;
static int unwound;
void sengoo_panic_integer_overflow(int64_t overflow) {
    ++checks;
    if (overflow) { fputs("integer overflow\n",stderr); exit(63); }
    if (unwound!=3) {
        void *frames[32];
        USHORT count=CaptureStackBackTrace(0,32,frames,NULL);
        for (USHORT i=0;i<count;++i) {
            DWORD64 image=0;
            PRUNTIME_FUNCTION entry=RtlLookupFunctionEntry((DWORD64)(uintptr_t)frames[i],&image,NULL);
            if (entry && image+entry->BeginAddress==(DWORD64)(uintptr_t)sg_native_add) unwound|=1;
            if (entry && image+entry->BeginAddress==(DWORD64)(uintptr_t)sg_native_swap) unwound|=2;
        }
    }
}
int64_t sg_native_external6(int64_t a,int64_t b,int64_t c,int64_t d,int64_t e,int64_t f) {
    if (a!=1 || b!=2 || c!=3 || d!=4 || e!=5) exit(64);
    return a+b+c+d+e+f;
}
static int64_t c_add(int64_t a,int64_t b) { return a+b; }
static int virtual_unwind(void *function) {
    DWORD64 image=0;
    PRUNTIME_FUNCTION entry=RtlLookupFunctionEntry((DWORD64)(uintptr_t)function,&image,NULL);
    if (!entry || image+entry->BeginAddress!=(DWORD64)(uintptr_t)function) return 1;
    const unsigned char *code=function;
    uint32_t frame=0;
    if (code[0]!=0x55 || code[1]!=0x48 || code[2]!=0x89 || code[3]!=0xe5 ||
        code[4]!=0x48 || code[5]!=0x81 || code[6]!=0xec) return 2;
    memcpy(&frame,code+7,4);
    if (frame>4000 || frame<16 || frame%16) return 3;
    uintptr_t stack[2048]={0};
    uintptr_t *original=stack+1800;
    const DWORD64 saved=0xabcdeffedcba1234ULL, returned=0x12345000;
    original[0]=(uintptr_t)returned;
    original[-1]=(uintptr_t)saved;
    const DWORD64 length=entry->EndAddress-entry->BeginAddress;
    const DWORD64 offsets[]={0,1,4,11,length-9,length-2,length-1};
    for (unsigned i=0;i<sizeof(offsets)/sizeof(offsets[0]);++i) {
        CONTEXT context={0};
        context.ContextFlags=CONTEXT_FULL;
        context.Rip=(DWORD64)(uintptr_t)function+offsets[i];
        context.Rsp=(DWORD64)(uintptr_t)original;
        context.Rbp=saved;
        if (i>=1 && i<=5) context.Rsp-=8;
        if (i==3 || i==4) context.Rsp-=frame;
        if (i>=2 && i<=5) context.Rbp=(DWORD64)(uintptr_t)(original-1);
        PVOID handler=NULL;
        DWORD64 establisher=0;
        RtlVirtualUnwind(UNW_FLAG_NHANDLER,image,context.Rip,entry,&context,&handler,&establisher,NULL);
        if (context.Rip!=returned || context.Rsp!=(DWORD64)(uintptr_t)(original+1) || context.Rbp!=saved) return 10+(int)i;
    }
    return 0;
}
int main(int argc,char **argv) {
    if (argc>1 && strcmp(argv[1],"overflow")==0) { (void)sg_native_add(INT64_MAX,1); return 65; }
    if (argc>1 && strcmp(argv[1],"overflow8")==0) { int8_t byte=127; (void)sg_native_memory(&byte); return 65; }
    if (argc>1 && strcmp(argv[1],"umul-overflow")==0) { (void)sg_native_umul(UINT64_MAX,2); return 65; }
    if (argc>1 && strcmp(argv[1],"uadd-overflow")==0) { (void)sg_native_uadd(UINT64_MAX,1); return 65; }
    if (argc>1 && strcmp(argv[1],"mul-overflow")==0) { (void)sg_native_mul(INT64_MAX,2); return 65; }
    if (argc>1 && strcmp(argv[1],"wrap")==0) {
        if (sg_native_add(INT64_MAX,1)!=INT64_MIN || sg_native_umul(UINT64_MAX,2)!=UINT64_MAX-1 || checks!=0) return 66;
        puts("native wrapping arithmetic: ok"); return 0;
    }
    int8_t byte=-2;
    if (sg_native_add(40,2)!=42 || sg_native_call6(27)!=42 ||
        sg_native_loop(10)!=45 || sg_native_loop(0)!=0 || sg_native_loop(-1)!=0 ||
        sg_native_indirect(c_add,19,23)!=42 || sg_native_address()!=sg_native_add ||
        sg_native_address()(20,22)!=42 || sg_native_memory(&byte)!=-1 || byte!=-1 ||
        sg_native_local()!=-123 || sg_native_swap(0)!=12 || sg_native_swap(1)!=21 ||
        sg_native_swap(2)!=12 || sg_native_swap(101)!=21 ||
        sg_native_mul(-6,7)!=-42 || sg_native_sub(2,44)!=-42 || sg_native_shift(-84,1)!=-42 ||
        !sg_native_less(INT64_MIN,INT64_MAX) || sg_native_less(INT64_MAX,INT64_MIN) ||
        sg_native_uadd(40,2)!=42 || sg_native_umul(6,7)!=42 || checks<4 || unwound!=3) return 67;
    if (virtual_unwind(sg_native_add) || virtual_unwind(sg_native_swap)) return 68;
    puts("native MIR calls, loop, pointers, narrow memory and checked arithmetic: ok");
    puts("native unwind metadata: ok");
    return 0;
}
