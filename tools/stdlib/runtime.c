#define _CRT_SECURE_NO_WARNINGS

#include <assert.h>
#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>

#include "runtime_shared.h"

extern long long sengoo_string_from_bytes_copy(long long bytes_ptr, long long len);

#ifdef _WIN32
#include <direct.h>
#include <io.h>
#include <windows.h>
#else
#include <dirent.h>
#include <fcntl.h>
#include <signal.h>
#include <sys/wait.h>
#include <unistd.h>
#endif

void sengoo_print_i64(long long val) {
    printf("%lld\n", val);
}

void sengoo_print_bool(long long val) {
    printf("%s\n", val ? "true" : "false");
}

void sengoo_print_f64(double val) {
    printf("%g\n", val);
}

void sengoo_print_str(const char* s) {
    if (s) {
        printf("%s\n", s);
    } else {
        printf("\n");
    }
}

long long sengoo_stdlib_str_ptr(const char* s) {
    return (long long)(intptr_t)s;
}

char* sengoo_str_concat(const char* lhs, const char* rhs) {
    size_t lhs_len = lhs ? strlen(lhs) : 0;
    size_t rhs_len = rhs ? strlen(rhs) : 0;
    if (lhs_len > SIZE_MAX - rhs_len - 1) {
        return NULL;
    }
    char* out = (char*)malloc(lhs_len + rhs_len + 1);
    if (!out) {
        return NULL;
    }
    if (lhs_len > 0) {
        memcpy(out, lhs, lhs_len);
    }
    if (rhs_len > 0) {
        memcpy(out + lhs_len, rhs, rhs_len);
    }
    out[lhs_len + rhs_len] = '\0';
    return out;
}

long long sengoo_str_eq(const char* lhs, const char* rhs) {
    if (!lhs || !rhs) {
        return lhs == rhs ? 1 : 0;
    }
    return strcmp(lhs, rhs) == 0 ? 1 : 0;
}

long long sengoo_f64_eq(double lhs, double rhs) {
    return lhs == rhs ? 1 : 0;
}

long long sengoo_f64_ne(double lhs, double rhs) {
    return lhs != rhs ? 1 : 0;
}

enum {
    SENGOO_FFI_STATUS_OK = 0,
    SENGOO_FFI_ERR_INVALID_ARGUMENT = -2001,
    SENGOO_FFI_ERR_INVALID_HANDLE = -2002,
    SENGOO_FFI_ERR_UNSUPPORTED = -2007,
    SENGOO_FFI_ERR_BUFFER = -2006,
    SENGOO_FFI_ERR_INTERNAL = -2099
};

static int sengoo_ffi_last_error = SENGOO_FFI_STATUS_OK;
static char sengoo_ffi_last_error_message[256] = {0};

static void sengoo_ffi_clear_error_state(void) {
    sengoo_ffi_last_error = SENGOO_FFI_STATUS_OK;
    sengoo_ffi_last_error_message[0] = '\0';
}

static int sengoo_ffi_set_error(int code, const char* message) {
    sengoo_ffi_last_error = code;
    if (message) {
        snprintf(
            sengoo_ffi_last_error_message,
            sizeof(sengoo_ffi_last_error_message),
            "%s",
            message
        );
    } else {
        sengoo_ffi_last_error_message[0] = '\0';
    }
    return code;
}

static const char* sengoo_status_name(long long code) {
    switch (code) {
        case SENGOO_STATUS_OK: return "ok";
        case SENGOO_STATUS_UNKNOWN: return "unknown";
        case SENGOO_STATUS_INVALID_ARGUMENT: return "invalid_argument";
        case SENGOO_STATUS_INVALID_HANDLE: return "invalid_handle";
        case SENGOO_STATUS_BUFFER_TOO_SMALL: return "buffer_too_small";
        case SENGOO_STATUS_NOT_FOUND: return "not_found";
        case SENGOO_STATUS_ALREADY_EXISTS: return "already_exists";
        case SENGOO_STATUS_PERMISSION_DENIED: return "permission_denied";
        case SENGOO_STATUS_UNSUPPORTED: return "unsupported";
        case SENGOO_STATUS_IO: return "io";
        case SENGOO_STATUS_PARSE: return "parse";
        case SENGOO_STATUS_TIMEOUT: return "timeout";
        case SENGOO_STATUS_INTERRUPTED: return "interrupted";
        case SENGOO_STATUS_OVERFLOW: return "overflow";
        case SENGOO_STATUS_OUT_OF_MEMORY: return "out_of_memory";
        case SENGOO_STATUS_TLS_CERT_INVALID: return "tls_cert_invalid";
        case SENGOO_STATUS_TLS_HOSTNAME_MISMATCH: return "tls_hostname_mismatch";
        case SENGOO_STATUS_TLS_HANDSHAKE: return "tls_handshake";
        case SENGOO_STATUS_TLS_UNAVAILABLE: return "tls_unavailable";
        default: return "unknown";
    }
}

static const char* sengoo_status_message(long long code) {
    switch (code) {
        case SENGOO_STATUS_OK: return "success";
        case SENGOO_STATUS_UNKNOWN: return "unknown failure";
        case SENGOO_STATUS_INVALID_ARGUMENT: return "invalid argument";
        case SENGOO_STATUS_INVALID_HANDLE: return "invalid handle";
        case SENGOO_STATUS_BUFFER_TOO_SMALL: return "buffer too small";
        case SENGOO_STATUS_NOT_FOUND: return "not found";
        case SENGOO_STATUS_ALREADY_EXISTS: return "already exists";
        case SENGOO_STATUS_PERMISSION_DENIED: return "permission denied";
        case SENGOO_STATUS_UNSUPPORTED: return "unsupported operation";
        case SENGOO_STATUS_IO: return "I/O failure";
        case SENGOO_STATUS_PARSE: return "parse failure";
        case SENGOO_STATUS_TIMEOUT: return "operation timed out";
        case SENGOO_STATUS_INTERRUPTED: return "operation interrupted";
        case SENGOO_STATUS_OVERFLOW: return "numeric overflow";
        case SENGOO_STATUS_OUT_OF_MEMORY: return "out of memory";
        case SENGOO_STATUS_TLS_CERT_INVALID: return "TLS certificate invalid or untrusted";
        case SENGOO_STATUS_TLS_HOSTNAME_MISMATCH: return "TLS certificate hostname mismatch";
        case SENGOO_STATUS_TLS_HANDSHAKE: return "TLS handshake failed";
        case SENGOO_STATUS_TLS_UNAVAILABLE: return "TLS backend unavailable";
        default: return "unknown failure";
    }
}

long long sengoo_status_from_raw_ffi(long long code) {
    switch (code) {
        case SENGOO_FFI_STATUS_OK: return SENGOO_STATUS_OK;
        case SENGOO_FFI_ERR_INVALID_ARGUMENT: return SENGOO_STATUS_INVALID_ARGUMENT;
        case SENGOO_FFI_ERR_INVALID_HANDLE: return SENGOO_STATUS_INVALID_HANDLE;
        case SENGOO_FFI_ERR_BUFFER: return SENGOO_STATUS_BUFFER_TOO_SMALL;
        case SENGOO_FFI_ERR_UNSUPPORTED: return SENGOO_STATUS_UNSUPPORTED;
        case SENGOO_FFI_ERR_INTERNAL: return SENGOO_STATUS_UNKNOWN;
        case -SENGOO_STATUS_UNKNOWN: return SENGOO_STATUS_UNKNOWN;
        case -SENGOO_STATUS_INVALID_ARGUMENT: return SENGOO_STATUS_INVALID_ARGUMENT;
        case -SENGOO_STATUS_INVALID_HANDLE: return SENGOO_STATUS_INVALID_HANDLE;
        case -SENGOO_STATUS_BUFFER_TOO_SMALL: return SENGOO_STATUS_BUFFER_TOO_SMALL;
        case -SENGOO_STATUS_NOT_FOUND: return SENGOO_STATUS_NOT_FOUND;
        case -SENGOO_STATUS_ALREADY_EXISTS: return SENGOO_STATUS_ALREADY_EXISTS;
        case -SENGOO_STATUS_PERMISSION_DENIED: return SENGOO_STATUS_PERMISSION_DENIED;
        case -SENGOO_STATUS_UNSUPPORTED: return SENGOO_STATUS_UNSUPPORTED;
        case -SENGOO_STATUS_IO: return SENGOO_STATUS_IO;
        case -SENGOO_STATUS_PARSE: return SENGOO_STATUS_PARSE;
        case -SENGOO_STATUS_TIMEOUT: return SENGOO_STATUS_TIMEOUT;
        case -SENGOO_STATUS_INTERRUPTED: return SENGOO_STATUS_INTERRUPTED;
        case -SENGOO_STATUS_OVERFLOW: return SENGOO_STATUS_OVERFLOW;
        case -SENGOO_STATUS_OUT_OF_MEMORY: return SENGOO_STATUS_OUT_OF_MEMORY;
        case -SENGOO_STATUS_TLS_CERT_INVALID: return SENGOO_STATUS_TLS_CERT_INVALID;
        case -SENGOO_STATUS_TLS_HOSTNAME_MISMATCH: return SENGOO_STATUS_TLS_HOSTNAME_MISMATCH;
        case -SENGOO_STATUS_TLS_HANDSHAKE: return SENGOO_STATUS_TLS_HANDSHAKE;
        case -SENGOO_STATUS_TLS_UNAVAILABLE: return SENGOO_STATUS_TLS_UNAVAILABLE;
        case SENGOO_STATUS_UNKNOWN: return SENGOO_STATUS_UNKNOWN;
        case SENGOO_STATUS_INVALID_ARGUMENT: return SENGOO_STATUS_INVALID_ARGUMENT;
        case SENGOO_STATUS_INVALID_HANDLE: return SENGOO_STATUS_INVALID_HANDLE;
        case SENGOO_STATUS_BUFFER_TOO_SMALL: return SENGOO_STATUS_BUFFER_TOO_SMALL;
        case SENGOO_STATUS_NOT_FOUND: return SENGOO_STATUS_NOT_FOUND;
        case SENGOO_STATUS_ALREADY_EXISTS: return SENGOO_STATUS_ALREADY_EXISTS;
        case SENGOO_STATUS_PERMISSION_DENIED: return SENGOO_STATUS_PERMISSION_DENIED;
        case SENGOO_STATUS_UNSUPPORTED: return SENGOO_STATUS_UNSUPPORTED;
        case SENGOO_STATUS_IO: return SENGOO_STATUS_IO;
        case SENGOO_STATUS_PARSE: return SENGOO_STATUS_PARSE;
        case SENGOO_STATUS_TIMEOUT: return SENGOO_STATUS_TIMEOUT;
        case SENGOO_STATUS_INTERRUPTED: return SENGOO_STATUS_INTERRUPTED;
        case SENGOO_STATUS_OVERFLOW: return SENGOO_STATUS_OVERFLOW;
        case SENGOO_STATUS_OUT_OF_MEMORY: return SENGOO_STATUS_OUT_OF_MEMORY;
        case SENGOO_STATUS_TLS_CERT_INVALID: return SENGOO_STATUS_TLS_CERT_INVALID;
        case SENGOO_STATUS_TLS_HOSTNAME_MISMATCH: return SENGOO_STATUS_TLS_HOSTNAME_MISMATCH;
        case SENGOO_STATUS_TLS_HANDSHAKE: return SENGOO_STATUS_TLS_HANDSHAKE;
        case SENGOO_STATUS_TLS_UNAVAILABLE: return SENGOO_STATUS_TLS_UNAVAILABLE;
        default: return SENGOO_STATUS_UNKNOWN;
    }
}

long long sengoo_status_from_net_error(long long code) {
    switch (code) {
        case 0: return SENGOO_STATUS_OK;
        case 1: return SENGOO_STATUS_INVALID_ARGUMENT;
        case 2: return SENGOO_STATUS_INVALID_ARGUMENT;
        case 3: return SENGOO_STATUS_UNSUPPORTED;
        case 4: return SENGOO_STATUS_IO;
        case 5: return SENGOO_STATUS_IO;
        case 6: return SENGOO_STATUS_IO;
        case 7: return SENGOO_STATUS_TIMEOUT;
        case 8: return SENGOO_STATUS_PARSE;
        case 9: return SENGOO_STATUS_PARSE;
        case 10: return SENGOO_STATUS_IO;
        case 11: return SENGOO_STATUS_PARSE;
        case 12: return SENGOO_STATUS_INVALID_HANDLE;
        case 13: return SENGOO_STATUS_UNKNOWN;
        case 14: return SENGOO_STATUS_IO;
        case 15: return SENGOO_STATUS_TLS_CERT_INVALID;
        case 16: return SENGOO_STATUS_TLS_HOSTNAME_MISMATCH;
        case 17: return SENGOO_STATUS_TLS_HANDSHAKE;
        case 18: return SENGOO_STATUS_TLS_UNAVAILABLE;
        default: return SENGOO_STATUS_UNKNOWN;
    }
}

long long sengoo_status_from_net_bench_error(long long code) {
    switch (code) {
        case 0: return SENGOO_STATUS_OK;
        case -2601: return SENGOO_STATUS_INVALID_ARGUMENT;
        case -2602: return SENGOO_STATUS_IO;
        case -2699: return SENGOO_STATUS_UNKNOWN;
        default: return sengoo_status_from_raw_ffi(code);
    }
}

static long long sengoo_copy_status_text(const char* text, long long out_buffer, long long out_capacity) {
    char* out = (char*)(intptr_t)out_buffer;
    if (out_capacity < 0) {
        return SENGOO_FFI_ERR_INVALID_ARGUMENT;
    }
    size_t len = strlen(text);
    if ((unsigned long long)len > (unsigned long long)out_capacity || (len > 0 && !out)) {
        return SENGOO_FFI_ERR_BUFFER;
    }
    if (len > 0) {
        memcpy(out, text, len);
    }
    return (long long)len;
}

long long sengoo_status_name_copy(long long code, long long out_buffer, long long out_capacity) {
    return sengoo_copy_status_text(sengoo_status_name(code), out_buffer, out_capacity);
}

long long sengoo_status_message_copy(long long code, long long out_buffer, long long out_capacity) {
    return sengoo_copy_status_text(sengoo_status_message(code), out_buffer, out_capacity);
}

static long long sengoo_negative_status_from_errno(int err, long long fallback_status) {
    switch (err) {
        case 0:
            return -fallback_status;
#ifdef ENOENT
        case ENOENT:
            return -SENGOO_STATUS_NOT_FOUND;
#endif
#ifdef ENOTDIR
        case ENOTDIR:
            return -SENGOO_STATUS_NOT_FOUND;
#endif
#ifdef EACCES
        case EACCES:
            return -SENGOO_STATUS_PERMISSION_DENIED;
#endif
#ifdef EPERM
        case EPERM:
            return -SENGOO_STATUS_PERMISSION_DENIED;
#endif
#ifdef EEXIST
        case EEXIST:
            return -SENGOO_STATUS_ALREADY_EXISTS;
#endif
#ifdef ENOMEM
        case ENOMEM:
            return -SENGOO_STATUS_OUT_OF_MEMORY;
#endif
        default:
            return -fallback_status;
    }
}

long long sengoo_ffi_last_error_code(void) {
    return (long long)sengoo_ffi_last_error;
}

long long sengoo_ffi_last_error_len(void) {
    return (long long)strlen(sengoo_ffi_last_error_message);
}

long long sengoo_ffi_last_error_copy(long long out_buffer, long long out_capacity) {
    char* out = (char*)(intptr_t)out_buffer;
    if (out_capacity < 0) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "negative output capacity");
    }

    size_t len = strlen(sengoo_ffi_last_error_message);
    if ((unsigned long long)len > (unsigned long long)out_capacity || (len > 0 && !out)) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_BUFFER, "output capacity too small");
    }

    if (len > 0) {
        memcpy(out, sengoo_ffi_last_error_message, len);
    }
    return (long long)len;
}

long long sengoo_ffi_last_error_clear(void) {
    sengoo_ffi_clear_error_state();
    return SENGOO_FFI_STATUS_OK;
}

static long long sengoo_ffi_set_unsupported(const char* feature) {
    char message[160];
    snprintf(
        message,
        sizeof(message),
        "%s is not supported by the C stdlib runtime bundle",
        feature ? feature : "ffi bridge"
    );
    return sengoo_ffi_set_error(SENGOO_FFI_ERR_UNSUPPORTED, message);
}

long long sengoo_ffi_c_open(long long path) {
    (void)path;
    sengoo_ffi_set_unsupported("dynamic library loading");
    return 0;
}

long long sengoo_ffi_c_close(long long handle) {
    (void)handle;
    return sengoo_ffi_set_unsupported("dynamic library close");
}

long long sengoo_ffi_c_call_i64(
    long long handle,
    long long symbol,
    long long argc,
    long long argv,
    long long out_value
) {
    (void)handle;
    (void)symbol;
    (void)argc;
    (void)argv;
    (void)out_value;
    return sengoo_ffi_set_unsupported("dynamic function call");
}

long long sengoo_ffi_c_call_i64_value(
    long long handle,
    long long symbol,
    long long argc,
    long long a0,
    long long a1,
    long long a2,
    long long a3
) {
    (void)handle;
    (void)symbol;
    (void)argc;
    (void)a0;
    (void)a1;
    (void)a2;
    (void)a3;
    sengoo_ffi_set_unsupported("dynamic function call");
    return 0;
}

long long sengoo_ffi_object_create(
    long long lib_handle,
    long long constructor_symbol,
    long long argc,
    long long argv,
    long long destructor_symbol
) {
    (void)lib_handle;
    (void)constructor_symbol;
    (void)argc;
    (void)argv;
    (void)destructor_symbol;
    sengoo_ffi_set_unsupported("ffi object creation");
    return 0;
}

long long sengoo_ffi_object_create_value(
    long long lib_handle,
    long long constructor_symbol,
    long long argc,
    long long a0,
    long long a1,
    long long a2,
    long long a3,
    long long destructor_symbol
) {
    (void)lib_handle;
    (void)constructor_symbol;
    (void)argc;
    (void)a0;
    (void)a1;
    (void)a2;
    (void)a3;
    (void)destructor_symbol;
    sengoo_ffi_set_unsupported("ffi object creation");
    return 0;
}

long long sengoo_ffi_object_raw_ptr(long long object_handle) {
    (void)object_handle;
    sengoo_ffi_set_unsupported("ffi object raw pointer access");
    return 0;
}

long long sengoo_ffi_object_call_i64(
    long long object_handle,
    long long method_symbol,
    long long argc,
    long long argv,
    long long out_value
) {
    (void)object_handle;
    (void)method_symbol;
    (void)argc;
    (void)argv;
    (void)out_value;
    return sengoo_ffi_set_unsupported("ffi object method call");
}

long long sengoo_ffi_object_call_i64_value(
    long long object_handle,
    long long method_symbol,
    long long argc,
    long long a0,
    long long a1,
    long long a2
) {
    (void)object_handle;
    (void)method_symbol;
    (void)argc;
    (void)a0;
    (void)a1;
    (void)a2;
    sengoo_ffi_set_unsupported("ffi object method call");
    return 0;
}

long long sengoo_ffi_object_destroy(long long object_handle) {
    (void)object_handle;
    return sengoo_ffi_set_unsupported("ffi object destruction");
}

long long sengoo_ffi_callback_bind_i64(long long lib_handle, long long symbol, long long arity) {
    (void)lib_handle;
    (void)symbol;
    (void)arity;
    sengoo_ffi_set_unsupported("ffi callback binding");
    return 0;
}

long long sengoo_ffi_callback_dispatch_i64(
    long long callback_id,
    long long a0,
    long long a1,
    long long a2,
    long long a3,
    long long a4,
    long long a5
) {
    (void)callback_id;
    (void)a0;
    (void)a1;
    (void)a2;
    (void)a3;
    (void)a4;
    (void)a5;
    sengoo_ffi_set_unsupported("ffi callback dispatch");
    return 0;
}

long long sengoo_ffi_callback_unbind(long long callback_id) {
    (void)callback_id;
    return sengoo_ffi_set_unsupported("ffi callback unbind");
}

typedef struct {
    SengooFfiBuffer* buffer;
    uint32_t generation;
    unsigned char alive;
} SengooBufferSlot;

static SengooBufferSlot* g_buffer_slots = NULL;
static size_t g_buffer_slot_count = 0;
static size_t g_buffer_slot_capacity = 0;

static int sengoo_buffer_slot_ensure_capacity(size_t min_slots) {
    if (g_buffer_slot_capacity >= min_slots) {
        return 1;
    }
    size_t new_cap = g_buffer_slot_capacity == 0 ? 8 : g_buffer_slot_capacity;
    while (new_cap < min_slots) {
        if (new_cap > (SIZE_MAX / 2)) {
            return 0;
        }
        new_cap *= 2;
    }
    SengooBufferSlot* next =
        (SengooBufferSlot*)realloc(g_buffer_slots, new_cap * sizeof(SengooBufferSlot));
    if (!next) {
        return 0;
    }
    if (new_cap > g_buffer_slot_capacity) {
        memset(
            next + g_buffer_slot_capacity,
            0,
            (new_cap - g_buffer_slot_capacity) * sizeof(SengooBufferSlot));
    }
    g_buffer_slots = next;
    g_buffer_slot_capacity = new_cap;
    return 1;
}

static long long sengoo_buffer_alloc_handle(SengooFfiBuffer* buffer) {
    size_t index = 0;
    for (; index < g_buffer_slot_count; ++index) {
        if (!g_buffer_slots[index].alive) {
            break;
        }
    }
    if (index == g_buffer_slot_count) {
        if (!sengoo_buffer_slot_ensure_capacity(g_buffer_slot_count + 1)) {
            return 0;
        }
        g_buffer_slot_count += 1;
    }

    SengooBufferSlot* slot = &g_buffer_slots[index];
    slot->buffer = buffer;
    slot->alive = 1;
    slot->generation += 1;
    if (slot->generation == 0) {
        slot->generation = 1;
    }
    return ((long long)slot->generation << 32) | (long long)(index + 1);
}

static int sengoo_buffer_decode_handle(long long handle, size_t* out_index, uint32_t* out_generation) {
    if (handle <= 0) {
        return 0;
    }
    size_t index = ((size_t)handle & 0xFFFFFFFFu) - 1;
    uint32_t generation = (uint32_t)((unsigned long long)handle >> 32);
    if (index >= g_buffer_slot_count) {
        return 0;
    }
    *out_index = index;
    *out_generation = generation;
    return 1;
}

static SengooFfiBuffer* sengoo_buffer_resolve(long long handle) {
    size_t index = 0;
    uint32_t generation = 0;
    if (!sengoo_buffer_decode_handle(handle, &index, &generation)) {
        return NULL;
    }
    SengooBufferSlot* slot = &g_buffer_slots[index];
    if (!slot->alive || slot->generation != generation || !slot->buffer) {
        return NULL;
    }
    return slot->buffer;
}

SengooFfiBuffer* sengoo_ffi_buffer_from_handle(long long handle) {
    return sengoo_buffer_resolve(handle);
}

long long sengoo_copy_bytes_to_managed_buffer(long long buffer_handle, const char* bytes, size_t len) {
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (len > 0 && !bytes) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (len > buffer->capacity || (len > 0 && !buffer->bytes)) {
        return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    }
    if (len > 0) {
        memcpy(buffer->bytes, bytes, len);
    }
    buffer->used_len = len;
    return (long long)len;
}

long long sengoo_ffi_buffer_new(long long capacity) {
    sengoo_ffi_clear_error_state();
    if (capacity < 0) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "negative buffer capacity");
    }
    if ((unsigned long long)capacity > (unsigned long long)SENGOO_RUNTIME_MAX_BUFFER_BYTES) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "buffer capacity exceeds runtime limit");
    }

    SengooFfiBuffer* buffer = (SengooFfiBuffer*)calloc(1, sizeof(SengooFfiBuffer));
    if (!buffer) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INTERNAL, "buffer allocation failed");
    }

    if (capacity > 0) {
        buffer->bytes = (unsigned char*)calloc((size_t)capacity, 1);
        if (!buffer->bytes) {
            free(buffer);
            return sengoo_ffi_set_error(SENGOO_FFI_ERR_INTERNAL, "buffer bytes allocation failed");
        }
    }
    buffer->capacity = (size_t)capacity;
    buffer->used_len = 0;
    long long handle = sengoo_buffer_alloc_handle(buffer);
    if (handle == 0) {
        free(buffer->bytes);
        free(buffer);
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INTERNAL, "buffer handle allocation failed");
    }
    return handle;
}

long long sengoo_ffi_buffer_from_bytes(long long data_ptr, long long len) {
    sengoo_ffi_clear_error_state();
    const unsigned char* data = (const unsigned char*)(intptr_t)data_ptr;
    if (len < 0 || (len > 0 && !data)) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "invalid source bytes");
    }

    long long handle = sengoo_ffi_buffer_new(len);
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(handle);
    if (!buffer) {
        return 0;
    }
    if (len > 0) {
        memcpy(buffer->bytes, data, (size_t)len);
    }
    buffer->used_len = (size_t)len;
    return handle;
}

long long sengoo_ffi_buffer_len(long long buffer_handle) {
    sengoo_ffi_clear_error_state();
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_HANDLE, "buffer handle not found");
    }
    return (long long)buffer->capacity;
}

long long sengoo_ffi_buffer_capacity(long long buffer_handle) {
    return sengoo_ffi_buffer_len(buffer_handle);
}

long long sengoo_ffi_buffer_used_len(long long buffer_handle) {
    sengoo_ffi_clear_error_state();
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_HANDLE, "buffer handle not found");
    }
    return (long long)buffer->used_len;
}

long long sengoo_ffi_buffer_ptr(long long buffer_handle) {
    sengoo_ffi_clear_error_state();
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_HANDLE, "buffer handle not found");
    }
    return (long long)(intptr_t)buffer->bytes;
}

long long sengoo_ffi_buffer_copy_out(long long buffer_handle, long long out_buffer, long long out_capacity) {
    sengoo_ffi_clear_error_state();
    char* out = (char*)(intptr_t)out_buffer;
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_HANDLE, "buffer handle not found");
    }
    if (out_capacity < 0 || (unsigned long long)buffer->used_len > (unsigned long long)out_capacity || (buffer->used_len > 0 && !out)) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_BUFFER, "output capacity too small");
    }
    if (buffer->used_len > 0) {
        memcpy(out, buffer->bytes, buffer->used_len);
    }
    return (long long)buffer->used_len;
}

long long sengoo_ffi_buffer_copy_in(long long buffer_handle, long long src_ptr, long long src_len) {
    sengoo_ffi_clear_error_state();
    const unsigned char* src = (const unsigned char*)(intptr_t)src_ptr;
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_HANDLE, "buffer handle not found");
    }
    if (src_len < 0 || (src_len > 0 && !src)) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "invalid source bytes");
    }

    if ((unsigned long long)src_len > (unsigned long long)buffer->capacity) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_BUFFER, "buffer capacity too small");
    }

    if (src_len > 0) {
        memcpy(buffer->bytes, src, (size_t)src_len);
    }
    buffer->used_len = (size_t)src_len;
    return SENGOO_FFI_STATUS_OK;
}

long long sengoo_ffi_buffer_clear(long long buffer_handle) {
    sengoo_ffi_clear_error_state();
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_HANDLE, "buffer handle not found");
    }
    buffer->used_len = 0;
    return SENGOO_FFI_STATUS_OK;
}

long long sengoo_ffi_buffer_copy_range(long long buffer_handle, long long start, long long len, long long out_buffer_handle) {
    sengoo_ffi_clear_error_state();
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    SengooFfiBuffer* out = sengoo_ffi_buffer_from_handle(out_buffer_handle);
    if (!buffer || !out) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_HANDLE, "buffer handle not found");
    }
    if (start < 0 || len < 0) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "negative range");
    }
    size_t range_start = (size_t)start;
    size_t range_len = (size_t)len;
    if (range_start > buffer->used_len || range_len > buffer->used_len - range_start) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "range outside buffer used length");
    }
    if (range_len > out->capacity) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_BUFFER, "output capacity too small");
    }
    if (range_len > 0) {
        memcpy(out->bytes, buffer->bytes + range_start, range_len);
    }
    out->used_len = range_len;
    return (long long)range_len;
}

long long sengoo_ffi_buffer_append(long long buffer_handle, long long src_ptr, long long src_len) {
    sengoo_ffi_clear_error_state();
    const unsigned char* src = (const unsigned char*)(intptr_t)src_ptr;
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_HANDLE, "buffer handle not found");
    }
    if (src_len < 0 || (src_len > 0 && !src)) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_ARGUMENT, "invalid source bytes");
    }
    size_t append_len = (size_t)src_len;
    if (append_len > buffer->capacity - buffer->used_len) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_BUFFER, "buffer capacity too small");
    }
    if (append_len > 0) {
        memcpy(buffer->bytes + buffer->used_len, src, append_len);
    }
    buffer->used_len += append_len;
    return (long long)append_len;
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
            if (i + 3 >= len || (bytes[i + 1] & 0xC0) != 0x80 || (bytes[i + 2] & 0xC0) != 0x80 || (bytes[i + 3] & 0xC0) != 0x80) {
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

long long sengoo_ffi_buffer_is_utf8(long long buffer_handle) {
    sengoo_ffi_clear_error_state();
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_HANDLE, "buffer handle not found");
    }
    return sengoo_bytes_are_utf8(buffer->bytes, buffer->used_len) ? 1 : 0;
}

long long sengoo_ffi_buffer_free(long long buffer_handle) {
    sengoo_ffi_clear_error_state();
    size_t index = 0;
    uint32_t generation = 0;
    if (!sengoo_buffer_decode_handle(buffer_handle, &index, &generation)) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_HANDLE, "buffer handle not found");
    }
    SengooBufferSlot* slot = &g_buffer_slots[index];
    if (!slot->alive || slot->generation != generation || !slot->buffer) {
        return sengoo_ffi_set_error(SENGOO_FFI_ERR_INVALID_HANDLE, "buffer handle not found");
    }
    SengooFfiBuffer* buffer = slot->buffer;
    slot->alive = 0;
    slot->buffer = NULL;
    free(buffer->bytes);
    free(buffer);
    return SENGOO_FFI_STATUS_OK;
}

long long sengoo_str_len(const char* value) {
    return value ? (long long)strlen(value) : 0;
}

long long sengoo_str_contains(const char* value, const char* needle) {
    if (!value || !needle) {
        return 0;
    }
    return strstr(value, needle) != NULL ? 1 : 0;
}

long long sengoo_str_starts_with(const char* value, const char* prefix) {
    if (!value || !prefix) {
        return 0;
    }
    size_t prefix_len = strlen(prefix);
    return strncmp(value, prefix, prefix_len) == 0 ? 1 : 0;
}

long long sengoo_str_ends_with(const char* value, const char* suffix) {
    if (!value || !suffix) {
        return 0;
    }
    size_t value_len = strlen(value);
    size_t suffix_len = strlen(suffix);
    if (suffix_len > value_len) {
        return 0;
    }
    return memcmp(value + value_len - suffix_len, suffix, suffix_len) == 0 ? 1 : 0;
}

long long sengoo_str_index_of(const char* value, const char* needle) {
    if (!value || !needle) {
        return -1;
    }
    const char* found = strstr(value, needle);
    if (!found) {
        return -1;
    }
    return (long long)(found - value);
}

enum {
    SENGOO_STRCONV_STATUS_OK = 0,
    SENGOO_STRCONV_ERR_INVALID = 1,
    SENGOO_STRCONV_ERR_OVERFLOW = 2,
    SENGOO_STRCONV_ERR_BUFFER = 3,
    SENGOO_STRCONV_ERR_INTERNAL = 4
};

static int sengoo_strconv_last_error = SENGOO_STRCONV_STATUS_OK;

static long long sengoo_strconv_set_error(int code) {
    sengoo_strconv_last_error = code;
    return 0;
}

long long sengoo_strconv_last_error_code(void) {
    return (long long)sengoo_strconv_last_error;
}

long long sengoo_strconv_parse_i64(long long data_ptr, long long len) {
    sengoo_strconv_last_error = SENGOO_STRCONV_STATUS_OK;
    const unsigned char* data = (const unsigned char*)(intptr_t)data_ptr;
    if (len < 0 || (len > 0 && !data)) {
        return sengoo_strconv_set_error(SENGOO_STRCONV_ERR_INVALID);
    }

    size_t n = (size_t)len;
    size_t i = 0;
    while (i < n && isspace((unsigned char)data[i])) {
        i++;
    }

    int negative = 0;
    if (i < n && (data[i] == '+' || data[i] == '-')) {
        negative = data[i] == '-';
        i++;
    }

    if (i >= n || !isdigit((unsigned char)data[i])) {
        return sengoo_strconv_set_error(SENGOO_STRCONV_ERR_INVALID);
    }

    unsigned long long limit = negative
        ? (unsigned long long)LLONG_MAX + 1ULL
        : (unsigned long long)LLONG_MAX;
    unsigned long long acc = 0;
    while (i < n && isdigit((unsigned char)data[i])) {
        unsigned long long digit = (unsigned long long)(data[i] - '0');
        if (acc > (limit - digit) / 10ULL) {
            return sengoo_strconv_set_error(SENGOO_STRCONV_ERR_OVERFLOW);
        }
        acc = acc * 10ULL + digit;
        i++;
    }

    while (i < n && isspace((unsigned char)data[i])) {
        i++;
    }
    if (i != n) {
        return sengoo_strconv_set_error(SENGOO_STRCONV_ERR_INVALID);
    }

    if (negative) {
        if (acc == (unsigned long long)LLONG_MAX + 1ULL) {
            return LLONG_MIN;
        }
        return -(long long)acc;
    }
    return (long long)acc;
}

long long sengoo_strconv_format_i64(long long value, long long buffer_ptr, long long capacity) {
    sengoo_strconv_last_error = SENGOO_STRCONV_STATUS_OK;
    char* out = (char*)(intptr_t)buffer_ptr;
    if (capacity < 0) {
        return sengoo_strconv_set_error(SENGOO_STRCONV_ERR_BUFFER) - 1;
    }

    char temp[32];
    int written = snprintf(temp, sizeof(temp), "%lld", value);
    if (written < 0 || (size_t)written >= sizeof(temp)) {
        return sengoo_strconv_set_error(SENGOO_STRCONV_ERR_INTERNAL) - 1;
    }
    if ((unsigned long long)written > (unsigned long long)capacity || (written > 0 && !out)) {
        return sengoo_strconv_set_error(SENGOO_STRCONV_ERR_BUFFER) - 1;
    }

    if (written > 0) {
        memcpy(out, temp, (size_t)written);
    }
    return (long long)written;
}

long long sengoo_file_exists(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path || path[0] == '\0') {
        return 0;
    }

    FILE* file = fopen(path, "rb");
    if (!file) {
        return 0;
    }
    fclose(file);
    return 1;
}

long long sengoo_file_len(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path || path[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    FILE* file = fopen(path, "rb");
    if (!file) {
        return sengoo_negative_status_from_errno(errno, SENGOO_STATUS_IO);
    }
    if (fseek(file, 0, SEEK_END) != 0) {
        int err = errno;
        fclose(file);
        return sengoo_negative_status_from_errno(err, SENGOO_STATUS_IO);
    }
    long size = ftell(file);
    fclose(file);
    if (size < 0) {
        return sengoo_negative_status_from_errno(errno, SENGOO_STATUS_IO);
    }
    return (long long)size;
}

long long sengoo_file_read_into(long long path_ptr, long long out_buffer, long long out_capacity) {
    const char* path = (const char*)(intptr_t)path_ptr;
    char* out = (char*)(intptr_t)out_buffer;
    if (!path || path[0] == '\0' || !out || out_capacity < 0) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    FILE* file = fopen(path, "rb");
    if (!file) {
        return sengoo_negative_status_from_errno(errno, SENGOO_STATUS_IO);
    }
    size_t read = fread(out, 1, (size_t)out_capacity, file);
    int failed = ferror(file);
    int err = errno;
    fclose(file);
    if (failed) {
        return sengoo_negative_status_from_errno(err, SENGOO_STATUS_IO);
    }
    return (long long)read;
}

static long long sengoo_file_write_mode(
    long long path_ptr,
    long long data_ptr,
    long long len,
    const char* mode
) {
    const char* path = (const char*)(intptr_t)path_ptr;
    const char* data = (const char*)(intptr_t)data_ptr;
    if (!path || path[0] == '\0' || len < 0 || (!data && len > 0)) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    FILE* file = fopen(path, mode);
    if (!file) {
        return sengoo_negative_status_from_errno(errno, SENGOO_STATUS_IO);
    }
    size_t expected = (size_t)len;
    size_t wrote = expected == 0 ? 0 : fwrite(data, 1, expected, file);
    int write_failed = ferror(file);
    int write_errno = errno;
    int close_status = fclose(file);
    if (wrote != expected || write_failed || close_status != 0) {
        return sengoo_negative_status_from_errno(write_errno, SENGOO_STATUS_IO);
    }
    return (long long)wrote;
}

long long sengoo_file_write_str(long long path_ptr, long long data_ptr, long long len) {
    return sengoo_file_write_mode(path_ptr, data_ptr, len, "wb");
}

long long sengoo_file_append_str(long long path_ptr, long long data_ptr, long long len) {
    return sengoo_file_write_mode(path_ptr, data_ptr, len, "ab");
}

long long sengoo_file_remove(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path || path[0] == '\0') {
        return 1;
    }

    return remove(path);
}

static int sengoo_path_entry_exists_cstr(const char* path) {
    if (!path || path[0] == '\0') {
        return 0;
    }

#ifdef _WIN32
    return GetFileAttributesA(path) != INVALID_FILE_ATTRIBUTES ? 1 : 0;
#else
    struct stat info;
    return lstat(path, &info) == 0 ? 1 : 0;
#endif
}

static int sengoo_path_is_regular_file_cstr(const char* path) {
    if (!path || path[0] == '\0') {
        return 0;
    }

#ifdef _WIN32
    DWORD attributes = GetFileAttributesA(path);
    return attributes != INVALID_FILE_ATTRIBUTES && !(attributes & FILE_ATTRIBUTE_DIRECTORY) ? 1 : 0;
#else
    struct stat info;
    return stat(path, &info) == 0 && S_ISREG(info.st_mode) ? 1 : 0;
#endif
}

static int sengoo_paths_refer_to_same_file_cstr(const char* left, const char* right) {
#ifdef _WIN32
    HANDLE left_handle = CreateFileA(
        left,
        0,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        NULL,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        NULL
    );
    if (left_handle == INVALID_HANDLE_VALUE) {
        return 0;
    }

    HANDLE right_handle = CreateFileA(
        right,
        0,
        FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        NULL,
        OPEN_EXISTING,
        FILE_ATTRIBUTE_NORMAL,
        NULL
    );
    if (right_handle == INVALID_HANDLE_VALUE) {
        CloseHandle(left_handle);
        return 0;
    }

    BY_HANDLE_FILE_INFORMATION left_info;
    BY_HANDLE_FILE_INFORMATION right_info;
    int same = GetFileInformationByHandle(left_handle, &left_info)
        && GetFileInformationByHandle(right_handle, &right_info)
        && left_info.dwVolumeSerialNumber == right_info.dwVolumeSerialNumber
        && left_info.nFileIndexHigh == right_info.nFileIndexHigh
        && left_info.nFileIndexLow == right_info.nFileIndexLow;
    CloseHandle(right_handle);
    CloseHandle(left_handle);
    return same ? 1 : 0;
#else
    struct stat left_info;
    struct stat right_info;
    return stat(left, &left_info) == 0
        && stat(right, &right_info) == 0
        && left_info.st_dev == right_info.st_dev
        && left_info.st_ino == right_info.st_ino ? 1 : 0;
#endif
}

long long sengoo_file_copy(long long source_ptr, long long destination_ptr, long long overwrite) {
    const char* source = (const char*)(intptr_t)source_ptr;
    const char* destination = (const char*)(intptr_t)destination_ptr;
    if (!sengoo_path_is_regular_file_cstr(source) || !destination || destination[0] == '\0') {
        return source && source[0] != '\0'
            ? -SENGOO_STATUS_NOT_FOUND
            : -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    int destination_existed = sengoo_path_entry_exists_cstr(destination);
    if (!overwrite && destination_existed) {
        return -SENGOO_STATUS_ALREADY_EXISTS;
    }
    if (destination_existed && sengoo_paths_refer_to_same_file_cstr(source, destination)) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    FILE* input = fopen(source, "rb");
    if (!input) {
        return sengoo_negative_status_from_errno(errno, SENGOO_STATUS_IO);
    }

    FILE* output = fopen(destination, "wb");
    if (!output) {
        int err = errno;
        fclose(input);
        return sengoo_negative_status_from_errno(err, SENGOO_STATUS_IO);
    }

    unsigned char buffer[8192];
    unsigned long long total = 0;
    int failed = 0;
    for (;;) {
        size_t read = fread(buffer, 1, sizeof(buffer), input);
        if (read > 0) {
            if ((unsigned long long)read > (unsigned long long)LLONG_MAX - total
                || fwrite(buffer, 1, read, output) != read) {
                failed = 1;
                break;
            }
            total += (unsigned long long)read;
        }
        if (read < sizeof(buffer)) {
            if (ferror(input)) {
                failed = 1;
            }
            break;
        }
    }

    if (fclose(output) != 0) {
        failed = 1;
    }
    if (fclose(input) != 0) {
        failed = 1;
    }
    if (failed) {
        if (!destination_existed) {
            remove(destination);
        }
        return -SENGOO_STATUS_IO;
    }
    return (long long)total;
}

long long sengoo_file_move(long long source_ptr, long long destination_ptr, long long overwrite) {
    const char* source = (const char*)(intptr_t)source_ptr;
    const char* destination = (const char*)(intptr_t)destination_ptr;
    if (!sengoo_path_is_regular_file_cstr(source) || !destination || destination[0] == '\0') {
        return source && source[0] != '\0'
            ? -SENGOO_STATUS_NOT_FOUND
            : -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (!overwrite && sengoo_path_entry_exists_cstr(destination)) {
        return -SENGOO_STATUS_ALREADY_EXISTS;
    }

#ifdef _WIN32
    return MoveFileExA(source, destination, overwrite ? MOVEFILE_REPLACE_EXISTING : 0)
        ? 0
        : -SENGOO_STATUS_IO;
#else
    return rename(source, destination) == 0
        ? 0
        : sengoo_negative_status_from_errno(errno, SENGOO_STATUS_IO);
#endif
}

enum {
    SENGOO_PATH_KIND_FILE = 1,
    SENGOO_PATH_KIND_DIR = 2,
    SENGOO_PATH_KIND_SYMLINK = 3
};

long long sengoo_file_kind(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path || path[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

#ifdef _WIN32
    DWORD attributes = GetFileAttributesA(path);
    if (attributes == INVALID_FILE_ATTRIBUTES) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    if (attributes & FILE_ATTRIBUTE_REPARSE_POINT) {
        return SENGOO_PATH_KIND_SYMLINK;
    }
    if (attributes & FILE_ATTRIBUTE_DIRECTORY) {
        return SENGOO_PATH_KIND_DIR;
    }
    return SENGOO_PATH_KIND_FILE;
#else
    struct stat info;
    if (lstat(path, &info) != 0) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    if (S_ISREG(info.st_mode)) {
        return SENGOO_PATH_KIND_FILE;
    }
    if (S_ISDIR(info.st_mode)) {
        return SENGOO_PATH_KIND_DIR;
    }
#ifdef S_ISLNK
    if (S_ISLNK(info.st_mode)) {
        return SENGOO_PATH_KIND_SYMLINK;
    }
#endif
    return -SENGOO_STATUS_UNSUPPORTED;
#endif
}

long long sengoo_file_size(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path || path[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

#ifdef _WIN32
    WIN32_FILE_ATTRIBUTE_DATA info;
    if (!GetFileAttributesExA(path, GetFileExInfoStandard, &info)) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    if (info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) {
        return -SENGOO_STATUS_UNSUPPORTED;
    }
    unsigned long long size = ((unsigned long long)info.nFileSizeHigh << 32) | info.nFileSizeLow;
    if (size > (unsigned long long)LLONG_MAX) {
        return -SENGOO_STATUS_OVERFLOW;
    }
    return (long long)size;
#else
    struct stat info;
    if (stat(path, &info) != 0) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    if (!S_ISREG(info.st_mode)) {
        return -SENGOO_STATUS_UNSUPPORTED;
    }
    if (info.st_size < 0) {
        return -SENGOO_STATUS_OVERFLOW;
    }
    return (long long)info.st_size;
#endif
}

long long sengoo_file_modified_unix_ms(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path || path[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

#ifdef _WIN32
    WIN32_FILE_ATTRIBUTE_DATA info;
    if (!GetFileAttributesExA(path, GetFileExInfoStandard, &info)) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    ULARGE_INTEGER file_time;
    file_time.LowPart = info.ftLastWriteTime.dwLowDateTime;
    file_time.HighPart = info.ftLastWriteTime.dwHighDateTime;
    unsigned long long windows_ms = file_time.QuadPart / 10000ULL;
    const unsigned long long unix_epoch_ms = 11644473600000ULL;
    if (windows_ms < unix_epoch_ms) {
        return -SENGOO_STATUS_UNSUPPORTED;
    }
    unsigned long long unix_ms = windows_ms - unix_epoch_ms;
    if (unix_ms > (unsigned long long)LLONG_MAX) {
        return -SENGOO_STATUS_OVERFLOW;
    }
    return (long long)unix_ms;
#else
    struct stat info;
    if (stat(path, &info) != 0) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    if (info.st_mtime < 0) {
        return -SENGOO_STATUS_UNSUPPORTED;
    }
    return ((long long)info.st_mtime) * 1000LL;
#endif
}

long long sengoo_env_var_len(long long name_ptr) {
    const char* name = (const char*)(intptr_t)name_ptr;
    if (!name || name[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    const char* value = getenv(name);
    if (!value) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    return (long long)strlen(value);
}

long long sengoo_env_var_copy(long long name_ptr, long long out_buffer, long long out_capacity) {
    const char* name = (const char*)(intptr_t)name_ptr;
    char* out = (char*)(intptr_t)out_buffer;
    if (!name || name[0] == '\0' || out_capacity < 0) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    const char* value = getenv(name);
    if (!value) {
        return -SENGOO_STATUS_NOT_FOUND;
    }

    size_t len = strlen(value);
    if ((unsigned long long)len > (unsigned long long)out_capacity || (len > 0 && !out)) {
        return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    }
    if (len > 0) {
        memcpy(out, value, len);
    }
    return (long long)len;
}

long long sengoo_env_is_windows(void) {
#ifdef _WIN32
    return 1;
#else
    return 0;
#endif
}

long long sengoo_env_is_unix(void) {
#ifdef _WIN32
    return 0;
#else
    return 1;
#endif
}

long long sengoo_time_unix_seconds(void) {
    time_t now = time(NULL);
    if (now == (time_t)-1) {
        return -1;
    }
    return (long long)now;
}

long long sengoo_time_unix_ms(void) {
#if defined(TIME_UTC)
    struct timespec ts;
    if (timespec_get(&ts, TIME_UTC) == TIME_UTC) {
        return ((long long)ts.tv_sec * 1000LL) + ((long long)ts.tv_nsec / 1000000LL);
    }
#endif
    long long seconds = sengoo_time_unix_seconds();
    if (seconds < 0) {
        return -1;
    }
    return seconds * 1000LL;
}

long long sengoo_time_sleep_ms(long long ms) {
    if (ms < 0) {
        return 1;
    }
    if (ms == 0) {
        return 0;
    }

#ifdef _WIN32
    while (ms > 0) {
        DWORD chunk = ms > (long long)UINT_MAX ? (DWORD)UINT_MAX : (DWORD)ms;
        Sleep(chunk);
        ms -= (long long)chunk;
    }
    return 0;
#else
    struct timespec req;
    req.tv_sec = (time_t)(ms / 1000LL);
    req.tv_nsec = (long)((ms % 1000LL) * 1000000LL);
    while (nanosleep(&req, &req) != 0) {
        if (errno != EINTR) {
            return 1;
        }
    }
    return 0;
#endif
}

static uint64_t sengoo_random_state = 0x9e3779b97f4a7c15ULL;

static uint64_t sengoo_random_next_u64(void) {
    uint64_t x = sengoo_random_state;
    if (x == 0) {
        x = 0x9e3779b97f4a7c15ULL;
    }

    x ^= x >> 12;
    x ^= x << 25;
    x ^= x >> 27;
    sengoo_random_state = x;
    return x * 2685821657736338717ULL;
}

long long sengoo_random_seed(long long seed) {
    uint64_t normalized = (uint64_t)seed;
    if (normalized == 0) {
        normalized = 0x9e3779b97f4a7c15ULL;
    }
    sengoo_random_state = normalized;
    return 0;
}

long long sengoo_random_i64(void) {
    return (long long)(sengoo_random_next_u64() >> 1);
}

long long sengoo_random_range_i64(long long min, long long max) {
    if (max <= min) {
        return min;
    }

    uint64_t span = (uint64_t)max - (uint64_t)min;
    uint64_t offset = sengoo_random_next_u64() % span;
    return min + (long long)offset;
}

long long sengoo_random_bool(void) {
    return (long long)(sengoo_random_next_u64() & 1ULL);
}

static int sengoo_path_is_sep(char ch) {
    return ch == '/' || ch == '\\';
}

static char sengoo_path_preferred_sep(void) {
#ifdef _WIN32
    return '\\';
#else
    return '/';
#endif
}

static int sengoo_path_has_drive_root(const char* path) {
    return path && isalpha((unsigned char)path[0]) && path[1] == ':' && sengoo_path_is_sep(path[2]);
}

static int sengoo_path_is_absolute_cstr(const char* path) {
    if (!path || path[0] == '\0') {
        return 0;
    }
    if (sengoo_path_has_drive_root(path)) {
        return 1;
    }
    if (sengoo_path_is_sep(path[0])) {
        return 1;
    }
    return 0;
}

static size_t sengoo_path_root_len(const char* path) {
    if (!path || path[0] == '\0') {
        return 0;
    }
    if (sengoo_path_has_drive_root(path)) {
        return 3;
    }
    if (sengoo_path_is_sep(path[0]) && sengoo_path_is_sep(path[1])) {
        return 2;
    }
    if (sengoo_path_is_sep(path[0])) {
        return 1;
    }
    return 0;
}

static long long sengoo_copy_path_bytes(const char* data, size_t len, long long out_buffer, long long out_capacity) {
    char* out = (char*)(intptr_t)out_buffer;
    if (out_capacity < 0) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if ((unsigned long long)len > (unsigned long long)out_capacity) {
        return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    }
    if (len > 0 && (!data || !out)) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (len > 0) {
        memcpy(out, data, len);
    }
    return (long long)len;
}

static int sengoo_path_file_name_range(const char* path, size_t* out_start, size_t* out_len) {
    if (!path || path[0] == '\0' || !out_start || !out_len) {
        return 0;
    }

    size_t root_len = sengoo_path_root_len(path);
    size_t end = strlen(path);
    while (end > root_len && sengoo_path_is_sep(path[end - 1])) {
        end--;
    }
    if (end <= root_len) {
        return 0;
    }

    size_t start = end;
    while (start > root_len && !sengoo_path_is_sep(path[start - 1])) {
        start--;
    }

    *out_start = start;
    *out_len = end - start;
    return *out_len > 0;
}

long long sengoo_path_separator(void) {
    return (long long)(unsigned char)sengoo_path_preferred_sep();
}

long long sengoo_path_is_absolute(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    return sengoo_path_is_absolute_cstr(path) ? 1 : 0;
}

long long sengoo_path_join(long long left_ptr, long long right_ptr, long long out_buffer, long long out_capacity) {
    const char* left = (const char*)(intptr_t)left_ptr;
    const char* right = (const char*)(intptr_t)right_ptr;
    if (!left || !right) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    if (sengoo_path_is_absolute_cstr(right) || left[0] == '\0') {
        return sengoo_copy_path_bytes(right, strlen(right), out_buffer, out_capacity);
    }
    if (right[0] == '\0') {
        return sengoo_copy_path_bytes(left, strlen(left), out_buffer, out_capacity);
    }

    size_t left_len = strlen(left);
    size_t left_root = sengoo_path_root_len(left);
    while (left_len > left_root && sengoo_path_is_sep(left[left_len - 1])) {
        left_len--;
    }

    size_t right_len = strlen(right);
    size_t right_start = 0;
    while (right_start < right_len && sengoo_path_is_sep(right[right_start])) {
        right_start++;
    }

    int needs_sep = left_len > 0 && right_start < right_len && !sengoo_path_is_sep(left[left_len - 1]);
    size_t result_len = left_len + (needs_sep ? 1u : 0u) + (right_len - right_start);
    char* result = (char*)malloc(result_len + 1u);
    if (!result) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }

    size_t pos = 0;
    if (left_len > 0) {
        memcpy(result + pos, left, left_len);
        pos += left_len;
    }
    if (needs_sep) {
        result[pos++] = sengoo_path_preferred_sep();
    }
    if (right_len > right_start) {
        memcpy(result + pos, right + right_start, right_len - right_start);
        pos += right_len - right_start;
    }
    result[pos] = '\0';

    long long copied = sengoo_copy_path_bytes(result, result_len, out_buffer, out_capacity);
    free(result);
    return copied;
}

long long sengoo_path_parent(long long path_ptr, long long out_buffer, long long out_capacity) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path || path[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    size_t root_len = sengoo_path_root_len(path);
    size_t end = strlen(path);
    while (end > root_len && sengoo_path_is_sep(path[end - 1])) {
        end--;
    }
    if (end <= root_len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }

    size_t pos = end;
    while (pos > root_len && !sengoo_path_is_sep(path[pos - 1])) {
        pos--;
    }
    if (pos == 0) {
        return -SENGOO_STATUS_NOT_FOUND;
    }

    size_t sep_index = pos - 1;
    size_t parent_len = sep_index < root_len ? root_len : sep_index;
    if (parent_len == 0 && root_len > 0) {
        parent_len = root_len;
    }
    if (parent_len == 0) {
        return -SENGOO_STATUS_NOT_FOUND;
    }

    return sengoo_copy_path_bytes(path, parent_len, out_buffer, out_capacity);
}

long long sengoo_path_file_name(long long path_ptr, long long out_buffer, long long out_capacity) {
    const char* path = (const char*)(intptr_t)path_ptr;
    size_t start = 0;
    size_t len = 0;
    if (!sengoo_path_file_name_range(path, &start, &len)) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    return sengoo_copy_path_bytes(path + start, len, out_buffer, out_capacity);
}

long long sengoo_path_stem(long long path_ptr, long long out_buffer, long long out_capacity) {
    const char* path = (const char*)(intptr_t)path_ptr;
    size_t start = 0;
    size_t len = 0;
    if (!sengoo_path_file_name_range(path, &start, &len)) {
        return -SENGOO_STATUS_NOT_FOUND;
    }

    size_t dot = start + len;
    while (dot > start && path[dot - 1] != '.') {
        dot--;
    }
    if (dot == start || dot == start + len) {
        return sengoo_copy_path_bytes(path + start, len, out_buffer, out_capacity);
    }

    size_t dot_index = dot - 1;
    size_t stem_len = dot_index > start ? dot_index - start : len;
    return sengoo_copy_path_bytes(path + start, stem_len, out_buffer, out_capacity);
}

long long sengoo_path_extension(long long path_ptr, long long out_buffer, long long out_capacity) {
    const char* path = (const char*)(intptr_t)path_ptr;
    size_t start = 0;
    size_t len = 0;
    if (!sengoo_path_file_name_range(path, &start, &len)) {
        return -SENGOO_STATUS_NOT_FOUND;
    }

    size_t dot = start + len;
    while (dot > start && path[dot - 1] != '.') {
        dot--;
    }
    if (dot == start || dot == start + len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }

    size_t dot_index = dot - 1;
    if (dot_index == start || dot_index + 1 >= start + len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    return sengoo_copy_path_bytes(path + dot_index + 1, (start + len) - dot_index - 1, out_buffer, out_capacity);
}

static int sengoo_path_segment_is_dotdot(const char* segment, size_t len) {
    return len == 2 && segment[0] == '.' && segment[1] == '.';
}

long long sengoo_path_normalize(long long path_ptr, long long out_buffer, long long out_capacity) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    size_t input_len = strlen(path);
    char sep = sengoo_path_preferred_sep();
    char* result = (char*)malloc(input_len + 4u);
    size_t* segment_starts = (size_t*)calloc(input_len + 1u, sizeof(size_t));
    size_t* segment_lens = (size_t*)calloc(input_len + 1u, sizeof(size_t));
    if (!result || !segment_starts || !segment_lens) {
        free(result);
        free(segment_starts);
        free(segment_lens);
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }

    size_t cursor = 0;
    size_t result_len = 0;
    size_t base_len = 0;
    size_t segment_count = 0;
    int absolute = 0;

    if (sengoo_path_has_drive_root(path)) {
        result[result_len++] = path[0];
        result[result_len++] = ':';
        result[result_len++] = sep;
        cursor = 3;
        base_len = 3;
        absolute = 1;
        while (cursor < input_len && sengoo_path_is_sep(path[cursor])) {
            cursor++;
        }
    } else if (sengoo_path_is_sep(path[0]) && sengoo_path_is_sep(path[1])) {
        result[result_len++] = sep;
        result[result_len++] = sep;
        cursor = 2;
        base_len = 2;
        absolute = 1;
        while (cursor < input_len && sengoo_path_is_sep(path[cursor])) {
            cursor++;
        }
    } else if (sengoo_path_is_sep(path[0])) {
        result[result_len++] = sep;
        cursor = 1;
        base_len = 1;
        absolute = 1;
        while (cursor < input_len && sengoo_path_is_sep(path[cursor])) {
            cursor++;
        }
    }

    while (cursor < input_len) {
        while (cursor < input_len && sengoo_path_is_sep(path[cursor])) {
            cursor++;
        }
        size_t start = cursor;
        while (cursor < input_len && !sengoo_path_is_sep(path[cursor])) {
            cursor++;
        }
        size_t len = cursor - start;
        if (len == 0 || (len == 1 && path[start] == '.')) {
            continue;
        }

        if (sengoo_path_segment_is_dotdot(path + start, len)) {
            if (segment_count > 0
                && !sengoo_path_segment_is_dotdot(result + segment_starts[segment_count - 1], segment_lens[segment_count - 1])) {
                segment_count--;
                result_len = segment_starts[segment_count];
                if (result_len > base_len && result_len > 0 && result[result_len - 1] == sep) {
                    result_len--;
                }
                continue;
            }
            if (absolute) {
                continue;
            }
        }

        if (result_len > 0 && result[result_len - 1] != sep && result[result_len - 1] != ':') {
            result[result_len++] = sep;
        }
        segment_starts[segment_count] = result_len;
        segment_lens[segment_count] = len;
        memcpy(result + result_len, path + start, len);
        result_len += len;
        segment_count++;
    }

    if (result_len == 0) {
        result[result_len++] = '.';
    }
    result[result_len] = '\0';

    long long copied = sengoo_copy_path_bytes(result, result_len, out_buffer, out_capacity);
    free(result);
    free(segment_starts);
    free(segment_lens);
    return copied;
}

static long long sengoo_path_result_to_string(long long copied, char* temp) {
    if (copied < 0) {
        return copied;
    }
    return sengoo_string_from_bytes_copy((long long)(intptr_t)temp, copied);
}

long long sengoo_path_join_string(long long left_ptr, long long right_ptr) {
    char* temp = (char*)malloc(SENGOO_RUNTIME_MAX_PATH_BYTES);
    if (!temp) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    long long copied = sengoo_path_join(
        left_ptr,
        right_ptr,
        (long long)(intptr_t)temp,
        (long long)SENGOO_RUNTIME_MAX_PATH_BYTES);
    long long handle = sengoo_path_result_to_string(copied, temp);
    free(temp);
    return handle;
}

long long sengoo_path_parent_string(long long path_ptr) {
    char* temp = (char*)malloc(SENGOO_RUNTIME_MAX_PATH_BYTES);
    if (!temp) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    long long copied = sengoo_path_parent(
        path_ptr,
        (long long)(intptr_t)temp,
        (long long)SENGOO_RUNTIME_MAX_PATH_BYTES);
    long long handle = sengoo_path_result_to_string(copied, temp);
    free(temp);
    return handle;
}

long long sengoo_path_file_name_string(long long path_ptr) {
    char* temp = (char*)malloc(SENGOO_RUNTIME_MAX_PATH_BYTES);
    if (!temp) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    long long copied = sengoo_path_file_name(
        path_ptr,
        (long long)(intptr_t)temp,
        (long long)SENGOO_RUNTIME_MAX_PATH_BYTES);
    long long handle = sengoo_path_result_to_string(copied, temp);
    free(temp);
    return handle;
}

long long sengoo_path_stem_string(long long path_ptr) {
    char* temp = (char*)malloc(SENGOO_RUNTIME_MAX_PATH_BYTES);
    if (!temp) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    long long copied = sengoo_path_stem(
        path_ptr,
        (long long)(intptr_t)temp,
        (long long)SENGOO_RUNTIME_MAX_PATH_BYTES);
    long long handle = sengoo_path_result_to_string(copied, temp);
    free(temp);
    return handle;
}

long long sengoo_path_extension_string(long long path_ptr) {
    char* temp = (char*)malloc(SENGOO_RUNTIME_MAX_PATH_BYTES);
    if (!temp) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    long long copied = sengoo_path_extension(
        path_ptr,
        (long long)(intptr_t)temp,
        (long long)SENGOO_RUNTIME_MAX_PATH_BYTES);
    long long handle = sengoo_path_result_to_string(copied, temp);
    free(temp);
    return handle;
}

long long sengoo_path_normalize_string(long long path_ptr) {
    char* temp = (char*)malloc(SENGOO_RUNTIME_MAX_PATH_BYTES);
    if (!temp) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    long long copied = sengoo_path_normalize(
        path_ptr,
        (long long)(intptr_t)temp,
        (long long)SENGOO_RUNTIME_MAX_PATH_BYTES);
    long long handle = sengoo_path_result_to_string(copied, temp);
    free(temp);
    return handle;
}

static int sengoo_dir_exists_cstr(const char* path) {
    if (!path || path[0] == '\0') {
        return 0;
    }

#ifdef _WIN32
    DWORD attributes = GetFileAttributesA(path);
    return attributes != INVALID_FILE_ATTRIBUTES && (attributes & FILE_ATTRIBUTE_DIRECTORY) ? 1 : 0;
#else
    struct stat info;
    return stat(path, &info) == 0 && S_ISDIR(info.st_mode) ? 1 : 0;
#endif
}

static int sengoo_dir_create_one_cstr(const char* path) {
    if (!path || path[0] == '\0') {
        return -1;
    }
    if (sengoo_dir_exists_cstr(path)) {
        return 0;
    }

#ifdef _WIN32
    if (_mkdir(path) == 0) {
        return 0;
    }
#else
    if (mkdir(path, 0777) == 0) {
        return 0;
    }
#endif

    return sengoo_dir_exists_cstr(path) ? 0 : -1;
}

long long sengoo_dir_exists(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    return sengoo_dir_exists_cstr(path) ? 1 : 0;
}

long long sengoo_dir_create(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    return sengoo_dir_create_one_cstr(path);
}

long long sengoo_dir_create_all(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path || path[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (sengoo_dir_exists_cstr(path)) {
        return 0;
    }

    size_t len = strlen(path);
    char* scratch = (char*)malloc(len + 1);
    if (!scratch) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    memcpy(scratch, path, len + 1);

    size_t root_len = sengoo_path_root_len(scratch);
    for (size_t i = root_len; i < len; i++) {
        if (!sengoo_path_is_sep(scratch[i])) {
            continue;
        }

        scratch[i] = '\0';
        if (i > root_len && sengoo_dir_create_one_cstr(scratch) != 0) {
            int err = errno;
            scratch[i] = path[i];
            free(scratch);
            return sengoo_negative_status_from_errno(err, SENGOO_STATUS_IO);
        }
        scratch[i] = path[i];
    }

    int status = sengoo_dir_create_one_cstr(scratch);
    int err = errno;
    free(scratch);
    return status == 0 ? 0 : sengoo_negative_status_from_errno(err, SENGOO_STATUS_IO);
}

long long sengoo_dir_remove(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path || path[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

#ifdef _WIN32
    return _rmdir(path) == 0 ? 0 : sengoo_negative_status_from_errno(errno, SENGOO_STATUS_IO);
#else
    return rmdir(path) == 0 ? 0 : sengoo_negative_status_from_errno(errno, SENGOO_STATUS_IO);
#endif
}

typedef struct {
    char** names;
    size_t len;
    size_t cap;
} SengooDirEntryList;

static void sengoo_dir_entry_list_free(SengooDirEntryList* list) {
    if (!list) {
        return;
    }
    for (size_t i = 0; i < list->len; i++) {
        free(list->names[i]);
    }
    free(list->names);
    list->names = NULL;
    list->len = 0;
    list->cap = 0;
}

char* sengoo_strdup_bytes(const char* value) {
    if (!value) {
        return NULL;
    }
    size_t len = strlen(value);
    char* copy = (char*)malloc(len + 1);
    if (!copy) {
        return NULL;
    }
    memcpy(copy, value, len + 1);
    return copy;
}

static int sengoo_dir_entry_list_push(SengooDirEntryList* list, const char* name) {
    if (!list || !name) {
        return -1;
    }
    if (strcmp(name, ".") == 0 || strcmp(name, "..") == 0) {
        return 0;
    }

    if (list->len == list->cap) {
        size_t next_cap = list->cap == 0 ? 8 : list->cap * 2;
        char** next_names = (char**)realloc(list->names, next_cap * sizeof(char*));
        if (!next_names) {
            return -1;
        }
        list->names = next_names;
        list->cap = next_cap;
    }

    char* copy = sengoo_strdup_bytes(name);
    if (!copy) {
        return -1;
    }
    list->names[list->len++] = copy;
    return 0;
}

static int sengoo_dir_entry_name_compare(const void* lhs, const void* rhs) {
    const char* a = *(const char* const*)lhs;
    const char* b = *(const char* const*)rhs;
    return strcmp(a, b);
}

static int sengoo_dir_collect_entries(const char* path, SengooDirEntryList* list) {
    if (!path || path[0] == '\0' || !list) {
        return -1;
    }
    memset(list, 0, sizeof(*list));

#ifdef _WIN32
    DWORD attributes = GetFileAttributesA(path);
    if (attributes == INVALID_FILE_ATTRIBUTES || !(attributes & FILE_ATTRIBUTE_DIRECTORY)) {
        return -1;
    }

    size_t path_len = strlen(path);
    int needs_sep = path_len > 0 && !sengoo_path_is_sep(path[path_len - 1]);
    size_t pattern_len = path_len + (needs_sep ? 1 : 0) + 1;
    char* pattern = (char*)malloc(pattern_len + 1);
    if (!pattern) {
        return -1;
    }
    memcpy(pattern, path, path_len);
    size_t pos = path_len;
    if (needs_sep) {
        pattern[pos++] = '\\';
    }
    pattern[pos++] = '*';
    pattern[pos] = '\0';

    WIN32_FIND_DATAA find_data;
    HANDLE handle = FindFirstFileA(pattern, &find_data);
    free(pattern);
    if (handle == INVALID_HANDLE_VALUE) {
        return -1;
    }

    do {
        if (sengoo_dir_entry_list_push(list, find_data.cFileName) != 0) {
            FindClose(handle);
            sengoo_dir_entry_list_free(list);
            return -1;
        }
    } while (FindNextFileA(handle, &find_data));
    DWORD err = GetLastError();
    FindClose(handle);
    if (err != ERROR_NO_MORE_FILES) {
        sengoo_dir_entry_list_free(list);
        return -1;
    }
#else
    DIR* dir = opendir(path);
    if (!dir) {
        return -1;
    }

    errno = 0;
    struct dirent* entry = NULL;
    while ((entry = readdir(dir)) != NULL) {
        if (sengoo_dir_entry_list_push(list, entry->d_name) != 0) {
            closedir(dir);
            sengoo_dir_entry_list_free(list);
            return -1;
        }
    }
    int read_errno = errno;
    closedir(dir);
    if (read_errno != 0) {
        sengoo_dir_entry_list_free(list);
        return -1;
    }
#endif

    if (list->len > 1) {
        qsort(list->names, list->len, sizeof(char*), sengoo_dir_entry_name_compare);
    }
    return 0;
}

long long sengoo_dir_entry_count(long long path_ptr) {
    const char* path = (const char*)(intptr_t)path_ptr;
    SengooDirEntryList list;
    if (sengoo_dir_collect_entries(path, &list) != 0) {
        return (!path || path[0] == '\0') ? -SENGOO_STATUS_INVALID_ARGUMENT : -SENGOO_STATUS_NOT_FOUND;
    }
    size_t count = list.len;
    sengoo_dir_entry_list_free(&list);
    if (count > (size_t)LLONG_MAX) {
        return -SENGOO_STATUS_OVERFLOW;
    }
    return (long long)count;
}

long long sengoo_dir_entry_name(long long path_ptr, long long index, long long out_buffer, long long out_capacity) {
    const char* path = (const char*)(intptr_t)path_ptr;
    char* out = (char*)(intptr_t)out_buffer;
    if (index < 0 || out_capacity < 0) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    SengooDirEntryList list;
    if (sengoo_dir_collect_entries(path, &list) != 0) {
        return (!path || path[0] == '\0') ? -SENGOO_STATUS_INVALID_ARGUMENT : -SENGOO_STATUS_NOT_FOUND;
    }

    if ((unsigned long long)index >= (unsigned long long)list.len) {
        sengoo_dir_entry_list_free(&list);
        return -SENGOO_STATUS_NOT_FOUND;
    }

    const char* name = list.names[index];
    size_t len = strlen(name);
    if ((unsigned long long)len > (unsigned long long)out_capacity || (len > 0 && !out)) {
        sengoo_dir_entry_list_free(&list);
        return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    }
    if (len > 0) {
        memcpy(out, name, len);
    }
    sengoo_dir_entry_list_free(&list);
    return (long long)len;
}

long long sengoo_dir_entry_name_string(long long path_ptr, long long index) {
    char* temp = (char*)malloc(SENGOO_RUNTIME_MAX_PATH_BYTES);
    if (!temp) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    long long copied = sengoo_dir_entry_name(
        path_ptr,
        index,
        (long long)(intptr_t)temp,
        (long long)SENGOO_RUNTIME_MAX_PATH_BYTES);
    long long handle = sengoo_path_result_to_string(copied, temp);
    free(temp);
    return handle;
}

typedef struct {
    char** paths;
    size_t len;
    size_t cap;
    size_t index;
} SengooDirWalk;

static SengooDirWalk* sengoo_dir_walk_from_handle(long long handle) {
    return (SengooDirWalk*)sengoo_handle_to_ptr(handle);
}

static int sengoo_dir_walk_push(SengooDirWalk* walk, const char* path) {
    if (!walk || !path) {
        return 0;
    }
    if (walk->len == walk->cap) {
        size_t next = walk->cap == 0 ? 16 : walk->cap;
        if (next > SIZE_MAX / 2) {
            return 0;
        }
        next *= 2;
        char** paths = (char**)realloc(walk->paths, next * sizeof(char*));
        if (!paths) {
            return 0;
        }
        walk->paths = paths;
        walk->cap = next;
    }
    char* copy = sengoo_strdup_bytes(path);
    if (!copy) {
        return 0;
    }
    walk->paths[walk->len++] = copy;
    return 1;
}

static void sengoo_dir_walk_free(SengooDirWalk* walk) {
    if (!walk) {
        return;
    }
    for (size_t i = 0; i < walk->len; ++i) {
        free(walk->paths[i]);
    }
    free(walk->paths);
    free(walk);
}

static char* sengoo_dir_join_full_path(const char* root, const char* rel) {
    if (!root || !rel) {
        return NULL;
    }
    size_t root_len = strlen(root);
    size_t rel_len = strlen(rel);
    int needs_sep = root_len > 0 && rel_len > 0 && !sengoo_path_is_sep(root[root_len - 1]);
    size_t len = root_len + (needs_sep ? 1u : 0u) + rel_len;
    char* out = (char*)malloc(len + 1u);
    if (!out) {
        return NULL;
    }
    size_t pos = 0;
    memcpy(out + pos, root, root_len);
    pos += root_len;
    if (needs_sep) {
        out[pos++] = sengoo_path_preferred_sep();
    }
    for (size_t i = 0; i < rel_len; ++i) {
        out[pos++] = rel[i] == '/' ? sengoo_path_preferred_sep() : rel[i];
    }
    out[pos] = '\0';
    return out;
}

static char* sengoo_dir_join_relative_path(const char* base, const char* name) {
    if (!name) {
        return NULL;
    }
    if (!base || base[0] == '\0') {
        return sengoo_strdup_bytes(name);
    }
    size_t base_len = strlen(base);
    size_t name_len = strlen(name);
    size_t len = base_len + 1u + name_len;
    char* out = (char*)malloc(len + 1u);
    if (!out) {
        return NULL;
    }
    memcpy(out, base, base_len);
    out[base_len] = '/';
    memcpy(out + base_len + 1u, name, name_len);
    out[len] = '\0';
    return out;
}

static int sengoo_path_is_dir_nofollow(const char* path) {
    if (!path || path[0] == '\0') {
        return 0;
    }
#ifdef _WIN32
    DWORD attributes = GetFileAttributesA(path);
    return attributes != INVALID_FILE_ATTRIBUTES
        && (attributes & FILE_ATTRIBUTE_DIRECTORY)
        && !(attributes & FILE_ATTRIBUTE_REPARSE_POINT) ? 1 : 0;
#else
    struct stat info;
    return lstat(path, &info) == 0 && S_ISDIR(info.st_mode) ? 1 : 0;
#endif
}

static int sengoo_dir_walk_collect(SengooDirWalk* walk, const char* root, const char* rel, long long depth, long long max_depth) {
    char* full = sengoo_dir_join_full_path(root, rel ? rel : "");
    if (!full) {
        return 0;
    }

    SengooDirEntryList entries;
    if (sengoo_dir_collect_entries(full, &entries) != 0) {
        free(full);
        return 0;
    }

    for (size_t i = 0; i < entries.len; ++i) {
        char* child_rel = sengoo_dir_join_relative_path(rel ? rel : "", entries.names[i]);
        if (!child_rel) {
            sengoo_dir_entry_list_free(&entries);
            free(full);
            return 0;
        }
        if (!sengoo_dir_walk_push(walk, child_rel)) {
            free(child_rel);
            sengoo_dir_entry_list_free(&entries);
            free(full);
            return 0;
        }
        char* child_full = sengoo_dir_join_full_path(root, child_rel);
        if (!child_full) {
            free(child_rel);
            sengoo_dir_entry_list_free(&entries);
            free(full);
            return 0;
        }
        int should_recurse = depth < max_depth && sengoo_path_is_dir_nofollow(child_full);
        free(child_full);
        if (should_recurse && !sengoo_dir_walk_collect(walk, root, child_rel, depth + 1, max_depth)) {
            free(child_rel);
            sengoo_dir_entry_list_free(&entries);
            free(full);
            return 0;
        }
        free(child_rel);
    }

    sengoo_dir_entry_list_free(&entries);
    free(full);
    return 1;
}

long long sengoo_dir_walk_new(long long root_ptr, long long max_depth) {
    const char* root = (const char*)(intptr_t)root_ptr;
    if (!root || root[0] == '\0' || max_depth < 0) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (!sengoo_dir_exists_cstr(root)) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    SengooDirWalk* walk = (SengooDirWalk*)calloc(1, sizeof(SengooDirWalk));
    if (!walk) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    if (!sengoo_dir_walk_collect(walk, root, "", 0, max_depth)) {
        sengoo_dir_walk_free(walk);
        return -SENGOO_STATUS_IO;
    }
    return sengoo_ptr_to_handle(walk);
}

long long sengoo_dir_walk_next(long long handle, long long buffer_handle) {
    SengooDirWalk* walk = sengoo_dir_walk_from_handle(handle);
    if (!walk) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (walk->index >= walk->len) {
        SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
        if (buffer) {
            buffer->used_len = 0;
        }
        return 0;
    }
    const char* path = walk->paths[walk->index];
    long long copied = sengoo_copy_bytes_to_managed_buffer(buffer_handle, path, strlen(path));
    if (copied >= 0) {
        walk->index += 1;
    }
    return copied;
}

long long sengoo_dir_walk_close(long long handle) {
    SengooDirWalk* walk = sengoo_dir_walk_from_handle(handle);
    if (!walk) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    sengoo_dir_walk_free(walk);
    return 0;
}

static int sengoo_path_is_symlink_nofollow(const char* path) {
    if (!path || path[0] == '\0') {
        return 0;
    }
#ifdef _WIN32
    DWORD attributes = GetFileAttributesA(path);
    return attributes != INVALID_FILE_ATTRIBUTES && (attributes & FILE_ATTRIBUTE_REPARSE_POINT) ? 1 : 0;
#else
    struct stat info;
    return lstat(path, &info) == 0 && S_ISLNK(info.st_mode) ? 1 : 0;
#endif
}

typedef struct {
    size_t entries;
    long long max_entries;
    long long max_depth;
    long long error;
} SengooDirTreeLimits;

static long long sengoo_dir_tree_resolve_limit(long long value, long long fallback) {
    return value > 0 ? value : fallback;
}

static int sengoo_dir_tree_consume_entry(SengooDirTreeLimits* limits) {
    if (!limits) {
        return 0;
    }
    limits->entries += 1;
    if ((long long)limits->entries > limits->max_entries) {
        limits->error = SENGOO_STATUS_OVERFLOW;
        return 0;
    }
    return 1;
}

static int sengoo_dir_remove_tree_recursive(const char* path, long long depth, SengooDirTreeLimits* limits) {
    if (!path || !limits || limits->error != SENGOO_STATUS_OK) {
        return 0;
    }
    if (depth > limits->max_depth) {
        limits->error = SENGOO_STATUS_OVERFLOW;
        return 0;
    }
    if (!sengoo_dir_tree_consume_entry(limits)) {
        return 0;
    }
    if (sengoo_path_is_symlink_nofollow(path)) {
        return remove(path) == 0 ? 1 : 0;
    }
    if (!sengoo_path_is_dir_nofollow(path)) {
        return remove(path) == 0 ? 1 : 0;
    }

    SengooDirEntryList entries;
    if (sengoo_dir_collect_entries(path, &entries) != 0) {
        limits->error = SENGOO_STATUS_IO;
        return 0;
    }

    char* child_full = NULL;
    for (size_t i = 0; i < entries.len; ++i) {
        child_full = sengoo_dir_join_full_path(path, entries.names[i]);
        if (!child_full) {
            limits->error = SENGOO_STATUS_OUT_OF_MEMORY;
            break;
        }
        if (!sengoo_dir_remove_tree_recursive(child_full, depth + 1, limits)) {
            free(child_full);
            break;
        }
        free(child_full);
        child_full = NULL;
        if (limits->error != SENGOO_STATUS_OK) {
            break;
        }
    }
    sengoo_dir_entry_list_free(&entries);
    if (limits->error != SENGOO_STATUS_OK) {
        return 0;
    }
    return sengoo_dir_remove((long long)(intptr_t)path) == 0 ? 1 : 0;
}

long long sengoo_dir_remove_tree(long long path_ptr, long long max_depth, long long max_entries) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path || path[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (!sengoo_dir_exists_cstr(path)) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    SengooDirTreeLimits limits;
    limits.entries = 0;
    limits.max_depth = sengoo_dir_tree_resolve_limit(max_depth, SENGOO_RUNTIME_MAX_DIR_DEPTH);
    limits.max_entries = sengoo_dir_tree_resolve_limit(max_entries, SENGOO_RUNTIME_MAX_DIR_ENTRIES);
    limits.error = SENGOO_STATUS_OK;
    if (!sengoo_dir_remove_tree_recursive(path, 0, &limits)) {
        return limits.error == SENGOO_STATUS_OK ? -SENGOO_STATUS_IO : -(long long)limits.error;
    }
    return (long long)limits.entries;
}

static int sengoo_dir_copy_tree_recursive(
    const char* source,
    const char* destination,
    long long depth,
    SengooDirTreeLimits* limits) {
    if (!source || !destination || !limits || limits->error != SENGOO_STATUS_OK) {
        return 0;
    }
    if (depth > limits->max_depth) {
        limits->error = SENGOO_STATUS_OVERFLOW;
        return 0;
    }
    if (!sengoo_dir_tree_consume_entry(limits)) {
        return 0;
    }
    if (sengoo_path_is_symlink_nofollow(source)) {
        return 1;
    }
    if (sengoo_path_is_dir_nofollow(source)) {
        if (sengoo_dir_create_all((long long)(intptr_t)destination) != 0) {
            limits->error = SENGOO_STATUS_IO;
            return 0;
        }
        SengooDirEntryList entries;
        if (sengoo_dir_collect_entries(source, &entries) != 0) {
            limits->error = SENGOO_STATUS_IO;
            return 0;
        }
        char* child_src = NULL;
        char* child_dst = NULL;
        for (size_t i = 0; i < entries.len; ++i) {
            child_src = sengoo_dir_join_full_path(source, entries.names[i]);
            child_dst = sengoo_dir_join_full_path(destination, entries.names[i]);
            if (!child_src || !child_dst) {
                limits->error = SENGOO_STATUS_OUT_OF_MEMORY;
                free(child_src);
                free(child_dst);
                break;
            }
            if (!sengoo_dir_copy_tree_recursive(child_src, child_dst, depth + 1, limits)) {
                free(child_src);
                free(child_dst);
                break;
            }
            free(child_src);
            free(child_dst);
            child_src = NULL;
            child_dst = NULL;
            if (limits->error != SENGOO_STATUS_OK) {
                break;
            }
        }
        sengoo_dir_entry_list_free(&entries);
        return limits->error == SENGOO_STATUS_OK ? 1 : 0;
    }
    if (sengoo_path_is_regular_file_cstr(source)) {
        long long copied = sengoo_file_copy(
            (long long)(intptr_t)source,
            (long long)(intptr_t)destination,
            1);
        if (copied < 0) {
            limits->error = (int)(-copied);
            return 0;
        }
        return 1;
    }
    return 1;
}

long long sengoo_dir_copy_tree(
    long long source_ptr,
    long long destination_ptr,
    long long max_depth,
    long long max_entries) {
    const char* source = (const char*)(intptr_t)source_ptr;
    const char* destination = (const char*)(intptr_t)destination_ptr;
    if (!source || source[0] == '\0' || !destination || destination[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (!sengoo_path_entry_exists_cstr(source)) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    SengooDirTreeLimits limits;
    limits.entries = 0;
    limits.max_depth = sengoo_dir_tree_resolve_limit(max_depth, SENGOO_RUNTIME_MAX_DIR_DEPTH);
    limits.max_entries = sengoo_dir_tree_resolve_limit(max_entries, SENGOO_RUNTIME_MAX_DIR_ENTRIES);
    limits.error = SENGOO_STATUS_OK;
    if (!sengoo_dir_copy_tree_recursive(source, destination, 0, &limits)) {
        return limits.error == SENGOO_STATUS_OK ? -SENGOO_STATUS_IO : -(long long)limits.error;
    }
    return (long long)limits.entries;
}

long long sengoo_io_stdin_read(long long out_buffer, long long out_capacity) {
    char* out = (char*)(intptr_t)out_buffer;
    if (out_capacity < 0 || (out_capacity > 0 && !out)) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (out_capacity == 0) {
        return 0;
    }

    size_t read = fread(out, 1, (size_t)out_capacity, stdin);
    if (ferror(stdin)) {
        return -SENGOO_STATUS_IO;
    }
    return (long long)read;
}

long long sengoo_io_stdin_read_line(long long out_buffer, long long out_capacity) {
    char* out = (char*)(intptr_t)out_buffer;
    if (out_capacity < 0 || (out_capacity > 0 && !out)) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (out_capacity == 0) {
        return 0;
    }

    size_t count = 0;
    while (count < (size_t)out_capacity) {
        int ch = fgetc(stdin);
        if (ch == EOF) {
            if (ferror(stdin)) {
                return -SENGOO_STATUS_IO;
            }
            break;
        }

        out[count++] = (char)ch;
        if (ch == '\n') {
            break;
        }
    }
    return (long long)count;
}

static long long sengoo_io_write_stream(FILE* stream, long long data_ptr, long long len) {
    const char* data = (const char*)(intptr_t)data_ptr;
    if (!stream || len < 0 || (len > 0 && !data)) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    size_t expected = (size_t)len;
    size_t wrote = expected == 0 ? 0 : fwrite(data, 1, expected, stream);
    if (wrote != expected || ferror(stream)) {
        return -SENGOO_STATUS_IO;
    }
    return (long long)wrote;
}

long long sengoo_io_stdout_write(long long data_ptr, long long len) {
    return sengoo_io_write_stream(stdout, data_ptr, len);
}

long long sengoo_io_stderr_write(long long data_ptr, long long len) {
    return sengoo_io_write_stream(stderr, data_ptr, len);
}

long long sengoo_io_stdout_flush(void) {
    return fflush(stdout) == 0 ? 0 : -SENGOO_STATUS_IO;
}

long long sengoo_io_stderr_flush(void) {
    return fflush(stderr) == 0 ? 0 : -SENGOO_STATUS_IO;
}

long long sengoo_io_fd_read(long long fd, long long out_buffer, long long out_capacity) {
    char* out = (char*)(intptr_t)out_buffer;
    if (fd < 0 || out_capacity < 0 || (out_capacity > 0 && !out)) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (out_capacity == 0) {
        return 0;
    }
#ifdef _WIN32
    HANDLE handle = (HANDLE)_get_osfhandle((int)fd);
    if (handle == INVALID_HANDLE_VALUE) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    DWORD read = 0;
    if (!ReadFile(handle, out, (DWORD)out_capacity, &read, NULL)) {
        return -SENGOO_STATUS_IO;
    }
    return (long long)read;
#else
    ssize_t nread = read((int)fd, out, (size_t)out_capacity);
    if (nread < 0) {
        return -SENGOO_STATUS_IO;
    }
    return (long long)nread;
#endif
}

long long sengoo_io_fd_write(long long fd, long long data_ptr, long long len) {
    const char* data = (const char*)(intptr_t)data_ptr;
    if (fd < 0 || len < 0 || (len > 0 && !data)) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
#ifdef _WIN32
    HANDLE handle = (HANDLE)_get_osfhandle((int)fd);
    if (handle == INVALID_HANDLE_VALUE) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    DWORD wrote = 0;
    if (len > 0 && !WriteFile(handle, data, (DWORD)len, &wrote, NULL)) {
        return -SENGOO_STATUS_IO;
    }
    return (long long)wrote;
#else
    ssize_t nwrote = write((int)fd, data, (size_t)len);
    if (nwrote < 0) {
        return -SENGOO_STATUS_IO;
    }
    return (long long)nwrote;
#endif
}

long long sengoo_process_id(void) {
#ifdef _WIN32
    return (long long)GetCurrentProcessId();
#else
    return (long long)getpid();
#endif
}

static char* sengoo_process_current_dir_alloc(void) {
#ifdef _WIN32
    return _getcwd(NULL, 0);
#else
    size_t capacity = 256;
    while (capacity <= (size_t)(1024 * 1024)) {
        char* buffer = (char*)malloc(capacity);
        if (!buffer) {
            return NULL;
        }

        errno = 0;
        if (getcwd(buffer, capacity)) {
            return buffer;
        }

        int err = errno;
        free(buffer);
        if (err != ERANGE) {
            return NULL;
        }
        capacity *= 2;
    }
    return NULL;
#endif
}

long long sengoo_process_current_dir_len(void) {
    char* cwd = sengoo_process_current_dir_alloc();
    if (!cwd) {
        return -SENGOO_STATUS_IO;
    }

    size_t len = strlen(cwd);
    free(cwd);
    return (long long)len;
}

long long sengoo_process_current_dir_copy(long long out_buffer, long long out_capacity) {
    char* out = (char*)(intptr_t)out_buffer;
    if (out_capacity < 0) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    char* cwd = sengoo_process_current_dir_alloc();
    if (!cwd) {
        return -SENGOO_STATUS_IO;
    }

    size_t len = strlen(cwd);
    if ((unsigned long long)len > (unsigned long long)out_capacity || (len > 0 && !out)) {
        free(cwd);
        return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    }

    if (len > 0) {
        memcpy(out, cwd, len);
    }
    free(cwd);
    return (long long)len;
}

static int sengoo_process_run_args_are_valid(
    const char* executable,
    const char* const args[3],
    long long arg_count
) {
    if (!executable || executable[0] == '\0' || arg_count < 0 || arg_count > 3) {
        return 0;
    }
    for (long long i = 0; i < arg_count; i++) {
        if (!args[i]) {
            return 0;
        }
    }
    return 1;
}

#ifdef _WIN32
int sengoo_size_add(size_t* total, size_t value) {
    if (SIZE_MAX - *total < value) {
        return -1;
    }
    *total += value;
    return 0;
}

char* sengoo_windows_append_quoted_arg(char* out, const char* arg) {
    *out++ = '"';
    while (*arg) {
        size_t backslashes = 0;
        while (*arg == '\\') {
            backslashes++;
            arg++;
        }

        if (*arg == '"') {
            for (size_t i = 0; i < backslashes * 2 + 1; i++) {
                *out++ = '\\';
            }
            *out++ = *arg++;
        } else if (*arg == '\0') {
            for (size_t i = 0; i < backslashes * 2; i++) {
                *out++ = '\\';
            }
        } else {
            for (size_t i = 0; i < backslashes; i++) {
                *out++ = '\\';
            }
            *out++ = *arg++;
        }
    }
    *out++ = '"';
    return out;
}

static int sengoo_windows_arg_needs_quotes(const char* arg) {
    if (arg[0] == '\0') {
        return 1;
    }
    while (*arg) {
        if (*arg == ' ' || *arg == '\t' || *arg == '"') {
            return 1;
        }
        arg++;
    }
    return 0;
}

char* sengoo_windows_append_arg(char* out, const char* arg) {
    if (sengoo_windows_arg_needs_quotes(arg)) {
        return sengoo_windows_append_quoted_arg(out, arg);
    }
    while (*arg) {
        *out++ = *arg++;
    }
    return out;
}

static char* sengoo_windows_process_command_line(
    const char* executable,
    const char* const args[3],
    long long arg_count
) {
    const char* values[4] = {executable, args[0], args[1], args[2]};
    size_t value_count = (size_t)arg_count + 1;
    size_t capacity = 1;
    for (size_t i = 0; i < value_count; i++) {
        size_t len = strlen(values[i]);
        if (len > (SIZE_MAX - 3) / 2
            || sengoo_size_add(&capacity, len * 2 + 3) != 0) {
            return NULL;
        }
    }

    char* command_line = (char*)malloc(capacity);
    if (!command_line) {
        return NULL;
    }

    char* out = command_line;
    for (size_t i = 0; i < value_count; i++) {
        if (i > 0) {
            *out++ = ' ';
        }
        out = sengoo_windows_append_arg(out, values[i]);
    }
    *out = '\0';
    return command_line;
}

static long long sengoo_process_run_windows(
    const char* executable,
    const char* const args[3],
    long long arg_count
) {
    char* command_line = sengoo_windows_process_command_line(executable, args, arg_count);
    if (!command_line) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }

    STARTUPINFOA startup_info = {0};
    PROCESS_INFORMATION process_info = {0};
    startup_info.cb = sizeof(startup_info);
    BOOL created = CreateProcessA(
        NULL,
        command_line,
        NULL,
        NULL,
        TRUE,
        0,
        NULL,
        NULL,
        &startup_info,
        &process_info
    );
    free(command_line);
    if (!created) {
        return -SENGOO_STATUS_IO;
    }

    DWORD wait_status = WaitForSingleObject(process_info.hProcess, INFINITE);
    DWORD exit_code = 0;
    int ok = wait_status == WAIT_OBJECT_0 && GetExitCodeProcess(process_info.hProcess, &exit_code);
    CloseHandle(process_info.hThread);
    CloseHandle(process_info.hProcess);
    return ok ? (long long)exit_code : -SENGOO_STATUS_IO;
}
#else
static long long sengoo_process_run_unix(
    const char* executable,
    const char* const args[3],
    long long arg_count
) {
    int startup_pipe[2];
    if (pipe(startup_pipe) != 0) {
        return -SENGOO_STATUS_IO;
    }

    int flags = fcntl(startup_pipe[1], F_GETFD);
    if (flags < 0 || fcntl(startup_pipe[1], F_SETFD, flags | FD_CLOEXEC) != 0) {
        close(startup_pipe[0]);
        close(startup_pipe[1]);
        return -SENGOO_STATUS_IO;
    }

    pid_t pid = fork();
    if (pid < 0) {
        close(startup_pipe[0]);
        close(startup_pipe[1]);
        return -SENGOO_STATUS_IO;
    }
    if (pid == 0) {
        close(startup_pipe[0]);
        char* argv[5] = {(char*)executable, NULL, NULL, NULL, NULL};
        for (long long i = 0; i < arg_count; i++) {
            argv[i + 1] = (char*)args[i];
        }
        execvp(executable, argv);
        int startup_errno = errno;
        (void)write(startup_pipe[1], &startup_errno, sizeof(startup_errno));
        _exit(127);
    }

    close(startup_pipe[1]);
    int startup_errno = 0;
    ssize_t startup_read;
    do {
        startup_read = read(startup_pipe[0], &startup_errno, sizeof(startup_errno));
    } while (startup_read < 0 && errno == EINTR);
    close(startup_pipe[0]);

    int status = 0;
    pid_t waited;
    do {
        waited = waitpid(pid, &status, 0);
    } while (waited < 0 && errno == EINTR);
    if (startup_read != 0 || waited != pid || !WIFEXITED(status)) {
        return startup_read != 0
            ? sengoo_negative_status_from_errno(startup_errno, SENGOO_STATUS_IO)
            : -SENGOO_STATUS_IO;
    }
    return (long long)WEXITSTATUS(status);
}
#endif

long long sengoo_process_run(
    long long executable_ptr,
    long long arg0_ptr,
    long long arg1_ptr,
    long long arg2_ptr,
    long long arg_count
) {
    const char* executable = (const char*)(intptr_t)executable_ptr;
    const char* args[3] = {
        (const char*)(intptr_t)arg0_ptr,
        (const char*)(intptr_t)arg1_ptr,
        (const char*)(intptr_t)arg2_ptr,
    };
    if (!sengoo_process_run_args_are_valid(executable, args, arg_count)) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

#ifdef _WIN32
    return sengoo_process_run_windows(executable, args, arg_count);
#else
    return sengoo_process_run_unix(executable, args, arg_count);
#endif
}

static long long sengoo_runtime_argc = 0;
static char** sengoo_runtime_argv = NULL;

void sengoo_args_init(long long argc, long long argv_ptr) {
    if (argc <= 0 || argv_ptr == 0) {
        sengoo_runtime_argc = 0;
        sengoo_runtime_argv = NULL;
        return;
    }

    sengoo_runtime_argc = argc;
    sengoo_runtime_argv = (char**)(intptr_t)argv_ptr;
}

static const char* sengoo_user_arg_at(long long index) {
    if (index < 0 || !sengoo_runtime_argv) {
        return NULL;
    }

    long long os_index = index + 1;
    if (os_index <= 0 || os_index >= sengoo_runtime_argc) {
        return NULL;
    }

    return sengoo_runtime_argv[os_index];
}

long long sengoo_args_len(void) {
    if (sengoo_runtime_argc <= 1) {
        return 0;
    }
    return sengoo_runtime_argc - 1;
}

long long sengoo_arg_len(long long index) {
    const char* arg = sengoo_user_arg_at(index);
    if (!arg) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    return (long long)strlen(arg);
}

long long sengoo_arg_copy(long long index, long long out_buffer, long long out_capacity) {
    char* out = (char*)(intptr_t)out_buffer;
    if (out_capacity < 0) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }

    const char* arg = sengoo_user_arg_at(index);
    if (!arg) {
        return -SENGOO_STATUS_NOT_FOUND;
    }

    size_t len = strlen(arg);
    if ((unsigned long long)len > (unsigned long long)out_capacity || (len > 0 && !out)) {
        return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    }

    if (len > 0) {
        memcpy(out, arg, len);
    }
    return (long long)len;
}

#define SENGOO_ASSERT_ENVELOPE_MAX 65536

typedef enum {
    SENGOO_ASSERT_KIND_BOOL = 0,
    SENGOO_ASSERT_KIND_TRUE = 1,
    SENGOO_ASSERT_KIND_FALSE = 2,
    SENGOO_ASSERT_KIND_EQ_I64 = 3,
    SENGOO_ASSERT_KIND_NE_I64 = 4,
    SENGOO_ASSERT_KIND_EQ_BOOL = 5,
    SENGOO_ASSERT_KIND_NE_BOOL = 6,
    SENGOO_ASSERT_KIND_EQ_STR = 7,
    SENGOO_ASSERT_KIND_NE_STR = 8,
    SENGOO_ASSERT_KIND_EQ_F64 = 9,
    SENGOO_ASSERT_KIND_NE_F64 = 10,
} SengooAssertKind;

static void sengoo_assert_append_json_string(char* out, size_t out_cap, size_t* used, const char* value) {
    size_t pos = *used;
    if (pos >= out_cap) {
        return;
    }
    out[pos++] = '"';
    for (const unsigned char* cursor = (const unsigned char*)(value ? value : ""); *cursor && pos + 2 < out_cap; ++cursor) {
        unsigned char ch = *cursor;
        if (ch == '"' || ch == '\\') {
            if (pos + 2 >= out_cap) {
                break;
            }
            out[pos++] = '\\';
            out[pos++] = (char)ch;
            continue;
        }
        if (ch < 0x20) {
            if (pos + 6 >= out_cap) {
                break;
            }
            snprintf(out + pos, out_cap - pos, "\\u%04x", ch);
            pos += 6;
            continue;
        }
        out[pos++] = (char)ch;
    }
    if (pos < out_cap) {
        out[pos++] = '"';
        out[pos] = '\0';
        *used = pos;
    }
}

static void sengoo_assert_append_json_field_string(
    char* out,
    size_t out_cap,
    size_t* used,
    const char* key,
    const char* value
) {
    size_t pos = *used;
    if (pos >= out_cap) {
        return;
    }
    if (pos > 1) {
        out[pos++] = ',';
    }
    snprintf(out + pos, out_cap - pos, "\"%s\":", key);
    pos = strlen(out);
    sengoo_assert_append_json_string(out, out_cap, &pos, value);
    *used = pos;
}

static void sengoo_assert_append_json_field_u64(
    char* out,
    size_t out_cap,
    size_t* used,
    const char* key,
    unsigned long long value
) {
    size_t pos = *used;
    if (pos >= out_cap) {
        return;
    }
    if (pos > 1) {
        out[pos++] = ',';
    }
    snprintf(out + pos, out_cap - pos, "\"%s\":%llu", key, value);
    *used = strlen(out);
}

static void sengoo_assert_format_i64(long long value, char* out, size_t out_cap) {
    if (!out || out_cap == 0) {
        return;
    }
    snprintf(out, out_cap, "%lld", value);
}

static void sengoo_assert_format_bool(long long value, char* out, size_t out_cap) {
    if (!out || out_cap == 0) {
        return;
    }
    snprintf(out, out_cap, "%s", value ? "true" : "false");
}

static void sengoo_assert_format_f64(double value, char* out, size_t out_cap) {
    if (!out || out_cap == 0) {
        return;
    }
    snprintf(out, out_cap, "%g", value);
}

static const char* sengoo_assert_helper_name(SengooAssertKind kind) {
    switch (kind) {
        case SENGOO_ASSERT_KIND_BOOL:
            return "assert";
        case SENGOO_ASSERT_KIND_TRUE:
            return "assert_true";
        case SENGOO_ASSERT_KIND_FALSE:
            return "assert_false";
        case SENGOO_ASSERT_KIND_EQ_I64:
            return "assert_eq_i64";
        case SENGOO_ASSERT_KIND_NE_I64:
            return "assert_ne_i64";
        case SENGOO_ASSERT_KIND_EQ_BOOL:
            return "assert_eq_bool";
        case SENGOO_ASSERT_KIND_NE_BOOL:
            return "assert_ne_bool";
        case SENGOO_ASSERT_KIND_EQ_STR:
            return "assert_eq_str";
        case SENGOO_ASSERT_KIND_NE_STR:
            return "assert_ne_str";
        case SENGOO_ASSERT_KIND_EQ_F64:
            return "assert_eq_f64";
        case SENGOO_ASSERT_KIND_NE_F64:
            return "assert_ne_f64";
        default:
            return "assert";
    }
}

static void sengoo_assert_build_message(
    SengooAssertKind kind,
    long long i64_a,
    long long i64_b,
    const char* str_a,
    const char* str_b,
    double f64_a,
    double f64_b,
    char* message,
    size_t message_cap,
    char* expected,
    size_t expected_cap,
    char* actual,
    size_t actual_cap
) {
    if (!message || message_cap == 0) {
        return;
    }
    message[0] = '\0';
    if (expected && expected_cap > 0) {
        expected[0] = '\0';
    }
    if (actual && actual_cap > 0) {
        actual[0] = '\0';
    }

    switch (kind) {
        case SENGOO_ASSERT_KIND_BOOL:
            snprintf(message, message_cap, "assertion failed");
            break;
        case SENGOO_ASSERT_KIND_TRUE:
            snprintf(message, message_cap, "expected true, got false");
            break;
        case SENGOO_ASSERT_KIND_FALSE:
            snprintf(message, message_cap, "expected false, got true");
            break;
        case SENGOO_ASSERT_KIND_EQ_I64:
        case SENGOO_ASSERT_KIND_NE_I64:
            sengoo_assert_format_i64(i64_a, expected, expected_cap);
            sengoo_assert_format_i64(i64_b, actual, actual_cap);
            snprintf(message, message_cap, "expected %s, got %s", expected, actual);
            break;
        case SENGOO_ASSERT_KIND_EQ_BOOL:
        case SENGOO_ASSERT_KIND_NE_BOOL:
            sengoo_assert_format_bool(i64_a, expected, expected_cap);
            sengoo_assert_format_bool(i64_b, actual, actual_cap);
            snprintf(message, message_cap, "expected %s, got %s", expected, actual);
            break;
        case SENGOO_ASSERT_KIND_EQ_STR:
        case SENGOO_ASSERT_KIND_NE_STR:
            if (str_a) {
                strncpy(expected, str_a, expected_cap - 1);
                expected[expected_cap - 1] = '\0';
            }
            if (str_b) {
                strncpy(actual, str_b, actual_cap - 1);
                actual[actual_cap - 1] = '\0';
            }
            snprintf(message, message_cap, "expected %s, got %s", expected, actual);
            break;
        case SENGOO_ASSERT_KIND_EQ_F64:
        case SENGOO_ASSERT_KIND_NE_F64:
            sengoo_assert_format_f64(f64_a, expected, expected_cap);
            sengoo_assert_format_f64(f64_b, actual, actual_cap);
            snprintf(message, message_cap, "expected %s, got %s", expected, actual);
            break;
        default:
            snprintf(message, message_cap, "assertion failed");
            break;
    }
}

static int sengoo_assert_has_typed_operands(SengooAssertKind kind) {
    return kind >= SENGOO_ASSERT_KIND_EQ_I64;
}

static void sengoo_assert_write_envelope(
    const char* report_path,
    SengooAssertKind kind,
    const char* file,
    long long file_len,
    long long line,
    long long i64_a,
    long long i64_b,
    const char* str_a,
    const char* str_b,
    double f64_a,
    double f64_b
) {
    char message[256];
    char expected[128];
    char actual[128];
    const char* helper = sengoo_assert_helper_name(kind);
    sengoo_assert_build_message(
        kind,
        i64_a,
        i64_b,
        str_a,
        str_b,
        f64_a,
        f64_b,
        message,
        sizeof(message),
        expected,
        sizeof(expected),
        actual,
        sizeof(actual)
    );

    char envelope[SENGOO_ASSERT_ENVELOPE_MAX];
    size_t used = 0;
    envelope[0] = '{';
    used = 1;
    sengoo_assert_append_json_field_u64(envelope, sizeof(envelope), &used, "schema_version", 1);
    sengoo_assert_append_json_field_string(envelope, sizeof(envelope), &used, "kind", "assertion_failure");
    sengoo_assert_append_json_field_string(envelope, sizeof(envelope), &used, "helper", helper);
    sengoo_assert_append_json_field_string(envelope, sizeof(envelope), &used, "message", message);
    if (file && file_len > 0) {
        char file_buf[512];
        size_t copy_len = (size_t)file_len;
        if (copy_len >= sizeof(file_buf)) {
            copy_len = sizeof(file_buf) - 1;
        }
        memcpy(file_buf, file, copy_len);
        file_buf[copy_len] = '\0';
        sengoo_assert_append_json_field_string(envelope, sizeof(envelope), &used, "file", file_buf);
    }
    if (line > 0) {
        sengoo_assert_append_json_field_u64(envelope, sizeof(envelope), &used, "line", (unsigned long long)line);
    }
    if (sengoo_assert_has_typed_operands(kind)) {
        sengoo_assert_append_json_field_string(envelope, sizeof(envelope), &used, "expected", expected);
        sengoo_assert_append_json_field_string(envelope, sizeof(envelope), &used, "actual", actual);
    }
    if (used + 2 < sizeof(envelope)) {
        envelope[used++] = '}';
        envelope[used++] = '\n';
        envelope[used] = '\0';
    }

    FILE* out = fopen(report_path, "wb");
    if (out) {
        fwrite(envelope, 1, used, out);
        fclose(out);
    }
}

long long sengoo_assert_failure_v1(
    long long kind,
    long long file_ptr,
    long long file_len,
    long long line,
    long long i64_a,
    long long i64_b,
    long long str_a_ptr,
    long long str_b_ptr,
    double f64_a,
    double f64_b
) {
    const char* file = file_ptr ? (const char*)(intptr_t)file_ptr : NULL;
    const char* str_a = str_a_ptr ? (const char*)(intptr_t)str_a_ptr : NULL;
    const char* str_b = str_b_ptr ? (const char*)(intptr_t)str_b_ptr : NULL;
    const char* helper = sengoo_assert_helper_name((SengooAssertKind)kind);
    char message[256];
    char expected[128];
    char actual[128];
    sengoo_assert_build_message(
        (SengooAssertKind)kind,
        i64_a,
        i64_b,
        str_a,
        str_b,
        f64_a,
        f64_b,
        message,
        sizeof(message),
        expected,
        sizeof(expected),
        actual,
        sizeof(actual)
    );

    const char* report_path = getenv("SENGOO_ASSERT_REPORT");
    if (report_path && report_path[0] != '\0') {
        sengoo_assert_write_envelope(
            report_path,
            (SengooAssertKind)kind,
            file,
            file_len,
            line,
            i64_a,
            i64_b,
            str_a,
            str_b,
            f64_a,
            f64_b
        );
    } else {
        fprintf(stderr, "Assertion failed (%s): %s", helper, message);
        if (file && file_len > 0) {
            fprintf(stderr, " at %.*s:%lld", (int)file_len, file, line);
        }
        fprintf(stderr, "\n");
    }
    exit(1);
}

long long sengoo_panic_option_unwrap_i64(void) {
    fprintf(stderr, "Option unwrap failed\n");
    exit(1);
}

long long sengoo_panic_result_unwrap_i64(void) {
    fprintf(stderr, "Result unwrap failed\n");
    exit(1);
}

void* sengoo_alloc(long long size, long long align) {
    if (size < 0) {
        return NULL;
    }

    size_t alignment = (align <= 1) ? 1u : (size_t)align;
    if (alignment > 1) {
        size_t normalized = sizeof(void*);
        while (normalized < alignment && normalized <= (SIZE_MAX / 2)) {
            normalized <<= 1;
        }
        if (normalized < alignment) {
            return NULL;
        }
        alignment = normalized;
    }

    size_t bytes = (size_t)size;
    if (alignment <= 1) {
        return malloc(bytes);
    }

    if (bytes > SIZE_MAX - alignment - sizeof(void*)) {
        return NULL;
    }

    void* raw = malloc(bytes + alignment - 1 + sizeof(void*));
    if (raw == NULL) {
        return NULL;
    }

    uintptr_t start = (uintptr_t)raw + sizeof(void*);
    uintptr_t aligned = (start + (alignment - 1)) & ~(uintptr_t)(alignment - 1);
    ((void**)aligned)[-1] = raw;
    return (void*)aligned;
}

void sengoo_free(void* ptr, long long size, long long align) {
    (void)size;
    if (ptr == NULL) {
        return;
    }
    if (align <= 1) {
        free(ptr);
        return;
    }

    void* raw = ((void**)ptr)[-1];
    free(raw);
}

void* sengoo_realloc(void* ptr, long long old_size, long long old_align, long long new_size) {
    if (ptr == NULL) {
        return sengoo_alloc(new_size, old_align);
    }
    if (new_size < 0) {
        return NULL;
    }
    if (old_align <= 1) {
        return realloc(ptr, (size_t)new_size);
    }

    void* new_ptr = sengoo_alloc(new_size, old_align);
    if (new_ptr == NULL) {
        return NULL;
    }

    size_t copy_size = 0;
    if (old_size > 0) {
        copy_size = (size_t)old_size;
    }
    if ((size_t)new_size < copy_size) {
        copy_size = (size_t)new_size;
    }
    if (copy_size > 0) {
        memcpy(new_ptr, ptr, copy_size);
    }
    sengoo_free(ptr, old_size, old_align);
    return new_ptr;
}

long long sengoo_ptr_to_handle(void* ptr) {
    return (long long)(intptr_t)ptr;
}

void* sengoo_handle_to_ptr(long long handle) {
    return (void*)(intptr_t)handle;
}

typedef struct {
    long long* data;
    long long len;
    long long cap;
} SengooVecI64;

static SengooVecI64* sengoo_vec_from_handle(long long handle) {
    return (SengooVecI64*)sengoo_handle_to_ptr(handle);
}

static int sengoo_vec_reserve(SengooVecI64* vec, long long min_cap) {
    long long next = vec->cap;
    if (next <= 0) {
        next = 8;
    }
    while (next < min_cap) {
        if (next > (LLONG_MAX / 2)) {
            return 0;
        }
        next *= 2;
    }

    long long* resized = (long long*)realloc(vec->data, (size_t)next * sizeof(long long));
    if (!resized) {
        return 0;
    }
    vec->data = resized;
    vec->cap = next;
    return 1;
}

long long sengoo_vec_new_i64(void) {
    SengooVecI64* vec = (SengooVecI64*)malloc(sizeof(SengooVecI64));
    if (!vec) {
        return 0;
    }

    vec->len = 0;
    vec->cap = 8;
    vec->data = (long long*)malloc((size_t)vec->cap * sizeof(long long));
    if (!vec->data) {
        free(vec);
        return 0;
    }

    return sengoo_ptr_to_handle(vec);
}

void sengoo_vec_free_i64(long long handle) {
    SengooVecI64* vec = sengoo_vec_from_handle(handle);
    if (!vec) {
        return;
    }

    free(vec->data);
    vec->data = NULL;
    vec->len = 0;
    vec->cap = 0;
    free(vec);
}

long long sengoo_vec_len_i64(long long handle) {
    SengooVecI64* vec = sengoo_vec_from_handle(handle);
    if (!vec) {
        return 0;
    }
    return vec->len;
}

long long sengoo_vec_push_i64(long long handle, long long value) {
    SengooVecI64* vec = sengoo_vec_from_handle(handle);
    if (!vec) {
        return 0;
    }

    if (vec->len >= vec->cap && !sengoo_vec_reserve(vec, vec->len + 1)) {
        return 0;
    }

    vec->data[vec->len] = value;
    vec->len += 1;
    return 1;
}

long long sengoo_vec_get_i64(long long handle, long long index, long long* out_value) {
    SengooVecI64* vec = sengoo_vec_from_handle(handle);
    if (!vec || !out_value || index < 0 || index >= vec->len) {
        return 0;
    }

    *out_value = vec->data[index];
    return 1;
}

long long sengoo_vec_set_i64(long long handle, long long index, long long value) {
    SengooVecI64* vec = sengoo_vec_from_handle(handle);
    if (!vec || index < 0 || index >= vec->len) {
        return 0;
    }

    vec->data[index] = value;
    return 1;
}

long long sengoo_vec_pop_i64(long long handle, long long* out_value) {
    SengooVecI64* vec = sengoo_vec_from_handle(handle);
    if (!vec || vec->len == 0 || !out_value) {
        return 0;
    }

    vec->len -= 1;
    *out_value = vec->data[vec->len];
    return 1;
}

void sengoo_vec_clear_i64(long long handle) {
    SengooVecI64* vec = sengoo_vec_from_handle(handle);
    if (!vec) {
        return;
    }

    vec->len = 0;
}

long long sengoo_vec_contains_i64(long long handle, long long value) {
    SengooVecI64* vec = sengoo_vec_from_handle(handle);
    if (!vec) {
        return 0;
    }

    for (long long i = 0; i < vec->len; ++i) {
        if (vec->data[i] == value) {
            return 1;
        }
    }
    return 0;
}

long long sengoo_vec_remove_i64(long long handle, long long index, long long* out_value) {
    SengooVecI64* vec = sengoo_vec_from_handle(handle);
    if (!vec || !out_value || index < 0 || index >= vec->len) {
        return 0;
    }

    *out_value = vec->data[index];
    for (long long i = index + 1; i < vec->len; ++i) {
        vec->data[i - 1] = vec->data[i];
    }
    vec->len -= 1;
    return 1;
}

typedef struct {
    long long key;
    long long value;
    unsigned char state;
} SengooHashMapEntryI64;

typedef struct {
    SengooHashMapEntryI64* entries;
    long long len;
    long long used;
    long long cap;
} SengooHashMapI64;

static SengooHashMapI64* sengoo_hashmap_from_handle(long long handle) {
    return (SengooHashMapI64*)sengoo_handle_to_ptr(handle);
}

static uint64_t sengoo_hashmap_hash_i64(long long key) {
    uint64_t x = (uint64_t)key;
    x ^= x >> 33;
    x *= 0xff51afd7ed558ccdULL;
    x ^= x >> 33;
    x *= 0xc4ceb9fe1a85ec53ULL;
    x ^= x >> 33;
    return x;
}

static int sengoo_hashmap_alloc_entries(SengooHashMapI64* map, long long cap) {
    map->entries = (SengooHashMapEntryI64*)calloc((size_t)cap, sizeof(SengooHashMapEntryI64));
    if (!map->entries) {
        return 0;
    }
    map->cap = cap;
    map->len = 0;
    map->used = 0;
    return 1;
}

static long long sengoo_hashmap_find_slot(const SengooHashMapI64* map, long long key, int* found) {
    if (!map || map->cap <= 0) {
        if (found) {
            *found = 0;
        }
        return -1;
    }

    uint64_t mask = (uint64_t)(map->cap - 1);
    uint64_t start = sengoo_hashmap_hash_i64(key) & mask;
    long long first_tombstone = -1;

    for (uint64_t probe = 0; probe < (uint64_t)map->cap; ++probe) {
        long long index = (long long)((start + probe) & mask);
        const SengooHashMapEntryI64* entry = &map->entries[index];
        if (entry->state == 0) {
            if (found) {
                *found = 0;
            }
            return first_tombstone >= 0 ? first_tombstone : index;
        }
        if (entry->state == 2) {
            if (first_tombstone < 0) {
                first_tombstone = index;
            }
            continue;
        }
        if (entry->key == key) {
            if (found) {
                *found = 1;
            }
            return index;
        }
    }

    if (found) {
        *found = 0;
    }
    return first_tombstone;
}

static int sengoo_hashmap_rehash(SengooHashMapI64* map, long long new_cap) {
    SengooHashMapEntryI64* old_entries = map->entries;
    long long old_cap = map->cap;

    SengooHashMapEntryI64* new_entries = (SengooHashMapEntryI64*)calloc((size_t)new_cap, sizeof(SengooHashMapEntryI64));
    if (!new_entries) {
        return 0;
    }

    map->entries = new_entries;
    map->cap = new_cap;
    map->len = 0;
    map->used = 0;

    if (old_entries) {
        for (long long i = 0; i < old_cap; ++i) {
            if (old_entries[i].state != 1) {
                continue;
            }
            int found = 0;
            long long slot = sengoo_hashmap_find_slot(map, old_entries[i].key, &found);
            if (slot < 0) {
                free(new_entries);
                map->entries = old_entries;
                map->cap = old_cap;
                map->len = 0;
                map->used = 0;
                for (long long j = 0; j < old_cap; ++j) {
                    if (old_entries[j].state == 1) {
                        map->len += 1;
                        map->used += 1;
                    } else if (old_entries[j].state == 2) {
                        map->used += 1;
                    }
                }
                return 0;
            }
            map->entries[slot].key = old_entries[i].key;
            map->entries[slot].value = old_entries[i].value;
            map->entries[slot].state = 1;
            map->len += 1;
            map->used += 1;
        }
        free(old_entries);
    }

    return 1;
}

static int sengoo_hashmap_ensure_capacity(SengooHashMapI64* map) {
    if (!map) {
        return 0;
    }
    if (map->cap == 0) {
        return sengoo_hashmap_rehash(map, 8);
    }
    if ((map->used + 1) * 10 >= map->cap * 7) {
        if (map->cap > (LLONG_MAX / 2)) {
            return 0;
        }
        return sengoo_hashmap_rehash(map, map->cap * 2);
    }
    return 1;
}

long long sengoo_hashmap_new_i64(void) {
    SengooHashMapI64* map = (SengooHashMapI64*)malloc(sizeof(SengooHashMapI64));
    if (!map) {
        return 0;
    }

    map->entries = NULL;
    map->len = 0;
    map->used = 0;
    map->cap = 0;
    if (!sengoo_hashmap_alloc_entries(map, 8)) {
        free(map);
        return 0;
    }

    return sengoo_ptr_to_handle(map);
}

void sengoo_hashmap_free_i64(long long handle) {
    SengooHashMapI64* map = sengoo_hashmap_from_handle(handle);
    if (!map) {
        return;
    }

    free(map->entries);
    map->entries = NULL;
    map->len = 0;
    map->used = 0;
    map->cap = 0;
    free(map);
}

long long sengoo_hashmap_len_i64(long long handle) {
    SengooHashMapI64* map = sengoo_hashmap_from_handle(handle);
    if (!map) {
        return 0;
    }
    return map->len;
}

long long sengoo_hashmap_insert_i64(long long handle, long long key, long long value) {
    SengooHashMapI64* map = sengoo_hashmap_from_handle(handle);
    if (!map || !sengoo_hashmap_ensure_capacity(map)) {
        return 0;
    }

    int found = 0;
    long long slot = sengoo_hashmap_find_slot(map, key, &found);
    if (slot < 0) {
        return 0;
    }

    SengooHashMapEntryI64* entry = &map->entries[slot];
    if (found) {
        entry->value = value;
        return 1;
    }

    if (entry->state == 0) {
        map->used += 1;
    }
    entry->key = key;
    entry->value = value;
    entry->state = 1;
    map->len += 1;
    return 1;
}

long long sengoo_hashmap_get_i64(long long handle, long long key, long long* out_value) {
    SengooHashMapI64* map = sengoo_hashmap_from_handle(handle);
    if (!map || !out_value) {
        return 0;
    }

    int found = 0;
    long long slot = sengoo_hashmap_find_slot(map, key, &found);
    if (!found || slot < 0) {
        return 0;
    }

    *out_value = map->entries[slot].value;
    return 1;
}

long long sengoo_hashmap_contains_i64(long long handle, long long key) {
    SengooHashMapI64* map = sengoo_hashmap_from_handle(handle);
    if (!map) {
        return 0;
    }
    int found = 0;
    sengoo_hashmap_find_slot(map, key, &found);
    return found;
}

long long sengoo_hashmap_clear_i64(long long handle) {
    SengooHashMapI64* map = sengoo_hashmap_from_handle(handle);
    if (!map) {
        return 0;
    }

    if (map->entries && map->cap > 0) {
        memset(map->entries, 0, (size_t)map->cap * sizeof(SengooHashMapEntryI64));
    }
    map->len = 0;
    map->used = 0;
    return 1;
}

long long sengoo_hashmap_remove_i64(long long handle, long long key) {
    SengooHashMapI64* map = sengoo_hashmap_from_handle(handle);
    if (!map) {
        return 0;
    }

    int found = 0;
    long long slot = sengoo_hashmap_find_slot(map, key, &found);
    if (!found || slot < 0) {
        return 0;
    }

    map->entries[slot].state = 2;
    map->len -= 1;
    return 1;
}

typedef struct {
    SengooHashMapI64* map;
    long long index;
} SengooHashMapIterI64;

static SengooHashMapIterI64* sengoo_hashmap_iter_from_handle(long long handle) {
    return (SengooHashMapIterI64*)sengoo_handle_to_ptr(handle);
}

long long sengoo_hashmap_iter_new_i64(long long map_handle) {
    SengooHashMapI64* map = sengoo_hashmap_from_handle(map_handle);
    if (!map) {
        return 0;
    }

    SengooHashMapIterI64* iter = (SengooHashMapIterI64*)malloc(sizeof(SengooHashMapIterI64));
    if (!iter) {
        return 0;
    }

    iter->map = map;
    iter->index = 0;
    return sengoo_ptr_to_handle(iter);
}

void sengoo_hashmap_iter_free_i64(long long iter_handle) {
    SengooHashMapIterI64* iter = sengoo_hashmap_iter_from_handle(iter_handle);
    if (!iter) {
        return;
    }
    free(iter);
}

void sengoo_hashmap_iter_reset_i64(long long iter_handle) {
    SengooHashMapIterI64* iter = sengoo_hashmap_iter_from_handle(iter_handle);
    if (!iter) {
        return;
    }
    iter->index = 0;
}

long long sengoo_hashmap_iter_done_i64(long long iter_handle) {
    SengooHashMapIterI64* iter = sengoo_hashmap_iter_from_handle(iter_handle);
    if (!iter || !iter->map) {
        return 1;
    }

    while (iter->index < iter->map->cap) {
        if (iter->map->entries[iter->index].state == 1) {
            return 0;
        }
        iter->index += 1;
    }
    return 1;
}

long long sengoo_hashmap_iter_next_i64(long long iter_handle, long long* out_value) {
    SengooHashMapIterI64* iter = sengoo_hashmap_iter_from_handle(iter_handle);
    if (!iter || !iter->map || !out_value) {
        return 0;
    }

    while (iter->index < iter->map->cap) {
        SengooHashMapEntryI64* entry = &iter->map->entries[iter->index];
        iter->index += 1;
        if (entry->state == 1) {
            *out_value = entry->value;
            return 1;
        }
    }

    return 0;
}

long long sengoo_hashmap_iter_next_or_default_i64(long long iter_handle, long long fallback) {
    long long value = fallback;
    if (!sengoo_hashmap_iter_next_i64(iter_handle, &value)) {
        return fallback;
    }
    return value;
}

typedef struct {
    SengooVecI64* vec;
    long long index;
} SengooVecIterI64;

static SengooVecIterI64* sengoo_vec_iter_from_handle(long long handle) {
    return (SengooVecIterI64*)sengoo_handle_to_ptr(handle);
}

long long sengoo_vec_iter_new_i64(long long vec_handle) {
    SengooVecI64* vec = sengoo_vec_from_handle(vec_handle);
    if (!vec) {
        return 0;
    }

    SengooVecIterI64* iter = (SengooVecIterI64*)malloc(sizeof(SengooVecIterI64));
    if (!iter) {
        return 0;
    }

    iter->vec = vec;
    iter->index = 0;
    return sengoo_ptr_to_handle(iter);
}

void sengoo_vec_iter_free_i64(long long iter_handle) {
    SengooVecIterI64* iter = sengoo_vec_iter_from_handle(iter_handle);
    if (!iter) {
        return;
    }
    free(iter);
}

void sengoo_vec_iter_reset_i64(long long iter_handle) {
    SengooVecIterI64* iter = sengoo_vec_iter_from_handle(iter_handle);
    if (!iter) {
        return;
    }
    iter->index = 0;
}

long long sengoo_vec_iter_next_i64(long long iter_handle, long long* out_value) {
    SengooVecIterI64* iter = sengoo_vec_iter_from_handle(iter_handle);
    if (!iter || !iter->vec || !out_value) {
        return 0;
    }

    if (iter->index >= iter->vec->len) {
        return 0;
    }

    *out_value = iter->vec->data[iter->index];
    iter->index += 1;
    return 1;
}

long long sengoo_vec_iter_map_add_i64(long long iter_handle, long long addend, long long* out_value) {
    long long value = 0;
    if (!sengoo_vec_iter_next_i64(iter_handle, &value) || !out_value) {
        return 0;
    }

    *out_value = value + addend;
    return 1;
}

long long sengoo_vec_iter_filter_even_i64(long long iter_handle, long long* out_value) {
    if (!out_value) {
        return 0;
    }

    long long value = 0;
    while (sengoo_vec_iter_next_i64(iter_handle, &value)) {
        if ((value % 2) == 0) {
            *out_value = value;
            return 1;
        }
    }

    return 0;
}

typedef struct {
    int is_some;
    long long value;
} SengooOptionI64;

SengooOptionI64 sengoo_option_some_i64(long long value) {
    SengooOptionI64 option;
    option.is_some = 1;
    option.value = value;
    return option;
}

SengooOptionI64 sengoo_option_none_i64(void) {
    SengooOptionI64 option;
    option.is_some = 0;
    option.value = 0;
    return option;
}

long long sengoo_option_is_some_i64(SengooOptionI64 option) {
    return option.is_some;
}

long long sengoo_option_is_none_i64(SengooOptionI64 option) {
    return !option.is_some;
}

long long sengoo_option_unwrap_or_i64(SengooOptionI64 option, long long fallback) {
    if (option.is_some) {
        return option.value;
    }
    return fallback;
}

SengooOptionI64 sengoo_option_map_add_i64(SengooOptionI64 option, long long delta) {
    if (!option.is_some) {
        return option;
    }
    option.value += delta;
    return option;
}

SengooOptionI64 sengoo_option_and_then_mul_i64(SengooOptionI64 option, long long factor) {
    if (!option.is_some) {
        return option;
    }
    option.value *= factor;
    return option;
}

typedef struct {
    int is_ok;
    long long value;
    long long error;
} SengooResultI64;

SengooResultI64 sengoo_result_ok_i64(long long value) {
    SengooResultI64 result;
    result.is_ok = 1;
    result.value = value;
    result.error = 0;
    return result;
}

SengooResultI64 sengoo_result_err_i64(long long error) {
    SengooResultI64 result;
    result.is_ok = 0;
    result.value = 0;
    result.error = error;
    return result;
}

long long sengoo_result_is_ok_i64(SengooResultI64 result) {
    return result.is_ok;
}

long long sengoo_result_is_err_i64(SengooResultI64 result) {
    return !result.is_ok;
}

long long sengoo_result_unwrap_or_i64(SengooResultI64 result, long long fallback) {
    if (result.is_ok) {
        return result.value;
    }
    return fallback;
}

SengooResultI64 sengoo_result_map_add_i64(SengooResultI64 result, long long delta) {
    if (!result.is_ok) {
        return result;
    }
    result.value += delta;
    return result;
}

SengooResultI64 sengoo_result_and_then_mul_i64(SengooResultI64 result, long long factor) {
    if (!result.is_ok) {
        return result;
    }
    result.value *= factor;
    return result;
}

SengooResultI64 sengoo_result_map_err_add_i64(SengooResultI64 result, long long delta) {
    if (result.is_ok) {
        return result;
    }
    result.error += delta;
    return result;
}


long long sengoo_vec_get_or_default_i64(long long handle, long long index, long long fallback) {
    long long value = fallback;
    if (!sengoo_vec_get_i64(handle, index, &value)) {
        return fallback;
    }
    return value;
}

long long sengoo_vec_pop_or_default_i64(long long handle, long long fallback) {
    long long value = fallback;
    if (!sengoo_vec_pop_i64(handle, &value)) {
        return fallback;
    }
    return value;
}

long long sengoo_hashmap_get_or_default_i64(long long handle, long long key, long long fallback) {
    long long value = fallback;
    if (!sengoo_hashmap_get_i64(handle, key, &value)) {
        return fallback;
    }
    return value;
}

long long sengoo_vec_iter_done_i64(long long iter_handle) {
    SengooVecIterI64* iter = sengoo_vec_iter_from_handle(iter_handle);
    if (!iter || !iter->vec) {
        return 1;
    }
    return iter->index >= iter->vec->len;
}

long long sengoo_vec_iter_next_or_default_i64(long long iter_handle, long long fallback) {
    long long value = fallback;
    if (!sengoo_vec_iter_next_i64(iter_handle, &value)) {
        return fallback;
    }
    return value;
}


long long sengoo_vec_free_i64_status(long long handle) {
    sengoo_vec_free_i64(handle);
    return 1;
}

long long sengoo_hashmap_free_i64_status(long long handle) {
    sengoo_hashmap_free_i64(handle);
    return 1;
}

long long sengoo_vec_iter_free_i64_status(long long iter_handle) {
    sengoo_vec_iter_free_i64(iter_handle);
    return 1;
}

long long sengoo_vec_iter_reset_i64_status(long long iter_handle) {
    sengoo_vec_iter_reset_i64(iter_handle);
    return 1;
}


long long sengoo_vec_clear_i64_status(long long handle) {
    sengoo_vec_clear_i64(handle);
    return 1;
}

long long sengoo_hashmap_clear_i64_status(long long handle) {
    return sengoo_hashmap_clear_i64(handle);
}

long long sengoo_vec_remove_or_default_i64(long long handle, long long index, long long fallback) {
    long long value = fallback;
    if (!sengoo_vec_remove_i64(handle, index, &value)) {
        return fallback;
    }
    return value;
}


long long sengoo_hashmap_iter_free_i64_status(long long iter_handle) {
    sengoo_hashmap_iter_free_i64(iter_handle);
    return 1;
}

long long sengoo_hashmap_iter_reset_i64_status(long long iter_handle) {
    sengoo_hashmap_iter_reset_i64(iter_handle);
    return 1;
}

/* ========== Async Runtime: Frame-backed coroutine helpers ========== */

static long long* sengoo_async_frame_data(long long handle) {
    return handle == 0 ? NULL : (long long*)(intptr_t)handle;
}

static int sengoo_async_frame_access_is_valid(long long* frame, long long offset) {
    return frame != NULL && offset >= 0 && offset < frame[0];
}

static int sengoo_async_frame_guard_access(long long* frame, long long offset) {
#ifndef NDEBUG
    assert(frame != NULL && "async frame access requires a non-null compiler-managed handle");
    assert(offset >= 0 && "async frame access requires a non-negative slot offset");
    assert(offset < frame[0] && "async frame access offset is out of bounds");
#endif
    return sengoo_async_frame_access_is_valid(frame, offset);
}

long long sengoo_async_frame_alloc(long long slot_count) {
    if (slot_count < 0) {
        return 0;
    }
    if ((unsigned long long)slot_count > (SIZE_MAX / sizeof(long long)) - 1) {
        return 0;
    }
    long long* frame = (long long*)calloc((size_t)slot_count + 1, sizeof(long long));
    if (frame == NULL) {
        return 0;
    }
    frame[0] = slot_count;
    return (long long)(intptr_t)frame;
}

void sengoo_async_frame_free(long long handle) {
    long long* frame = sengoo_async_frame_data(handle);
#ifndef NDEBUG
    assert(frame != NULL && "async frame free requires a non-null compiler-managed handle");
#endif
    if (frame == NULL) {
        return;
    }
    free((void *)(intptr_t)frame);
}

/*
 * Async frame helpers are a compiler/runtime ABI and are not intended for
 * user-authored source calls.
 *
 * Contract summary:
 *   - alloc(slot_count): returns 0 on allocation/size failure
 *   - free(handle):
 *       debug   -> assert on null / invalid compiler-managed handle
 *       release -> null is ignored to preserve ABI-compatible behavior
 *   - store(handle, offset, value):
 *       debug   -> assert on invalid handle or offset
 *       release -> no-op on invalid access
 *   - load(handle, offset):
 *       debug   -> assert on invalid handle or offset
 *       release -> returns 0 on invalid access
 *
 * The release-path 0 fallback is ambiguous and must not be treated as a
 * reliable semantic value.
 */
void sengoo_async_frame_store(long long handle, long long offset, long long value) {
    long long* frame = sengoo_async_frame_data(handle);
    if (!sengoo_async_frame_guard_access(frame, offset)) {
        return;
    }
    frame[offset + 1] = value;
}

long long sengoo_async_frame_load(long long handle, long long offset) {
    long long* frame = sengoo_async_frame_data(handle);
    if (!sengoo_async_frame_guard_access(frame, offset)) {
        return 0;
    }
    return frame[offset + 1];
}

/*
 * Async program-side dispatch contract fallbacks.
 *
 * The native runtime staticlib references `main__*` and the
 * `sengoo_async_*_dispatch*` family, which async Sengoo programs define in
 * their compiled IR. Non-async programs never run the scheduler bridge, but
 * COMDAT-driven archive member extraction can still pull bridge objects that
 * carry those references, so the link needs benign defaults. Windows resolves
 * them through /alternatename (only used when the real symbol is absent);
 * POSIX uses weak definitions that any strong IR definition overrides.
 */
long long sengoo_async_fallback_dispatch_i64(long long kind, long long handle) {
    (void)kind;
    (void)handle;
    return 1;
}

long long sengoo_async_fallback_zero_i64(long long kind, long long handle) {
    (void)kind;
    (void)handle;
    return 0;
}

unsigned char sengoo_async_fallback_dispatch_bool(long long kind, long long handle) {
    (void)kind;
    (void)handle;
    return 0;
}

void sengoo_async_fallback_dispatch_void(long long kind, long long handle) {
    (void)kind;
    (void)handle;
}

signed char sengoo_async_fallback_dispatch_i8(long long kind, long long handle) {
    (void)kind;
    (void)handle;
    return 0;
}

short sengoo_async_fallback_dispatch_i16(long long kind, long long handle) {
    (void)kind;
    (void)handle;
    return 0;
}

int sengoo_async_fallback_dispatch_i32(long long kind, long long handle) {
    (void)kind;
    (void)handle;
    return 0;
}

float sengoo_async_fallback_dispatch_f32(long long kind, long long handle) {
    (void)kind;
    (void)handle;
    return 0.0f;
}

double sengoo_async_fallback_dispatch_f64(long long kind, long long handle) {
    (void)kind;
    (void)handle;
    return 0.0;
}

long long sengoo_async_fallback_main_start(void) {
    return 0;
}

long long sengoo_async_fallback_main_poll(long long handle) {
    (void)handle;
    return 1;
}

long long sengoo_async_fallback_main_result(long long handle) {
    (void)handle;
    return 0;
}

#if defined(_WIN32)
#pragma comment(linker, "/alternatename:main__start=sengoo_async_fallback_main_start")
#pragma comment(linker, "/alternatename:main__poll=sengoo_async_fallback_main_poll")
#pragma comment(linker, "/alternatename:main__result=sengoo_async_fallback_main_result")
#pragma comment(linker, "/alternatename:sengoo_async_poll_dispatch=sengoo_async_fallback_dispatch_i64")
#pragma comment(linker, "/alternatename:sengoo_async_cancel_dispatch=sengoo_async_fallback_dispatch_bool")
#pragma comment(linker, "/alternatename:sengoo_async_drop_dispatch=sengoo_async_fallback_dispatch_void")
#pragma comment(linker, "/alternatename:sengoo_async_result_dispatch_i8=sengoo_async_fallback_dispatch_i8")
#pragma comment(linker, "/alternatename:sengoo_async_result_dispatch_i16=sengoo_async_fallback_dispatch_i16")
#pragma comment(linker, "/alternatename:sengoo_async_result_dispatch_i32=sengoo_async_fallback_dispatch_i32")
#pragma comment(linker, "/alternatename:sengoo_async_result_dispatch_i64=sengoo_async_fallback_zero_i64")
#pragma comment(linker, "/alternatename:sengoo_async_result_dispatch_bool=sengoo_async_fallback_dispatch_bool")
#pragma comment(linker, "/alternatename:sengoo_async_result_dispatch_f32=sengoo_async_fallback_dispatch_f32")
#pragma comment(linker, "/alternatename:sengoo_async_result_dispatch_f64=sengoo_async_fallback_dispatch_f64")
#else
__attribute__((weak)) long long main__start(void) {
    return sengoo_async_fallback_main_start();
}

__attribute__((weak)) long long main__poll(long long handle) {
    return sengoo_async_fallback_main_poll(handle);
}

__attribute__((weak)) long long main__result(long long handle) {
    return sengoo_async_fallback_main_result(handle);
}

__attribute__((weak)) long long sengoo_async_poll_dispatch(long long kind, long long handle) {
    return sengoo_async_fallback_dispatch_i64(kind, handle);
}

__attribute__((weak)) unsigned char sengoo_async_cancel_dispatch(long long kind, long long handle) {
    return sengoo_async_fallback_dispatch_bool(kind, handle);
}

__attribute__((weak)) void sengoo_async_drop_dispatch(long long kind, long long handle) {
    sengoo_async_fallback_dispatch_void(kind, handle);
}

__attribute__((weak)) signed char sengoo_async_result_dispatch_i8(long long kind, long long handle) {
    return sengoo_async_fallback_dispatch_i8(kind, handle);
}

__attribute__((weak)) short sengoo_async_result_dispatch_i16(long long kind, long long handle) {
    return sengoo_async_fallback_dispatch_i16(kind, handle);
}

__attribute__((weak)) int sengoo_async_result_dispatch_i32(long long kind, long long handle) {
    return sengoo_async_fallback_dispatch_i32(kind, handle);
}

__attribute__((weak)) long long sengoo_async_result_dispatch_i64(long long kind, long long handle) {
    return sengoo_async_fallback_zero_i64(kind, handle);
}

__attribute__((weak)) unsigned char sengoo_async_result_dispatch_bool(long long kind, long long handle) {
    return sengoo_async_fallback_dispatch_bool(kind, handle);
}

__attribute__((weak)) float sengoo_async_result_dispatch_f32(long long kind, long long handle) {
    return sengoo_async_fallback_dispatch_f32(kind, handle);
}

__attribute__((weak)) double sengoo_async_result_dispatch_f64(long long kind, long long handle) {
    return sengoo_async_fallback_dispatch_f64(kind, handle);
}
#endif
