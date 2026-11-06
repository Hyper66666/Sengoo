/* Isolated synchronous boundary and allocation checks; no runtime stubs used. */
#define _CRT_SECURE_NO_WARNINGS
#include <assert.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
#include <windows.h>
#include "../runtime_shared.h"
extern long long sengoo_dir_create_all(long long);
extern long long sengoo_file_write_str(long long,long long,long long);
extern long long sengoo_file_read_into(long long,long long,long long);
extern long long sengoo_file_copy(long long,long long,long long);
extern long long sengoo_file_move(long long,long long,long long);
extern long long sengoo_file_len(long long);
extern long long sengoo_file_kind(long long);
extern long long sengoo_file_modified_unix_ms(long long);
extern long long sengoo_dir_entry_count(long long);
extern long long sengoo_dir_entry_name(long long,long long,long long,long long);
extern long long sengoo_file_remove(long long);
extern long long sengoo_dir_remove(long long);
static long long raw(const void* value){return (long long)(intptr_t)value;}
int main(void) {
    wchar_t temp[32768], folder[32768];
    assert(GetTempPathW(32000,temp));
    swprintf(folder,32768,L"%lssengoo-fs-\x9879\x76ee\xd83d\xde00-%lu",temp,GetCurrentProcessId());
    char utf8[131072],original[131072],temporary[131072],copy[131072];
    assert(WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,folder,-1,utf8,sizeof(utf8),NULL,NULL));
    snprintf(original,sizeof(original),"%s/数据🌏.txt",utf8);
    snprintf(temporary,sizeof(temporary),"%s/临时😀.txt",utf8);
    snprintf(copy,sizeof(copy),"%s/副本😀.txt",utf8);
    assert(sengoo_dir_create_all(raw(utf8))==0);
    const char* before="世界😀 before",*after="世界😀 after";
    unsigned char buffer[256];
    for(int iteration=0;iteration<1000;iteration++) {
        assert(sengoo_file_write_str(raw(original),raw(before),strlen(before))==(long long)strlen(before));
        assert(sengoo_file_write_str(raw(temporary),raw(after),strlen(after))==(long long)strlen(after));
        assert(sengoo_file_move(raw(temporary),raw(original),1)==0);
        assert(sengoo_file_read_into(raw(original),raw(buffer),sizeof(buffer))==(long long)strlen(after));
        assert(!memcmp(buffer,after,strlen(after)));
        assert(sengoo_file_copy(raw(original),raw(original),1)==-2);
        assert(sengoo_file_copy(raw(original),raw(copy),0)==(long long)strlen(after));
        assert(sengoo_file_copy(raw(original),raw(copy),0)==-6);
        assert(sengoo_file_move(raw(copy),raw(original),0)==-6);
        assert(sengoo_file_kind(raw(copy))==1);
        assert(sengoo_file_len(raw(copy))==(long long)strlen(after));
        assert(sengoo_file_modified_unix_ms(raw(copy))>0);
        assert(sengoo_dir_entry_count(raw(utf8))==2);
        long long count=sengoo_dir_entry_name(raw(utf8),0,raw(buffer),sizeof(buffer)-1);
        assert(count>0);buffer[count]=0;
        assert(strstr((const char*)buffer,"😀"));
        assert(sengoo_file_remove(raw(copy))==0);
        assert(sengoo_file_remove(raw(original))==0);
    }
    const char bad[]={ (char)0xff, 0 };
    assert(sengoo_file_write_str(raw(bad),raw("x"),1)==-20);
    assert(sengoo_file_len(raw(bad))==-20);
    assert(sengoo_file_kind(raw(bad))==-20);
    assert(sengoo_dir_entry_count(raw(bad))==-20);
    assert(sengoo_dir_create_all(raw(bad))==-20);
    assert(sengoo_dir_remove(raw(utf8))==0);
    puts("Unicode filesystem native: 1000 read/atomic-replace/copy/list/remove cycles passed");
    return 0;
}
