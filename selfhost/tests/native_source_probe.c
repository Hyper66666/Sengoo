#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>

void sengoo_panic_integer_overflow(int64_t overflow) {
    if (overflow) { fputs("integer overflow\n", stderr); exit(63); }
}
void sengoo_panic_division_by_zero(int64_t divisor) {
    if (!divisor) { fputs("division by zero\n", stderr); exit(62); }
}
int64_t sg_native_external6(int64_t a,int64_t b,int64_t c,int64_t d,int64_t e,int64_t f) {
    return a+b+c+d+e+f;
}

static int64_t drop_values[16];
static int64_t drop_count;
void sg_drop_record(int64_t value) {
    if (drop_count >= 16) exit(61);
    drop_values[drop_count++] = value;
}
int64_t sg_drop_count(void) { return drop_count; }
int64_t sg_drop_at(int64_t index) {
    return index >= 0 && index < drop_count ? drop_values[index] : -1;
}
