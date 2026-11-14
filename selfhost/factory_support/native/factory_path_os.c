#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>

/* Case comparison only, with the same ordinal Windows rules used for paths.
 * Source identity, selection policy, and all retained handles stay in SG. */
int64_t sengoo_factory_path_equal(int64_t left, int64_t left_bytes,
                                int64_t right, int64_t right_bytes) {
    if (!left || !right || left_bytes < 1 || right_bytes < 1 ||
        left_bytes >= 32768 || right_bytes >= 32768) return -2;
    const char *a = (const char *)(intptr_t)left;
    const char *b = (const char *)(intptr_t)right;
    for (int64_t i = 0; i < left_bytes; ++i) if (!a[i]) return -2;
    for (int64_t i = 0; i < right_bytes; ++i) if (!b[i]) return -2;
    HANDLE heap = GetProcessHeap();
    wchar_t *storage = HeapAlloc(heap, 0, 65536 * sizeof(wchar_t));
    if (!storage) return -14;
    int na = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, a,
                               (int)left_bytes, storage, 32768);
    int nb = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, b,
                               (int)right_bytes, storage + 32768, 32768);
    int result = na && nb ? CompareStringOrdinal(storage, na, storage + 32768,
                                                 nb, TRUE) : 0;
    HeapFree(heap, 0, storage);
    if (!na || !nb) return -20;
    if (!result) return -2;
    return result == CSTR_EQUAL ? 1 : 0;
}
