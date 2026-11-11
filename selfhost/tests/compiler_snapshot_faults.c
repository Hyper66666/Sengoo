/* Test-only adapter wrapper. The runner copies the exact production OS source
 * to compiler_snapshot_os_original.inc beside this file. Only read/metadata
 * responses are injected; SG admission and retained ownership stay unchanged. */
#define sengoo_compiler_snapshot_read snapshot_test_actual_read
#define sengoo_compiler_snapshot_metadata snapshot_test_actual_metadata
#include "compiler_snapshot_os_original.inc"
#undef sengoo_compiler_snapshot_read
#undef sengoo_compiler_snapshot_metadata

static int64_t snapshot_test_mode;
static int64_t snapshot_test_metadata_calls;

int64_t sengoo_compiler_snapshot_test_fault(int64_t mode) {
    snapshot_test_mode = mode;
    snapshot_test_metadata_calls = 0;
    return 0;
}

int64_t sengoo_compiler_snapshot_read(int64_t state, int64_t extent,
                                    int64_t output, int64_t capacity) {
    if (snapshot_test_mode == 1) return 0; /* EOF before the advertised size. */
    if (snapshot_test_mode == 2) return capacity + 1; /* No output write. */
    if (snapshot_test_mode == 4 && capacity > 3) capacity = 3;
    int64_t count = snapshot_test_actual_read(state, extent, output, capacity);
    if (snapshot_test_mode == 5 && count == 0) return 1; /* Missing final EOF. */
    return count;
}

int64_t sengoo_compiler_snapshot_metadata(int64_t state, int64_t extent,
                                        int64_t output, int64_t capacity) {
    int64_t status = snapshot_test_actual_metadata(state, extent, output, capacity);
    if (status == 0 && snapshot_test_mode == 3 && ++snapshot_test_metadata_calls == 2) {
        uint64_t words[10];
        ss_copy(words, (const void *)(intptr_t)output, sizeof(words));
        words[9] ^= 1; /* Change only the second observation's write-time word. */
        ss_copy((void *)(intptr_t)output, words, sizeof(words));
    }
    return status;
}
