#include "runtime_shared.h"

#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    char* data;
    size_t len;
    size_t cap;
} SengooOwnedString;

typedef struct {
    unsigned char* data;
    size_t len;
    size_t offset;
} SengooStringIter;

typedef struct {
    unsigned char* data;
    size_t len;
    unsigned char* delimiter;
    size_t delimiter_len;
    size_t offset;
    unsigned char finished;
} SengooStringSplitIter;

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

static int sengoo_utf8_is_boundary(const unsigned char* bytes, size_t len, size_t offset) {
    if (offset > len) {
        return 0;
    }
    if (offset == 0 || offset == len) {
        return 1;
    }
    return (bytes[offset] & 0xC0) != 0x80;
}

static int sengoo_utf8_decode_next(
    const unsigned char* bytes,
    size_t len,
    size_t* offset,
    long long* out_codepoint) {
    if (!bytes || !offset || !out_codepoint || *offset >= len) {
        return 0;
    }
    size_t i = *offset;
    unsigned char c = bytes[i];
    if (c <= 0x7F) {
        *out_codepoint = (long long)c;
        *offset = i + 1;
        return 1;
    }
    if ((c & 0xE0) == 0xC0 && i + 1 < len) {
        *out_codepoint = (long long)(((c & 0x1F) << 6) | (bytes[i + 1] & 0x3F));
        *offset = i + 2;
        return 1;
    }
    if ((c & 0xF0) == 0xE0 && i + 2 < len) {
        *out_codepoint =
            (long long)(((c & 0x0F) << 12) | ((bytes[i + 1] & 0x3F) << 6) | (bytes[i + 2] & 0x3F));
        *offset = i + 3;
        return 1;
    }
    if ((c & 0xF8) == 0xF0 && i + 3 < len) {
        *out_codepoint = (long long)(((c & 0x07) << 18)
            | ((bytes[i + 1] & 0x3F) << 12)
            | ((bytes[i + 2] & 0x3F) << 6)
            | (bytes[i + 3] & 0x3F));
        *offset = i + 4;
        return 1;
    }
    return 0;
}

static long long sengoo_string_iter_new_from_bytes(const char* data, size_t len) {
    SengooStringIter* iter = (SengooStringIter*)calloc(1, sizeof(SengooStringIter));
    if (!iter) {
        return 0;
    }
    if (len > 0) {
        iter->data = (unsigned char*)malloc(len);
        if (!iter->data) {
            free(iter);
            return 0;
        }
        memcpy(iter->data, data, len);
    }
    iter->len = len;
    iter->offset = 0;
    return sengoo_ptr_to_handle(iter);
}

static SengooStringIter* sengoo_string_iter_from_handle(long long handle) {
    if (handle <= 0) {
        return NULL;
    }
    return (SengooStringIter*)sengoo_handle_to_ptr(handle);
}

static SengooStringSplitIter* sengoo_string_split_iter_from_handle(long long handle) {
    if (handle <= 0) {
        return NULL;
    }
    return (SengooStringSplitIter*)sengoo_handle_to_ptr(handle);
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
    if (slot->generation != generation) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    if (!slot->alive || !slot->owned) {
        return SENGOO_STATUS_OK;
    }
    sengoo_owned_string_destroy(slot->owned);
    slot->owned = NULL;
    slot->alive = 0;
    return SENGOO_STATUS_OK;
}

long long sengoo_string_live_handle_count(void) {
    size_t live = 0;
    for (size_t index = 0; index < g_string_slot_count; ++index) {
        if (g_string_slots[index].alive && g_string_slots[index].owned) {
            live += 1;
        }
    }
    return (long long)live;
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

static void sengoo_string_write_line(long long handle, FILE* stream) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (owned && owned->data && owned->len > 0) {
        fwrite(owned->data, 1, owned->len, stream);
    }
    fputc('\n', stream);
}

void sengoo_print_string(long long handle) {
    sengoo_string_write_line(handle, stdout);
}

void sengoo_eprint_string(long long handle) {
    sengoo_string_write_line(handle, stderr);
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
        return -(long long)SENGOO_STATUS_INVALID_UTF8;
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

long long sengoo_str_slice_copy(long long value_ptr, long long start, long long end) {
    const char* value = (const char*)sengoo_handle_to_ptr(value_ptr);
    if (!value || start < 0 || end < start) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    size_t len = strlen(value);
    size_t start_offset = (size_t)start;
    size_t end_offset = (size_t)end;
    if (end_offset > len
        || !sengoo_utf8_is_boundary((const unsigned char*)value, len, start_offset)
        || !sengoo_utf8_is_boundary((const unsigned char*)value, len, end_offset)) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    return sengoo_owned_string_from_bytes(value + start_offset, end_offset - start_offset);
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
        return -(long long)SENGOO_STATUS_INVALID_UTF8;
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

long long sengoo_string_concat_str_status(long long handle, long long value_ptr) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    const char* value = (const char*)sengoo_handle_to_ptr(value_ptr);
    if (!owned || !value) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    long long copy_handle = sengoo_owned_string_from_bytes(owned->data ? owned->data : "", owned->len);
    if (copy_handle < 0) {
        return copy_handle;
    }
    long long pushed = sengoo_string_push_str_status(copy_handle, value_ptr);
    if (pushed < 0) {
        sengoo_string_free_status(copy_handle);
        return pushed;
    }
    return copy_handle;
}

static long long sengoo_owned_string_append_bytes(
    long long handle, const char* bytes, size_t add_len) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (!owned || (!bytes && add_len != 0)) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (add_len == 0) {
        return SENGOO_STATUS_OK;
    }
    if (!sengoo_bytes_are_utf8((const unsigned char*)bytes, add_len)) {
        return -(long long)SENGOO_STATUS_INVALID_UTF8;
    }
    size_t new_len = owned->len + add_len;
    if (!sengoo_owned_string_reserve(owned, sengoo_string_capacity_for_len(new_len))) {
        return -(long long)SENGOO_STATUS_OUT_OF_MEMORY;
    }
    memcpy(owned->data + owned->len, bytes, add_len);
    owned->len = new_len;
    owned->data[owned->len] = '\0';
    return SENGOO_STATUS_OK;
}

long long sengoo_string_with_capacity(long long capacity) {
    if (capacity < 0) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    long long handle = sengoo_owned_string_new_handle();
    if (handle <= 0) {
        return handle;
    }
    if (capacity > 0) {
        SengooOwnedString* owned = sengoo_string_resolve(handle);
        if (!sengoo_owned_string_reserve(
                owned, sengoo_string_capacity_for_len((size_t)capacity))) {
            sengoo_string_free_status(handle);
            return -(long long)SENGOO_STATUS_OUT_OF_MEMORY;
        }
    }
    return handle;
}

long long sengoo_string_push_i64_status(long long handle, long long value) {
    char buffer[32];
    int written = snprintf(buffer, sizeof(buffer), "%lld", value);
    if (written < 0) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    return sengoo_owned_string_append_bytes(handle, buffer, (size_t)written);
}

long long sengoo_string_push_bool_status(long long handle, long long value) {
    const char* text = value ? "true" : "false";
    return sengoo_owned_string_append_bytes(handle, text, strlen(text));
}

long long sengoo_string_push_char_status(long long handle, int codepoint) {
    if (codepoint < 0 || codepoint > 0x10FFFF
        || (codepoint >= 0xD800 && codepoint <= 0xDFFF)) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    unsigned char buf[4];
    size_t n;
    unsigned long cp = (unsigned long)codepoint;
    if (cp < 0x80) {
        buf[0] = (unsigned char)cp;
        n = 1;
    } else if (cp < 0x800) {
        buf[0] = (unsigned char)(0xC0 | (cp >> 6));
        buf[1] = (unsigned char)(0x80 | (cp & 0x3F));
        n = 2;
    } else if (cp < 0x10000) {
        buf[0] = (unsigned char)(0xE0 | (cp >> 12));
        buf[1] = (unsigned char)(0x80 | ((cp >> 6) & 0x3F));
        buf[2] = (unsigned char)(0x80 | (cp & 0x3F));
        n = 3;
    } else {
        buf[0] = (unsigned char)(0xF0 | (cp >> 18));
        buf[1] = (unsigned char)(0x80 | ((cp >> 12) & 0x3F));
        buf[2] = (unsigned char)(0x80 | ((cp >> 6) & 0x3F));
        buf[3] = (unsigned char)(0x80 | (cp & 0x3F));
        n = 4;
    }
    return sengoo_owned_string_append_bytes(handle, (const char*)buf, n);
}

long long sengoo_string_push_f64_precision_status(
    long long handle,
    double value,
    long long precision) {
    if (precision < 0 || precision > 15) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    char format[16];
    int format_written = snprintf(format, sizeof(format), "%%.%lldf", precision);
    if (format_written < 0 || (size_t)format_written >= sizeof(format)) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    char buffer[512];
    int written = snprintf(buffer, sizeof(buffer), format, value);
    if (written < 0) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if ((size_t)written >= sizeof(buffer)) {
        return -(long long)SENGOO_STATUS_BUFFER_TOO_SMALL;
    }
    return sengoo_owned_string_append_bytes(handle, buffer, (size_t)written);
}

static long long sengoo_owned_string_append_repeated_byte(
    long long handle,
    char byte,
    size_t count) {
    if (count == 0) {
        return SENGOO_STATUS_OK;
    }
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (!owned) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    size_t new_len = owned->len + count;
    if (new_len < owned->len) {
        return -(long long)SENGOO_STATUS_OVERFLOW;
    }
    if (!sengoo_owned_string_reserve(owned, sengoo_string_capacity_for_len(new_len))) {
        return -(long long)SENGOO_STATUS_OUT_OF_MEMORY;
    }
    memset(owned->data + owned->len, (unsigned char)byte, count);
    owned->len = new_len;
    owned->data[owned->len] = '\0';
    return SENGOO_STATUS_OK;
}

long long sengoo_string_push_padded_string_status(
    long long handle,
    long long value_handle,
    long long align,
    long long width) {
    SengooOwnedString* value = sengoo_string_resolve(value_handle);
    if (!value || width < 0) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    size_t value_len = value->len;
    size_t target_width = (size_t)width;
    size_t pad = target_width > value_len ? target_width - value_len : 0;

    if (align == 1) {
        long long padded = sengoo_owned_string_append_repeated_byte(handle, ' ', pad);
        if (padded < 0) {
            return padded;
        }
        return sengoo_owned_string_append_bytes(handle, value->data ? value->data : "", value_len);
    }

    return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
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

long long sengoo_string_slice_status(long long handle, long long start, long long end) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (!owned || start < 0 || end < start) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    size_t start_offset = (size_t)start;
    size_t end_offset = (size_t)end;
    if (end_offset > owned->len
        || !sengoo_utf8_is_boundary((const unsigned char*)owned->data, owned->len, start_offset)
        || !sengoo_utf8_is_boundary((const unsigned char*)owned->data, owned->len, end_offset)) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    return sengoo_owned_string_from_bytes(owned->data + start_offset, end_offset - start_offset);
}

long long sengoo_string_bytes_iter_new(long long handle) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (!owned) {
        return 0;
    }
    return sengoo_string_iter_new_from_bytes(owned->data ? owned->data : "", owned->len);
}

long long sengoo_string_chars_iter_new(long long handle) {
    return sengoo_string_bytes_iter_new(handle);
}

long long sengoo_string_iter_done(long long iter_handle) {
    SengooStringIter* iter = sengoo_string_iter_from_handle(iter_handle);
    if (!iter) {
        return 1;
    }
    return iter->offset >= iter->len ? 1 : 0;
}

long long sengoo_string_bytes_iter_next_or_default(long long iter_handle, long long fallback) {
    SengooStringIter* iter = sengoo_string_iter_from_handle(iter_handle);
    if (!iter || iter->offset >= iter->len) {
        return fallback;
    }
    unsigned char value = iter->data[iter->offset];
    iter->offset += 1;
    return (long long)value;
}

long long sengoo_string_chars_iter_next_or_default(long long iter_handle, long long fallback) {
    SengooStringIter* iter = sengoo_string_iter_from_handle(iter_handle);
    if (!iter || iter->offset >= iter->len) {
        return fallback;
    }
    long long codepoint = fallback;
    if (!sengoo_utf8_decode_next(iter->data, iter->len, &iter->offset, &codepoint)) {
        iter->offset = iter->len;
        return fallback;
    }
    return codepoint;
}

int sengoo_string_chars_iter_next_char(long long iter_handle, int fallback) {
    SengooStringIter* iter = sengoo_string_iter_from_handle(iter_handle);
    if (!iter || iter->offset >= iter->len) {
        return fallback;
    }
    long long codepoint = (long long)(unsigned int)fallback;
    if (!sengoo_utf8_decode_next(iter->data, iter->len, &iter->offset, &codepoint)) {
        iter->offset = iter->len;
        return fallback;
    }
    return (int)codepoint;
}

long long sengoo_string_iter_free_status(long long iter_handle) {
    SengooStringIter* iter = sengoo_string_iter_from_handle(iter_handle);
    if (!iter) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    free(iter->data);
    free(iter);
    return SENGOO_STATUS_OK;
}

long long sengoo_string_split_iter_new(long long handle, long long delimiter_ptr) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    const char* delimiter = (const char*)sengoo_handle_to_ptr(delimiter_ptr);
    if (!owned || !delimiter) {
        return 0;
    }

    SengooStringSplitIter* iter =
        (SengooStringSplitIter*)calloc(1, sizeof(SengooStringSplitIter));
    if (!iter) {
        return 0;
    }

    size_t delimiter_len = strlen(delimiter);
    if (owned->len > 0) {
        iter->data = (unsigned char*)malloc(owned->len);
        if (!iter->data) {
            free(iter);
            return 0;
        }
        memcpy(iter->data, owned->data, owned->len);
    }
    if (delimiter_len > 0) {
        iter->delimiter = (unsigned char*)malloc(delimiter_len);
        if (!iter->delimiter) {
            free(iter->data);
            free(iter);
            return 0;
        }
        memcpy(iter->delimiter, delimiter, delimiter_len);
    }

    iter->len = owned->len;
    iter->delimiter_len = delimiter_len;
    iter->offset = 0;
    iter->finished = delimiter_len == 0 ? 1 : 0;
    return sengoo_ptr_to_handle(iter);
}

long long sengoo_string_split_iter_done(long long iter_handle) {
    SengooStringSplitIter* iter = sengoo_string_split_iter_from_handle(iter_handle);
    if (!iter) {
        return 1;
    }
    return iter->finished ? 1 : 0;
}

static size_t sengoo_string_split_find_delimiter(SengooStringSplitIter* iter) {
    if (!iter || !iter->delimiter || iter->delimiter_len == 0 || iter->delimiter_len > iter->len) {
        return SIZE_MAX;
    }
    size_t max_start = iter->len - iter->delimiter_len;
    for (size_t index = iter->offset; index <= max_start; ++index) {
        if (memcmp(iter->data + index, iter->delimiter, iter->delimiter_len) == 0) {
            return index;
        }
    }
    return SIZE_MAX;
}

long long sengoo_string_split_iter_next(long long iter_handle) {
    SengooStringSplitIter* iter = sengoo_string_split_iter_from_handle(iter_handle);
    if (!iter || iter->finished) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }

    size_t start = iter->offset;
    size_t end = iter->len;
    size_t delimiter_at = sengoo_string_split_find_delimiter(iter);
    if (delimiter_at != SIZE_MAX) {
        end = delimiter_at;
        iter->offset = delimiter_at + iter->delimiter_len;
    } else {
        iter->finished = 1;
    }

    if (iter->offset > iter->len) {
        iter->offset = iter->len;
        iter->finished = 1;
    }

    const char* segment = iter->data ? (const char*)(iter->data + start) : "";
    return sengoo_owned_string_from_bytes(segment, end >= start ? end - start : 0);
}

long long sengoo_string_split_iter_free_status(long long iter_handle) {
    SengooStringSplitIter* iter = sengoo_string_split_iter_from_handle(iter_handle);
    if (!iter) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    free(iter->data);
    free(iter->delimiter);
    free(iter);
    return SENGOO_STATUS_OK;
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

long long sengoo_string_compare(long long lhs_handle, long long rhs_handle) {
    SengooOwnedString* lhs = sengoo_string_resolve(lhs_handle);
    SengooOwnedString* rhs = sengoo_string_resolve(rhs_handle);
    if (!lhs || !rhs) {
        return 0;
    }
    size_t min_len = lhs->len < rhs->len ? lhs->len : rhs->len;
    int cmp = 0;
    if (min_len > 0) {
        cmp = memcmp(lhs->data, rhs->data, min_len);
    }
    if (cmp < 0) {
        return -1;
    }
    if (cmp > 0) {
        return 1;
    }
    if (lhs->len < rhs->len) {
        return -1;
    }
    if (lhs->len > rhs->len) {
        return 1;
    }
    return 0;
}

/* Unicode 17.0.0 baseline: UTF-8 validation + scalar iteration only.
 * Full property tables / casefold remain follow-up; provenance pin is the version string. */
static const char SENGOO_UNICODE_VERSION[] = "17.0.0";
static const char SENGOO_UNICODE_PROVENANCE[] =
    "Unicode 17.0.0; UTF-8 well-formedness + scalar decode only (no property tables)";

long long sengoo_unicode_version_copy(long long out_buffer, long long out_capacity) {
    if (out_capacity < 0) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    char* out = (char*)(intptr_t)out_buffer;
    size_t need = sizeof(SENGOO_UNICODE_VERSION) - 1;
    if (!out || (size_t)out_capacity < need) {
        return -(long long)SENGOO_STATUS_BUFFER_TOO_SMALL;
    }
    memcpy(out, SENGOO_UNICODE_VERSION, need);
    return (long long)need;
}

long long sengoo_unicode_provenance_copy(long long out_buffer, long long out_capacity) {
    if (out_capacity < 0) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    char* out = (char*)(intptr_t)out_buffer;
    size_t need = sizeof(SENGOO_UNICODE_PROVENANCE) - 1;
    if (!out || (size_t)out_capacity < need) {
        return -(long long)SENGOO_STATUS_BUFFER_TOO_SMALL;
    }
    memcpy(out, SENGOO_UNICODE_PROVENANCE, need);
    return (long long)need;
}

long long sengoo_string_char_count(long long handle) {
    SengooOwnedString* owned = sengoo_string_resolve(handle);
    if (!owned) {
        return -(long long)SENGOO_STATUS_INVALID_HANDLE;
    }
    size_t offset = 0;
    long long count = 0;
    long long codepoint = 0;
    while (offset < owned->len) {
        if (!sengoo_utf8_decode_next(
                (const unsigned char*)owned->data,
                owned->len,
                &offset,
                &codepoint)) {
            return -(long long)SENGOO_STATUS_INVALID_UTF8;
        }
        count += 1;
    }
    return count;
}

long long sengoo_char_codepoint(int codepoint) {
    if (codepoint < 0 || codepoint > 0x10FFFF
        || (codepoint >= 0xD800 && codepoint <= 0xDFFF)) {
        return -(long long)SENGOO_STATUS_INVALID_ARGUMENT;
    }
    return (long long)(unsigned int)codepoint;
}

long long sengoo_string_from_utf8(long long buffer_handle, long long used_len) {
    return sengoo_string_from_buffer(buffer_handle, used_len);
}
