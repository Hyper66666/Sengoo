#include <stdio.h>
#include <stdlib.h>
#include <string.h>

void sengoo_print_i64(long long val) {
    printf("%lld\n", val);
}

void sengoo_print_bool(long long val) {
    printf("%s\n", val ? "true" : "false");
}

void sengoo_print_f64(double val) {
    printf("%g\n", val);
}

void sengoo_print_str(const char* s) {
    if (s) {
        printf("%s\n", s);
    } else {
        printf("\n");
    }
}

void* sengoo_alloc(long long size, long long align) {
    if (align <= 0) align = 1;
    return malloc((size_t)size);
}

void sengoo_free(void* ptr, long long size, long long align) {
    free(ptr);
}

void* sengoo_realloc(void* ptr, long long old_size, long long old_align, long long new_size) {
    return realloc(ptr, (size_t)new_size);
}
