#include <stdint.h>
__declspec(dllimport) int64_t sengoo_ps_component_equal(const uint8_t *,int64_t,const uint8_t *,int64_t);
__declspec(noinline) int64_t package_unwind_caller(void){
    const uint8_t upper[1]={65},lower[1]={97};
    volatile int64_t value=sengoo_ps_component_equal(upper,1,lower,1);
    return value+41;
}
int main(void){return (int)package_unwind_caller();}
