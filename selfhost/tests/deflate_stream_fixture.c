#include <stdint.h>
static uint8_t state[45056] = {1};
static uint8_t output[1] = {1};
static const uint8_t input[] = {0x73,0x1c,0x05,0x00};
uint8_t *sengoo_ds_fixture_state(void) { return state; }
uint8_t *sengoo_ds_fixture_output(void) { return output; }
const uint8_t *sengoo_ds_fixture_input(int64_t offset) { return input+offset; }
uint8_t sengoo_ds_fixture_byte(void) { return output[0]; }
uint8_t sengoo_ds_fixture_state_byte(int64_t offset) { return state[offset]; }
