#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int wmain(int argc,wchar_t **argv){
 if(argc!=4)return 64;
 unsigned long entry=wcstoul(argv[2],NULL,0),storage=wcstoul(argv[3],NULL,0);
 HANDLE file=CreateFileW(argv[1],GENERIC_READ,FILE_SHARE_READ,NULL,OPEN_EXISTING,0,NULL);if(file==INVALID_HANDLE_VALUE)return 65;
 unsigned char head[4096];DWORD read=0;if(!ReadFile(file,head,sizeof(head),&read,NULL)||read<512){CloseHandle(file);return 66;}CloseHandle(file);
 IMAGE_DOS_HEADER *d=(IMAGE_DOS_HEADER*)head;IMAGE_NT_HEADERS64 *n=(IMAGE_NT_HEADERS64*)(head+d->e_lfanew);if(d->e_magic!=IMAGE_DOS_SIGNATURE||n->Signature!=IMAGE_NT_SIGNATURE)return 67;
 void *reserved=VirtualAlloc((void*)(uintptr_t)n->OptionalHeader.ImageBase,n->OptionalHeader.SizeOfImage,MEM_RESERVE,PAGE_NOACCESS);
 HMODULE module=LoadLibraryExW(argv[1],NULL,DONT_RESOLVE_DLL_REFERENCES);if(!module)return 68;
 unsigned char *base=(unsigned char*)module;if((uintptr_t)base==n->OptionalHeader.ImageBase)return 69;
 IMAGE_NT_HEADERS64 *actual=(IMAGE_NT_HEADERS64*)(base+d->e_lfanew);IMAGE_SECTION_HEADER *sections=IMAGE_FIRST_SECTION(actual);unsigned long data=0;
 for(unsigned i=0;i<actual->FileHeader.NumberOfSections;i++)if(!memcmp(sections[i].Name,".data",5))data=sections[i].VirtualAddress;
 if(!data||storage+8193>actual->OptionalHeader.SizeOfImage)return 70;
 uintptr_t address=0;memcpy(&address,base+data,sizeof(address));if(address!=(uintptr_t)(base+storage))return 71;
 for(unsigned i=0;i<8193;i++)if(base[storage+i]!=0)return 72;
 int (__cdecl *function)(void)=(int(__cdecl*)(void))(base+entry);int result=function();if(result!=42||base[storage]!=20||base[storage+8192]!=20)return 73;
 if(!FreeLibrary(module))return 74;if(reserved)VirtualFree(reserved,0,MEM_RELEASE);
 puts("actual loader rebased BSS zero/write/address: ok");return 0;
}
