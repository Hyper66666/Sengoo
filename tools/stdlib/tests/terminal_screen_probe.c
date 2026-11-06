/* Terminal transport contract: no renderer, editor or application state here. */
#define _CRT_SECURE_NO_WARNINGS
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../runtime_terminal.c"

typedef struct { void* pointer; uint32_t generation; } ProbeSlot;
static ProbeSlot slots[64];
long long sengoo_opaque_handle_new(void* p) {
    for (size_t i = 1; i < 64; i++) if (!slots[i].pointer) {
        slots[i].pointer = p; slots[i].generation++;
        return ((long long)slots[i].generation << 32) | (long long)i;
    }
    return 0;
}
void* sengoo_opaque_handle_get(long long h) {
    size_t i = (size_t)((uint64_t)h & UINT32_MAX);
    return h > 0 && i < 64 && slots[i].generation == ((uint64_t)h >> 32) ? slots[i].pointer : NULL;
}
void* sengoo_opaque_handle_take(long long h) {
    void* p = sengoo_opaque_handle_get(h);
    if (p) slots[(uint64_t)h & UINT32_MAX].pointer = NULL;
    return p;
}
long long sengoo_string_from_bytes_copy(long long p, long long n) {
    char* text = malloc((size_t)n + 1); assert(text);
    memcpy(text, (void*)(intptr_t)p, (size_t)n); text[n] = 0;
    return (long long)(intptr_t)text;
}
static void input(SengooTerminal* terminal, const char* packet) {
    size_t n = strlen(packet); assert(n <= sizeof(terminal->pending));
    memcpy(terminal->pending, packet, n); terminal->pending_len = n;
    terminal->code = terminal->modifiers = terminal->escape_since = 0;
    terminal->mouse_x = terminal->mouse_y = terminal->mouse_button = terminal->mouse_action = 0;
    terminal->scroll_x = terminal->scroll_y = terminal->mouse_clicks = 0;
}
static void parser_probe(void) {
    SengooTerminal* t = calloc(1, sizeof(*t)); assert(t); t->mouse = 1;
    input(t, "\033[<0;8;12M");
    assert(sengoo_terminal_parse(t) == SG_TERM_MOUSE && !t->pending_len);
    assert(t->mouse_x == 7 && t->mouse_y == 11 && t->mouse_button == 1 && t->mouse_action == 1 && t->mouse_clicks == 1);
    input(t, "\033[<22;2;3m");
    assert(sengoo_terminal_parse(t) == SG_TERM_MOUSE && t->mouse_button == 3 && t->mouse_action == 2 && t->modifiers == 5);
    input(t, "\033[<35;80;24M");
    assert(sengoo_terminal_parse(t) == SG_TERM_MOUSE && t->mouse_action == 3 && !t->mouse_button);
    input(t, "\033[<64;2;3M");
    assert(sengoo_terminal_parse(t) == SG_TERM_MOUSE && t->scroll_y == 120 && t->mouse_action == 4 && !t->mouse_button);
    input(t, "\033[<65;2;3M");
    assert(sengoo_terminal_parse(t) == SG_TERM_MOUSE && t->scroll_y == -120);
    input(t, "\033[<66;2;3M");
    assert(sengoo_terminal_parse(t) == SG_TERM_MOUSE && t->scroll_x == -120);
    input(t, "\033[<67;2;3M");
    assert(sengoo_terminal_parse(t) == SG_TERM_MOUSE && t->scroll_x == 120);
    input(t, "\033[<128;2;3M");
    assert(sengoo_terminal_parse(t) == SG_TERM_MOUSE && t->mouse_button == 4);
    input(t, "\033[<0;8;");
    assert(sengoo_terminal_parse(t) == 0);
    memcpy(t->pending + t->pending_len, "12M", 3); t->pending_len += 3;
    assert(sengoo_terminal_parse(t) == SG_TERM_MOUSE && t->mouse_y == 11);
    input(t, "\033[<0;0;3M");
    assert(sengoo_terminal_parse(t) == -SENGOO_STATUS_PARSE && !t->pending_len);
    input(t, "\033[<0;9999999999999999999999999;3M");
    assert(sengoo_terminal_parse(t) == -SENGOO_STATUS_PARSE);
    input(t, "\033[<1;x;3M");
    assert(sengoo_terminal_parse(t) == -SENGOO_STATUS_PARSE);
    input(t, "\xe4\xb8\xad\033[<0;2;3M\xf0\x9f\x98\x80");
    assert(sengoo_terminal_parse(t) == SG_TERM_KEY && t->code == 0x4e2d);
    assert(sengoo_terminal_parse(t) == SG_TERM_MOUSE && t->mouse_x == 1 && t->mouse_y == 2);
    assert(sengoo_terminal_parse(t) == SG_TERM_KEY && t->code == 0x1f600 && !t->pending_len);
    free(t);
}

#ifdef _WIN32
static int console_snapshot(DWORD in_mode, DWORD out_mode, UINT in_cp, UINT out_cp, CONSOLE_CURSOR_INFO* cursor) {
    DWORD in_now, out_now; CONSOLE_CURSOR_INFO now;
    return GetConsoleMode(GetStdHandle(STD_INPUT_HANDLE), &in_now)
        && GetConsoleMode(GetStdHandle(STD_OUTPUT_HANDLE), &out_now)
        && GetConsoleCursorInfo(GetStdHandle(STD_OUTPUT_HANDLE), &now)
        && in_now == in_mode && out_now == out_mode && GetConsoleCP() == in_cp && GetConsoleOutputCP() == out_cp
        && now.bVisible == cursor->bVisible && now.dwSize == cursor->dwSize;
}
static int inject(long long handle, MOUSE_EVENT_RECORD mouse, int button, int action, int clicks, int sx, int sy) {
    INPUT_RECORD record; memset(&record, 0, sizeof(record)); record.EventType = MOUSE_EVENT; record.Event.MouseEvent = mouse;
    DWORD count;
    if (!WriteConsoleInputW(GetStdHandle(STD_INPUT_HANDLE), &record, 1, &count) || count != 1) return 0;
    if (sengoo_terminal_read_event(handle, 1000) != SG_TERM_MOUSE) return 0;
    SengooTerminal* t = sengoo_terminal_resolve(handle);
    return t && sengoo_terminal_event_field(handle, 7) == button && sengoo_terminal_event_field(handle, 8) == action
        && sengoo_terminal_event_field(handle, 9) == sx && sengoo_terminal_event_field(handle, 10) == sy
        && sengoo_terminal_event_field(handle, 11) == clicks;
}
static int tty_probe(void) {
    HANDLE in = GetStdHandle(STD_INPUT_HANDLE), out = GetStdHandle(STD_OUTPUT_HANDLE);
    DWORD in_mode, out_mode; CONSOLE_CURSOR_INFO cursor;
    if (!GetConsoleMode(in, &in_mode) || !GetConsoleMode(out, &out_mode) || !GetConsoleCursorInfo(out, &cursor)) return 10;
    UINT in_cp = GetConsoleCP(), out_cp = GetConsoleOutputCP();
    cursor.bVisible = FALSE; cursor.dwSize = 37;
    if (!SetConsoleCursorInfo(out, &cursor)) return 11;
    long long handle = sengoo_terminal_open();
    if (handle <= 0 || sengoo_terminal_is_tty(handle) != 1 || sengoo_terminal_enter_fullscreen(handle) != 1
        || sengoo_terminal_enter_fullscreen(handle) != 1 || !sengoo_terminal_resolve(handle)->raw
        || !sengoo_terminal_resolve(handle)->mouse || sengoo_terminal_discard_input(handle) != 1) return 12;
    DWORD active;
    if (!GetConsoleMode(in, &active) || !(active & ENABLE_MOUSE_INPUT) || (active & ENABLE_QUICK_EDIT_MODE)) return 13;
    CONSOLE_SCREEN_BUFFER_INFO info;
    if (!GetConsoleScreenBufferInfo(out, &info)) return 14;
    MOUSE_EVENT_RECORD event; memset(&event, 0, sizeof(event));
    event.dwMousePosition.X = info.srWindow.Left + 6; event.dwMousePosition.Y = info.srWindow.Top + 4;
    event.dwButtonState = FROM_LEFT_1ST_BUTTON_PRESSED; event.dwControlKeyState = SHIFT_PRESSED | LEFT_CTRL_PRESSED;
    if (!inject(handle, event, 1, 1, 1, 0, 0)
        || sengoo_terminal_event_field(handle, 5) != 6 || sengoo_terminal_event_field(handle, 6) != 4
        || sengoo_terminal_event_field(handle, 2) != 5) return 15;
    event.dwEventFlags = MOUSE_MOVED;
    if (!inject(handle, event, 1, 3, 0, 0, 0)) return 16;
    event.dwEventFlags = 0; event.dwButtonState = 0;
    if (!inject(handle, event, 1, 2, 0, 0, 0)) return 17;
    event.dwEventFlags = DOUBLE_CLICK; event.dwButtonState = RIGHTMOST_BUTTON_PRESSED;
    if (!inject(handle, event, 3, 1, 2, 0, 0)) return 18;
    event.dwEventFlags = MOUSE_WHEELED; event.dwButtonState = ((DWORD)(WORD)(SHORT)-240) << 16;
    if (!inject(handle, event, 0, 4, 0, 0, -240)) return 19;
    event.dwEventFlags = MOUSE_HWHEELED; event.dwButtonState = ((DWORD)(WORD)120) << 16;
    if (!inject(handle, event, 0, 4, 0, 120, 0)) return 20;
    if (sengoo_terminal_discard_input(handle) != 1 || sengoo_terminal_read_event(handle, 0) != 0
        || sengoo_terminal_event_field(handle, 9) != 0 || sengoo_terminal_event_field(handle, 10) != 0) return 21;
    /* Windows coalesces key-down records; preserve every reported repeat. */
    INPUT_RECORD repeated; memset(&repeated, 0, sizeof(repeated));
    repeated.EventType = KEY_EVENT; repeated.Event.KeyEvent.bKeyDown = TRUE;
    repeated.Event.KeyEvent.wRepeatCount = 3; repeated.Event.KeyEvent.wVirtualKeyCode = VK_DOWN;
    repeated.Event.KeyEvent.dwControlKeyState = SHIFT_PRESSED;
    DWORD written;
    if (!WriteConsoleInputW(in, &repeated, 1, &written) || written != 1) return 31;
    for (int i = 0; i < 3; i++) {
        if (sengoo_terminal_read_event(handle, 1000) != SG_TERM_KEY
            || sengoo_terminal_event_field(handle, 1) != 1002
            || sengoo_terminal_event_field(handle, 2) != 1) return 32;
    }
    if (sengoo_terminal_read_event(handle, 0) != 0) return 33;
    repeated.Event.KeyEvent.uChar.UnicodeChar = 0x4e2d;
    repeated.Event.KeyEvent.wVirtualKeyCode = 0; repeated.Event.KeyEvent.dwControlKeyState = 0;
    if (!WriteConsoleInputW(in, &repeated, 1, &written) || written != 1) return 34;
    for (int i = 0; i < 3; i++) {
        if (sengoo_terminal_read_event(handle, 1000) != SG_TERM_KEY
            || sengoo_terminal_event_field(handle, 1) != 0x4e2d) return 35;
    }
    repeated.Event.KeyEvent.wRepeatCount = 4;
    if (!WriteConsoleInputW(in, &repeated, 1, &written) || written != 1
        || sengoo_terminal_read_event(handle, 1000) != SG_TERM_KEY
        || sengoo_terminal_discard_input(handle) != 1 || sengoo_terminal_read_event(handle, 0) != 0) return 36;
    if (sengoo_terminal_set_mouse(handle, 0) != 1 || sengoo_terminal_set_mouse(handle, 2) != -SENGOO_STATUS_INVALID_ARGUMENT
        || sengoo_terminal_leave_fullscreen(handle) != 1 || sengoo_terminal_leave_fullscreen(handle) != 1
        || !console_snapshot(in_mode, out_mode, in_cp, out_cp, &cursor)) return 22;
    /* Starting from raw preserves raw and the caller's mouse preference. */
    if (sengoo_terminal_enter_raw(handle) != 1 || sengoo_terminal_enter_fullscreen(handle) != 1
        || sengoo_terminal_leave_fullscreen(handle) != 1 || !sengoo_terminal_resolve(handle)->raw
        || sengoo_terminal_resolve(handle)->mouse || sengoo_terminal_restore(handle) != 1) return 23;
    if (sengoo_terminal_enter_raw(handle) != 1 || sengoo_terminal_set_mouse(handle, 1) != 1
        || sengoo_terminal_enter_fullscreen(handle) != 1 || sengoo_terminal_leave_fullscreen(handle) != 1
        || !sengoo_terminal_resolve(handle)->mouse || sengoo_terminal_restore(handle) != 1) return 24;
    /* Drop while fullscreen restores console modes and previous visibility. */
    if (!SetConsoleCursorInfo(out, &cursor) || sengoo_terminal_enter_fullscreen(handle) != 1
        || sengoo_terminal_close(handle) != 0 || !console_snapshot(in_mode, out_mode, in_cp, out_cp, &cursor)
        || sengoo_terminal_live_handle_count() != 0) return 25;
    if (sengoo_terminal_enter_fullscreen(handle) != -SENGOO_STATUS_INVALID_HANDLE) return 26;
    /* Normal exit must unwind even without explicitly closing the handle. */
    wchar_t self[32768], command[32768]; if (!GetModuleFileNameW(NULL, self, 32768)) return 27;
    swprintf(command, 32768, L"\"%ls\" --normal-exit", self);
    STARTUPINFOW start; PROCESS_INFORMATION process; memset(&start, 0, sizeof(start)); start.cb = sizeof(start);
    if (!CreateProcessW(self, command, NULL, NULL, FALSE, 0, NULL, NULL, &start, &process)) return 28;
    DWORD code;
    if (WaitForSingleObject(process.hProcess, 10000) != WAIT_OBJECT_0 || !GetExitCodeProcess(process.hProcess, &code)) return 29;
    CloseHandle(process.hThread); CloseHandle(process.hProcess);
    if (code != 0 || !console_snapshot(in_mode, out_mode, in_cp, out_cp, &cursor)) return 30;
    return 0;
}
static void real_console_probe(void) {
    wchar_t self[32768], command[32768]; assert(GetModuleFileNameW(NULL, self, 32768));
    swprintf(command, 32768, L"\"%ls\" --tty", self);
    STARTUPINFOW start; PROCESS_INFORMATION child; memset(&start, 0, sizeof(start)); start.cb = sizeof(start);
    start.dwFlags = STARTF_USESHOWWINDOW; start.wShowWindow = SW_HIDE;
    assert(CreateProcessW(self, command, NULL, NULL, FALSE, CREATE_NEW_CONSOLE, NULL, NULL, &start, &child));
    DWORD code;
    assert(WaitForSingleObject(child.hProcess, 15000) == WAIT_OBJECT_0);
    assert(GetExitCodeProcess(child.hProcess, &code));
    if (code) fprintf(stderr, "hidden console probe failed: %lu\n", code);
    CloseHandle(child.hThread); CloseHandle(child.hProcess); assert(code == 0);
}
#endif
int main(int argc, char** argv) {
#ifdef _WIN32
    if (argc == 2 && !strcmp(argv[1], "--tty")) return tty_probe();
    if (argc == 2 && !strcmp(argv[1], "--normal-exit")) {
        long long handle = sengoo_terminal_open();
        return handle > 0 && sengoo_terminal_enter_fullscreen(handle) == 1 ? 0 : 80;
    }
    real_console_probe();
#else
    (void)argc; (void)argv;
#endif
    parser_probe();
    long long handle = sengoo_terminal_open(); assert(handle > 0);
    assert(sengoo_terminal_is_tty(handle) == 0);
    assert(sengoo_terminal_enter_fullscreen(handle) == -SENGOO_STATUS_UNSUPPORTED);
    assert(sengoo_terminal_set_mouse(handle, 1) == -SENGOO_STATUS_UNSUPPORTED);
    assert(sengoo_terminal_close(handle) == 0 && sengoo_terminal_live_handle_count() == 0);
    assert(sengoo_terminal_leave_fullscreen(handle) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_terminal_set_mouse(handle, 0) == -SENGOO_STATUS_INVALID_HANDLE);
    puts("terminal: fullscreen ownership, restore, native mouse and fragmented SGR parser passed");
    return 0;
}
