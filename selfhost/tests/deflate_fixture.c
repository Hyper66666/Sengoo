#include <stdint.h>

static const uint8_t stored[] = {1,3,0,252,255,65,66,67};
static const uint8_t fixed[] = {0x73,0x1c,0xf9,0,0};
static const uint8_t dynamic[] = {0x05,0xe0,0x01,0x04,0x00,0x00,0x00,0x00,0x10,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x00,0x04};
/* Nonzero initialization keeps the writable verification range in .data. */
static uint8_t output[512] = {1};
const uint8_t *sengoo_deflate_fixture_input(int64_t kind) {
    return kind==0?stored:kind==1?fixed:dynamic;
}
uint8_t *sengoo_deflate_fixture_output(void) { return output; }
int64_t sengoo_deflate_fixture_length(int64_t kind) {
    return kind==0?(int64_t)sizeof stored:kind==1?(int64_t)sizeof fixed:(int64_t)sizeof dynamic;
}
int64_t sengoo_deflate_fixture_check(int64_t kind,int64_t count) {
    if(kind==0)return count==3 && output[0]==65 && output[1]==66 && output[2]==67;
    if(kind==1) {
        if(count!=259)return 0;
        for(int64_t index=0;index<count;++index)if(output[index]!=65)return 0;
        return 1;
    }
    return kind==2 && count==0;
}
