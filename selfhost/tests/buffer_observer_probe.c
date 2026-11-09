#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
extern long long sengoo_ffi_buffer_new(long long);
extern long long sengoo_ffi_buffer_free(long long);
extern long long sengoo_ffi_buffer_copy_in(long long,long long,long long);
extern long long sengoo_ffi_buffer_ptr(long long);
extern long long sengoo_ffi_buffer_len(long long);
extern long long sengoo_buffer_live_handle_count(void);
extern long long sengoo_string_live_handle_count(void);
extern long long sengoo_opaque_live_handle_count(void);
static long long created,freed,verified;
typedef struct{int64_t handle;} Buffer;
int64_t sg_buffer_acquire(void){if(sengoo_buffer_live_handle_count())exit(61);long long h=sengoo_ffi_buffer_new(128);const unsigned char data[]={1,41};if(h<=0||sengoo_ffi_buffer_copy_in(h,(long long)(intptr_t)data,2))exit(60);sengoo_ffi_buffer_len(-1);++created;return h;}
void sg_buffer_release(int64_t h){if(sengoo_buffer_live_handle_count()!=1||verified!=created)exit(59);if(sengoo_ffi_buffer_free(h)||sengoo_buffer_live_handle_count()||sengoo_string_live_handle_count()||sengoo_opaque_live_handle_count())exit(58);++freed;}
int64_t sg_buffer_live(void){long long live=sengoo_buffer_live_handle_count();if(live)fprintf(stderr,"reference-resource-gap live=%lld created=%lld freed=%lld verified=%lld\n",live,created,freed,verified);return live;}
int64_t sg_buffer_frees(void){return freed;}
int64_t sg_check_owned(int64_t value,int64_t raw,int64_t first,int64_t second){if(sengoo_buffer_live_handle_count()!=1||raw!=sengoo_ffi_buffer_ptr(value)||first!=1||second!=41)exit(57);++verified;return 42;}
int64_t sg_check_copy(int64_t value,int64_t kind,int64_t first,int64_t second){static const char* expected[]={"17.0.0","Unicode 17.0.0; UTF-8 well-formedness + scalar decode only (no property tables)","buffer handle not found"};if(kind<0||kind>2||sengoo_buffer_live_handle_count()!=1)exit(56);size_t length=strlen(expected[kind]);if(first!=(int64_t)length||second!=first||memcmp((void*)(intptr_t)sengoo_ffi_buffer_ptr(value),expected[kind],length))exit(55);++verified;return 42;}

int64_t sg_prepare_error(int64_t capacity){sengoo_ffi_buffer_len(-1);return capacity;}
