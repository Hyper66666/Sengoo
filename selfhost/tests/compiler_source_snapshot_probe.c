#define WIN32_LEAN_AND_MEAN
#include <windows.h>

/* Qualification observation only; source ownership stays in SG. */
long long sengoo_compiler_snapshot_test_handle_count(void) {
    DWORD count = 0;
    return GetProcessHandleCount(GetCurrentProcess(), &count) ? count : -1;
}
