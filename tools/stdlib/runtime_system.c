#define _CRT_SECURE_NO_WARNINGS
#if !defined(_WIN32)
#ifndef _XOPEN_SOURCE
#define _XOPEN_SOURCE 700
#endif
#endif
#include "runtime_shared.h"
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#include <wincred.h>
#else
#include <fcntl.h>
#include <sys/file.h>
#include <unistd.h>
#ifdef __APPLE__
#include <mach-o/dyld.h>
#endif
#endif

long long sengoo_string_from_bytes_copy(long long bytes, long long length);
typedef struct SengooFileLock {
    long long token;
    struct SengooFileLock* next;
#ifdef _WIN32
    HANDLE file;
#else
    int file;
#endif
} SengooFileLock;
static SengooFileLock* sengoo_system_locks;

long long sengoo_current_executable_string(void) {
#ifdef _WIN32
    DWORD capacity = 512;
    for (;;) {
        wchar_t* wide = malloc((size_t)capacity * sizeof(*wide));
        if (!wide) return -SENGOO_STATUS_OUT_OF_MEMORY;
        DWORD count = GetModuleFileNameW(NULL, wide, capacity);
        if (!count) { free(wide); return -SENGOO_STATUS_IO; }
        if (count >= capacity) {
            free(wide);
            if (capacity >= 32768) return -SENGOO_STATUS_OVERFLOW;
            capacity = capacity > 16384 ? 32768 : capacity * 2;
            continue;
        }
        int bytes = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide,
            (int)count, NULL, 0, NULL, NULL);
        if (bytes <= 0) { free(wide); return -SENGOO_STATUS_INVALID_UTF8; }
        char* utf8 = malloc((size_t)bytes);
        if (!utf8) { free(wide); return -SENGOO_STATUS_OUT_OF_MEMORY; }
        if (WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide, (int)count,
                utf8, bytes, NULL, NULL) != bytes) {
            free(utf8); free(wide); return -SENGOO_STATUS_INVALID_UTF8;
        }
        long long result = sengoo_string_from_bytes_copy((long long)(intptr_t)utf8, bytes);
        free(utf8); free(wide); return result;
    }
#elif defined(__linux__)
    size_t capacity = 512;
    while (capacity <= SENGOO_RUNTIME_MAX_PATH_BYTES) {
        char* path = malloc(capacity);
        if (!path) return -SENGOO_STATUS_OUT_OF_MEMORY;
        ssize_t count = readlink("/proc/self/exe", path, capacity);
        if (count < 0) { free(path); return -SENGOO_STATUS_IO; }
        if ((size_t)count < capacity) {
            long long result = sengoo_string_from_bytes_copy((long long)(intptr_t)path, count);
            free(path); return result;
        }
        free(path); capacity *= 2;
    }
    return -SENGOO_STATUS_OVERFLOW;
#elif defined(__APPLE__)
    uint32_t capacity = 0;
    _NSGetExecutablePath(NULL, &capacity);
    if (!capacity || capacity > SENGOO_RUNTIME_MAX_PATH_BYTES) return -SENGOO_STATUS_OVERFLOW;
    char* path = malloc(capacity);
    if (!path) return -SENGOO_STATUS_OUT_OF_MEMORY;
    if (_NSGetExecutablePath(path, &capacity)) { free(path); return -SENGOO_STATUS_IO; }
    char* resolved = realpath(path, NULL); free(path);
    if (!resolved) return -SENGOO_STATUS_IO;
    long long result = sengoo_string_from_bytes_copy((long long)(intptr_t)resolved, (long long)strlen(resolved));
    free(resolved); return result;
#else
    return -SENGOO_STATUS_UNSUPPORTED;
#endif
}

static int sengoo_system_utf8(const unsigned char* text, size_t length) {
    for (size_t i = 0; i < length;) {
        unsigned c = text[i++]; int n;
        if (!c) return 0;
        if (c < 128) continue;
        if (c >= 0xc2 && c <= 0xdf) { n = 1; c &= 31; }
        else if (c >= 0xe0 && c <= 0xef) { n = 2; c &= 15; }
        else if (c >= 0xf0 && c <= 0xf4) { n = 3; c &= 7; }
        else return 0;
        if (length - i < (size_t)n) return 0;
        for (int j = 0; j < n; j++) {
            unsigned next = text[i++]; if ((next & 0xc0) != 0x80) return 0;
            c = (c << 6) | (next & 63);
        }
        if ((n == 1 && c < 128) || (n == 2 && c < 2048) || (n == 3 && c < 65536)
            || c > 0x10ffff || (c >= 0xd800 && c <= 0xdfff)) return 0;
    }
    return 1;
}
static long long sengoo_system_text(long long bytes, long long length, size_t maximum, int empty, char** out) {
    if (length < 0 || (unsigned long long)length > maximum || (!empty && !length) || (length && !bytes))
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    if (!sengoo_system_utf8((const unsigned char*)(intptr_t)bytes, (size_t)length)) return -SENGOO_STATUS_INVALID_UTF8;
    *out = malloc((size_t)length + 1);
    if (!*out) return -SENGOO_STATUS_OUT_OF_MEMORY;
    if (length) memcpy(*out, (const void*)(intptr_t)bytes, (size_t)length);
    (*out)[length] = 0; return 0;
}
#ifdef _WIN32
static long long sengoo_system_error(DWORD error) {
    if (error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND || error == ERROR_NOT_FOUND) return -SENGOO_STATUS_NOT_FOUND;
    if (error == ERROR_ACCESS_DENIED || error == ERROR_PRIVILEGE_NOT_HELD) return -SENGOO_STATUS_PERMISSION_DENIED;
    if (error == ERROR_LOCK_VIOLATION || error == ERROR_SHARING_VIOLATION) return -SENGOO_STATUS_TIMEOUT;
    if (error == ERROR_NOT_SUPPORTED) return -SENGOO_STATUS_UNSUPPORTED;
    return -SENGOO_STATUS_IO;
}
static wchar_t* sengoo_system_wide(const char* text) {
    int length = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, -1, NULL, 0);
    if (!length) return NULL;
    wchar_t* wide = malloc((size_t)length * sizeof(*wide));
    if (wide && !MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, -1, wide, length)) { free(wide); return NULL; }
    return wide;
}
#else
static long long sengoo_system_error(int error) {
    if (error == ENOENT || error == ENOTDIR) return -SENGOO_STATUS_NOT_FOUND;
    if (error == EACCES || error == EPERM) return -SENGOO_STATUS_PERMISSION_DENIED;
    if (error == EAGAIN || error == EWOULDBLOCK) return -SENGOO_STATUS_TIMEOUT;
    return -SENGOO_STATUS_IO;
}
#endif

long long sengoo_file_lock(long long bytes, long long length, long long exclusive) {
    if (exclusive != 0 && exclusive != 1) return -SENGOO_STATUS_INVALID_ARGUMENT;
    char* path = NULL;
    long long result = sengoo_system_text(bytes, length, SENGOO_RUNTIME_MAX_PATH_BYTES, 0, &path);
    if (result < 0) return result;
    SengooFileLock* lock = calloc(1, sizeof(*lock));
    if (!lock) { free(path); return -SENGOO_STATUS_OUT_OF_MEMORY; }
#ifdef _WIN32
    wchar_t* wide = sengoo_system_wide(path); free(path);
    if (!wide) { free(lock); return -SENGOO_STATUS_OUT_OF_MEMORY; }
    lock->file = CreateFileW(wide, GENERIC_READ | GENERIC_WRITE, FILE_SHARE_READ | FILE_SHARE_WRITE,
        NULL, OPEN_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL); free(wide);
    if (lock->file == INVALID_HANDLE_VALUE) { result = sengoo_system_error(GetLastError()); free(lock); return result; }
    OVERLAPPED range; memset(&range, 0, sizeof(range));
    if (!LockFileEx(lock->file, LOCKFILE_FAIL_IMMEDIATELY | (exclusive ? LOCKFILE_EXCLUSIVE_LOCK : 0), 0, MAXDWORD, MAXDWORD, &range)) {
        result = sengoo_system_error(GetLastError()); CloseHandle(lock->file); free(lock); return result;
    }
#else
    lock->file = open(path, O_RDWR | O_CREAT | O_CLOEXEC, 0600); free(path);
    if (lock->file < 0) { result = sengoo_system_error(errno); free(lock); return result; }
    if (flock(lock->file, (exclusive ? LOCK_EX : LOCK_SH) | LOCK_NB)) {
        result = sengoo_system_error(errno); close(lock->file); free(lock); return result;
    }
#endif
    lock->token = sengoo_opaque_handle_new(lock);
    if (lock->token <= 0) {
#ifdef _WIN32
        CloseHandle(lock->file);
#else
        close(lock->file);
#endif
        free(lock); return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    lock->next = sengoo_system_locks; sengoo_system_locks = lock; return lock->token;
}
long long sengoo_file_lock_close(long long token) {
    SengooFileLock** entry = &sengoo_system_locks;
    while (*entry && (*entry)->token != token) entry = &(*entry)->next;
    if (!*entry) return token == 0 ? 0 : -SENGOO_STATUS_INVALID_HANDLE;
    SengooFileLock* lock = *entry; *entry = lock->next;
    sengoo_opaque_handle_take(token);
#ifdef _WIN32
    int ok = CloseHandle(lock->file);
#else
    int ok = close(lock->file) == 0;
#endif
    free(lock); return ok ? 0 : -SENGOO_STATUS_IO;
}
long long sengoo_file_lock_live_count(void) {
    long long count = 0; for (SengooFileLock* lock = sengoo_system_locks; lock; lock = lock->next) count++;
    return count;
}

long long sengoo_canonical_path_string(long long bytes, long long length) {
    char* path = NULL;
    long long result = sengoo_system_text(bytes, length, SENGOO_RUNTIME_MAX_PATH_BYTES, 0, &path);
    if (result < 0) return result;
#ifdef _WIN32
    wchar_t* wide = sengoo_system_wide(path); free(path);
    if (!wide) return -SENGOO_STATUS_OUT_OF_MEMORY;
    HANDLE file = CreateFileW(wide, FILE_READ_ATTRIBUTES, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
        NULL, OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, NULL); free(wide);
    if (file == INVALID_HANDLE_VALUE) return sengoo_system_error(GetLastError());
    DWORD capacity = GetFinalPathNameByHandleW(file, NULL, 0, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS);
    wchar_t* final = capacity ? malloc(((size_t)capacity + 1) * sizeof(wchar_t)) : NULL;
    if (!final) { result = capacity ? -SENGOO_STATUS_OUT_OF_MEMORY : sengoo_system_error(GetLastError()); CloseHandle(file); return result; }
    DWORD used = GetFinalPathNameByHandleW(file, final, capacity + 1, FILE_NAME_NORMALIZED | VOLUME_NAME_DOS);
    CloseHandle(file);
    if (!used || used > capacity) { result = sengoo_system_error(GetLastError()); free(final); return result; }
    wchar_t* normalized = final;
    if (wcsncmp(final, L"\\\\?\\UNC\\", 8) == 0) { final[6] = L'\\'; normalized = final + 6; }
    else if (wcsncmp(final, L"\\\\?\\", 4) == 0) normalized = final + 4;
    int utf8_size = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, normalized, -1, NULL, 0, NULL, NULL);
    if (utf8_size <= 0 || utf8_size > SENGOO_RUNTIME_MAX_PATH_BYTES + 1) { free(final); return -SENGOO_STATUS_OVERFLOW; }
    char* utf8 = malloc((size_t)utf8_size);
    if (!utf8) { free(final); return -SENGOO_STATUS_OUT_OF_MEMORY; }
    int ok = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, normalized, -1, utf8, utf8_size, NULL, NULL);
    free(final);
    result = ok ? sengoo_string_from_bytes_copy((long long)(intptr_t)utf8, utf8_size - 1) : -SENGOO_STATUS_INVALID_UTF8;
    free(utf8);
#else
    char* normalized = realpath(path, NULL); free(path);
    if (!normalized) return sengoo_system_error(errno);
    size_t size = strlen(normalized);
    if (size > SENGOO_RUNTIME_MAX_PATH_BYTES) { free(normalized); return -SENGOO_STATUS_OVERFLOW; }
    result = sengoo_system_utf8((unsigned char*)normalized, size)
        ? sengoo_string_from_bytes_copy((long long)(intptr_t)normalized, (long long)size) : -SENGOO_STATUS_INVALID_UTF8;
    free(normalized);
#endif
    return result > 0 ? result : result < 0 ? result : -SENGOO_STATUS_OUT_OF_MEMORY;
}

/* Generic credentials are per-user OS records. No enumeration or logging. */
long long sengoo_credential_read(long long target, long long target_length) {
#ifdef _WIN32
    char* text = NULL; long long result = sengoo_system_text(target, target_length, 1024, 0, &text);
    if (result < 0) return result;
    wchar_t* wide = sengoo_system_wide(text); free(text);
    if (!wide) return -SENGOO_STATUS_OUT_OF_MEMORY;
    PCREDENTIALW credential = NULL;
    BOOL ok = CredReadW(wide, CRED_TYPE_GENERIC, 0, &credential); free(wide);
    if (!ok) return sengoo_system_error(GetLastError());
    size_t size = credential->CredentialBlobSize;
    if (size > CRED_MAX_CREDENTIAL_BLOB_SIZE) result = -SENGOO_STATUS_OVERFLOW;
    else if (!sengoo_system_utf8(credential->CredentialBlob, size)) result = -SENGOO_STATUS_INVALID_UTF8;
    else result = sengoo_string_from_bytes_copy((long long)(intptr_t)credential->CredentialBlob, (long long)size);
    if (credential->CredentialBlob) SecureZeroMemory(credential->CredentialBlob, size);
    CredFree(credential);
    return result > 0 ? result : result < 0 ? result : -SENGOO_STATUS_OUT_OF_MEMORY;
#else
    (void)target; (void)target_length; return -SENGOO_STATUS_UNSUPPORTED;
#endif
}
long long sengoo_credential_write(long long target, long long target_length, long long value, long long value_length) {
#ifdef _WIN32
    char *text = NULL, *secret = NULL;
    long long result = sengoo_system_text(target, target_length, 1024, 0, &text);
    if (result < 0) return result;
    result = sengoo_system_text(value, value_length, CRED_MAX_CREDENTIAL_BLOB_SIZE, 1, &secret);
    if (result < 0) { free(text); return result; }
    wchar_t* wide = sengoo_system_wide(text); free(text);
    if (!wide) { SecureZeroMemory(secret, (size_t)value_length); free(secret); return -SENGOO_STATUS_OUT_OF_MEMORY; }
    CREDENTIALW credential; memset(&credential, 0, sizeof(credential));
    credential.Type = CRED_TYPE_GENERIC; credential.TargetName = wide;
    credential.CredentialBlob = (LPBYTE)secret; credential.CredentialBlobSize = (DWORD)value_length;
    credential.Persist = CRED_PERSIST_LOCAL_MACHINE;
    BOOL ok = CredWriteW(&credential, 0); DWORD error = GetLastError();
    SecureZeroMemory(secret, (size_t)value_length); free(secret); free(wide);
    return ok ? 1 : sengoo_system_error(error);
#else
    (void)target; (void)target_length; (void)value; (void)value_length; return -SENGOO_STATUS_UNSUPPORTED;
#endif
}
long long sengoo_credential_delete(long long target, long long target_length) {
#ifdef _WIN32
    char* text = NULL; long long result = sengoo_system_text(target, target_length, 1024, 0, &text);
    if (result < 0) return result;
    wchar_t* wide = sengoo_system_wide(text); free(text);
    if (!wide) return -SENGOO_STATUS_OUT_OF_MEMORY;
    BOOL ok = CredDeleteW(wide, CRED_TYPE_GENERIC, 0); DWORD error = GetLastError(); free(wide);
    return ok ? 1 : error == ERROR_NOT_FOUND ? 0 : sengoo_system_error(error);
#else
    (void)target; (void)target_length; return -SENGOO_STATUS_UNSUPPORTED;
#endif
}
