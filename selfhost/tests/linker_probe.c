#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static int check_unwind(uint8_t *image, DWORD64 base, PRUNTIME_FUNCTION function) {
    uintptr_t stack[128] = {0};
    uintptr_t *original = stack + 100;
    const DWORD64 returned = 0x12345000, saved_rbp = 0xabcdeffedcba1234ULL;
    original[0] = (uintptr_t)returned;
    const DWORD64 length = function->EndAddress - function->BeginAddress;
    const DWORD64 offsets[] = {0, 4, length - 5, length - 1};
    const uint8_t *code = image + function->BeginAddress;
    if (code[0] != 0x48 || code[1] != 0x83 || code[2] != 0xec || code[3] != 40) return 1;
    for (unsigned i = 0; i < sizeof(offsets) / sizeof(offsets[0]); ++i) {
        CONTEXT context = {0};
        context.ContextFlags = CONTEXT_FULL;
        context.Rip = base + function->BeginAddress + offsets[i];
        context.Rsp = (DWORD64)(uintptr_t)original;
        if (i == 1 || i == 2) context.Rsp -= 40;
        context.Rbp = saved_rbp;
        PVOID handler = NULL;
        DWORD64 establisher = 0;
        RtlVirtualUnwind(UNW_FLAG_NHANDLER, base, context.Rip, function, &context,
                         &handler, &establisher, NULL);
        if (context.Rip != returned || context.Rsp != (DWORD64)(uintptr_t)(original + 1) || context.Rbp != saved_rbp) return 2 + (int)i;
    }
    return 0;
}

int wmain(int argc, wchar_t **argv) {
    if (argc != 2) return 64;
    FILE *file = NULL;
    if (_wfopen_s(&file, argv[1], L"rb") || !file) return 65;
    uint8_t header[512];
    size_t count = fread(header, 1, sizeof(header), file);
    fclose(file);
    if (count < 328) return 66;
    IMAGE_DOS_HEADER *dos = (IMAGE_DOS_HEADER *)header;
    if (dos->e_magic != IMAGE_DOS_SIGNATURE || dos->e_lfanew != 64) return 67;
    IMAGE_NT_HEADERS64 *disk = (IMAGE_NT_HEADERS64 *)(header + dos->e_lfanew);
    if (disk->Signature != IMAGE_NT_SIGNATURE || disk->OptionalHeader.Magic != IMAGE_NT_OPTIONAL_HDR64_MAGIC) return 68;
    DWORD64 preferred = disk->OptionalHeader.ImageBase;
    // Reserve the preferred address before mapping. A successful image load
    // therefore needs real DIR64 relocation handling rather than lucky layout.
    void *reservation = VirtualAlloc((void *)(uintptr_t)preferred,
        disk->OptionalHeader.SizeOfImage, MEM_RESERVE, PAGE_NOACCESS);
    HMODULE module = LoadLibraryExW(argv[1], NULL, DONT_RESOLVE_DLL_REFERENCES);
    if (!module) return 69;
    uint8_t *image = (uint8_t *)module;
    DWORD64 base = (DWORD64)(uintptr_t)image;
    if (base == preferred) return 70;
    IMAGE_NT_HEADERS64 *nt = (IMAGE_NT_HEADERS64 *)(image + dos->e_lfanew);
    IMAGE_SECTION_HEADER *sections = IMAGE_FIRST_SECTION(nt);
    DWORD text = 0, data = 0;
    for (WORD i = 0; i < nt->FileHeader.NumberOfSections; ++i) {
        if (memcmp(sections[i].Name, ".text\0\0\0", 8) == 0) text = sections[i].VirtualAddress;
        if (memcmp(sections[i].Name, ".rdata\0\0", 8) == 0) data = sections[i].VirtualAddress;
    }
    if (!text || !data) return 71;
    uint64_t code_pointer = 0, data_pointer = 0;
    memcpy(&code_pointer, image + text + 11, 8);
    memcpy(&data_pointer, image + data + 16, 8);
    if (code_pointer != base + data + 16 || data_pointer != base + data + 8 ||
        *(uint64_t *)(image + data + 8) != 2) return 72;
    IMAGE_DATA_DIRECTORY exception = nt->OptionalHeader.DataDirectory[IMAGE_DIRECTORY_ENTRY_EXCEPTION];
    if (exception.Size != 24) return 73;
    PRUNTIME_FUNCTION functions = (PRUNTIME_FUNCTION)(image + exception.VirtualAddress);
    if (functions[0].BeginAddress != text || check_unwind(image, base, &functions[0])) return 74;
    DWORD64 lookup_base = 0;
    PRUNTIME_FUNCTION found = RtlLookupFunctionEntry(base + text + 4, &lookup_base, NULL);
    BOOL registered = FALSE;
    if (!found) {
        if (!RtlAddFunctionTable(functions, exception.Size / sizeof(RUNTIME_FUNCTION), base)) return 75;
        registered = TRUE;
        found = RtlLookupFunctionEntry(base + text + 4, &lookup_base, NULL);
    }
    if (!found || lookup_base != base || found->BeginAddress != text) return 76;
    if (registered) RtlDeleteFunctionTable(functions);
    FreeLibrary(module);
    if (reservation) VirtualFree(reservation, 0, MEM_RELEASE);
    puts("SG-linked PE forced rebasing and Windows unwind: ok");
    return 0;
}
