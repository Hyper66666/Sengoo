/* Real OS checks. Credential records contain only a unique disposable fixture. */
#include <assert.h>
#include <stdio.h>
#include "../runtime_system.c"
typedef struct { void* value; unsigned generation; } TestSlot;
static TestSlot slots[32];
long long sengoo_opaque_handle_new(void* value) {
    for (int i = 1; i < 32; i++) if (!slots[i].value) {
        slots[i].value = value; slots[i].generation++;
        return ((long long)slots[i].generation << 32) | i;
    }
    return 0;
}
void* sengoo_opaque_handle_get(long long token) {
    unsigned i = (unsigned)token;
    return token > 0 && i < 32 && slots[i].generation == ((unsigned long long)token >> 32) ? slots[i].value : NULL;
}
void* sengoo_opaque_handle_take(long long token) {
    void* value = sengoo_opaque_handle_get(token);
    if (value) slots[(unsigned)token].value = NULL;
    return value;
}
long long sengoo_string_from_bytes_copy(long long bytes, long long length) {
    char* value = malloc((size_t)length + 1);
    if (!value) return 0;
    if (length) memcpy(value, (void*)(intptr_t)bytes, (size_t)length);
    value[length] = 0; return (long long)(intptr_t)value;
}
static long long text(const char* value) { return (long long)(intptr_t)value; }
static long long lock_path(const char* path, int exclusive) { return sengoo_file_lock(text(path), strlen(path), exclusive); }
static int run(int argc, char** argv) {
    assert(argc >= 3);
    const char* path = argv[2];
    if (!strcmp(argv[1], "--check")) {
        long long lock = lock_path(path, 1);
        if (lock < 0) return (int)-lock;
        assert(sengoo_file_lock_close(lock) == 0); return 0;
    }
    if (!strcmp(argv[1], "--hold")) {
        long long lock = lock_path(path, 1); assert(lock > 0);
        puts("held"); fflush(stdout); getchar();
        assert(sengoo_file_lock_close(lock) == 0); return 0;
    }
    assert(argc == 5);
    long long first = lock_path(path, 1); assert(first > 0);
    assert(lock_path(path, 1) == -SENGOO_STATUS_TIMEOUT);
    assert(lock_path(path, 0) == -SENGOO_STATUS_TIMEOUT);
#ifdef _WIN32
    wchar_t* wide = sengoo_system_wide(path); assert(wide);
    assert(DeleteFileW(wide) == FALSE && GetLastError() == ERROR_SHARING_VIOLATION); free(wide);
#endif
    assert(sengoo_file_lock_close(first) == 0);
    assert(sengoo_file_lock_close(first) == -SENGOO_STATUS_INVALID_HANDLE);
    long long shared = lock_path(path, 0), shared2 = lock_path(path, 0);
    assert(shared > 0 && shared2 > 0 && lock_path(path, 1) == -SENGOO_STATUS_TIMEOUT);
    assert(sengoo_file_lock_close(shared) == 0 && sengoo_file_lock_close(shared2) == 0);
    assert(sengoo_file_lock_live_count() == 0);
    int foreign;
    long long token = sengoo_opaque_handle_new(&foreign);
    assert(sengoo_file_lock_close(token) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_opaque_handle_take(token) == &foreign);
    long long canonical = sengoo_canonical_path_string(text(argv[3]), strlen(argv[3]));
    long long alias = sengoo_canonical_path_string(text(argv[4]), strlen(argv[4]));
    assert(canonical > 0 && alias > 0);
    if (strcmp((char*)(intptr_t)canonical, (char*)(intptr_t)alias)) fprintf(stderr,"canonical mismatch: %s / %s\n",(char*)(intptr_t)canonical,(char*)(intptr_t)alias);
    assert(strcmp((char*)(intptr_t)canonical, (char*)(intptr_t)alias) == 0);
    free((void*)(intptr_t)canonical); free((void*)(intptr_t)alias);
    const char bad[] = {'x',0,'y'};
    assert(sengoo_canonical_path_string(text(bad), 3) == -SENGOO_STATUS_INVALID_UTF8);
    assert(sengoo_file_lock(text(bad), 3, 1) == -SENGOO_STATUS_INVALID_UTF8);
    char target[128];
#ifdef _WIN32
    snprintf(target,sizeof(target),"Sengoo/runtime-test/%lu/%llu",GetCurrentProcessId(),(unsigned long long)GetTickCount64());
    const char* fixture = "disposable-test-世界";
    assert(sengoo_credential_read(text(target),strlen(target)) == -SENGOO_STATUS_NOT_FOUND);
    long long stored = sengoo_credential_write(text(target),strlen(target),text(fixture),strlen(fixture));
    long long restored = sengoo_credential_read(text(target),strlen(target));
    long long removed = sengoo_credential_delete(text(target),strlen(target));
    assert(stored == 1 && restored > 0 && removed == 1);
    assert(!strcmp((char*)(intptr_t)restored,fixture)); free((void*)(intptr_t)restored);
    assert(sengoo_credential_delete(text(target),strlen(target)) == 0);
    assert(sengoo_credential_read(text(target),strlen(target)) == -SENGOO_STATUS_NOT_FOUND);
    assert(sengoo_credential_write(text(target),strlen(target),text(bad),3) == -SENGOO_STATUS_INVALID_UTF8);
    stored = sengoo_credential_write(text(target),strlen(target),text(""),0);
    restored = sengoo_credential_read(text(target),strlen(target));
    removed = sengoo_credential_delete(text(target),strlen(target));
    assert(stored == 1 && restored > 0 && removed == 1);
    assert(!*(char*)(intptr_t)restored); free((void*)(intptr_t)restored);
#else
    (void)target;
    assert(sengoo_credential_read(0,0) == -SENGOO_STATUS_UNSUPPORTED);
    assert(sengoo_credential_write(0,0,0,0) == -SENGOO_STATUS_UNSUPPORTED);
    assert(sengoo_credential_delete(0,0) == -SENGOO_STATUS_UNSUPPORTED);
#endif
    puts("system: locks, shared/exclusive, stale/domain, canonical aliases and credential contract passed");
    return 0;
}
#ifdef _WIN32
int wmain(int argc, wchar_t** wide) {
    char** argv = calloc((size_t)argc + 1,sizeof(char*)); assert(argv);
    for (int i = 0; i < argc; i++) {
        int n = WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,wide[i],-1,NULL,0,NULL,NULL);
        argv[i] = malloc((size_t)n); assert(argv[i]);
        assert(WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,wide[i],-1,argv[i],n,NULL,NULL));
    }
    int code = run(argc,argv);
    for (int i = 0; i < argc; i++) free(argv[i]); free(argv); return code;
}
#else
int main(int argc, char** argv) { return run(argc,argv); }
#endif
