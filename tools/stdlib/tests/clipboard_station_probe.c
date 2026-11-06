/* Real clipboard integration is permitted only in a freshly created private
 * window station AND desktop. Exit 77 records denied isolation, never falls
 * back to the interactive clipboard. Optional consumer runs in that station. */
#define _CRT_SECURE_NO_WARNINGS
#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>
#define main terminal_probe_main
#include "terminal_screen_probe.c"
#undef main
typedef struct SgDropFiles { DWORD pFiles; POINT pt; BOOL fNC; BOOL fWide; } SgDropFiles;
static void set_clipboard(HWND owner, UINT format, HGLOBAL memory) {
    assert(OpenClipboard(owner)); assert(EmptyClipboard()); assert(SetClipboardData(format, memory)); assert(CloseClipboard());
}
int main(int argc, char** argv) {
    const char* directory = NULL; const char* consumer = NULL; const char* kind = "files";
    for (int index = 1; index < argc; index++) {
        if (!strcmp(argv[index], "--cwd") && index + 1 < argc) directory = argv[++index];
        else if (!strcmp(argv[index], "--consumer") && index + 1 < argc) consumer = argv[++index];
        else if (!strcmp(argv[index], "--kind") && index + 1 < argc) kind = argv[++index];
        else { fprintf(stderr, "Usage: --cwd DIR [--consumer EXE] [--kind files|image|text]\n"); return 2; }
    }
    if (!directory) return 2;
    wchar_t station_name[80], desktop_name[] = L"SengooClipboardFixture";
    swprintf(station_name, 80, L"SengooClipboard-%lu-%llu", GetCurrentProcessId(), (unsigned long long)GetTickCount64());
    HWINSTA original_station = GetProcessWindowStation(); HDESK original_desktop = GetThreadDesktop(GetCurrentThreadId());
    HWINSTA station = CreateWindowStationW(station_name, 0, WINSTA_ALL_ACCESS, NULL);
    if (!station) { fprintf(stderr, "isolation unavailable: CreateWindowStation error=%lu; clipboard untouched\n", GetLastError()); return 77; }
    if (!SetProcessWindowStation(station)) { fprintf(stderr, "isolation unavailable: SetProcessWindowStation error=%lu; clipboard untouched\n", GetLastError()); CloseWindowStation(station); return 77; }
    HDESK desktop = CreateDesktopW(desktop_name, NULL, NULL, 0,
        DESKTOP_CREATEWINDOW | DESKTOP_READOBJECTS | DESKTOP_WRITEOBJECTS | DESKTOP_SWITCHDESKTOP, NULL);
    if (!desktop || !SetThreadDesktop(desktop)) { fprintf(stderr, "isolation unavailable: private desktop error=%lu; clipboard untouched\n", GetLastError()); SetProcessWindowStation(original_station); if (desktop) CloseDesktop(desktop); CloseWindowStation(station); return 77; }
    HWND owner = CreateWindowExW(0, L"STATIC", L"Owned clipboard fixture", 0, 0, 0, 0, 0, HWND_MESSAGE, NULL, GetModuleHandleW(NULL), NULL); assert(owner);
    size_t dib_size = sizeof(BITMAPV5HEADER) + 16; HGLOBAL dib = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, dib_size); assert(dib);
    BITMAPV5HEADER* bitmap = GlobalLock(dib); bitmap->bV5Size = sizeof(*bitmap); bitmap->bV5Width = 2; bitmap->bV5Height = 2; bitmap->bV5Planes = 1; bitmap->bV5BitCount = 32; bitmap->bV5SizeImage = 16;
    memset((char*)bitmap + sizeof(*bitmap), 128, 16); GlobalUnlock(dib); set_clipboard(owner, CF_DIBV5, dib);
    char output[32768]; assert(strlen(directory) + 30 < sizeof(output)); snprintf(output, sizeof(output), "%s/owned-station-original.png", directory);
    assert(sengoo_terminal_clipboard_has_image() == 1);
    assert(sengoo_terminal_clipboard_image_save((intptr_t)output, strlen(output), 65536) == 1);
    if (!strcmp(kind, "files")) {
        int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, output, -1, NULL, 0); assert(count > 0);
        HGLOBAL files = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, sizeof(SgDropFiles) + ((size_t)count + 1) * sizeof(wchar_t)); assert(files);
        SgDropFiles* drop = GlobalLock(files); drop->pFiles = sizeof(*drop); drop->fWide = TRUE; assert(MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, output, -1, (wchar_t*)((char*)drop + sizeof(*drop)), count)); GlobalUnlock(files);
        set_clipboard(owner, CF_HDROP, files);
        long long paths = sengoo_terminal_clipboard_files(); assert(paths > 0); free((void*)(intptr_t)paths);
    } else if (!strcmp(kind, "text")) {
        const wchar_t text[] = L"owned clipboard \x4e2d\x6587"; HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, sizeof(text)); assert(memory); void* p = GlobalLock(memory); memcpy(p, text, sizeof(text)); GlobalUnlock(memory); set_clipboard(owner, CF_UNICODETEXT, memory);
    } else assert(!strcmp(kind, "image"));
    int exit_code = 0;
    if (consumer) {
        long long error = 0; wchar_t* image = sg_clip_path((intptr_t)consumer, strlen(consumer), &error); wchar_t* cwd = sg_clip_path((intptr_t)directory, strlen(directory), &error); assert(image && cwd);
        size_t capacity = wcslen(image) + wcslen(cwd) + 32; wchar_t* command = malloc(capacity * sizeof(wchar_t)); assert(command); swprintf(command, capacity, L"\"%ls\" --cwd \"%ls\"", image, cwd);
        wchar_t desktop_path[170]; swprintf(desktop_path, 170, L"%ls\\%ls", station_name, desktop_name);
        STARTUPINFOW start; PROCESS_INFORMATION process; memset(&start, 0, sizeof(start)); memset(&process, 0, sizeof(process)); start.cb = sizeof(start); start.lpDesktop = desktop_path;
        assert(CreateProcessW(image, command, NULL, NULL, FALSE, CREATE_NO_WINDOW, NULL, cwd, &start, &process));
        DWORD waited = WaitForSingleObject(process.hProcess, 30000);
        if (waited != WAIT_OBJECT_0) { assert(TerminateProcess(process.hProcess, 80)); assert(WaitForSingleObject(process.hProcess, 5000) == WAIT_OBJECT_0); }
        DWORD code = 0; assert(GetExitCodeProcess(process.hProcess, &code)); exit_code = (int)code;
        CloseHandle(process.hThread); CloseHandle(process.hProcess); free(command); free(image); free(cwd);
    }
    assert(OpenClipboard(owner)); assert(EmptyClipboard()); assert(CloseClipboard()); DestroyWindow(owner);
    assert(SetThreadDesktop(original_desktop)); assert(SetProcessWindowStation(original_station)); CloseDesktop(desktop); CloseWindowStation(station);
    assert(DeleteFileA(output));
    if (!exit_code) puts("real clipboard private station fixture passed; interactive station untouched");
    return exit_code;
}
