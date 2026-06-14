#include "runtime_shared.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    char* data;
    size_t len;
    size_t cap;
} SengooOwnedString;

typedef struct {
    SengooOwnedString* owned;
    uint32_t generation;
    unsigned char alive;
} SengooStringSlot;

static SengooStringSlot* g_string_slots = NULL;
static size_t g_string_slot_count = 0;
static size_t g_string_slot_capacity = 0;

static size_t sengoo_string_capacity_for_len(size_t len) {
    return len + 1;
}

static int sengoo_string_slot_ensure_capacity(size_t min_slots) {
    if (g_string_slot_capacity >= min_slots) {
        return 1;
    }
    size_t new_cap = g_string_slot_capacity == 0 ? 8 : g_string_slot_capacity;
    while (new_cap < min_slots) {
        if (new_cap > (SIZE_MAX / 2)) {
            return 0;
        }
        new_cap *= 2;
    }
    SengooStringSlot* next =
        (SengooStringSlot*)realloc(g_string_slots, new_cap * sizeof(SengooStringSlot));
    if (!next) {
        return 0;
    }
    if (new_cap > g_string_slot_capacity) {
        memset(
            next + g_string_slot_capacity,
            0,
            (new_cap - g_string_slot_capacity) * sizeof(SengooStringSlot));
    }
    g_string_slots = next;
    g_string_slot_capacity = new_cap;
    return 1;
}

static long long sengoo_string_alloc_handle(SengooOwnedString* owned) {
    size_t index = 0;
    for (; index < g_string_slot_count; ++index) {
        if (!g_string_slots[index].alive) {
            break;
        }
    }
    if (index == g_string_slot_count) {
        if (!sengoo_string_slot_ensure_capacity(g_string_slot_count + 1)) {
            return -(long long)SENGOO_STATUS_OUT_OF_MEMORY;
        }
        g_string_slot_count += 1;
    }

    SengooStringSlot* slot = &g_string_slots[index];
    slot->owned = owned;
    slot->alive = 1;
    slot->generation += 1;
    if (slot->generation == 0) {
        slot->generation = 1;
    }
    return ((long long)slot->generation << 32) | (long long)(index + 1);
}

static int sengoo_string_decode_handle(long long handle, size_t* out_index, uint32_t* out_generation) {
    if (handle <= 0) {
        return 0;
    }
    size_t index = ((size_t)handle & 0xFFFFFFFFu) - 1;
    uint32_t generation = (uint32_t)((unsigned long long)handle >> 32);
    if (index >= g_string_slot_count) {
        return 0;
    }
    *out_index = index;
    *out_generation = generation;
    return 1;
}

static SengooOwnedString* sengoo_string_resolve(long long handle) {
    size_t index = 0;
    uint32_t generation = 0;
    if (!sengoo_string_decode_handle(handle, &index, &generation)) {
        return NULL;
    }
    SengooStringSlot* slot = &g_string_slots[index];
    if (!slot->alive || slot->generation != generation || !slot->owned) {
        return NULL;
    }
    return slot->owned;
}

static void sengoo_owned_string_destroy(SengooOwnedString* owned) {
    if (!owned) {
        return;
    }
    free(owned->data);
    free(owned);
}

static int sengoo_bytes_are_utf8(const unsigned char* bytes, size_t len) {
    size_t i = 0;
    while (i < len) {
        unsigned char c = bytes[i];
        if (c <= 0x7F) {
            i++;
        } else if ((c & 0xE0) == 0xC0) {
            if (i + 1 >= len || (bytes[i + 1] & 0xC0) != 0x80 || c < 0xC2) {
                return 0;
            }
            i += 2;
        } else if ((c & 0xF0) == 0xE0) {
            if (i + 2 >= len || (bytes[i + 1] & 0xC0) != 0x80 || (bytes[i + 2] & 0xC0) != 0x80) {
                return 0;
            }
            if (c == 0xE0 && bytes[i + 1] < 0xA0) {
                return 0;
            }
            if (c == 0xED && bytes[i + 1] >= 0xA0) {
                return 0;
            }
            i += 3;
        } else if ((c & 0xF8) == 0xF0) {
            if (i + 3 >= len || (bytes[i + 1] & 0xC0) != 0x80 || (bytes[i + 2] & 0xC0) != 0x80 ||
                (bytes[i + 3] & 0xC0) != 0x80) {
                return 0;
            }
            if (c == 0xF0 && bytes[i + 1] < 0x90) {
                return 0;
            }
            if (c > 0xF4 || (c == 0xF4 && bytes[i + 1] > 0x8F)) {
                return 0;
            }
            i += 4;
        } else {
            return 0;
        }
    }
    return 1;
}

static int sengoo_owned_string_reserve(SengooOwnedString* owned, size_t min_capacity) {
    if (owned->cap >= min_capacity) {
        return 1;
    }
    size_t new_cap = owned->cap == 0 ? 8 : owned->cap;
    while (new_cap < min_capacity) {
        if (new_cap > (SIZE_MAX / 2)) {
            return 0;
        }
        new_cap *= 2;
    }
    char* next = (char*)realloc(owned->data, new_cap);
    if (!next) {
        return 0;
    }
    owned->data = next;
    owned->cap = new_cap;
    return 1;
}

static long long sengoo_owned_string_new_handle(void) {
    SengooOwnedString* owned = (SengooOwnedString*)calloc(1, sizeof(SengooOwnedString));
    if (!owned) {
        return -(long long)SENGOO_STATUS_OUT_OF_MEMORY;
    }
    return sengoo_string_alloc_handle(owned);
}

long long sengoo_string_new(void) {
    return sengoo_owned_string_new_handle();
}

long long sengoo_string_free_status(long long handle) {
    size_t index = 0;
    uint32_t generation = 0;
    if (!sengoo_string_decode_handle(handle, &index, &generation)) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    SengooStringSlot* slot = &g_string_slots[index];
    if (!slot->alive || slot->generation != generation || !slot->owned) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    sengoo_owned_string_destroy(slot->owned);
    slot->owned = NULL;
    slot->alive = 0;
    return SENGOO_STATUS_OK;
}

long long sengoo_string_len(long long handle) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (!owned) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    return (long long)owned->len;
}

long long sengoo_string_is_empty(long long handle) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (!owned) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    return owned->len == 0 ? 1 : 0;
}

long long sengoo_string_as_str_ptr(long long handle) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (!owned) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    if (owned->len == 0) {
        return sengoo_ptr_to_handle("");
    }
    if (!owned->data) {
        return sengoo_ptr_to_handle("");
    }
    if (!sengoo_owned_string_reserve(owned, sengoo_string_capacity_for_len(owned->len))) {
        return -(long long)SENGOO_STATUS_OUT_OF_MEMORY;
    }
    owned->data[owned->len] = '\0';
    return sengoo_ptr_to_handle(owned->data);
}

static long long sengoo_owned_string_from_bytes(const char* bytes, size_t len) {
    if (!bytes && len != 0) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (!sengoo_bytes_are_utf8((const unsigned char*)bytes, len)) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    long long handle = sengoo_owned_string_new_handle();
    if (handle <= 0) {
        return handle;
    }
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (len == 0) {
        return handle;
    }
    if (!sengoo_owned_string_reserve(owned, sengoo_string_capacity_for_len(len))) {
        sengoo_string_free_status(handle);
        return -(long long)SENGOO_STATUS_OUT_OF_MEMORY;
    }
    memcpy(owned->data, bytes, len);
    owned->len = len;
    owned->data[len] = '\0';
    return handle;
}

long long sengoo_string_from_bytes_copy(long long bytes_ptr, long long len) {
    if (len < 0) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    const char* bytes = (const char*)(intptr_t)bytes_ptr;
    return sengoo_owned_string_from_bytes(bytes, (size_t)len);
}

long long sengoo_string_from_str_copy(long long value_ptr) {
    const char* value = (const char*)sengoo_handle_to_ptr(value_ptr);
    if (!value) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    return sengoo_owned_string_from_bytes(value, strlen(value));
}

long long sengoo_string_from_buffer(long long buffer_handle, long long used_len) {
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    if (used_len < 0 || (size_t)used_len > buffer->used_len) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    return sengoo_owned_string_from_bytes((const char*)buffer->bytes, (size_t)used_len);
}

long long sengoo_string_clone_status(long long handle) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (!owned) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    return sengoo_owned_string_from_bytes(owned->data, owned->len);
}

long long sengoo_string_push_str_status(long long handle, long long value_ptr) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    const char* value = (const char*)sengoo_handle_to_ptr(value_ptr);
    if (!owned || !value) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    size_t add_len = strlen(value);
    if (add_len == 0) {
        return SENGOO_STATUS_OK;
    }
    if (!sengoo_bytes_are_utf8((const unsigned char*)value, add_len)) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    size_t new_len = owned->len + add_len;
    if (!sengoo_owned_string_reserve(owned, sengoo_string_capacity_for_len(new_len))) {
        return -(long long)SENGOO_STATUS_OUT_OF_MEMORY;
    }
    memcpy(owned->data + owned->len, value, add_len);
    owned->len = new_len;
    owned->data[owned->len] = '\0';
    return SENGOO_STATUS_OK;
}

long long sengoo_string_concat_str(long long handle, char* value) {
    long long status = sengoo_string_push_str_status(handle, sengoo_ptr_to_handle(value));
    if (status < 0) {
        return status;
    }
    return handle;
}

long long sengoo_string_clear_status(long long handle) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (!owned) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    owned->len = 0;
    if (owned->data && owned->cap > 0) {
        owned->data[0] = '\0';
    }
    return SENGOO_STATUS_OK;
}

long long sengoo_string_copy_to_buffer(long long handle, long long buffer_handle) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (!owned) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    return sengoo_copy_bytes_to_managed_buffer(
        buffer_handle,
        owned->data ? owned->data : "",
        owned->len);
}

long long sengoo_string_eq(long long lhs_handle, long long rhs_handle) {
    SengooOwnedString* lhs = sengoo_string_resolve(lhs_handle);
    SengooOwnedString* rhs = sengoo_string_resolve(rhs_handle);
    if (!lhs || !rhs) {
        return 0;
    }
    if (lhs->len != rhs->len) {
        return 0;
    }
    if (lhs->len == 0) {
        return 1;
    }
    return memcmp(lhs->data, rhs->data, lhs->len) == 0 ? 1 : 0;
}
