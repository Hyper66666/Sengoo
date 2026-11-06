#define _CRT_SECURE_NO_WARNINGS
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../runtime_pty.c"
#ifdef _WIN32
typedef struct { void* pointer; uint32_t generation; } ProbeSlot;
static ProbeSlot slots[256];
long long sengoo_opaque_handle_new(void* p) {
    for (size_t i = 1; i < 256; i++) if (!slots[i].pointer) {
        slots[i].pointer = p; slots[i].generation++;
        return ((long long)slots[i].generation << 32) | (long long)i;
    }
    return 0;
}
void* sengoo_opaque_handle_take(long long handle) {
    size_t index = (size_t)((uint64_t)handle & UINT32_MAX);
    if (handle <= 0 || index >= 256 || slots[index].generation != ((uint64_t)handle >> 32)) return NULL;
    void* result = slots[index].pointer; slots[index].pointer = NULL; return result;
}
SengooFfiBuffer* sengoo_ffi_buffer_from_handle(long long handle) { return (SengooFfiBuffer*)(intptr_t)handle; }
static long long raw(const void* pointer) { return (long long)(intptr_t)pointer; }
static long long add_arg(long long command, const char* text) { return sengoo_pty_command_arg(command, raw(text), (long long)strlen(text)); }
static void output_wide(const wchar_t* text) {
    DWORD written;
    assert(WriteConsoleW(GetStdHandle(STD_OUTPUT_HANDLE), text, (DWORD)wcslen(text), &written, NULL));
}
static int child(int argc, wchar_t** argv) {
    if (!wcscmp(argv[1], L"--grandson")) { Sleep(INFINITE); return 0; }
    if (!wcscmp(argv[1], L"--flood")) {
        unsigned char data[8192]; memset(data, 'X', sizeof(data)); DWORD written;
        for (int i = 0; i < 2048; i++) if (!WriteFile(GetStdHandle(STD_OUTPUT_HANDLE), data, sizeof(data), &written, NULL)) break;
        Sleep(INFINITE); return 0;
    }
    if (!wcscmp(argv[1], L"--stall")) { output_wide(L"STALL\r\n"); Sleep(INFINITE); return 0; }
    if (!wcscmp(argv[1], L"--tree")) {
        wchar_t image[32768], line[32768], message[128];
        assert(GetModuleFileNameW(NULL, image, 32768));
        swprintf(line, 32768, L"\"%ls\" --grandson", image);
        STARTUPINFOW startup; memset(&startup, 0, sizeof(startup)); startup.cb = sizeof(startup);
        PROCESS_INFORMATION info;
        assert(CreateProcessW(NULL, line, NULL, NULL, FALSE, 0, NULL, NULL, &startup, &info));
        swprintf(message, 128, L"GRANDSON=%lu\r\n", info.dwProcessId); output_wide(message);
        CloseHandle(info.hThread); CloseHandle(info.hProcess); Sleep(INFINITE); return 0;
    }
    CONSOLE_SCREEN_BUFFER_INFO size;
    assert(GetConsoleScreenBufferInfo(GetStdHandle(STD_OUTPUT_HANDLE), &size));
    wchar_t cwd[32768], value[256], message[65536];
    assert(GetCurrentDirectoryW(32768, cwd));
    assert(GetEnvironmentVariableW(L"SENGOO_PTY_PROBE", value, 256));
    assert(!GetEnvironmentVariableW(L"SENGOO_PTY_PARENT_ONLY", message, 256));
    swprintf(message, 65536, L"READY \x4e2d\x6587 \xd83d\xde00 SIZE=%dx%d ARG=%ls END CWD=%ls ENV=%ls\r\n",
        (int)size.dwSize.X, (int)size.dwSize.Y, argc > 2 ? argv[2] : L"", cwd, value);
    output_wide(message);
    wchar_t input[512]; DWORD count = 0;
    assert(ReadConsoleW(GetStdHandle(STD_INPUT_HANDLE), input, 511, &count, NULL)); input[count] = 0;
    swprintf(message, 65536, L"INPUT=%ls", input); output_wide(message);
    assert(GetConsoleScreenBufferInfo(GetStdHandle(STD_OUTPUT_HANDLE), &size));
    swprintf(message, 65536, L"RESIZED=%dx%d\r\nDONE\r\n", (int)size.dwSize.X, (int)size.dwSize.Y); output_wide(message);
    return 7;
}
static long long executable_command(const char* executable, const char* mode) {
    long long command = sengoo_pty_command_new(raw(executable), (long long)strlen(executable)); assert(command > 0);
    assert(add_arg(command, mode) == 0); return command;
}
static void until(long long process, char* text, size_t capacity, const char* needle, DWORD timeout) {
    unsigned char bytes[4096]; SengooFfiBuffer buffer = {0}; buffer.bytes = bytes; buffer.capacity = sizeof(bytes);
    size_t used = strlen(text); ULONGLONG deadline = GetTickCount64() + timeout;
    do {
        long long count = sengoo_pty_process_read(process, raw(&buffer)); assert(count >= 0);
        assert(used + (size_t)count + 1 < capacity);
        memcpy(text + used, bytes, (size_t)count); used += (size_t)count; text[used] = 0;
        if (strstr(text, needle)) return;
        assert(sengoo_pty_process_poll(process, 1, 20) >= 0);
    } while (GetTickCount64() < deadline);
    fprintf(stderr, "missing %s in output:\n%s\n", needle, text); assert(0);
}
int wmain(int argc, wchar_t** argv) {
    if (argc > 1) return child(argc, argv);
    assert(sengoo_pty_available() == 1);
    DWORD before_handles; assert(GetProcessHandleCount(GetCurrentProcess(), &before_handles));
    wchar_t image[32768], temp[32768], directory[32768]; char executable[131072], cwd[131072];
    assert(GetModuleFileNameW(NULL, image, 32768));
    assert(WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, image, -1, executable, sizeof(executable), NULL, NULL));
    assert(GetTempPathW(32700, temp));
    swprintf(directory, 32768, L"%lsSengoo-Pty-\x4e2d\xd83d\xde00-%lu", temp, GetCurrentProcessId());
    assert(CreateDirectoryW(directory, NULL));
    assert(WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, directory, -1, cwd, sizeof(cwd), NULL, NULL));
    assert(SetEnvironmentVariableW(L"SENGOO_PTY_PARENT_ONLY", L"must-be-absent"));
    long long command = executable_command(executable, "--interactive");
    const char argument[] = "space \"quote\" \xe4\xb8\xad\xe6\x96\x87 \xf0\x9f\x98\x80 trailing\\";
    assert(add_arg(command, argument) == 0);
    assert(sengoo_pty_command_cwd(command, raw(cwd), (long long)strlen(cwd)) == 0);
    assert(sengoo_pty_command_inherit_env(command, 0) == 0);
    assert(sengoo_pty_command_env(command, raw("SENGOO_PTY_PROBE"), 16, raw("\xe5\x80\xbc"), 3, 0) == 0);
    assert(sengoo_pty_command_env(command, raw("sengoo_pty_probe"), 16, raw("\xe4\xb8\xad"), 3, 0) == 0);
    assert(sengoo_pty_command_arg(command, raw("a\0b"), 3) == -SENGOO_STATUS_INVALID_ARGUMENT);
    const unsigned char bad[] = {0xc0, 0x80};
    assert(sengoo_pty_command_arg(command, raw(bad), 2) == -SENGOO_STATUS_INVALID_UTF8);
    assert(sengoo_pty_command_spawn(command, 0, 28) == -SENGOO_STATUS_INVALID_ARGUMENT);
    long long process = sengoo_pty_command_spawn(command, 96, 28); assert(process > 0);
    assert(sengoo_pty_command_close(command) == 0); /* child owns independent launch inputs */
    char transcript[65536] = {0}; until(process, transcript, sizeof(transcript), "ENV=", 8000);
    assert(strstr(transcript, "SIZE=96x28") && strstr(transcript, "READY \xe4\xb8\xad\xe6\x96\x87 \xf0\x9f\x98\x80"));
    assert(strstr(transcript, argument) && strstr(transcript, cwd));
    assert(sengoo_pty_process_try_wait(process) == 0);
    assert(sengoo_pty_process_tree_quiescent(process) == 0);
    assert(sengoo_pty_process_reap(process, 0) == -SENGOO_STATUS_TIMEOUT);
    assert(sengoo_pty_process_resize(process, 73, 19) == 0);
    assert(sengoo_pty_process_resize(process, 1001, 19) == -SENGOO_STATUS_INVALID_ARGUMENT);
    char input[] = "\xe4\xbd\xa0\xe5\xa5\xbd \xf0\x9f\x98\x80\r";
    SengooFfiBuffer buffer = {0}; buffer.bytes = (unsigned char*)input; buffer.capacity = buffer.used_len = strlen(input);
    assert(sengoo_pty_process_write(process, raw(&buffer), (long long)buffer.used_len) == (long long)buffer.used_len);
    memset(input, 'x', strlen(input)); /* borrowed storage may immediately change */
    until(process, transcript, sizeof(transcript), "DONE", 8000);
    assert(strstr(transcript, "INPUT=\xe4\xbd\xa0\xe5\xa5\xbd \xf0\x9f\x98\x80"));
    assert(strstr(transcript, "RESIZED=73x19"));
    assert(sengoo_pty_process_reap(process, 8000) == 7 && sengoo_pty_process_try_wait(process) == 1);
    ULONGLONG quiescent_deadline = GetTickCount64() + 8000;
    while (!sengoo_pty_process_tree_quiescent(process)) { assert(GetTickCount64() < quiescent_deadline); Sleep(1); }
    ULONGLONG eof_deadline = GetTickCount64() + 8000;
    unsigned char drain[4096]; buffer.bytes = drain; buffer.capacity = sizeof(drain);
    while (!sengoo_pty_process_eof(process)) { assert(sengoo_pty_process_read(process, raw(&buffer)) >= 0); assert(GetTickCount64() < eof_deadline); Sleep(1); }
    assert(sengoo_pty_process_poll(process, 5, 0) == 12);
    assert(sengoo_pty_process_close(process) == 0 && sengoo_pty_process_close(0) == 0);
    assert(sengoo_pty_process_read(process, raw(&buffer)) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_pty_command_close(process) == -SENGOO_STATUS_INVALID_HANDLE);

    /* Failed creation must unwind console, job, pipe and launch allocations. */
    DWORD failure_baseline; assert(GetProcessHandleCount(GetCurrentProcess(), &failure_baseline));
    for (int attempt = 0; attempt < 8; attempt++) {
        command = executable_command("C:\\Sengoo-probe-missing-executable-does-not-exist.exe", "--unused");
        assert(sengoo_pty_command_spawn(command, 80, 24) == -SENGOO_STATUS_NOT_FOUND);
        assert(sengoo_pty_command_close(command) == 0);
    }
    DWORD failure_after; assert(GetProcessHandleCount(GetCurrentProcess(), &failure_after) && failure_after <= failure_baseline + 2);
    command = executable_command(executable, "--stall");
    for (int arg = 1; arg < SG_PTY_MAX_ARGS; arg++) assert(add_arg(command, "") == 0);
    assert(add_arg(command, "over-limit") == -SENGOO_STATUS_OVERFLOW);
    assert(sengoo_pty_command_env(command, raw("a=b"), 3, raw("x"), 1, 0) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_pty_command_env(command, raw("name"), 4, raw("x\0y"), 3, 0) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_pty_command_close(command) == 0);
    assert(sengoo_pty_command_close(command) == -SENGOO_STATUS_INVALID_HANDLE);

    /* Full input and unread output must not hold up owner cancellation. */
    for (int cycle = 0; cycle < 12; cycle++) {
        command = executable_command(executable, "--stall"); process = sengoo_pty_command_spawn(command, 80, 24); assert(process > 0);
        transcript[0] = 0; until(process, transcript, sizeof(transcript), "STALL", 8000);
        unsigned char fill[65536]; memset(fill, 'A', sizeof(fill)); buffer.bytes = fill; buffer.capacity = buffer.used_len = sizeof(fill);
        int backpressure = 0; ULONGLONG start = GetTickCount64();
        for (int i = 0; i < 1000; i++) { long long count = sengoo_pty_process_write(process, raw(&buffer), sizeof(fill)); assert(count >= 0); if (!count) { backpressure = 1; break; } }
        assert(backpressure && GetTickCount64() - start < 2000);
        start = GetTickCount64();
        for (int i = 0; i < 1000; i++) assert(sengoo_pty_process_write(process, raw(&buffer), sizeof(fill)) == 0);
        assert(GetTickCount64() - start < 2000);
        assert(sengoo_pty_process_reap(process, 1) == -SENGOO_STATUS_TIMEOUT);
        assert(sengoo_pty_process_close(process) == 0 && sengoo_pty_command_close(command) == 0);
        command = executable_command(executable, "--flood"); process = sengoo_pty_command_spawn(command, 80, 24); assert(process > 0);
        Sleep(30); start = GetTickCount64(); assert(sengoo_pty_process_close(process) == 0); assert(GetTickCount64() - start < 4000);
        assert(sengoo_pty_command_close(command) == 0);
    }
    command = executable_command(executable, "--tree"); process = sengoo_pty_command_spawn(command, 80, 24); assert(process > 0);
    transcript[0] = 0; until(process, transcript, sizeof(transcript), "GRANDSON=", 8000);
    /* Wait for the complete PID, which may be split across VT output reads. */
    until(process, transcript, sizeof(transcript), "\n", 8000);
    DWORD grandson_pid = (DWORD)strtoul(strstr(transcript, "GRANDSON=") + 9, NULL, 10);
    assert(grandson_pid); HANDLE grandson = OpenProcess(SYNCHRONIZE, FALSE, grandson_pid); assert(grandson);
    assert(sengoo_pty_process_terminate_tree(process) == 0 && sengoo_pty_process_reap(process, 8000) == 137);
    assert(WaitForSingleObject(grandson, 8000) == WAIT_OBJECT_0); CloseHandle(grandson);
    quiescent_deadline = GetTickCount64() + 8000;
    while (!sengoo_pty_process_tree_quiescent(process)) { assert(GetTickCount64() < quiescent_deadline); Sleep(1); }
    assert(sengoo_pty_process_close(process) == 0 && sengoo_pty_command_close(command) == 0);
    assert(sengoo_pty_live_handle_count() == 0 && RemoveDirectoryW(directory));
    assert(SetEnvironmentVariableW(L"SENGOO_PTY_PARENT_ONLY", NULL));
    DWORD after_handles; assert(GetProcessHandleCount(GetCurrentProcess(), &after_handles));
    assert(after_handles <= before_handles + 4);
    puts("ConPTY: isolated Unicode interactive echo, argv/cwd/env, resize, EOF, bounded backpressure, flood cancellation, grandchildren and handle cleanup passed");
    return 0;
}
#else
int main(void) { assert(sengoo_pty_available() == 0); puts("ConPTY unavailable on non-Windows"); return 0; }
#endif
