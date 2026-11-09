#include <stdint.h>

const uint8_t *sengoo_runtime_text_fixture(int64_t kind) {
    static const uint8_t valid[] = {0xE4, 0xB8, 0x96, 0xF0, 0x9F, 0x98, 0x80};
    static const uint8_t invalid[] = {0xED, 0xA0, 0x80};
    return kind == 0 ? valid : invalid;
}
