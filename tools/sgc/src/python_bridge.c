/* Private typed binding helpers. Export lists hide these and the native runtime. */
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>

#ifdef _WIN32
#define SGPY_EXPORT
#else
#define SGPY_EXPORT __attribute__((visibility("default")))
#endif

typedef struct { int64_t bits; void* data; int64_t len; } SgPyInput;
typedef struct {
    int64_t status, bits;
    void* data;
    int64_t len, owner, kind;
} SgPyOutput;

extern int64_t sengoo_string_from_str_copy(int64_t);
extern int64_t sengoo_string_as_str_ptr(int64_t);
extern int64_t sengoo_string_len(int64_t);
extern int64_t sengoo_string_free_status(int64_t);
extern int64_t sengoo_raw_vec_new_parts(int64_t, int64_t, void*, void*);
extern int64_t sengoo_raw_vec_push(int64_t, void*);
extern int64_t sengoo_raw_vec_len(int64_t);
extern void* sengoo_raw_vec_get(int64_t, int64_t);
extern int64_t sengoo_raw_vec_free(int64_t);

static void sgpy_move(void* dst, void* src) { memcpy(dst, src, 8); }
static void sgpy_drop(void* value) { (void)value; }
int64_t __sgpy_bits(SgPyInput* args, int64_t index) { return args[index].bits; }
void* __sgpy_ptr(SgPyInput* args, int64_t index) { return args[index].data; }
void __sgpy_reset(SgPyOutput* out) { memset(out, 0, sizeof(*out)); }
void __sgpy_error(SgPyOutput* out, int64_t status, int64_t code) { out->status = status; out->bits = code; }
void __sgpy_scalar(SgPyOutput* out, int64_t bits) { out->bits = bits; }
void __sgpy_discard(int64_t owner, int64_t kind) {
    if (owner <= 0) return;
    if (kind == 1) sengoo_string_free_status(owner);
    else if (kind == 2) sengoo_raw_vec_free(owner);
}
int64_t __sgpy_make(SgPyInput* args, int64_t index, int64_t kind) {
    SgPyInput* input = &args[index];
    if (kind == 1) return sengoo_string_from_str_copy((int64_t)(intptr_t)input->data);
    if (input->len < 0 || (uint64_t)input->len > SIZE_MAX / 8) return 0;
    int64_t owner = sengoo_raw_vec_new_parts(8, 8, (void*)sgpy_move, (void*)sgpy_drop);
    if (owner <= 0) return 0;
    for (int64_t i = 0; i < input->len; ++i) {
        if (sengoo_raw_vec_push(owner, (char*)input->data + i * 8) < 0) {
            sengoo_raw_vec_free(owner);
            return 0;
        }
    }
    return owner;
}
void __sgpy_owned(SgPyOutput* out, int64_t owner, int64_t kind) {
    out->owner = owner;
    out->kind = kind;
    if (owner <= 0) { __sgpy_error(out, -1, -1); return; }
    if (kind == 1) {
        out->data = (void*)(intptr_t)sengoo_string_as_str_ptr(owner);
        out->len = sengoo_string_len(owner);
        if (!out->data || out->len < 0) __sgpy_error(out, -1, -1);
        return;
    }
    out->len = sengoo_raw_vec_len(owner);
    if (out->len < 0 || (uint64_t)out->len > SIZE_MAX / 8) { __sgpy_error(out, -1, -1); return; }
    if (out->len == 0) return;
    out->data = malloc((size_t)out->len * 8);
    if (!out->data) { __sgpy_error(out, -1, -1); return; }
    for (int64_t i = 0; i < out->len; ++i) {
        void* value = sengoo_raw_vec_get(owner, i);
        if (!value) { __sgpy_error(out, -1, -1); return; }
        memcpy((char*)out->data + i * 8, value, 8);
    }
}
SGPY_EXPORT void __sgpy_release_v1(SgPyOutput* out) {
    if (out->kind == 2) free(out->data);
    __sgpy_discard(out->owner, out->kind);
    __sgpy_reset(out);
}
