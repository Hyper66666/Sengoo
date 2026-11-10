#include "runtime_shared.h"
#include <stdint.h>
#include <string.h>
extern long long sengoo_ffi_buffer_new(long long);
extern long long sengoo_ffi_buffer_free(long long);
extern long long sengoo_buffer_live_handle_count(void);
typedef struct {int64_t handle;int64_t value;} Owner;
extern void sg_drop_cache_only(void *);
extern void sg_drop_cache_empty(void *);
static int drops;
void cache_owner_release(Owner *p){if(!p->handle||sengoo_ffi_buffer_free(p->handle))__builtin_trap();++drops;p->handle=0;}
void cache_empty_release_unused(Owner *p){(void)p;__builtin_trap();}
int main(int argc,char **argv){(void)argv;long long baseline=sengoo_buffer_live_handle_count();if(argc>1){Owner item={sengoo_ffi_buffer_new(64),42};if(!item.handle)return 70;sg_drop_cache_empty(&item);int retained=item.handle&&drops==0&&sengoo_buffer_live_handle_count()==baseline+1;cache_owner_release(&item);return retained?78:71;}for(int i=0;i<1000;++i){Owner item={sengoo_ffi_buffer_new(64),42};if(!item.handle)return 72;sg_drop_cache_only(&item);if(item.handle||drops!=i+1||sengoo_buffer_live_handle_count()!=baseline)return 73;}return 42;}
