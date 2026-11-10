#include <stdint.h>
#include <stdlib.h>
/* The imported old synchronous exports need these symbols to link. Any actual
 * use by the new caller-owned framing path is a test failure, not an allocator. */
uint8_t *sengoo_ds_storage_alloc(int64_t length){(void)length;_Exit(99);}
int64_t sengoo_ds_storage_present(uint8_t *state){(void)state;_Exit(99);}
void sengoo_ds_storage_free(uint8_t *state){(void)state;_Exit(99);}
