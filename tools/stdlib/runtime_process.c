#define _CRT_SECURE_NO_WARNINGS

#include "runtime_shared.h"

#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#ifdef _WIN32
#include <windows.h>
#else
#include <fcntl.h>
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

static SengooProcessCommand* sengoo_process_command_from_handle(long long handle) {
    return (SengooProcessCommand*)sengoo_handle_to_ptr(handle);
}

static SengooProcessOutput* sengoo_process_output_from_handle(long long handle) {
    return (SengooProcessOutput*)sengoo_handle_to_ptr(handle);
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
    return sengoo_ptr_to_handle(command);
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
    return sengoo_time_unix_ms();
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
    return sengoo_ptr_to_handle(output);
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
    return sengoo_ptr_to_handle(output);
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
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    if (!command || command->closed) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    long long upstream_handle = command->pipe_stdout_upstream_handle;
    command->pipe_stdout_upstream_handle = 0;
    if (upstream_handle != 0 && upstream_handle != handle) {
        SengooProcessCommand* upstream = sengoo_process_command_from_handle(upstream_handle);
        if (upstream && !upstream->closed) {
            (void)sengoo_process_command_close(upstream_handle);
        }
    }
    sengoo_process_command_free_fields(command);
    command->closed = 1;
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
    SengooProcessOutput* output = sengoo_process_output_from_handle(handle);
    if (!output || output->closed) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    free(output->stdout_data);
    free(output->stderr_data);
    output->stdout_data = NULL;
    output->stderr_data = NULL;
    output->stdout_len = 0;
    output->stderr_len = 0;
    output->closed = 1;
    return 0;
}

typedef struct {
#ifdef _WIN32
    HANDLE process;
    HANDLE thread;
#else
    pid_t pid;
#endif
    int completed;
    int timed_out;
    int killed;
    long long exit_code;
} SengooProcessHandleState;

typedef struct {
    SengooProcessHandleState* state;
    uint32_t generation;
    unsigned char alive;
} SengooProcessHandleSlot;

static SengooProcessHandleSlot* g_process_handle_slots = NULL;
static size_t g_process_handle_slot_count = 0;
static size_t g_process_handle_slot_capacity = 0;

static int sengoo_process_handle_slot_ensure_capacity(size_t min_slots) {
    if (g_process_handle_slot_capacity >= min_slots) {
        return 1;
    }
    size_t new_cap = g_process_handle_slot_capacity == 0 ? 8 : g_process_handle_slot_capacity;
    while (new_cap < min_slots) {
        if (new_cap > (SIZE_MAX / 2)) {
            return 0;
        }
        new_cap *= 2;
    }
    SengooProcessHandleSlot* next = (SengooProcessHandleSlot*)realloc(
        g_process_handle_slots,
        new_cap * sizeof(SengooProcessHandleSlot));
    if (!next) {
        return 0;
    }
    if (new_cap > g_process_handle_slot_capacity) {
        memset(
            next + g_process_handle_slot_capacity,
            0,
            (new_cap - g_process_handle_slot_capacity) * sizeof(SengooProcessHandleSlot));
    }
    g_process_handle_slots = next;
    g_process_handle_slot_capacity = new_cap;
    return 1;
}

static long long sengoo_process_handle_alloc(SengooProcessHandleState* state) {
    size_t index = 0;
    for (; index < g_process_handle_slot_count; ++index) {
        if (!g_process_handle_slots[index].alive) {
            break;
        }
    }
    if (index == g_process_handle_slot_count) {
        if (!sengoo_process_handle_slot_ensure_capacity(g_process_handle_slot_count + 1)) {
            return -(long long)SENGOO_STATUS_OUT_OF_MEMORY;
        }
        g_process_handle_slot_count += 1;
    }
    SengooProcessHandleSlot* slot = &g_process_handle_slots[index];
    slot->state = state;
    slot->alive = 1;
    slot->generation += 1;
    if (slot->generation == 0) {
        slot->generation = 1;
    }
    return ((long long)slot->generation << 32) | (long long)(index + 1);
}

static SengooProcessHandleState* sengoo_process_handle_resolve(long long handle) {
    if (handle <= 0) {
        return NULL;
    }
    size_t index = ((size_t)handle & 0xFFFFFFFFu) - 1;
    uint32_t generation = (uint32_t)((unsigned long long)handle >> 32);
    if (index >= g_process_handle_slot_count) {
        return NULL;
    }
    SengooProcessHandleSlot* slot = &g_process_handle_slots[index];
    if (!slot->alive || slot->generation != generation || !slot->state) {
        return NULL;
    }
    return slot->state;
}

static void sengoo_process_handle_state_destroy(SengooProcessHandleState* state) {
    if (!state) {
        return;
    }
#ifdef _WIN32
    if (state->process) {
        CloseHandle(state->process);
    }
    if (state->thread) {
        CloseHandle(state->thread);
    }
#else
    (void)state;
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

#ifdef _WIN32
static long long sengoo_process_command_spawn_platform(SengooProcessCommand* command) {
    char* command_line = sengoo_windows_process_command_line_dyn(command);
    if (!command_line) {
        return 0;
    }
    char* env_block = sengoo_process_build_windows_env_block(command);
    STARTUPINFOA startup_info;
    PROCESS_INFORMATION process_info;
    memset(&startup_info, 0, sizeof(startup_info));
    memset(&process_info, 0, sizeof(process_info));
    startup_info.cb = sizeof(startup_info);
    BOOL created = CreateProcessA(
        NULL,
        command_line,
        NULL,
        NULL,
        FALSE,
        0,
        env_block,
        command->cwd,
        &startup_info,
        &process_info);
    free(command_line);
    free(env_block);
    if (!created) {
        return 0;
    }
    SengooProcessHandleState* state = (SengooProcessHandleState*)calloc(1, sizeof(SengooProcessHandleState));
    if (!state) {
        TerminateProcess(process_info.hProcess, 1);
        CloseHandle(process_info.hThread);
        CloseHandle(process_info.hProcess);
        return 0;
    }
    state->process = process_info.hProcess;
    state->thread = process_info.hThread;
    state->completed = 0;
    state->timed_out = 0;
    state->killed = 0;
    state->exit_code = -1;
    return sengoo_process_handle_alloc(state);
}
#else
static long long sengoo_process_command_spawn_platform(SengooProcessCommand* command) {
    char** argv = sengoo_process_build_argv(command);
    if (!argv) {
        return 0;
    }
    pid_t pid = fork();
    if (pid < 0) {
        free(argv);
        return 0;
    }
    if (pid == 0) {
        if (command->cwd && chdir(command->cwd) != 0) {
            _exit(127);
        }
        for (size_t i = 0; i < command->env_len; ++i) {
            if (command->env[i].remove) {
                unsetenv(command->env[i].name);
            } else {
                setenv(command->env[i].name, command->env[i].value, 1);
            }
        }
        execvp(command->executable, argv);
        _exit(127);
    }
    free(argv);
    SengooProcessHandleState* state = (SengooProcessHandleState*)calloc(1, sizeof(SengooProcessHandleState));
    if (!state) {
        return 0;
    }
    state->pid = pid;
    state->completed = 0;
    state->timed_out = 0;
    state->killed = 0;
    state->exit_code = -1;
    return sengoo_process_handle_alloc(state);
}
#endif

long long sengoo_process_command_spawn(long long handle) {
    SengooProcessCommand* command = sengoo_process_command_from_handle(handle);
    if (!sengoo_process_command_is_live(command)) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    long long spawned = sengoo_process_command_spawn_platform(command);
    if (spawned <= 0) {
        return -SENGOO_STATUS_IO;
    }
    command->closed = 1;
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
            state->timed_out = 1;
            state->completed = 1;
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
            state->timed_out = 1;
            state->completed = 1;
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
            state->timed_out = 1;
            state->completed = 1;
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
            state->timed_out = 1;
            state->completed = 1;
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
    if (handle <= 0) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    size_t index = ((size_t)handle & 0xFFFFFFFFu) - 1;
    uint32_t generation = (uint32_t)((unsigned long long)handle >> 32);
    if (index >= g_process_handle_slot_count) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    SengooProcessHandleSlot* slot = &g_process_handle_slots[index];
    if (!slot->alive || slot->generation != generation || !slot->state) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    sengoo_process_handle_state_destroy(slot->state);
    slot->state = NULL;
    slot->alive = 0;
    return 0;
}
