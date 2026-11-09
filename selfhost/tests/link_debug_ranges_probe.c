#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>

int wmain(int argc, wchar_t **argv) {
    if (argc != 2) return 64;
    HMODULE module = LoadLibraryExW(argv[1], NULL, DONT_RESOLVE_DLL_REFERENCES);
    if (!module) return 65;
    uint8_t *image = (uint8_t *)module;
    IMAGE_DOS_HEADER *dos = (IMAGE_DOS_HEADER *)image;
    IMAGE_NT_HEADERS64 *nt = (IMAGE_NT_HEADERS64 *)(image + dos->e_lfanew);
    IMAGE_DATA_DIRECTORY directory = nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION];
    if (directory.Size < sizeof(RUNTIME_FUNCTION)) return 66;
    PRUNTIME_FUNCTION functions = (PRUNTIME_FUNCTION)(image + directory.VirtualAddress);
    PRUNTIME_FUNCTION function = functions;
    if (function->EndAddress - function->BeginAddress != 61 || image[function->BeginAddress] != 0x55) return 67;
    const DWORD64 base = (DWORD64)(uintptr_t)image;
    DWORD64 found_base = 0;
    BOOL registered = FALSE;
    PRUNTIME_FUNCTION found = RtlLookupFunctionEntry(base + function->BeginAddress + 30, &found_base, NULL);
    if (!found) {
        if (!RtlAddFunctionTable(functions, directory.Size / sizeof(RUNTIME_FUNCTION), base)) return 68;
        registered = TRUE;
        found = RtlLookupFunctionEntry(base + function->BeginAddress + 30, &found_base, NULL);
    }
    if (!found || found_base != base || found->BeginAddress != function->BeginAddress) return 69;
    uintptr_t stack[256] = {0};
    uintptr_t *entry = stack + 129;
    const DWORD64 returned = 0x12345000;
    const DWORD64 saved_rbp = 0xabcdeffedcba1234ULL;
    entry[0] = returned;
    entry[-1] = saved_rbp;
    const DWORD64 offsets[] = {0, 1, 4, 11, 30, 39, 42, 43, 56, 59, 60};
    for (unsigned index = 0; index < sizeof(offsets) / sizeof(offsets[0]); ++index) {
        DWORD64 offset = offsets[index];
        CONTEXT context = {0};
        context.ContextFlags = CONTEXT_FULL;
        context.Rip = base + function->BeginAddress + offset;
        context.Rsp = (DWORD64)(uintptr_t)entry;
        context.Rbp = saved_rbp;
        if (offset >= 1) context.Rsp -= 8;
        if (offset >= 4) context.Rbp = (DWORD64)(uintptr_t)(entry - 1);
        if (offset >= 11) context.Rsp -= 48;
        if (offset == 42 || offset == 59) context.Rsp = (DWORD64)(uintptr_t)(entry - 1);
        if (offset == 43 || offset == 60) {
            context.Rsp = (DWORD64)(uintptr_t)entry;
            context.Rbp = saved_rbp;
        }
        PVOID handler = NULL;
        DWORD64 establisher = 0;
        RtlVirtualUnwind(UNW_FLAG_NHANDLER, base, context.Rip, function, &context, &handler, &establisher, NULL);
        if (context.Rip != returned || context.Rsp != (DWORD64)(uintptr_t)(entry + 1) || context.Rbp != saved_rbp) return 70 + (int)index;
    }
    if (registered) RtlDeleteFunctionTable(functions);
    FreeLibrary(module);
    puts("SG RBP frame and both epilogues Windows runtime unwind: ok");
    return 0;
}
