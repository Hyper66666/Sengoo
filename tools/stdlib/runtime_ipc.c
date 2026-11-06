#define _CRT_SECURE_NO_WARNINGS
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0601
#endif
#include "runtime_shared.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#include <sddl.h>
#include <stdio.h>

/* Local byte transport only. Framing, protocol versions, request identities
 * and application state are deliberately kept outside the native runtime. */
#define SENGOO_IPC_CHUNK 65536
#define SENGOO_IPC_MAX_HANDLES 64
enum { SG_IPC_LISTENER = 1, SG_IPC_STREAM = 2 };
typedef struct {
    unsigned char user[SECURITY_MAX_SID_SIZE], logon[SECURITY_MAX_SID_SIZE];
    DWORD session;
} SengooIpcIdentity;
typedef struct SengooIpcBase {
    long long handle;
    int kind;
    struct SengooIpcBase* next;
} SengooIpcBase;
typedef struct {
    SengooIpcBase base;
    SengooIpcIdentity identity;
    wchar_t name[512];
    HANDLE pipe;
    OVERLAPPED connect;
    int pending, connected;
} SengooIpcListener;
typedef struct {
    SengooIpcBase base;
    HANDLE pipe;
    OVERLAPPED read, write;
    DWORD peer_pid, read_len, read_offset, write_len, write_offset;
    int read_pending, write_pending, eof, write_error;
    unsigned char read_bytes[SENGOO_IPC_CHUNK], write_bytes[SENGOO_IPC_CHUNK];
} SengooIpcStream;
static SengooIpcBase* sengoo_ipc_handles;
static long long sengoo_ipc_live_count;

static SengooIpcBase* sengoo_ipc_resolve(long long handle, int kind) {
    for (SengooIpcBase* p = sengoo_ipc_handles; p; p = p->next)
        if (p->handle == handle && p->kind == kind) return p;
    return NULL;
}
static long long sengoo_ipc_register(SengooIpcBase* p, int kind) {
    if (sengoo_ipc_live_count >= SENGOO_IPC_MAX_HANDLES) return -SENGOO_STATUS_OVERFLOW;
    long long handle = sengoo_opaque_handle_new(p);
    if (handle <= 0) return -SENGOO_STATUS_OUT_OF_MEMORY;
    p->handle = handle; p->kind = kind; p->next = sengoo_ipc_handles;
    sengoo_ipc_handles = p; sengoo_ipc_live_count++; return handle;
}
static void sengoo_ipc_unregister(SengooIpcBase* p) {
    SengooIpcBase** at = &sengoo_ipc_handles;
    while (*at && *at != p) at = &(*at)->next;
    if (*at) *at = p->next;
    sengoo_opaque_handle_take(p->handle); sengoo_ipc_live_count--;
}
static long long sengoo_ipc_error(DWORD code) {
    if (code == ERROR_ACCESS_DENIED || code == ERROR_PRIVILEGE_NOT_HELD) return -SENGOO_STATUS_PERMISSION_DENIED;
    if (code == ERROR_FILE_NOT_FOUND || code == ERROR_PATH_NOT_FOUND) return -SENGOO_STATUS_NOT_FOUND;
    if (code == ERROR_PIPE_BUSY || code == ERROR_SEM_TIMEOUT) return -SENGOO_STATUS_TIMEOUT;
    if (code == ERROR_NOT_ENOUGH_MEMORY || code == ERROR_OUTOFMEMORY) return -SENGOO_STATUS_OUT_OF_MEMORY;
    return -SENGOO_STATUS_IO;
}
static void* sengoo_ipc_token_info(HANDLE token, TOKEN_INFORMATION_CLASS kind) {
    DWORD count = 0; GetTokenInformation(token, kind, NULL, 0, &count);
    if (!count || count > 65536) return NULL;
    void* p = malloc(count);
    if (!p) return NULL;
    if (!GetTokenInformation(token, kind, p, count, &count)) { free(p); return NULL; }
    return p;
}
static int sengoo_ipc_identity(HANDLE process, SengooIpcIdentity* identity) {
    HANDLE token;
    if (!OpenProcessToken(process, TOKEN_QUERY, &token)) return 0;
    TOKEN_USER* user = sengoo_ipc_token_info(token, TokenUser);
    TOKEN_GROUPS* groups = sengoo_ipc_token_info(token, TokenGroups);
    DWORD count; int ok = 0;
    memset(identity, 0, sizeof(*identity));
    if (user && groups && IsValidSid(user->User.Sid) && GetLengthSid(user->User.Sid) <= SECURITY_MAX_SID_SIZE
        && CopySid(SECURITY_MAX_SID_SIZE, identity->user, user->User.Sid)
        && GetTokenInformation(token, TokenSessionId, &identity->session, sizeof(identity->session), &count)) {
        for (DWORD i = 0; i < groups->GroupCount; i++) {
            SID_AND_ATTRIBUTES group = groups->Groups[i];
            if ((group.Attributes & SE_GROUP_LOGON_ID) == SE_GROUP_LOGON_ID && IsValidSid(group.Sid)
                && GetLengthSid(group.Sid) <= SECURITY_MAX_SID_SIZE) {
                ok = CopySid(SECURITY_MAX_SID_SIZE, identity->logon, group.Sid) != 0; break;
            }
        }
    }
    free(user); free(groups); CloseHandle(token); return ok;
}
static int sengoo_ipc_same_identity(const SengooIpcIdentity* expected, const SengooIpcIdentity* actual) {
    return expected->session == actual->session && EqualSid((PSID)expected->user, (PSID)actual->user)
        && EqualSid((PSID)expected->logon, (PSID)actual->logon);
}
static int sengoo_ipc_check_peer(HANDLE pipe, int server, const SengooIpcIdentity* expected, DWORD* peer_pid) {
    ULONG pid;
    if (!(server ? GetNamedPipeClientProcessId(pipe, &pid) : GetNamedPipeServerProcessId(pipe, &pid))) return 0;
    HANDLE peer = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, pid);
    if (!peer) return 0;
    SengooIpcIdentity actual;
    int ok = sengoo_ipc_identity(peer, &actual) && sengoo_ipc_same_identity(expected, &actual);
    CloseHandle(peer); if (ok && peer_pid) *peer_pid = pid; return ok;
}
static long long sengoo_ipc_endpoint(long long bytes, long long length, SengooIpcIdentity* identity, wchar_t* out) {
    if (!bytes || length < 1 || length > 120) return -SENGOO_STATUS_INVALID_ARGUMENT;
    const unsigned char* text = (const unsigned char*)(intptr_t)bytes;
    wchar_t suffix[121];
    for (long long i = 0; i < length; i++) {
        unsigned char c = text[i];
        if (!((c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '.' || c == '_' || c == '-'))
            return -SENGOO_STATUS_INVALID_ARGUMENT;
        suffix[i] = (wchar_t)c;
    }
    suffix[length] = 0;
    if (!sengoo_ipc_identity(GetCurrentProcess(), identity)) return -SENGOO_STATUS_PERMISSION_DENIED;
    wchar_t* sid = NULL;
    if (!ConvertSidToStringSidW(identity->logon, &sid)) return -SENGOO_STATUS_PERMISSION_DENIED;
    int n = swprintf(out, 512, L"\\\\.\\pipe\\Sengoo.Local.v1.%ls.%lu.%ls", sid, identity->session, suffix);
    LocalFree(sid); return n > 0 && n < 512 ? 0 : -SENGOO_STATUS_INVALID_ARGUMENT;
}
static HANDLE sengoo_ipc_new_pipe(SengooIpcListener* listener, int first) {
    /* Only this logon SID receives an ACE; anonymous, other users and a second
     * login of the same user cannot open the pipe. Both peers also check tokens. */
    unsigned char acl_bytes[sizeof(ACL) + sizeof(ACCESS_ALLOWED_ACE) + SECURITY_MAX_SID_SIZE];
    ACL* acl = (ACL*)acl_bytes; SECURITY_DESCRIPTOR descriptor;
    if (!InitializeAcl(acl, sizeof(acl_bytes), ACL_REVISION)
        || !AddAccessAllowedAce(acl, ACL_REVISION, GENERIC_READ | GENERIC_WRITE | SYNCHRONIZE, listener->identity.logon)
        || !InitializeSecurityDescriptor(&descriptor, SECURITY_DESCRIPTOR_REVISION)
        || !SetSecurityDescriptorDacl(&descriptor, TRUE, acl, FALSE)) return INVALID_HANDLE_VALUE;
    SECURITY_ATTRIBUTES security = { sizeof(security), &descriptor, FALSE };
    return CreateNamedPipeW(listener->name, PIPE_ACCESS_DUPLEX | FILE_FLAG_OVERLAPPED | (first ? FILE_FLAG_FIRST_PIPE_INSTANCE : 0),
        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS, SENGOO_IPC_MAX_HANDLES,
        SENGOO_IPC_CHUNK, SENGOO_IPC_CHUNK, 0, &security);
}
static void sengoo_ipc_cancel(HANDLE pipe, OVERLAPPED* operation, int pending) {
    if (!pending) return;
    CancelIoEx(pipe, operation);
    DWORD count;
    /* Completion owns the OVERLAPPED and its byte buffer until this wait ends.
     * Named pipes are local kernel I/O; no application response is required. */
    GetOverlappedResult(pipe, operation, &count, TRUE);
}
static int sengoo_ipc_begin_accept(SengooIpcListener* listener) {
    HANDLE event = listener->connect.hEvent;
    memset(&listener->connect, 0, sizeof(listener->connect)); listener->connect.hEvent = event;
    ResetEvent(event); listener->pending = listener->connected = 0;
    if (ConnectNamedPipe(listener->pipe, &listener->connect)) { listener->connected = 1; return 1; }
    DWORD error = GetLastError();
    if (error == ERROR_IO_PENDING) { listener->pending = 1; return 1; }
    if (error == ERROR_PIPE_CONNECTED) { listener->connected = 1; return 1; }
    return 0;
}
static void sengoo_ipc_destroy_stream(SengooIpcStream* stream) {
    sengoo_ipc_cancel(stream->pipe, &stream->read, stream->read_pending);
    sengoo_ipc_cancel(stream->pipe, &stream->write, stream->write_pending);
    CloseHandle(stream->pipe);
    if (stream->read.hEvent) CloseHandle(stream->read.hEvent);
    if (stream->write.hEvent) CloseHandle(stream->write.hEvent);
    free(stream);
}
static SengooIpcStream* sengoo_ipc_make_stream(HANDLE pipe, DWORD peer) {
    SengooIpcStream* stream = calloc(1, sizeof(*stream));
    if (!stream) return NULL;
    stream->pipe = pipe; stream->peer_pid = peer;
    stream->read.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    stream->write.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (!stream->read.hEvent || !stream->write.hEvent) {
        if (stream->read.hEvent) CloseHandle(stream->read.hEvent);
        if (stream->write.hEvent) CloseHandle(stream->write.hEvent);
        free(stream); return NULL;
    }
    return stream;
}
long long sengoo_ipc_listen(long long bytes, long long length) {
    if (sengoo_ipc_live_count >= SENGOO_IPC_MAX_HANDLES) return -SENGOO_STATUS_OVERFLOW;
    SengooIpcListener* listener = calloc(1, sizeof(*listener));
    if (!listener) return -SENGOO_STATUS_OUT_OF_MEMORY;
    long long status = sengoo_ipc_endpoint(bytes, length, &listener->identity, listener->name);
    if (status < 0) { free(listener); return status; }
    listener->connect.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    if (!listener->connect.hEvent) { free(listener); return -SENGOO_STATUS_OUT_OF_MEMORY; }
    listener->pipe = sengoo_ipc_new_pipe(listener, 1);
    if (listener->pipe == INVALID_HANDLE_VALUE) {
        DWORD error = GetLastError(); CloseHandle(listener->connect.hEvent); free(listener);
        return error == ERROR_ACCESS_DENIED || error == ERROR_PIPE_BUSY ? -SENGOO_STATUS_ALREADY_EXISTS : sengoo_ipc_error(error);
    }
    if (!sengoo_ipc_begin_accept(listener)) {
        CloseHandle(listener->pipe); CloseHandle(listener->connect.hEvent); free(listener); return -SENGOO_STATUS_IO;
    }
    status = sengoo_ipc_register(&listener->base, SG_IPC_LISTENER);
    if (status < 0) {
        sengoo_ipc_cancel(listener->pipe, &listener->connect, listener->pending);
        CloseHandle(listener->pipe); CloseHandle(listener->connect.hEvent); free(listener);
    }
    return status;
}
long long sengoo_ipc_connect(long long bytes, long long length) {
    if (sengoo_ipc_live_count >= SENGOO_IPC_MAX_HANDLES) return -SENGOO_STATUS_OVERFLOW;
    SengooIpcIdentity identity; wchar_t name[512];
    long long status = sengoo_ipc_endpoint(bytes, length, &identity, name);
    if (status < 0) return status;
    HANDLE pipe = CreateFileW(name, GENERIC_READ | GENERIC_WRITE, 0, NULL, OPEN_EXISTING,
        FILE_FLAG_OVERLAPPED | SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION, NULL);
    if (pipe == INVALID_HANDLE_VALUE) return sengoo_ipc_error(GetLastError());
    DWORD pid;
    if (!sengoo_ipc_check_peer(pipe, 0, &identity, &pid)) { CloseHandle(pipe); return -SENGOO_STATUS_PERMISSION_DENIED; }
    SengooIpcStream* stream = sengoo_ipc_make_stream(pipe, pid);
    if (!stream) { CloseHandle(pipe); return -SENGOO_STATUS_OUT_OF_MEMORY; }
    status = sengoo_ipc_register(&stream->base, SG_IPC_STREAM);
    if (status < 0) sengoo_ipc_destroy_stream(stream);
    return status;
}
long long sengoo_ipc_listener_try_accept(long long handle) {
    SengooIpcListener* listener = (SengooIpcListener*)sengoo_ipc_resolve(handle, SG_IPC_LISTENER);
    if (!listener) return -SENGOO_STATUS_INVALID_HANDLE;
    if (sengoo_ipc_live_count >= SENGOO_IPC_MAX_HANDLES) return -SENGOO_STATUS_OVERFLOW;
    if (listener->pending) {
        DWORD count;
        if (!GetOverlappedResult(listener->pipe, &listener->connect, &count, FALSE)) {
            DWORD error = GetLastError();
            if (error == ERROR_IO_INCOMPLETE) return 0;
            listener->pending = 0; DisconnectNamedPipe(listener->pipe);
            sengoo_ipc_begin_accept(listener); return sengoo_ipc_error(error);
        }
        listener->pending = 0; listener->connected = 1;
    }
    if (!listener->connected) return -SENGOO_STATUS_IO;
    DWORD pid;
    if (!sengoo_ipc_check_peer(listener->pipe, 1, &listener->identity, &pid)) {
        DisconnectNamedPipe(listener->pipe); sengoo_ipc_begin_accept(listener); return -SENGOO_STATUS_PERMISSION_DENIED;
    }
    SengooIpcStream* stream = sengoo_ipc_make_stream(listener->pipe, pid);
    if (!stream) return -SENGOO_STATUS_OUT_OF_MEMORY;
    /* Keep the namespace continuously owned while handing off this instance. */
    HANDLE next = sengoo_ipc_new_pipe(listener, 0);
    if (next == INVALID_HANDLE_VALUE) {
        CloseHandle(stream->read.hEvent); CloseHandle(stream->write.hEvent); free(stream); return -SENGOO_STATUS_IO;
    }
    long long result = sengoo_ipc_register(&stream->base, SG_IPC_STREAM);
    if (result < 0) {
        CloseHandle(next); CloseHandle(stream->read.hEvent); CloseHandle(stream->write.hEvent); free(stream); return result;
    }
    listener->pipe = next;
    if (!sengoo_ipc_begin_accept(listener)) {
        /* The accepted byte stream remains valid; future accepts return I/O. */
        listener->pending = listener->connected = 0;
    }
    return result;
}
static long long sengoo_ipc_pump_read(SengooIpcStream* stream) {
    if (stream->eof || stream->read_len > stream->read_offset) return 0;
    DWORD count = 0; BOOL ok;
    if (stream->read_pending) {
        ok = GetOverlappedResult(stream->pipe, &stream->read, &count, FALSE);
        if (!ok && GetLastError() == ERROR_IO_INCOMPLETE) return 0;
        stream->read_pending = 0;
    } else {
        HANDLE event = stream->read.hEvent; memset(&stream->read, 0, sizeof(stream->read));
        stream->read.hEvent = event; ResetEvent(event);
        ok = ReadFile(stream->pipe, stream->read_bytes, SENGOO_IPC_CHUNK, &count, &stream->read);
        if (!ok && GetLastError() == ERROR_IO_PENDING) { stream->read_pending = 1; return 0; }
    }
    if (!ok) {
        DWORD error = GetLastError();
        if (error == ERROR_BROKEN_PIPE || error == ERROR_NO_DATA || error == ERROR_PIPE_NOT_CONNECTED) { stream->eof = 1; return 0; }
        return sengoo_ipc_error(error);
    }
    stream->read_offset = 0; stream->read_len = count;
    if (!count) stream->eof = 1;
    return 0;
}
static long long sengoo_ipc_pump_write(SengooIpcStream* stream) {
    if (stream->write_error) return -stream->write_error;
    DWORD count = 0; BOOL ok;
    if (stream->write_pending) {
        ok = GetOverlappedResult(stream->pipe, &stream->write, &count, FALSE);
        if (!ok && GetLastError() == ERROR_IO_INCOMPLETE) return 0;
        stream->write_pending = 0;
        if (!ok) { stream->write_error = (int)-sengoo_ipc_error(GetLastError()); return -stream->write_error; }
        stream->write_offset += count;
    }
    if (stream->write_offset < stream->write_len) {
        HANDLE event = stream->write.hEvent; memset(&stream->write, 0, sizeof(stream->write));
        stream->write.hEvent = event; ResetEvent(event);
        ok = WriteFile(stream->pipe, stream->write_bytes + stream->write_offset, stream->write_len - stream->write_offset, &count, &stream->write);
        if (!ok && GetLastError() == ERROR_IO_PENDING) { stream->write_pending = 1; return 0; }
        if (!ok) { stream->write_error = (int)-sengoo_ipc_error(GetLastError()); return -stream->write_error; }
        if (!count) { stream->write_error = SENGOO_STATUS_IO; return -stream->write_error; }
        stream->write_offset += count;
    }
    if (stream->write_offset == stream->write_len) stream->write_len = stream->write_offset = 0;
    return 0;
}
long long sengoo_ipc_stream_read(long long handle, long long buffer_handle) {
    SengooIpcStream* stream = (SengooIpcStream*)sengoo_ipc_resolve(handle, SG_IPC_STREAM);
    if (!stream) return -SENGOO_STATUS_INVALID_HANDLE;
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) return -SENGOO_STATUS_INVALID_HANDLE;
    if (!buffer->bytes || !buffer->capacity) return -SENGOO_STATUS_INVALID_ARGUMENT;
    buffer->used_len = 0;
    long long status = sengoo_ipc_pump_read(stream);
    if (status < 0) return status;
    size_t count = stream->read_len - stream->read_offset;
    if (count > buffer->capacity) count = buffer->capacity;
    if (count) memcpy(buffer->bytes, stream->read_bytes + stream->read_offset, count);
    buffer->used_len = count; stream->read_offset += (DWORD)count;
    return (long long)count;
}
long long sengoo_ipc_stream_write(long long handle, long long buffer_handle, long long length) {
    SengooIpcStream* stream = (SengooIpcStream*)sengoo_ipc_resolve(handle, SG_IPC_STREAM);
    if (!stream) return -SENGOO_STATUS_INVALID_HANDLE;
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) return -SENGOO_STATUS_INVALID_HANDLE;
    if (length < 0 || length > SENGOO_IPC_CHUNK || (size_t)length > buffer->used_len || (size_t)length > buffer->capacity
        || (length && !buffer->bytes)) return -SENGOO_STATUS_INVALID_ARGUMENT;
    long long status = sengoo_ipc_pump_write(stream);
    if (status < 0) return status;
    if (stream->write_len || !length) return 0;
    memcpy(stream->write_bytes, buffer->bytes, (size_t)length); stream->write_len = (DWORD)length;
    status = sengoo_ipc_pump_write(stream);
    return status < 0 ? status : length;
}
long long sengoo_ipc_stream_poll(long long handle, long long events, long long timeout_ms) {
    SengooIpcStream* stream = (SengooIpcStream*)sengoo_ipc_resolve(handle, SG_IPC_STREAM);
    if (!stream) return -SENGOO_STATUS_INVALID_HANDLE;
    if (events < 1 || events > 3 || timeout_ms < 0 || timeout_ms > 60000) return -SENGOO_STATUS_INVALID_ARGUMENT;
    ULONGLONG deadline = GetTickCount64() + (ULONGLONG)timeout_ms;
    for (;;) {
        long long status = sengoo_ipc_pump_write(stream);
        if (status < 0) return status;
        if (events & 1) { status = sengoo_ipc_pump_read(stream); if (status < 0) return status; }
        long long ready = 0;
        if ((events & 1) && stream->read_len > stream->read_offset) ready |= 1;
        if ((events & 1) && stream->eof) ready |= 4;
        if ((events & 2) && !stream->write_len) ready |= 2;
        if (ready) return ready;
        ULONGLONG now = GetTickCount64(); if (now >= deadline) return 0;
        HANDLE wait[2]; DWORD count = 0;
        if ((events & 1) && stream->read_pending) wait[count++] = stream->read.hEvent;
        if (stream->write_pending) wait[count++] = stream->write.hEvent;
        if (!count) return 0;
        DWORD result = WaitForMultipleObjects(count, wait, FALSE, (DWORD)(deadline - now));
        if (result == WAIT_TIMEOUT) return 0;
        if (result == WAIT_FAILED) return -SENGOO_STATUS_IO;
    }
}
long long sengoo_ipc_stream_eof(long long handle) {
    SengooIpcStream* stream = (SengooIpcStream*)sengoo_ipc_resolve(handle, SG_IPC_STREAM);
    if (!stream) return -SENGOO_STATUS_INVALID_HANDLE;
    long long status = sengoo_ipc_pump_read(stream);
    return status < 0 ? status : stream->eof && stream->read_len == stream->read_offset;
}
long long sengoo_ipc_stream_peer_pid(long long handle) {
    SengooIpcStream* stream = (SengooIpcStream*)sengoo_ipc_resolve(handle, SG_IPC_STREAM);
    return stream ? (long long)stream->peer_pid : -SENGOO_STATUS_INVALID_HANDLE;
}
long long sengoo_ipc_listener_close(long long handle) {
    if (!handle) return 0;
    SengooIpcListener* listener = (SengooIpcListener*)sengoo_ipc_resolve(handle, SG_IPC_LISTENER);
    if (!listener) return -SENGOO_STATUS_INVALID_HANDLE;
    sengoo_ipc_unregister(&listener->base);
    sengoo_ipc_cancel(listener->pipe, &listener->connect, listener->pending);
    CloseHandle(listener->pipe); CloseHandle(listener->connect.hEvent); free(listener); return 0;
}
long long sengoo_ipc_stream_close(long long handle) {
    if (!handle) return 0;
    SengooIpcStream* stream = (SengooIpcStream*)sengoo_ipc_resolve(handle, SG_IPC_STREAM);
    if (!stream) return -SENGOO_STATUS_INVALID_HANDLE;
    sengoo_ipc_unregister(&stream->base); sengoo_ipc_destroy_stream(stream); return 0;
}
long long sengoo_ipc_live_handle_count(void) { return sengoo_ipc_live_count; }
#else
/* The API stays importable; only Windows same-logon transport is implemented. */
long long sengoo_ipc_listen(long long p, long long n) { (void)p; (void)n; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_ipc_connect(long long p, long long n) { (void)p; (void)n; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_ipc_listener_try_accept(long long h) { (void)h; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_ipc_stream_read(long long h, long long b) { (void)h; (void)b; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_ipc_stream_write(long long h, long long b, long long n) { (void)h; (void)b; (void)n; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_ipc_stream_poll(long long h, long long e, long long t) { (void)h; (void)e; (void)t; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_ipc_stream_eof(long long h) { (void)h; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_ipc_stream_peer_pid(long long h) { (void)h; return -SENGOO_STATUS_UNSUPPORTED; }
long long sengoo_ipc_listener_close(long long h) { return h ? -SENGOO_STATUS_UNSUPPORTED : 0; }
long long sengoo_ipc_stream_close(long long h) { return h ? -SENGOO_STATUS_UNSUPPORTED : 0; }
long long sengoo_ipc_live_handle_count(void) { return 0; }
#endif
