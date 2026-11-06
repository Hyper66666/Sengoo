/* BackTab decoder and owned hidden-console parity. No application policy. */
#define _CRT_SECURE_NO_WARNINGS
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../runtime_terminal.c"

typedef struct { void* value; uint32_t generation; } BacktabSlot;
static BacktabSlot backtab_slots[16];
long long sengoo_opaque_handle_new(void* value) {
    for (size_t i = 1; i < 16; i++) if (!backtab_slots[i].value) {
        backtab_slots[i].value = value; backtab_slots[i].generation++;
        return ((long long)backtab_slots[i].generation << 32) | (long long)i;
    } return 0;
}
void* sengoo_opaque_handle_get(long long handle) {
    size_t i = (size_t)((uint64_t)handle & UINT32_MAX);
    return handle > 0 && i < 16 && backtab_slots[i].generation == ((uint64_t)handle >> 32)
        ? backtab_slots[i].value : NULL;
}
void* sengoo_opaque_handle_take(long long handle) {
    void* value = sengoo_opaque_handle_get(handle);
    if (value) backtab_slots[(uint64_t)handle & UINT32_MAX].value = NULL;
    return value;
}
long long sengoo_string_from_bytes_copy(long long bytes, long long length) {
    char* value = malloc((size_t)length + 1); assert(value);
    memcpy(value, (void*)(intptr_t)bytes, (size_t)length); value[length] = 0;
    return (long long)(intptr_t)value;
}

static int decoded(SengooTerminal* terminal, long long code, long long modifiers, size_t pending) {
    long long kind = sengoo_terminal_parse(terminal);
    if (kind != SG_TERM_KEY || terminal->code != code || terminal->modifiers != modifiers
        || terminal->pending_len != pending) {
        fprintf(stderr, "expected key %lld modifiers %lld pending %zu; got kind %lld key %lld modifiers %lld pending %zu\n",
            code, modifiers, pending, kind, terminal->code, terminal->modifiers, terminal->pending_len);
        return 0;
    } return 1;
}
static int byte_parser(void) {
    SengooTerminal terminal; memset(&terminal, 0, sizeof(terminal));
    memcpy(terminal.pending, "\033[Z\tx", 5); terminal.pending_len = 5;
    if (!decoded(&terminal, 9, 1, 2)) return 11;
    terminal.modifiers = 0;
    if (!decoded(&terminal, 9, 0, 1)) return 12;
    if (!decoded(&terminal, 'x', 0, 0)) return 13;
    for (size_t split = 1; split < 3; split++) {
        memset(&terminal, 0, sizeof(terminal));
        memcpy(terminal.pending, "\033[Z", split); terminal.pending_len = split;
        if (sengoo_terminal_parse(&terminal) != 0 || terminal.pending_len != split) return 14;
        memcpy(terminal.pending + split, &"\033[Z"[split], 3 - split); terminal.pending_len = 3;
        if (!decoded(&terminal, 9, 1, 0)) return 15;
    }
    memset(&terminal, 0, sizeof(terminal));
    memcpy(terminal.pending, "\033[Z", 3); terminal.pending_len = 3; terminal.eof = 1;
    if (!decoded(&terminal, 9, 1, 0)) return 16;
    memset(&terminal, 0, sizeof(terminal));
    memcpy(terminal.pending, "\033[200~\033[Z\033[201~", 15); terminal.pending_len = 15;
    if (sengoo_terminal_parse(&terminal) != SG_TERM_PASTE || terminal.text_len != 3
        || memcmp(terminal.text, "\033[Z", 3)) return 17;
    return 0;
}

#ifdef _WIN32
static int native_keys(void) {
    long long handle = sengoo_terminal_open();
    if (handle <= 0 || sengoo_terminal_enter_raw(handle) != 1
        || sengoo_terminal_discard_input(handle) != 1) return 21;
    INPUT_RECORD events[2]; memset(events, 0, sizeof(events));
    for (size_t i = 0; i < 2; i++) {
        events[i].EventType = KEY_EVENT; events[i].Event.KeyEvent.bKeyDown = TRUE;
        events[i].Event.KeyEvent.wRepeatCount = 1; events[i].Event.KeyEvent.wVirtualKeyCode = VK_TAB;
        events[i].Event.KeyEvent.uChar.UnicodeChar = L'\t';
    }
    events[0].Event.KeyEvent.dwControlKeyState = SHIFT_PRESSED;
    DWORD written = 0;
    if (!WriteConsoleInputW(GetStdHandle(STD_INPUT_HANDLE), events, 2, &written) || written != 2) return 22;
    for (int i = 0; i < 2; i++) if (sengoo_terminal_read_event(handle, 0) != SG_TERM_KEY
        || sengoo_terminal_event_field(handle, 1) != 9
        || sengoo_terminal_event_field(handle, 2) != (i == 0 ? 1 : 0)) return 23;
    if (sengoo_terminal_close(handle) != 0 || sengoo_terminal_live_handle_count() != 0) return 24;
    return 0;
}
static int hidden_console(void) {
    wchar_t self[32768], command[32768];
    if (!GetModuleFileNameW(NULL, self, 32768)) return 25;
    swprintf(command, 32768, L"\"%ls\" --console", self);
    STARTUPINFOW start; PROCESS_INFORMATION child;
    memset(&start, 0, sizeof(start)); start.cb = sizeof(start);
    start.dwFlags = STARTF_USESHOWWINDOW; start.wShowWindow = SW_HIDE;
    if (!CreateProcessW(self, command, NULL, NULL, FALSE, CREATE_NEW_CONSOLE, NULL, NULL, &start, &child)) return 26;
    DWORD code = 99;
    int exited = WaitForSingleObject(child.hProcess, 10000) == WAIT_OBJECT_0;
    if (exited) GetExitCodeProcess(child.hProcess, &code); else TerminateProcess(child.hProcess, 99);
    CloseHandle(child.hThread); CloseHandle(child.hProcess);
    return (int)code;
}
int wmain(int argc, wchar_t** argv) {
    if (argc == 2 && !wcscmp(argv[1], L"--console")) return native_keys();
    int code = byte_parser(); if (code) return code;
    code = hidden_console(); if (code) return code;
    puts("BackTab: complete/split byte decoder, plain Tab, paste literal and owned Windows Shift+Tab parity passed");
    return 0;
}
#else
int main(void) { int code = byte_parser(); if (!code) puts("BackTab byte decoder passed; native Windows console not applicable"); return code; }
#endif
