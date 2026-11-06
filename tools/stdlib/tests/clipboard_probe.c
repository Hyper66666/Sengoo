/* Never touches the interactive clipboard. Win32 clipboard/window calls are
 * replaced by an isolated state fixture; UTF conversion and GlobalAlloc are
 * real. This covers ownership and failure cleanup, not a real clipboard test. */
#define _CRT_SECURE_NO_WARNINGS
#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#ifdef _WIN32
#include <windows.h>
static HGLOBAL clipboard_memory;
static int live_memory, live_library, live_window, open_count, opened, failure;
static HGLOBAL WINAPI allocate(UINT flags, SIZE_T count) {
    HGLOBAL p = GlobalAlloc(flags, count); if (p) live_memory++; return p;
}
static HGLOBAL WINAPI release(HGLOBAL p) {
    HGLOBAL result = GlobalFree(p); if (!result) live_memory--; return result;
}
static BOOL WINAPI clipboard_open(HWND window) {
    assert(window == (HWND)(uintptr_t)2); open_count++;
    if (failure == 1) return FALSE;
    assert(!opened); opened = 1; return TRUE;
}
static BOOL WINAPI clipboard_close(void) { assert(opened); opened = 0; return TRUE; }
static BOOL WINAPI clipboard_empty(void) {
    assert(opened); if (failure == 2) return FALSE;
    if (clipboard_memory) release(clipboard_memory);
    clipboard_memory = NULL; return TRUE;
}
static HANDLE WINAPI clipboard_set(UINT format, HANDLE memory) {
    assert(opened && format == CF_UNICODETEXT && memory);
    if (failure == 3) return NULL;
    assert(!clipboard_memory); clipboard_memory = memory; return memory;
}
static HWND WINAPI clipboard_window(DWORD ex, LPCWSTR cls, LPCWSTR title, DWORD style, int x, int y, int w, int h,
    HWND parent, HMENU menu, HINSTANCE instance, LPVOID parameter) {
    (void)ex; (void)title; (void)style; (void)x; (void)y; (void)w; (void)h; (void)menu; (void)instance; (void)parameter;
    assert(!wcscmp(cls, L"STATIC") && parent == HWND_MESSAGE);
    if (failure == 4) return NULL;
    live_window++; return (HWND)(uintptr_t)2;
}
static BOOL WINAPI clipboard_destroy(HWND window) { assert(window == (HWND)(uintptr_t)2); live_window--; return TRUE; }
static HMODULE WINAPI isolated_library(LPCWSTR name, HANDLE file, DWORD flags) {
    assert(!wcscmp(name, L"user32.dll") && !file && flags == LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (failure == 5) return NULL;
    live_library++; return (HMODULE)(uintptr_t)1;
}
static BOOL WINAPI isolated_free(HMODULE library) { assert(library == (HMODULE)(uintptr_t)1); live_library--; return TRUE; }
static FARPROC WINAPI isolated_symbol(HMODULE library, LPCSTR symbol) {
    assert(library == (HMODULE)(uintptr_t)1);
    if (failure == 6) return NULL;
    if (!strcmp(symbol, "OpenClipboard")) return (FARPROC)clipboard_open;
    if (!strcmp(symbol, "CloseClipboard")) return (FARPROC)clipboard_close;
    if (!strcmp(symbol, "EmptyClipboard")) return (FARPROC)clipboard_empty;
    if (!strcmp(symbol, "SetClipboardData")) return (FARPROC)clipboard_set;
    if (!strcmp(symbol, "CreateWindowExW")) return (FARPROC)clipboard_window;
    if (!strcmp(symbol, "DestroyWindow")) return (FARPROC)clipboard_destroy;
    assert(!"unexpected imported symbol"); return NULL;
}
#define LoadLibraryExW isolated_library
#define GetProcAddress isolated_symbol
#define FreeLibrary isolated_free
#define GlobalAlloc allocate
#define GlobalFree release
#endif
#define main terminal_contract_main
#include "terminal_screen_probe.c"
#undef main
int main(void) {
#ifdef _WIN32
    const unsigned char text[] = "\xe4\xb8\xad\xe6\x96\x87 \xf0\x9f\x98\x80\nsecond line";
    assert(sengoo_terminal_copy_text((long long)(intptr_t)text, sizeof(text) - 1) == 1);
    wchar_t* copied = GlobalLock(clipboard_memory);
    assert(copied && !wcscmp(copied, L"\x4e2d\x6587 \xd83d\xde00\nsecond line")); GlobalUnlock(clipboard_memory);
    assert(!live_library && !live_window && !opened && live_memory == 1);
    int before = open_count;
    const unsigned char invalid[] = {0xc0, 0x80};
    assert(sengoo_terminal_copy_text((long long)(intptr_t)invalid, 2) == -SENGOO_STATUS_INVALID_UTF8);
    assert(sengoo_terminal_copy_text((long long)(intptr_t)"a\0b", 3) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_terminal_copy_text(0, 1) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_terminal_copy_text((long long)(intptr_t)text, SENGOO_TERMINAL_LIMIT + 1) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(open_count == before && live_memory == 1);
    assert(sengoo_terminal_copy_text(0, 0) == 1);
    copied = GlobalLock(clipboard_memory); assert(copied && !copied[0]); GlobalUnlock(clipboard_memory);
    release(clipboard_memory); clipboard_memory = NULL;
    for (int i = 1; i <= 6; i++) {
        failure = i;
        assert(sengoo_terminal_copy_text((long long)(intptr_t)text, sizeof(text) - 1) < 0);
        assert(!live_library && !live_window && !opened && live_memory == 0 && !clipboard_memory);
    }
    failure = 0;
    for (int i = 0; i < 100; i++) {
        assert(sengoo_terminal_copy_text((long long)(intptr_t)text, sizeof(text) - 1) == 1);
        assert(live_memory == 1 && !live_library && !live_window && !opened);
    }
    release(clipboard_memory); clipboard_memory = NULL; assert(live_memory == 0);
    puts("clipboard: isolated Win32 fixture, Unicode, empty/NUL/invalid input and failure ownership passed");
#else
    assert(sengoo_terminal_copy_text(0, 0) == -SENGOO_STATUS_UNSUPPORTED);
    puts("clipboard: non-Windows explicitly unsupported");
#endif
    return 0;
}
