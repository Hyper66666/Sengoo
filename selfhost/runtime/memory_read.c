#include <stdint.h>

uint8_t sengoo_runtime_memory_read_u8(const uint8_t *source, int64_t index) {
    return source[index];
}
