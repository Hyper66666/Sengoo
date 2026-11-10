#include <stdint.h>
static uint8_t state[46080]={1};
static uint8_t output[1]={1};
static const uint8_t input[]={31,139,8,0,0,0,0,0,0,255,115,28,5,0,191,174,228,79,3,1,0,0};
uint8_t *sengoo_gzs_fixture_state(void){return state;}
uint8_t *sengoo_gzs_fixture_output(void){return output;}
const uint8_t *sengoo_gzs_fixture_input(int64_t offset){return input+offset;}
uint8_t sengoo_gzs_fixture_output_byte(void){return output[0];}
uint8_t sengoo_gzs_fixture_state_byte(int64_t offset){return state[offset];}
