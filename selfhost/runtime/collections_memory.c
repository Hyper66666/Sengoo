#include <stdint.h>
#include <stddef.h>
#include <string.h>
#define WIN32_LEAN_AND_MEAN
#include <windows.h>

/* Primitive memory and C callback ABI only. No slot/handle/container policy. */
#ifndef SENGOO_COLLECTIONS_TEST_METADATA
int64_t sengoo_collections_word(uint8_t *state,int64_t index){int64_t out;memcpy(&out,state+index*8,8);return out;}
void sengoo_collections_put(uint8_t *state,int64_t index,int64_t value){memcpy(state+index*8,&value,8);}
uint8_t *sengoo_collections_pointer(uint8_t *state,int64_t index){uint8_t *out;memcpy(&out,state+index*8,sizeof(out));return out;}
void sengoo_collections_pointer_put(uint8_t *state,int64_t index,uint8_t *value){memcpy(state+index*8,&value,sizeof(value));}
#endif
uint8_t *sengoo_collections_at(uint8_t *bytes,int64_t offset){return offset ? bytes+offset : bytes;}
uint8_t *sengoo_collections_null(void){return NULL;}
int64_t sengoo_collections_present(uint8_t *bytes){return bytes!=NULL;}
int64_t sengoo_collections_equal(uint8_t *left,uint8_t *right){return left==right;}
#ifndef SENGOO_COLLECTIONS_TEST_ALLOCATOR
uint8_t *sengoo_collections_allocate(int64_t bytes,int64_t align){(void)align;return VirtualAlloc(NULL,(SIZE_T)bytes,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);}
void sengoo_collections_release(uint8_t *bytes){if(bytes)VirtualFree(bytes,0,MEM_RELEASE);}
#endif
void sengoo_collections_move(int64_t callback,uint8_t *target,uint8_t *source){((void(*)(void*,void*))(intptr_t)callback)(target,source);}
void sengoo_collections_drop(int64_t callback,uint8_t *value){((void(*)(void*))(intptr_t)callback)(value);}
