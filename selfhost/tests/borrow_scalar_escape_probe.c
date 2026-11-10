#include <stdint.h>
#include <stdlib.h>

static int64_t live, created, released;

int64_t se_acquire(void) {
    if (live != 0) exit(81);
    ++live;
    ++created;
    return 42;
}

void se_release(int64_t value) {
    if (value != 42 || live != 1 || created != released + 1) exit(82);
    --live;
    ++released;
}

int64_t se_counts(int64_t round) {
    return live == 0 && created == round && released == round ? 42 : 0;
}

void sengoo_panic_integer_overflow(int64_t value) { if (value) exit(83); }
void sengoo_panic_division_by_zero(int64_t value) { if (!value) exit(84); }
