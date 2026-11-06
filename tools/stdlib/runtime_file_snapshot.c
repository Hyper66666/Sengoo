#define _CRT_SECURE_NO_WARNINGS
#include "runtime_shared.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

long long sengoo_string_from_bytes_copy(long long bytes, long long length);

#ifdef _WIN32
#include <windows.h>
#include <wchar.h>
#include <wctype.h>

#define SG_SNAPSHOT_MAX_OWNERS 32
#define SG_SNAPSHOT_MAX_BYTES (64LL * 1024 * 1024)
#define SG_SNAPSHOT_TOTAL_BYTES (128LL * 1024 * 1024)
#define SG_SNAPSHOT_MAX_DIRS 64
#define SG_SNAPSHOT_PATH_CHARS 32768

/* Single-owner API; no application state or format parsing enters this bridge.
 * Parents deny DELETE sharing so a held file cannot be redirected through a
 * renamed ancestor. The volume GUID path also avoids mutable DOS drive aliases. */
typedef struct SgFileSnapshot {
    long long handle, length;
    HANDLE file, directories[SG_SNAPSHOT_MAX_DIRS];
    size_t directory_count;
    unsigned char* bytes;
    char *path, *pinned_path;
    char digest[65], identity[96];
    BY_HANDLE_FILE_INFORMATION before;
    struct SgFileSnapshot* next;
} SgFileSnapshot;
static SgFileSnapshot* sg_snapshot_owners;
static long long sg_snapshot_count, sg_snapshot_bytes;

static long long sg_snapshot_error(DWORD error) {
    if (error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND) return -SENGOO_STATUS_NOT_FOUND;
    if (error == ERROR_ACCESS_DENIED || error == ERROR_SHARING_VIOLATION || error == ERROR_LOCK_VIOLATION) return -SENGOO_STATUS_PERMISSION_DENIED;
    if (error == ERROR_NOT_ENOUGH_MEMORY || error == ERROR_OUTOFMEMORY) return -SENGOO_STATUS_OUT_OF_MEMORY;
    if (error == ERROR_NOT_SUPPORTED || error == ERROR_INVALID_FUNCTION) return -SENGOO_STATUS_UNSUPPORTED;
    return -SENGOO_STATUS_IO;
}
static SgFileSnapshot* sg_snapshot_find(long long handle) {
    for (SgFileSnapshot* at = sg_snapshot_owners; at; at = at->next)
        if (at->handle == handle) return at;
    return NULL;
}
static long long sg_snapshot_destroy(SgFileSnapshot* value) {
    long long status = 0;
    if (!value) return 0;
    if (value->file && value->file != INVALID_HANDLE_VALUE && !CloseHandle(value->file)) status = -SENGOO_STATUS_IO;
    while (value->directory_count)
        if (!CloseHandle(value->directories[--value->directory_count])) status = -SENGOO_STATUS_IO;
    free(value->bytes); free(value->path); free(value->pinned_path); free(value);
    return status;
}
static long long sg_snapshot_wide_to_utf8(const wchar_t* text, char** out) {
    *out = NULL;
    int count = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text, -1, NULL, 0, NULL, NULL);
    if (!count) return -SENGOO_STATUS_INVALID_UTF8;
    if (count > SENGOO_RUNTIME_MAX_PATH_BYTES) return -SENGOO_STATUS_OVERFLOW;
    char* bytes = malloc((size_t)count);
    if (!bytes) return -SENGOO_STATUS_OUT_OF_MEMORY;
    if (!WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, text, -1, bytes, count, NULL, NULL)) { free(bytes); return -SENGOO_STATUS_INVALID_UTF8; }
    *out = bytes; return 0;
}
static int sg_snapshot_reserved(const wchar_t* name, size_t length) {
    size_t count = 0;
    while (count < length && name[count] != L'.') count++;
    if (count == 3) {
        wchar_t a = towupper(name[0]), b = towupper(name[1]), c = towupper(name[2]);
        return (a == L'C' && b == L'O' && c == L'N') || (a == L'P' && b == L'R' && c == L'N')
            || (a == L'A' && b == L'U' && c == L'X') || (a == L'N' && b == L'U' && c == L'L');
    }
    return count == 4 && name[3] >= L'1' && name[3] <= L'9'
        && ((!_wcsnicmp(name, L"COM", 3)) || (!_wcsnicmp(name, L"LPT", 3)));
}
static long long sg_snapshot_path(long long pointer, long long length, wchar_t** out) {
    *out = NULL;
    if (!pointer || length < 1 || length >= SENGOO_RUNTIME_MAX_PATH_BYTES) return -SENGOO_STATUS_INVALID_ARGUMENT;
    const char* bytes = (const char*)(intptr_t)pointer;
    if (memchr(bytes, 0, (size_t)length)) return -SENGOO_STATUS_INVALID_ARGUMENT;
    int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, bytes, (int)length, NULL, 0);
    if (!count) return -SENGOO_STATUS_INVALID_UTF8;
    wchar_t* input = malloc(((size_t)count + 1) * sizeof(wchar_t));
    wchar_t* full = malloc(SG_SNAPSHOT_PATH_CHARS * sizeof(wchar_t));
    if (!input || !full) { free(input); free(full); return -SENGOO_STATUS_OUT_OF_MEMORY; }
    if (!MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, bytes, (int)length, input, count)) { free(input); free(full); return -SENGOO_STATUS_INVALID_UTF8; }
    input[count] = 0;
    for (int i = 0; i < count; i++) if (input[i] == L'/') input[i] = L'\\';
    /* Do not normalize device/UNC names, ADS or drive-relative paths into a
     * different policy. A normal relative local path is allowed. */
    if ((count >= 2 && input[0] == L'\\' && input[1] == L'\\') || (count >= 2 && input[1] == L':' && (count < 3 || input[2] != L'\\'))) {
        free(input); free(full); return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    for (int i = 0; i < count; i++) if (input[i] == L':' && i != 1) { free(input); free(full); return -SENGOO_STATUS_INVALID_ARGUMENT; }
    DWORD used = GetFullPathNameW(input, SG_SNAPSHOT_PATH_CHARS - 4, full + 4, NULL);
    free(input);
    if (!used || used >= SG_SNAPSHOT_PATH_CHARS - 4) { free(full); return used ? -SENGOO_STATUS_OVERFLOW : sg_snapshot_error(GetLastError()); }
    wchar_t* normalized = full + 4;
    if (used < 4 || normalized[1] != L':' || normalized[2] != L'\\' || normalized[used - 1] == L'\\') { free(full); return -SENGOO_STATUS_INVALID_ARGUMENT; }
    wchar_t root[4] = {normalized[0], L':', L'\\', 0};
    if (GetDriveTypeW(root) != DRIVE_FIXED) { free(full); return -SENGOO_STATUS_UNSUPPORTED; }
    size_t start = 3;
    for (size_t i = 3; i <= used; i++) {
        if (normalized[i] == L':' || (normalized[i] < 32 && normalized[i] != 0)) { free(full); return -SENGOO_STATUS_INVALID_ARGUMENT; }
        if (i == used || normalized[i] == L'\\') {
            size_t part = i - start;
            if (!part || normalized[i - 1] == L'.' || normalized[i - 1] == L' ' || sg_snapshot_reserved(normalized + start, part)) { free(full); return -SENGOO_STATUS_INVALID_ARGUMENT; }
            start = i + 1;
        }
    }
    memcpy(full, L"\\\\?\\", 4 * sizeof(wchar_t)); *out = full; return 0;
}
static long long sg_snapshot_guard_directory(SgFileSnapshot* value, const wchar_t* path) {
    if (value->directory_count >= SG_SNAPSHOT_MAX_DIRS) return -SENGOO_STATUS_OVERFLOW;
    HANDLE directory = CreateFileW(path, FILE_READ_ATTRIBUTES, FILE_SHARE_READ | FILE_SHARE_WRITE, NULL,
        OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT, NULL);
    if (directory == INVALID_HANDLE_VALUE) return sg_snapshot_error(GetLastError());
    BY_HANDLE_FILE_INFORMATION info;
    if (!GetFileInformationByHandle(directory, &info)) { long long status = sg_snapshot_error(GetLastError()); CloseHandle(directory); return status; }
    if (!(info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY) || (info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT)) { CloseHandle(directory); return -SENGOO_STATUS_INVALID_ARGUMENT; }
    value->directories[value->directory_count++] = directory; return 0;
}
static int sg_snapshot_same(const BY_HANDLE_FILE_INFORMATION* a, const BY_HANDLE_FILE_INFORMATION* b) {
    return a->dwVolumeSerialNumber == b->dwVolumeSerialNumber && a->nFileIndexHigh == b->nFileIndexHigh && a->nFileIndexLow == b->nFileIndexLow
        && a->nFileSizeHigh == b->nFileSizeHigh && a->nFileSizeLow == b->nFileSizeLow
        && a->dwFileAttributes == b->dwFileAttributes
        && !memcmp(&a->ftCreationTime, &b->ftCreationTime, sizeof(FILETIME))
        && !memcmp(&a->ftLastWriteTime, &b->ftLastWriteTime, sizeof(FILETIME));
}
static void sg_snapshot_digest(const unsigned char* bytes, size_t length, char out[65]) {
    uint32_t state[8] = {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19};
    size_t position = 0;
    while (length - position >= 64) { sengoo_internal_sha256_transform(state, bytes + position); position += 64; }
    unsigned char tail[64] = {0}; size_t remaining = length - position;
    if (remaining) memcpy(tail, bytes + position, remaining);
    tail[remaining] = 0x80;
    if (remaining >= 56) { sengoo_internal_sha256_transform(state, tail); memset(tail, 0, 64); }
    uint64_t bits = (uint64_t)length * 8;
    for (int i = 0; i < 8; i++) tail[63 - i] = (unsigned char)(bits >> (i * 8));
    sengoo_internal_sha256_transform(state, tail);
    for (int i = 0; i < 8; i++) snprintf(out + i * 8, 9, "%08x", state[i]);
}
static long long sg_snapshot_final_path(HANDLE file, DWORD flags, char** out) {
    wchar_t* wide = malloc(SG_SNAPSHOT_PATH_CHARS * sizeof(wchar_t));
    if (!wide) return -SENGOO_STATUS_OUT_OF_MEMORY;
    DWORD count = GetFinalPathNameByHandleW(file, wide, SG_SNAPSHOT_PATH_CHARS, flags);
    if (!count || count >= SG_SNAPSHOT_PATH_CHARS) { long long status = count ? -SENGOO_STATUS_OVERFLOW : sg_snapshot_error(GetLastError()); free(wide); return status; }
    const wchar_t* text = wide;
    if (flags == VOLUME_NAME_DOS && count >= 7 && !wcsncmp(wide, L"\\\\?\\", 4) && wide[5] == L':') text += 4;
    long long status = sg_snapshot_wide_to_utf8(text, out); free(wide); return status;
}
long long sengoo_file_snapshot_available(void) { return 1; }
long long sengoo_file_snapshot_live_handle_count(void) { return sg_snapshot_count; }
long long sengoo_file_snapshot_live_bytes(void) { return sg_snapshot_bytes; }
long long sengoo_file_snapshot_open(long long path, long long path_length, long long max_bytes) {
    if (max_bytes < 0 || max_bytes > SG_SNAPSHOT_MAX_BYTES) return -SENGOO_STATUS_INVALID_ARGUMENT;
    if (sg_snapshot_count >= SG_SNAPSHOT_MAX_OWNERS) return -SENGOO_STATUS_OVERFLOW;
    wchar_t* full = NULL;
    long long status = sg_snapshot_path(path, path_length, &full);
    if (status < 0) return status;
    SgFileSnapshot* value = calloc(1, sizeof(*value));
    if (!value) { free(full); return -SENGOO_STATUS_OUT_OF_MEMORY; }
    /* Capture a root handle, then use its volume-GUID path for every remaining
     * lookup. A later user DOS-drive alias change cannot redirect those opens. */
    wchar_t saved = full[7]; full[7] = 0; status = sg_snapshot_guard_directory(value, full); full[7] = saved;
    size_t root_length = 0;
    if (!status) {
        wchar_t volume[128];
        DWORD count = GetFinalPathNameByHandleW(value->directories[0], volume, 128, VOLUME_NAME_GUID);
        if (!count || count >= 128) status = count ? -SENGOO_STATUS_OVERFLOW : sg_snapshot_error(GetLastError());
        else if (wcsncmp(volume, L"\\\\?\\Volume{", 11) || volume[count - 1] != L'\\') status = -SENGOO_STATUS_UNSUPPORTED;
        else {
            size_t suffix = wcslen(full + 7);
            if ((size_t)count + suffix >= SG_SNAPSHOT_PATH_CHARS) status = -SENGOO_STATUS_OVERFLOW;
            else {
                wchar_t* pinned = malloc(SG_SNAPSHOT_PATH_CHARS * sizeof(wchar_t));
                if (!pinned) status = -SENGOO_STATUS_OUT_OF_MEMORY;
                else { memcpy(pinned, volume, count * sizeof(wchar_t)); memcpy(pinned + count, full + 7, (suffix + 1) * sizeof(wchar_t)); free(full); full = pinned; root_length = count; }
            }
        }
    }
    /* Pin every ancestor before resolving the next component. */
    for (size_t i = root_length; !status && full[i]; i++) if (full[i] == L'\\') {
        full[i] = 0; status = sg_snapshot_guard_directory(value, full); full[i] = L'\\';
    }
    if (!status) {
        value->file = CreateFileW(full, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING,
            FILE_FLAG_SEQUENTIAL_SCAN | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS, NULL);
        if (value->file == INVALID_HANDLE_VALUE) status = sg_snapshot_error(GetLastError());
    }
    free(full);
    if (!status && GetFileType(value->file) != FILE_TYPE_DISK) status = -SENGOO_STATUS_INVALID_ARGUMENT;
    if (!status && !GetFileInformationByHandle(value->file, &value->before)) status = sg_snapshot_error(GetLastError());
    if (!status && value->before.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)) status = -SENGOO_STATUS_INVALID_ARGUMENT;
    if (!status) {
        uint64_t length = (uint64_t)value->before.nFileSizeHigh << 32 | value->before.nFileSizeLow;
        if (length > (uint64_t)max_bytes || length > (uint64_t)(SG_SNAPSHOT_TOTAL_BYTES - sg_snapshot_bytes)) status = -SENGOO_STATUS_OVERFLOW;
        else value->length = (long long)length;
    }
    if (!status) status = sg_snapshot_final_path(value->file, VOLUME_NAME_DOS, &value->path);
    if (!status) status = sg_snapshot_final_path(value->file, VOLUME_NAME_GUID, &value->pinned_path);
    if (!status && strncmp(value->pinned_path, "\\\\?\\Volume{", 11)) status = -SENGOO_STATUS_UNSUPPORTED;
    if (!status) { value->bytes = malloc((size_t)value->length + 1); if (!value->bytes) status = -SENGOO_STATUS_OUT_OF_MEMORY; }
    size_t offset = 0;
    while (!status && offset < (size_t)value->length) {
        DWORD count = (DWORD)((size_t)value->length - offset); if (count > 65536) count = 65536;
        DWORD got = 0;
        if (!ReadFile(value->file, value->bytes + offset, count, &got, NULL)) status = sg_snapshot_error(GetLastError());
        else if (!got || got > count) status = -SENGOO_STATUS_IO;
        else offset += got;
    }
    if (!status) {
        unsigned char extra; DWORD got = 0;
        if (!ReadFile(value->file, &extra, 1, &got, NULL)) status = sg_snapshot_error(GetLastError());
        else if (got) status = value->length >= max_bytes ? -SENGOO_STATUS_OVERFLOW : -SENGOO_STATUS_IO;
    }
    if (!status) {
        BY_HANDLE_FILE_INFORMATION after;
        if (!GetFileInformationByHandle(value->file, &after)) status = sg_snapshot_error(GetLastError());
        else if (!sg_snapshot_same(&value->before, &after)) status = -SENGOO_STATUS_IO;
    }
    if (!status) {
        sg_snapshot_digest(value->bytes, (size_t)value->length, value->digest);
        snprintf(value->identity, sizeof(value->identity), "%08lx:%08lx%08lx:%08lx%08lx",
            value->before.dwVolumeSerialNumber, value->before.nFileIndexHigh, value->before.nFileIndexLow,
            value->before.ftCreationTime.dwHighDateTime, value->before.ftCreationTime.dwLowDateTime);
        long long handle = sengoo_opaque_handle_new(value);
        if (handle <= 0) status = -SENGOO_STATUS_OUT_OF_MEMORY;
        else { value->handle = handle; value->next = sg_snapshot_owners; sg_snapshot_owners = value; sg_snapshot_count++; sg_snapshot_bytes += value->length; return handle; }
    }
    sg_snapshot_destroy(value); return status;
}
long long sengoo_file_snapshot_length(long long handle) {
    SgFileSnapshot* value = sg_snapshot_find(handle); return value ? value->length : -SENGOO_STATUS_INVALID_HANDLE;
}
long long sengoo_file_snapshot_text(long long handle, long long field) {
    SgFileSnapshot* value = sg_snapshot_find(handle);
    if (!value) return -SENGOO_STATUS_INVALID_HANDLE;
    const char* text = field == 0 ? value->digest : field == 1 ? value->identity : field == 2 ? value->path : field == 3 ? value->pinned_path : NULL;
    if (!text) return -SENGOO_STATUS_INVALID_ARGUMENT;
    long long result = sengoo_string_from_bytes_copy((long long)(intptr_t)text, (long long)strlen(text));
    return result > 0 ? result : -SENGOO_STATUS_OUT_OF_MEMORY;
}
long long sengoo_file_snapshot_modified_ms(long long handle) {
    SgFileSnapshot* value = sg_snapshot_find(handle);
    if (!value) return -SENGOO_STATUS_INVALID_HANDLE;
    uint64_t ticks = (uint64_t)value->before.ftLastWriteTime.dwHighDateTime << 32 | value->before.ftLastWriteTime.dwLowDateTime;
    if (ticks < 116444736000000000ULL) return -SENGOO_STATUS_UNSUPPORTED;
    return (long long)((ticks - 116444736000000000ULL) / 10000);
}
long long sengoo_file_snapshot_copy_bytes(long long handle, long long output) {
    SgFileSnapshot* value = sg_snapshot_find(handle);
    if (!value) return -SENGOO_STATUS_INVALID_HANDLE;
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(output);
    if (!buffer) return -SENGOO_STATUS_INVALID_HANDLE;
    if ((uint64_t)value->length > buffer->capacity) return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    if (value->length) memcpy(buffer->bytes, value->bytes, (size_t)value->length);
    buffer->used_len = (size_t)value->length; return value->length;
}
long long sengoo_file_snapshot_still_matches(long long handle) {
    SgFileSnapshot* value = sg_snapshot_find(handle);
    if (!value) return -SENGOO_STATUS_INVALID_HANDLE;
    BY_HANDLE_FILE_INFORMATION after;
    if (!GetFileInformationByHandle(value->file, &after)) return sg_snapshot_error(GetLastError());
    return sg_snapshot_same(&value->before, &after);
}
long long sengoo_file_snapshot_close(long long handle) {
    if (!handle) return 0;
    SgFileSnapshot* value = sg_snapshot_find(handle);
    if (!value) return -SENGOO_STATUS_INVALID_HANDLE;
    SgFileSnapshot** at = &sg_snapshot_owners;
    while (*at && *at != value) at = &(*at)->next;
    if (*at) *at = value->next;
    sengoo_opaque_handle_take(handle); sg_snapshot_count--; sg_snapshot_bytes -= value->length;
    return sg_snapshot_destroy(value);
}
#else
long long sengoo_file_snapshot_available(void) { return 0; }
long long sengoo_file_snapshot_live_handle_count(void) { return 0; }
long long sengoo_file_snapshot_live_bytes(void) { return 0; }
long long sengoo_file_snapshot_open(long long path, long long length, long long cap) { (void)path; (void)length; (void)cap; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_file_snapshot_length(long long h) { (void)h; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_file_snapshot_text(long long h, long long field) { (void)h; (void)field; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_file_snapshot_modified_ms(long long h) { (void)h; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_file_snapshot_copy_bytes(long long h, long long b) { (void)h; (void)b; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_file_snapshot_still_matches(long long h) { (void)h; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_file_snapshot_close(long long h) { return h ? -SENGOO_STATUS_UNSUPPORTED : 0; }
#endif
