#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <stdlib.h>
static void *live;
static SIZE_T live_bytes;
static unsigned allocations;
static void *tracked_alloc(HANDLE heap,DWORD flags,SIZE_T bytes){
    if(live) abort();
    void *p=HeapAlloc(heap,flags,bytes);
    if(p){live=p;live_bytes=bytes;++allocations;}
    return p;
}
static BOOL tracked_free(HANDLE heap,DWORD flags,void *p){
    if(!p||p!=live||!live_bytes||allocations!=1)abort();
    BOOL result=HeapFree(heap,flags,p);
    if(result){live=NULL;live_bytes=0;--allocations;}
    return result;
}
#define HeapAlloc tracked_alloc
#define HeapFree tracked_free
#include "../factory_support/native/factory_path_os.c"
#undef HeapAlloc
#undef HeapFree
static DWORD handles(void){DWORD n=0;if(!GetProcessHandleCount(GetCurrentProcess(),&n))abort();return n;}
static void check(const char *a,int64_t na,const char *b,int64_t nb,int64_t expected){
    DWORD before=handles();
    int64_t actual=sengoo_factory_path_equal((int64_t)(intptr_t)a,na,(int64_t)(intptr_t)b,nb);
    if(actual!=expected||live||live_bytes||allocations||before!=handles()){
        fprintf(stderr,"path compare mismatch actual=%lld expected=%lld alloc=%u bytes=%zu\n",(long long)actual,(long long)expected,allocations,live_bytes);exit(1);
    }
}
int main(void){
    for(int round=0;round<1000;++round){
        check("D:\\Root\\file.sg",15,"d:\\root\\FILE.SG",15,1);
        check("D:\\\xc3\xa9\xd1\x8f\\file.sg",15,"d:\\\xc3\x89\xd0\xaf\\FILE.SG",15,1);
        check("D:\\Root\\file.sg",15,"D:\\Root\\evil.sg",15,0);
        check("D:\\Root\\file.sg",15,"D:\\Root\\file.sg.x",17,0);
        check("D:/Root/file.sg",15,"D:\\Root\\file.sg",15,0);
        check("D:\\Root\\file.sg",15,"D:\\Root\\.\\file.sg",17,0);
        check("D:\\\xc3\xa9\\f",7,"D:\\e\\f",6,0);
        check("D:\\\xc3\xa9\\f",7,"D:\\e\xcc\x81\\f",8,0);
        check("D:\\file.sg",10,"D:\\file.sg",10,1);
        check("a",1,"A",1,1);
        check("",0,"a",1,-2);
        check(NULL,1,"a",1,-2);
        check("a",1,"a",32768,-2);
        check("a\0b",3,"a",1,-2);
        check("\xc0\xaf",2,"a",1,-20);
        check("a",1,"\xff",1,-20);
    }
    puts("16 path cases, 16000 scopes, exact allocations and OS handles restored");
    return 0;
}
