#include <stdint.h>
#include <stdio.h>
extern int64_t sg_object_answer(void);
extern const int64_t sg_object_value;
extern const int64_t *sg_object_pointer;
int64_t sg_object_external(void) { return 40; }
int main(void) {
    if (sg_object_answer() != 42 || sg_object_value != 2 ||
        sg_object_pointer != &sg_object_value || *sg_object_pointer != 2) return 1;
    puts("SG COFF external call, data and absolute pointer: ok");
    return 0;
}
