#ifndef SENGOO_COLLECTION_OS_PROBE_H
#define SENGOO_COLLECTION_OS_PROBE_H
#include <stdio.h>
#include <windows.h>
static int sh_collection_os_modules(void) {
    const wchar_t *names[] = {L"kernel32.dll", L"KernelBase.dll", L"ntdll.dll"};
    const char *labels[] = {"kernel32.dll", "KernelBase.dll", "ntdll.dll"};
    for (unsigned index = 0; index < 3; ++index) {
        wchar_t path[32768];
        char utf8[131072];
        HMODULE module = GetModuleHandleW(names[index]);
        DWORD length = module ? GetModuleFileNameW(module, path, 32768) : 0;
        if (!length || length >= 32768) return 86;
        int bytes = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, path,
                                       (int)length, utf8, sizeof utf8 - 1, NULL, NULL);
        if (bytes <= 0) return 87;
        utf8[bytes] = 0;
        printf("OS_MODULE|%s|%s\n", labels[index], utf8);
    }
    return 42;
}
#endif
