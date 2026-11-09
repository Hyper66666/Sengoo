#include <stdint.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
#include <math.h>
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
extern int64_t sg_native_large(int64_t,int64_t,int64_t,int64_t,int64_t,int64_t);
extern double sg_native_fadd(double,double);
extern double sg_native_fsub(double,double);
extern double sg_native_fmul(double,double);
extern double sg_native_fdiv(double,double);
extern float sg_native_f32add(float,float);
extern double sg_native_fconstant(void);
extern bool sg_native_feq(double,double), sg_native_fne(double,double), sg_native_flt(double,double);
extern bool sg_native_fle(double,double), sg_native_fgt(double,double), sg_native_fge(double,double);
extern double sg_native_float_call6(int64_t,double,int64_t,float,double,int64_t);
double sg_native_float_external6(int64_t a,double b,int64_t c,float d,double e,int64_t f) {
    if (a!=1 || b!=2 || c!=3 || d!=4 || e!=5 || f!=27) exit(71);
    return a+b+c+d+e+f;
}
struct Pair { int64_t x,y; };
struct Small { int64_t x; };
struct Odd { uint8_t a,b,c; };
struct NativeChoice { int64_t tag; struct {int64_t x;uint8_t y;} payload; };
extern struct Pair sg_native_pair(int64_t,int64_t);
extern int64_t sg_native_pair_sum(struct Pair);
extern struct Pair sg_native_pair_call(struct Pair);
extern struct Small sg_native_small(struct Small);
extern struct Pair sg_native_pair_select(bool,struct Pair,struct Pair);
extern struct Odd sg_native_odd(struct Odd);
extern struct NativeChoice sg_native_enum(void);
struct Pair sg_native_pair_external(struct Pair p) { p.x+=2; return p; }
static int checks;
static int unwound;
void sengoo_panic_integer_overflow(int64_t overflow) {
    ++checks;
    if (overflow) { fputs("integer overflow\n",stderr); exit(63); }
    if (unwound!=7) {
        void *frames[32];
        USHORT count=CaptureStackBackTrace(0,32,frames,NULL);
        for (USHORT i=0;i<count;++i) {
            DWORD64 image=0;
            PRUNTIME_FUNCTION entry=RtlLookupFunctionEntry((DWORD64)(uintptr_t)frames[i],&image,NULL);
            if (entry && image+entry->BeginAddress==(DWORD64)(uintptr_t)sg_native_add) unwound|=1;
            if (entry && image+entry->BeginAddress==(DWORD64)(uintptr_t)sg_native_swap) unwound|=2;
            if (entry && image+entry->BeginAddress==(DWORD64)(uintptr_t)sg_native_large) unwound|=4;
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
        sg_native_uadd(40,2)!=42 || sg_native_umul(6,7)!=42 ||
        sg_native_large(1,2,3,4,5,6)!=42 || checks<4 || unwound!=7) return 67;
    if (virtual_unwind(sg_native_add) || virtual_unwind(sg_native_swap)) return 68;
    struct Pair pair=sg_native_pair(40,2);
    struct Pair called_pair=sg_native_pair_call(pair);
    struct Odd odd=sg_native_odd((struct Odd){1,2,3});
    struct NativeChoice choice=sg_native_enum();
    if (pair.x!=40 || pair.y!=2 || sg_native_pair_sum(pair)!=42 ||
        called_pair.x!=42 || called_pair.y!=2 || sg_native_small((struct Small){42}).x!=42 ||
        sg_native_pair_select(true,pair,called_pair).x!=40 ||
        sg_native_pair_select(false,pair,called_pair).x!=42 ||
        odd.a!=1 || odd.b!=2 || odd.c!=3 || choice.tag!=1 || choice.payload.x!=42 || choice.payload.y!=9) return 73;
    if (sg_native_fadd(40,2)!=42 || sg_native_fsub(44,2)!=42 || sg_native_fmul(6,7)!=42 ||
        sg_native_fdiv(84,2)!=42 || sg_native_f32add(40,2)!=42 || sg_native_fconstant()!=42 ||
        sg_native_float_call6(1,2,3,4,5,27)!=42 || !sg_native_feq(0.0,-0.0) ||
        sg_native_feq(NAN,NAN) || !sg_native_fne(NAN,NAN) || sg_native_flt(NAN,0) ||
        sg_native_fle(NAN,0) || sg_native_fgt(NAN,0) || sg_native_fge(NAN,0) ||
        !sg_native_flt(-INFINITY,0) || !sg_native_fgt(INFINITY,0) ||
        !isinf(sg_native_fdiv(1.0,0.0)) || !isnan(sg_native_fdiv(0.0,0.0))) return 72;
    puts("native MIR calls, loop, pointers, narrow memory and checked arithmetic: ok");
    puts("native unwind metadata: ok");
    puts("native large stack probe: ok");
    puts("native float arithmetic, NaN and mixed Windows ABI: ok");
    puts("native aggregate Windows parameters and return pointers: ok");
    return 0;
}
