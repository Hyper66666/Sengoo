#include <stdint.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdbool.h>
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
void sengoo_panic_integer_overflow(int64_t overflow) {
    ++checks;
    if (overflow) { fputs("integer overflow\n",stderr); exit(63); }
}
int64_t sg_native_external6(int64_t a,int64_t b,int64_t c,int64_t d,int64_t e,int64_t f) {
    if (a!=1 || b!=2 || c!=3 || d!=4 || e!=5) exit(64);
    return a+b+c+d+e+f;
}
static int64_t c_add(int64_t a,int64_t b) { return a+b; }
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
        sg_native_uadd(40,2)!=42 || sg_native_umul(6,7)!=42 || checks<4) return 67;
    puts("native MIR calls, loop, pointers, narrow memory and checked arithmetic: ok");
    return 0;
}
