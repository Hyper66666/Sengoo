#include "runtime_shared.h"
#include <stdint.h>
#include <stdlib.h>

typedef struct { int64_t handle, marker; } GeneratedVec;
extern GeneratedVec sg_generated_i64(void);
extern GeneratedVec sg_generated_bool(void);
extern GeneratedVec sg_generated_owner(void);

typedef struct { int64_t token, value; } Owner;
static void *tokens[100];
static int live, released, bad;
void generated_release(int64_t token) {
    for (int i = 0; i < 100; ++i) {
        if (tokens[i] && (int64_t)(intptr_t)tokens[i] == token) {
            free(tokens[i]); tokens[i] = NULL; --live; ++released; return;
        }
    }
    bad = 1;
}

int main(void) {
    for (int round = 0; round < 100; ++round) {
        GeneratedVec scalar_value = sg_generated_i64(), boolean_value = sg_generated_bool(), owner_value = sg_generated_owner();
        if (scalar_value.marker || boolean_value.marker || owner_value.marker) return 69;
        long long scalar = scalar_value.handle, boolean = boolean_value.handle, owner = owner_value.handle;
        if (!scalar || !boolean || !owner) return 70;
        for (int i = 0; i < 100; ++i) {
            int64_t value = i;
            uint8_t flag = (uint8_t)(i & 1);
            tokens[i] = malloc(8); if (!tokens[i]) return 71; ++live;
            Owner record = {(int64_t)(intptr_t)tokens[i], i};
            if (sengoo_raw_vec_push(scalar, &value) != 0 || value != 0) return 72;
            if (sengoo_raw_vec_push(boolean, &flag) != 0 || flag != 0) return 73;
            if (sengoo_raw_vec_push(owner, &record) != 0 || record.token != 0 || record.value != 0) return 74;
        }
        for (int i = 0; i < 100; ++i) {
            const int64_t *value = sengoo_raw_vec_get(scalar, i);
            const uint8_t *flag = sengoo_raw_vec_get(boolean, i);
            const Owner *record = sengoo_raw_vec_get(owner, i);
            if (!value || !flag || !record || *value != i || *flag != (i & 1) ||
                record->token != (int64_t)(intptr_t)tokens[i] || record->value != i) return 75;
        }
        if (live != 100 || bad) return 76;
        if (sengoo_raw_vec_free(owner) != 0 || sengoo_raw_vec_free(scalar) != 0 || sengoo_raw_vec_free(boolean) != 0) return 77;
        if (live || bad || released != (round + 1) * 100 || sengoo_opaque_live_handle_count() != 0) return 78;
    }
    return 42;
}
