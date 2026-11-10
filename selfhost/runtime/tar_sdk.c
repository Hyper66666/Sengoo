/* Development DLL boundary: preserve the compiler's real unwind metadata. */
#define sengoo_tar_read sh_tar_provider_read
#define sengoo_tar_output_read sh_tar_provider_output_read
#define sengoo_tar_write sh_tar_provider_write
#define sengoo_tar_windows_scalar_equal sh_tar_provider_windows_scalar_equal
#include "tar_memory.c"
#undef sengoo_tar_read
#undef sengoo_tar_output_read
#undef sengoo_tar_write
#undef sengoo_tar_windows_scalar_equal

__declspec(dllexport) uint8_t sengoo_tar_read(const uint8_t *bytes, int64_t index) {
    return sh_tar_provider_read(bytes,index);
}
__declspec(dllexport) uint8_t sengoo_tar_output_read(uint8_t *bytes, int64_t index) {
    return sh_tar_provider_output_read(bytes,index);
}
__declspec(dllexport) void sengoo_tar_write(uint8_t *bytes, int64_t index, uint8_t value) {
    sh_tar_provider_write(bytes,index,value);
}
__declspec(dllexport) int64_t sengoo_tar_windows_scalar_equal(int64_t left, int64_t right) {
    return sh_tar_provider_windows_scalar_equal(left,right);
}
