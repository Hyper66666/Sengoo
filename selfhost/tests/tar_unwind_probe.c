#include <stdint.h>
#include <windows.h>
__declspec(dllimport) int64_t sengoo_tar_windows_scalar_equal(int64_t,int64_t);
__declspec(noinline) int64_t tar_unwind_caller(void) {
    volatile int64_t result=sengoo_tar_windows_scalar_equal(65,97);
    return result+41;
}
int main(void) {return (int)tar_unwind_caller();}
