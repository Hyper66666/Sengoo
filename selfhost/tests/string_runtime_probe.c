#include <stdint.h>
#include <stdlib.h>
#include <string.h>

extern long long sengoo_string_from_bytes_copy(long long, long long);
extern long long sengoo_string_free_status(long long);
extern long long sengoo_string_live_handle_count(void);
extern long long sengoo_buffer_live_handle_count(void);
extern long long sengoo_opaque_live_handle_count(void);

static const unsigned char text[] = { 'h', 'e', 'l', 'l', 'o', ' ',
    0xe4, 0xb8, 0x96, 0xe7, 0x95, 0x8c, 0 };
static long long created_count, free_count, observation_count;

int64_t sg_acquire(void) {
    if (sengoo_string_live_handle_count() || sengoo_buffer_live_handle_count() ||
        sengoo_opaque_live_handle_count()) exit(61);
    long long handle = sengoo_string_from_bytes_copy((long long)(intptr_t)text, 12);
    if (handle <= 0 || sengoo_string_live_handle_count() != 1) exit(60);
    ++created_count;
    return handle;
}
void sg_release(int64_t handle) {
    if (sengoo_string_live_handle_count() != 1 || observation_count != (free_count + 1) * 2) exit(59);
    if (sengoo_string_free_status(handle) || sengoo_string_live_handle_count() ||
        sengoo_buffer_live_handle_count() || sengoo_opaque_live_handle_count()) exit(58);
    ++free_count;
}
int64_t sg_live(void) { return sengoo_string_live_handle_count(); }
int64_t sg_created(void) { return created_count; }
int64_t sg_frees(void) { return free_count; }
int64_t sg_observe_text(const unsigned char *value) {
    if (sengoo_string_live_handle_count() != 1 || memcmp(value, text, sizeof(text))) exit(57);
    ++observation_count;
    return 21;
}
