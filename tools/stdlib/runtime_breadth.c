#define _CRT_SECURE_NO_WARNINGS

#include "runtime_shared.h"

#include <ctype.h>
#include <limits.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

extern long long sengoo_dir_entry_count(long long path_ptr);
extern long long sengoo_dir_entry_name(
    long long path_ptr,
    long long index,
    long long out_buffer,
    long long out_capacity
);

#define SENGOO_BREADTH_MAX_PATTERN 256
#define SENGOO_BREADTH_MAX_INPUT 65536
#define SENGOO_BREADTH_MAX_CAPTURES 16
#define SENGOO_BREADTH_MAX_GLOB_RESULTS 4096
#define SENGOO_BREADTH_MAX_CONFIG_BYTES 65536
#define SENGOO_BREADTH_MAX_LOG_BYTES 4096
#define SENGOO_BREADTH_REGEX_STEPS 65536

static int sengoo_breadth_last_error = SENGOO_STATUS_OK;

static long long sengoo_breadth_fail(long long code) {
    sengoo_breadth_last_error = (int)code;
    return -code;
}

long long sengoo_breadth_last_status(void) {
    return sengoo_breadth_last_error;
}

/* --- logging --- */

static int sengoo_log_level = 2; /* info */
static char sengoo_log_test_sink[SENGOO_BREADTH_MAX_LOG_BYTES];
static size_t sengoo_log_test_sink_len = 0;

long long sengoo_log_set_level(long long level) {
    if (level < 0 || level > 4) {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_ARGUMENT);
    }
    sengoo_log_level = (int)level;
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return 0;
}

long long sengoo_log_level_get(void) {
    return sengoo_log_level;
}

static void sengoo_log_append_test(const char* line) {
    size_t line_len = strlen(line);
    if (line_len >= SENGOO_BREADTH_MAX_LOG_BYTES) {
        return;
    }
    if (sengoo_log_test_sink_len + line_len + 2 > SENGOO_BREADTH_MAX_LOG_BYTES) {
        return;
    }
    if (sengoo_log_test_sink_len > 0) {
        sengoo_log_test_sink[sengoo_log_test_sink_len++] = '\n';
    }
    memcpy(sengoo_log_test_sink + sengoo_log_test_sink_len, line, line_len);
    sengoo_log_test_sink_len += line_len;
    sengoo_log_test_sink[sengoo_log_test_sink_len] = '\0';
}

long long sengoo_log_write(long long level, long long message_ptr, long long message_len) {
    const char* message = (const char*)(intptr_t)message_ptr;
    if (!message || message_len < 0 || message_len > SENGOO_BREADTH_MAX_INPUT) {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_ARGUMENT);
    }
    if (level < sengoo_log_level) {
        sengoo_breadth_last_error = SENGOO_STATUS_OK;
        return 0;
    }
    char line[512];
    size_t copy_len = (size_t)message_len;
    if (copy_len >= sizeof(line) - 16) {
        copy_len = sizeof(line) - 16;
    }
    snprintf(line, sizeof(line), "[%lld] %.*s", (long long)level, (int)copy_len, message);
    sengoo_log_append_test(line);
    fputs(line, stderr);
    fputc('\n', stderr);
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return 0;
}

long long sengoo_log_test_sink_clear(void) {
    sengoo_log_test_sink[0] = '\0';
    sengoo_log_test_sink_len = 0;
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return 0;
}

long long sengoo_log_test_sink_copy(long long buffer_handle, long long capacity) {
    return sengoo_copy_bytes_to_managed_buffer(
        buffer_handle,
        sengoo_log_test_sink,
        sengoo_log_test_sink_len
    ) >= 0
        ? (long long)sengoo_log_test_sink_len
        : sengoo_breadth_fail(SENGOO_STATUS_BUFFER_TOO_SMALL);
}

/* --- time format/parse (UTC subset) --- */

long long sengoo_time_format_unix_ms(long long unix_ms, long long buffer_handle, long long capacity) {
    if (unix_ms < 0) {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_ARGUMENT);
    }
    time_t seconds = (time_t)(unix_ms / 1000);
    struct tm tm_value;
#ifdef _WIN32
    if (gmtime_s(&tm_value, &seconds) != 0) {
        return sengoo_breadth_fail(SENGOO_STATUS_PARSE);
    }
#else
    if (!gmtime_r(&seconds, &tm_value)) {
        return sengoo_breadth_fail(SENGOO_STATUS_PARSE);
    }
#endif
    char scratch[32];
    if (strftime(scratch, sizeof(scratch), "%Y-%m-%dT%H:%M:%SZ", &tm_value) == 0) {
        return sengoo_breadth_fail(SENGOO_STATUS_PARSE);
    }
    size_t len = strlen(scratch);
    long long copied = sengoo_copy_bytes_to_managed_buffer(buffer_handle, scratch, len);
    if (copied < 0) {
        return sengoo_breadth_fail(SENGOO_STATUS_BUFFER_TOO_SMALL);
    }
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return (long long)len;
}

long long sengoo_time_parse_unix_ms(long long text_ptr, long long text_len) {
    const char* text = (const char*)(intptr_t)text_ptr;
    if (!text || text_len != 20) {
        return sengoo_breadth_fail(SENGOO_STATUS_PARSE);
    }
    int year = 0;
    int month = 0;
    int day = 0;
    int hour = 0;
    int minute = 0;
    int second = 0;
    if (sscanf(text, "%4d-%2d-%2dT%2d:%2d:%2dZ", &year, &month, &day, &hour, &minute, &second) != 6) {
        return sengoo_breadth_fail(SENGOO_STATUS_PARSE);
    }
    struct tm tm_value;
    memset(&tm_value, 0, sizeof(tm_value));
    tm_value.tm_year = year - 1900;
    tm_value.tm_mon = month - 1;
    tm_value.tm_mday = day;
    tm_value.tm_hour = hour;
    tm_value.tm_min = minute;
    tm_value.tm_sec = second;
#ifdef _WIN32
    time_t seconds = _mkgmtime(&tm_value);
#else
    time_t seconds = timegm(&tm_value);
#endif
    if (seconds < 0) {
        return sengoo_breadth_fail(SENGOO_STATUS_PARSE);
    }
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return (long long)seconds * 1000;
}

/* --- simple regex (bounded literal + . * ? only) --- */

typedef struct {
    char pattern[SENGOO_BREADTH_MAX_PATTERN + 1];
} SengooRegex;

static int sengoo_regex_match_at(const char* pattern, const char* text, size_t text_len, size_t* steps) {
    if (*pattern == '\0') {
        return 1;
    }
    if (*pattern == '$' && pattern[1] == '\0') {
        return 1;
    }
    if (*pattern == '^') {
        return sengoo_regex_match_at(pattern + 1, text, text_len, steps);
    }
    if (*steps >= SENGOO_BREADTH_REGEX_STEPS) {
        return -1;
    }
    (*steps)++;
    char c = *pattern;
    if (c == '.' && pattern[1] == '*') {
        for (size_t i = 0; i <= text_len; ++i) {
            int tail = sengoo_regex_match_at(pattern + 2, text + i, text_len - i, steps);
            if (tail == -1) {
                return -1;
            }
            if (tail) {
                return 1;
            }
        }
        return 0;
    }
    if (text_len == 0) {
        return pattern[0] == '\0' || (pattern[0] == '$' && pattern[1] == '\0');
    }
    if (c == '.') {
        return sengoo_regex_match_at(pattern + 1, text + 1, text_len - 1, steps);
    }
    if (c == '\\' && pattern[1] != '\0') {
        if (text[0] != pattern[1]) {
            return 0;
        }
        return sengoo_regex_match_at(pattern + 2, text + 1, text_len - 1, steps);
    }
    if (c != text[0]) {
        return 0;
    }
    return sengoo_regex_match_at(pattern + 1, text + 1, text_len - 1, steps);
}

long long sengoo_regex_compile(long long pattern_ptr, long long pattern_len) {
    const char* pattern = (const char*)(intptr_t)pattern_ptr;
    if (!pattern || pattern_len <= 0 || pattern_len > SENGOO_BREADTH_MAX_PATTERN) {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_ARGUMENT);
    }
    SengooRegex* regex = (SengooRegex*)calloc(1, sizeof(SengooRegex));
    if (!regex) {
        return sengoo_breadth_fail(SENGOO_STATUS_OUT_OF_MEMORY);
    }
    memcpy(regex->pattern, pattern, (size_t)pattern_len);
    regex->pattern[pattern_len] = '\0';
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return sengoo_ptr_to_handle(regex);
}

long long sengoo_regex_free(long long handle) {
    SengooRegex* regex = (SengooRegex*)sengoo_handle_to_ptr(handle);
    free(regex);
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return 0;
}

long long sengoo_regex_is_match(long long handle, long long text_ptr, long long text_len) {
    SengooRegex* regex = (SengooRegex*)sengoo_handle_to_ptr(handle);
    const char* text = (const char*)(intptr_t)text_ptr;
    if (!regex || !text || text_len < 0 || text_len > SENGOO_BREADTH_MAX_INPUT) {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_ARGUMENT);
    }
    size_t steps = 0;
    int matched = sengoo_regex_match_at(regex->pattern, text, (size_t)text_len, &steps);
    if (matched < 0) {
        return sengoo_breadth_fail(SENGOO_STATUS_UNSUPPORTED);
    }
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return matched ? 1 : 0;
}

/* --- glob (deterministic, no symlink follow) --- */

typedef struct {
    char** paths;
    size_t len;
    size_t cap;
} SengooGlobList;

static int sengoo_glob_push(SengooGlobList* list, const char* path) {
    if (list->len >= SENGOO_BREADTH_MAX_GLOB_RESULTS) {
        return -1;
    }
    if (list->len == list->cap) {
        size_t new_cap = list->cap == 0 ? 32 : list->cap * 2;
        char** next = (char**)realloc(list->paths, new_cap * sizeof(char*));
        if (!next) {
            return -1;
        }
        list->paths = next;
        list->cap = new_cap;
    }
    char* copy = sengoo_strdup_bytes(path);
    if (!copy) {
        return -1;
    }
    list->paths[list->len++] = copy;
    return 0;
}

static int sengoo_glob_match(const char* pattern, const char* value) {
    size_t p = 0;
    size_t v = 0;
    size_t star_p = (size_t)-1;
    size_t star_v = 0;
    while (v < strlen(value) || p < strlen(pattern)) {
        if (p < strlen(pattern) && (pattern[p] == '?' || pattern[p] == value[v])) {
            p++;
            v++;
            continue;
        }
        if (p < strlen(pattern) && pattern[p] == '*') {
            star_p = p++;
            star_v = v;
            continue;
        }
        if (star_p == (size_t)-1) {
            return 0;
        }
        p = star_p + 1;
        v = ++star_v;
    }
    while (p < strlen(pattern) && pattern[p] == '*') {
        p++;
    }
    return p == strlen(pattern);
}

static int sengoo_glob_name_compare(const void* lhs, const void* rhs) {
    const char* const* a = (const char* const*)lhs;
    const char* const* b = (const char* const*)rhs;
    return strcmp(*a, *b);
}

static void sengoo_glob_list_free(SengooGlobList* list) {
    if (!list) {
        return;
    }
    for (size_t i = 0; i < list->len; ++i) {
        free(list->paths[i]);
    }
    free(list->paths);
    list->paths = NULL;
    list->len = 0;
    list->cap = 0;
}

long long sengoo_fs_glob_collect(long long root_ptr, long long pattern_ptr) {
    const char* root = (const char*)(intptr_t)root_ptr;
    const char* pattern = (const char*)(intptr_t)pattern_ptr;
    if (!root || !pattern || pattern[0] == '\0') {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_ARGUMENT);
    }
    long long count = sengoo_dir_entry_count((long long)(intptr_t)root);
    if (count < 0) {
        return count;
    }
    SengooGlobList* list = (SengooGlobList*)calloc(1, sizeof(SengooGlobList));
    if (!list) {
        return sengoo_breadth_fail(SENGOO_STATUS_OUT_OF_MEMORY);
    }
    for (long long i = 0; i < count; ++i) {
        char name[512];
        long long copied = sengoo_dir_entry_name(
            (long long)(intptr_t)root,
            i,
            (long long)(intptr_t)name,
            (long long)sizeof(name)
        );
        if (copied < 0) {
            continue;
        }
        name[copied] = '\0';
        if (sengoo_glob_match(pattern, name) && sengoo_glob_push(list, name) != 0) {
            sengoo_glob_list_free(list);
            free(list);
            return sengoo_breadth_fail(SENGOO_STATUS_OUT_OF_MEMORY);
        }
    }
    if (list->len > 1) {
        qsort(list->paths, list->len, sizeof(char*), sengoo_glob_name_compare);
    }
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return sengoo_ptr_to_handle(list);
}

long long sengoo_fs_glob_count(long long handle) {
    SengooGlobList* list = (SengooGlobList*)sengoo_handle_to_ptr(handle);
    if (!list) {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_HANDLE);
    }
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return (long long)list->len;
}

long long sengoo_fs_glob_copy(long long handle, long long index, long long buffer_handle, long long capacity) {
    SengooGlobList* list = (SengooGlobList*)sengoo_handle_to_ptr(handle);
    if (!list || index < 0 || (size_t)index >= list->len) {
        return sengoo_breadth_fail(SENGOO_STATUS_NOT_FOUND);
    }
    size_t len = strlen(list->paths[(size_t)index]);
    long long copied = sengoo_copy_bytes_to_managed_buffer(buffer_handle, list->paths[(size_t)index], len);
    if (copied < 0) {
        return sengoo_breadth_fail(SENGOO_STATUS_BUFFER_TOO_SMALL);
    }
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return (long long)len;
}

long long sengoo_fs_glob_free(long long handle) {
    SengooGlobList* list = (SengooGlobList*)sengoo_handle_to_ptr(handle);
    sengoo_glob_list_free(list);
    free(list);
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return 0;
}

long long sengoo_fs_watch_supported(void) {
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return 0;
}

/* --- SHA-256 / hex / base64 (minimal, no external deps) --- */

static const uint32_t sengoo_sha256_k[64] = {
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
};

static uint32_t sengoo_sha256_rotr(uint32_t value, uint32_t bits) {
    return (value >> bits) | (value << (32 - bits));
}

static void sengoo_sha256_transform(uint32_t state[8], const uint8_t block[64]) {
    uint32_t w[64];
    for (int i = 0; i < 16; ++i) {
        w[i] = ((uint32_t)block[i * 4] << 24) | ((uint32_t)block[i * 4 + 1] << 16) |
               ((uint32_t)block[i * 4 + 2] << 8) | (uint32_t)block[i * 4 + 3];
    }
    for (int i = 16; i < 64; ++i) {
        uint32_t s0 = sengoo_sha256_rotr(w[i - 15], 7) ^ sengoo_sha256_rotr(w[i - 15], 18) ^ (w[i - 15] >> 3);
        uint32_t s1 = sengoo_sha256_rotr(w[i - 2], 17) ^ sengoo_sha256_rotr(w[i - 2], 19) ^ (w[i - 2] >> 10);
        w[i] = w[i - 16] + s0 + w[i - 7] + s1;
    }
    uint32_t a = state[0];
    uint32_t b = state[1];
    uint32_t c = state[2];
    uint32_t d = state[3];
    uint32_t e = state[4];
    uint32_t f = state[5];
    uint32_t g = state[6];
    uint32_t h = state[7];
    for (int i = 0; i < 64; ++i) {
        uint32_t s1 = sengoo_sha256_rotr(e, 6) ^ sengoo_sha256_rotr(e, 11) ^ sengoo_sha256_rotr(e, 25);
        uint32_t ch = (e & f) ^ ((~e) & g);
        uint32_t temp1 = h + s1 + ch + sengoo_sha256_k[i] + w[i];
        uint32_t s0 = sengoo_sha256_rotr(a, 2) ^ sengoo_sha256_rotr(a, 13) ^ sengoo_sha256_rotr(a, 22);
        uint32_t maj = (a & b) ^ (a & c) ^ (b & c);
        uint32_t temp2 = s0 + maj;
        h = g;
        g = f;
        f = e;
        e = d + temp1;
        d = c;
        c = b;
        b = a;
        a = temp1 + temp2;
    }
    state[0] += a;
    state[1] += b;
    state[2] += c;
    state[3] += d;
    state[4] += e;
    state[5] += f;
    state[6] += g;
    state[7] += h;
}

long long sengoo_hash_sha256(
    long long data_ptr,
    long long data_len,
    long long out_buffer,
    long long out_capacity
) {
    const uint8_t* data = (const uint8_t*)(intptr_t)data_ptr;
    if (!data || data_len < 0 || data_len > SENGOO_BREADTH_MAX_INPUT) {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_ARGUMENT);
    }
    uint32_t state[8] = {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                         0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};
    uint8_t block[64];
    size_t offset = 0;
    unsigned long long total_bits = (unsigned long long)data_len * 8ULL;
    while (offset + 64 <= (size_t)data_len) {
        memcpy(block, data + offset, 64);
        sengoo_sha256_transform(state, block);
        offset += 64;
    }
    size_t rem = (size_t)data_len - offset;
    memset(block, 0, sizeof(block));
    if (rem > 0) {
        memcpy(block, data + offset, rem);
    }
    block[rem] = 0x80;
    if (rem >= 56) {
        sengoo_sha256_transform(state, block);
        memset(block, 0, sizeof(block));
    }
    for (int i = 0; i < 8; ++i) {
        block[63 - i] = (uint8_t)(total_bits >> (i * 8));
    }
    sengoo_sha256_transform(state, block);
    char hex[65];
    for (int i = 0; i < 8; ++i) {
        snprintf(hex + i * 8, 9, "%08x", state[i]);
    }
    long long copied = sengoo_copy_bytes_to_managed_buffer(out_buffer, hex, 64);
    if (copied < 0) {
        return sengoo_breadth_fail(SENGOO_STATUS_BUFFER_TOO_SMALL);
    }
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return 64;
}

static const char sengoo_base64_table[] =
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

long long sengoo_encoding_base64_encode(
    long long data_ptr,
    long long data_len,
    long long out_buffer,
    long long out_capacity
) {
    const uint8_t* data = (const uint8_t*)(intptr_t)data_ptr;
    if (!data || data_len < 0 || data_len > SENGOO_BREADTH_MAX_INPUT) {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_ARGUMENT);
    }
    size_t out_len = ((size_t)data_len + 2) / 3 * 4;
    char* scratch = (char*)malloc(out_len + 1);
    if (!scratch) {
        return sengoo_breadth_fail(SENGOO_STATUS_OUT_OF_MEMORY);
    }
    size_t j = 0;
    for (size_t i = 0; i < (size_t)data_len; i += 3) {
        uint32_t triple = (uint32_t)data[i] << 16;
        if (i + 1 < (size_t)data_len) {
            triple |= (uint32_t)data[i + 1] << 8;
        }
        if (i + 2 < (size_t)data_len) {
            triple |= (uint32_t)data[i + 2];
        }
        scratch[j++] = sengoo_base64_table[(triple >> 18) & 0x3f];
        scratch[j++] = sengoo_base64_table[(triple >> 12) & 0x3f];
        scratch[j++] = (i + 1 < (size_t)data_len) ? sengoo_base64_table[(triple >> 6) & 0x3f] : '=';
        scratch[j++] = (i + 2 < (size_t)data_len) ? sengoo_base64_table[triple & 0x3f] : '=';
    }
    long long copied = sengoo_copy_bytes_to_managed_buffer(out_buffer, scratch, j);
    free(scratch);
    if (copied < 0) {
        return sengoo_breadth_fail(SENGOO_STATUS_BUFFER_TOO_SMALL);
    }
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return (long long)j;
}

long long sengoo_encoding_hex_encode(
    long long data_ptr,
    long long data_len,
    long long out_buffer,
    long long out_capacity
) {
    const uint8_t* data = (const uint8_t*)(intptr_t)data_ptr;
    if (!data || data_len < 0 || data_len > SENGOO_BREADTH_MAX_INPUT) {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_ARGUMENT);
    }
    if ((size_t)data_len * 2 > (size_t)out_capacity) {
        return sengoo_breadth_fail(SENGOO_STATUS_BUFFER_TOO_SMALL);
    }
    static const char* digits = "0123456789abcdef";
    char* scratch = (char*)malloc((size_t)data_len * 2 + 1);
    if (!scratch) {
        return sengoo_breadth_fail(SENGOO_STATUS_OUT_OF_MEMORY);
    }
    for (size_t i = 0; i < (size_t)data_len; ++i) {
        scratch[i * 2] = digits[(data[i] >> 4) & 0xf];
        scratch[i * 2 + 1] = digits[data[i] & 0xf];
    }
    long long copied = sengoo_copy_bytes_to_managed_buffer(out_buffer, scratch, (size_t)data_len * 2);
    free(scratch);
    if (copied < 0) {
        return sengoo_breadth_fail(SENGOO_STATUS_BUFFER_TOO_SMALL);
    }
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return (long long)data_len * 2;
}

long long sengoo_compress_gzip(long long data_ptr, long long data_len, long long out_buffer, long long out_capacity) {
    (void)data_ptr;
    (void)data_len;
    (void)out_buffer;
    (void)out_capacity;
    return sengoo_breadth_fail(SENGOO_STATUS_UNSUPPORTED);
}

long long sengoo_compress_gunzip(long long data_ptr, long long data_len, long long out_buffer, long long out_capacity) {
    (void)data_ptr;
    (void)data_len;
    (void)out_buffer;
    (void)out_capacity;
    return sengoo_breadth_fail(SENGOO_STATUS_UNSUPPORTED);
}

/* --- INI subset --- */

typedef struct {
    char** keys;
    char** values;
    size_t len;
} SengooIniDoc;

static void sengoo_ini_free(SengooIniDoc* doc) {
    if (!doc) {
        return;
    }
    for (size_t i = 0; i < doc->len; ++i) {
        free(doc->keys[i]);
        free(doc->values[i]);
    }
    free(doc->keys);
    free(doc->values);
    doc->keys = NULL;
    doc->values = NULL;
    doc->len = 0;
}

long long sengoo_config_ini_parse(long long text_ptr, long long text_len) {
    const char* text = (const char*)(intptr_t)text_ptr;
    if (!text || text_len < 0 || text_len > SENGOO_BREADTH_MAX_CONFIG_BYTES) {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_ARGUMENT);
    }
    SengooIniDoc* doc = (SengooIniDoc*)calloc(1, sizeof(SengooIniDoc));
    if (!doc) {
        return sengoo_breadth_fail(SENGOO_STATUS_OUT_OF_MEMORY);
    }
    char* copy = (char*)malloc((size_t)text_len + 1);
    if (!copy) {
        free(doc);
        return sengoo_breadth_fail(SENGOO_STATUS_OUT_OF_MEMORY);
    }
    memcpy(copy, text, (size_t)text_len);
    copy[text_len] = '\0';
    char* line = copy;
    while (line && *line) {
        char* next = strchr(line, '\n');
        if (next) {
            *next = '\0';
            next++;
        }
        while (*line == ' ' || *line == '\t') {
            line++;
        }
        if (*line == '\0' || *line == ';' || *line == '#') {
            line = next;
            continue;
        }
        char* eq = strchr(line, '=');
        if (eq) {
            *eq = '\0';
            char* key = line;
            char* value = eq + 1;
            char** keys = (char**)realloc(doc->keys, (doc->len + 1) * sizeof(char*));
            char** values = (char**)realloc(doc->values, (doc->len + 1) * sizeof(char*));
            if (!keys || !values) {
                free(copy);
                sengoo_ini_free(doc);
                free(doc);
                return sengoo_breadth_fail(SENGOO_STATUS_OUT_OF_MEMORY);
            }
            doc->keys = keys;
            doc->values = values;
            doc->keys[doc->len] = sengoo_strdup_bytes(key);
            doc->values[doc->len] = sengoo_strdup_bytes(value);
            if (!doc->keys[doc->len] || !doc->values[doc->len]) {
                free(copy);
                sengoo_ini_free(doc);
                free(doc);
                return sengoo_breadth_fail(SENGOO_STATUS_OUT_OF_MEMORY);
            }
            doc->len++;
        }
        line = next;
    }
    free(copy);
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return sengoo_ptr_to_handle(doc);
}

long long sengoo_config_ini_get(long long handle, long long key_ptr, long long value_buffer, long long capacity) {
    SengooIniDoc* doc = (SengooIniDoc*)sengoo_handle_to_ptr(handle);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!doc || !key) {
        return sengoo_breadth_fail(SENGOO_STATUS_INVALID_ARGUMENT);
    }
    for (size_t i = 0; i < doc->len; ++i) {
        if (strcmp(doc->keys[i], key) == 0) {
            size_t len = strlen(doc->values[i]);
            long long copied = sengoo_copy_bytes_to_managed_buffer(value_buffer, doc->values[i], len);
            if (copied < 0) {
                return sengoo_breadth_fail(SENGOO_STATUS_BUFFER_TOO_SMALL);
            }
            sengoo_breadth_last_error = SENGOO_STATUS_OK;
            return (long long)len;
        }
    }
    return sengoo_breadth_fail(SENGOO_STATUS_NOT_FOUND);
}

long long sengoo_config_ini_free(long long handle) {
    SengooIniDoc* doc = (SengooIniDoc*)sengoo_handle_to_ptr(handle);
    sengoo_ini_free(doc);
    free(doc);
    sengoo_breadth_last_error = SENGOO_STATUS_OK;
    return 0;
}

long long sengoo_config_toml_parse(long long text_ptr, long long text_len) {
    return sengoo_config_ini_parse(text_ptr, text_len);
}

long long sengoo_config_toml_get(long long handle, long long key_ptr, long long value_buffer, long long capacity) {
    return sengoo_config_ini_get(handle, key_ptr, value_buffer, capacity);
}

long long sengoo_config_toml_free(long long handle) {
    return sengoo_config_ini_free(handle);
}

/* --- Network fallback ABI for native stdlib builds --- */

enum {
    SENGOO_NET_ERR_OK = 0,
    SENGOO_NET_ERR_INVALID_ARGUMENT = 1,
    SENGOO_NET_ERR_INVALID_URL = 2,
    SENGOO_NET_ERR_UNSUPPORTED_SCHEME = 3,
    SENGOO_NET_ERR_RESOLVE_FAILED = 4,
    SENGOO_NET_ERR_CONNECT_FAILED = 5,
    SENGOO_NET_ERR_IO = 6,
    SENGOO_NET_ERR_TIMEOUT = 7,
    SENGOO_NET_ERR_HTTP_PROTOCOL = 8,
    SENGOO_NET_ERR_HTTP_CHUNKED = 9,
    SENGOO_NET_ERR_WS_HANDSHAKE = 10,
    SENGOO_NET_ERR_WS_PROTOCOL = 11,
    SENGOO_NET_ERR_HANDLE_NOT_FOUND = 12,
    SENGOO_NET_ERR_INTERNAL = 13,
    SENGOO_NET_ERR_REMOTE_CLOSED = 14
};

static int sengoo_net_fallback_last_error = SENGOO_NET_ERR_OK;
static int sengoo_net_bench_fallback_last_error = 0;

static long long sengoo_net_fallback_handle_error(int code) {
    sengoo_net_fallback_last_error = code;
    return 0;
}

static long long sengoo_net_fallback_i64_error(int code) {
    sengoo_net_fallback_last_error = code;
    return -1;
}

static long long sengoo_net_fallback_bool_error(int code) {
    sengoo_net_fallback_last_error = code;
    return 0;
}

static const char* sengoo_net_fallback_error_name(long long code) {
    switch (code) {
        case SENGOO_NET_ERR_OK: return "ok";
        case SENGOO_NET_ERR_INVALID_ARGUMENT: return "invalid_argument";
        case SENGOO_NET_ERR_INVALID_URL: return "invalid_url";
        case SENGOO_NET_ERR_UNSUPPORTED_SCHEME: return "unsupported_scheme";
        case SENGOO_NET_ERR_RESOLVE_FAILED: return "resolve_failed";
        case SENGOO_NET_ERR_CONNECT_FAILED: return "connect_failed";
        case SENGOO_NET_ERR_IO: return "io_error";
        case SENGOO_NET_ERR_TIMEOUT: return "timeout";
        case SENGOO_NET_ERR_HTTP_PROTOCOL: return "http_protocol_error";
        case SENGOO_NET_ERR_HTTP_CHUNKED: return "http_chunk_decode_error";
        case SENGOO_NET_ERR_WS_HANDSHAKE: return "websocket_handshake_error";
        case SENGOO_NET_ERR_WS_PROTOCOL: return "websocket_protocol_error";
        case SENGOO_NET_ERR_HANDLE_NOT_FOUND: return "handle_not_found";
        case SENGOO_NET_ERR_INTERNAL: return "internal_error";
        case SENGOO_NET_ERR_REMOTE_CLOSED: return "remote_closed";
        default: return "unknown_error";
    }
}

static long long sengoo_copy_to_raw_buffer(const char* text, long long buffer, long long capacity) {
    if (!text || buffer == 0 || capacity < 0) {
        return -1;
    }
    size_t len = strlen(text);
    size_t copy_len = len < (size_t)capacity ? len : (size_t)capacity;
    if (copy_len > 0) {
        memcpy((char*)(intptr_t)buffer, text, copy_len);
    }
    return (long long)copy_len;
}

long long sengoo_net_last_error(void) {
    return (long long)sengoo_net_fallback_last_error;
}

void sengoo_net_clear_error(void) {
    sengoo_net_fallback_last_error = SENGOO_NET_ERR_OK;
}

long long sengoo_net_error_name_copy(long long code, long long buffer, long long capacity) {
    return sengoo_copy_to_raw_buffer(sengoo_net_fallback_error_name(code), buffer, capacity);
}

long long sengoo_tcp_connect(long long host, long long port, long long timeout_ms) {
    (void)host;
    (void)port;
    (void)timeout_ms;
    return sengoo_net_fallback_handle_error(SENGOO_NET_ERR_UNSUPPORTED_SCHEME);
}

long long sengoo_tcp_send(long long handle, long long data, long long len) {
    (void)handle;
    (void)data;
    (void)len;
    return sengoo_net_fallback_i64_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_tcp_recv(long long handle, long long buffer, long long capacity, long long timeout_ms) {
    (void)handle;
    (void)buffer;
    (void)capacity;
    (void)timeout_ms;
    return sengoo_net_fallback_i64_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_tcp_close(long long handle) {
    (void)handle;
    return sengoo_net_fallback_bool_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_udp_bind(long long host, long long port) {
    (void)host;
    (void)port;
    return sengoo_net_fallback_handle_error(SENGOO_NET_ERR_UNSUPPORTED_SCHEME);
}

long long sengoo_udp_connect(long long handle, long long host, long long port) {
    (void)handle;
    (void)host;
    (void)port;
    return sengoo_net_fallback_bool_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_udp_send(long long handle, long long data, long long len) {
    (void)handle;
    (void)data;
    (void)len;
    return sengoo_net_fallback_i64_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_udp_recv(long long handle, long long buffer, long long capacity, long long timeout_ms) {
    (void)handle;
    (void)buffer;
    (void)capacity;
    (void)timeout_ms;
    return sengoo_net_fallback_i64_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_udp_close(long long handle) {
    (void)handle;
    return sengoo_net_fallback_bool_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_http_get(long long url, long long timeout_ms) {
    (void)url;
    (void)timeout_ms;
    return sengoo_net_fallback_handle_error(SENGOO_NET_ERR_UNSUPPORTED_SCHEME);
}

long long sengoo_http_post(long long url, long long body, long long len, long long timeout_ms) {
    (void)url;
    (void)body;
    (void)len;
    (void)timeout_ms;
    return sengoo_net_fallback_handle_error(SENGOO_NET_ERR_UNSUPPORTED_SCHEME);
}

long long sengoo_http_status(long long handle) {
    (void)handle;
    return sengoo_net_fallback_i64_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_http_body_len(long long handle) {
    (void)handle;
    return sengoo_net_fallback_i64_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_http_body_copy(long long handle, long long buffer, long long capacity) {
    (void)handle;
    (void)buffer;
    (void)capacity;
    return sengoo_net_fallback_i64_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_http_close(long long handle) {
    (void)handle;
    return sengoo_net_fallback_bool_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_http_server_bind(long long host, long long port) {
    (void)host;
    (void)port;
    return sengoo_net_fallback_handle_error(SENGOO_NET_ERR_UNSUPPORTED_SCHEME);
}

long long sengoo_http_server_local_port(long long handle) {
    (void)handle;
    return sengoo_net_fallback_i64_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_http_server_set_limits(long long handle, long long max_header_bytes, long long max_body_bytes) {
    (void)handle;
    (void)max_header_bytes;
    (void)max_body_bytes;
    return sengoo_net_fallback_bool_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_http_server_add_route(
    long long handle,
    long long method,
    long long path_pattern,
    long long status,
    long long body,
    long long body_len
) {
    (void)handle;
    (void)method;
    (void)path_pattern;
    (void)status;
    (void)body;
    (void)body_len;
    return sengoo_net_fallback_bool_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_http_server_add_middleware_require_header(
    long long handle,
    long long name,
    long long expected_value,
    long long reject_status,
    long long reject_body,
    long long reject_body_len
) {
    (void)handle;
    (void)name;
    (void)expected_value;
    (void)reject_status;
    (void)reject_body;
    (void)reject_body_len;
    return sengoo_net_fallback_bool_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_http_server_add_ws_echo_route(long long handle, long long path_pattern) {
    (void)handle;
    (void)path_pattern;
    return sengoo_net_fallback_bool_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_http_server_serve_once(long long handle, long long timeout_ms) {
    (void)handle;
    (void)timeout_ms;
    return sengoo_net_fallback_bool_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_http_server_close(long long handle) {
    (void)handle;
    return sengoo_net_fallback_bool_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_ws_connect(long long url, long long timeout_ms) {
    (void)url;
    (void)timeout_ms;
    return sengoo_net_fallback_handle_error(SENGOO_NET_ERR_UNSUPPORTED_SCHEME);
}

long long sengoo_ws_send_text(long long handle, long long data, long long len) {
    (void)handle;
    (void)data;
    (void)len;
    return sengoo_net_fallback_i64_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_ws_recv_text(long long handle, long long buffer, long long capacity, long long timeout_ms) {
    (void)handle;
    (void)buffer;
    (void)capacity;
    (void)timeout_ms;
    return sengoo_net_fallback_i64_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_ws_close(long long handle) {
    (void)handle;
    return sengoo_net_fallback_bool_error(SENGOO_NET_ERR_HANDLE_NOT_FOUND);
}

long long sengoo_net_bench_last_error_code(void) {
    return (long long)sengoo_net_bench_fallback_last_error;
}

long long sengoo_net_bench_last_error_len(void) {
    return (long long)strlen("unsupported");
}

long long sengoo_net_bench_last_error_copy(long long buffer, long long capacity) {
    return sengoo_copy_to_raw_buffer("unsupported", buffer, capacity);
}

long long sengoo_net_bench_last_error_clear(void) {
    sengoo_net_bench_fallback_last_error = 0;
    return 0;
}

long long sengoo_net_bench_run(
    long long connections,
    long long rtt_messages_per_connection,
    long long broadcast_rounds,
    long long payload_bytes,
    long long report_buffer,
    long long report_capacity
) {
    (void)connections;
    (void)rtt_messages_per_connection;
    (void)broadcast_rounds;
    (void)payload_bytes;
    (void)report_buffer;
    (void)report_capacity;
    sengoo_net_bench_fallback_last_error = -2699;
    return -1;
}
