#include <stdint.h>
uint8_t *sengoo_ts_mut_offset(uint8_t *bytes,int64_t offset){return offset?bytes+offset:bytes;}
const uint8_t *sengoo_ts_const_offset(uint8_t *bytes,int64_t offset){return offset?bytes+offset:bytes;}
