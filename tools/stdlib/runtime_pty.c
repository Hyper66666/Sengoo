#define _CRT_SECURE_NO_WARNINGS
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0A00
#endif
#include "runtime_shared.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
#include <stdio.h>
#include <wchar.h>

/* Generic terminal process ownership only: no terminal emulator or app state. */
#define SG_PTY_CHUNK 65536
#define SG_PTY_MAX_HANDLES 64
#define SG_PTY_MAX_PROCESSES 16
#define SG_PTY_MAX_ARGS 128
#define SG_PTY_MAX_ENV 128
#define SG_PTY_MAX_ENV_CHARS 1048576
#ifndef PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE
#define PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE 0x00020016
#endif
typedef HRESULT (WINAPI *SgCreatePseudoConsole)(COORD, HANDLE, HANDLE, DWORD, HANDLE*);
typedef HRESULT (WINAPI *SgResizePseudoConsole)(HANDLE, COORD);
typedef void (WINAPI *SgClosePseudoConsole)(HANDLE);
static SgCreatePseudoConsole sg_pty_create;
static SgResizePseudoConsole sg_pty_resize;
static SgClosePseudoConsole sg_pty_close_console;
typedef struct SgPtyBase { long long handle; int kind; struct SgPtyBase* next; } SgPtyBase;
typedef struct { wchar_t* name; wchar_t* value; int remove; } SgPtyEnv;
typedef struct {
    SgPtyBase base;
    wchar_t *executable, *cwd, *args[SG_PTY_MAX_ARGS];
    size_t argc, envc;
    int inherit_env;
    SgPtyEnv env[SG_PTY_MAX_ENV];
} SgPtyCommand;
typedef struct {
    SgPtyBase base;
    HANDLE process, job, console, close_thread, input, output;
    OVERLAPPED read, write;
    DWORD read_len, read_offset, write_len, write_offset, exit_code;
    int read_pending, write_pending, eof, exited, input_closed;
    long long read_error, write_error;
    unsigned char read_bytes[SG_PTY_CHUNK], write_bytes[SG_PTY_CHUNK];
} SgPtyProcess;
static SgPtyBase* sg_pty_handles;
static long long sg_pty_live_count, sg_pty_process_count;
static volatile LONG sg_pty_pipe_sequence;
enum { SG_PTY_COMMAND = 1, SG_PTY_PROCESS = 2 };

long long sengoo_pty_available(void) {
    if (sg_pty_create && sg_pty_resize && sg_pty_close_console) return 1;
    HMODULE kernel = GetModuleHandleW(L"kernel32.dll");
    if (!kernel) return 0;
    sg_pty_create = (SgCreatePseudoConsole)GetProcAddress(kernel, "CreatePseudoConsole");
    sg_pty_resize = (SgResizePseudoConsole)GetProcAddress(kernel, "ResizePseudoConsole");
    sg_pty_close_console = (SgClosePseudoConsole)GetProcAddress(kernel, "ClosePseudoConsole");
    return sg_pty_create && sg_pty_resize && sg_pty_close_console;
}
long long sengoo_pty_live_handle_count(void) { return sg_pty_live_count; }
static SgPtyBase* sg_pty_resolve(long long handle, int kind) {
    for (SgPtyBase* at = sg_pty_handles; at; at = at->next)
        if (at->handle == handle && at->kind == kind) return at;
    return NULL;
}
static long long sg_pty_register(SgPtyBase* value, int kind) {
    if (sg_pty_live_count >= SG_PTY_MAX_HANDLES) return -SENGOO_STATUS_OVERFLOW;
    long long handle = sengoo_opaque_handle_new(value);
    if (handle <= 0) return -SENGOO_STATUS_OUT_OF_MEMORY;
    value->handle = handle; value->kind = kind; value->next = sg_pty_handles;
    sg_pty_handles = value; sg_pty_live_count++;
    if (kind == SG_PTY_PROCESS) sg_pty_process_count++;
    return handle;
}
static void sg_pty_unregister(SgPtyBase* value) {
    SgPtyBase** at = &sg_pty_handles;
    while (*at && *at != value) at = &(*at)->next;
    if (*at) *at = value->next;
    sengoo_opaque_handle_take(value->handle); sg_pty_live_count--;
    if (value->kind == SG_PTY_PROCESS) sg_pty_process_count--;
}
static long long sg_pty_error(DWORD error) {
    if (error == ERROR_ACCESS_DENIED || error == ERROR_PRIVILEGE_NOT_HELD) return -SENGOO_STATUS_PERMISSION_DENIED;
    if (error == ERROR_FILE_NOT_FOUND || error == ERROR_PATH_NOT_FOUND) return -SENGOO_STATUS_NOT_FOUND;
    if (error == ERROR_NOT_ENOUGH_MEMORY || error == ERROR_OUTOFMEMORY) return -SENGOO_STATUS_OUT_OF_MEMORY;
    if (error == ERROR_NOT_SUPPORTED || error == ERROR_CALL_NOT_IMPLEMENTED) return -SENGOO_STATUS_UNSUPPORTED;
    return -SENGOO_STATUS_IO;
}
static long long sg_pty_utf16(long long bytes, long long length, int empty, wchar_t** out) {
    *out = NULL;
    if (length < 0 || length > 131068 || (!empty && !length) || (!bytes && length)) return -SENGOO_STATUS_INVALID_ARGUMENT;
    const char* text = (const char*)(intptr_t)bytes;
    if (length && memchr(text, 0, (size_t)length)) return -SENGOO_STATUS_INVALID_ARGUMENT;
    int count = length ? MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, (int)length, NULL, 0) : 0;
    if (length && !count) return -SENGOO_STATUS_INVALID_UTF8;
    if (count > 32766) return -SENGOO_STATUS_OVERFLOW;
    wchar_t* wide = malloc(((size_t)count + 1) * sizeof(wchar_t));
    if (!wide) return -SENGOO_STATUS_OUT_OF_MEMORY;
    if (count && MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, (int)length, wide, count) != count) {
        free(wide); return -SENGOO_STATUS_INVALID_UTF8;
    }
    wide[count] = 0; *out = wide; return 0;
}
static void sg_pty_destroy_command(SgPtyCommand* command) {
    free(command->executable); free(command->cwd);
    for (size_t i = 0; i < command->argc; i++) free(command->args[i]);
    for (size_t i = 0; i < command->envc; i++) { free(command->env[i].name); free(command->env[i].value); }
    free(command);
}
long long sengoo_pty_command_new(long long bytes, long long length) {
    if (!sengoo_pty_available()) return -SENGOO_STATUS_UNSUPPORTED;
    if (sg_pty_live_count >= SG_PTY_MAX_HANDLES) return -SENGOO_STATUS_OVERFLOW;
    SgPtyCommand* command = calloc(1, sizeof(*command));
    if (!command) return -SENGOO_STATUS_OUT_OF_MEMORY;
    long long status = sg_pty_utf16(bytes, length, 0, &command->executable);
    if (status < 0) { free(command); return status; }
    command->inherit_env = 1;
    status = sg_pty_register(&command->base, SG_PTY_COMMAND);
    if (status < 0) sg_pty_destroy_command(command);
    return status;
}
long long sengoo_pty_command_arg(long long handle, long long bytes, long long length) {
    SgPtyCommand* command = (SgPtyCommand*)sg_pty_resolve(handle, SG_PTY_COMMAND);
    if (!command) return -SENGOO_STATUS_INVALID_HANDLE;
    if (command->argc >= SG_PTY_MAX_ARGS) return -SENGOO_STATUS_OVERFLOW;
    wchar_t* wide; long long status = sg_pty_utf16(bytes, length, 1, &wide);
    if (status < 0) return status;
    command->args[command->argc++] = wide; return 0;
}
long long sengoo_pty_command_cwd(long long handle, long long bytes, long long length) {
    SgPtyCommand* command = (SgPtyCommand*)sg_pty_resolve(handle, SG_PTY_COMMAND);
    if (!command) return -SENGOO_STATUS_INVALID_HANDLE;
    wchar_t* wide; long long status = sg_pty_utf16(bytes, length, 0, &wide);
    if (status < 0) return status;
    free(command->cwd); command->cwd = wide; return 0;
}
long long sengoo_pty_command_inherit_env(long long handle, long long enabled) {
    SgPtyCommand* command = (SgPtyCommand*)sg_pty_resolve(handle, SG_PTY_COMMAND);
    if (!command) return -SENGOO_STATUS_INVALID_HANDLE;
    if (enabled != 0 && enabled != 1) return -SENGOO_STATUS_INVALID_ARGUMENT;
    command->inherit_env = (int)enabled; return 0;
}
long long sengoo_pty_command_env(long long handle, long long name, long long name_len,
    long long value, long long value_len, long long remove) {
    SgPtyCommand* command = (SgPtyCommand*)sg_pty_resolve(handle, SG_PTY_COMMAND);
    if (!command) return -SENGOO_STATUS_INVALID_HANDLE;
    if (remove != 0 && remove != 1) return -SENGOO_STATUS_INVALID_ARGUMENT;
    wchar_t *wide_name = NULL, *wide_value = NULL;
    long long status = sg_pty_utf16(name, name_len, 0, &wide_name);
    if (status < 0) return status;
    if (wcschr(wide_name, L'=')) { free(wide_name); return -SENGOO_STATUS_INVALID_ARGUMENT; }
    if (!remove) status = sg_pty_utf16(value, value_len, 1, &wide_value);
    if (status < 0) { free(wide_name); return status; }
    size_t index = 0;
    while (index < command->envc && CompareStringOrdinal(command->env[index].name, -1, wide_name, -1, TRUE) != CSTR_EQUAL) index++;
    if (index == command->envc) {
        if (index >= SG_PTY_MAX_ENV) { free(wide_name); free(wide_value); return -SENGOO_STATUS_OVERFLOW; }
        command->envc++;
    }
    free(command->env[index].name); free(command->env[index].value);
    command->env[index] = (SgPtyEnv){wide_name, wide_value, (int)remove}; return 0;
}
static int sg_pty_append(wchar_t* out, size_t* used, wchar_t ch) {
    if (*used >= 32766) return 0;
    out[(*used)++] = ch; out[*used] = 0; return 1;
}
static int sg_pty_quote(wchar_t* out, size_t* used, const wchar_t* value) {
    if (!sg_pty_append(out, used, L'"')) return 0;
    while (*value) {
        size_t slashes = 0;
        while (*value == L'\\') { slashes++; value++; }
        size_t escaped = (*value == L'"' || !*value) ? slashes * 2 : slashes;
        for (size_t i = 0; i < escaped; i++) if (!sg_pty_append(out, used, L'\\')) return 0;
        if (*value == L'"' && !sg_pty_append(out, used, L'\\')) return 0;
        if (*value && !sg_pty_append(out, used, *value++)) return 0;
    }
    return sg_pty_append(out, used, L'"');
}
static int sg_pty_env_compare(const void* a, const void* b) {
    return CompareStringOrdinal(*(const wchar_t* const*)a, -1, *(const wchar_t* const*)b, -1, TRUE) - CSTR_EQUAL;
}
static long long sg_pty_environment(SgPtyCommand* command, wchar_t** result) {
    LPWCH inherited = command->inherit_env ? GetEnvironmentStringsW() : NULL;
    if (command->inherit_env && !inherited) return -SENGOO_STATUS_IO;
    size_t count = command->envc, total = 1, used = 0;
    if (inherited) for (const wchar_t* at = inherited; *at; at += wcslen(at) + 1) count++;
    if (count > 16384) { if (inherited) FreeEnvironmentStringsW(inherited); return -SENGOO_STATUS_OVERFLOW; }
    wchar_t** entries = calloc(count ? count : 1, sizeof(*entries));
    if (!entries) { if (inherited) FreeEnvironmentStringsW(inherited); return -SENGOO_STATUS_OUT_OF_MEMORY; }
    long long status = 0;
    if (inherited) for (const wchar_t* at = inherited; *at; at += wcslen(at) + 1) {
        const wchar_t* equal = wcschr(at + (*at == L'='), L'=');
        int overridden = 0;
        for (size_t i = 0; equal && i < command->envc; i++)
            if (CompareStringOrdinal(command->env[i].name, -1, at, (int)(equal - at), TRUE) == CSTR_EQUAL) overridden = 1;
        if (overridden) continue;
        size_t length = wcslen(at) + 1;
        if (length > SG_PTY_MAX_ENV_CHARS - total) { status = -SENGOO_STATUS_OVERFLOW; goto finish; }
        entries[used] = malloc(length * sizeof(wchar_t));
        if (!entries[used]) { status = -SENGOO_STATUS_OUT_OF_MEMORY; goto finish; }
        memcpy(entries[used++], at, length * sizeof(wchar_t)); total += length;
    }
    for (size_t i = 0; i < command->envc; i++) {
        SgPtyEnv* edit = &command->env[i]; if (edit->remove) continue;
        size_t names = wcslen(edit->name), values = wcslen(edit->value), length = names + values + 2;
        if (length > SG_PTY_MAX_ENV_CHARS - total) { status = -SENGOO_STATUS_OVERFLOW; goto finish; }
        entries[used] = malloc(length * sizeof(wchar_t));
        if (!entries[used]) { status = -SENGOO_STATUS_OUT_OF_MEMORY; goto finish; }
        memcpy(entries[used], edit->name, names * sizeof(wchar_t)); entries[used][names] = L'=';
        memcpy(entries[used++] + names + 1, edit->value, (values + 1) * sizeof(wchar_t)); total += length;
    }
    qsort(entries, used, sizeof(*entries), sg_pty_env_compare);
    *result = calloc(total + 1, sizeof(wchar_t));
    if (!*result) { status = -SENGOO_STATUS_OUT_OF_MEMORY; goto finish; }
    size_t cursor = 0;
    for (size_t i = 0; i < used; i++) { size_t length = wcslen(entries[i]) + 1; memcpy(*result + cursor, entries[i], length * sizeof(wchar_t)); cursor += length; }
finish:
    for (size_t i = 0; i < used; i++) free(entries[i]);
    free(entries); if (inherited) FreeEnvironmentStringsW(inherited); return status;
}
static int sg_pty_pipe(int input, HANDLE* owner, HANDLE* terminal) {
    /* Only this logon may open the short-lived local pipe namespace. Also
     * validate that the connected endpoint belongs to this exact process. */
    HANDLE token = NULL; TOKEN_GROUPS* groups = NULL; PSID logon = NULL;
    DWORD needed = 0; int ok = 0;
    *owner = *terminal = NULL;
    if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) goto finish;
    GetTokenInformation(token, TokenGroups, NULL, 0, &needed);
    if (!needed || needed > 65536 || !(groups = malloc(needed))) goto finish;
    if (!GetTokenInformation(token, TokenGroups, groups, needed, &needed)) goto finish;
    for (DWORD i = 0; i < groups->GroupCount; i++)
        if ((groups->Groups[i].Attributes & SE_GROUP_LOGON_ID) == SE_GROUP_LOGON_ID) { logon = groups->Groups[i].Sid; break; }
    if (!logon || !IsValidSid(logon)) goto finish;
    unsigned char acl_bytes[sizeof(ACL) + sizeof(ACCESS_ALLOWED_ACE) + SECURITY_MAX_SID_SIZE];
    ACL* acl = (ACL*)acl_bytes; SECURITY_DESCRIPTOR descriptor;
    if (!InitializeAcl(acl, sizeof(acl_bytes), ACL_REVISION)
        || !AddAccessAllowedAce(acl, ACL_REVISION, GENERIC_READ | GENERIC_WRITE | SYNCHRONIZE, logon)
        || !InitializeSecurityDescriptor(&descriptor, SECURITY_DESCRIPTOR_REVISION)
        || !SetSecurityDescriptorDacl(&descriptor, TRUE, acl, FALSE)) goto finish;
    SECURITY_ATTRIBUTES security = {sizeof(security), &descriptor, FALSE};
    wchar_t name[160];
    swprintf(name, 160, L"\\\\.\\pipe\\Sengoo.Pty.v1.%lu.%llu.%lu", GetCurrentProcessId(),
        (unsigned long long)GetTickCount64(), (unsigned long)InterlockedIncrement(&sg_pty_pipe_sequence));
    *owner = CreateNamedPipeW(name, (input ? PIPE_ACCESS_OUTBOUND : PIPE_ACCESS_INBOUND)
        | FILE_FLAG_OVERLAPPED | FILE_FLAG_FIRST_PIPE_INSTANCE,
        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
        1, SG_PTY_CHUNK, SG_PTY_CHUNK, 0, &security);
    if (*owner == INVALID_HANDLE_VALUE) { *owner = NULL; goto finish; }
    *terminal = CreateFileW(name, input ? GENERIC_READ : GENERIC_WRITE, 0, NULL, OPEN_EXISTING,
        SECURITY_SQOS_PRESENT | SECURITY_IDENTIFICATION, NULL);
    if (*terminal == INVALID_HANDLE_VALUE) { *terminal = NULL; goto finish; }
    OVERLAPPED connected; memset(&connected, 0, sizeof(connected));
    if (!ConnectNamedPipe(*owner, &connected) && GetLastError() != ERROR_PIPE_CONNECTED) goto finish;
    ULONG pid;
    if (!GetNamedPipeClientProcessId(*owner, &pid) || pid != GetCurrentProcessId()) goto finish;
    ok = 1;
finish:
    if (token) CloseHandle(token); free(groups);
    if (!ok) { if (*owner) CloseHandle(*owner); if (*terminal) CloseHandle(*terminal); *owner = *terminal = NULL; }
    return ok;
}
static void sg_pty_cancel(HANDLE pipe, OVERLAPPED* operation, int pending) {
    if (!pipe || !pending) return;
    CancelIoEx(pipe, operation); DWORD count;
    GetOverlappedResult(pipe, operation, &count, TRUE);
}
static DWORD WINAPI sg_pty_close_worker(void* console) { sg_pty_close_console((HANDLE)console); return 0; }
static long long sg_pty_update_exit(SgPtyProcess* process) {
    if (!process->exited && WaitForSingleObject(process->process, 0) == WAIT_OBJECT_0) {
        if (!GetExitCodeProcess(process->process, &process->exit_code)) return -SENGOO_STATUS_IO;
        process->exited = 1;
    }
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION accounting;
    if (process->exited && process->console && !QueryInformationJobObject(process->job,
        JobObjectBasicAccountingInformation, &accounting, sizeof(accounting), NULL)) return -SENGOO_STATUS_IO;
    if (process->exited && process->console && !accounting.ActiveProcesses) {
        /* ClosePseudoConsole may wait for its output drain. Only this OS
         * disposal call runs on a worker; owner-thread reads remain usable. */
        process->close_thread = CreateThread(NULL, 0, sg_pty_close_worker, process->console, 0, NULL);
        if (!process->close_thread) return -SENGOO_STATUS_OUT_OF_MEMORY;
        process->console = NULL;
    }
    return process->exited;
}
static void sg_pty_destroy_process(SgPtyProcess* process) {
    if (process->job) TerminateJobObject(process->job, 137);
    if (process->process) WaitForSingleObject(process->process, INFINITE);
    if (process->job) {
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION accounting;
        while (QueryInformationJobObject(process->job, JobObjectBasicAccountingInformation,
            &accounting, sizeof(accounting), NULL) && accounting.ActiveProcesses) Sleep(1);
    }
    sg_pty_cancel(process->input, &process->write, process->write_pending);
    sg_pty_cancel(process->output, &process->read, process->read_pending);
    /* Broken endpoints unblock ConHost even if output was never consumed. */
    if (process->input) CloseHandle(process->input);
    if (process->output) CloseHandle(process->output);
    if (process->console) sg_pty_close_console(process->console);
    if (process->close_thread) { WaitForSingleObject(process->close_thread, INFINITE); CloseHandle(process->close_thread); }
    if (process->read.hEvent) CloseHandle(process->read.hEvent);
    if (process->write.hEvent) CloseHandle(process->write.hEvent);
    if (process->process) CloseHandle(process->process);
    if (process->job) CloseHandle(process->job);
    free(process);
}
long long sengoo_pty_command_spawn(long long handle, long long columns, long long rows) {
    SgPtyCommand* command = (SgPtyCommand*)sg_pty_resolve(handle, SG_PTY_COMMAND);
    if (!command) return -SENGOO_STATUS_INVALID_HANDLE;
    if (!sengoo_pty_available()) return -SENGOO_STATUS_UNSUPPORTED;
    if (columns < 1 || columns > 1000 || rows < 1 || rows > 1000) return -SENGOO_STATUS_INVALID_ARGUMENT;
    if (sg_pty_live_count >= SG_PTY_MAX_HANDLES || sg_pty_process_count >= SG_PTY_MAX_PROCESSES) return -SENGOO_STATUS_OVERFLOW;
    long long status = -SENGOO_STATUS_OUT_OF_MEMORY;
    wchar_t *line = calloc(32768, sizeof(wchar_t)), *environment = NULL;
    SgPtyProcess* process = calloc(1, sizeof(*process));
    HANDLE terminal_input = NULL, terminal_output = NULL;
    STARTUPINFOEXW startup; memset(&startup, 0, sizeof(startup));
    PROCESS_INFORMATION child; memset(&child, 0, sizeof(child));
    if (!line || !process) goto finish;
    size_t used = 0;
    if (!sg_pty_quote(line, &used, command->executable)) { status = -SENGOO_STATUS_OVERFLOW; goto finish; }
    for (size_t i = 0; i < command->argc; i++)
        if (!sg_pty_append(line, &used, L' ') || !sg_pty_quote(line, &used, command->args[i])) { status = -SENGOO_STATUS_OVERFLOW; goto finish; }
    status = sg_pty_environment(command, &environment); if (status < 0) goto finish;
    if (!sg_pty_pipe(1, &process->input, &terminal_input) || !sg_pty_pipe(0, &process->output, &terminal_output)) {
        status = -SENGOO_STATUS_IO; goto finish;
    }
    process->read.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    process->write.hEvent = CreateEventW(NULL, TRUE, FALSE, NULL);
    process->job = CreateJobObjectW(NULL, NULL);
    if (!process->read.hEvent || !process->write.hEvent || !process->job) { status = -SENGOO_STATUS_OUT_OF_MEMORY; goto finish; }
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits; memset(&limits, 0, sizeof(limits));
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (!SetInformationJobObject(process->job, JobObjectExtendedLimitInformation, &limits, sizeof(limits))) { status = -SENGOO_STATUS_IO; goto finish; }
    COORD size = {(SHORT)columns, (SHORT)rows};
    HRESULT created = sg_pty_create(size, terminal_input, terminal_output, 0, &process->console);
    if (FAILED(created)) { status = created == E_NOTIMPL ? -SENGOO_STATUS_UNSUPPORTED : -SENGOO_STATUS_IO; goto finish; }
    CloseHandle(terminal_input); terminal_input = NULL; CloseHandle(terminal_output); terminal_output = NULL;
    SIZE_T attribute_bytes = 0;
    InitializeProcThreadAttributeList(NULL, 1, 0, &attribute_bytes);
    startup.lpAttributeList = malloc(attribute_bytes);
    if (!startup.lpAttributeList) { status = -SENGOO_STATUS_OUT_OF_MEMORY; goto finish; }
    if (!InitializeProcThreadAttributeList(startup.lpAttributeList, 1, 0, &attribute_bytes)) {
        free(startup.lpAttributeList); startup.lpAttributeList = NULL; status = -SENGOO_STATUS_IO; goto finish;
    }
    if (!UpdateProcThreadAttribute(startup.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE,
        process->console, sizeof(HANDLE), NULL, NULL)) { status = -SENGOO_STATUS_IO; goto finish; }
    startup.StartupInfo.cb = sizeof(startup);
    /* Empty explicit standard handles let ConPTY install its console handles.
     * Leaving STARTF_USESTDHANDLES unset can retain the parent's redirected
     * stdout/stderr even with bInheritHandles=FALSE. */
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    if (!CreateProcessW(NULL, line, NULL, NULL, FALSE,
        EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT | CREATE_SUSPENDED,
        environment, command->cwd, &startup.StartupInfo, &child)) { status = sg_pty_error(GetLastError()); goto finish; }
    process->process = child.hProcess;
    if (!AssignProcessToJobObject(process->job, child.hProcess)) {
        TerminateProcess(child.hProcess, 137); status = -SENGOO_STATUS_IO; goto finish;
    }
    if (ResumeThread(child.hThread) == (DWORD)-1) { status = -SENGOO_STATUS_IO; goto finish; }
    status = sg_pty_register(&process->base, SG_PTY_PROCESS);
    if (status > 0) process = NULL;
finish:
    if (child.hThread) CloseHandle(child.hThread);
    if (startup.lpAttributeList) { DeleteProcThreadAttributeList(startup.lpAttributeList); free(startup.lpAttributeList); }
    if (terminal_input) CloseHandle(terminal_input); if (terminal_output) CloseHandle(terminal_output);
    if (process) sg_pty_destroy_process(process);
    free(environment); free(line); return status;
}
static void sg_pty_read_pump(SgPtyProcess* process) {
    if (process->eof || process->read_error || process->read_offset < process->read_len) return;
    DWORD count = 0;
    if (process->read_pending) {
        if (!GetOverlappedResult(process->output, &process->read, &count, FALSE)) {
            DWORD error = GetLastError(); if (error == ERROR_IO_INCOMPLETE) return;
            process->read_pending = 0;
            if (error == ERROR_BROKEN_PIPE || error == ERROR_PIPE_NOT_CONNECTED) process->eof = 1;
            else process->read_error = sg_pty_error(error);
            return;
        }
        process->read_pending = 0; process->read_offset = 0; process->read_len = count;
        if (count) return;
    }
    process->read_offset = process->read_len = 0; ResetEvent(process->read.hEvent);
    if (ReadFile(process->output, process->read_bytes, SG_PTY_CHUNK, &count, &process->read)) process->read_len = count;
    else {
        DWORD error = GetLastError();
        if (error == ERROR_IO_PENDING) process->read_pending = 1;
        else if (error == ERROR_BROKEN_PIPE || error == ERROR_PIPE_NOT_CONNECTED) process->eof = 1;
        else process->read_error = sg_pty_error(error);
    }
}
static void sg_pty_write_pump(SgPtyProcess* process) {
    if (process->input_closed || process->write_error) return;
    DWORD count = 0;
    if (process->write_pending) {
        if (!GetOverlappedResult(process->input, &process->write, &count, FALSE)) {
            DWORD error = GetLastError(); if (error == ERROR_IO_INCOMPLETE) return;
            process->write_pending = 0; process->write_error = sg_pty_error(error); return;
        }
        process->write_pending = 0; process->write_offset += count;
    }
    while (process->write_offset < process->write_len) {
        ResetEvent(process->write.hEvent);
        if (!WriteFile(process->input, process->write_bytes + process->write_offset,
            process->write_len - process->write_offset, &count, &process->write)) {
            DWORD error = GetLastError();
            if (error == ERROR_IO_PENDING) process->write_pending = 1;
            else process->write_error = sg_pty_error(error);
            return;
        }
        if (!count) return;
        process->write_offset += count;
    }
    process->write_offset = process->write_len = 0;
}
long long sengoo_pty_process_read(long long handle, long long buffer) {
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    if (!process) return -SENGOO_STATUS_INVALID_HANDLE;
    SengooFfiBuffer* out = sengoo_ffi_buffer_from_handle(buffer);
    if (!out || (!out->bytes && out->capacity)) return -SENGOO_STATUS_INVALID_HANDLE;
    out->used_len = 0;
    long long status = sg_pty_update_exit(process); if (status < 0) return status;
    sg_pty_read_pump(process); if (process->read_error) return process->read_error;
    size_t count = process->read_len - process->read_offset;
    if (count > out->capacity) count = out->capacity;
    if (count) memcpy(out->bytes, process->read_bytes + process->read_offset, count);
    process->read_offset += (DWORD)count; out->used_len = count; return (long long)count;
}
long long sengoo_pty_process_write(long long handle, long long buffer, long long length) {
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    if (!process) return -SENGOO_STATUS_INVALID_HANDLE;
    SengooFfiBuffer* input = sengoo_ffi_buffer_from_handle(buffer);
    if (!input || (!input->bytes && input->capacity)) return -SENGOO_STATUS_INVALID_HANDLE;
    if (length < 0 || (uint64_t)length > input->used_len || input->used_len > input->capacity) return -SENGOO_STATUS_INVALID_ARGUMENT;
    if (process->input_closed) return -SENGOO_STATUS_IO;
    sg_pty_write_pump(process); if (process->write_error) return process->write_error;
    if (process->write_len) return 0;
    DWORD count = length > SG_PTY_CHUNK ? SG_PTY_CHUNK : (DWORD)length;
    if (count) memcpy(process->write_bytes, input->bytes, count);
    process->write_len = count; process->write_offset = 0;
    sg_pty_write_pump(process); if (process->write_error) return process->write_error;
    return count;
}
long long sengoo_pty_process_poll(long long handle, long long events, long long timeout_ms) {
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    if (!process) return -SENGOO_STATUS_INVALID_HANDLE;
    if (events < 1 || events > 7 || timeout_ms < 0 || timeout_ms > 60000) return -SENGOO_STATUS_INVALID_ARGUMENT;
    ULONGLONG deadline = GetTickCount64() + (ULONGLONG)timeout_ms;
    for (;;) {
        long long status = sg_pty_update_exit(process); if (status < 0) return status;
        sg_pty_read_pump(process); sg_pty_write_pump(process);
        if ((events & 1) && process->read_error) return process->read_error;
        if ((events & 2) && process->write_error) return process->write_error;
        long long ready = 0;
        if ((events & 1) && process->read_offset < process->read_len) ready |= 1;
        if ((events & 2) && !process->input_closed && !process->write_len) ready |= 2;
        if ((events & 1) && process->eof && process->read_offset == process->read_len) ready |= 4;
        if ((events & 4) && process->exited) ready |= 8;
        if (ready || !timeout_ms) return ready;
        ULONGLONG now = GetTickCount64(); if (now >= deadline) return 0;
        HANDLE waiting[3]; DWORD count = 0;
        if ((events & 1) && process->read_pending) waiting[count++] = process->read.hEvent;
        if ((events & 2) && process->write_pending) waiting[count++] = process->write.hEvent;
        if (!process->exited) waiting[count++] = process->process;
        if (!count) return 0;
        DWORD waited = WaitForMultipleObjects(count, waiting, FALSE, (DWORD)(deadline - now));
        if (waited == WAIT_FAILED) return -SENGOO_STATUS_IO;
        if (waited == WAIT_TIMEOUT) return 0;
    }
}
long long sengoo_pty_process_resize(long long handle, long long columns, long long rows) {
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    if (!process) return -SENGOO_STATUS_INVALID_HANDLE;
    if (columns < 1 || columns > 1000 || rows < 1 || rows > 1000) return -SENGOO_STATUS_INVALID_ARGUMENT;
    if (!process->console || process->exited) return -SENGOO_STATUS_IO;
    COORD size = {(SHORT)columns, (SHORT)rows};
    return SUCCEEDED(sg_pty_resize(process->console, size)) ? 0 : -SENGOO_STATUS_IO;
}
long long sengoo_pty_process_try_wait(long long handle) {
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    return process ? sg_pty_update_exit(process) : -SENGOO_STATUS_INVALID_HANDLE;
}
long long sengoo_pty_process_tree_quiescent(long long handle) {
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    if (!process) return -SENGOO_STATUS_INVALID_HANDLE;
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION accounting;
    if (!QueryInformationJobObject(process->job, JobObjectBasicAccountingInformation,
        &accounting, sizeof(accounting), NULL)) return -SENGOO_STATUS_IO;
    return accounting.ActiveProcesses == 0;
}
long long sengoo_pty_process_exit_code(long long handle) {
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    if (!process) return -SENGOO_STATUS_INVALID_HANDLE;
    long long status = sg_pty_update_exit(process);
    return status < 0 ? status : (process->exited ? (long long)process->exit_code : -SENGOO_STATUS_TIMEOUT);
}
long long sengoo_pty_process_eof(long long handle) {
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    if (!process) return -SENGOO_STATUS_INVALID_HANDLE;
    long long status = sg_pty_update_exit(process); if (status < 0) return status;
    sg_pty_read_pump(process); if (process->read_error) return process->read_error;
    return process->eof && process->read_offset == process->read_len;
}
long long sengoo_pty_process_terminate_tree(long long handle) {
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    if (!process) return -SENGOO_STATUS_INVALID_HANDLE;
    return TerminateJobObject(process->job, 137) ? 0 : sg_pty_error(GetLastError());
}
long long sengoo_pty_process_reap(long long handle, long long timeout_ms) {
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    if (!process) return -SENGOO_STATUS_INVALID_HANDLE;
    if (timeout_ms < 0 || timeout_ms > 60000) return -SENGOO_STATUS_INVALID_ARGUMENT;
    DWORD waited = WaitForSingleObject(process->process, (DWORD)timeout_ms);
    if (waited == WAIT_TIMEOUT) return -SENGOO_STATUS_TIMEOUT;
    if (waited != WAIT_OBJECT_0) return -SENGOO_STATUS_IO;
    return sengoo_pty_process_exit_code(handle);
}
long long sengoo_pty_process_close_input(long long handle) {
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    if (!process) return -SENGOO_STATUS_INVALID_HANDLE;
    if (!process->input_closed) {
        sg_pty_cancel(process->input, &process->write, process->write_pending);
        process->write_pending = 0; process->write_offset = process->write_len = 0;
        CloseHandle(process->input); process->input = NULL; process->input_closed = 1;
    }
    return 0;
}
long long sengoo_pty_command_close(long long handle) {
    if (!handle) return 0;
    SgPtyCommand* command = (SgPtyCommand*)sg_pty_resolve(handle, SG_PTY_COMMAND);
    if (!command) return -SENGOO_STATUS_INVALID_HANDLE;
    sg_pty_unregister(&command->base); sg_pty_destroy_command(command); return 0;
}
long long sengoo_pty_process_close(long long handle) {
    if (!handle) return 0;
    SgPtyProcess* process = (SgPtyProcess*)sg_pty_resolve(handle, SG_PTY_PROCESS);
    if (!process) return -SENGOO_STATUS_INVALID_HANDLE;
    sg_pty_unregister(&process->base); sg_pty_destroy_process(process); return 0;
}
#else
long long sengoo_pty_available(void) { return 0; }
long long sengoo_pty_live_handle_count(void) { return 0; }
#define SG_PTY_UNSUPPORTED(name, signature) long long name signature { return -SENGOO_STATUS_UNSUPPORTED; }
SG_PTY_UNSUPPORTED(sengoo_pty_command_new, (long long bytes, long long length))
SG_PTY_UNSUPPORTED(sengoo_pty_command_arg, (long long handle, long long bytes, long long length))
SG_PTY_UNSUPPORTED(sengoo_pty_command_cwd, (long long handle, long long bytes, long long length))
SG_PTY_UNSUPPORTED(sengoo_pty_command_inherit_env, (long long handle, long long enabled))
SG_PTY_UNSUPPORTED(sengoo_pty_command_env, (long long handle, long long name, long long name_len, long long value, long long value_len, long long remove))
SG_PTY_UNSUPPORTED(sengoo_pty_command_spawn, (long long handle, long long columns, long long rows))
SG_PTY_UNSUPPORTED(sengoo_pty_process_read, (long long handle, long long buffer))
SG_PTY_UNSUPPORTED(sengoo_pty_process_write, (long long handle, long long buffer, long long length))
SG_PTY_UNSUPPORTED(sengoo_pty_process_poll, (long long handle, long long events, long long timeout_ms))
SG_PTY_UNSUPPORTED(sengoo_pty_process_resize, (long long handle, long long columns, long long rows))
SG_PTY_UNSUPPORTED(sengoo_pty_process_try_wait, (long long handle))
SG_PTY_UNSUPPORTED(sengoo_pty_process_tree_quiescent, (long long handle))
SG_PTY_UNSUPPORTED(sengoo_pty_process_exit_code, (long long handle))
SG_PTY_UNSUPPORTED(sengoo_pty_process_eof, (long long handle))
SG_PTY_UNSUPPORTED(sengoo_pty_process_terminate_tree, (long long handle))
SG_PTY_UNSUPPORTED(sengoo_pty_process_reap, (long long handle, long long timeout_ms))
SG_PTY_UNSUPPORTED(sengoo_pty_process_close_input, (long long handle))
long long sengoo_pty_command_close(long long handle) { return handle ? -SENGOO_STATUS_UNSUPPORTED : 0; }
long long sengoo_pty_process_close(long long handle) { return handle ? -SENGOO_STATUS_UNSUPPORTED : 0; }
#endif
