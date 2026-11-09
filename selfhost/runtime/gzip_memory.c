#include <stdint.h>

const uint8_t *sengoo_gzip_input_offset(const uint8_t *source, int64_t index) {
    return index == 0 ? source : source + index;
}
uint8_t *sengoo_gzip_output_offset(uint8_t *target, int64_t index) {
    return index == 0 ? target : target + index;
}
