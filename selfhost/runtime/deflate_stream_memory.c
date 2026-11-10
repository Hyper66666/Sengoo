#include <stdint.h>
#include <string.h>

uint8_t sengoo_ds_input_byte(const uint8_t *bytes, int64_t index) {
    return bytes[index];
}
uint8_t sengoo_ds_state_byte(uint8_t *bytes, int64_t index) {
    return bytes[index];
}
void sengoo_ds_state_write(uint8_t *bytes, int64_t index, uint8_t value) {
    bytes[index] = value;
}
int64_t sengoo_ds_word_get(uint8_t *bytes, int64_t index) {
    int64_t value;
    memcpy(&value, bytes + index * 8, 8);
    return value;
}
void sengoo_ds_word_put(uint8_t *bytes, int64_t index, int64_t value) {
    memcpy(bytes + index * 8, &value, 8);
}
void sengoo_ds_output_write(uint8_t *bytes, int64_t index, uint8_t value) {
    bytes[index] = value;
}
