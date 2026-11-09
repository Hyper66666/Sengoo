#include <stdint.h>

uint8_t sengoo_deflate_input_read(const uint8_t *source, int64_t index) {
    return source[index];
}
uint8_t sengoo_deflate_output_read(uint8_t *target, int64_t index) {
    return target[index];
}
void sengoo_deflate_output_write(uint8_t *target, int64_t index, uint8_t value) {
    target[index] = value;
}
