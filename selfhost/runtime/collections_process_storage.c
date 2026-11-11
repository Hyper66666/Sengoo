#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>

/* Only process/DLL storage and OS synchronization. SG owns all slot policy. */
static __declspec(align(8)) uint8_t process_registry[4160];
static __declspec(align(8)) uint8_t raw_vec_context[12352];
static SRWLOCK process_lock = SRWLOCK_INIT;
__declspec(dllexport) uint8_t *sengoo_process_registry_address(void){return process_registry;}
__declspec(dllexport) uint8_t *sengoo_process_vec_context_address(void){return raw_vec_context;}
__declspec(dllexport) void sengoo_process_registry_lock(void){AcquireSRWLockExclusive(&process_lock);}
__declspec(dllexport) void sengoo_process_registry_unlock(void){ReleaseSRWLockExclusive(&process_lock);}
