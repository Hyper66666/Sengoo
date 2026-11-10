#include <stdint.h>
#include <windows.h>

#ifndef SENGOO_TAR_INSTRUMENTED
uint8_t sengoo_tar_read(const uint8_t *bytes, int64_t index) {
    return bytes[index];
}
uint8_t sengoo_tar_output_read(uint8_t *bytes, int64_t index) {
    return bytes[index];
}
void sengoo_tar_write(uint8_t *bytes, int64_t index, uint8_t value) {
    bytes[index] = value;
}
#endif
/* SG supplies one valid UTF-16 scalar in the low two words. No path policy. */
int64_t sengoo_tar_windows_scalar_equal(int64_t left, int64_t right) {
#ifdef SENGOO_TAR_INSTRUMENTED
    extern void sengoo_tar_observe_os_compare(void);
    sengoo_tar_observe_os_compare();
#endif
    WCHAR a[2] = {(WCHAR)left, (WCHAR)((uint64_t)left >> 16)};
    WCHAR b[2] = {(WCHAR)right, (WCHAR)((uint64_t)right >> 16)};
    return CompareStringOrdinal(a, a[1] ? 2 : 1, b, b[1] ? 2 : 1, TRUE) == CSTR_EQUAL;
}
