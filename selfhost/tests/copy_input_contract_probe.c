#include "runtime_shared.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
extern long long sengoo_string_from_bytes_copy(long long,long long);
extern long long sengoo_string_slice_status(long long,long long,long long);
extern long long sengoo_string_as_str_ptr(long long);
extern long long sengoo_string_free_status(long long);
extern long long sengoo_string_clear_status(long long);
extern long long sengoo_ffi_buffer_new(long long);
extern long long sengoo_ffi_buffer_free(long long);
extern long long sengoo_status_name_copy(long long,long long,long long);
extern long long sengoo_status_message_copy(long long,long long,long long);
static void check(int good,int error){if(!good)exit(error);}
int main(void) {
    static const unsigned char text[]={ 'h','e','l','l','o',0xe4,0xb8,0x96,0xe7,0x95,0x8c,0 };
    for(int round=0;round<100;++round) {
        check(!sengoo_string_live_handle_count()&&!sengoo_buffer_live_handle_count()&&!sengoo_opaque_live_handle_count(),71);
        long long owner=sengoo_string_from_bytes_copy((long long)(intptr_t)text,11);
        check(owner>0&&sengoo_string_live_handle_count()==1,72);
        long long left=sengoo_string_slice_status(owner,0,5);
        long long right=sengoo_string_slice_status(owner,5,11);
        check(left>0&&right>0&&left!=right&&left!=owner&&right!=owner&&sengoo_string_live_handle_count()==3,73);
        check(!memcmp((void*)(intptr_t)sengoo_string_as_str_ptr(owner),text,12),74);
        check(!memcmp((void*)(intptr_t)sengoo_string_as_str_ptr(left),"hello",6),75);
        check(!memcmp((void*)(intptr_t)sengoo_string_as_str_ptr(right),text+5,7),76);
        check(sengoo_string_slice_status(owner,6,11)==-2,77);
        check(sengoo_string_live_handle_count()==3,78);
        check(sengoo_string_clear_status(owner)==0,89);
        check(!memcmp((void*)(intptr_t)sengoo_string_as_str_ptr(left),"hello",6)&&!memcmp((void*)(intptr_t)sengoo_string_as_str_ptr(right),text+5,7),90);
        check(sengoo_string_free_status(owner)==0&&sengoo_string_live_handle_count()==2,79);
        check(!memcmp((void*)(intptr_t)sengoo_string_as_str_ptr(left),"hello",6)&&!memcmp((void*)(intptr_t)sengoo_string_as_str_ptr(right),text+5,7),80);
        check(!sengoo_string_free_status(left)&&!sengoo_string_free_status(right),81);
        long long output=sengoo_ffi_buffer_new(128);
        SengooFfiBuffer *buffer=sengoo_ffi_buffer_from_handle(output);
        check(buffer&&sengoo_buffer_live_handle_count()==1,82);
        memset(buffer->bytes,0xa5,buffer->capacity);
        long long len=sengoo_status_name_copy(3,(long long)(intptr_t)buffer->bytes,(long long)buffer->capacity);
        check(len==14&&!memcmp(buffer->bytes,"invalid_handle",14)&&buffer->bytes[14]==0xa5,83);
        memset(buffer->bytes,0xa5,buffer->capacity);
        len=sengoo_status_message_copy(3,(long long)(intptr_t)buffer->bytes,(long long)buffer->capacity);
        check(len==14&&!memcmp(buffer->bytes,"invalid handle",14)&&buffer->bytes[14]==0xa5&&sengoo_buffer_live_handle_count()==1,84);
        unsigned char saved[128];memcpy(saved,buffer->bytes,128);
        check(sengoo_status_message_copy(3,(long long)(intptr_t)buffer->bytes,0)==-2006,85);
        check(!memcmp(saved,buffer->bytes,128),86);
        check(sengoo_ffi_buffer_free(output)==0,87);
        check(!sengoo_string_live_handle_count()&&!sengoo_buffer_live_handle_count()&&!sengoo_opaque_live_handle_count(),88);
    }
    return 0;
}
