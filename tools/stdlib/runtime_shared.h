#ifndef SENGOO_RUNTIME_SHARED_H
#define SENGOO_RUNTIME_SHARED_H

#include <stddef.h>

enum {
    SENGOO_RUNTIME_MAX_BUFFER_BYTES = 64 * 1024 * 1024,
    SENGOO_RUNTIME_MAX_COMMAND_OUTPUT_BYTES = 16 * 1024 * 1024,

    SENGOO_STATUS_OK = 0,
    SENGOO_STATUS_UNKNOWN = 1,
    SENGOO_STATUS_INVALID_ARGUMENT = 2,
    SENGOO_STATUS_INVALID_HANDLE = 3,
    SENGOO_STATUS_BUFFER_TOO_SMALL = 4,
    SENGOO_STATUS_NOT_FOUND = 5,
    SENGOO_STATUS_ALREADY_EXISTS = 6,
    SENGOO_STATUS_PERMISSION_DENIED = 7,
    SENGOO_STATUS_UNSUPPORTED = 8,
    SENGOO_STATUS_IO = 9,
    SENGOO_STATUS_PARSE = 10,
    SENGOO_STATUS_TIMEOUT = 11,
    SENGOO_STATUS_INTERRUPTED = 12,
    SENGOO_STATUS_OVERFLOW = 13,
    SENGOO_STATUS_OUT_OF_MEMORY = 14
};

typedef struct {
    unsigned char* bytes;
    size_t capacity;
    size_t used_len;
} SengooFfiBuffer;

long long sengoo_ptr_to_handle(void* ptr);
void* sengoo_handle_to_ptr(long long handle);
SengooFfiBuffer* sengoo_ffi_buffer_from_handle(long long handle);
long long sengoo_copy_bytes_to_managed_buffer(long long buffer_handle, const char* bytes, size_t len);
char* sengoo_copy_cstr_from_handle(long long value_ptr);
char* sengoo_strdup_bytes(const char* value);

#ifdef _WIN32
int sengoo_size_add(size_t* total, size_t value);
char* sengoo_windows_append_arg(char* out, const char* arg);
char* sengoo_windows_append_quoted_arg(char* out, const char* arg);
#endif

#endif
