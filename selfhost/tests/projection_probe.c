#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static int64_t live_count, created_count, free_count, observation_count;
static const unsigned char text[] = { 'h', 'e', 'l', 'l', 'o', ' ',
    0xe4, 0xb8, 0x96, 0xe7, 0x95, 0x8c, 0 };

void sengoo_panic_integer_overflow(int64_t value) { if (value) exit(63); }
void sengoo_panic_division_by_zero(int64_t value) { if (!value) exit(62); }
int64_t sg_acquire(void) {
    if (live_count) exit(61);
    ++live_count; ++created_count;
    return 7;
}
void sg_release(int64_t value) {
    if (value != 7 || live_count != 1) exit(60);
    if (observation_count != (free_count + 1) * 2) exit(54);
    --live_count; ++free_count;
}
int64_t sg_live(void) { return live_count; }
int64_t sg_created(void) { return created_count; }
int64_t sg_frees(void) { return free_count; }

/* Exact reserved observation contracts: no retention and no release. */
int64_t sengoo_ffi_buffer_copy_out(int64_t handle, int64_t output, int64_t capacity) {
    if (handle != 0 || output != 7 || capacity != 0 || live_count != 1) exit(59);
    ++observation_count;
    return 21;
}
int64_t sengoo_ffi_buffer_ptr(int64_t handle) {
    if (handle != 7 || live_count != 1) exit(58);
    return 7;
}
int64_t sengoo_ffi_buffer_len(int64_t handle) {
    if (handle != 7 || live_count != 1) exit(57);
    return 0;
}
int64_t sengoo_string_as_str_ptr(int64_t handle) {
    if (handle != 7 || live_count != 1) exit(56);
    return (int64_t)(intptr_t)text;
}
int64_t sg_observe_text(const unsigned char *value) {
    if (live_count != 1 || memcmp(value, text, sizeof(text)) != 0) exit(55);
    ++observation_count;
    return 21;
}
