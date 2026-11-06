/* Real Windows console polling regression. No renderer or application state. */
#define _CRT_SECURE_NO_WARNINGS
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../runtime_terminal.c"

typedef struct { void* value; uint32_t generation; } PollSlot;
static PollSlot poll_slots[16];
long long sengoo_opaque_handle_new(void* value) {
    for (size_t i = 1; i < 16; i++) if (!poll_slots[i].value) {
        poll_slots[i].value = value; poll_slots[i].generation++;
        return ((long long)poll_slots[i].generation << 32) | (long long)i;
    }
    return 0;
}
void* sengoo_opaque_handle_get(long long handle) {
    size_t i = (size_t)((uint64_t)handle & UINT32_MAX);
    return handle > 0 && i < 16 && poll_slots[i].generation == ((uint64_t)handle >> 32)
        ? poll_slots[i].value : NULL;
}
void* sengoo_opaque_handle_take(long long handle) {
    void* value = sengoo_opaque_handle_get(handle);
    if (value) poll_slots[(uint64_t)handle & UINT32_MAX].value = NULL;
    return value;
}
long long sengoo_string_from_bytes_copy(long long bytes, long long length) {
    char* value = malloc((size_t)length + 1); assert(value);
    memcpy(value, (void*)(intptr_t)bytes, (size_t)length); value[length] = 0;
    return (long long)(intptr_t)value;
}

#ifdef _WIN32
static void key_record(INPUT_RECORD* event, wchar_t scalar, int down) {
    memset(event, 0, sizeof(*event)); event->EventType = KEY_EVENT;
    event->Event.KeyEvent.bKeyDown = down;
    event->Event.KeyEvent.wRepeatCount = 1;
    event->Event.KeyEvent.uChar.UnicodeChar = scalar;
}
static int enqueue(const INPUT_RECORD* events, DWORD size) {
    DWORD written = 0;
    return WriteConsoleInputW(GetStdHandle(STD_INPUT_HANDLE), events, size, &written)
        && written == size;
}
static int queued_sequence(void) {
    INPUT_RECORD events[7]; memset(events, 0, sizeof(events));
    events[0].EventType = FOCUS_EVENT;
    key_record(&events[1], L'x', 0);
    events[2].EventType = MOUSE_EVENT; /* mouse tracking is disabled */
    key_record(&events[3], 0xd83d, 1);
    key_record(&events[4], 0xde00, 1);
    key_record(&events[5], L'\x4e2d', 1);
    key_record(&events[6], L'z', 1);
    return enqueue(events, 7);
}
static int expect_key(long long handle, long long timeout, long long code) {
    long long kind = sengoo_terminal_read_event(handle, timeout);
    long long actual = sengoo_terminal_event_field(handle, 1);
    if (kind != SG_TERM_KEY || actual != code) {
        fprintf(stderr, "ready key expected %lld; timeout %lld returned kind %lld code %lld\n",
            code, timeout, kind, actual);
        return 0;
    }
    return 1;
}
static int native_poll(void) {
    long long handle = sengoo_terminal_open();
    if (handle <= 0 || sengoo_terminal_enter_raw(handle) != 1
        || sengoo_terminal_discard_input(handle) != 1) return 10;
    if (!queued_sequence() || !expect_key(handle, 0, 0x1f600)
        || !expect_key(handle, 0, 0x4e2d) || !expect_key(handle, 0, 'z')
        || sengoo_terminal_read_event(handle, 0) != 0) return 11;
    if (!queued_sequence() || !expect_key(handle, 1000, 0x1f600)
        || !expect_key(handle, 1000, 0x4e2d) || !expect_key(handle, 1000, 'z')) return 12;
    /* Split UTF-16 may span polls; a timeout must retain the high surrogate. */
    INPUT_RECORD event; key_record(&event, 0xd83d, 1);
    if (!enqueue(&event, 1) || sengoo_terminal_read_event(handle, 0) != 0) return 13;
    key_record(&event, 0xde00, 1);
    if (!enqueue(&event, 1) || !expect_key(handle, 0, 0x1f600)) return 14;
    /* A large ignored queue stays bounded and does not lose its trailing key. */
    INPUT_RECORD ignored[2048];
    for (size_t i = 0; i < 2048; i++) key_record(&ignored[i], L'x', 0);
    key_record(&event, L'q', 1);
    if (!enqueue(ignored, 2048) || !enqueue(&event, 1)) return 15;
    DWORD queued;
    if (!GetNumberOfConsoleInputEvents(GetStdHandle(STD_INPUT_HANDLE), &queued)) return 23;
    if (sengoo_terminal_read_event(handle, 0) != 0) return 16;
    DWORD pending;
    if (!GetNumberOfConsoleInputEvents(GetStdHandle(STD_INPUT_HANDLE), &pending)
        || pending < 2 || pending + 64 < queued) return 17;
    if (sengoo_terminal_read_event(handle, 1) != 0
        || !GetNumberOfConsoleInputEvents(GetStdHandle(STD_INPUT_HANDLE), &pending) || pending < 2) return 24;
    sengoo_terminal_cancelled = 1;
    if (sengoo_terminal_read_event(handle, 0) != SG_TERM_CANCEL) return 18;
    int found = 0;
    for (int i = 0; i < 256; i++) {
        long long kind = sengoo_terminal_read_event(handle, 0);
        if (kind == SG_TERM_KEY) {
            if (sengoo_terminal_event_field(handle, 1) != 'q') return 19;
            found = 1; break;
        }
        if (kind != 0) return 20;
    }
    if (!found || sengoo_terminal_read_event(handle, 0) != 0) return 21;
    if (sengoo_terminal_close(handle) != 0 || sengoo_terminal_live_handle_count() != 0) return 22;
    return 0;
}
static int public_poll(const wchar_t* program) {
    if (!FlushConsoleInputBuffer(GetStdHandle(STD_INPUT_HANDLE)) || !queued_sequence()) return 30;
    wchar_t command[32768]; swprintf(command, 32768, L"\"%ls\"", program);
    STARTUPINFOW start; PROCESS_INFORMATION child;
    memset(&start, 0, sizeof(start)); start.cb = sizeof(start);
    if (!CreateProcessW(program, command, NULL, NULL, FALSE, 0, NULL, NULL, &start, &child)) return 31;
    DWORD code = 99;
    int ok = WaitForSingleObject(child.hProcess, 15000) == WAIT_OBJECT_0
        && GetExitCodeProcess(child.hProcess, &code);
    if (!ok) TerminateProcess(child.hProcess, 99);
    CloseHandle(child.hThread); CloseHandle(child.hProcess);
    return ok ? (int)code : 32;
}
int wmain(int argc, wchar_t** argv) {
    if (argc >= 2 && !wcscmp(argv[1], L"--tty")) {
        int code = native_poll();
        if (!code && argc == 3) code = public_poll(argv[2]);
        return code;
    }
    wchar_t self[32768], command[32768];
    assert(GetModuleFileNameW(NULL, self, 32768));
    int count = swprintf(command, 32768, L"\"%ls\" --tty%ls%ls%ls", self,
        argc == 2 ? L" \"" : L"", argc == 2 ? argv[1] : L"", argc == 2 ? L"\"" : L"");
    assert(count > 0);
    STARTUPINFOW start; PROCESS_INFORMATION child;
    memset(&start, 0, sizeof(start)); start.cb = sizeof(start);
    start.dwFlags = STARTF_USESHOWWINDOW; start.wShowWindow = SW_HIDE;
    assert(CreateProcessW(self, command, NULL, NULL, FALSE, CREATE_NEW_CONSOLE, NULL, NULL, &start, &child));
    DWORD code = 99;
    assert(WaitForSingleObject(child.hProcess, 30000) == WAIT_OBJECT_0);
    assert(GetExitCodeProcess(child.hProcess, &code));
    CloseHandle(child.hThread); CloseHandle(child.hProcess);
    if (code) fprintf(stderr, "hidden console polling failed: %lu\n", code);
    assert(code == 0);
    puts(argc == 2
        ? "terminal polling: ready keys, bounded ignored queue, UTF-16 split, cancellation and public Sengoo API passed"
        : "terminal polling: ready keys, bounded ignored queue, UTF-16 split and cancellation passed");
    return 0;
}
#else
int main(void) {
    puts("Windows console record polling is not applicable on this host");
    return 0;
}
#endif
