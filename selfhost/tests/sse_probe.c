#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdio.h>
#include <stdint.h>
#include <string.h>
typedef double (*binary64)(double,double);
typedef float (*binary32)(float,float);
typedef double (*load64)(const double*,double);
typedef void (*store64)(double*,double);
typedef void (*store32)(float*,float);
typedef double (*frombits)(uint64_t);
int main(int argc,char **argv) {
    if (argc!=2) return 64;
    FILE *f=NULL;
    if (fopen_s(&f,argv[1],"rb") || !f) return 65;
    unsigned char bytes[512];
    size_t size=fread(bytes,1,sizeof(bytes),f); fclose(f);
    const unsigned char add[]={0xf2,0x40,0x0f,0x58,0xc1,0xc3};
    const unsigned char load[]={0xf2,0x40,0x0f,0x10,0x81,0,0,0,0,0xf2,0x40,0x0f,0x58,0xc1,0xc3};
    const unsigned char single[]={0xf3,0x40,0x0f,0x58,0xc1,0xc3};
    if (size!=134 || memcmp(bytes,add,sizeof(add)) || memcmp(bytes+16,load,sizeof(load)) ||
        memcmp(bytes+32,single,sizeof(single))) return 66;
    void *code=VirtualAlloc(NULL,size,MEM_COMMIT|MEM_RESERVE,PAGE_READWRITE);
    if (!code) return 67;
    memcpy(code,bytes,size);
    DWORD old; if (!VirtualProtect(code,size,PAGE_EXECUTE_READ,&old)) return 68;
    FlushInstructionCache(GetCurrentProcess(),code,size);
    unsigned char *p=code; double value=40, stored=0; float stored32=0;
    if (((binary64)p)(40,2)!=42 || ((load64)(p+16))(&value,2)!=42 ||
        ((binary32)(p+32))(40,2)!=42 || ((binary64)(p+48))(44,2)!=42 ||
        ((binary64)(p+64))(6,7)!=42 || ((binary64)(p+80))(84,2)!=42 ||
        ((frombits)(p+128))(UINT64_C(0x4045000000000000))!=42) return 69;
    ((store64)(p+96))(&stored,42); ((store32)(p+112))(&stored32,42);
    if (stored!=42 || stored32!=42) return 70;
    VirtualFree(code,0,MEM_RELEASE);
    puts("exact SSE bytes and f32/f64 native execution: ok"); return 0;
}
