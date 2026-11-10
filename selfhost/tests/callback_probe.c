#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <malloc.h>
#include <intrin.h>

typedef void (*Move)(void *, void *);
typedef void (*Drop)(void *);
typedef struct { int64_t token; int64_t value; } Owner16;
_Static_assert(sizeof(Owner16) == 16, "owner layout");
_Static_assert(_Alignof(Owner16) == 8, "owner alignment");
static int failed;
static int calls[5];
static int released;
static int alive;
static void *active;

void callback_forbidden_foreign_move(void *destination, void *source) {
    (void)destination;
    (void)source;
    failed = 3; /* It has a C definition, but no body in the SG MIR. */
}

void callback_touch(int64_t which, void *address) {
    volatile uint8_t dirty[8192];
    for (unsigned i = 0; i < sizeof dirty; ++i) dirty[i] = (uint8_t)(i ^ 0xa5);
    if (which < 1 || which > 4 || ((uintptr_t)address & (which >= 3 ? 15 : 7)) != 0 ||
        ((uintptr_t)_AddressOfReturnAddress() & 15) != 8 || dirty[17] != (uint8_t)(17 ^ 0xa5)) {
        failed = 1;
        return;
    }
    ++calls[which];
}

void callback_release(int64_t token) {
    if (!token || (void *)(uintptr_t)token != active || alive != 1) { failed = 2; return; }
    free(active); active = NULL; alive = 0; ++released;
}

int64_t callback_probe(void *move8, void *drop8, void *move_owner, void *drop_owner,
                       int64_t fifth, int64_t sixth) {
    Move move = (Move)move8, owner_move = (Move)move_owner;
    Drop drop = (Drop)drop8, owner_drop = (Drop)drop_owner;
    if (!move || !drop || !owner_move || !owner_drop || fifth != 17 || sixth != 19) return 70;
    for (int round = 0; round < 100; ++round) {
        int64_t *from = (int64_t *)_aligned_malloc(16, 16);
        int64_t *to = (int64_t *)_aligned_malloc(16, 16);
        if (!from || !to) return 71;
        from[0] = INT64_C(0x1122334455667788); to[0] = 0;
        move(to, from);
        if (from[0] != 0 || to[0] != INT64_C(0x1122334455667788)) return 72;
        drop(to);
        if (to[0] != 0) return 73;
        _aligned_free(to); _aligned_free(from);
        Owner16 *source = (Owner16 *)_aligned_malloc(sizeof(Owner16), 16);
        Owner16 *destination = (Owner16 *)_aligned_malloc(sizeof(Owner16), 16);
        if (!source || !destination) return 79;
        active = malloc(8); if (!active) return 74; alive = 1;
        source->token = (int64_t)(uintptr_t)active; source->value = 42;
        destination->token = destination->value = 0;
        owner_move(destination, source);
        if (source->token || source->value || (void *)(uintptr_t)destination->token != active || destination->value != 42 || alive != 1) return 75;
        owner_drop(destination);
        if (destination->token || destination->value || alive || active || failed) return 76;
        _aligned_free(destination); _aligned_free(source);
    }
    if (released != 100 || calls[1] != 100 || calls[2] != 100 || calls[3] != 100 || calls[4] != 100) return 77;
    return 42;
}

extern int64_t sg_callbacks_probe(void);
int main(void) {
    int64_t result = sg_callbacks_probe();
    if (result != 42 || failed || alive) { fprintf(stderr,"callback failure %lld/%d/%d\n",(long long)result,failed,alive); return 78; }
    puts("C -> SG -> C -> SG callbacks: move8/owned16, exact release, aligned stack and data passed");
    return 42;
}
