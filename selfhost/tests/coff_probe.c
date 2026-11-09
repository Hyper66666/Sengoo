#include <stdio.h>

extern long long sgprobe(void);
extern long long sengoo_native_probe_long(void);

int main(void) {
    if (sgprobe() != 42 || sengoo_native_probe_long() != 42) return 1;
    puts("SG COFF: short and long symbols linked and returned 42");
    return 0;
}
