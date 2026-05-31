#define _CRT_SECURE_NO_WARNINGS

#include <assert.h>
#include <ctype.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#ifdef _WIN32
#include <direct.h>
#include <windows.h>
#else
#include <errno.h>
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
        return -1;
    }

    FILE* file = fopen(path, "rb");
    if (!file) {
        return -1;
    }
    if (fseek(file, 0, SEEK_END) != 0) {
        fclose(file);
        return -1;
    }
    long size = ftell(file);
    fclose(file);
    if (size < 0) {
        return -1;
    }
    return (long long)size;
}

long long sengoo_file_read_into(long long path_ptr, long long out_buffer, long long out_capacity) {
    const char* path = (const char*)(intptr_t)path_ptr;
    char* out = (char*)(intptr_t)out_buffer;
    if (!path || path[0] == '\0' || !out || out_capacity < 0) {
        return -1;
    }

    FILE* file = fopen(path, "rb");
    if (!file) {
        return -1;
    }
    size_t read = fread(out, 1, (size_t)out_capacity, file);
    int failed = ferror(file);
    fclose(file);
    if (failed) {
        return -1;
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
        return -1;
    }

    FILE* file = fopen(path, mode);
    if (!file) {
        return -1;
    }
    size_t expected = (size_t)len;
    size_t wrote = expected == 0 ? 0 : fwrite(data, 1, expected, file);
    int close_status = fclose(file);
    if (wrote != expected || close_status != 0) {
        return -1;
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

long long sengoo_env_var_len(long long name_ptr) {
    const char* name = (const char*)(intptr_t)name_ptr;
    if (!name || name[0] == '\0') {
        return -1;
    }

    const char* value = getenv(name);
    if (!value) {
        return -1;
    }
    return (long long)strlen(value);
}

long long sengoo_env_var_copy(long long name_ptr, long long out_buffer, long long out_capacity) {
    const char* name = (const char*)(intptr_t)name_ptr;
    char* out = (char*)(intptr_t)out_buffer;
    if (!name || name[0] == '\0' || out_capacity < 0) {
        return -1;
    }

    const char* value = getenv(name);
    if (!value) {
        return -1;
    }

    size_t len = strlen(value);
    if ((unsigned long long)len > (unsigned long long)out_capacity || (len > 0 && !out)) {
        return -1;
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
    if (out_capacity < 0 || (unsigned long long)len > (unsigned long long)out_capacity) {
        return -1;
    }
    if (len > 0 && (!data || !out)) {
        return -1;
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
        return -1;
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
        return -1;
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
        return -1;
    }

    size_t root_len = sengoo_path_root_len(path);
    size_t end = strlen(path);
    while (end > root_len && sengoo_path_is_sep(path[end - 1])) {
        end--;
    }
    if (end <= root_len) {
        return -1;
    }

    size_t pos = end;
    while (pos > root_len && !sengoo_path_is_sep(path[pos - 1])) {
        pos--;
    }
    if (pos == 0) {
        return -1;
    }

    size_t sep_index = pos - 1;
    size_t parent_len = sep_index < root_len ? root_len : sep_index;
    if (parent_len == 0 && root_len > 0) {
        parent_len = root_len;
    }
    if (parent_len == 0) {
        return -1;
    }

    return sengoo_copy_path_bytes(path, parent_len, out_buffer, out_capacity);
}

long long sengoo_path_file_name(long long path_ptr, long long out_buffer, long long out_capacity) {
    const char* path = (const char*)(intptr_t)path_ptr;
    size_t start = 0;
    size_t len = 0;
    if (!sengoo_path_file_name_range(path, &start, &len)) {
        return -1;
    }
    return sengoo_copy_path_bytes(path + start, len, out_buffer, out_capacity);
}

long long sengoo_path_stem(long long path_ptr, long long out_buffer, long long out_capacity) {
    const char* path = (const char*)(intptr_t)path_ptr;
    size_t start = 0;
    size_t len = 0;
    if (!sengoo_path_file_name_range(path, &start, &len)) {
        return -1;
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
        return -1;
    }

    size_t dot = start + len;
    while (dot > start && path[dot - 1] != '.') {
        dot--;
    }
    if (dot == start || dot == start + len) {
        return -1;
    }

    size_t dot_index = dot - 1;
    if (dot_index == start || dot_index + 1 >= start + len) {
        return -1;
    }
    return sengoo_copy_path_bytes(path + dot_index + 1, (start + len) - dot_index - 1, out_buffer, out_capacity);
}

static int sengoo_path_segment_is_dotdot(const char* segment, size_t len) {
    return len == 2 && segment[0] == '.' && segment[1] == '.';
}

long long sengoo_path_normalize(long long path_ptr, long long out_buffer, long long out_capacity) {
    const char* path = (const char*)(intptr_t)path_ptr;
    if (!path) {
        return -1;
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
        return -1;
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
        return -1;
    }

    size_t len = strlen(cwd);
    free(cwd);
    return (long long)len;
}

long long sengoo_process_current_dir_copy(long long out_buffer, long long out_capacity) {
    char* out = (char*)(intptr_t)out_buffer;
    if (out_capacity < 0) {
        return -1;
    }

    char* cwd = sengoo_process_current_dir_alloc();
    if (!cwd) {
        return -1;
    }

    size_t len = strlen(cwd);
    if ((unsigned long long)len > (unsigned long long)out_capacity || (len > 0 && !out)) {
        free(cwd);
        return -1;
    }

    if (len > 0) {
        memcpy(out, cwd, len);
    }
    free(cwd);
    return (long long)len;
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

static long long sengoo_ptr_to_handle(void* ptr) {
    return (long long)(intptr_t)ptr;
}

static void* sengoo_handle_to_ptr(long long handle) {
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

long long sengoo_async_run_main_i64(long long handle) {
    (void)handle;
    return handle;
}
