/* Independent consumer of the strict-Python corpus, not a UTF-8 decoder.
 * Main gate: link this file and the SG object, WITHOUT memory_read.c.
 * Production adapter smoke: define SENGOO_UTF8_PRODUCTION_READ and link
 * memory_read.c as well, then invoke --smoke. JSON declares that mode.
 * Binary format: SGUTF8 01 00, LE u64 count; each record is u8 length,
 * u8 valid, ceil((length+1)/8) little-endian boundary bytes, then payload.
 */
#include <stdint.h>
#include <inttypes.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#error This probe requires Windows VirtualAlloc guard pages; no host skip is a pass.
#endif

extern int64_t sengoo_sg_utf8_validate(const uint8_t *, int64_t);
extern int64_t sengoo_sg_utf8_boundary(const uint8_t *, int64_t, int64_t);

#define TEXT_LIMIT INT64_C(67108864)
#define CORPUS_LIMIT 64
static const uint8_t *active_source;
static uint64_t active_extent, visited;
static uint64_t memory_reads, validation_calls, boundary_calls;
static uint64_t corpus_records, valid_records, invalid_records, guard_cases;
static uint64_t no_read_calls;
static int large_limit_checked;
#ifndef SENGOO_UTF8_PRODUCTION_READ
static uint8_t *large_visits;
static size_t large_visit_bytes;
#endif
static uint64_t current_case;

static void fail(const char *reason, int64_t length, int64_t offset,
                 int64_t expected, int64_t actual) {
    fprintf(stderr, "runtime-text failure case=%" PRIu64 " reason=%s length=%" PRId64
            " offset=%" PRId64 " expected=%" PRId64 " actual=%" PRId64 "\n",
            current_case, reason, length, offset, expected, actual);
    exit(1);
}

#ifndef SENGOO_UTF8_PRODUCTION_READ
/* This replaces the product adapter only. It does not classify UTF-8. */
uint8_t sengoo_runtime_memory_read_u8(const uint8_t *source, int64_t index) {
    uintptr_t begin = (uintptr_t)active_source, supplied = (uintptr_t)source;
    if (index < 0 || supplied < begin || supplied - begin > active_extent ||
        (uint64_t)index >= active_extent - (supplied - begin))
        fail("read-outside-authorized-input", (int64_t)active_extent, index, 0, 1);
    uint64_t at = (uint64_t)(supplied - begin) + (uint64_t)index;
    if (active_extent <= CORPUS_LIMIT) {
        visited |= UINT64_C(1) << at;
    } else {
        if (!large_visits || at / 8 >= large_visit_bytes)
            fail("probe-visit-mask-limit", (int64_t)active_extent, index, 0, 1);
        large_visits[at / 8] |= (uint8_t)(1u << (at % 8));
    }
    ++memory_reads;
    return source[index];
}
#endif

#ifndef SENGOO_UTF8_PRODUCTION_READ
static uint64_t all_bytes_mask(int64_t length) {
    return length == 64 ? UINT64_MAX : (UINT64_C(1) << length) - 1;
}
#endif

static void call_subject(int boundary, const uint8_t *source, int64_t length,
                         int64_t offset, int expected, int must_visit_all,
                         int must_not_read, const uint8_t *before, size_t bytes) {
    uint64_t reads_before = memory_reads;
    visited = 0;
#ifndef SENGOO_UTF8_PRODUCTION_READ
    if (active_extent > CORPUS_LIMIT && large_visits) memset(large_visits, 0, large_visit_bytes);
#endif
    int64_t actual;
    if (boundary) {
        ++boundary_calls;
        actual = sengoo_sg_utf8_boundary(source, length, offset);
    } else {
        ++validation_calls;
        actual = sengoo_sg_utf8_validate(source, length);
    }
    if (actual != expected)
        fail(boundary ? "boundary-oracle" : "validation-oracle", length, offset, expected, actual);
#ifndef SENGOO_UTF8_PRODUCTION_READ
    if (must_not_read && memory_reads != reads_before)
        fail("required-no-read-path", length, offset, 0, (int64_t)(memory_reads - reads_before));
    if (must_visit_all) {
        if (length <= CORPUS_LIMIT) {
            if (visited != all_bytes_mask(length))
                fail("did-not-validate-complete-input", length, offset, (int64_t)all_bytes_mask(length), (int64_t)visited);
        } else {
            if (length != TEXT_LIMIT || !large_visits || large_visit_bytes != (size_t)TEXT_LIMIT / 8)
                fail("unconfigured-large-coverage", length, offset, 1, 0);
            for (size_t index = 0; index < large_visit_bytes; ++index)
                if (large_visits[index] != 255)
                    fail("did-not-validate-complete-large-input", length, (int64_t)index * 8, 255, large_visits[index]);
        }
    }
#else
    (void)reads_before;
    (void)must_visit_all;
#endif
    if (must_not_read) ++no_read_calls;
    if (bytes && memcmp(source, before, bytes))
        fail("input-modified", length, offset, 0, 1);
}

static void check_case(const uint8_t *source, const uint8_t *original, uint8_t length,
                       int valid, const uint8_t *boundaries, int guard) {
    active_source = source;
    active_extent = length;
    call_subject(0, source, length, -2, valid, valid, length == 0, original, length);
    for (int64_t offset = 0; offset <= length; ++offset) {
        int expected = (boundaries[(size_t)offset / 8] >> (offset % 8)) & 1;
        call_subject(1, source, length, offset, expected, valid, length == 0, original, length);
    }
    call_subject(1, source, length, -1, 0, 0, 1, original, length);
    call_subject(1, source, length, (int64_t)length + 1, 0, 0, 1, original, length);
    if (guard) ++guard_cases;
}

static void read_exact(FILE *stream, void *destination, size_t bytes) {
    if (bytes && fread(destination, 1, bytes, stream) != bytes)
        fail("truncated-corpus", (int64_t)bytes, -2, 1, 0);
}

static void check_corpus(const char *path) {
    FILE *stream = NULL;
    if (fopen_s(&stream, path, "rb") || !stream) fail("open-corpus", 0, -2, 1, 0);
    const uint8_t magic[8] = {'S','G','U','T','F','8',1,0};
    uint8_t header[16];
    read_exact(stream, header, sizeof header);
    if (memcmp(header, magic, sizeof magic)) fail("corpus-magic", 0, -2, 1, 0);
    uint64_t count = 0;
    for (unsigned i = 0; i < 8; ++i) count |= (uint64_t)header[8 + i] << (8 * i);
    if (!count || count > UINT64_C(5000000)) fail("corpus-count", (int64_t)count, -2, 1, 0);
    for (uint64_t record = 0; record < count; ++record) {
        current_case = record;
        uint8_t prefix[2], bits[9] = {0}, original[CORPUS_LIMIT], input[CORPUS_LIMIT];
        read_exact(stream, prefix, sizeof prefix);
        uint8_t length = prefix[0], valid = prefix[1];
        if (length > CORPUS_LIMIT || valid > 1) fail("corpus-record-header", length, -2, 1, 0);
        size_t mask_bytes = ((size_t)length + 8) / 8;
        read_exact(stream, bits, mask_bytes);
        read_exact(stream, original, length);
        if (valid) {
            if (!(bits[0] & 1) || !(bits[length / 8] & (1u << (length % 8))))
                fail("corpus-valid-endpoints", length, -2, 1, 0);
        } else {
            for (size_t i = 0; i < mask_bytes; ++i)
                if (bits[i]) fail("invalid-input-boundaries-not-zero", length, -2, 0, bits[i]);
        }
        unsigned used_last = (length + 1) % 8;
        if (used_last && (bits[mask_bytes - 1] >> used_last))
            fail("corpus-unused-boundary-bits", length, -2, 0, 1);
        memset(input, 0xA5, sizeof input);
        memcpy(input, original, length);
        check_case(input, original, length, valid, bits, 0);
        ++corpus_records;
        if (valid) ++valid_records; else ++invalid_records;
    }
    if (fgetc(stream) != EOF || ferror(stream)) fail("corpus-trailing-data", 0, -2, 0, 1);
    if (fclose(stream)) fail("close-corpus", 0, -2, 0, 1);
}

struct guard_case { uint8_t length, valid, boundaries, bytes[4]; };
/* Explicit scalar and invalid byte examples, not a decoder. */
static const struct guard_case guard_data[] = {
    {0,1,1,{0}}, {1,1,3,{0}}, {1,1,3,{0x41}}, {1,0,0,{0x80}},
    {1,0,0,{0xC2}}, {1,0,0,{0xFF}}, {2,1,5,{0xC2,0xA2}},
    {2,1,5,{0xDF,0xBF}}, {2,1,7,{0x41,0x42}}, {2,0,0,{0xC0,0xAF}},
    {2,0,0,{0xE0,0xA0}}, {2,0,0,{0x80,0x80}}, {3,1,9,{0xE0,0xA0,0x80}},
    {3,1,9,{0xED,0x9F,0xBF}}, {3,1,9,{0xEF,0xBF,0xBF}},
    {3,1,11,{0x41,0xC2,0xA2}}, {3,1,13,{0xC2,0xA2,0x41}},
    {3,0,0,{0xED,0xA0,0x80}}, {3,0,0,{0xE0,0x9F,0xBF}},
    {3,0,0,{0xF4,0x8F,0xBF}}, {4,1,17,{0xF0,0x90,0x80,0x80}},
    {4,1,17,{0xF4,0x8F,0xBF,0xBF}}, {4,1,31,{0x41,0x42,0x43,0x44}},
    {4,1,21,{0xC2,0xA2,0xC2,0xA2}}, {4,1,19,{0x41,0xE2,0x82,0xAC}},
    {4,0,0,{0xF4,0x90,0x80,0x80}}, {4,0,0,{0xF5,0x80,0x80,0x80}},
    {4,0,0,{0xF0,0x80,0x80,0x80}}, {4,0,0,{0xED,0xBF,0xBF,0x41}},
    {4,0,0,{0xF0,0x90,0x80,0xFF}}
};

static void check_guard_pages(void) {
    SYSTEM_INFO information;
    GetSystemInfo(&information);
    size_t page = information.dwPageSize;
    uint8_t *region = (uint8_t *)VirtualAlloc(NULL, page * 3, MEM_RESERVE | MEM_COMMIT, PAGE_NOACCESS);
    if (!region) fail("VirtualAlloc", 0, -2, 1, 0);
    DWORD old_protection;
    if (!VirtualProtect(region + page, page, PAGE_READWRITE, &old_protection))
        fail("VirtualProtect", 0, -2, 1, 0);
    for (unsigned placement = 0; placement < 2; ++placement) {
        for (size_t index = 0; index < sizeof guard_data / sizeof guard_data[0]; ++index) {
            const struct guard_case *test = &guard_data[index];
            uint8_t *source = placement ? region + page : region + 2 * page - test->length;
            memcpy(source, test->bytes, test->length);
            current_case = UINT64_C(0x80000000) + placement * 100 + index;
            check_case(source, test->bytes, test->length, test->valid, &test->boundaries, 1);
        }
    }
    const uint8_t *unreadable[] = {NULL, region, region + 2 * page};
    const int64_t invalid_lengths[] = {INT64_MIN, -1, TEXT_LIMIT + 1, INT64_MAX};
    for (size_t pointer = 0; pointer < sizeof unreadable / sizeof unreadable[0]; ++pointer) {
        active_source = unreadable[pointer];
        active_extent = 0;
        ++current_case;
        call_subject(0, active_source, 0, -2, 1, 1, 1, NULL, 0);
        call_subject(1, active_source, 0, 0, 1, 1, 1, NULL, 0);
        call_subject(1, active_source, 0, -1, 0, 0, 1, NULL, 0);
        call_subject(1, active_source, 0, 1, 0, 0, 1, NULL, 0);
        for (size_t i = 0; i < sizeof invalid_lengths / sizeof invalid_lengths[0]; ++i) {
            int64_t length = invalid_lengths[i];
            call_subject(0, active_source, length, -2, 0, 0, 1, NULL, 0);
            call_subject(1, active_source, length, 0, 0, 0, 1, NULL, 0);
            call_subject(1, active_source, length, INT64_MIN, 0, 0, 1, NULL, 0);
            call_subject(1, active_source, length, INT64_MAX, 0, 0, 1, NULL, 0);
        }
        const int64_t rejected_offset_lengths[] = {1,4,TEXT_LIMIT};
        for (size_t i = 0; i < sizeof rejected_offset_lengths / sizeof rejected_offset_lengths[0]; ++i) {
            int64_t length = rejected_offset_lengths[i];
            call_subject(1, active_source, length, -1, 0, 0, 1, NULL, 0);
            call_subject(1, active_source, length, length + 1, 0, 0, 1, NULL, 0);
            call_subject(1, active_source, length, INT64_MIN, 0, 0, 1, NULL, 0);
            call_subject(1, active_source, length, INT64_MAX, 0, 0, 1, NULL, 0);
        }
    }
    if (!VirtualFree(region, 0, MEM_RELEASE)) fail("VirtualFree", 0, -2, 1, 0);
}

/* A real readable maximum-size source, read-only and immediately followed by
 * PAGE_NOACCESS. The coverage bitmap is probe memory, never an SG allocation. */
static void check_large_limit(void) {
    SYSTEM_INFO information;
    GetSystemInfo(&information);
    size_t page = information.dwPageSize;
    if ((size_t)TEXT_LIMIT % page) fail("large-page-alignment", TEXT_LIMIT, -2, 1, 0);
    uint8_t *region = (uint8_t *)VirtualAlloc(NULL, (size_t)TEXT_LIMIT + 2 * page,
                                             MEM_RESERVE | MEM_COMMIT, PAGE_NOACCESS);
    if (!region) fail("large-VirtualAlloc", TEXT_LIMIT, -2, 1, 0);
    DWORD previous;
    uint8_t *source = region + page;
    if (!VirtualProtect(source, (SIZE_T)TEXT_LIMIT, PAGE_READWRITE, &previous))
        fail("large-VirtualProtect-write", TEXT_LIMIT, -2, 1, 0);
    memset(source, 'A', (size_t)TEXT_LIMIT);
    if (!VirtualProtect(source, (SIZE_T)TEXT_LIMIT, PAGE_READONLY, &previous))
        fail("large-VirtualProtect-readonly", TEXT_LIMIT, -2, 1, 0);
#ifndef SENGOO_UTF8_PRODUCTION_READ
    large_visit_bytes = (size_t)TEXT_LIMIT / 8;
    large_visits = (uint8_t *)calloc(large_visit_bytes, 1);
    if (!large_visits) fail("large-coverage-allocation", TEXT_LIMIT, -2, 1, 0);
#endif
    active_source = source;
    active_extent = (uint64_t)TEXT_LIMIT;
    ++current_case;
    call_subject(0, source, TEXT_LIMIT, -2, 1, 1, 0, NULL, 0);
    call_subject(1, source, TEXT_LIMIT, 0, 1, 1, 0, NULL, 0);
    call_subject(1, source, TEXT_LIMIT, TEXT_LIMIT, 1, 1, 0, NULL, 0);
    call_subject(0, source, TEXT_LIMIT + 1, -2, 0, 0, 1, NULL, 0);
    call_subject(1, source, TEXT_LIMIT + 1, 0, 0, 0, 1, NULL, 0);
#ifndef SENGOO_UTF8_PRODUCTION_READ
    free(large_visits);
    large_visits = NULL;
    large_visit_bytes = 0;
#endif
    active_source = NULL;
    active_extent = 0;
    if (!VirtualFree(region, 0, MEM_RELEASE)) fail("large-VirtualFree", TEXT_LIMIT, -2, 1, 0);
    large_limit_checked = 1;
}

int main(int argc, char **argv) {
    if (argc != 2 && argc != 3) { fprintf(stderr, "usage: runtime_text_probe corpus.bin | --smoke | --large-limit\n"); return 64; }
    int large = !strcmp(argv[1], "--large-limit");
    if (argc == 3) {
        if (strcmp(argv[2], "--large-limit")) return 64;
        large = 1;
    }
    if (strcmp(argv[1], "--smoke") && strcmp(argv[1], "--large-limit")) check_corpus(argv[1]);
    check_guard_pages();
    if (large) check_large_limit();
    printf("{\"schema\":1,\"ok\":true,\"corpus_records\":%" PRIu64
           ",\"valid_records\":%" PRIu64 ",\"invalid_records\":%" PRIu64
           ",\"guard_cases\":%" PRIu64 ",\"validation_calls\":%" PRIu64
           ",\"boundary_calls\":%" PRIu64 ",\"no_read_calls\":%" PRIu64,
           corpus_records, valid_records, invalid_records, guard_cases, validation_calls,
           boundary_calls, no_read_calls);
#ifdef SENGOO_UTF8_PRODUCTION_READ
    printf(",\"instrumented\":false,\"memory_reads\":null");
#else
    printf(",\"instrumented\":true,\"memory_reads\":%" PRIu64, memory_reads);
#endif
    printf(",\"input_unchanged\":true,\"guard_pages\":true,\"large_limit_checked\":%s}\n", large_limit_checked ? "true" : "false");
    return 0;
}
