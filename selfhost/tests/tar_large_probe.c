#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <windows.h>
extern int64_t sengoo_sg_tar_scan(const uint8_t *, int64_t, uint8_t *, int64_t, uint8_t *, int64_t);
static int read_header(const char *directory,const char *name,uint8_t header[512]) {
    char path[32768];if(snprintf(path,sizeof(path),"%s/%s.header",directory,name)<0)return 0;
    FILE *file=fopen(path,"rb");if(!file)return 0;size_t got=fread(header,1,512,file);int tail=fgetc(file);fclose(file);return got==512&&tail==EOF;
}
static uint64_t word(const uint8_t *at){uint64_t value=0;for(int i=0;i<8;i++)value|=(uint64_t)at[i]<<(8*i);return value;}
int main(int argc,char **argv){
    if(argc!=2)return 2;uint8_t first[512],zero[512],one[512];
    if(!read_header(argv[1],"first",first)||!read_header(argv[1],"second-zero",zero)||!read_header(argv[1],"second-one",one))return 3;
    SYSTEM_INFO system;GetSystemInfo(&system);size_t page=system.dwPageSize;
    const size_t lengths[3]={268437504,268438016,536870912};
    for(int which=0;which<3;which++){
        size_t length=lengths[which],rounded=(length+page-1)/page*page;
        uint8_t *allocation=VirtualAlloc(NULL,rounded+page*2,MEM_RESERVE,PAGE_NOACCESS);if(!allocation)return 4;
        uint8_t *middle=VirtualAlloc(allocation+page,rounded,MEM_COMMIT,PAGE_READWRITE);if(!middle)return 5;
        uint8_t *input=middle+rounded-length;memcpy(input,first,512);memcpy(input+268435968,which==1?one:zero,512);
        DWORD old;if(!VirtualProtect(middle,rounded,PAGE_READONLY,&old))return 6;
        uint8_t *out=VirtualAlloc(NULL,page*5,MEM_RESERVE,PAGE_NOACCESS);if(!out)return 7;
        uint8_t *table_page=VirtualAlloc(out+page,page,MEM_COMMIT,PAGE_READWRITE);if(!table_page)return 8;
        uint8_t *path_page=VirtualAlloc(out+page*3,page,MEM_COMMIT,PAGE_READWRITE);if(!path_page)return 9;
        uint8_t *table=table_page+page-128,*paths=path_page+page-2;memset(table,0xa5,128);memset(paths,0xa5,2);
        int64_t result=sengoo_sg_tar_scan(input,(int64_t)length,table,128,paths,2);
        if(result!=(which==1?-7:2)){fprintf(stderr,"case=%d result=%lld\n",which,(long long)result);return 10;}
        if(result>=0){if(word(table)!=1||word(table+8)!=2||word(table+16)!=268435456||word(table+24)!=268437504||word(table+40)!=512||word(table+48)!=268435456||word(table+80)!=268435968||word(table+88)!=268436480||word(table+96)!=0||paths[0]!='a'||paths[1]!='b')return 11;}
        else for(int i=0;i<32;i++)if(table[i]!=0xa5)return 12;
        VirtualFree(out,0,MEM_RELEASE);VirtualFree(allocation,0,MEM_RELEASE);
    }
    puts("{\"schema\":1,\"ok\":true,\"cases\":3,\"content_limit\":268435456,\"raw_extent_limit\":536870912,\"readonly_input\":true,\"tail_guards\":true,\"all_input_bytes_observed\":false}");return 0;
}
