#ifdef SENGOO_PUBLIC_WRAPPER_PROBE
#include "runtime_shared.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
extern long long sengoo_ffi_buffer_new(long long);
extern long long sengoo_ffi_buffer_len(long long);
extern long long sengoo_ffi_last_error_code(void);
extern long long sengoo_ffi_last_error_len(void);
static long long handle,created,verified;
int64_t sg_acquire(void) {
    if(sengoo_buffer_live_handle_count()||sengoo_string_live_handle_count()||sengoo_opaque_live_handle_count())exit(61);
    handle=sengoo_ffi_buffer_new(64);
    SengooFfiBuffer *buffer=sengoo_ffi_buffer_from_handle(handle);
    if(!buffer)exit(60);
    memset(buffer->bytes,0xa5,buffer->capacity);
    buffer->used_len=50;
    ++created;
    return handle;
}
int64_t sg_prepare_error(void) {
    if(sengoo_ffi_buffer_len(-1)!=-2002)exit(59);
    return sengoo_ffi_last_error_len();
}
int64_t sg_validate_copy(int64_t copied) {
    SengooFfiBuffer *buffer=sengoo_ffi_buffer_from_handle(handle);
    if(!buffer||sengoo_buffer_live_handle_count()!=1||copied!=23||buffer->used_len!=23)exit(58);
    if(sengoo_ffi_last_error_code()!=-2002||sengoo_ffi_last_error_len()!=23||memcmp(buffer->bytes,"buffer handle not found",23))exit(57);
    for(size_t i=23;i<buffer->capacity;++i)if(buffer->bytes[i]!=0xa5)exit(56);
    ++verified;
    return 42;
}
int64_t sg_resources(void) {
    if(created!=verified)exit(55);
    return sengoo_buffer_live_handle_count()+sengoo_string_live_handle_count()+sengoo_opaque_live_handle_count();
}

#else
#include "runtime_shared.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
extern long long sengoo_ffi_buffer_new(long long);
extern long long sengoo_ffi_buffer_free(long long);
extern long long sengoo_ffi_buffer_len(long long);
extern long long sengoo_ffi_buffer_ptr(long long);
extern long long sengoo_ffi_last_error_code(void);
extern long long sengoo_ffi_last_error_len(void);
extern long long sengoo_ffi_last_error_copy(long long,long long);
extern long long sengoo_ffi_last_error_copy_to_buffer(long long);
extern long long sengoo_ffi_last_error_clear(void);
static const char expected_error[]="buffer handle not found";
#define ERROR_BYTES (sizeof(expected_error)-1)
static long long copy_error(long long handle) {
#ifdef SENGOO_OLD_WRAPPER_CONTROL
    return sengoo_ffi_last_error_copy(sengoo_ffi_buffer_ptr(handle),
                                     sengoo_ffi_buffer_len(handle));
#else
    return sengoo_ffi_last_error_copy_to_buffer(handle);
#endif
}
static void check(int value, int code) { if (!value) exit(code); }
static void clear_resources(long long handle) {
    check(sengoo_ffi_buffer_free(handle)==0,91);
    check(!sengoo_buffer_live_handle_count()&&!sengoo_string_live_handle_count()&&
          !sengoo_opaque_live_handle_count(),92);
}
int main(int argc,char **argv) {
    if(argc!=2)return 99;
    int which=atoi(argv[1]);
    for(int round=0;round<100;++round) {
        check(!sengoo_buffer_live_handle_count(),93);
        long long handle=sengoo_ffi_buffer_new(which==2?8:64);
        check(handle>0,94);
        SengooFfiBuffer *buffer=sengoo_ffi_buffer_from_handle(handle);
        check(buffer!=NULL,95);
        memset(buffer->bytes,0xa5,buffer->capacity);
        buffer->used_len=which==2?7:50;
        if(which==0) {
            check(sengoo_ffi_buffer_len(-1)==-2002,70);
            long long before_code=sengoo_ffi_last_error_code(),before_len=sengoo_ffi_last_error_len();
            long long copied=copy_error(handle);
            if(copied!=(long long)ERROR_BYTES) { fprintf(stderr,"managed-copy-lost-diagnostic before=%lld/%lld after=%lld/%lld copied=%lld\n",before_code,before_len,sengoo_ffi_last_error_code(),sengoo_ffi_last_error_len(),copied);return 73; }
            check(sengoo_ffi_last_error_code()==-2002,74);
            check(sengoo_ffi_last_error_len()==(long long)ERROR_BYTES,75);
            check(buffer->used_len==ERROR_BYTES,76);
            check(memcmp(buffer->bytes,expected_error,ERROR_BYTES)==0,77);
            for(size_t i=ERROR_BYTES;i<buffer->capacity;++i)check(buffer->bytes[i]==0xa5,78);
        } else if(which==1) {
            check(sengoo_ffi_last_error_clear()==0,79);
            check(copy_error(handle)==0,80);
            check(sengoo_ffi_last_error_code()==0&&sengoo_ffi_last_error_len()==0,81);
            check(buffer->used_len==0,82);
            for(size_t i=0;i<buffer->capacity;++i)check(buffer->bytes[i]==0xa5,83);
        } else if(which==2) {
            check(sengoo_ffi_buffer_len(-1)==-2002,84);
            check(copy_error(handle)==-2006,85);
            check(sengoo_ffi_last_error_code()==-2006,86);
            check(buffer->used_len==7,87);
            for(size_t i=0;i<buffer->capacity;++i)check(buffer->bytes[i]==0xa5,88);
        } else if(which==3) {
            check(copy_error(0)==-2002,89);
            check(sengoo_ffi_last_error_code()==-2002,90);
            clear_resources(handle);
            check(sengoo_ffi_buffer_new(-1)==-2001,96);
            check(copy_error(handle)==-2002,97);
            check(sengoo_ffi_last_error_code()==-2002,98);
            continue;
        } else return 99;
        clear_resources(handle);
    }
    return 0;
}

#endif
