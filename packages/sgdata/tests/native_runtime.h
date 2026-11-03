/* Small ownership-aware host for unit-testing package shims independently of
 * the compiler. Sengoo-level tests separately exercise the actual runtime. */
#ifndef SENGOO_PACKAGE_TEST_RUNTIME_H
#define SENGOO_PACKAGE_TEST_RUNTIME_H
#ifndef _WIN32
#ifndef _GNU_SOURCE
#define _GNU_SOURCE 1
#endif
#endif
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
static long long test_pointer(const void *p) {return (long long)(intptr_t)p;}
long long sengoo_string_from_bytes_copy(long long bytes,long long length) {
    if(length<0)return -2;
    char *s=(char*)malloc((size_t)length+1);if(!s)return -14;
    if(length)memcpy(s,(const void*)(intptr_t)bytes,(size_t)length);s[length]=0;return test_pointer(s);
}
static void test_string(long long handle,const char *expected) {
    if(handle<=0)fprintf(stderr,"expected string [%s], received error %lld\n",expected,handle);
    assert(handle>0);assert(strcmp((const char*)(intptr_t)handle,expected)==0);free((void*)(intptr_t)handle);
}
typedef struct TestBytes {size_t size;char *data;} TestBytes;
long long sengoo_ffi_buffer_from_bytes(long long bytes,long long length) {
    TestBytes *b=(TestBytes*)calloc(1,sizeof(*b));assert(b);
    b->data=(char*)malloc(length?(size_t)length:1);assert(b->data);b->size=(size_t)length;
    if(length)memcpy(b->data,(const void*)(intptr_t)bytes,(size_t)length);return test_pointer(b);
}
static void test_buffer_free(long long handle) {TestBytes *b=(TestBytes*)(intptr_t)handle;free(b->data);free(b);}
long long sengoo_args_len(void) {return 0;}
long long sengoo_arg_len(long long index) {(void)index;return -5;}
long long sengoo_arg_copy(long long index,long long bytes,long long capacity) {(void)index;(void)bytes;(void)capacity;return -5;}
#endif
