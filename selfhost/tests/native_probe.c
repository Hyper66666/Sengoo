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
extern int64_t sg_native_div(int64_t,int64_t), sg_native_rem(int64_t,int64_t);
extern int64_t sg_native_shift(int64_t,int64_t);
extern bool sg_native_less(int64_t,int64_t);
extern uint64_t sg_native_uadd(uint64_t,uint64_t);
extern uint64_t sg_native_umul(uint64_t,uint64_t);
extern uint64_t sg_native_udiv(uint64_t,uint64_t), sg_native_urem(uint64_t,uint64_t);
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
extern int64_t sg_cast_f64_i64(double), sg_cast_f32_i64(float);
extern uint64_t sg_cast_f64_u64(double), sg_cast_f32_u64(float);
extern int8_t sg_cast_f64_i8(double);
extern uint8_t sg_cast_f64_u8(double);
extern double sg_cast_i64_f64(int64_t), sg_cast_u64_f64(uint64_t), sg_cast_f32_f64(float);
extern float sg_cast_u64_f32(uint64_t), sg_cast_f64_f32(double);
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
extern int64_t sg_native_cleanup(bool);
extern int64_t sg_native_nested_cleanup(void), sg_native_array_cleanup(void);
struct DropChoice { int64_t tag,value; };
extern int64_t sg_native_enum_cleanup(struct DropChoice);
extern const uint8_t *sg_native_string_utf8(void), *sg_native_string_same(void);
extern const uint8_t *sg_native_string_empty(void), *sg_native_string_nul(void);
extern int64_t sg_native_phi_bool(bool,bool,bool);
extern int64_t sg_native_phi_i8(bool,int8_t,int8_t), sg_native_phi_i16(bool,int16_t,int16_t);
extern int64_t sg_native_phi_i32(bool,int32_t,int32_t);
extern int64_t sg_native_phi_u8(bool,uint8_t,uint8_t), sg_native_phi_u16(bool,uint16_t,uint16_t);
extern int64_t sg_native_phi_u32(bool,uint32_t,uint32_t);
struct NarrowFields { int8_t byte; int16_t short_value; int32_t word; };
extern int64_t sg_native_narrow_fields(struct NarrowFields);

__declspec(noinline) static void poison_stack(void) {
    volatile uint8_t bytes[8192];
    for (unsigned index=0;index<sizeof(bytes);++index) bytes[index]=0xA5;
}

static int test_narrow_copies(void) {
    for (unsigned round=0;round<20;++round) {
        for (unsigned choose=0;choose<2;++choose) {
            for (unsigned left=0;left<2;++left) {
                for (unsigned right=0;right<2;++right) {
                    poison_stack();
                    int64_t expected=(choose ? left : right) ? 2 : 42;
                    if (sg_native_phi_bool(choose!=0,left!=0,right!=0)!=expected) return 1;
                }
            }
            poison_stack();
            if (sg_native_phi_i8(choose!=0,INT8_MIN,INT8_MAX)!=(choose ? INT8_MIN : INT8_MAX)) return 2;
            poison_stack();
            if (sg_native_phi_i16(choose!=0,INT16_MIN,INT16_MAX)!=(choose ? INT16_MIN : INT16_MAX)) return 3;
            poison_stack();
            if (sg_native_phi_i32(choose!=0,INT32_MIN,INT32_MAX)!=(choose ? INT32_MIN : INT32_MAX)) return 4;
            poison_stack();
            if (sg_native_phi_u8(choose!=0,UINT8_MAX,0)!=(choose ? UINT8_MAX : 0)) return 5;
            poison_stack();
            if (sg_native_phi_u16(choose!=0,UINT16_MAX,0)!=(choose ? UINT16_MAX : 0)) return 6;
            poison_stack();
            if (sg_native_phi_u32(choose!=0,UINT32_MAX,0)!=(choose ? (int64_t)UINT32_MAX : 0)) return 7;
        }
        poison_stack();
        if (sg_native_narrow_fields((struct NarrowFields){-7,-300,-100000})!=-100307) return 8;
    }
    return 0;
}
typedef void (*move_fn)(void *,const void *,uint64_t);
extern void sg_native_memmove_u8(void *,const void *,uint64_t);
extern void sg_native_memmove_u64(void *,const void *,uint64_t);
extern void sg_native_memmove_pair(void *,const void *,uint64_t);
extern uint8_t *sg_native_index_bytes(uint8_t *,int64_t);
extern struct Pair *sg_native_index_pairs(struct Pair *,int64_t);
extern struct Odd *sg_native_index_odd(struct Odd *,int64_t);
extern int64_t *sg_native_index_array(int64_t *,int64_t);

static int test_memory_ranges(void) {
    move_fn moves[]={sg_native_memmove_u8,sg_native_memmove_u64,sg_native_memmove_pair};
    uint8_t actual[128],expected[128],source[128];
    for (unsigned i=0;i<128;++i) source[i]=(uint8_t)(i*73+19);
    for (unsigned f=0;f<3;++f) {
        moves[f](NULL,NULL,0);
        for (unsigned destination=0;destination<25;++destination) {
            for (unsigned input=0;input<25;++input) {
                for (unsigned count=0;count<41;++count) {
                    memcpy(actual,source,sizeof(actual));
                    memcpy(expected,source,sizeof(expected));
                    memmove(expected+destination,expected+input,count);
                    moves[f](actual+destination,actual+input,count);
                    if (memcmp(actual,expected,sizeof(actual))) return 1;
                }
            }
        }
        for (unsigned count=0;count<=65;++count) {
            memset(actual,0xA5,sizeof(actual));
            memset(expected,0xA5,sizeof(expected));
            memmove(expected+1,source+3,count);
            moves[f](actual+1,source+3,count);
            if (memcmp(actual,expected,sizeof(actual))) return 2;
        }
    }
    SYSTEM_INFO information;
    GetSystemInfo(&information);
    SIZE_T page=information.dwPageSize;
    uint8_t *guard=VirtualAlloc(NULL,page*2,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
    DWORD old;
    if (!guard || !VirtualProtect(guard+page,page,PAGE_NOACCESS,&old)) return 3;
    for (unsigned f=0;f<3;++f) {
        for (unsigned count=1;count<64;++count) {
            memcpy(guard+page-count,source,count);
            memset(actual,0xA5,sizeof(actual));
            moves[f](actual+1,guard+page-count,count);
            if (memcmp(actual+1,source,count) || actual[0]!=0xA5 || actual[count+1]!=0xA5) return 4;
            memset(guard+page-count,0,count);
            moves[f](guard+page-count,source,count);
            if (memcmp(guard+page-count,source,count)) return 5;
        }
    }
    if (!VirtualFree(guard,0,MEM_RELEASE)) return 6;
    struct Pair pairs[8];
    struct Odd odds[8];
    int64_t array[3]={20,22,0};
    for (int64_t index=-3;index<=3;++index) {
        if (sg_native_index_bytes(actual+64,index)!=actual+64+index ||
            sg_native_index_pairs(pairs+4,index)!=pairs+4+index ||
            sg_native_index_odd(odds+4,index)!=odds+4+index) return 7;
    }
    for (int64_t index=0;index<3;++index) {
        if (sg_native_index_array(array,index)!=array+index) return 8;
    }
    *sg_native_index_array(array,2)=42;
    if (array[2]!=42) return 9;
    return 0;
}

static int test_string_data(void) {
    const uint8_t expected[]={0xE4,0xB8,0x96,0xE7,0x95,0x8C,0x20,0x26,
        0x0A,0x09,0x22,0x5C,0x0D,0xF0,0x9F,0x98,0x80,0};
    const uint8_t embedded[]={'A',0,'B',0};
    const uint8_t *value=sg_native_string_utf8();
    if (memcmp(value,expected,sizeof(expected)) || value!=sg_native_string_same() ||
        sg_native_string_empty()[0]!=0 ||
        memcmp(sg_native_string_nul(),embedded,sizeof(embedded))) return 1;
    MEMORY_BASIC_INFORMATION information;
    if (VirtualQuery(value,&information,sizeof(information))!=sizeof(information) ||
        (information.Protect & (PAGE_READWRITE|PAGE_WRITECOPY|PAGE_EXECUTE_READWRITE|PAGE_EXECUTE_WRITECOPY))) return 2;
    return 0;
}
static int drop_count;
static int64_t drop_values[32];
void sg_drop_test_owner(void *pointer) {
    int64_t *value=pointer;
    if (*value!=42 && *value!=1 && *value!=2) exit(75);
    if (drop_count>=32) exit(75);
    drop_values[drop_count]=*value;
    *value=-999;
    ++drop_count;
}
struct Pair sg_native_pair_external(struct Pair p) { p.x+=2; return p; }
static int checks;
void sengoo_panic_division_by_zero(int64_t divisor) {
    if (!divisor) { fputs("division by zero\n",stderr); exit(62); }
}
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
static int test_casts(void) {
    const double values[]={NAN,INFINITY,-INFINITY,0.0,-0.0,42.9,-42.9,127.9,128,-129,
        255.9,256,-1,0x1p63,-0x1p63,0x1p64,0x1.fffffffffffffp62,0x1.fffffffffffffp63,
        1e-300,-1e-300,1e300,-1e300};
    for (unsigned i=0;i<sizeof(values)/sizeof(values[0]);++i) {
        double v=values[i];
        int64_t s=isnan(v)?0:v>=0x1p63?INT64_MAX:v<=-0x1p63?INT64_MIN:(int64_t)v;
        uint64_t u=isnan(v)||v<=0?0:v>=0x1p64?UINT64_MAX:(uint64_t)v;
        int8_t s8=isnan(v)?0:v>=128?127:v<=-128?-128:(int8_t)v;
        uint8_t u8=isnan(v)||v<=0?0:v>=256?255:(uint8_t)v;
        if (sg_cast_f64_i64(v)!=s || sg_cast_f64_u64(v)!=u ||
            sg_cast_f64_i8(v)!=s8 || sg_cast_f64_u8(v)!=u8) return 1;
        float f=(float)v;
        int64_t fs=isnan(f)?0:f>=0x1p63f?INT64_MAX:f<=-0x1p63f?INT64_MIN:(int64_t)f;
        uint64_t fu=isnan(f)||f<=0?0:f>=0x1p64f?UINT64_MAX:(uint64_t)f;
        if (sg_cast_f32_i64(f)!=fs || sg_cast_f32_u64(f)!=fu) return 2;
        if (!isnan(v) && sg_cast_f64_f32(v)!=(float)v) return 3;
        if (!isnan(f) && sg_cast_f32_f64(f)!=(double)f) return 4;
    }
    uint64_t state=0x1234abcdef987654ULL;
    for (unsigned i=0;i<4096;++i) {
        state=state*6364136223846793005ULL+1442695040888963407ULL;
        if (sg_cast_u64_f64(state)!=(double)state || sg_cast_u64_f32(state)!=(float)state ||
            sg_cast_i64_f64((int64_t)state)!=(double)(int64_t)state) return 5;
    }
    return 0;
}
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
    if (argc>1 && strcmp(argv[1],"division-zero")==0) { (void)sg_native_div(42,0); return 65; }
    if (argc>1 && strcmp(argv[1],"division-overflow")==0) { (void)sg_native_div(INT64_MIN,-1); return 65; }
    if (argc>1 && strcmp(argv[1],"wrap")==0) {
        if (sg_native_add(INT64_MAX,1)!=INT64_MIN || sg_native_umul(UINT64_MAX,2)!=UINT64_MAX-1 ||
            sg_native_div(INT64_MIN,-1)!=INT64_MIN || sg_native_rem(INT64_MIN,-1)!=0 || checks!=0) return 66;
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
        sg_native_div(-85,2)!=-42 || sg_native_rem(-85,2)!=-1 ||
        sg_native_udiv(UINT64_MAX,2)!=UINT64_MAX/2 || sg_native_urem(UINT64_MAX,2)!=1 ||
        !sg_native_less(INT64_MIN,INT64_MAX) || sg_native_less(INT64_MAX,INT64_MIN) ||
        sg_native_uadd(40,2)!=42 || sg_native_umul(6,7)!=42 ||
        sg_native_large(1,2,3,4,5,6)!=42 || checks<4 || unwound!=7) return 67;
    if (virtual_unwind(sg_native_add) || virtual_unwind(sg_native_swap)) return 68;
    if (test_casts()) return 74;
    if (test_memory_ranges()) return 79;
    if (test_string_data()) return 80;
    if (test_narrow_copies()) return 81;
    if (sg_native_cleanup(false)!=42 || drop_count!=0 ||
        sg_native_cleanup(true)!=42 || drop_count!=1 ||
        sg_native_cleanup(true)!=42 || drop_count!=2) return 76;
    if (sg_native_nested_cleanup()!=42 || drop_count!=4 || drop_values[2]!=2 || drop_values[3]!=1 ||
        sg_native_array_cleanup()!=42 || drop_count!=6 || drop_values[4]!=2 || drop_values[5]!=1) return 77;
    if (sg_native_enum_cleanup((struct DropChoice){0,-777})!=42 || drop_count!=6 ||
        sg_native_enum_cleanup((struct DropChoice){1,42})!=42 || drop_count!=7 || drop_values[6]!=42) return 78;
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
    puts("native saturating casts, NaN, ranges and integer rounding: ok");
    puts("native signed and unsigned division and remainder: ok");
    puts("native conditional initialization and exact user Drop: ok");
    puts("native nested and array reverse cleanup with cleared flags: ok");
    puts("native active enum payload cleanup: ok");
    puts("native overlapping byte memmove and typed index strides: ok");
    puts("native readonly UTF-8 string data, NUL and literal relocations: ok");
    puts("native dirty-stack bool and narrow integer phi/field reads: ok");
    return 0;
}
