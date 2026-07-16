#include "runtime_shared.h"

#include <stdlib.h>
#include <string.h>

typedef struct {
    unsigned char* bytes;
    size_t capacity;
    size_t used_len;
    size_t position;
} SengooStreamCursor;

static size_t g_sengoo_stream_cursor_live_count = 0;

static SengooStreamCursor* sengoo_stream_cursor_get(long long handle) {
    return (SengooStreamCursor*)sengoo_opaque_handle_get(handle);
}

static long long sengoo_stream_cursor_allocate(size_t capacity) {
    if (capacity > SENGOO_RUNTIME_MAX_BUFFER_BYTES) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    SengooStreamCursor* cursor = (SengooStreamCursor*)calloc(1, sizeof(SengooStreamCursor));
    if (!cursor) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    if (capacity > 0) {
        cursor->bytes = (unsigned char*)calloc(capacity, 1);
        if (!cursor->bytes) {
            free(cursor);
            return -SENGOO_STATUS_OUT_OF_MEMORY;
        }
    }
    cursor->capacity = capacity;

    long long handle = sengoo_opaque_handle_new(cursor);
    if (handle <= 0) {
        free(cursor->bytes);
        free(cursor);
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    g_sengoo_stream_cursor_live_count += 1;
    return handle;
}

long long sengoo_stream_cursor_from_buffer(long long buffer_handle) {
    SengooFfiBuffer* source = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!source) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }

    long long handle = sengoo_stream_cursor_allocate(source->capacity);
    if (handle <= 0) {
        return handle;
    }
    SengooStreamCursor* cursor = sengoo_stream_cursor_get(handle);
    if (source->used_len > 0) {
        memcpy(cursor->bytes, source->bytes, source->used_len);
    }
    cursor->used_len = source->used_len;
    return handle;
}

long long sengoo_stream_cursor_with_capacity(long long capacity) {
    if (capacity < 0) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    return sengoo_stream_cursor_allocate((size_t)capacity);
}

long long sengoo_stream_cursor_position(long long cursor_handle) {
    SengooStreamCursor* cursor = sengoo_stream_cursor_get(cursor_handle);
    if (!cursor) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    return (long long)cursor->position;
}

long long sengoo_stream_cursor_read(long long cursor_handle, long long out_buffer_handle) {
    SengooStreamCursor* cursor = sengoo_stream_cursor_get(cursor_handle);
    SengooFfiBuffer* out = sengoo_ffi_buffer_from_handle(out_buffer_handle);
    if (!cursor || !out) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (cursor->position >= cursor->used_len) {
        return 0;
    }
    if (out->capacity == 0) {
        return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    }

    size_t remaining = cursor->used_len - cursor->position;
    size_t count = remaining < out->capacity ? remaining : out->capacity;
    memcpy(out->bytes, cursor->bytes + cursor->position, count);
    out->used_len = count;
    cursor->position += count;
    return (long long)count;
}

long long sengoo_stream_cursor_write(
    long long cursor_handle,
    long long data_buffer_handle,
    long long used_len
) {
    SengooStreamCursor* cursor = sengoo_stream_cursor_get(cursor_handle);
    SengooFfiBuffer* data = sengoo_ffi_buffer_from_handle(data_buffer_handle);
    if (!cursor || !data) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (used_len < 0 || (size_t)used_len > data->used_len) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (used_len == 0) {
        return 0;
    }
    if (cursor->position != cursor->used_len) {
        return -SENGOO_STATUS_UNSUPPORTED;
    }

    size_t available = cursor->capacity - cursor->used_len;
    if (available == 0) {
        return 0;
    }
    size_t requested = (size_t)used_len;
    size_t count = requested < available ? requested : available;
    memcpy(cursor->bytes + cursor->used_len, data->bytes, count);
    cursor->used_len += count;
    cursor->position += count;
    return (long long)count;
}

long long sengoo_stream_cursor_free(long long cursor_handle) {
    SengooStreamCursor* cursor = (SengooStreamCursor*)sengoo_opaque_handle_take(cursor_handle);
    if (!cursor) {
        return 0;
    }
    free(cursor->bytes);
    free(cursor);
    if (g_sengoo_stream_cursor_live_count > 0) {
        g_sengoo_stream_cursor_live_count -= 1;
    }
    return 0;
}

long long sengoo_stream_cursor_live_handle_count(void) {
    return (long long)g_sengoo_stream_cursor_live_count;
}
