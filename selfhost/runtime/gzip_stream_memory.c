#include <stdint.h>

uint8_t *sengoo_gzs_state_offset(uint8_t *state, int64_t offset) {
    return offset == 0 ? state : state + offset;
}
