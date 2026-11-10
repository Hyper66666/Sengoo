#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>
extern int64_t sengoo_sg_manifest_toml_parse(const uint8_t *,int64_t,uint8_t *,int64_t);
extern int64_t sengoo_sg_manifest_validate(const uint8_t *,int64_t,uint8_t *,int64_t);
extern int64_t sengoo_sg_semver_validate(const uint8_t *,int64_t,int64_t);
extern int64_t sengoo_sg_registry_metadata_parse(const uint8_t *,int64_t,uint8_t *,int64_t);
extern int64_t sengoo_sg_registry_metadata_encode(uint8_t *,int64_t,uint8_t *,int64_t);
extern int64_t sh_mt_word(uint8_t *,int64_t,int64_t);
static const uint8_t *input;static int64_t length;static uint8_t *state,*initialized;
static uint64_t reads,writes;
static uint8_t *encoded;
static int64_t encoded_written;
static int64_t encoded_capacity=1048704;
static void fail(const char *reason){fprintf(stderr,"manifest-probe:%s\n",reason);exit(1);}
static uint8_t source_byte(const uint8_t *value,int64_t index){
    if(value!=input||index<0||index>=length)fail("input-bound");reads++;return value[index];
}
uint8_t sengoo_tar_read(const uint8_t *value,int64_t index){
    if(value==input)return source_byte(value,index);
    uintptr_t start=(uintptr_t)state,at=(uintptr_t)value;if(at<start||at-start>8388608||index<0||(uint64_t)index>=8388608-(at-start))fail("aliased-state-bound");
    if(!initialized[at-start+index])fail("aliased-uninitialized-byte");reads++;return value[index];
}
uint8_t sengoo_runtime_memory_read_u8(const uint8_t *value,int64_t index){return sengoo_tar_read(value,index);}
const uint8_t *sengoo_ps_const_offset(uint8_t *value,int64_t index){if(value!=state||index<0||index>8388608)fail("pointer-offset");return index?value+index:value;}
static int64_t range(const uint8_t *value,int64_t index,int width){
    uintptr_t base=(uintptr_t)state,at=(uintptr_t)value;
    if(at<base||at-base>8388608||index<0||8388608-(at-base)<(unsigned)width||(uint64_t)index>(8388608-(at-base)-width)/(unsigned)width)fail("state-bound");
    return (int64_t)(at-base)+index*width;
}
uint8_t sengoo_tar_output_read(uint8_t *value,int64_t index){int64_t at=range(value,index,1);if(!initialized[at])fail("uninitialized-byte");reads++;return value[index];}
void sengoo_tar_write(uint8_t *value,int64_t index,uint8_t byte){
    if(encoded&&value==encoded){if(index<0||index>=encoded_capacity||index!=encoded_written)fail("encoded-write-bound-sequence");value[index]=byte;encoded_written++;writes++;return;}
    int64_t at=range(value,index,1);initialized[at]=1;value[index]=byte;writes++;
}
int64_t sengoo_ds_word_get(uint8_t *value,int64_t index){int64_t at=range(value,index,8),word;for(int i=0;i<8;i++)if(!initialized[at+i])fail("uninitialized-word");memcpy(&word,value+index*8,8);reads++;return word;}
void sengoo_ds_word_put(uint8_t *value,int64_t index,int64_t word){int64_t at=range(value,index,8);memset(initialized+at,1,8);memcpy(value+index*8,&word,8);writes++;}
static void hex(int64_t at,int64_t size){
    if(at<0||size<0||at>8388608-size)fail("printed-byte-range");
    for(int64_t i=0;i<size;i++)printf("%02x",sengoo_tar_output_read(state,at+i));
}
int main(int argc,char **argv){
    if(argc!=2&&argc!=3)return 64;FILE *file=NULL;if(fopen_s(&file,argv[argc-1],"rb")||!file)return 65;
    fseek(file,0,SEEK_END);length=ftell(file);rewind(file);if(length<0||length>1048576)return 66;
    uint8_t *bytes=malloc((size_t)length+1);if(!bytes||fread(bytes,1,(size_t)length,file)!=(size_t)length)return 67;fclose(file);input=bytes;
    if(argc==3){int mode=!strcmp(argv[1],"--req");if(!mode&&strcmp(argv[1],"--semver"))return 64;
        int64_t result=sengoo_sg_semver_validate(input,length,mode);printf("{\"schema\":1,\"valid\":%s,\"reads\":%llu}\n",result==1?"true":"false",(unsigned long long)reads);free(bytes);return 0;}
    state=VirtualAlloc(NULL,8388608,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);initialized=calloc(8388608,1);if(!state||!initialized)return 68;
    int64_t result=
#ifdef SENGOO_REGISTRY_METADATA
        sengoo_sg_registry_metadata_parse(input,length,state,8388608);
#elif defined(SENGOO_MANIFEST_VALIDATE)
        sengoo_sg_manifest_validate(input,length,state,8388608);
#else
        sengoo_sg_manifest_toml_parse(input,length,state,8388608);
#endif
    printf("{\"schema\":1,\"result\":%lld,\"error_at\":%lld,\"nodes\":[",(long long)result,(long long)sengoo_ds_word_get(state,6));
    int64_t count=sengoo_ds_word_get(state,3);
    if(result==0)for(int64_t id=0;id<count;id++){
        int64_t kind=sh_mt_word(state,id,0),n=sh_mt_word(state,id,3);if(id)putchar(',');
        printf("{\"id\":%lld,\"kind\":%lld,\"parent\":%lld,\"key\":\"",(long long)id,(long long)kind,(long long)sh_mt_word(state,id,1));hex(sh_mt_word(state,id,2),n);
        printf("\",\"first\":%lld,\"next\":%lld,\"flags\":%lld",(long long)sh_mt_word(state,id,6),(long long)sh_mt_word(state,id,7),(long long)sh_mt_word(state,id,8));
        if(kind==1){printf(",\"text\":\"");hex(sh_mt_word(state,id,4),sh_mt_word(state,id,5));putchar('"');}
        else if(kind==2)printf(",\"integer\":%lld",(long long)sh_mt_word(state,id,4));
        else if(kind==5)printf(",\"boolean\":%s",sh_mt_word(state,id,4)?"true":"false");putchar('}');
    }
    printf("]");
#ifdef SENGOO_REGISTRY_METADATA
    if(result==0){encoded=VirtualAlloc(NULL,1048704,MEM_RESERVE|MEM_COMMIT,PAGE_READWRITE);if(!encoded)return 69;
        int64_t n=sengoo_sg_registry_metadata_encode(state,8388608,encoded,1048704);if(n<0||n!=encoded_written)fail("encoded-result");
        printf(",\"encoded_hex\":\"");for(int64_t i=0;i<n;i++)printf("%02x",encoded[i]);putchar('"');
        encoded_written=0;encoded_capacity=n-1;if(sengoo_sg_registry_metadata_encode(state,8388608,encoded,n-1)!=-22||encoded_written>n-1)fail("encoded-capacity-minus-one");
        encoded_written=0;encoded_capacity=n;if(sengoo_sg_registry_metadata_encode(state,8388608,encoded,n)!=n||encoded_written!=n)fail("encoded-exact-capacity");
        uint64_t before_reads=reads,before_writes=writes;if(sengoo_sg_registry_metadata_encode(NULL,0,NULL,32)!=-3||sengoo_sg_registry_metadata_encode(NULL,8388608,NULL,1048705)!=-3||reads!=before_reads||writes!=before_writes)fail("encoded-bad-extent-access");
        printf(",\"encoder_capacity_controls\":4");VirtualFree(encoded,0,MEM_RELEASE);encoded=NULL;}
#endif
    printf(",\"reads\":%llu,\"writes\":%llu}\n",(unsigned long long)reads,(unsigned long long)writes);
    VirtualFree(state,0,MEM_RELEASE);free(initialized);free(bytes);return 0;
}
