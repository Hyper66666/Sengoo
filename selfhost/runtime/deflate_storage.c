#include <stdint.h>
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

/* Raw memory acquisition only. SG chooses the bounded size and owns release. */
uint8_t *sengoo_ds_storage_alloc(int64_t length) {
    if(length<=0 || (uint64_t)length>SIZE_MAX)return NULL;
    return VirtualAlloc(NULL,(SIZE_T)length,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);
}
int64_t sengoo_ds_storage_present(uint8_t *state) { return state!=NULL; }
void sengoo_ds_storage_free(uint8_t *state) {
    if(state)VirtualFree(state,0,MEM_RELEASE);
}
