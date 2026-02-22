#include <stdint.h>
#include <stdio.h>

int64_t sengoo_add_export(int64_t a, int64_t b);

int main(void) {
    printf("%lld\n", (long long)sengoo_add_export(40, 2));
    return 0;
}
