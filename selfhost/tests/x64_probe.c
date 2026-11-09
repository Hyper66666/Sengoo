/* Independent CPU execution of a known SG-encoded byte sequence. Test only. */
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <string.h>

int main(int argc, char **argv) {
    static const unsigned char expected[] = {
        0x48, 0xb8, 40, 0, 0, 0, 0, 0, 0, 0,
        0x48, 0xb9, 2, 0, 0, 0, 0, 0, 0, 0,
        0x48, 0x01, 0xc8, 0xc3
    };
    if (argc != 2) return 64;
    FILE *file = fopen(argv[1], "rb");
    if (!file) return 65;
    unsigned char bytes[sizeof(expected) + 1];
    size_t count = fread(bytes, 1, sizeof(bytes), file);
    int failed = ferror(file);
    fclose(file);
    if (failed || count != sizeof(expected) || memcmp(bytes, expected, sizeof(expected))) return 66;
    void *code = VirtualAlloc(NULL, sizeof(expected), MEM_COMMIT | MEM_RESERVE, PAGE_READWRITE);
    if (!code) return 67;
    memcpy(code, bytes, count);
    DWORD old;
    if (!VirtualProtect(code, count, PAGE_EXECUTE_READ, &old)) { VirtualFree(code, 0, MEM_RELEASE); return 68; }
    if (!FlushInstructionCache(GetCurrentProcess(), code, count)) { VirtualFree(code, 0, MEM_RELEASE); return 69; }
    __int64 result = ((__int64 (*)(void))code)();
    if (!VirtualFree(code, 0, MEM_RELEASE)) return 70;
    if (result != 42) return 71;
    puts("SG x64 bytes: exact encoding and native execution returned 42");
    return 0;
}
