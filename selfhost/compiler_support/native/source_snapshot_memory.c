#include <stdint.h>
#include <string.h>

/* Memory/ABI leaves only. SG retains the SHA and UTF-8 algorithms unchanged. */
uint8_t sengoo_tar_read(const uint8_t *bytes,int64_t index){return bytes[index];}
uint8_t sengoo_tar_output_read(uint8_t *bytes,int64_t index){return bytes[index];}
void sengoo_tar_write(uint8_t *bytes,int64_t index,uint8_t value){bytes[index]=value;}
int64_t sengoo_ds_word_get(uint8_t *bytes,int64_t index){int64_t value;memcpy(&value,bytes+index*8,8);return value;}
void sengoo_ds_word_put(uint8_t *bytes,int64_t index,int64_t value){memcpy(bytes+index*8,&value,8);}
uint8_t sengoo_runtime_memory_read_u8(const uint8_t *bytes,int64_t index){return bytes[index];}
uint8_t *sengoo_compiler_snapshot_bytes_ptr(int64_t address){return (uint8_t*)(intptr_t)address;}
const uint8_t *sengoo_compiler_snapshot_const_bytes_ptr(int64_t address){return (const uint8_t*)(intptr_t)address;}
