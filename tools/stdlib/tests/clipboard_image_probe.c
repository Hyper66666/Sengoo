/* All clipboard calls are an owned in-memory fixture. GDI+, COM streams,
 * Unicode conversion and filesystem writes are real Windows system APIs.
 * This does not inspect, empty, or replace the interactive user's clipboard. */
#define _CRT_SECURE_NO_WARNINGS
#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>
#include <shellapi.h>
#include <objidl.h>
typedef struct SgDropFiles { DWORD pFiles; POINT pt; BOOL fNC; BOOL fWide; } SgDropFiles;
static HGLOBAL image_memory, text_memory, files_memory;
static UINT image_format;
static int opened, library_count, open_calls;
static int failure;
static int (WINAPI *real_image_save)(void*, IStream*, const CLSID*, const void*);
static int WINAPI fixture_image_save(void* image, IStream* stream, const CLSID* codec, const void* parameters) {
    /* GDI+ must not outlive a clipboard-owned HBITMAP. Owned DIB/PNG snapshots
     * can release the clipboard before encoding. */
    assert(opened == (image_format == CF_BITMAP));
    return real_image_save(image, stream, codec, parameters);
}
static BOOL WINAPI fixture_open(HWND owner) { assert(!owner && !opened); open_calls++; if (failure) return FALSE; opened = 1; return TRUE; }
static BOOL WINAPI fixture_close(void) { assert(opened); opened = 0; return TRUE; }
static BOOL WINAPI fixture_available(UINT format) { return (image_memory && format == image_format) || (text_memory && format == CF_UNICODETEXT) || (files_memory && format == CF_HDROP); }
static HANDLE WINAPI fixture_get(UINT format) { assert(opened); if (format == image_format) return image_memory; if (format == CF_UNICODETEXT) return text_memory; if (format == CF_HDROP) return files_memory; return NULL; }
static UINT WINAPI fixture_format(LPCWSTR name) { assert(!wcscmp(name, L"PNG")); return 0xc001; }
static HMODULE WINAPI fixture_library(LPCWSTR name, HANDLE file, DWORD flags) {
    if (!wcscmp(name, L"user32.dll")) { assert(!file && flags == LOAD_LIBRARY_SEARCH_SYSTEM32); library_count++; return (HMODULE)(uintptr_t)7; }
    return LoadLibraryExW(name, file, flags);
}
static FARPROC WINAPI fixture_symbol(HMODULE library, LPCSTR symbol) {
    if (library != (HMODULE)(uintptr_t)7) {
        FARPROC actual = GetProcAddress(library, symbol);
        if (!strcmp(symbol, "GdipSaveImageToStream")) { real_image_save = (void*)actual; return (FARPROC)fixture_image_save; }
        return actual;
    }
    if (!strcmp(symbol, "OpenClipboard")) return (FARPROC)fixture_open;
    if (!strcmp(symbol, "CloseClipboard")) return (FARPROC)fixture_close;
    if (!strcmp(symbol, "IsClipboardFormatAvailable")) return (FARPROC)fixture_available;
    if (!strcmp(symbol, "GetClipboardData")) return (FARPROC)fixture_get;
    if (!strcmp(symbol, "RegisterClipboardFormatW")) return (FARPROC)fixture_format;
    return NULL;
}
static BOOL WINAPI fixture_free(HMODULE library) { if (library == (HMODULE)(uintptr_t)7) { library_count--; return TRUE; } return FreeLibrary(library); }
#define LoadLibraryExW fixture_library
#define GetProcAddress fixture_symbol
#define FreeLibrary fixture_free
#define main terminal_probe_main
#include "terminal_screen_probe.c"
#undef main

static HGLOBAL fixture_bytes(const void* bytes, size_t size) {
    HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, size); assert(memory); void* dest = GlobalLock(memory); assert(dest); memcpy(dest, bytes, size); GlobalUnlock(memory); return memory;
}
static void fixture_dib(UINT header_size) {
    unsigned char bytes[124 + 16] = {0}; BITMAPINFOHEADER* header = (void*)bytes;
    header->biSize = header_size; header->biWidth = 2; header->biHeight = 2; header->biPlanes = 1; header->biBitCount = 32; header->biCompression = BI_RGB; header->biSizeImage = 16;
    for (int index = 0; index < 16; index++) bytes[header_size + index] = (unsigned char)(index * 17);
    image_memory = fixture_bytes(bytes, header_size + 16); image_format = header_size == 124 ? CF_DIBV5 : CF_DIB;
}
static void assert_png(const char* path) {
    FILE* file = fopen(path, "rb"); assert(file); unsigned char header[24]; assert(fread(header, 1, sizeof(header), file) == sizeof(header)); fclose(file);
    assert(!memcmp(header, "\x89PNG\r\n\x1a\n", 8)); assert(header[19] == 2 && header[23] == 2);
}
int main(int argc, char** argv) {
    assert(argc == 2); const char* path = argv[1]; size_t length = strlen(path);
    assert(sengoo_terminal_clipboard_has_image() == 0);
    assert(sengoo_terminal_clipboard_image_save((intptr_t)path, length, 65536) == 0);
    assert(GetFileAttributesA(path) == INVALID_FILE_ATTRIBUTES);
    fixture_dib(40); assert(sengoo_terminal_clipboard_has_image() == 1);
    assert(sengoo_terminal_clipboard_image_save((intptr_t)path, length, 65536) == 1); assert_png(path);
    assert(sengoo_terminal_clipboard_image_save((intptr_t)path, length, 65536) == -SENGOO_STATUS_ALREADY_EXISTS);
    GlobalFree(image_memory); image_memory = NULL;
    FILE* file = fopen(path, "rb"); assert(file); fseek(file, 0, SEEK_END); long size = ftell(file); rewind(file); unsigned char* png = malloc(size); assert(fread(png, 1, size, file) == (size_t)size); fclose(file);
    assert(DeleteFileA(path)); image_memory = fixture_bytes(png, size); image_format = 0xc001; free(png);
    assert(sengoo_terminal_clipboard_image_save((intptr_t)path, length, 65536) == 1); assert_png(path); assert(DeleteFileA(path));
    assert(sengoo_terminal_clipboard_image_save((intptr_t)path, length, 8) == -SENGOO_STATUS_OVERFLOW); assert(GetFileAttributesA(path) == INVALID_FILE_ATTRIBUTES);
    unsigned char* bad_png = GlobalLock(image_memory); unsigned char original_width[4]; memcpy(original_width, bad_png + 16, 4); memset(bad_png + 16, 255, 4); GlobalUnlock(image_memory);
    assert(sengoo_terminal_clipboard_image_save((intptr_t)path, length, 65536) == -SENGOO_STATUS_OVERFLOW);
    bad_png = GlobalLock(image_memory); memcpy(bad_png + 16, original_width, 4); GlobalUnlock(image_memory);
    GlobalFree(image_memory); image_memory = NULL;
    fixture_dib(124); assert(sengoo_terminal_clipboard_image_save((intptr_t)path, length, 65536) == 1); assert_png(path); assert(DeleteFileA(path));
    BITMAPINFOHEADER* dib = GlobalLock(image_memory); dib->biWidth = 100000; GlobalUnlock(image_memory);
    assert(sengoo_terminal_clipboard_image_save((intptr_t)path, length, 65536) == -SENGOO_STATUS_OVERFLOW); GlobalFree(image_memory); image_memory = NULL;
    HMODULE gdi32 = LoadLibraryExW(L"gdi32.dll", NULL, LOAD_LIBRARY_SEARCH_SYSTEM32); assert(gdi32);
    typedef HBITMAP (WINAPI *CreateFn)(int, int, UINT, UINT, const void*);
    typedef BOOL (WINAPI *DeleteFn)(HGDIOBJ);
    CreateFn create_bitmap = (void*)GetProcAddress(gdi32, "CreateBitmap"); DeleteFn delete_bitmap = (void*)GetProcAddress(gdi32, "DeleteObject"); assert(create_bitmap && delete_bitmap);
    const DWORD pixels[] = {0xff0000, 0x00ff00, 0x0000ff, 0xffffff}; image_memory = create_bitmap(2, 2, 1, 32, pixels); assert(image_memory); image_format = CF_BITMAP;
    assert(sengoo_terminal_clipboard_image_save((intptr_t)path, length, 65536) == 1); assert_png(path); assert(DeleteFileA(path));
    assert(delete_bitmap(image_memory)); image_memory = NULL; FreeLibrary(gdi32);
    int before = open_calls;
    assert(sengoo_terminal_clipboard_image_save((intptr_t)"x\0hidden", 8, 65536) == -SENGOO_STATUS_INVALID_ARGUMENT);
    const char invalid[] = {(char)0xc0, (char)0x80}; assert(sengoo_terminal_clipboard_image_save((intptr_t)invalid, 2, 65536) == -SENGOO_STATUS_INVALID_UTF8);
    assert(open_calls == before);
    const wchar_t wide[] = L"\x4e2d\x6587 \xd83d\xde00\nsecond line"; text_memory = fixture_bytes(wide, sizeof(wide));
    char* text = (void*)(intptr_t)sengoo_terminal_clipboard_text(); assert(text && !strcmp(text, "\xe4\xb8\xad\xe6\x96\x87 \xf0\x9f\x98\x80\nsecond line")); free(text); GlobalFree(text_memory); text_memory = NULL;
    text = (void*)(intptr_t)sengoo_terminal_clipboard_files(); assert(text && !strcmp(text, "[]")); free(text);
    const wchar_t paths[] = L"C:\\original\\one.png\0D:\\\x56fe\x7247\\two.jpg\0\0";
    size_t drop_size = sizeof(SgDropFiles) + sizeof(paths); files_memory = GlobalAlloc(GMEM_MOVEABLE | GMEM_ZEROINIT, drop_size); SgDropFiles* drop = GlobalLock(files_memory); drop->pFiles = sizeof(SgDropFiles); drop->fWide = TRUE; memcpy((char*)drop + sizeof(SgDropFiles), paths, sizeof(paths)); GlobalUnlock(files_memory);
    text = (void*)(intptr_t)sengoo_terminal_clipboard_files(); assert(text && !strcmp(text, "[\"C:\\\\original\\\\one.png\",\"D:\\\\\xe5\x9b\xbe\xe7\x89\x87\\\\two.jpg\"]")); free(text); GlobalFree(files_memory); files_memory = NULL;
    failure = 1; assert(sengoo_terminal_clipboard_text() == -SENGOO_STATUS_IO); failure = 0;
    fixture_dib(40); DWORD handles_before = 0, handles_after = 0; assert(GetProcessHandleCount(GetCurrentProcess(), &handles_before));
    for (int iteration = 0; iteration < 20; iteration++) { assert(sengoo_terminal_clipboard_image_save((intptr_t)path, length, 65536) == 1); assert(DeleteFileA(path)); assert(!opened && !library_count); }
    assert(GetProcessHandleCount(GetCurrentProcess(), &handles_after) && handles_after <= handles_before + 1); GlobalFree(image_memory); image_memory = NULL;
    assert(!opened && !library_count && GetFileAttributesA(path) == INVALID_FILE_ATTRIBUTES);
    puts("clipboard image: isolated clipboard state, real GDI+ DIB/DIBV5/PNG conversion, exclusive PNG write, byte/pixel/path limits, Unicode text/files and cleanup passed");
    return 0;
}
