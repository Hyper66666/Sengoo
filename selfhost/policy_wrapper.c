#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stddef.h>
#include <stdio.h>
#include <string.h>

static BOOL WINAPI audit_info(HANDLE, FILE_INFO_BY_HANDLE_CLASS, LPVOID, DWORD);
static HANDLE WINAPI audit_open(LPCWSTR, DWORD, DWORD, LPSECURITY_ATTRIBUTES,
                               DWORD, DWORD, HANDLE);
#define GetFileInformationByHandleEx audit_info
#define CreateFileW audit_open
#include "source_snapshot_original.inc"
#undef CreateFileW
#undef GetFileInformationByHandleEx

static CompilerSnapshotState *audit_state;
static int64_t audit_mode, audit_fault_at, audit_queries, audit_opens, audit_bad;
static HANDLE audit_directories[SS_DIRECTORIES];
static uint64_t audit_directory_count;

int64_t sengoo_policy_test_arm(int64_t address, int64_t extent, int64_t mode,
                             int64_t fault_at) {
    CompilerSnapshotState *state = ss_active(address, extent);
    if (!state || !state->file || !state->count || mode < 0 || mode > 4) return -3;
    audit_state = state;
    audit_directory_count = state->count;
    for (uint64_t i = 0; i < state->count; ++i) audit_directories[i] = state->directories[i];
    audit_mode = mode;
    audit_fault_at = fault_at;
    audit_queries = audit_opens = audit_bad = 0;
    return (int64_t)state->count;
}
int64_t sengoo_policy_test_observe(int64_t kind) {
    if (kind == 0) return audit_queries;
    if (kind == 1) return audit_opens;
    if (kind == 2) return audit_bad;
    if (kind == 3) return (int64_t)audit_directory_count;
    return -1;
}
int64_t sengoo_policy_test_handles(void) {
    DWORD count = 0;
    return GetProcessHandleCount(GetCurrentProcess(), &count) ? count : -1;
}
void sengoo_policy_test_disarm(void) { audit_state = NULL; }

static HANDLE WINAPI audit_open(LPCWSTR path, DWORD access, DWORD share,
                               LPSECURITY_ATTRIBUTES security, DWORD disposition,
                               DWORD flags, HANDLE template_file) {
    if (audit_state) ++audit_opens;
    return CreateFileW(path, access, share, security, disposition, flags, template_file);
}
static BOOL WINAPI audit_info(HANDLE handle, FILE_INFO_BY_HANDLE_CLASS kind,
                             LPVOID output, DWORD length) {
    int64_t index = audit_queries++;
    if (!audit_state || kind != FileCaseSensitiveInfo || length != sizeof(FILE_CASE_SENSITIVE_INFO) ||
        index < 0 || (uint64_t)index >= audit_directory_count || handle != audit_directories[index]) {
        ++audit_bad;
        SetLastError(ERROR_INVALID_HANDLE);
        return FALSE;
    }
    if (audit_mode == 0) return GetFileInformationByHandleEx(handle, kind, output, length);
    if (audit_mode == 3 && index == audit_fault_at) {
        SetLastError(ERROR_NOT_SUPPORTED);
        return FALSE;
    }
    FILE_CASE_SENSITIVE_INFO *info = output;
    info->Flags = (audit_mode == 2 && index == audit_fault_at) ? 1 :
                  (audit_mode == 4 && index == audit_fault_at) ? 2 : 0;
    return TRUE;
}

#ifdef POLICY_STANDALONE
#define CHECK(value) do { if (!(value)) { fprintf(stderr, "policy C check line %d: %s\n", __LINE__, #value); return 1; } } while (0)
int main(int argc, char **argv) {
    CHECK(argc == 2);
    uint64_t measured = 0;
    for (int round = 0; round < 3; ++round) {
        int64_t baseline = sengoo_policy_test_handles();
        uint64_t storage[128] = {0};
        int64_t address = (int64_t)(intptr_t)storage;
        char normalized[32768];
        int64_t bytes = sengoo_compiler_snapshot_full_path((int64_t)(intptr_t)argv[1], (int64_t)strlen(argv[1]), (int64_t)(intptr_t)normalized, sizeof(normalized));
        CHECK(bytes > 0);
        CHECK(sengoo_compiler_snapshot_init(address, 1024) == 0);
        CHECK(sengoo_compiler_snapshot_case_insensitive(address, 1024) == -3);
        CHECK(sengoo_compiler_snapshot_open(address, 1024, (int64_t)(intptr_t)normalized, bytes) == 0);
        int64_t count = sengoo_policy_test_arm(address, 1024, 0, -1);
        CHECK(count > 0);
        int64_t actual = sengoo_compiler_snapshot_case_insensitive(address, 1024);
        CHECK(actual == 1);
        CHECK(audit_queries == count && audit_opens == 0 && audit_bad == 0);
        for (int mode = 1; mode <= 4; ++mode) {
            for (int64_t index = 0; index < count; ++index) {
                CHECK(sengoo_policy_test_arm(address, 1024, mode, index) == count);
                int64_t result = sengoo_compiler_snapshot_case_insensitive(address, 1024);
                CHECK(result == (mode == 1 ? 1 : mode == 3 ? -8 : 0));
                CHECK(audit_queries == (mode == 1 ? count : index + 1));
                CHECK(audit_opens == 0 && audit_bad == 0);
                ++measured;
            }
        }
        CHECK(sengoo_compiler_snapshot_case_insensitive(address, 1000) == -3);
        sengoo_policy_test_disarm();
        CHECK(sengoo_compiler_snapshot_close(address, 1024) == 0);
        CHECK(sengoo_compiler_snapshot_case_insensitive(address, 1024) == -3);
        CHECK(sengoo_policy_test_handles() == baseline);
        printf("policy C scope %d: %lld retained ancestor handles checked\n", round, (long long)count);
    }
    printf("policy C injected observations: %llu\n", (unsigned long long)measured);
    return 0;
}
#endif
