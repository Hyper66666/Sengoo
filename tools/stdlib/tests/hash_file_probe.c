#define _CRT_SECURE_NO_WARNINGS
#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
static int force_failure, reads;
static DWORD fragment_limit;
static BOOL WINAPI probe_read(HANDLE file, LPVOID bytes, DWORD count, LPDWORD read, LPOVERLAPPED operation) {
    if (force_failure && reads++ >= 2) { SetLastError(ERROR_READ_FAULT); return FALSE; }
    if (fragment_limit && count > fragment_limit) count = fragment_limit;
    return ReadFile(file, bytes, count, read, operation);
}
#define ReadFile probe_read
#endif
#include "../runtime_hash_file.c"
long long sengoo_string_from_bytes_copy(long long bytes, long long length) {
    char* text = malloc((size_t)length + 1); assert(text);
    memcpy(text, (void*)(intptr_t)bytes, (size_t)length); text[length] = 0; return (long long)(intptr_t)text;
}
static long long raw(const void* p) { return (long long)(intptr_t)p; }
static void equal_hash(long long result, const char* expected) {
    assert(result > 0); const char* actual = (const char*)(intptr_t)result;
    assert(strlen(actual) == 64 && !strcmp(actual, expected)); free((void*)(intptr_t)result);
}
int main(void) {
#ifdef _WIN32
    wchar_t root[32768], directory[32768], file_name[32768];
    assert(GetTempPathW(32700, root));
    swprintf(directory, 32768, L"%lsSengoo-hash-\x6587-\xd83d\xde00-%lu", root, GetCurrentProcessId());
    assert(CreateDirectoryW(directory, NULL));
    swprintf(file_name, 32768, L"%ls\\sample.bin", directory);
    HANDLE file = CreateFileW(file_name, GENERIC_WRITE, 0, NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, NULL); assert(file != INVALID_HANDLE_VALUE);
    DWORD count; assert(WriteFile(file, "abc", 3, &count, NULL) && count == 3 && CloseHandle(file));
    char path[65536], dir_path[65536];
    assert(WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, file_name, -1, path, sizeof(path), NULL, NULL));
    assert(WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, directory, -1, dir_path, sizeof(dir_path), NULL, NULL));
    DWORD baseline; assert(GetProcessHandleCount(GetCurrentProcess(), &baseline));
    equal_hash(sengoo_hash_sha256_file(raw(path), strlen(path), 3), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    assert(sengoo_hash_sha256_file(raw(path), strlen(path), 2) == -SENGOO_STATUS_OVERFLOW);
    assert(sengoo_hash_sha256_file(raw(path), strlen(path), -1) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_hash_sha256_file(raw(dir_path), strlen(dir_path), 1000) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_hash_sha256_file(raw("NUL"), 3, 1000) == -SENGOO_STATUS_INVALID_ARGUMENT);
    const unsigned char invalid[] = { 0xc0, 0x80 };
    assert(sengoo_hash_sha256_file(raw(invalid), 2, 1000) == -SENGOO_STATUS_INVALID_UTF8);
    assert(sengoo_hash_sha256_file(raw("x\0y"), 3, 1000) == -SENGOO_STATUS_INVALID_ARGUMENT);
    fragment_limit = 1;
    equal_hash(sengoo_hash_sha256_file(raw(path), strlen(path), 3), "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad");
    for (int i = 0; i < 100; i++) {
        force_failure = 1; reads = 0;
        assert(sengoo_hash_sha256_file(raw(path), strlen(path), 3) == -SENGOO_STATUS_IO);
    }
    force_failure = 0; fragment_limit = 0;
    assert(DeleteFileW(file_name));
    assert(sengoo_hash_sha256_file(raw(path), strlen(path), 3) == -SENGOO_STATUS_NOT_FOUND);
    file = CreateFileW(file_name, GENERIC_WRITE, 0, NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, NULL); assert(file != INVALID_HANDLE_VALUE && CloseHandle(file));
    equal_hash(sengoo_hash_sha256_file(raw(path), strlen(path), 0), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    DWORD after; assert(GetProcessHandleCount(GetCurrentProcess(), &after) && after <= baseline + 2);
    assert(DeleteFileW(file_name) && RemoveDirectoryW(directory));
    puts("file hash: Unicode paths, short reads, failure cleanup, regular files, size cap and known vectors passed");
#else
    puts("file hash: native failure injection is Windows only");
#endif
    return 0;
}
