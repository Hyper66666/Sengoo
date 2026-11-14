#define WIN32_LEAN_AND_MEAN
#include <windows.h>

/* Test observation only. No factory state or authorization is kept here. */
long long sengoo_async_factory_test_handle_count(void) {
    DWORD count = 0;
    return GetProcessHandleCount(GetCurrentProcess(), &count) ? count : -1;
}
