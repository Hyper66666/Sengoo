#define _CRT_SECURE_NO_WARNINGS
#if !defined(_WIN32)
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#endif
#endif

#include "runtime_shared.h"

#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <wchar.h>

#ifdef _WIN32
#include <windows.h>
#else
#include <fcntl.h>
#include <pthread.h>
#include <signal.h>
#include <sys/wait.h>
#include <unistd.h>
#endif

typedef struct {
    char* name;
    char* value;
    int remove;
} SengooProcessEnvEdit;

typedef struct {
    char* executable;
    char** args;
    size_t arg_len;
    size_t arg_cap;
    char* cwd;
    SengooProcessEnvEdit* env;
    size_t env_len;
    size_t env_cap;
    int env_clear;
    int capture_stdout;
    int capture_stderr;
    long long timeout_ms;
    long long pipe_stdout_upstream_handle;
    int closed;
} SengooProcessCommand;

typedef struct {
    long long exit_code;
    int timed_out;
    char* stdout_data;
    size_t stdout_len;
    char* stderr_data;
    size_t stderr_len;
    int closed;
} SengooProcessOutput;

/* All three process domains share the runtime's generational token namespace.
 * Resolve through a domain-checked table before touching an allocation. Retain
 * only one metadata slot per concurrent owner, not a tombstone allocation for
 * every completed command. Recently closed handles remain idempotent until the
 * slot is reused, matching ProcessHandle's existing close behavior. */
enum { SENGOO_PROCESS_COMMAND = 1, SENGOO_PROCESS_OUTPUT, SENGOO_PROCESS_CHILD };
typedef struct {
    void* pointer;
    long long handle;
    int domain;
} SengooProcessOwnedSlot;
static SengooProcessOwnedSlot* sengoo_process_owned_slots;
static size_t sengoo_process_owned_count, sengoo_process_owned_capacity;
static long long sengoo_process_owned_new(void* pointer, int domain) {
    size_t index = 0;
    while (index < sengoo_process_owned_count && sengoo_process_owned_slots[index].pointer) index++;
    if (index == sengoo_process_owned_count) {
        if (index == sengoo_process_owned_capacity) {
            size_t capacity = sengoo_process_owned_capacity ? sengoo_process_owned_capacity * 2 : 8;
            if (capacity < sengoo_process_owned_capacity || capacity > SIZE_MAX / sizeof(SengooProcessOwnedSlot)) return 0;
            SengooProcessOwnedSlot* next = realloc(sengoo_process_owned_slots, capacity * sizeof(*next));
            if (!next) return 0;
            sengoo_process_owned_slots = next; sengoo_process_owned_capacity = capacity;
        }
    }
    long long handle = sengoo_opaque_handle_new(pointer);
    if (handle <= 0) return 0;
    if (index == sengoo_process_owned_count) sengoo_process_owned_count++;
    sengoo_process_owned_slots[index] = (SengooProcessOwnedSlot){pointer, handle, domain};
    return handle;
}
static SengooProcessOwnedSlot* sengoo_process_owned_slot(long long handle, int domain) {
    if (handle <= 0) return NULL;
    for (size_t index = 0; index < sengoo_process_owned_count; index++) {
        SengooProcessOwnedSlot* slot = &sengoo_process_owned_slots[index];
        if (slot->handle == handle && slot->domain == domain) return slot;
    }
    return NULL;
}
static void* sengoo_process_owned_get(long long handle, int domain) {
    SengooProcessOwnedSlot* slot = sengoo_process_owned_slot(handle, domain);
    return slot ? slot->pointer : NULL;
}
static void* sengoo_process_owned_take(long long handle, int domain) {
    SengooProcessOwnedSlot* slot = sengoo_process_owned_slot(handle, domain);
    if (!slot || !slot->pointer) return NULL;
    void* pointer = slot->pointer;
    if (sengoo_opaque_handle_take(handle) != pointer) return NULL;
    slot->pointer = NULL;
    return pointer;
}
static SengooProcessCommand* sengoo_process_command_from_handle(long long handle) {
    return (SengooProcessCommand*)sengoo_process_owned_get(handle, SENGOO_PROCESS_COMMAND);
}

static SengooProcessOutput* sengoo_process_output_from_handle(long long handle) {
    return (SengooProcessOutput*)sengoo_process_owned_get(handle, SENGOO_PROCESS_OUTPUT);
}

static int sengoo_process_string_array_push(char*** items, size_t* len, size_t* cap, const char* value) {
    if (!items || !len || !cap || !value) {
        return 0;
    }
    if (*len == *cap) {
        size_t next = *cap == 0 ? 4 : *cap * 2;
        if (next < *cap || next > SIZE_MAX / sizeof(char*)) {
            return 0;
        }
        char** resized = (char**)realloc(*items, next * sizeof(char*));
        if (!resized) {
            return 0;
        }
        *items = resized;
        *cap = next;
    }
    char* copy = sengoo_strdup_bytes(value);
    if (!copy) {
        return 0;
    }
    (*items)[(*len)++] = copy;
    return 1;
}

static void sengoo_process_command_free_fields(SengooProcessCommand* command) {
    if (!command) {
        return;
    }
    free(command->executable);
    command->executable = NULL;
    for (size_t i = 0; i < command->arg_len; ++i) {
        free(command->args[i]);
    }
    free(command->args);
    command->args = NULL;
    command->arg_len = 0;
    command->arg_cap = 0;
    free(command->cwd);
    command->cwd = NULL;
    for (size_t i = 0; i < command->env_len; ++i) {
        free(command->env[i].name);
        free(command->env[i].value);
    }
    free(command->env);
    command->env = NULL;
    command->env_len = 0;
    command->env_cap = 0;
}

static int sengoo_process_command_is_live(SengooProcessCommand* command) {
    return command && !command->closed && command->executable && command->executable[0] != '\0';
}

static int sengoo_process_output_is_live(SengooProcessOutput* output) {
    return output && !output->closed;
}

long long sengoo_process_command_new(long long executable_ptr) {
    const char* executable = (const char*)(intptr_t)executable_ptr;
    if (!executable || executable[0] == '\0') {
        return 0;
    }
    SengooProcessCommand* command = (SengooProcessCommand*)calloc(1, sizeof(SengooProcessCommand));
    if (!command) {
        return 0;
    }
    command->executable = sengoo_strdup_bytes(executable);
    command->timeout_ms = -1;
    if (!command->executable) {
        free(command);
        return 0;
    }
    long long handle = sengoo_process_owned_new(command, SENGOO_PROCESS_COMMAND);
    if (!handle) { sengoo_process_command_free_fields(command); free(command); }
    return handle;
}

long long sengoo_process_command_arg(long long handle, long long arg_ptr) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    const char* arg = (const char*)(intptr_t)arg_ptr;
    if (!sengoo_process_command_is_live(command) || !arg) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    return sengoo_process_string_array_push(&command->args, &command->arg_len, &command->arg_cap, arg)
        ? 1
        : -SENGOO_STATUS_OUT_OF_MEMORY;
}

long long sengoo_process_command_cwd(long long handle, long long cwd_ptr) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    const char* cwd = (const char*)(intptr_t)cwd_ptr;
    if (!sengoo_process_command_is_live(command) || !cwd || cwd[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    char* copy = sengoo_strdup_bytes(cwd);
    if (!copy) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    free(command->cwd);
    command->cwd = copy;
    return 1;
}

static long long sengoo_process_command_find_env(SengooProcessCommand* command, const char* name) {
    if (!command || !name) {
        return -1;
    }
    for (size_t i = 0; i < command->env_len; ++i) {
        if (strcmp(command->env[i].name, name) == 0) {
            return (long long)i;
        }
    }
    return -1;
}

static int sengoo_process_command_reserve_env(SengooProcessCommand* command) {
    if (!command) {
        return 0;
    }
    if (command->env_len < command->env_cap) {
        return 1;
    }
    size_t next = command->env_cap == 0 ? 4 : command->env_cap * 2;
    if (next < command->env_cap || next > SIZE_MAX / sizeof(SengooProcessEnvEdit)) {
        return 0;
    }
    SengooProcessEnvEdit* env = (SengooProcessEnvEdit*)realloc(command->env, next * sizeof(SengooProcessEnvEdit));
    if (!env) {
        return 0;
    }
    memset(env + command->env_cap, 0, (next - command->env_cap) * sizeof(SengooProcessEnvEdit));
    command->env = env;
    command->env_cap = next;
    return 1;
}

long long sengoo_process_command_env_set(long long handle, long long name_ptr, long long value_ptr) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    const char* name = (const char*)(intptr_t)name_ptr;
    const char* value = (const char*)(intptr_t)value_ptr;
    if (!sengoo_process_command_is_live(command) || !name || name[0] == '\0' || strchr(name, '=') || !value) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    char* name_copy = sengoo_strdup_bytes(name);
    char* value_copy = sengoo_strdup_bytes(value);
    if (!name_copy || !value_copy) {
        free(name_copy);
        free(value_copy);
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    long long existing = sengoo_process_command_find_env(command, name);
    if (existing >= 0) {
        SengooProcessEnvEdit* edit = &command->env[(size_t)existing];
        free(edit->name);
        free(edit->value);
        edit->name = name_copy;
        edit->value = value_copy;
        edit->remove = 0;
        return 1;
    }
    if (!sengoo_process_command_reserve_env(command)) {
        free(name_copy);
        free(value_copy);
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    command->env[command->env_len].name = name_copy;
    command->env[command->env_len].value = value_copy;
    command->env[command->env_len].remove = 0;
    command->env_len += 1;
    return 1;
}

long long sengoo_process_command_env_remove(long long handle, long long name_ptr) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    const char* name = (const char*)(intptr_t)name_ptr;
    if (!sengoo_process_command_is_live(command) || !name || name[0] == '\0' || strchr(name, '=')) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    char* name_copy = sengoo_strdup_bytes(name);
    if (!name_copy) {
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    long long existing = sengoo_process_command_find_env(command, name);
    if (existing >= 0) {
        SengooProcessEnvEdit* edit = &command->env[(size_t)existing];
        free(edit->name);
        free(edit->value);
        edit->name = name_copy;
        edit->value = NULL;
        edit->remove = 1;
        return 1;
    }
    if (!sengoo_process_command_reserve_env(command)) {
        free(name_copy);
        return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    command->env[command->env_len].name = name_copy;
    command->env[command->env_len].value = NULL;
    command->env[command->env_len].remove = 1;
    command->env_len += 1;
    return 1;
}

long long sengoo_process_command_env_clear(long long handle) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    if (!sengoo_process_command_is_live(command)) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    command->env_clear = 1;
    return 1;
}

long long sengoo_process_command_capture_stdout(long long handle, long long capture) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    if (!sengoo_process_command_is_live(command)) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    command->capture_stdout = capture != 0;
    return 1;
}

long long sengoo_process_command_capture_stderr(long long handle, long long capture) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    if (!sengoo_process_command_is_live(command)) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    command->capture_stderr = capture != 0;
    return 1;
}

long long sengoo_process_command_timeout_ms(long long handle, long long timeout_ms) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    if (!sengoo_process_command_is_live(command) || timeout_ms < 0) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    command->timeout_ms = timeout_ms;
    return 1;
}

static int sengoo_process_bytes_append(char** data, size_t* len, size_t* cap, const char* chunk, size_t chunk_len) {
    if (!data || !len || !cap || (chunk_len > 0 && !chunk)) {
        return 0;
    }
    if (chunk_len > SIZE_MAX - *len) {
        return 0;
    }
    size_t needed = *len + chunk_len;
    if (*cap < needed) {
        size_t next = *cap == 0 ? 64 : *cap;
        while (next < needed) {
            if (next > SIZE_MAX / 2) {
                return 0;
            }
            next *= 2;
        }
        char* resized = (char*)realloc(*data, next);
        if (!resized) {
            return 0;
        }
        *data = resized;
        *cap = next;
    }
    if (chunk_len > 0) {
        memcpy(*data + *len, chunk, chunk_len);
        *len += chunk_len;
    }
    return 1;
}

static long long sengoo_process_now_ms(void) {
#ifdef _WIN32
    return (long long)GetTickCount64();
#else
    struct timespec now;
    if (clock_gettime(CLOCK_MONOTONIC, &now) != 0) return sengoo_time_unix_ms();
    return (long long)now.tv_sec * 1000 + now.tv_nsec / 1000000;
#endif
}

static void sengoo_process_sleep_short(void) {
#ifdef _WIN32
    Sleep(5);
#else
    struct timespec req;
    req.tv_sec = 0;
    req.tv_nsec = 5L * 1000L * 1000L;
    nanosleep(&req, NULL);
#endif
}

static char** sengoo_process_build_argv(SengooProcessCommand* command) {
    if (!sengoo_process_command_is_live(command) || command->arg_len > (SIZE_MAX / sizeof(char*)) - 2) {
        return NULL;
    }
    char** argv = (char**)calloc(command->arg_len + 2, sizeof(char*));
    if (!argv) {
        return NULL;
    }
    argv[0] = command->executable;
    for (size_t i = 0; i < command->arg_len; ++i) {
        argv[i + 1] = command->args[i];
    }
    argv[command->arg_len + 1] = NULL;
    return argv;
}

static SengooProcessOutput* sengoo_process_output_new(void) {
    return (SengooProcessOutput*)calloc(1, sizeof(SengooProcessOutput));
}

static long long sengoo_process_output_register(SengooProcessOutput* output) {
    long long handle = sengoo_process_owned_new(output, SENGOO_PROCESS_OUTPUT);
    if (!handle) { free(output->stdout_data); free(output->stderr_data); free(output); }
    return handle;
}

#ifdef _WIN32
static char* sengoo_windows_process_command_line_dyn(SengooProcessCommand* command) {
    if (!sengoo_process_command_is_live(command)) {
        return NULL;
    }
    size_t capacity = 1;
    if (strlen(command->executable) > (SIZE_MAX - 3) / 2
        || sengoo_size_add(&capacity, strlen(command->executable) * 2 + 3) != 0) {
        return NULL;
    }
    for (size_t i = 0; i < command->arg_len; ++i) {
        size_t len = strlen(command->args[i]);
        if (len > (SIZE_MAX - 3) / 2 || sengoo_size_add(&capacity, len * 2 + 3) != 0) {
            return NULL;
        }
    }
    char* command_line = (char*)malloc(capacity);
    if (!command_line) {
        return NULL;
    }
    char* out = command_line;
    out = sengoo_windows_append_arg(out, command->executable);
    for (size_t i = 0; i < command->arg_len; ++i) {
        *out++ = ' ';
        out = sengoo_windows_append_arg(out, command->args[i]);
    }
    *out = '\0';
    return command_line;
}

static int sengoo_process_env_edit_matches(const char* edit_name, const char* entry, size_t entry_name_len) {
    return edit_name
        && strlen(edit_name) == entry_name_len
        && _strnicmp(edit_name, entry, entry_name_len) == 0;
}

static SengooProcessEnvEdit* sengoo_process_find_env_edit_for_entry(SengooProcessCommand* command, const char* entry) {
    const char* equals = strchr(entry, '=');
    if (!equals || equals == entry) {
        return NULL;
    }
    size_t name_len = (size_t)(equals - entry);
    for (size_t i = 0; i < command->env_len; ++i) {
        if (sengoo_process_env_edit_matches(command->env[i].name, entry, name_len)) {
            return &command->env[i];
        }
    }
    return NULL;
}

static int sengoo_process_env_block_append(char** block, size_t* len, size_t* cap, const char* entry) {
    size_t entry_len = strlen(entry) + 1u;
    return sengoo_process_bytes_append(block, len, cap, entry, entry_len);
}

static char* sengoo_process_build_windows_env_block(SengooProcessCommand* command) {
    if (!command->env_clear && command->env_len == 0) {
        return NULL;
    }
    char* block = NULL;
    size_t len = 0;
    size_t cap = 0;

    if (!command->env_clear) {
        LPCH inherited = GetEnvironmentStringsA();
        if (!inherited) {
            return NULL;
        }
        for (const char* entry = inherited; *entry != '\0'; entry += strlen(entry) + 1u) {
            SengooProcessEnvEdit* edit = sengoo_process_find_env_edit_for_entry(command, entry);
            if (edit) {
                continue;
            }
            if (!sengoo_process_env_block_append(&block, &len, &cap, entry)) {
                FreeEnvironmentStringsA(inherited);
                free(block);
                return NULL;
            }
        }
        FreeEnvironmentStringsA(inherited);
    }

    for (size_t i = 0; i < command->env_len; ++i) {
        if (command->env[i].remove) {
            continue;
        }
        size_t name_len = strlen(command->env[i].name);
        size_t value_len = strlen(command->env[i].value);
        if (name_len > SIZE_MAX - value_len - 2u) {
            free(block);
            return NULL;
        }
        size_t entry_len = name_len + 1u + value_len;
        char* entry = (char*)malloc(entry_len + 1u);
        if (!entry) {
            free(block);
            return NULL;
        }
        memcpy(entry, command->env[i].name, name_len);
        entry[name_len] = '=';
        memcpy(entry + name_len + 1u, command->env[i].value, value_len);
        entry[entry_len] = '\0';
        int appended = sengoo_process_env_block_append(&block, &len, &cap, entry);
        free(entry);
        if (!appended) {
            free(block);
            return NULL;
        }
    }

    char zero = '\0';
    if (!sengoo_process_bytes_append(&block, &len, &cap, &zero, 1)) {
        free(block);
        return NULL;
    }
    return block;
}

static int sengoo_process_windows_pipe_read_available(HANDLE handle, char** data, size_t* len, size_t* cap) {
    if (handle == NULL || handle == INVALID_HANDLE_VALUE) {
        return 1;
    }
    DWORD available = 0;
    if (!PeekNamedPipe(handle, NULL, 0, NULL, &available, NULL)) {
        return 1;
    }
    while (available > 0) {
        DWORD chunk = available > 4096 ? 4096 : available;
        char buffer[4096];
        DWORD read = 0;
        if (!ReadFile(handle, buffer, chunk, &read, NULL) || read == 0) {
            return 1;
        }
        if (!sengoo_process_bytes_append(data, len, cap, buffer, (size_t)read)) {
            return 0;
        }
        available -= read;
    }
    return 1;
}

static long long sengoo_process_command_run_platform_with_stdin(
    SengooProcessCommand* command,
    const char* stdin_data,
    size_t stdin_len) {
    if (stdin_len > 0 && !stdin_data) {
        return 0;
    }
    char* command_line = sengoo_windows_process_command_line_dyn(command);
    if (!command_line) {
        return 0;
    }

    SECURITY_ATTRIBUTES security;
    memset(&security, 0, sizeof(security));
    security.nLength = sizeof(security);
    security.bInheritHandle = TRUE;

    HANDLE stdout_read = NULL;
    HANDLE stdout_write = NULL;
    HANDLE stderr_read = NULL;
    HANDLE stderr_write = NULL;
    HANDLE stdin_read = NULL;
    HANDLE stdin_write = NULL;
    int provide_stdin = stdin_data != NULL;
    if (provide_stdin) {
        if (!CreatePipe(&stdin_read, &stdin_write, &security, 0)
            || !SetHandleInformation(stdin_write, HANDLE_FLAG_INHERIT, 0)) {
            if (stdin_read) CloseHandle(stdin_read);
            if (stdin_write) CloseHandle(stdin_write);
            free(command_line);
            return 0;
        }
    }
    if (command->capture_stdout) {
        if (!CreatePipe(&stdout_read, &stdout_write, &security, 0)
            || !SetHandleInformation(stdout_read, HANDLE_FLAG_INHERIT, 0)) {
            if (stdin_read) CloseHandle(stdin_read);
            if (stdin_write) CloseHandle(stdin_write);
            free(command_line);
            return 0;
        }
    }
    if (command->capture_stderr) {
        if (!CreatePipe(&stderr_read, &stderr_write, &security, 0)
            || !SetHandleInformation(stderr_read, HANDLE_FLAG_INHERIT, 0)) {
            if (stdout_read) CloseHandle(stdout_read);
            if (stdout_write) CloseHandle(stdout_write);
            if (stdin_read) CloseHandle(stdin_read);
            if (stdin_write) CloseHandle(stdin_write);
            free(command_line);
            return 0;
        }
    }

    char* env_block = sengoo_process_build_windows_env_block(command);
    STARTUPINFOA startup_info;
    PROCESS_INFORMATION process_info;
    memset(&startup_info, 0, sizeof(startup_info));
    memset(&process_info, 0, sizeof(process_info));
    startup_info.cb = sizeof(startup_info);
    startup_info.dwFlags = STARTF_USESTDHANDLES;
    startup_info.hStdInput = provide_stdin ? stdin_read : GetStdHandle(STD_INPUT_HANDLE);
    startup_info.hStdOutput = command->capture_stdout ? stdout_write : GetStdHandle(STD_OUTPUT_HANDLE);
    startup_info.hStdError = command->capture_stderr ? stderr_write : GetStdHandle(STD_ERROR_HANDLE);

    BOOL created = CreateProcessA(
        NULL,
        command_line,
        NULL,
        NULL,
        TRUE,
        0,
        env_block,
        command->cwd,
        &startup_info,
        &process_info
    );
    free(command_line);
    free(env_block);
    if (stdout_write) {
        CloseHandle(stdout_write);
    }
    if (stderr_write) {
        CloseHandle(stderr_write);
    }
    if (stdin_read) {
        CloseHandle(stdin_read);
    }
    if (!created) {
        if (stdout_read) CloseHandle(stdout_read);
        if (stderr_read) CloseHandle(stderr_read);
        if (stdin_write) CloseHandle(stdin_write);
        return 0;
    }

    if (stdin_write) {
        size_t offset = 0;
        while (offset < stdin_len) {
            DWORD chunk = (DWORD)((stdin_len - offset) > (size_t)MAXDWORD
                ? MAXDWORD
                : (stdin_len - offset));
            DWORD written = 0;
            if (!WriteFile(stdin_write, stdin_data + offset, chunk, &written, NULL)
                || written == 0) {
                break;
            }
            offset += (size_t)written;
        }
        CloseHandle(stdin_write);
    }

    SengooProcessOutput* output = sengoo_process_output_new();
    if (!output) {
        TerminateProcess(process_info.hProcess, 1);
        CloseHandle(process_info.hThread);
        CloseHandle(process_info.hProcess);
        if (stdout_read) CloseHandle(stdout_read);
        if (stderr_read) CloseHandle(stderr_read);
        return 0;
    }

    size_t stdout_cap = 0;
    size_t stderr_cap = 0;
    long long start_ms = sengoo_process_now_ms();
    for (;;) {
        DWORD wait_status = WaitForSingleObject(process_info.hProcess, 0);
        if (!sengoo_process_windows_pipe_read_available(stdout_read, &output->stdout_data, &output->stdout_len, &stdout_cap)
            || !sengoo_process_windows_pipe_read_available(stderr_read, &output->stderr_data, &output->stderr_len, &stderr_cap)) {
            output->exit_code = -1;
            break;
        }
        if (wait_status == WAIT_OBJECT_0) {
            break;
        }
        if (wait_status != WAIT_TIMEOUT) {
            output->exit_code = -1;
            break;
        }
        if (command->timeout_ms >= 0 && sengoo_process_now_ms() - start_ms >= command->timeout_ms) {
            output->timed_out = 1;
            TerminateProcess(process_info.hProcess, 1);
            WaitForSingleObject(process_info.hProcess, INFINITE);
            break;
        }
        sengoo_process_sleep_short();
    }

    sengoo_process_windows_pipe_read_available(stdout_read, &output->stdout_data, &output->stdout_len, &stdout_cap);
    sengoo_process_windows_pipe_read_available(stderr_read, &output->stderr_data, &output->stderr_len, &stderr_cap);
    if (!output->timed_out && output->exit_code >= 0) {
        DWORD exit_code = 0;
        if (GetExitCodeProcess(process_info.hProcess, &exit_code)) {
            output->exit_code = (long long)exit_code;
        } else {
            output->exit_code = -1;
        }
    }

    CloseHandle(process_info.hThread);
    CloseHandle(process_info.hProcess);
    if (stdout_read) CloseHandle(stdout_read);
    if (stderr_read) CloseHandle(stderr_read);
    return sengoo_process_output_register(output);
}
#else
static char** sengoo_process_build_clear_envp(SengooProcessCommand* command) {
    size_t count = 0;
    for (size_t i = 0; i < command->env_len; ++i) {
        if (!command->env[i].remove) {
            count += 1;
        }
    }
    char** envp = (char**)calloc(count + 1u, sizeof(char*));
    if (!envp) {
        return NULL;
    }
    size_t pos = 0;
    for (size_t i = 0; i < command->env_len; ++i) {
        if (command->env[i].remove) {
            continue;
        }
        size_t name_len = strlen(command->env[i].name);
        size_t value_len = strlen(command->env[i].value);
        if (name_len > SIZE_MAX - value_len - 2u) {
            for (size_t j = 0; j < pos; ++j) free(envp[j]);
            free(envp);
            return NULL;
        }
        size_t len = name_len + 1u + value_len;
        envp[pos] = (char*)malloc(len + 1u);
        if (!envp[pos]) {
            for (size_t j = 0; j < pos; ++j) free(envp[j]);
            free(envp);
            return NULL;
        }
        memcpy(envp[pos], command->env[i].name, name_len);
        envp[pos][name_len] = '=';
        memcpy(envp[pos] + name_len + 1u, command->env[i].value, value_len);
        envp[pos][len] = '\0';
        pos += 1;
    }
    envp[pos] = NULL;
    return envp;
}

static void sengoo_process_free_envp(char** envp) {
    if (!envp) {
        return;
    }
    for (size_t i = 0; envp[i]; ++i) {
        free(envp[i]);
    }
    free(envp);
}

static int sengoo_process_make_nonblocking(int fd) {
    if (fd < 0) {
        return 1;
    }
    int flags = fcntl(fd, F_GETFL, 0);
    return flags >= 0 && fcntl(fd, F_SETFL, flags | O_NONBLOCK) == 0;
}

static int sengoo_process_read_available_fd(int fd, char** data, size_t* len, size_t* cap, int* eof) {
    if (fd < 0) {
        if (eof) *eof = 1;
        return 1;
    }
    char buffer[4096];
    for (;;) {
        ssize_t read_count = read(fd, buffer, sizeof(buffer));
        if (read_count > 0) {
            if (!sengoo_process_bytes_append(data, len, cap, buffer, (size_t)read_count)) {
                return 0;
            }
            continue;
        }
        if (read_count == 0) {
            if (eof) *eof = 1;
            return 1;
        }
        if (errno == EAGAIN || errno == EWOULDBLOCK) {
            return 1;
        }
        if (errno == EINTR) {
            continue;
        }
        return 0;
    }
}

static long long sengoo_process_command_run_platform_with_stdin(
    SengooProcessCommand* command,
    const char* stdin_data,
    size_t stdin_len) {
    if (stdin_len > 0 && !stdin_data) {
        return 0;
    }
    char** argv = sengoo_process_build_argv(command);
    if (!argv) {
        return 0;
    }
    char** clear_envp = command->env_clear ? sengoo_process_build_clear_envp(command) : NULL;
    if (command->env_clear && !clear_envp) {
        free(argv);
        return 0;
    }

    int startup_pipe[2] = {-1, -1};
    int stdin_pipe[2] = {-1, -1};
    int stdout_pipe[2] = {-1, -1};
    int stderr_pipe[2] = {-1, -1};
    if (pipe(startup_pipe) != 0) {
        free(argv);
        sengoo_process_free_envp(clear_envp);
        return 0;
    }
    int provide_stdin = stdin_data != NULL;
    if (provide_stdin && pipe(stdin_pipe) != 0) {
        close(startup_pipe[0]);
        close(startup_pipe[1]);
        free(argv);
        sengoo_process_free_envp(clear_envp);
        return 0;
    }
    int flags = fcntl(startup_pipe[1], F_GETFD);
    if (flags < 0 || fcntl(startup_pipe[1], F_SETFD, flags | FD_CLOEXEC) != 0) {
        close(startup_pipe[0]);
        close(startup_pipe[1]);
        if (stdin_pipe[0] >= 0) close(stdin_pipe[0]);
        if (stdin_pipe[1] >= 0) close(stdin_pipe[1]);
        free(argv);
        sengoo_process_free_envp(clear_envp);
        return 0;
    }
    if (command->capture_stdout && pipe(stdout_pipe) != 0) {
        close(startup_pipe[0]);
        close(startup_pipe[1]);
        if (stdin_pipe[0] >= 0) close(stdin_pipe[0]);
        if (stdin_pipe[1] >= 0) close(stdin_pipe[1]);
        free(argv);
        sengoo_process_free_envp(clear_envp);
        return 0;
    }
    if (command->capture_stderr && pipe(stderr_pipe) != 0) {
        close(startup_pipe[0]);
        close(startup_pipe[1]);
        if (stdin_pipe[0] >= 0) close(stdin_pipe[0]);
        if (stdin_pipe[1] >= 0) close(stdin_pipe[1]);
        if (stdout_pipe[0] >= 0) close(stdout_pipe[0]);
        if (stdout_pipe[1] >= 0) close(stdout_pipe[1]);
        free(argv);
        sengoo_process_free_envp(clear_envp);
        return 0;
    }

    pid_t pid = fork();
    if (pid < 0) {
        close(startup_pipe[0]);
        close(startup_pipe[1]);
        if (stdin_pipe[0] >= 0) close(stdin_pipe[0]);
        if (stdin_pipe[1] >= 0) close(stdin_pipe[1]);
        if (stdout_pipe[0] >= 0) close(stdout_pipe[0]);
        if (stdout_pipe[1] >= 0) close(stdout_pipe[1]);
        if (stderr_pipe[0] >= 0) close(stderr_pipe[0]);
        if (stderr_pipe[1] >= 0) close(stderr_pipe[1]);
        free(argv);
        sengoo_process_free_envp(clear_envp);
        return 0;
    }

    if (pid == 0) {
        close(startup_pipe[0]);
        if (provide_stdin) {
            close(stdin_pipe[1]);
            dup2(stdin_pipe[0], STDIN_FILENO);
            close(stdin_pipe[0]);
        }
        if (command->capture_stdout) {
            close(stdout_pipe[0]);
            dup2(stdout_pipe[1], STDOUT_FILENO);
            close(stdout_pipe[1]);
        }
        if (command->capture_stderr) {
            close(stderr_pipe[0]);
            dup2(stderr_pipe[1], STDERR_FILENO);
            close(stderr_pipe[1]);
        }
        if (command->cwd && chdir(command->cwd) != 0) {
            int startup_errno = errno;
            (void)write(startup_pipe[1], &startup_errno, sizeof(startup_errno));
            _exit(127);
        }
        if (command->env_clear) {
            execve(command->executable, argv, clear_envp);
        } else {
            for (size_t i = 0; i < command->env_len; ++i) {
                if (command->env[i].remove) {
                    unsetenv(command->env[i].name);
                } else {
                    setenv(command->env[i].name, command->env[i].value, 1);
                }
            }
            execvp(command->executable, argv);
        }
        int startup_errno = errno;
        (void)write(startup_pipe[1], &startup_errno, sizeof(startup_errno));
        _exit(127);
    }

    close(startup_pipe[1]);
    if (stdin_pipe[0] >= 0) close(stdin_pipe[0]);
    if (stdout_pipe[1] >= 0) close(stdout_pipe[1]);
    if (stderr_pipe[1] >= 0) close(stderr_pipe[1]);
    sengoo_process_make_nonblocking(stdout_pipe[0]);
    sengoo_process_make_nonblocking(stderr_pipe[0]);

    int startup_errno = 0;
    ssize_t startup_read;
    do {
        startup_read = read(startup_pipe[0], &startup_errno, sizeof(startup_errno));
    } while (startup_read < 0 && errno == EINTR);
    close(startup_pipe[0]);
    if (startup_read != 0) {
        int status = 0;
        waitpid(pid, &status, 0);
        if (stdin_pipe[1] >= 0) close(stdin_pipe[1]);
        if (stdout_pipe[0] >= 0) close(stdout_pipe[0]);
        if (stderr_pipe[0] >= 0) close(stderr_pipe[0]);
        free(argv);
        sengoo_process_free_envp(clear_envp);
        return 0;
    }

    if (stdin_pipe[1] >= 0) {
        size_t offset = 0;
        int write_ok = 1;
        while (offset < stdin_len) {
            ssize_t written = write(stdin_pipe[1], stdin_data + offset, stdin_len - offset);
            if (written > 0) {
                offset += (size_t)written;
                continue;
            }
            if (written < 0 && errno == EINTR) {
                continue;
            }
            write_ok = 0;
            break;
        }
        close(stdin_pipe[1]);
        if (!write_ok) {
            kill(pid, SIGKILL);
            waitpid(pid, NULL, 0);
            if (stdout_pipe[0] >= 0) close(stdout_pipe[0]);
            if (stderr_pipe[0] >= 0) close(stderr_pipe[0]);
            free(argv);
            sengoo_process_free_envp(clear_envp);
            return 0;
        }
    }

    SengooProcessOutput* output = sengoo_process_output_new();
    if (!output) {
        kill(pid, SIGKILL);
        waitpid(pid, NULL, 0);
        if (stdout_pipe[0] >= 0) close(stdout_pipe[0]);
        if (stderr_pipe[0] >= 0) close(stderr_pipe[0]);
        free(argv);
        sengoo_process_free_envp(clear_envp);
        return 0;
    }

    size_t stdout_cap = 0;
    size_t stderr_cap = 0;
    int stdout_eof = stdout_pipe[0] < 0;
    int stderr_eof = stderr_pipe[0] < 0;
    long long start_ms = sengoo_process_now_ms();
    int status = 0;
    int exited = 0;
    while (!exited || !stdout_eof || !stderr_eof) {
        if (!sengoo_process_read_available_fd(stdout_pipe[0], &output->stdout_data, &output->stdout_len, &stdout_cap, &stdout_eof)
            || !sengoo_process_read_available_fd(stderr_pipe[0], &output->stderr_data, &output->stderr_len, &stderr_cap, &stderr_eof)) {
            output->exit_code = -1;
            break;
        }
        if (!exited) {
            pid_t waited = waitpid(pid, &status, WNOHANG);
            if (waited == pid) {
                exited = 1;
            } else if (waited < 0 && errno != EINTR) {
                output->exit_code = -1;
                exited = 1;
            }
        }
        if (!exited && command->timeout_ms >= 0 && sengoo_process_now_ms() - start_ms >= command->timeout_ms) {
            output->timed_out = 1;
            kill(pid, SIGKILL);
            waitpid(pid, &status, 0);
            exited = 1;
        }
        if (!exited || !stdout_eof || !stderr_eof) {
            sengoo_process_sleep_short();
        }
    }
    if (!output->timed_out && output->exit_code >= 0) {
        if (WIFEXITED(status)) {
            output->exit_code = (long long)WEXITSTATUS(status);
        } else if (WIFSIGNALED(status)) {
            output->exit_code = 128 + WTERMSIG(status);
        } else {
            output->exit_code = -1;
        }
    }
    if (stdout_pipe[0] >= 0) close(stdout_pipe[0]);
    if (stderr_pipe[0] >= 0) close(stderr_pipe[0]);
    free(argv);
    sengoo_process_free_envp(clear_envp);
    return sengoo_process_output_register(output);
}
#endif

static long long sengoo_process_command_run_platform(SengooProcessCommand* command) {
    return sengoo_process_command_run_platform_with_stdin(command, NULL, 0);
}

static long long sengoo_process_command_run_pipeline(SengooProcessCommand* final_command);

long long sengoo_process_command_run(long long handle) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    if (!sengoo_process_command_is_live(command)) {
        return 0;
    }
    if (command->pipe_stdout_upstream_handle != 0) {
        return sengoo_process_command_run_pipeline(command);
    }
    return sengoo_process_command_run_platform(command);
}

long long sengoo_process_command_close(long long handle) {
    SengooProcessOwnedSlot* slot = sengoo_process_owned_slot(handle, SENGOO_PROCESS_COMMAND);
    if (!slot) return -SENGOO_STATUS_INVALID_HANDLE;
    if (!slot->pointer) return 0;
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    if (!command) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (command->closed) {
        return 0;
    }
    long long upstream_handle = command->pipe_stdout_upstream_handle;
    command->pipe_stdout_upstream_handle = 0;
    if (upstream_handle != 0 && upstream_handle != handle) {
        SengooProcessCommand* upstream = sengoo_process_command_from_handle(upstream_handle);
        if (upstream && !upstream->closed) {
            (void)sengoo_process_command_close(upstream_handle);
        }
    }
    sengoo_process_owned_take(handle, SENGOO_PROCESS_COMMAND);
    sengoo_process_command_free_fields(command);
    command->closed = 1;
    free(command);
    return 0;
}

long long sengoo_process_output_exit_code(long long handle) {
    SengooProcessOutput* output = sengoo_process_output_from_handle(handle);
    if (!sengoo_process_output_is_live(output)) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (output->timed_out) {
        return -SENGOO_STATUS_TIMEOUT;
    }
    if (output->exit_code < 0) {
        return -SENGOO_STATUS_IO;
    }
    return output->exit_code;
}

long long sengoo_process_output_timed_out(long long handle) {
    SengooProcessOutput* output = sengoo_process_output_from_handle(handle);
    if (!sengoo_process_output_is_live(output)) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    return output->timed_out ? 1 : 0;
}

long long sengoo_process_output_stdout_len(long long handle) {
    SengooProcessOutput* output = sengoo_process_output_from_handle(handle);
    if (!sengoo_process_output_is_live(output)) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    return (long long)output->stdout_len;
}

long long sengoo_process_output_stderr_len(long long handle) {
    SengooProcessOutput* output = sengoo_process_output_from_handle(handle);
    if (!sengoo_process_output_is_live(output)) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    return (long long)output->stderr_len;
}

static long long sengoo_process_output_copy_bytes(SengooProcessOutput* output, const char* data, size_t len, long long out_buffer, long long out_capacity) {
    char* out = (char*)(intptr_t)out_buffer;
    if (!sengoo_process_output_is_live(output)) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (out_capacity < 0 || (unsigned long long)len > (unsigned long long)out_capacity || (len > 0 && (!data || !out))) {
        return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    }
    if (len > 0) {
        memcpy(out, data, len);
    }
    return (long long)len;
}

long long sengoo_process_output_stdout_copy(long long handle, long long out_buffer, long long out_capacity) {
    SengooProcessOutput* output = sengoo_process_output_from_handle(handle);
    return sengoo_process_output_copy_bytes(output, output ? output->stdout_data : NULL, output ? output->stdout_len : 0, out_buffer, out_capacity);
}

long long sengoo_process_output_stderr_copy(long long handle, long long out_buffer, long long out_capacity) {
    SengooProcessOutput* output = sengoo_process_output_from_handle(handle);
    return sengoo_process_output_copy_bytes(output, output ? output->stderr_data : NULL, output ? output->stderr_len : 0, out_buffer, out_capacity);
}

long long sengoo_process_output_close(long long handle) {
    SengooProcessOwnedSlot* slot = sengoo_process_owned_slot(handle, SENGOO_PROCESS_OUTPUT);
    if (!slot) return -SENGOO_STATUS_INVALID_HANDLE;
    if (!slot->pointer) return 0;
    SengooProcessOutput* output = sengoo_process_output_from_handle(handle);
    if (!output) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (output->closed) {
        return 0;
    }
    sengoo_process_owned_take(handle, SENGOO_PROCESS_OUTPUT);
    free(output->stdout_data);
    free(output->stderr_data);
    output->stdout_data = NULL;
    output->stderr_data = NULL;
    output->stdout_len = 0;
    output->stderr_len = 0;
    output->closed = 1;
    free(output);
    return 0;
}

typedef struct {
#ifdef _WIN32
    HANDLE process;
    HANDLE thread;
    HANDLE job;
    HANDLE stdin_write;
    HANDLE stdout_read;
    HANDLE stderr_read;
#else
    pid_t pid;
    int stdin_write;
    int stdout_read;
    int stderr_read;
#endif
    int piped;
    int owned_group;
    int stdout_eof;
    int stderr_eof;
    int completed;
    int timed_out;
    int killed;
    long long exit_code;
} SengooProcessHandleState;

static long long sengoo_process_handle_alloc(SengooProcessHandleState* state) {
    long long handle = sengoo_process_owned_new(state, SENGOO_PROCESS_CHILD);
    return handle ? handle : -(long long)SENGOO_STATUS_OUT_OF_MEMORY;
}

static SengooProcessHandleState* sengoo_process_handle_resolve(long long handle) {
    return (SengooProcessHandleState*)sengoo_process_owned_get(handle, SENGOO_PROCESS_CHILD);
}

static void sengoo_process_handle_state_destroy(SengooProcessHandleState* state) {
    if (!state) {
        return;
    }
#ifdef _WIN32
    if (state->job) TerminateJobObject(state->job, 1);
    else if (!state->completed && state->process) TerminateProcess(state->process, 1);
    if (state->process && !state->completed) WaitForSingleObject(state->process, INFINITE);
    if (state->stdin_write) CloseHandle(state->stdin_write);
    if (state->stdout_read) CloseHandle(state->stdout_read);
    if (state->stderr_read) CloseHandle(state->stderr_read);
    if (state->job) CloseHandle(state->job);
    if (state->process) {
        CloseHandle(state->process);
    }
    if (state->thread) {
        CloseHandle(state->thread);
    }
#else
    if (state->pid > 0) {
        if (state->owned_group) kill(-state->pid, SIGKILL);
        else if (!state->completed) kill(state->pid, SIGKILL);
        if (!state->completed) { int status; while (waitpid(state->pid, &status, 0) < 0 && errno == EINTR) {} }
    }
    if (state->piped) {
        if (state->stdin_write >= 0) close(state->stdin_write);
        if (state->stdout_read >= 0) close(state->stdout_read);
        if (state->stderr_read >= 0) close(state->stderr_read);
    }
#endif
    free(state);
}

long long sengoo_process_command_pipe_stdout_to(long long upstream_handle, long long downstream_handle) {
    SengooProcessCommand* upstream = sengoo_process_command_from_handle(upstream_handle);
    SengooProcessCommand* downstream = sengoo_process_command_from_handle(downstream_handle);
    if (!upstream || !downstream || downstream->pipe_stdout_upstream_handle != 0) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (!upstream->executable || upstream->executable[0] == '\0' || !downstream->executable || downstream->executable[0] == '\0') {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    downstream->pipe_stdout_upstream_handle = upstream_handle;
    return downstream_handle;
}

static long long sengoo_process_command_run_with_stdin_data(
    SengooProcessCommand* command,
    const char* stdin_data,
    size_t stdin_len) {
    if (!sengoo_process_command_is_live(command)) {
        return 0;
    }
    command->capture_stdout = 1;
    return sengoo_process_command_run_platform_with_stdin(command, stdin_data, stdin_len);
}

static long long sengoo_process_command_run_pipeline(SengooProcessCommand* final_command) {
    SengooProcessCommand* upstream = sengoo_process_command_from_handle(final_command->pipe_stdout_upstream_handle);
    if (!upstream || !upstream->executable) {
        return 0;
    }
    upstream->capture_stdout = 1;
    long long upstream_output_handle = sengoo_process_command_run_platform(upstream);
    (void)sengoo_process_command_close(final_command->pipe_stdout_upstream_handle);
    final_command->pipe_stdout_upstream_handle = 0;
    if (upstream_output_handle == 0) {
        return 0;
    }
    SengooProcessOutput* upstream_output = sengoo_process_output_from_handle(upstream_output_handle);
    if (!upstream_output) {
        return 0;
    }
    long long final_output_handle = sengoo_process_command_run_with_stdin_data(
        final_command,
        upstream_output->stdout_data ? upstream_output->stdout_data : "",
        upstream_output->stdout_len);
    sengoo_process_output_close(upstream_output_handle);
    return final_output_handle;
}

static long long sengoo_process_spawn_owned(SengooProcessCommand* command, int piped);

static long long sengoo_process_command_spawn_platform(SengooProcessCommand* command) {
    return sengoo_process_spawn_owned(command, 0);
}

long long sengoo_process_command_spawn(long long handle) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    if (!sengoo_process_command_is_live(command)) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    long long spawned = sengoo_process_command_spawn_platform(command);
    if (spawned <= 0) {
        return -SENGOO_STATUS_IO;
    }
    sengoo_process_command_close(handle);
    return spawned;
}

long long sengoo_process_handle_wait(long long handle, long long timeout_ms) {
    SengooProcessHandleState* state = sengoo_process_handle_resolve(handle);
    if (!state) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (state->completed) {
        return state->timed_out ? -SENGOO_STATUS_TIMEOUT : state->exit_code;
    }
    long long start_ms = sengoo_process_now_ms();
#ifdef _WIN32
    for (;;) {
        DWORD wait = WaitForSingleObject(state->process, 0);
        if (wait == WAIT_OBJECT_0) {
            DWORD code = 1;
            if (GetExitCodeProcess(state->process, &code)) {
                state->exit_code = (long long)code;
            }
            state->completed = 1;
            return state->exit_code;
        }
        if (timeout_ms >= 0 && sengoo_process_now_ms() - start_ms >= timeout_ms) {
            return -SENGOO_STATUS_TIMEOUT;
        }
        sengoo_process_sleep_short();
    }
#else
    for (;;) {
        int status = 0;
        pid_t waited = waitpid(state->pid, &status, WNOHANG);
        if (waited == state->pid) {
            if (WIFEXITED(status)) {
                state->exit_code = (long long)WEXITSTATUS(status);
            } else if (WIFSIGNALED(status)) {
                state->exit_code = 128 + WTERMSIG(status);
            } else {
                state->exit_code = -1;
            }
            state->completed = 1;
            return state->exit_code;
        }
        if (waited < 0 && errno != EINTR) {
            return -SENGOO_STATUS_IO;
        }
        if (timeout_ms >= 0 && sengoo_process_now_ms() - start_ms >= timeout_ms) {
            return -SENGOO_STATUS_TIMEOUT;
        }
        sengoo_process_sleep_short();
    }
#endif
}

long long sengoo_process_handle_wait_cancellable(long long handle, long long timeout_ms) {
    SengooProcessHandleState* state = sengoo_process_handle_resolve(handle);
    if (!state) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (state->completed) {
        if (state->timed_out) {
            return -SENGOO_STATUS_TIMEOUT;
        }
        return state->killed ? -SENGOO_STATUS_CANCELED : state->exit_code;
    }
    long long start_ms = sengoo_process_now_ms();
#ifdef _WIN32
    for (;;) {
        DWORD wait = WaitForSingleObject(state->process, 0);
        if (wait == WAIT_OBJECT_0) {
            DWORD code = 1;
            if (GetExitCodeProcess(state->process, &code)) {
                state->exit_code = (long long)code;
            }
            state->completed = 1;
            return state->killed ? -SENGOO_STATUS_CANCELED : state->exit_code;
        }
        if (timeout_ms >= 0 && sengoo_process_now_ms() - start_ms >= timeout_ms) {
            return -SENGOO_STATUS_TIMEOUT;
        }
        sengoo_process_sleep_short();
    }
#else
    for (;;) {
        int status = 0;
        pid_t waited = waitpid(state->pid, &status, WNOHANG);
        if (waited == state->pid) {
            if (WIFEXITED(status)) {
                state->exit_code = (long long)WEXITSTATUS(status);
            } else if (WIFSIGNALED(status)) {
                state->exit_code = 128 + WTERMSIG(status);
                state->killed = 1;
            } else {
                state->exit_code = -1;
            }
            state->completed = 1;
            return state->killed ? -SENGOO_STATUS_CANCELED : state->exit_code;
        }
        if (waited < 0 && errno != EINTR) {
            return -SENGOO_STATUS_IO;
        }
        if (timeout_ms >= 0 && sengoo_process_now_ms() - start_ms >= timeout_ms) {
            return -SENGOO_STATUS_TIMEOUT;
        }
        sengoo_process_sleep_short();
    }
#endif
}

long long sengoo_process_handle_kill(long long handle) {
    SengooProcessHandleState* state = sengoo_process_handle_resolve(handle);
    if (!state) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
#ifdef _WIN32
    if (!TerminateProcess(state->process, 1)) {
        return -SENGOO_STATUS_IO;
    }
#else
    if (kill(state->pid, SIGKILL) != 0) {
        return -SENGOO_STATUS_IO;
    }
#endif
    state->killed = 1;
    return 1;
}

long long sengoo_process_handle_exit_code(long long handle) {
    SengooProcessHandleState* state = sengoo_process_handle_resolve(handle);
    if (!state) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (!state->completed) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (state->timed_out) {
        return -SENGOO_STATUS_TIMEOUT;
    }
    return state->exit_code;
}

long long sengoo_process_handle_close(long long handle) {
    SengooProcessOwnedSlot* slot = sengoo_process_owned_slot(handle, SENGOO_PROCESS_CHILD);
    if (!slot) return -SENGOO_STATUS_INVALID_HANDLE;
    if (!slot->pointer) return 0;
    SengooProcessHandleState* state = sengoo_process_owned_take(handle, SENGOO_PROCESS_CHILD);
    if (!state) return -SENGOO_STATUS_INVALID_HANDLE;
    sengoo_process_handle_state_destroy(state);
    return 0;
}

/* Piped processes retain only kernel pipe buffers: callers must poll both output
 * streams. No background reader accumulates unbounded output. */
#define SENGOO_PROCESS_IO_CHUNK (64 * 1024)

#ifdef _WIN32
static wchar_t* sengoo_process_utf16(const char* text) {
    if (!text) return NULL;
    int size = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, -1, NULL, 0);
    if (size <= 0) return NULL;
    wchar_t* result = (wchar_t*)malloc((size_t)size * sizeof(wchar_t));
    if (!result) return NULL;
    if (!MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, -1, result, size)) {
        free(result); return NULL;
    }
    return result;
}

static int sengoo_process_wenv_compare(const void* a, const void* b) {
    return _wcsicmp(*(wchar_t* const*)a, *(wchar_t* const*)b);
}

static wchar_t* sengoo_process_wide_environment(SengooProcessCommand* command) {
    LPWCH inherited = command->env_clear ? NULL : GetEnvironmentStringsW();
    if (!command->env_clear && !inherited) return NULL;
    size_t count = command->env_len, used = 0, total = 1;
    if (inherited) for (const wchar_t* p = inherited; *p; p += wcslen(p) + 1) count++;
    wchar_t** entries = (wchar_t**)calloc(count + 1, sizeof(wchar_t*));
    wchar_t* block = NULL;
    if (!entries) goto done;
    if (inherited) for (const wchar_t* p = inherited; *p; p += wcslen(p) + 1) {
        const wchar_t* eq = wcschr(p + (*p == L'='), L'=');
        int replaced = 0;
        for (size_t i = 0; eq && i < command->env_len; i++) {
            wchar_t* name = sengoo_process_utf16(command->env[i].name);
            if (!name) goto done;
            replaced = wcslen(name) == (size_t)(eq - p) && _wcsnicmp(name, p, (size_t)(eq - p)) == 0;
            free(name);
            if (replaced) break;
        }
        if (!replaced) {
            entries[used] = _wcsdup(p);
            if (!entries[used]) goto done;
            used++;
        }
    }
    for (size_t i = 0; i < command->env_len; i++) {
        if (command->env[i].remove) continue;
        size_t n = strlen(command->env[i].name), v = strlen(command->env[i].value);
        if (n > SIZE_MAX - v - 2) goto done;
        char* pair = (char*)malloc(n + v + 2);
        if (!pair) goto done;
        memcpy(pair, command->env[i].name, n); pair[n] = '=';
        memcpy(pair + n + 1, command->env[i].value, v + 1);
        entries[used] = sengoo_process_utf16(pair); free(pair);
        if (!entries[used]) goto done;
        used++;
    }
    qsort(entries, used, sizeof(wchar_t*), sengoo_process_wenv_compare);
    for (size_t i = 0; i < used; i++) {
        size_t n = wcslen(entries[i]) + 1;
        if (n > 1024 * 1024 || total > 1024 * 1024 - n) goto done;
        total += n;
    }
    if (total < 2) total = 2;
    block = (wchar_t*)calloc(total, sizeof(wchar_t));
    if (block) {
        wchar_t* out = block;
        for (size_t i = 0; i < used; i++) {
            size_t n = wcslen(entries[i]) + 1;
            memcpy(out, entries[i], n * sizeof(wchar_t)); out += n;
        }
    }
done:
    if (inherited) FreeEnvironmentStringsW(inherited);
    if (entries) { for (size_t i = 0; i < used; i++) free(entries[i]); free(entries); }
    return block;
}

/* A nonwaiting byte-mode server makes stdin writes partial/nonblocking, unlike
 * an anonymous pipe's WriteFile. Only the child read end is inheritable. */
static int sengoo_process_stdin_pipe(HANDLE* writer, HANDLE* reader) {
    static volatile LONG serial = 0;
    wchar_t name[140];
    swprintf(name, 140, L"\\\\.\\pipe\\sengoo-stdin-%lu-%llu-%ld", GetCurrentProcessId(),
             (unsigned long long)GetTickCount64(), InterlockedIncrement(&serial));
    *writer = CreateNamedPipeW(name, PIPE_ACCESS_OUTBOUND | FILE_FLAG_FIRST_PIPE_INSTANCE,
        PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_NOWAIT | PIPE_REJECT_REMOTE_CLIENTS,
        1, SENGOO_PROCESS_IO_CHUNK, SENGOO_PROCESS_IO_CHUNK, 0, NULL);
    if (*writer == INVALID_HANDLE_VALUE) { *writer = NULL; return 0; }
    SECURITY_ATTRIBUTES security = { sizeof(security), NULL, TRUE };
    *reader = CreateFileW(name, GENERIC_READ, 0, &security, OPEN_EXISTING, 0, NULL);
    if (*reader == INVALID_HANDLE_VALUE) { CloseHandle(*writer); *writer = *reader = NULL; return 0; }
    if (!ConnectNamedPipe(*writer, NULL) && GetLastError() != ERROR_PIPE_CONNECTED) {
        CloseHandle(*writer); CloseHandle(*reader); *writer = *reader = NULL; return 0;
    }
    return 1;
}

static long long sengoo_process_spawn_owned(SengooProcessCommand* command, int piped) {
    char* line = sengoo_windows_process_command_line_dyn(command);
    wchar_t* wide_line = sengoo_process_utf16(line);
    wchar_t* cwd = command->cwd ? sengoo_process_utf16(command->cwd) : NULL;
    wchar_t* environment = sengoo_process_wide_environment(command);
    free(line);
    if (!wide_line || !environment || (command->cwd && !cwd)) {
        free(wide_line); free(cwd); free(environment); return -SENGOO_STATUS_INVALID_UTF8;
    }
    SengooProcessHandleState* state = (SengooProcessHandleState*)calloc(1, sizeof(*state));
    HANDLE child_in = NULL, child_out = NULL, child_err = NULL;
    STARTUPINFOEXW startup; PROCESS_INFORMATION info;
    memset(&startup, 0, sizeof(startup)); memset(&info, 0, sizeof(info));
    startup.StartupInfo.cb = sizeof(STARTUPINFOW);
    long long result = -SENGOO_STATUS_IO;
    if (!state) { result = -SENGOO_STATUS_OUT_OF_MEMORY; goto finish; }
    state->piped = piped; state->exit_code = -1;
    state->job = CreateJobObjectW(NULL, NULL);
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits; memset(&limits, 0, sizeof(limits));
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    if (!state->job || !SetInformationJobObject(state->job, JobObjectExtendedLimitInformation, &limits, sizeof(limits))) goto finish;
    if (piped) {
        SECURITY_ATTRIBUTES security = { sizeof(security), NULL, TRUE };
        if (!sengoo_process_stdin_pipe(&state->stdin_write, &child_in)
            || !CreatePipe(&state->stdout_read, &child_out, &security, SENGOO_PROCESS_IO_CHUNK)
            || !CreatePipe(&state->stderr_read, &child_err, &security, SENGOO_PROCESS_IO_CHUNK)
            || !SetHandleInformation(state->stdout_read, HANDLE_FLAG_INHERIT, 0)
            || !SetHandleInformation(state->stderr_read, HANDLE_FLAG_INHERIT, 0)) goto finish;
        startup.StartupInfo.cb = sizeof(startup);
        startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
        startup.StartupInfo.hStdInput = child_in;
        startup.StartupInfo.hStdOutput = child_out;
        startup.StartupInfo.hStdError = child_err;
        SIZE_T size = 0;
        InitializeProcThreadAttributeList(NULL, 1, 0, &size);
        startup.lpAttributeList = (LPPROC_THREAD_ATTRIBUTE_LIST)malloc(size);
        if (!startup.lpAttributeList) { result = -SENGOO_STATUS_OUT_OF_MEMORY; goto finish; }
        if (!InitializeProcThreadAttributeList(startup.lpAttributeList, 1, 0, &size)) {
            free(startup.lpAttributeList); startup.lpAttributeList = NULL; goto finish;
        }
        HANDLE allowed[] = { child_in, child_out, child_err };
        if (!UpdateProcThreadAttribute(startup.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                                      allowed, sizeof(allowed), NULL, NULL)) goto finish;
    }
    if (!CreateProcessW(NULL, wide_line, NULL, NULL, piped ? TRUE : FALSE,
        CREATE_UNICODE_ENVIRONMENT | CREATE_SUSPENDED | (piped ? EXTENDED_STARTUPINFO_PRESENT | CREATE_NO_WINDOW : 0),
        environment, cwd, &startup.StartupInfo, &info)) goto finish;
    state->process = info.hProcess; state->thread = info.hThread;
    if (!AssignProcessToJobObject(state->job, state->process)) {
        TerminateProcess(state->process, 1); goto finish;
    }
    if (ResumeThread(state->thread) == (DWORD)-1) goto finish;
    result = sengoo_process_handle_alloc(state);
    if (result > 0) state = NULL;
finish:
    if (child_in) CloseHandle(child_in);
    if (child_out) CloseHandle(child_out);
    if (child_err) CloseHandle(child_err);
    if (startup.lpAttributeList) { DeleteProcThreadAttributeList(startup.lpAttributeList); free(startup.lpAttributeList); }
    if (state) sengoo_process_handle_state_destroy(state);
    free(wide_line); free(cwd); free(environment);
    return result;
}
#else
static long long sengoo_process_spawn_owned(SengooProcessCommand* command, int piped) {
    int in[2] = {-1,-1}, out[2] = {-1,-1}, err[2] = {-1,-1}, exec_error[2] = {-1,-1};
    char** argv = sengoo_process_build_argv(command);
    char** clear_env = command->env_clear ? sengoo_process_build_clear_envp(command) : NULL;
    SengooProcessHandleState* state = (SengooProcessHandleState*)calloc(1, sizeof(*state));
    long long result = -SENGOO_STATUS_IO;
    if (!argv || !state || (command->env_clear && !clear_env)) { result = -SENGOO_STATUS_OUT_OF_MEMORY; goto finish; }
    state->stdin_write = state->stdout_read = state->stderr_read = -1;
    state->piped = piped; state->exit_code = -1;
    if (pipe(exec_error) || fcntl(exec_error[1], F_SETFD, FD_CLOEXEC) < 0) goto finish;
    if (piped && (pipe(in) || pipe(out) || pipe(err))) goto finish;
    pid_t pid = fork();
    if (pid < 0) goto finish;
    if (pid == 0) {
        close(exec_error[0]);
        if (setpgid(0, 0) < 0) goto child_failed;
        if (piped) {
            close(in[1]); close(out[0]); close(err[0]);
            if (dup2(in[0], STDIN_FILENO) < 0 || dup2(out[1], STDOUT_FILENO) < 0 || dup2(err[1], STDERR_FILENO) < 0) goto child_failed;
            close(in[0]); close(out[1]); close(err[1]);
        }
        if (command->cwd && chdir(command->cwd)) goto child_failed;
        if (command->env_clear) { extern char** environ; environ = clear_env; }
        else for (size_t i = 0; i < command->env_len; i++) {
            if (command->env[i].remove) unsetenv(command->env[i].name);
            else if (setenv(command->env[i].name, command->env[i].value, 1)) goto child_failed;
        }
        execvp(command->executable, argv);
child_failed: {
            int failure = errno; (void)write(exec_error[1], &failure, sizeof(failure)); _exit(127);
        }
    }
    state->pid = pid; state->owned_group = 1;
    close(exec_error[1]); exec_error[1] = -1;
    int failure = 0; ssize_t n;
    do { n = read(exec_error[0], &failure, sizeof(failure)); } while (n < 0 && errno == EINTR);
    if (n != 0) goto finish;
    if (piped) {
        state->stdin_write = in[1]; in[1] = -1;
        state->stdout_read = out[0]; out[0] = -1;
        state->stderr_read = err[0]; err[0] = -1;
        int descriptors[] = {state->stdin_write, state->stdout_read, state->stderr_read};
        for (size_t i = 0; i < 3; i++) {
            if (fcntl(descriptors[i], F_SETFD, FD_CLOEXEC) < 0 || !sengoo_process_make_nonblocking(descriptors[i])) goto finish;
        }
    }
    result = sengoo_process_handle_alloc(state);
    if (result > 0) state = NULL;
finish:
    for (size_t i = 0; i < 2; i++) {
        if (in[i] >= 0) close(in[i]); if (out[i] >= 0) close(out[i]);
        if (err[i] >= 0) close(err[i]); if (exec_error[i] >= 0) close(exec_error[i]);
    }
    if (state) sengoo_process_handle_state_destroy(state);
    free(argv); sengoo_process_free_envp(clear_env);
    return result;
}
#endif

long long sengoo_process_command_spawn_piped(long long handle) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    if (!sengoo_process_command_is_live(command)) return -SENGOO_STATUS_INVALID_HANDLE;
    if (command->pipe_stdout_upstream_handle) return -SENGOO_STATUS_INVALID_ARGUMENT;
    return sengoo_process_spawn_owned(command, 1);
}

long long sengoo_process_handle_try_wait(long long handle) {
    SengooProcessHandleState* state = sengoo_process_handle_resolve(handle);
    if (!state) return -SENGOO_STATUS_INVALID_HANDLE;
    if (state->completed) return 1;
#ifdef _WIN32
    DWORD status = WaitForSingleObject(state->process, 0);
    if (status == WAIT_TIMEOUT) return 0;
    if (status != WAIT_OBJECT_0) return -SENGOO_STATUS_IO;
    DWORD code;
    if (!GetExitCodeProcess(state->process, &code)) return -SENGOO_STATUS_IO;
    state->exit_code = code;
#else
    int status; pid_t waited;
    do { waited = waitpid(state->pid, &status, WNOHANG); } while (waited < 0 && errno == EINTR);
    if (waited == 0) return 0;
    if (waited < 0) return -SENGOO_STATUS_IO;
    state->exit_code = WIFEXITED(status) ? WEXITSTATUS(status) : 128 + WTERMSIG(status);
#endif
    state->completed = 1;
    return 1;
}

long long sengoo_process_handle_read(long long handle, long long stream, long long buffer_handle) {
    SengooProcessHandleState* state = sengoo_process_handle_resolve(handle);
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!state || !state->piped || !buffer) return -SENGOO_STATUS_INVALID_HANDLE;
    if (stream != 1 && stream != 2) return -SENGOO_STATUS_INVALID_ARGUMENT;
    buffer->used_len = 0;
    if (!buffer->capacity) return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    size_t capacity = buffer->capacity > SENGOO_PROCESS_IO_CHUNK ? SENGOO_PROCESS_IO_CHUNK : buffer->capacity;
    int* eof = stream == 1 ? &state->stdout_eof : &state->stderr_eof;
    if (*eof) return 0;
#ifdef _WIN32
    HANDLE pipe = stream == 1 ? state->stdout_read : state->stderr_read;
    DWORD available = 0, read_count = 0;
    if (!PeekNamedPipe(pipe, NULL, 0, NULL, &available, NULL)) {
        if (GetLastError() == ERROR_BROKEN_PIPE) { *eof = 1; return 0; }
        return -SENGOO_STATUS_IO;
    }
    if (!available) return 0;
    if (capacity > available) capacity = available;
    if (!ReadFile(pipe, buffer->bytes, (DWORD)capacity, &read_count, NULL)) return -SENGOO_STATUS_IO;
    buffer->used_len = read_count;
#else
    int pipe = stream == 1 ? state->stdout_read : state->stderr_read;
    ssize_t read_count;
    do { read_count = read(pipe, buffer->bytes, capacity); } while (read_count < 0 && errno == EINTR);
    if (read_count == 0) { *eof = 1; return 0; }
    if (read_count < 0) return errno == EAGAIN || errno == EWOULDBLOCK ? 0 : -SENGOO_STATUS_IO;
    buffer->used_len = (size_t)read_count;
#endif
    return (long long)buffer->used_len;
}

long long sengoo_process_handle_stream_eof(long long handle, long long stream) {
    SengooProcessHandleState* state = sengoo_process_handle_resolve(handle);
    if (!state || !state->piped) return -SENGOO_STATUS_INVALID_HANDLE;
    if (stream == 1) return state->stdout_eof;
    if (stream == 2) return state->stderr_eof;
    return -SENGOO_STATUS_INVALID_ARGUMENT;
}

long long sengoo_process_handle_write_stdin(long long handle, long long buffer_handle, long long length) {
    SengooProcessHandleState* state = sengoo_process_handle_resolve(handle);
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!state || !state->piped || !buffer) return -SENGOO_STATUS_INVALID_HANDLE;
    if (length < 0 || (unsigned long long)length > buffer->used_len || (unsigned long long)length > buffer->capacity
        || (length && !buffer->bytes)) return -SENGOO_STATUS_INVALID_ARGUMENT;
    if (length > SENGOO_PROCESS_IO_CHUNK) length = SENGOO_PROCESS_IO_CHUNK;
    if (!length) return 0;
#ifdef _WIN32
    if (!state->stdin_write) return -SENGOO_STATUS_INVALID_HANDLE;
    DWORD written = 0;
    if (!WriteFile(state->stdin_write, buffer->bytes, (DWORD)length, &written, NULL)) {
        return -SENGOO_STATUS_IO;
    }
#else
    if (state->stdin_write < 0) return -SENGOO_STATUS_INVALID_HANDLE;
    sigset_t blocked, previous, pending;
    sigemptyset(&blocked); sigaddset(&blocked, SIGPIPE);
    if (pthread_sigmask(SIG_BLOCK, &blocked, &previous)) return -SENGOO_STATUS_IO;
    sigpending(&pending); int had_signal = sigismember(&pending, SIGPIPE);
    ssize_t written;
    do { written = write(state->stdin_write, buffer->bytes, (size_t)length); } while (written < 0 && errno == EINTR);
    int error = errno;
    if (written < 0 && error == EPIPE && !had_signal) {
        /* An ignored SIGPIPE is never queued: do not wait for it indefinitely. */
        if (sigpending(&pending) == 0 && sigismember(&pending, SIGPIPE)) {
            int signal_number; sigwait(&blocked, &signal_number);
        }
    }
    pthread_sigmask(SIG_SETMASK, &previous, NULL);
    if (written < 0) return error == EAGAIN || error == EWOULDBLOCK ? 0 : -SENGOO_STATUS_IO;
#endif
    return (long long)written;
}

/* Explicit event-loop spelling. The original piped API uses PIPE_NOWAIT
 * (Windows) / O_NONBLOCK (POSIX), preserving partial-write and EOF semantics. */
long long sengoo_process_handle_try_write_stdin(long long handle, long long buffer_handle, long long length) {
    return sengoo_process_handle_write_stdin(handle, buffer_handle, length);
}

long long sengoo_process_handle_close_stdin(long long handle) {
    SengooProcessHandleState* state = sengoo_process_handle_resolve(handle);
    if (!state || !state->piped) return -SENGOO_STATUS_INVALID_HANDLE;
#ifdef _WIN32
    if (state->stdin_write) { CloseHandle(state->stdin_write); state->stdin_write = NULL; }
#else
    if (state->stdin_write >= 0) { close(state->stdin_write); state->stdin_write = -1; }
#endif
    return 1;
}

long long sengoo_process_handle_terminate_tree(long long handle) {
    SengooProcessHandleState* state = sengoo_process_handle_resolve(handle);
    if (!state) return -SENGOO_STATUS_INVALID_HANDLE;
#ifdef _WIN32
    if (state->job ? !TerminateJobObject(state->job, 1) : !TerminateProcess(state->process, 1)) return -SENGOO_STATUS_IO;
#else
    if (kill(state->owned_group ? -state->pid : state->pid, SIGKILL) && errno != ESRCH) return -SENGOO_STATUS_IO;
#endif
    state->killed = 1;
    return 1;
}

long long sengoo_process_handle_tree_quiescent(long long handle) {
    SengooProcessHandleState* state = sengoo_process_handle_resolve(handle);
    if (!state) return -SENGOO_STATUS_INVALID_HANDLE;
#ifdef _WIN32
    if (!state->job) return -SENGOO_STATUS_UNSUPPORTED;
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION accounting;
    if (!QueryInformationJobObject(state->job, JobObjectBasicAccountingInformation,
                                   &accounting, sizeof(accounting), NULL)) return -SENGOO_STATUS_IO;
    return accounting.ActiveProcesses == 0;
#else
    return -SENGOO_STATUS_UNSUPPORTED;
#endif
}

long long sengoo_process_handle_reap(long long handle, long long timeout_ms) {
    if (timeout_ms < -1) return -SENGOO_STATUS_INVALID_ARGUMENT;
    long long start = sengoo_process_now_ms();
    for (;;) {
        long long done = sengoo_process_handle_try_wait(handle);
        if (done < 0) return done;
        if (done) return sengoo_process_handle_exit_code(handle);
        if (timeout_ms >= 0 && sengoo_process_now_ms() - start >= timeout_ms) return -SENGOO_STATUS_TIMEOUT;
        sengoo_process_sleep_short();
    }
}

long long sengoo_process_live_handle_count(void) {
    long long count = 0;
    for (size_t i = 0; i < sengoo_process_owned_count; i++)
        count += sengoo_process_owned_slots[i].pointer && sengoo_process_owned_slots[i].domain == SENGOO_PROCESS_CHILD;
    return count;
}

long long sengoo_process_live_resource_count(void) {
    long long count = 0;
    for (size_t i = 0; i < sengoo_process_owned_count; i++) count += sengoo_process_owned_slots[i].pointer != NULL;
    return count;
}

/* Explicit detached launch returns only immutable PID/birth evidence. There is
 * no owned slot, caller stdio/pipe handle, or launcher kill-on-close Job.
 * The application must authenticate IPC; this API never kills arbitrary PIDs. */
long long sengoo_process_command_spawn_detached(long long handle, long long identity_buffer) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    SengooFfiBuffer* identity = sengoo_ffi_buffer_from_handle(identity_buffer);
    if (!sengoo_process_command_is_live(command) || !identity) return -SENGOO_STATUS_INVALID_HANDLE;
    identity->used_len = 0;
    if (identity->capacity < 8) return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    if (command->pipe_stdout_upstream_handle) return -SENGOO_STATUS_INVALID_ARGUMENT;
#ifdef _WIN32
    char* line = sengoo_windows_process_command_line_dyn(command);
    wchar_t* wide_line = sengoo_process_utf16(line);
    wchar_t* executable = sengoo_process_utf16(command->executable);
    wchar_t* cwd = command->cwd ? sengoo_process_utf16(command->cwd) : NULL;
    wchar_t* environment = sengoo_process_wide_environment(command);
    free(line);
    long long result = -SENGOO_STATUS_IO;
    PROCESS_INFORMATION info; STARTUPINFOEXW startup;
    HANDLE null_io = NULL;
    memset(&info, 0, sizeof(info)); memset(&startup, 0, sizeof(startup));
    startup.StartupInfo.cb = sizeof(startup);
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES | STARTF_USESHOWWINDOW;
    startup.StartupInfo.wShowWindow = SW_HIDE;
    if (!wide_line || !executable || !environment || (command->cwd && !cwd)) {
        result = -SENGOO_STATUS_INVALID_UTF8; goto detached_done;
    }
    if (wcslen(wide_line) >= 32767) { result = -SENGOO_STATUS_OVERFLOW; goto detached_done; }
    /* Fresh NUL handles avoid Windows/CRT's special standard-handle copying.
     * The whitelist contains only this device; no parent stdio or unrelated
     * inheritable event/file/pipe handle reaches the child. */
    SECURITY_ATTRIBUTES security = {sizeof(security), NULL, TRUE};
    null_io = CreateFileW(L"NUL", GENERIC_READ | GENERIC_WRITE,
                         FILE_SHARE_READ | FILE_SHARE_WRITE, &security,
                         OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
    if (null_io == INVALID_HANDLE_VALUE) { null_io = NULL; goto detached_done; }
    startup.StartupInfo.hStdInput = startup.StartupInfo.hStdOutput = startup.StartupInfo.hStdError = null_io;
    SIZE_T attributes_size = 0;
    InitializeProcThreadAttributeList(NULL, 1, 0, &attributes_size);
    startup.lpAttributeList = (LPPROC_THREAD_ATTRIBUTE_LIST)malloc(attributes_size);
    if (!startup.lpAttributeList) { result = -SENGOO_STATUS_OUT_OF_MEMORY; goto detached_done; }
    if (!InitializeProcThreadAttributeList(startup.lpAttributeList, 1, 0, &attributes_size)) {
        free(startup.lpAttributeList); startup.lpAttributeList = NULL; goto detached_done;
    }
    if (!UpdateProcThreadAttribute(startup.lpAttributeList, 0, PROC_THREAD_ATTRIBUTE_HANDLE_LIST,
                                   &null_io, sizeof(null_io), NULL, NULL)) goto detached_done;
    BOOL parent_job = FALSE;
    if (!IsProcessInJob(GetCurrentProcess(), NULL, &parent_job)) goto detached_done;
    DWORD flags = CREATE_UNICODE_ENVIRONMENT | CREATE_SUSPENDED | CREATE_NO_WINDOW | EXTENDED_STARTUPINFO_PRESENT;
    if (parent_job) flags |= CREATE_BREAKAWAY_FROM_JOB;
    if (!CreateProcessW(executable, wide_line, NULL, NULL, TRUE, flags,
                        environment, cwd, &startup.StartupInfo, &info)) {
        if (GetLastError() == ERROR_ACCESS_DENIED) result = -SENGOO_STATUS_PERMISSION_DENIED;
        goto detached_done;
    }
    /* CREATE_BREAKAWAY_FROM_JOB proves breakaway from the launcher's immediate
     * permitted Job. Windows may retain an outer system/supervisor Job in a
     * nested hierarchy; this API neither escapes nor claims to bypass it. */
    FILETIME created, exited, kernel, user;
    if (!GetProcessTimes(info.hProcess, &created, &exited, &kernel, &user)) goto detached_failed_child;
    uint64_t birth = ((uint64_t)created.dwHighDateTime << 32) | created.dwLowDateTime;
    if (!birth || birth > LLONG_MAX) { result = -SENGOO_STATUS_OVERFLOW; goto detached_failed_child; }
    /* Store all receipt evidence before the new host can execute any code. */
    for (size_t i = 0; i < 8; i++) identity->bytes[i] = (unsigned char)(birth >> (i * 8));
    if (ResumeThread(info.hThread) == (DWORD)-1) goto detached_failed_child;
    identity->used_len = 8;
    result = (long long)info.dwProcessId;
    goto detached_done;
detached_failed_child:
    TerminateProcess(info.hProcess, 1);
    WaitForSingleObject(info.hProcess, INFINITE);
detached_done:
    if (info.hThread) CloseHandle(info.hThread);
    if (info.hProcess) CloseHandle(info.hProcess);
    if (startup.lpAttributeList) { DeleteProcThreadAttributeList(startup.lpAttributeList); free(startup.lpAttributeList); }
    if (null_io) CloseHandle(null_io);
    free(wide_line); free(executable); free(cwd); free(environment);
    return result;
#else
    return -SENGOO_STATUS_UNSUPPORTED;
#endif
}

long long sengoo_process_identity_matches(long long pid, long long creation_identity) {
    if (pid <= 0 || pid > UINT32_MAX || creation_identity <= 0) return -SENGOO_STATUS_INVALID_ARGUMENT;
#ifdef _WIN32
    HANDLE process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE, FALSE, (DWORD)pid);
    if (!process) {
        DWORD error = GetLastError();
        if (error == ERROR_INVALID_PARAMETER || error == ERROR_NOT_FOUND) return 0;
        return error == ERROR_ACCESS_DENIED ? -SENGOO_STATUS_PERMISSION_DENIED : -SENGOO_STATUS_IO;
    }
    long long result = -SENGOO_STATUS_IO;
    FILETIME created, exited, kernel, user;
    if (GetProcessTimes(process, &created, &exited, &kernel, &user)) {
        uint64_t birth = ((uint64_t)created.dwHighDateTime << 32) | created.dwLowDateTime;
        if (birth != (uint64_t)creation_identity) result = 0;
        else {
            DWORD wait = WaitForSingleObject(process, 0);
            if (wait == WAIT_TIMEOUT) result = 1;
            else if (wait == WAIT_OBJECT_0) result = 0;
        }
    }
    CloseHandle(process);
    return result;
#else
    return -SENGOO_STATUS_UNSUPPORTED;
#endif
}
