#define _CRT_SECURE_NO_WARNINGS
#if !defined(_WIN32)
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#endif
#endif
#include "runtime_shared.h"
#include <errno.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#else
#include <fcntl.h>
#include <sys/stat.h>
#include <unistd.h>
#endif

#define SENGOO_HASH_FILE_CHUNK 65536
typedef struct {
    uint32_t state[8];
    unsigned char tail[64];
    size_t tail_len;
    uint64_t total;
} SengooFileSha256;
long long sengoo_string_from_bytes_copy(long long bytes, long long length);
static void sengoo_hash_file_update(SengooFileSha256* hash, const unsigned char* bytes, size_t length) {
    hash->total += length;
    if (hash->tail_len) {
        size_t count = 64 - hash->tail_len; if (count > length) count = length;
        memcpy(hash->tail + hash->tail_len, bytes, count); hash->tail_len += count; bytes += count; length -= count;
        if (hash->tail_len == 64) { sengoo_internal_sha256_transform(hash->state, hash->tail); hash->tail_len = 0; }
    }
    while (length >= 64) { sengoo_internal_sha256_transform(hash->state, bytes); bytes += 64; length -= 64; }
    if (length) { memcpy(hash->tail, bytes, length); hash->tail_len = length; }
}
static long long sengoo_hash_file_finish(SengooFileSha256* hash) {
    size_t length = hash->tail_len; hash->tail[length++] = 0x80;
    if (length > 56) { memset(hash->tail + length, 0, 64 - length); sengoo_internal_sha256_transform(hash->state, hash->tail); length = 0; }
    memset(hash->tail + length, 0, 56 - length);
    uint64_t bits = hash->total * 8;
    for (int i = 0; i < 8; i++) hash->tail[63 - i] = (unsigned char)(bits >> (i * 8));
    sengoo_internal_sha256_transform(hash->state, hash->tail);
    static const char hex[] = "0123456789abcdef";
    char result[64];
    for (int i = 0; i < 32; i++) {
        unsigned char byte = (unsigned char)(hash->state[i / 4] >> (24 - (i % 4) * 8));
        result[i * 2] = hex[byte >> 4]; result[i * 2 + 1] = hex[byte & 15];
    }
    long long owned = sengoo_string_from_bytes_copy((long long)(intptr_t)result, 64);
    return owned > 0 ? owned : -SENGOO_STATUS_OUT_OF_MEMORY;
}
long long sengoo_hash_sha256_file(long long path_bytes, long long path_length, long long max_bytes) {
    if (!path_bytes || path_length < 1 || path_length > SENGOO_RUNTIME_MAX_PATH_BYTES || max_bytes < 0
        || (uint64_t)max_bytes > UINT64_MAX / 8) return -SENGOO_STATUS_INVALID_ARGUMENT;
    const char* path = (const char*)(intptr_t)path_bytes;
    if (memchr(path, 0, (size_t)path_length)) return -SENGOO_STATUS_INVALID_ARGUMENT;
    SengooFileSha256 hash = { {0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
        0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19}, {0}, 0, 0 };
    unsigned char bytes[SENGOO_HASH_FILE_CHUNK];
    long long status = 0;
#ifdef _WIN32
    int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, (int)path_length, NULL, 0);
    if (!count) return -SENGOO_STATUS_INVALID_UTF8;
    wchar_t* wide = malloc(((size_t)count + 1) * sizeof(wchar_t));
    if (!wide) return -SENGOO_STATUS_OUT_OF_MEMORY;
    if (!MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, (int)path_length, wide, count)) { free(wide); return -SENGOO_STATUS_INVALID_UTF8; }
    wide[count] = 0;
    HANDLE file = CreateFileW(wide, GENERIC_READ, FILE_SHARE_READ, NULL, OPEN_EXISTING,
        FILE_FLAG_SEQUENTIAL_SCAN | FILE_FLAG_OPEN_REPARSE_POINT | FILE_FLAG_BACKUP_SEMANTICS, NULL);
    free(wide);
    if (file == INVALID_HANDLE_VALUE) {
        DWORD error = GetLastError();
        if (error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND) return -SENGOO_STATUS_NOT_FOUND;
        if (error == ERROR_ACCESS_DENIED) return -SENGOO_STATUS_PERMISSION_DENIED;
        return -SENGOO_STATUS_IO;
    }
    BY_HANDLE_FILE_INFORMATION info; LARGE_INTEGER size;
    if (GetFileType(file) != FILE_TYPE_DISK) status = -SENGOO_STATUS_INVALID_ARGUMENT;
    else if (!GetFileInformationByHandle(file, &info) || !GetFileSizeEx(file, &size)) status = -SENGOO_STATUS_IO;
    else if (info.dwFileAttributes & (FILE_ATTRIBUTE_DIRECTORY | FILE_ATTRIBUTE_REPARSE_POINT)) status = -SENGOO_STATUS_INVALID_ARGUMENT;
    else if (size.QuadPart < 0 || size.QuadPart > max_bytes) status = -SENGOO_STATUS_OVERFLOW;
    while (!status) {
        DWORD read;
        if (!ReadFile(file, bytes, sizeof(bytes), &read, NULL)) { status = -SENGOO_STATUS_IO; break; }
        if (!read) break;
        if ((uint64_t)read > (uint64_t)max_bytes - hash.total) { status = -SENGOO_STATUS_OVERFLOW; break; }
        sengoo_hash_file_update(&hash, bytes, read);
    }
    if (!status && hash.total != (uint64_t)size.QuadPart) status = -SENGOO_STATUS_IO;
    if (!CloseHandle(file) && !status) status = -SENGOO_STATUS_IO;
#else
    char* terminated = malloc((size_t)path_length + 1);
    if (!terminated) return -SENGOO_STATUS_OUT_OF_MEMORY;
    memcpy(terminated, path, (size_t)path_length); terminated[path_length] = 0;
    int flags = O_RDONLY | O_NONBLOCK;
#ifdef O_CLOEXEC
    flags |= O_CLOEXEC;
#endif
#ifdef O_NOFOLLOW
    flags |= O_NOFOLLOW;
#endif
    int file = open(terminated, flags); free(terminated);
    if (file < 0) return errno == ENOENT || errno == ENOTDIR ? -SENGOO_STATUS_NOT_FOUND : errno == EACCES ? -SENGOO_STATUS_PERMISSION_DENIED : -SENGOO_STATUS_IO;
    fcntl(file, F_SETFD, FD_CLOEXEC);
    struct stat info, after;
    if (fstat(file, &info)) status = -SENGOO_STATUS_IO;
    else if (!S_ISREG(info.st_mode)) status = -SENGOO_STATUS_INVALID_ARGUMENT;
    else if (info.st_size < 0 || (uint64_t)info.st_size > (uint64_t)max_bytes) status = -SENGOO_STATUS_OVERFLOW;
    while (!status) {
        ssize_t read_count = read(file, bytes, sizeof(bytes));
        if (read_count < 0) { if (errno == EINTR) continue; status = -SENGOO_STATUS_IO; break; }
        if (!read_count) break;
        if ((uint64_t)read_count > (uint64_t)max_bytes - hash.total) { status = -SENGOO_STATUS_OVERFLOW; break; }
        sengoo_hash_file_update(&hash, bytes, (size_t)read_count);
    }
    if (!status && (hash.total != (uint64_t)info.st_size || fstat(file, &after) || after.st_size != info.st_size || after.st_mtime != info.st_mtime)) status = -SENGOO_STATUS_IO;
    if (close(file) && !status) status = -SENGOO_STATUS_IO;
#endif
    return status < 0 ? status : sengoo_hash_file_finish(&hash);
}
