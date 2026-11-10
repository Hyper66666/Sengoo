#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
extern int64_t sengoo_sg_sha256_init(uint8_t *,int64_t);
extern int64_t sengoo_sg_sha256_update(uint8_t *,int64_t,const uint8_t *,int64_t);
extern int64_t sengoo_sg_sha256_final(uint8_t *,int64_t,uint8_t *,int64_t);
extern int64_t sengoo_sg_sha256_dispose(uint8_t *,int64_t);
static uint8_t state[1024],initialized[1024],output[32];
static const uint8_t *input;static int64_t length;static uint64_t reads,writes,calls,runs;
static unsigned fixture;
static void fail(const char *reason){fprintf(stderr,"sha256-probe:fixture=%u:%s\n",fixture,reason);exit(1);}
static int64_t at(const uint8_t *p,int64_t i,int width){uintptr_t begin=(uintptr_t)state,value=(uintptr_t)p;
 if(value<begin||value-begin>1024||i<0||1024-(value-begin)<(unsigned)width||(uint64_t)i>(1024-(value-begin)-width)/(unsigned)width)fail("state-bound");return value-begin+i*width;}
uint8_t sengoo_tar_read(const uint8_t *p,int64_t i){uintptr_t begin=(uintptr_t)input,value=(uintptr_t)p;if(value<begin||value-begin>(uint64_t)length||i<0||(uint64_t)i>=(uint64_t)length-(value-begin))fail("input-bound");reads++;return p[i];}
uint8_t sengoo_tar_output_read(uint8_t *p,int64_t i){int64_t x=at(p,i,1);if(!initialized[x])fail("uninitialized-byte");reads++;return p[i];}
void sengoo_tar_write(uint8_t *p,int64_t i,uint8_t v){if(p==output){if(i<0||i>=32)fail("digest-bound");p[i]=v;writes++;return;}int64_t x=at(p,i,1);initialized[x]=1;p[i]=v;writes++;}
int64_t sengoo_ds_word_get(uint8_t *p,int64_t i){int64_t x=at(p,i,8),v;for(int n=0;n<8;n++)if(!initialized[x+n])fail("uninitialized-word");memcpy(&v,p+i*8,8);reads++;return v;}
void sengoo_ds_word_put(uint8_t *p,int64_t i,int64_t v){int64_t x=at(p,i,8);memset(initialized+x,1,8);memcpy(p+i*8,&v,8);writes++;}
static void check(const uint8_t *bytes,int64_t n,const uint8_t *expected,int schedule){
 input=bytes;length=n;memset(initialized,0,sizeof(initialized));memset(output,0xa5,sizeof(output));
 if(sengoo_sg_sha256_init(state,1024))fail("init");int64_t used=0;uint64_t random=76131;
 while(used<n){int64_t amount=n-used;if(schedule==1)amount=1;else if(schedule==2){random=random*6364136223846793005ULL+1;amount=1+(random%113);if(amount>n-used)amount=n-used;}else if(schedule>2&&used==0)amount=schedule-2;
  if(sengoo_sg_sha256_update(state,1024,bytes+used,amount))fail("update");used+=amount;calls++;}
 if(sengoo_sg_sha256_update(state,1024,NULL,0)||sengoo_sg_sha256_final(state,1024,output,32)||memcmp(output,expected,32))fail("actual-digest");
 uint64_t w=writes;if(sengoo_sg_sha256_final(state,1024,output,32)!=-4||sengoo_sg_sha256_update(state,1024,NULL,0)!=-4||writes!=w)fail("finished-state");
 if(sengoo_sg_sha256_dispose(state,1024))fail("dispose");for(int i=0;i<1024;i++)if(state[i])fail("disposed-not-zero");runs++;
}
int main(int argc,char **argv){
 if(argc!=2)return 64;FILE *file=NULL;if(fopen_s(&file,argv[1],"rb")||!file)return 65;fseek(file,0,SEEK_END);long size=ftell(file);rewind(file);uint8_t *raw=malloc(size);if(!raw||fread(raw,1,size,file)!=(size_t)size)return 66;fclose(file);
 if(size<12||memcmp(raw,"SGSHA1\0\0",8))return 67;uint32_t count;memcpy(&count,raw+8,4);size_t position=12;
 uint64_t r=reads,w=writes;if(sengoo_sg_sha256_init(NULL,0)!=-3||sengoo_sg_sha256_update(NULL,0,NULL,0)!=-3||sengoo_sg_sha256_update(NULL,1024,NULL,-1)!=-3||sengoo_sg_sha256_final(NULL,1024,NULL,31)!=-3||sengoo_sg_sha256_dispose(NULL,0)!=-3||reads!=r||writes!=w)fail("invalid-api-access");
 for(uint32_t i=0;i<count;i++){fixture=i;uint32_t n;if(position+4>(size_t)size)fail("corpus-size");memcpy(&n,raw+position,4);position+=4;if(position+n+32>(size_t)size)fail("corpus-entry");
  for(int schedule=0;schedule<3;schedule++)check(raw+position,n,raw+position+n,schedule);
  if(n<=128)for(uint32_t cut=1;cut<n;cut++)check(raw+position,n,raw+position+n,cut+2);position+=n+32;
 }
 if(position!=(size_t)size)fail("corpus-tail");printf("{\"schema\":1,\"cases\":%u,\"runs\":%llu,\"update_calls\":%llu,\"reads\":%llu,\"writes\":%llu,\"invalid_api_zero_access\":true,\"initialized_state_checks\":true,\"disposed_zero\":true}\n",count,(unsigned long long)runs,(unsigned long long)calls,(unsigned long long)reads,(unsigned long long)writes);free(raw);return 0;
}
