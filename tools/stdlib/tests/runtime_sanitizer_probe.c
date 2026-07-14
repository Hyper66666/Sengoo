#include "runtime_shared.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

long long sengoo_ffi_buffer_new(long long capacity);
long long sengoo_ffi_buffer_copy_in(long long handle, long long source, long long length);
long long sengoo_ffi_buffer_copy_out(long long handle, long long output, long long capacity);
long long sengoo_ffi_buffer_free(long long handle);
long long sengoo_string_from_bytes_copy(long long source, long long length);
long long sengoo_string_free_status(long long handle);
long long sengoo_json_parse_text(long long source, long long length);
long long sengoo_json_doc_close(long long handle);
long long sengoo_json_last_error_code(void);
long long sengoo_config_ini_parse(long long source, long long length);
long long sengoo_config_ini_free(long long handle);

static long long dropped_values = 0;

static void move_i64(void* destination, void* source) {
    memcpy(destination, source, sizeof(int64_t));
    memset(source, 0, sizeof(int64_t));
}

static void drop_i64(void* value) {
    if (value) {
        dropped_values += 1;
    }
}

int main(void) {
    const long long buffers_before = sengoo_buffer_live_handle_count();
    const long long strings_before = sengoo_string_live_handle_count();
    const long long opaque_before = sengoo_opaque_live_handle_count();
    const char payload[] = "sanitizer-payload";
    char output[sizeof(payload)] = {0};

    long long buffer = sengoo_ffi_buffer_new((long long)sizeof(payload));
    if (buffer <= 0) return 10;
    if (sengoo_ffi_buffer_copy_in(buffer, (long long)(intptr_t)payload,
                                  (long long)sizeof(payload)) != SENGOO_STATUS_OK) return 11;
    if (sengoo_ffi_buffer_copy_out(buffer, (long long)(intptr_t)output,
                                   (long long)sizeof(output)) != (long long)sizeof(payload)) return 12;
    if (memcmp(payload, output, sizeof(payload)) != 0) return 13;
    if (sengoo_ffi_buffer_copy_in(buffer, 0, 1) >= 0) return 14;
    if (sengoo_ffi_buffer_copy_out(buffer, 0, (long long)sizeof(output)) >= 0) return 15;
    if (sengoo_ffi_buffer_free(buffer) != SENGOO_STATUS_OK) return 16;
    if (sengoo_ffi_buffer_free(buffer) != SENGOO_STATUS_OK) return 17;

    long long string = sengoo_string_from_bytes_copy(
        (long long)(intptr_t)payload, (long long)(sizeof(payload) - 1));
    if (string <= 0 || sengoo_string_free_status(string) != SENGOO_STATUS_OK) return 20;
    if (sengoo_string_free_status(string) != SENGOO_STATUS_OK) return 21;

    const char json_text[] = "{\"value\":42}";
    long long json = sengoo_json_parse_text(
        (long long)(intptr_t)json_text, (long long)(sizeof(json_text) - 1));
    if (json <= 0 || sengoo_json_doc_close(json) != SENGOO_STATUS_OK) return 30;
    if (sengoo_json_parse_text(0, 1) != 0 || sengoo_json_last_error_code() == 0) return 31;
    if (sengoo_json_parse_text((long long)(intptr_t)json_text, -1) != 0 ||
        sengoo_json_last_error_code() == 0) return 32;

    const char config_text[] = "answer=42\n";
    long long config = sengoo_config_ini_parse(
        (long long)(intptr_t)config_text, (long long)(sizeof(config_text) - 1));
    if (config <= 0 || sengoo_config_ini_free(config) != SENGOO_STATUS_OK) return 40;
    if (sengoo_config_ini_parse(0, 1) >= 0) return 41;

    SengooTypeDescriptor descriptor = {
        SENGOO_COLLECTIONS_ABI_VERSION,
        0,
        sizeof(int64_t),
        _Alignof(int64_t),
        move_i64,
        drop_i64,
        NULL,
        NULL,
        NULL,
        NULL,
    };
    int64_t value = 42;
    long long vector = sengoo_raw_vec_new(&descriptor);
    if (vector <= 0 || sengoo_raw_vec_push(vector, &value) != SENGOO_STATUS_OK) return 50;
    if (sengoo_raw_vec_free(vector) != SENGOO_STATUS_OK || dropped_values != 1) return 51;
    if (sengoo_raw_vec_free(vector) != SENGOO_STATUS_INVALID_HANDLE) return 52;

    void* owned = malloc(8);
    if (!owned) return 60;
    long long opaque = sengoo_opaque_handle_new(owned);
    if (opaque <= 0 || sengoo_opaque_handle_get(opaque) != owned) return 61;
    void* taken = sengoo_opaque_handle_take(opaque);
    if (taken != owned || sengoo_opaque_handle_get(opaque) != NULL) return 62;
    free(taken);

    if (sengoo_buffer_live_handle_count() != buffers_before) return 70;
    if (sengoo_string_live_handle_count() != strings_before) return 71;
    if (sengoo_opaque_live_handle_count() != opaque_before) return 72;
    return 0;
}
