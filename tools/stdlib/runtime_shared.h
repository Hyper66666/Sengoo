#ifndef SENGOO_RUNTIME_SHARED_H
#define SENGOO_RUNTIME_SHARED_H

#include <stddef.h>
#include <stdint.h>

#define SENGOO_RUNTIME_ABI_VERSION 1
#define SENGOO_COLLECTIONS_ABI_VERSION 1

enum {
    SENGOO_RUNTIME_MAX_BUFFER_BYTES = 64 * 1024 * 1024,
    SENGOO_RUNTIME_MAX_COMMAND_OUTPUT_BYTES = 16 * 1024 * 1024,
    SENGOO_RUNTIME_MAX_JSON_BYTES = 1024 * 1024,
    SENGOO_RUNTIME_MAX_PATH_BYTES = 32 * 1024,
    SENGOO_RUNTIME_MAX_DIR_DEPTH = 64,
    SENGOO_RUNTIME_MAX_DIR_ENTRIES = 100000,

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
    SENGOO_STATUS_OUT_OF_MEMORY = 14,
    SENGOO_STATUS_TLS_CERT_INVALID = 15,
    SENGOO_STATUS_TLS_HOSTNAME_MISMATCH = 16,
    SENGOO_STATUS_TLS_HANDSHAKE = 17,
    SENGOO_STATUS_TLS_UNAVAILABLE = 18,
    SENGOO_STATUS_CANCELED = 19,
    SENGOO_STATUS_INVALID_UTF8 = 20
};

typedef struct {
    unsigned char* bytes;
    size_t capacity;
    size_t used_len;
} SengooFfiBuffer;

typedef void (*SengooMoveFn)(void* destination, void* source);
typedef void (*SengooDropFn)(void* value);
typedef int (*SengooCloneFn)(void* destination, const void* source);
typedef uint64_t (*SengooHashFn)(const void* value);
typedef long long (*SengooEqFn)(const void* left, const void* right);
typedef long long (*SengooCompareFn)(const void* left, const void* right);

typedef struct {
    uint32_t abi_version;
    uint32_t flags;
    size_t size;
    size_t align;
    SengooMoveFn move_value;
    SengooDropFn drop_value;
    SengooCloneFn clone_value;
    SengooHashFn hash_value;
    SengooEqFn eq_value;
    SengooCompareFn compare_value;
} SengooTypeDescriptor;

long long sengoo_runtime_abi_version(void);
long long sengoo_collections_abi_version(void);
long long sengoo_type_descriptor_validate(const SengooTypeDescriptor* descriptor);
long long sengoo_arc_new(const SengooTypeDescriptor* descriptor, void* value);
long long sengoo_arc_new_parts(
    void* value,
    long long size,
    long long align,
    SengooMoveFn move_value,
    SengooDropFn drop_value
);
long long sengoo_arc_clone(long long handle);
long long sengoo_arc_strong_count(long long handle);
void* sengoo_arc_borrow_ptr(long long handle);
long long sengoo_arc_drop(long long handle);
long long sengoo_raw_vec_new(const SengooTypeDescriptor* descriptor);
long long sengoo_raw_vec_new_parts(
    long long size,
    long long align,
    void* move_value,
    void* drop_value
);
long long sengoo_raw_vec_len(long long handle);
long long sengoo_raw_vec_push(long long handle, void* value);
void* sengoo_raw_vec_get(long long handle, long long index);
long long sengoo_raw_vec_set(long long handle, long long index, void* value);
long long sengoo_raw_vec_insert(long long handle, long long index, void* value);
long long sengoo_raw_vec_pop(long long handle, void* out_value);
long long sengoo_raw_vec_remove(long long handle, long long index, void* out_value);
long long sengoo_raw_vec_clear(long long handle);
long long sengoo_raw_vec_free(long long handle);
void sengoo_raw_zero_bytes(void* value, long long size);
long long sengoo_raw_vec_remove_string(long long handle, long long index);
long long sengoo_raw_vec_iter_new(long long handle);
void* sengoo_raw_vec_iter_next(long long handle);
long long sengoo_raw_vec_iter_free(long long handle);
long long sengoo_raw_hashmap_new_parts(
    long long key_size, long long key_align, void* key_move, void* key_drop,
    void* key_hash, void* key_eq, long long value_size, long long value_align,
    void* value_move, void* value_drop
);
long long sengoo_raw_hashmap_len(long long handle);
long long sengoo_raw_hashmap_insert(long long handle, void* key, void* value);
void* sengoo_raw_hashmap_get(long long handle, const void* key);
long long sengoo_raw_hashmap_contains(long long handle, const void* key);
long long sengoo_raw_hashmap_remove(long long handle, const void* key, void* out_value);
long long sengoo_raw_hashmap_clear(long long handle);
long long sengoo_raw_hashmap_free(long long handle);
long long sengoo_raw_hashmap_remove_string(long long handle, const void* key);
long long sengoo_raw_btreemap_new_parts(
    long long key_size, long long key_align, void* key_move, void* key_drop,
    void* key_compare, long long value_size, long long value_align,
    void* value_move, void* value_drop
);
long long sengoo_raw_map_key_iter_new(long long handle);
void* sengoo_raw_map_key_iter_next(long long handle);
long long sengoo_raw_map_key_iter_done(long long handle);
long long sengoo_raw_map_key_iter_reset(long long handle);
long long sengoo_raw_map_key_iter_index(long long handle);
long long sengoo_raw_map_key_iter_free(long long handle);

long long sengoo_ptr_to_handle(void* ptr);
void* sengoo_handle_to_ptr(long long handle);
long long sengoo_opaque_handle_new(void* ptr);
void* sengoo_opaque_handle_get(long long handle);
void* sengoo_opaque_handle_take(long long handle);
long long sengoo_opaque_live_handle_count(void);
SengooFfiBuffer* sengoo_ffi_buffer_from_handle(long long handle);
long long sengoo_copy_bytes_to_managed_buffer(long long buffer_handle, const char* bytes, size_t len);
long long sengoo_buffer_live_handle_count(void);
long long sengoo_string_live_handle_count(void);
long long sengoo_stream_cursor_from_buffer(long long buffer_handle);
long long sengoo_stream_cursor_with_capacity(long long capacity);
long long sengoo_stream_cursor_position(long long cursor_handle);
long long sengoo_stream_cursor_read(long long cursor_handle, long long out_buffer_handle);
long long sengoo_stream_cursor_write(
    long long cursor_handle,
    long long data_buffer_handle,
    long long used_len
);
long long sengoo_stream_cursor_free(long long cursor_handle);
long long sengoo_stream_cursor_live_handle_count(void);
char* sengoo_copy_cstr_from_handle(long long value_ptr);
char* sengoo_strdup_bytes(const char* value);
long long sengoo_time_unix_ms(void);
void sengoo_coverage_register(long long line);
void sengoo_coverage_hit(long long line);

#ifdef _WIN32
int sengoo_size_add(size_t* total, size_t value);
char* sengoo_windows_append_arg(char* out, const char* arg);
char* sengoo_windows_append_quoted_arg(char* out, const char* arg);
#endif

#endif
