/* Exercise real bridge cleanup with deterministic allocation/push failures. */
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

static int live_strings, live_vectors, fail_push, fail_copy, fail_string, fail_vector_new;
static void* probe_malloc(size_t size) { return fail_copy ? NULL : malloc(size); }
#define malloc probe_malloc
#include "../../sgc/src/python_bridge.c"
#undef malloc

typedef struct { int64_t len; int64_t values[8]; } ProbeVector;
int64_t sengoo_string_from_str_copy(int64_t value) {
    if (fail_string) return 0;
    const char* source = (const char*)(intptr_t)value;
    char* result = (char*)malloc(strlen(source) + 1);
    if (!result) return 0;
    memcpy(result, source, strlen(source) + 1); ++live_strings;
    return (int64_t)(intptr_t)result;
}
int64_t sengoo_string_as_str_ptr(int64_t handle) { return handle; }
int64_t sengoo_string_len(int64_t handle) { return (int64_t)strlen((char*)(intptr_t)handle); }
int64_t sengoo_string_free_status(int64_t handle) { free((void*)(intptr_t)handle); --live_strings; return 0; }
int64_t sengoo_raw_vec_new_parts(int64_t size, int64_t align, void* move, void* drop) {
    (void)size; (void)align; (void)move; (void)drop;
    if (fail_vector_new) return 0;
    ProbeVector* vector = (ProbeVector*)calloc(1, sizeof(ProbeVector));
    if (!vector) return 0;
    ++live_vectors; return (int64_t)(intptr_t)vector;
}
int64_t sengoo_raw_vec_push(int64_t handle, void* value) {
    ProbeVector* vector = (ProbeVector*)(intptr_t)handle;
    if (fail_push && vector->len >= 1) return -1;
    memcpy(&vector->values[vector->len++], value, 8); return 0;
}
int64_t sengoo_raw_vec_len(int64_t handle) { return ((ProbeVector*)(intptr_t)handle)->len; }
void* sengoo_raw_vec_get(int64_t handle, int64_t index) { return &((ProbeVector*)(intptr_t)handle)->values[index]; }
int64_t sengoo_raw_vec_free(int64_t handle) { free((void*)(intptr_t)handle); --live_vectors; return 0; }

int main(void) {
    int64_t values[] = {10, 20, 30};
    SgPyInput input = {0, values, 3};
    fail_vector_new = 1;
    assert(__sgpy_make(&input, 0, 2) == 0);
    assert(live_vectors == 0);
    fail_vector_new = 0;
    fail_push = 1;
    assert(__sgpy_make(&input, 0, 2) == 0);
    assert(live_vectors == 0);
    fail_push = 0;
    int64_t owner = __sgpy_make(&input, 0, 2);
    assert(owner > 0 && live_vectors == 1);
    SgPyOutput output;
    __sgpy_reset(&output);
    fail_copy = 1;
    __sgpy_owned(&output, owner, 2);
    assert(output.status == -1 && output.owner == owner);
    __sgpy_release_v1(&output);
    assert(live_vectors == 0);
    __sgpy_release_v1(&output); /* Idempotent cleanup of the cleared output. */
    assert(live_vectors == 0);
    fail_copy = 0;
    owner = __sgpy_make(&input, 0, 2);
    __sgpy_owned(&output, owner, 2);
    assert(output.status == 0 && output.len == 3);
    assert(((int64_t*)output.data)[2] == 30);
    __sgpy_release_v1(&output);
    assert(live_vectors == 0);
    input.data = "owned UTF-8";
    owner = __sgpy_make(&input, 0, 1);
    __sgpy_owned(&output, owner, 1);
    assert(live_strings == 1 && output.len == 11);
    __sgpy_release_v1(&output);
    assert(live_strings == 0);
    fail_string = 1;
    assert(__sgpy_make(&input, 0, 1) == 0 && live_strings == 0);
    fail_string = 0;
    /* The second native input can fail after the first input was allocated. */
    SgPyInput inputs[] = {{0, "first native input", 18}, {0, values, 3}};
    int64_t first = __sgpy_make(inputs, 0, 1);
    assert(first > 0 && live_strings == 1);
    fail_push = 1;
    assert(__sgpy_make(inputs, 1, 2) == 0);
    __sgpy_discard(first, 1);
    assert(live_strings == 0 && live_vectors == 0);
    return 0;
}
