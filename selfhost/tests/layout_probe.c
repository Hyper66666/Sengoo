#include <stdint.h>
#include <stddef.h>
#include <stdio.h>

struct Small { uint8_t a; int64_t b; uint16_t c; };
struct Nested { uint8_t tag; struct Small items[3]; double d; };
struct Choice { int64_t tag; unsigned char payload[sizeof(struct Small)]; };

int main(void) {
    printf("%zu\n%zu\n%zu\n%zu\n%zu\n%zu\n%zu\n%zu\n%zu\n%zu\n",
        sizeof(struct Small), _Alignof(struct Small),
        offsetof(struct Small, b), offsetof(struct Small, c),
        sizeof(struct Nested), offsetof(struct Nested, items), offsetof(struct Nested, d),
        sizeof(struct Choice), offsetof(struct Choice, payload), sizeof(struct Small[3]));
    return 0;
}
