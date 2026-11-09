#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

void sengoo_panic_integer_overflow(int64_t overflow) {
    if (overflow) { fputs("integer overflow\n", stderr); exit(63); }
}
int64_t sg_native_external6(int64_t a,int64_t b,int64_t c,int64_t d,int64_t e,int64_t f) {
    return a+b+c+d+e+f;
}
