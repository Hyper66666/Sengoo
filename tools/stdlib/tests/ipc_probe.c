#define _CRT_SECURE_NO_WARNINGS
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../runtime_ipc.c"

typedef struct { void* p; uint32_t generation; } ProbeSlot;
static ProbeSlot slots[128];
long long sengoo_opaque_handle_new(void* p) {
    for (size_t i = 1; i < 128; i++) if (!slots[i].p) {
        slots[i].p = p; slots[i].generation++;
        return ((long long)slots[i].generation << 32) | (long long)i;
    }
    return 0;
}
void* sengoo_opaque_handle_get(long long h) {
    size_t i = (size_t)((uint64_t)h & UINT32_MAX);
    return h > 0 && i < 128 && slots[i].generation == ((uint64_t)h >> 32) ? slots[i].p : NULL;
}
void* sengoo_opaque_handle_take(long long h) {
    void* p = sengoo_opaque_handle_get(h); if (p) slots[(uint64_t)h & UINT32_MAX].p = NULL; return p;
}
SengooFfiBuffer* sengoo_ffi_buffer_from_handle(long long h) { return (SengooFfiBuffer*)(intptr_t)h; }
static long long raw(const void* p) { return (long long)(intptr_t)p; }
#ifdef _WIN32
#define PAYLOAD_SIZE (4 * SENGOO_IPC_CHUNK + 317)
static void validate_endpoint_security(SengooIpcListener* listener) {
    DWORD needed;
    GetKernelObjectSecurity(listener->pipe, DACL_SECURITY_INFORMATION, NULL, 0, &needed);
    assert(needed > 0 && needed < 65536);
    SECURITY_DESCRIPTOR* descriptor = malloc(needed); assert(descriptor);
    assert(GetKernelObjectSecurity(listener->pipe, DACL_SECURITY_INFORMATION, descriptor, needed, &needed));
    BOOL present, defaulted; ACL* acl;
    assert(GetSecurityDescriptorDacl(descriptor, &present, &acl, &defaulted) && present && acl && acl->AceCount == 1);
    ACCESS_ALLOWED_ACE* ace;
    assert(GetAce(acl, 0, (void**)&ace) && ace->Header.AceType == ACCESS_ALLOWED_ACE_TYPE);
    assert(EqualSid(&ace->SidStart, listener->identity.logon));
    DWORD flags;
    assert(GetNamedPipeInfo(listener->pipe, &flags, NULL, NULL, NULL) && (flags & PIPE_SERVER_END));
    SengooIpcIdentity actual, altered;
    assert(sengoo_ipc_identity(GetCurrentProcess(), &actual));
    assert(sengoo_ipc_same_identity(&listener->identity, &actual));
    altered = actual; altered.session++;
    assert(!sengoo_ipc_same_identity(&listener->identity, &altered));
    DWORD size = SECURITY_MAX_SID_SIZE; unsigned char everyone[SECURITY_MAX_SID_SIZE];
    assert(CreateWellKnownSid(WinWorldSid, NULL, everyone, &size));
    altered = actual; memcpy(altered.logon, everyone, size);
    assert(!sengoo_ipc_same_identity(&listener->identity, &altered));
    altered = actual; memcpy(altered.user, everyone, size);
    assert(!sengoo_ipc_same_identity(&listener->identity, &altered));
    free(descriptor);
}
static unsigned char byte_at(size_t at) { return (unsigned char)((at * 29 + at / 31) & 255); }
static long long accept_peer(long long listener) {
    ULONGLONG deadline = GetTickCount64() + 5000;
    for (;;) {
        long long stream = sengoo_ipc_listener_try_accept(listener);
        assert(stream >= 0);
        if (stream) return stream;
        assert(GetTickCount64() < deadline); Sleep(1);
    }
}
static void wait_write(long long stream) {
    long long ready = sengoo_ipc_stream_poll(stream, 2, 5000);
    assert(ready == 2);
}
static int client(const char* name) {
    long long stream = sengoo_ipc_connect(raw(name), (long long)strlen(name)); assert(stream > 0);
    unsigned char bytes[113]; SengooFfiBuffer buffer = { bytes, sizeof(bytes), 0 };
    size_t offset = 0; ULONGLONG deadline = GetTickCount64() + 10000;
    /* Give the sender enough time to fill both the kernel and owned queues. */
    Sleep(80);
    while (offset < PAYLOAD_SIZE) {
        assert(GetTickCount64() < deadline);
        long long n = sengoo_ipc_stream_read(stream, raw(&buffer)); assert(n >= 0 && n <= (long long)sizeof(bytes));
        assert(buffer.used_len == (size_t)n);
        for (long long i = 0; i < n; i++) assert(bytes[i] == byte_at(offset + (size_t)i));
        offset += (size_t)n;
        if (!n) { assert(sengoo_ipc_stream_eof(stream) == 0); assert(sengoo_ipc_stream_poll(stream, 1, 1000) >= 0); }
    }
    const unsigned char response[] = {0, 'O', 'K', 0xff, '\n'};
    memcpy(bytes, response, sizeof(response)); buffer.used_len = sizeof(response);
    assert(sengoo_ipc_stream_write(stream, raw(&buffer), sizeof(response)) == sizeof(response)); wait_write(stream);
    /* Closing after queue completion must leave every already sent byte. */
    assert(sengoo_ipc_stream_close(stream) == 0 && sengoo_ipc_live_handle_count() == 0);
    return 0;
}
static void paired_pressure_and_process_test(const char* name, long long listener) {
    wchar_t self[32768], command[32768]; assert(GetModuleFileNameW(NULL, self, 32768));
    wchar_t wide_name[128]; assert(MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, name, -1, wide_name, 128));
    swprintf(command, 32768, L"\"%ls\" --client %ls", self, wide_name);
    STARTUPINFOW start; PROCESS_INFORMATION process; memset(&start, 0, sizeof(start)); start.cb = sizeof(start);
    start.dwFlags = STARTF_USESHOWWINDOW; start.wShowWindow = SW_HIDE;
    assert(CreateProcessW(self, command, NULL, NULL, FALSE, CREATE_NO_WINDOW, NULL, NULL, &start, &process));
    long long stream = accept_peer(listener); assert(sengoo_ipc_stream_peer_pid(stream) == process.dwProcessId);
    unsigned char bytes[SENGOO_IPC_CHUNK]; SengooFfiBuffer buffer = { bytes, sizeof(bytes), sizeof(bytes) };
    size_t offset = 0; ULONGLONG deadline = GetTickCount64() + 10000; int backpressure = 0;
    while (offset < PAYLOAD_SIZE) {
        assert(GetTickCount64() < deadline);
        size_t count = PAYLOAD_SIZE - offset; if (count > sizeof(bytes)) count = sizeof(bytes);
        for (size_t i = 0; i < count; i++) bytes[i] = byte_at(offset + i);
        buffer.used_len = count;
        ULONGLONG before = GetTickCount64();
        long long n = sengoo_ipc_stream_write(stream, raw(&buffer), (long long)count);
        assert(n >= 0 && GetTickCount64() - before < 1000);
        if (!n) { backpressure = 1; assert(sengoo_ipc_stream_poll(stream, 2, 1000) >= 0); }
        else {
            assert(n == (long long)count); offset += (size_t)n;
            /* The caller may immediately mutate/release its borrowed bytes. */
            memset(bytes, 0xee, count);
        }
    }
    assert(backpressure); wait_write(stream);
    unsigned char response[5]; size_t received = 0;
    while (received < sizeof(response)) {
        buffer.capacity = sizeof(response) - received;
        long long n = sengoo_ipc_stream_read(stream, raw(&buffer)); assert(n >= 0);
        if (n) { memcpy(response + received, bytes, (size_t)n); received += (size_t)n; }
        else { assert(GetTickCount64() < deadline); assert(sengoo_ipc_stream_poll(stream, 1, 1000) >= 0); }
    }
    const unsigned char expected[] = {0, 'O', 'K', 0xff, '\n'}; assert(!memcmp(response, expected, sizeof(expected)));
    DWORD exit_code;
    assert(WaitForSingleObject(process.hProcess, 10000) == WAIT_OBJECT_0 && GetExitCodeProcess(process.hProcess, &exit_code) && exit_code == 0);
    CloseHandle(process.hThread); CloseHandle(process.hProcess);
    assert(sengoo_ipc_stream_poll(stream, 1, 1000) == 4 && sengoo_ipc_stream_eof(stream) == 1);
    assert(sengoo_ipc_stream_close(stream) == 0 && sengoo_ipc_stream_peer_pid(stream) == -SENGOO_STATUS_INVALID_HANDLE);
}
static void cancellation_and_stale_test(const char* name, long long listener) {
    long long stale = 0;
    DWORD baseline; assert(GetProcessHandleCount(GetCurrentProcess(), &baseline));
    for (int i = 0; i < 80; i++) {
        long long client_stream = sengoo_ipc_connect(raw(name), (long long)strlen(name)); assert(client_stream > 0);
        long long server_stream = accept_peer(listener);
        assert(sengoo_ipc_stream_peer_pid(client_stream) == GetCurrentProcessId());
        assert(sengoo_ipc_stream_poll(server_stream, 1, 0) == 0); /* pending read */
        unsigned char* bytes = malloc(SENGOO_IPC_CHUNK); assert(bytes); memset(bytes, 'x', SENGOO_IPC_CHUNK);
        SengooFfiBuffer buffer = { bytes, SENGOO_IPC_CHUNK, SENGOO_IPC_CHUNK };
        int blocked = 0;
        for (int j = 0; j < 32; j++) {
            long long count = sengoo_ipc_stream_write(client_stream, raw(&buffer), SENGOO_IPC_CHUNK);
            assert(count >= 0); if (!count) { blocked = 1; break; }
        }
        assert(blocked); free(bytes);
        ULONGLONG before = GetTickCount64();
        assert(sengoo_ipc_stream_close(client_stream) == 0 && sengoo_ipc_stream_close(server_stream) == 0);
        assert(GetTickCount64() - before < 1000);
        assert(sengoo_ipc_stream_poll(client_stream, 1, 0) == -SENGOO_STATUS_INVALID_HANDLE);
        assert(sengoo_ipc_stream_close(client_stream) == -SENGOO_STATUS_INVALID_HANDLE);
        if (stale) assert(sengoo_ipc_stream_eof(stale) == -SENGOO_STATUS_INVALID_HANDLE);
        stale = server_stream;
    }
    DWORD after; assert(GetProcessHandleCount(GetCurrentProcess(), &after) && after <= baseline + 2);
}
#endif
int main(int argc, char** argv) {
#ifdef _WIN32
    if (argc == 3 && !strcmp(argv[1], "--client")) return client(argv[2]);
    char name[120]; snprintf(name, sizeof(name), "ipc-probe-%lu-%llu", GetCurrentProcessId(), GetTickCount64());
    assert(sengoo_ipc_listen(raw("../bad"), 6) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_ipc_connect(raw("\\\\host\\pipe\\evil"), 16) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_ipc_listen(raw("name\0tail"), 9) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_ipc_connect(raw(name), (long long)strlen(name)) == -SENGOO_STATUS_NOT_FOUND);
    long long listener = sengoo_ipc_listen(raw(name), (long long)strlen(name)); assert(listener > 0);
    assert(sengoo_ipc_listen(raw(name), (long long)strlen(name)) == -SENGOO_STATUS_ALREADY_EXISTS);
    assert(sengoo_ipc_listener_try_accept(listener) == 0 && sengoo_ipc_live_handle_count() == 1);
    validate_endpoint_security((SengooIpcListener*)sengoo_ipc_resolve(listener, SG_IPC_LISTENER));
    assert(sengoo_ipc_stream_eof(listener) == -SENGOO_STATUS_INVALID_HANDLE);
    paired_pressure_and_process_test(name, listener);
    cancellation_and_stale_test(name, listener);
    /* A disconnected peer may fail authentication; accepting can recover. */
    long long disappeared = sengoo_ipc_connect(raw(name), (long long)strlen(name)); assert(disappeared > 0);
    assert(sengoo_ipc_stream_close(disappeared) == 0);
    long long result = sengoo_ipc_listener_try_accept(listener);
    assert(result == -SENGOO_STATUS_PERMISSION_DENIED || result == -SENGOO_STATUS_IO || result > 0);
    if (result > 0) assert(sengoo_ipc_stream_close(result) == 0);
    assert(sengoo_ipc_listener_close(listener) == 0); /* outstanding accept cancellation */
    assert(sengoo_ipc_listener_try_accept(listener) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_ipc_live_handle_count() == 0);
    listener = sengoo_ipc_listen(raw(name), (long long)strlen(name)); assert(listener > 0);
    long long c = sengoo_ipc_connect(raw(name), (long long)strlen(name)); assert(c > 0);
    long long s = accept_peer(listener);
    unsigned char byte = 0; SengooFfiBuffer empty = { &byte, 1, 0 };
    assert(sengoo_ipc_stream_write(c, raw(&empty), 1) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_ipc_stream_write(c, raw(&empty), SENGOO_IPC_CHUNK + 1) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_ipc_stream_poll(c, 0, 0) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_ipc_stream_poll(c, 1, -1) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_ipc_stream_poll(c, 1, 60001) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_ipc_stream_close(c) == 0 && sengoo_ipc_stream_close(s) == 0 && sengoo_ipc_listener_close(listener) == 0);
    long long owners[SENGOO_IPC_MAX_HANDLES];
    for (int i = 0; i < SENGOO_IPC_MAX_HANDLES; i++) {
        char bounded_name[128]; snprintf(bounded_name, sizeof(bounded_name), "%s-%d", name, i);
        owners[i] = sengoo_ipc_listen(raw(bounded_name), (long long)strlen(bounded_name)); assert(owners[i] > 0);
    }
    assert(sengoo_ipc_listen(raw(name), (long long)strlen(name)) == -SENGOO_STATUS_OVERFLOW);
    for (int i = 0; i < SENGOO_IPC_MAX_HANDLES; i++) assert(sengoo_ipc_listener_close(owners[i]) == 0);
    assert(sengoo_ipc_stream_close(0) == 0 && sengoo_ipc_listener_close(0) == 0 && sengoo_ipc_live_handle_count() == 0);
    puts("ipc: same-logon DACL/identity, two processes, binary pressure, cancellation, EOF and cleanup passed");
#else
    (void)argc; (void)argv;
    assert(sengoo_ipc_listen(raw("probe"), 5) == -SENGOO_STATUS_UNSUPPORTED);
    assert(sengoo_ipc_connect(raw("probe"), 5) == -SENGOO_STATUS_UNSUPPORTED);
    puts("ipc: non-Windows explicitly unsupported");
#endif
    return 0;
}
