/* Public native token boundary: stale/cross-domain values never dereference. */
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdint.h>
#include <string.h>
#include "../runtime_string.c"
typedef struct { void* pointer; uint32_t generation; } ProbeSlot;
static ProbeSlot slots[128];
long long sengoo_opaque_handle_new(void* p) {
    for (size_t i = 1; i < 128; i++) if (!slots[i].pointer) {
        slots[i].pointer = p; slots[i].generation++;
        return ((long long)slots[i].generation << 32) | (long long)i;
    }
    return 0;
}
void* sengoo_opaque_handle_get(long long h) {
    size_t i = (size_t)((uint64_t)h & UINT32_MAX);
    return h > 0 && i < 128 && slots[i].generation == ((uint64_t)h >> 32) ? slots[i].pointer : NULL;
}
void* sengoo_opaque_handle_take(long long h) {
    void* p = sengoo_opaque_handle_get(h); if (p) slots[(uint64_t)h & UINT32_MAX].pointer = NULL; return p;
}
long long sengoo_opaque_live_handle_count(void) {
    long long count = 0; for (size_t i = 1; i < 128; i++) if (slots[i].pointer) count++; return count;
}
long long sengoo_ptr_to_handle(void* p) { return (long long)(intptr_t)p; }
void* sengoo_handle_to_ptr(long long p) { return (void*)(intptr_t)p; }
SengooFfiBuffer* sengoo_ffi_buffer_from_handle(long long h) { return (SengooFfiBuffer*)(intptr_t)h; }
long long sengoo_copy_bytes_to_managed_buffer(long long h, const char* bytes, size_t count) {
    SengooFfiBuffer* out = sengoo_ffi_buffer_from_handle(h);
    if (!out || count > out->capacity) return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    if (count) memcpy(out->bytes, bytes, count); out->used_len = count; return (long long)count;
}
static long long raw(const void* p) { return (long long)(intptr_t)p; }
int main(void) {
    const char text[] = "\xe4\xb8\xad" "a,b";
    long long owned = sengoo_string_from_bytes_copy(raw(text), sizeof(text) - 1); assert(owned > 0);
    long long chars = sengoo_string_chars_iter_new(owned), bytes = sengoo_string_bytes_iter_new(owned);
    long long split = sengoo_string_split_iter_new(owned, raw(","));
    assert(chars > 0 && bytes > 0 && split > 0 && sengoo_string_iter_live_handle_count() == 3);
    assert(sengoo_string_iter_done(split) == 1 && sengoo_string_iter_free_status(split) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_string_split_iter_done(chars) == 1 && sengoo_string_split_iter_free_status(chars) == -SENGOO_STATUS_INVALID_HANDLE);
    void* pointer = malloc(1); assert(pointer); long long foreign = sengoo_opaque_handle_new(pointer);
    assert(sengoo_string_iter_free_status(foreign) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_string_split_iter_free_status(foreign) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_string_iter_done(foreign) == 1 && sengoo_string_split_iter_done(foreign) == 1);
    assert(sengoo_string_chars_iter_next_or_default(chars, -1) == 0x4e2d);
    assert(sengoo_string_iter_free_status(chars) == 0 && sengoo_string_iter_free_status(chars) == 0);
    assert(sengoo_string_iter_done(chars) == 1 && sengoo_string_chars_iter_next_or_default(chars, -1) == -1);
    long long replacement = sengoo_string_chars_iter_new(owned); assert(replacement > 0 && replacement != chars);
    assert(sengoo_string_iter_free_status(chars) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_string_chars_iter_next_or_default(replacement, -1) == 0x4e2d);
    long long part = sengoo_string_split_iter_next(split); assert(part > 0 && sengoo_string_len(part) == 4);
    assert(sengoo_string_free_status(owned) == 0); /* iterator owns independent snapshots */
    assert(sengoo_string_bytes_iter_next_or_default(bytes, -1) == 0xe4);
    assert(sengoo_string_iter_free_status(replacement) == 0 && sengoo_string_iter_free_status(bytes) == 0);
    assert(sengoo_string_split_iter_free_status(split) == 0 && sengoo_string_split_iter_free_status(split) == 0);
    assert(!strcmp((const char*)(intptr_t)sengoo_string_as_str_ptr(part), "\xe4\xb8\xad" "a"));
    assert(sengoo_string_free_status(part) == 0 && sengoo_string_iter_live_handle_count() == 0);
    assert(sengoo_opaque_handle_take(foreign) == pointer); free(pointer);
    owned = sengoo_string_from_bytes_copy(raw("repeat,x"), 8); assert(owned > 0);
    for (int i = 0; i < 50000; i++) {
        chars = sengoo_string_chars_iter_new(owned); split = sengoo_string_split_iter_new(owned, raw(","));
        assert(chars > 0 && split > 0);
        assert(sengoo_string_iter_free_status(chars) == 0 && sengoo_string_iter_free_status(chars) == 0);
        assert(sengoo_string_split_iter_free_status(split) == 0 && sengoo_string_split_iter_free_status(split) == 0);
        assert(sengoo_string_iter_live_handle_count() == 0 && sengoo_opaque_live_handle_count() == 0);
    }
    assert(sengoo_string_iter_slot_count() <= 3 && g_text_iter_slot_capacity <= 8);
    assert(sengoo_string_free_status(owned) == 0 && sengoo_string_iter_free_status(0) == 0 && sengoo_string_split_iter_free_status(0) == 0);
    puts("string iterators: generation/domain checks, idempotent free, independent snapshots and 50000 stable cycles passed");
    return 0;
}
