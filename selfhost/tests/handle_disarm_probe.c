#include <stdint.h>
#include <stdlib.h>
extern long long sengoo_ffi_buffer_new(long long);
extern long long sengoo_actual_ffi_buffer_free(long long);
extern long long sengoo_buffer_live_handle_count(void);
extern long long sengoo_string_live_handle_count(void);
extern long long sengoo_opaque_live_handle_count(void);
static int64_t created,freed,source_drops;
int64_t sg_acquire(void){if(sengoo_buffer_live_handle_count())exit(71);long long h=sengoo_ffi_buffer_new(16);if(h<=0)exit(72);++created;return h;}
long long sengoo_ffi_buffer_free(long long handle){++source_drops;return sengoo_actual_ffi_buffer_free(handle);}
void sg_destination_drop(int64_t handle){if(sengoo_buffer_live_handle_count()!=1||sengoo_actual_ffi_buffer_free(handle))exit(74);++freed;}
int64_t sg_counts(int64_t round){if(created!=round||freed!=round||source_drops||sengoo_buffer_live_handle_count()||sengoo_string_live_handle_count()||sengoo_opaque_live_handle_count())exit(75);return 42;}
