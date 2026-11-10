#include "runtime_shared.h"
#include <stdint.h>
#include <stdlib.h>

static int moved;
static int dropped;
static void move64(void *destination, void *source) {
    *(int64_t *)destination = *(int64_t *)source;
    *(int64_t *)source = 0;
    ++moved;
}
static void drop64(void *value) { *(int64_t *)value = 0; ++dropped; }

int main(void) {
    if (sengoo_collections_abi_version() != 1) return 70;
    if (sengoo_raw_vec_new_parts(0, 8, (void *)move64, (void *)drop64) != 0) return 71;
    if (sengoo_raw_vec_new_parts(8, 8, NULL, (void *)drop64) != 0) return 72;
    if (sengoo_raw_vec_iter_new(0) != 0 || sengoo_raw_vec_iter_next(0) != NULL) return 73;
    for (int round = 0; round < 100; ++round) {
        long long vector = sengoo_raw_vec_new_parts(8, 8, (void *)move64, (void *)drop64);
        if (!vector) return 74;
        int64_t first = 40, second = 2;
        if (sengoo_raw_vec_push(vector, &first) != 0 || first != 0) return 75;
        if (sengoo_raw_vec_push(vector, &second) != 0 || second != 0) return 76;
        long long iterator = sengoo_raw_vec_iter_new(vector);
        if (!iterator) return 77;
        int64_t *left = sengoo_raw_vec_iter_next(iterator);
        int64_t *right = sengoo_raw_vec_iter_next(iterator);
        if (!left || !right || left == right || *left != 40 || *right != 2 || *left + *right != 42) return 78;
        if (sengoo_raw_vec_iter_next(iterator) != NULL || sengoo_raw_vec_iter_next(iterator) != NULL) return 79;
        if (sengoo_raw_vec_iter_free(iterator) != 0 || sengoo_raw_vec_free(vector) != 0) return 80;
    }
    if (moved != 200 || dropped != 200 || sengoo_opaque_live_handle_count() != 0) return 81;
    return 42;
}
