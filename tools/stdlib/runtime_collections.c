#include "runtime_shared.h"

#include <stdatomic.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#include <malloc.h>
#endif

typedef struct {
    size_t strong;
    int kind;
    long long i64_value;
    void* payload;
    size_t payload_size;
    void (*drop_payload)(void*);
} SengooRcBox;

enum {
    SENGOO_RC_KIND_I64 = 1,
    SENGOO_RC_KIND_STRING = 2,
    SENGOO_RC_KIND_COPY = 3
};

extern long long sengoo_string_clone_status(long long handle);
extern long long sengoo_string_free_status(long long handle);

static SengooRcBox* sengoo_rc_from_handle(long long handle) {
    return (SengooRcBox*)sengoo_handle_to_ptr(handle);
}

static long long sengoo_rc_new_with_value(int kind, long long value) {
    SengooRcBox* box = (SengooRcBox*)calloc(1, sizeof(SengooRcBox));
    if (!box) {
        return 0;
    }
    box->strong = 1;
    box->kind = kind;
    box->i64_value = value;
    return sengoo_ptr_to_handle(box);
}

long long sengoo_rc_new_i64(long long value) {
    return sengoo_rc_new_with_value(SENGOO_RC_KIND_I64, value);
}

long long sengoo_rc_new_bool(long long value) {
    return sengoo_rc_new_with_value(SENGOO_RC_KIND_I64, value != 0 ? 1 : 0);
}

long long sengoo_rc_new_string(long long value) {
    long long owned = sengoo_string_clone_status(value);
    if (owned <= 0) {
        return 0;
    }
    return sengoo_rc_new_with_value(SENGOO_RC_KIND_STRING, owned);
}

long long sengoo_rc_new_copy(void* value, long long size, void* drop_fn) {
    if (!value || size < 0) {
        return 0;
    }
    SengooRcBox* box = (SengooRcBox*)calloc(1, sizeof(SengooRcBox));
    if (!box) {
        return 0;
    }
    box->payload_size = (size_t)size;
    if (box->payload_size > 0) {
        box->payload = malloc(box->payload_size);
        if (!box->payload) {
            free(box);
            return 0;
        }
        memcpy(box->payload, value, box->payload_size);
    }
    box->strong = 1;
    box->kind = SENGOO_RC_KIND_COPY;
    box->drop_payload = (void (*)(void*))drop_fn;
    return sengoo_ptr_to_handle(box);
}

long long sengoo_rc_clone(long long handle) {
    SengooRcBox* box = sengoo_rc_from_handle(handle);
    if (!box || box->strong == SIZE_MAX) {
        return 0;
    }
    box->strong += 1;
    return handle;
}

long long sengoo_rc_strong_count(long long handle) {
    SengooRcBox* box = sengoo_rc_from_handle(handle);
    return box ? (long long)box->strong : 0;
}

long long sengoo_rc_get_i64(long long handle) {
    SengooRcBox* box = sengoo_rc_from_handle(handle);
    return box ? box->i64_value : 0;
}

long long sengoo_rc_get_bool(long long handle) {
    return sengoo_rc_get_i64(handle) != 0 ? 1 : 0;
}

long long sengoo_rc_get_string_clone(long long handle) {
    SengooRcBox* box = sengoo_rc_from_handle(handle);
    if (!box || box->kind != SENGOO_RC_KIND_STRING) {
        return 0;
    }
    return sengoo_string_clone_status(box->i64_value);
}

void* sengoo_rc_borrow_ptr(long long handle) {
    SengooRcBox* box = sengoo_rc_from_handle(handle);
    if (!box) {
        return NULL;
    }
    if (box->kind == SENGOO_RC_KIND_COPY) {
        return box->payload;
    }
    return &box->i64_value;
}

long long sengoo_rc_drop(long long handle) {
    SengooRcBox* box = sengoo_rc_from_handle(handle);
    if (!box) {
        return 1;
    }
    if (box->strong == 0) {
        return 1;
    }
    box->strong -= 1;
    if (box->strong == 0) {
        if (box->kind == SENGOO_RC_KIND_STRING) {
            sengoo_string_free_status(box->i64_value);
        } else if (box->kind == SENGOO_RC_KIND_COPY) {
            if (box->drop_payload && box->payload) {
                box->drop_payload(box->payload);
            }
            free(box->payload);
        }
        free(box);
    }
    return 1;
}

typedef struct {
    atomic_size_t strong;
    long long value;
} SengooArcBox;

static SengooArcBox* sengoo_arc_from_handle(long long handle) {
    return (SengooArcBox*)sengoo_handle_to_ptr(handle);
}

static long long sengoo_arc_new_with_value(long long value) {
    SengooArcBox* box = (SengooArcBox*)calloc(1, sizeof(SengooArcBox));
    if (!box) {
        return 0;
    }
    atomic_init(&box->strong, 1);
    box->value = value;
    return sengoo_ptr_to_handle(box);
}

long long sengoo_arc_new_i64(long long value) {
    return sengoo_arc_new_with_value(value);
}

long long sengoo_arc_new_bool(long long value) {
    return sengoo_arc_new_with_value(value != 0 ? 1 : 0);
}

long long sengoo_arc_clone(long long handle) {
    SengooArcBox* box = sengoo_arc_from_handle(handle);
    if (!box) {
        return 0;
    }
    size_t old = atomic_load_explicit(&box->strong, memory_order_relaxed);
    while (old != SIZE_MAX) {
        if (atomic_compare_exchange_weak_explicit(
                &box->strong,
                &old,
                old + 1,
                memory_order_relaxed,
                memory_order_relaxed)) {
            return handle;
        }
    }
    return 0;
}

long long sengoo_arc_strong_count(long long handle) {
    SengooArcBox* box = sengoo_arc_from_handle(handle);
    return box ? (long long)atomic_load_explicit(&box->strong, memory_order_acquire) : 0;
}

long long sengoo_arc_get_i64(long long handle) {
    SengooArcBox* box = sengoo_arc_from_handle(handle);
    return box ? box->value : 0;
}

long long sengoo_arc_get_bool(long long handle) {
    return sengoo_arc_get_i64(handle) != 0 ? 1 : 0;
}

long long sengoo_arc_drop(long long handle) {
    SengooArcBox* box = sengoo_arc_from_handle(handle);
    if (!box) {
        return 1;
    }
    size_t old = atomic_load_explicit(&box->strong, memory_order_acquire);
    while (old > 0) {
        if (atomic_compare_exchange_weak_explicit(
                &box->strong,
                &old,
                old - 1,
                memory_order_acq_rel,
                memory_order_acquire)) {
            if (old == 1) {
                free(box);
            }
            return 1;
        }
    }
    return 1;
}

typedef struct {
    char** items;
    size_t len;
    size_t cap;
} SengooTextList;

typedef struct {
    long long list_handle;
    size_t index;
} SengooTextListIter;

static SengooTextList* sengoo_text_list_from_handle(long long handle) {
    return (SengooTextList*)sengoo_opaque_handle_get(handle);
}

static SengooTextListIter* sengoo_text_list_iter_from_handle(long long handle) {
    return (SengooTextListIter*)sengoo_opaque_handle_get(handle);
}

char* sengoo_copy_cstr_from_handle(long long value_ptr) {
    const char* value = (const char*)(intptr_t)value_ptr;
    if (!value) {
        return NULL;
    }
    size_t len = strlen(value);
    char* copy = (char*)malloc(len + 1);
    if (!copy) {
        return NULL;
    }
    memcpy(copy, value, len + 1);
    return copy;
}

static int sengoo_text_list_reserve(SengooTextList* list, size_t min_cap) {
    if (!list) {
        return 0;
    }
    if (list->cap >= min_cap) {
        return 1;
    }
    size_t next = list->cap == 0 ? 8 : list->cap;
    while (next < min_cap) {
        if (next > SIZE_MAX / 2) {
            return 0;
        }
        next *= 2;
    }
    char** items = (char**)realloc(list->items, next * sizeof(char*));
    if (!items) {
        return 0;
    }
    list->items = items;
    list->cap = next;
    return 1;
}

long long sengoo_text_list_new(void) {
    SengooTextList* list = (SengooTextList*)calloc(1, sizeof(SengooTextList));
    if (!list) {
        return 0;
    }
    long long handle = sengoo_opaque_handle_new(list);
    if (handle == 0) {
        free(list);
    }
    return handle;
}

long long sengoo_text_list_len(long long handle) {
    SengooTextList* list = sengoo_text_list_from_handle(handle);
    return list ? (long long)list->len : 0;
}

long long sengoo_text_list_clear_status(long long handle) {
    SengooTextList* list = sengoo_text_list_from_handle(handle);
    if (!list) {
        return 0;
    }
    for (size_t i = 0; i < list->len; ++i) {
        free(list->items[i]);
        list->items[i] = NULL;
    }
    list->len = 0;
    return 1;
}

long long sengoo_text_list_free_status(long long handle) {
    SengooTextList* list = (SengooTextList*)sengoo_opaque_handle_take(handle);
    if (!list) {
        return 1;
    }
    for (size_t i = 0; i < list->len; ++i) {
        free(list->items[i]);
    }
    free(list->items);
    free(list);
    return 1;
}

long long sengoo_text_list_push(long long handle, long long value_ptr) {
    SengooTextList* list = sengoo_text_list_from_handle(handle);
    if (!list) {
        return 0;
    }
    char* copy = sengoo_copy_cstr_from_handle(value_ptr);
    if (!copy) {
        return 0;
    }
    if (!sengoo_text_list_reserve(list, list->len + 1)) {
        free(copy);
        return 0;
    }
    list->items[list->len++] = copy;
    return 1;
}

long long sengoo_text_list_get_copy(long long handle, long long index, long long out_buffer) {
    SengooTextList* list = sengoo_text_list_from_handle(handle);
    if (!list) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (index < 0 || (size_t)index >= list->len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    const char* value = list->items[(size_t)index];
    return sengoo_copy_bytes_to_managed_buffer(out_buffer, value, strlen(value));
}

long long sengoo_text_list_set(long long handle, long long index, long long value_ptr) {
    SengooTextList* list = sengoo_text_list_from_handle(handle);
    if (!list || index < 0 || (size_t)index >= list->len) {
        return 0;
    }
    char* copy = sengoo_copy_cstr_from_handle(value_ptr);
    if (!copy) {
        return 0;
    }
    free(list->items[(size_t)index]);
    list->items[(size_t)index] = copy;
    return 1;
}

long long sengoo_text_list_remove_copy(long long handle, long long index, long long out_buffer) {
    SengooTextList* list = sengoo_text_list_from_handle(handle);
    if (!list) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (index < 0 || (size_t)index >= list->len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    size_t idx = (size_t)index;
    const char* value = list->items[idx];
    long long copied = sengoo_copy_bytes_to_managed_buffer(out_buffer, value, strlen(value));
    if (copied < 0) {
        return copied;
    }
    free(list->items[idx]);
    for (size_t i = idx + 1; i < list->len; ++i) {
        list->items[i - 1] = list->items[i];
    }
    list->len -= 1;
    if (list->len < list->cap) {
        list->items[list->len] = NULL;
    }
    return copied;
}

long long sengoo_text_list_iter_new(long long list_handle) {
    SengooTextList* list = sengoo_text_list_from_handle(list_handle);
    if (!list) {
        return 0;
    }
    SengooTextListIter* iter = (SengooTextListIter*)calloc(1, sizeof(SengooTextListIter));
    if (!iter) {
        return 0;
    }
    iter->list_handle = list_handle;
    iter->index = 0;
    long long handle = sengoo_opaque_handle_new(iter);
    if (handle == 0) {
        free(iter);
    }
    return handle;
}

long long sengoo_text_list_iter_done(long long iter_handle) {
    SengooTextListIter* iter = sengoo_text_list_iter_from_handle(iter_handle);
    SengooTextList* list = iter ? sengoo_text_list_from_handle(iter->list_handle) : NULL;
    return (!iter || !list || iter->index >= list->len) ? 1 : 0;
}

long long sengoo_text_list_iter_next_copy(long long iter_handle, long long out_buffer) {
    SengooTextListIter* iter = sengoo_text_list_iter_from_handle(iter_handle);
    SengooTextList* list = iter ? sengoo_text_list_from_handle(iter->list_handle) : NULL;
    if (!iter || !list) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (iter->index >= list->len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    const char* value = list->items[iter->index];
    long long copied = sengoo_copy_bytes_to_managed_buffer(out_buffer, value, strlen(value));
    if (copied >= 0) {
        iter->index += 1;
    }
    return copied;
}

long long sengoo_text_list_iter_reset_status(long long iter_handle) {
    SengooTextListIter* iter = sengoo_text_list_iter_from_handle(iter_handle);
    if (!iter) {
        return 0;
    }
    iter->index = 0;
    return 1;
}

long long sengoo_text_list_iter_free_status(long long iter_handle) {
    free(sengoo_opaque_handle_take(iter_handle));
    return 1;
}

typedef struct {
    char* key;
    long long i64_value;
    long long bool_value;
} SengooStringMapEntry;

typedef struct {
    SengooStringMapEntry* entries;
    size_t len;
    size_t cap;
} SengooStringMap;

typedef struct {
    long long map_handle;
    size_t index;
    size_t yielded;
} SengooStringMapKeyIter;

static SengooStringMap* sengoo_string_map_from_handle(long long handle) {
    return (SengooStringMap*)sengoo_opaque_handle_get(handle);
}

static SengooStringMapKeyIter* sengoo_string_map_key_iter_from_handle(long long handle) {
    return (SengooStringMapKeyIter*)sengoo_opaque_handle_get(handle);
}

static int sengoo_string_map_reserve(SengooStringMap* map, size_t min_cap) {
    if (!map) {
        return 0;
    }
    if (map->cap >= min_cap) {
        return 1;
    }
    size_t next = map->cap == 0 ? 8 : map->cap;
    while (next < min_cap) {
        if (next > SIZE_MAX / 2) {
            return 0;
        }
        next *= 2;
    }
    SengooStringMapEntry* entries = (SengooStringMapEntry*)realloc(
        map->entries,
        next * sizeof(SengooStringMapEntry)
    );
    if (!entries) {
        return 0;
    }
    map->entries = entries;
    map->cap = next;
    return 1;
}

static size_t sengoo_string_map_find_index(SengooStringMap* map, const char* key, int* found) {
    size_t low = 0;
    size_t high = map ? map->len : 0;
    if (found) {
        *found = 0;
    }

    while (low < high) {
        size_t mid = low + ((high - low) / 2);
        int cmp = strcmp(map->entries[mid].key, key);
        if (cmp == 0) {
            if (found) {
                *found = 1;
            }
            return mid;
        }
        if (cmp < 0) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    return low;
}

static long long sengoo_string_map_insert_value(long long handle, long long key_ptr, long long value, int is_bool) {
    SengooStringMap* map = sengoo_string_map_from_handle(handle);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!map || !key) {
        return 0;
    }

    int found = 0;
    size_t index = sengoo_string_map_find_index(map, key, &found);
    if (found) {
        if (is_bool) {
            map->entries[index].bool_value = value != 0 ? 1 : 0;
        } else {
            map->entries[index].i64_value = value;
        }
        return 1;
    }

    char* key_copy = sengoo_copy_cstr_from_handle(key_ptr);
    if (!key_copy || !sengoo_string_map_reserve(map, map->len + 1)) {
        free(key_copy);
        return 0;
    }
    for (size_t i = map->len; i > index; --i) {
        map->entries[i] = map->entries[i - 1];
    }
    map->entries[index].key = key_copy;
    map->entries[index].i64_value = is_bool ? 0 : value;
    map->entries[index].bool_value = is_bool ? (value != 0 ? 1 : 0) : 0;
    map->len += 1;
    return 1;
}

long long sengoo_string_map_new(void) {
    SengooStringMap* map = (SengooStringMap*)calloc(1, sizeof(SengooStringMap));
    if (!map) {
        return 0;
    }
    long long handle = sengoo_opaque_handle_new(map);
    if (handle == 0) {
        free(map);
    }
    return handle;
}

long long sengoo_string_map_len(long long handle) {
    SengooStringMap* map = sengoo_string_map_from_handle(handle);
    return map ? (long long)map->len : 0;
}

long long sengoo_string_map_clear_status(long long handle) {
    SengooStringMap* map = sengoo_string_map_from_handle(handle);
    if (!map) {
        return 0;
    }
    for (size_t i = 0; i < map->len; ++i) {
        free(map->entries[i].key);
        map->entries[i].key = NULL;
    }
    map->len = 0;
    return 1;
}

long long sengoo_string_map_free_status(long long handle) {
    SengooStringMap* map = (SengooStringMap*)sengoo_opaque_handle_take(handle);
    if (!map) {
        return 1;
    }
    for (size_t i = 0; i < map->len; ++i) {
        free(map->entries[i].key);
    }
    free(map->entries);
    free(map);
    return 1;
}

long long sengoo_string_map_insert_i64(long long handle, long long key, long long value) {
    return sengoo_string_map_insert_value(handle, key, value, 0);
}

long long sengoo_string_map_get_or_default_i64(long long handle, long long key_ptr, long long fallback) {
    SengooStringMap* map = sengoo_string_map_from_handle(handle);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!map || !key) {
        return fallback;
    }
    int found = 0;
    size_t index = sengoo_string_map_find_index(map, key, &found);
    return found ? map->entries[index].i64_value : fallback;
}

long long sengoo_string_map_insert_bool(long long handle, long long key, long long value) {
    return sengoo_string_map_insert_value(handle, key, value != 0 ? 1 : 0, 1);
}

long long sengoo_string_map_get_or_default_bool(long long handle, long long key_ptr, long long fallback) {
    SengooStringMap* map = sengoo_string_map_from_handle(handle);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!map || !key) {
        return fallback;
    }
    int found = 0;
    size_t index = sengoo_string_map_find_index(map, key, &found);
    return found ? map->entries[index].bool_value : fallback;
}

long long sengoo_string_map_contains(long long handle, long long key_ptr) {
    SengooStringMap* map = sengoo_string_map_from_handle(handle);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!map || !key) {
        return 0;
    }
    int found = 0;
    sengoo_string_map_find_index(map, key, &found);
    return found ? 1 : 0;
}

long long sengoo_string_map_remove(long long handle, long long key_ptr) {
    SengooStringMap* map = sengoo_string_map_from_handle(handle);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!map || !key) {
        return 0;
    }
    int found = 0;
    size_t index = sengoo_string_map_find_index(map, key, &found);
    if (!found) {
        return 0;
    }
    free(map->entries[index].key);
    for (size_t i = index + 1; i < map->len; ++i) {
        map->entries[i - 1] = map->entries[i];
    }
    map->len -= 1;
    if (map->len < map->cap) {
        memset(&map->entries[map->len], 0, sizeof(SengooStringMapEntry));
    }
    return 1;
}

long long sengoo_string_map_key_iter_new(long long map_handle) {
    SengooStringMap* map = sengoo_string_map_from_handle(map_handle);
    if (!map) {
        return 0;
    }
    SengooStringMapKeyIter* iter = (SengooStringMapKeyIter*)calloc(1, sizeof(SengooStringMapKeyIter));
    if (!iter) {
        return 0;
    }
    iter->map_handle = map_handle;
    iter->index = 0;
    iter->yielded = 0;
    long long handle = sengoo_opaque_handle_new(iter);
    if (handle == 0) {
        free(iter);
    }
    return handle;
}

long long sengoo_string_map_key_iter_done(long long iter_handle) {
    SengooStringMapKeyIter* iter = sengoo_string_map_key_iter_from_handle(iter_handle);
    SengooStringMap* map = iter ? sengoo_string_map_from_handle(iter->map_handle) : NULL;
    return (!iter || !map || iter->index >= map->len) ? 1 : 0;
}

long long sengoo_string_map_key_iter_index(long long iter_handle) {
    SengooStringMapKeyIter* iter = sengoo_string_map_key_iter_from_handle(iter_handle);
    return iter ? (long long)iter->yielded : 0;
}

long long sengoo_string_map_key_iter_next_copy(long long iter_handle, long long out_buffer) {
    SengooStringMapKeyIter* iter = sengoo_string_map_key_iter_from_handle(iter_handle);
    SengooStringMap* map = iter ? sengoo_string_map_from_handle(iter->map_handle) : NULL;
    if (!iter || !map) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (iter->index >= map->len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    const char* key = map->entries[iter->index].key;
    long long copied = sengoo_copy_bytes_to_managed_buffer(out_buffer, key, strlen(key));
    if (copied >= 0) {
        iter->index += 1;
        iter->yielded += 1;
    }
    return copied;
}

long long sengoo_string_map_key_iter_reset_status(long long iter_handle) {
    SengooStringMapKeyIter* iter = sengoo_string_map_key_iter_from_handle(iter_handle);
    if (!iter) {
        return 0;
    }
    iter->index = 0;
    iter->yielded = 0;
    return 1;
}

long long sengoo_string_map_key_iter_free_status(long long iter_handle) {
    free(sengoo_opaque_handle_take(iter_handle));
    return 1;
}

extern long long sengoo_string_free_status(long long handle);

long long sengoo_collections_abi_version(void) {
    return SENGOO_COLLECTIONS_ABI_VERSION;
}

long long sengoo_type_descriptor_validate(const SengooTypeDescriptor* descriptor) {
    if (!descriptor || descriptor->abi_version != SENGOO_COLLECTIONS_ABI_VERSION) {
        return SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (descriptor->size == 0 || descriptor->align == 0
        || (descriptor->align & (descriptor->align - 1)) != 0) {
        return SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (!descriptor->move_value || !descriptor->drop_value) {
        return SENGOO_STATUS_INVALID_ARGUMENT;
    }
    return SENGOO_STATUS_OK;
}

typedef struct {
    unsigned char* data;
    size_t len;
    size_t capacity;
    SengooTypeDescriptor element;
} SengooRawVec;

static void* sengoo_aligned_alloc_bytes(size_t align, size_t bytes) {
    if (bytes == 0) {
        return NULL;
    }
#ifdef _WIN32
    return _aligned_malloc(bytes, align);
#else
    size_t rounded = bytes;
    size_t remainder = rounded % align;
    if (remainder != 0) {
        if (rounded > SIZE_MAX - (align - remainder)) {
            return NULL;
        }
        rounded += align - remainder;
    }
    return aligned_alloc(align, rounded);
#endif
}

static void sengoo_aligned_free_bytes(void* value) {
#ifdef _WIN32
    _aligned_free(value);
#else
    free(value);
#endif
}

static SengooRawVec* sengoo_raw_vec_from_handle(long long handle) {
    return (SengooRawVec*)sengoo_opaque_handle_get(handle);
}

static void* sengoo_raw_vec_slot(const SengooRawVec* vec, size_t index) {
    return vec->data + index * vec->element.size;
}

static int sengoo_raw_vec_reserve(SengooRawVec* vec, size_t minimum) {
    if (minimum <= vec->capacity) {
        return 1;
    }
    size_t capacity = vec->capacity == 0 ? 4 : vec->capacity;
    while (capacity < minimum) {
        if (capacity > SIZE_MAX / 2) {
            return 0;
        }
        capacity *= 2;
    }
    if (vec->element.size != 0 && capacity > SIZE_MAX / vec->element.size) {
        return 0;
    }
    unsigned char* replacement = (unsigned char*)sengoo_aligned_alloc_bytes(
        vec->element.align,
        capacity * vec->element.size
    );
    if (!replacement) {
        return 0;
    }
    for (size_t index = 0; index < vec->len; ++index) {
        vec->element.move_value(
            replacement + index * vec->element.size,
            sengoo_raw_vec_slot(vec, index)
        );
    }
    sengoo_aligned_free_bytes(vec->data);
    vec->data = replacement;
    vec->capacity = capacity;
    return 1;
}

long long sengoo_raw_vec_new(const SengooTypeDescriptor* descriptor) {
    if (sengoo_type_descriptor_validate(descriptor) != SENGOO_STATUS_OK) {
        return 0;
    }
    SengooRawVec* vec = (SengooRawVec*)calloc(1, sizeof(SengooRawVec));
    if (!vec) {
        return 0;
    }
    vec->element = *descriptor;
    long long handle = sengoo_opaque_handle_new(vec);
    if (handle == 0) {
        free(vec);
    }
    return handle;
}

long long sengoo_raw_vec_new_parts(
    long long size,
    long long align,
    void* move_value,
    void* drop_value
) {
    if (size <= 0 || align <= 0) {
        return 0;
    }
    SengooTypeDescriptor descriptor = {
        SENGOO_COLLECTIONS_ABI_VERSION,
        0,
        (size_t)size,
        (size_t)align,
        (SengooMoveFn)move_value,
        (SengooDropFn)drop_value,
        NULL,
        NULL,
        NULL,
        NULL
    };
    return sengoo_raw_vec_new(&descriptor);
}

long long sengoo_raw_vec_len(long long handle) {
    SengooRawVec* vec = sengoo_raw_vec_from_handle(handle);
    return vec ? (long long)vec->len : 0;
}

long long sengoo_raw_vec_push(long long handle, void* value) {
    SengooRawVec* vec = sengoo_raw_vec_from_handle(handle);
    if (!vec || !value) {
        return SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (!sengoo_raw_vec_reserve(vec, vec->len + 1)) {
        vec->element.drop_value(value);
        return SENGOO_STATUS_OUT_OF_MEMORY;
    }
    vec->element.move_value(sengoo_raw_vec_slot(vec, vec->len), value);
    vec->len += 1;
    return SENGOO_STATUS_OK;
}

void* sengoo_raw_vec_get(long long handle, long long index) {
    SengooRawVec* vec = sengoo_raw_vec_from_handle(handle);
    if (!vec || index < 0 || (size_t)index >= vec->len) {
        return NULL;
    }
    return sengoo_raw_vec_slot(vec, (size_t)index);
}

long long sengoo_raw_vec_set(long long handle, long long index, void* value) {
    SengooRawVec* vec = sengoo_raw_vec_from_handle(handle);
    if (!vec || !value) {
        return SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (index < 0 || (size_t)index >= vec->len) {
        vec->element.drop_value(value);
        return SENGOO_STATUS_INVALID_ARGUMENT;
    }
    void* slot = sengoo_raw_vec_slot(vec, (size_t)index);
    if (slot == value) {
        return SENGOO_STATUS_OK;
    }
    vec->element.drop_value(slot);
    vec->element.move_value(slot, value);
    return SENGOO_STATUS_OK;
}

long long sengoo_raw_vec_insert(long long handle, long long index, void* value) {
    SengooRawVec* vec = sengoo_raw_vec_from_handle(handle);
    if (!vec || !value) {
        return SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (index < 0 || (size_t)index > vec->len) {
        vec->element.drop_value(value);
        return SENGOO_STATUS_INVALID_ARGUMENT;
    }
    if (!sengoo_raw_vec_reserve(vec, vec->len + 1)) {
        vec->element.drop_value(value);
        return SENGOO_STATUS_OUT_OF_MEMORY;
    }
    size_t at = (size_t)index;
    for (size_t cursor = vec->len; cursor > at; --cursor) {
        vec->element.move_value(
            sengoo_raw_vec_slot(vec, cursor),
            sengoo_raw_vec_slot(vec, cursor - 1)
        );
    }
    vec->element.move_value(sengoo_raw_vec_slot(vec, at), value);
    vec->len += 1;
    return SENGOO_STATUS_OK;
}

long long sengoo_raw_vec_pop(long long handle, void* out_value) {
    SengooRawVec* vec = sengoo_raw_vec_from_handle(handle);
    if (!vec || !out_value || vec->len == 0) {
        return SENGOO_STATUS_NOT_FOUND;
    }
    vec->len -= 1;
    vec->element.move_value(out_value, sengoo_raw_vec_slot(vec, vec->len));
    return SENGOO_STATUS_OK;
}

long long sengoo_raw_vec_remove(long long handle, long long index, void* out_value) {
    SengooRawVec* vec = sengoo_raw_vec_from_handle(handle);
    if (!vec || !out_value || index < 0 || (size_t)index >= vec->len) {
        return SENGOO_STATUS_NOT_FOUND;
    }
    size_t at = (size_t)index;
    vec->element.move_value(out_value, sengoo_raw_vec_slot(vec, at));
    for (size_t cursor = at + 1; cursor < vec->len; ++cursor) {
        vec->element.move_value(
            sengoo_raw_vec_slot(vec, cursor - 1),
            sengoo_raw_vec_slot(vec, cursor)
        );
    }
    vec->len -= 1;
    return SENGOO_STATUS_OK;
}

void sengoo_raw_zero_bytes(void* value, long long size) {
    if (value && size > 0) memset(value, 0, (size_t)size);
}

long long sengoo_raw_vec_remove_string(long long handle, long long index) {
    long long value = 0;
    long long status = sengoo_raw_vec_remove(handle, index, &value);
    return status == SENGOO_STATUS_OK ? value : -status;
}

long long sengoo_raw_vec_clear(long long handle) {
    SengooRawVec* vec = sengoo_raw_vec_from_handle(handle);
    if (!vec) {
        return SENGOO_STATUS_INVALID_HANDLE;
    }
    while (vec->len > 0) {
        vec->len -= 1;
        vec->element.drop_value(sengoo_raw_vec_slot(vec, vec->len));
    }
    return SENGOO_STATUS_OK;
}

long long sengoo_raw_vec_free(long long handle) {
    SengooRawVec* vec = sengoo_raw_vec_from_handle(handle);
    if (!vec) {
        return SENGOO_STATUS_INVALID_HANDLE;
    }
    sengoo_raw_vec_clear(handle);
    vec = (SengooRawVec*)sengoo_opaque_handle_take(handle);
    if (!vec) {
        return SENGOO_STATUS_INVALID_HANDLE;
    }
    sengoo_aligned_free_bytes(vec->data);
    free(vec);
    return SENGOO_STATUS_OK;
}

typedef struct {
    long long vec_handle;
    size_t index;
} SengooRawVecIter;

long long sengoo_raw_vec_iter_new(long long handle) {
    if (!sengoo_raw_vec_from_handle(handle)) return 0;
    SengooRawVecIter* iter = (SengooRawVecIter*)calloc(1, sizeof(SengooRawVecIter));
    if (!iter) return 0;
    iter->vec_handle = handle;
    return sengoo_opaque_handle_new(iter);
}

void* sengoo_raw_vec_iter_next(long long handle) {
    SengooRawVecIter* iter = (SengooRawVecIter*)sengoo_opaque_handle_get(handle);
    if (!iter) return NULL;
    SengooRawVec* vec = sengoo_raw_vec_from_handle(iter->vec_handle);
    if (!vec || iter->index >= vec->len) return NULL;
    return sengoo_raw_vec_slot(vec, iter->index++);
}

long long sengoo_raw_vec_iter_done(long long handle) {
    SengooRawVecIter* iter = (SengooRawVecIter*)sengoo_opaque_handle_get(handle);
    if (!iter) return 1;
    SengooRawVec* vec = sengoo_raw_vec_from_handle(iter->vec_handle);
    return !vec || iter->index >= vec->len;
}

long long sengoo_raw_vec_iter_reset(long long handle) {
    SengooRawVecIter* iter = (SengooRawVecIter*)sengoo_opaque_handle_get(handle);
    if (!iter) return SENGOO_STATUS_INVALID_HANDLE;
    iter->index = 0;
    return SENGOO_STATUS_OK;
}

long long sengoo_raw_vec_iter_index(long long handle) {
    SengooRawVecIter* iter = (SengooRawVecIter*)sengoo_opaque_handle_get(handle);
    return iter ? (long long)iter->index : 0;
}

long long sengoo_raw_vec_iter_free(long long handle) {
    SengooRawVecIter* iter = (SengooRawVecIter*)sengoo_opaque_handle_take(handle);
    if (!iter) return SENGOO_STATUS_INVALID_ARGUMENT;
    free(iter);
    return SENGOO_STATUS_OK;
}

typedef struct {
    unsigned char* keys;
    unsigned char* values;
    uint64_t* hashes;
    size_t len;
    size_t capacity;
    SengooTypeDescriptor key;
    SengooTypeDescriptor value;
} SengooRawHashMap;

static SengooRawHashMap* sengoo_raw_hashmap_from_handle(long long handle) {
    return (SengooRawHashMap*)sengoo_opaque_handle_get(handle);
}

static void* sengoo_raw_hashmap_key(const SengooRawHashMap* map, size_t index) {
    return map->keys + index * map->key.size;
}

static void* sengoo_raw_hashmap_value(const SengooRawHashMap* map, size_t index) {
    return map->values + index * map->value.size;
}

static size_t sengoo_raw_hashmap_find(
    const SengooRawHashMap* map,
    const void* key,
    uint64_t hash
) {
    for (size_t index = 0; index < map->len; ++index) {
        if (map->key.compare_value) {
            long long order = map->key.compare_value(
                sengoo_raw_hashmap_key(map, index), key
            );
            if (order == 0) return index;
            if (order > 0) return SIZE_MAX;
            continue;
        }
        if (map->hashes[index] == hash
            && map->key.eq_value(sengoo_raw_hashmap_key(map, index), key)) {
            return index;
        }
    }
    return SIZE_MAX;
}

static size_t sengoo_raw_btreemap_insert_index(
    const SengooRawHashMap* map,
    const void* key
) {
    size_t index = 0;
    while (index < map->len
        && map->key.compare_value(sengoo_raw_hashmap_key(map, index), key) < 0) {
        index += 1;
    }
    return index;
}

static int sengoo_raw_hashmap_reserve(SengooRawHashMap* map, size_t minimum) {
    if (minimum <= map->capacity) return 1;
    size_t capacity = map->capacity == 0 ? 8 : map->capacity;
    while (capacity < minimum) {
        if (capacity > SIZE_MAX / 2) return 0;
        capacity *= 2;
    }
    if (capacity > SIZE_MAX / map->key.size || capacity > SIZE_MAX / map->value.size
        || capacity > SIZE_MAX / sizeof(uint64_t)) return 0;
    unsigned char* keys = (unsigned char*)sengoo_aligned_alloc_bytes(
        map->key.align, capacity * map->key.size
    );
    unsigned char* values = (unsigned char*)sengoo_aligned_alloc_bytes(
        map->value.align, capacity * map->value.size
    );
    uint64_t* hashes = (uint64_t*)calloc(capacity, sizeof(uint64_t));
    if (!keys || !values || !hashes) {
        sengoo_aligned_free_bytes(keys);
        sengoo_aligned_free_bytes(values);
        free(hashes);
        return 0;
    }
    for (size_t index = 0; index < map->len; ++index) {
        map->key.move_value(keys + index * map->key.size, sengoo_raw_hashmap_key(map, index));
        map->value.move_value(values + index * map->value.size, sengoo_raw_hashmap_value(map, index));
        hashes[index] = map->hashes[index];
    }
    sengoo_aligned_free_bytes(map->keys);
    sengoo_aligned_free_bytes(map->values);
    free(map->hashes);
    map->keys = keys;
    map->values = values;
    map->hashes = hashes;
    map->capacity = capacity;
    return 1;
}

long long sengoo_raw_hashmap_new_parts(
    long long key_size, long long key_align, void* key_move, void* key_drop,
    void* key_hash, void* key_eq, long long value_size, long long value_align,
    void* value_move, void* value_drop
) {
    if (key_size <= 0 || key_align <= 0 || value_size <= 0 || value_align <= 0
        || !key_move || !key_drop || !key_hash || !key_eq || !value_move || !value_drop) return 0;
    SengooRawHashMap* map = (SengooRawHashMap*)calloc(1, sizeof(SengooRawHashMap));
    if (!map) return 0;
    map->key = (SengooTypeDescriptor){
        SENGOO_COLLECTIONS_ABI_VERSION, 0, (size_t)key_size, (size_t)key_align,
        (SengooMoveFn)key_move, (SengooDropFn)key_drop, NULL,
        (SengooHashFn)key_hash, (SengooEqFn)key_eq, NULL
    };
    map->value = (SengooTypeDescriptor){
        SENGOO_COLLECTIONS_ABI_VERSION, 0, (size_t)value_size, (size_t)value_align,
        (SengooMoveFn)value_move, (SengooDropFn)value_drop, NULL, NULL, NULL, NULL
    };
    long long handle = sengoo_opaque_handle_new(map);
    if (!handle) free(map);
    return handle;
}

long long sengoo_raw_btreemap_new_parts(
    long long key_size, long long key_align, void* key_move, void* key_drop,
    void* key_compare, long long value_size, long long value_align,
    void* value_move, void* value_drop
) {
    if (key_size <= 0 || key_align <= 0 || value_size <= 0 || value_align <= 0
        || !key_move || !key_drop || !key_compare || !value_move || !value_drop) return 0;
    SengooRawHashMap* map = (SengooRawHashMap*)calloc(1, sizeof(SengooRawHashMap));
    if (!map) return 0;
    map->key = (SengooTypeDescriptor){
        SENGOO_COLLECTIONS_ABI_VERSION, 0, (size_t)key_size, (size_t)key_align,
        (SengooMoveFn)key_move, (SengooDropFn)key_drop, NULL, NULL, NULL,
        (SengooCompareFn)key_compare
    };
    map->value = (SengooTypeDescriptor){
        SENGOO_COLLECTIONS_ABI_VERSION, 0, (size_t)value_size, (size_t)value_align,
        (SengooMoveFn)value_move, (SengooDropFn)value_drop, NULL, NULL, NULL, NULL
    };
    long long handle = sengoo_opaque_handle_new(map);
    if (!handle) free(map);
    return handle;
}

long long sengoo_raw_hashmap_len(long long handle) {
    SengooRawHashMap* map = sengoo_raw_hashmap_from_handle(handle);
    return map ? (long long)map->len : 0;
}

long long sengoo_raw_hashmap_insert(long long handle, void* key, void* value) {
    SengooRawHashMap* map = sengoo_raw_hashmap_from_handle(handle);
    if (!map || !key || !value) return SENGOO_STATUS_INVALID_ARGUMENT;
    uint64_t hash = map->key.hash_value ? map->key.hash_value(key) : 0;
    size_t existing = sengoo_raw_hashmap_find(map, key, hash);
    if (existing != SIZE_MAX) {
        map->key.drop_value(key);
        map->value.drop_value(sengoo_raw_hashmap_value(map, existing));
        map->value.move_value(sengoo_raw_hashmap_value(map, existing), value);
        return SENGOO_STATUS_OK;
    }
    if (!sengoo_raw_hashmap_reserve(map, map->len + 1)) {
        map->key.drop_value(key);
        map->value.drop_value(value);
        return SENGOO_STATUS_OUT_OF_MEMORY;
    }
    size_t insertion = map->key.compare_value
        ? sengoo_raw_btreemap_insert_index(map, key)
        : map->len;
    for (size_t cursor = map->len; cursor > insertion; --cursor) {
        map->key.move_value(sengoo_raw_hashmap_key(map, cursor), sengoo_raw_hashmap_key(map, cursor - 1));
        map->value.move_value(sengoo_raw_hashmap_value(map, cursor), sengoo_raw_hashmap_value(map, cursor - 1));
        map->hashes[cursor] = map->hashes[cursor - 1];
    }
    map->key.move_value(sengoo_raw_hashmap_key(map, insertion), key);
    map->value.move_value(sengoo_raw_hashmap_value(map, insertion), value);
    map->hashes[insertion] = hash;
    map->len += 1;
    return SENGOO_STATUS_OK;
}

void* sengoo_raw_hashmap_get(long long handle, const void* key) {
    SengooRawHashMap* map = sengoo_raw_hashmap_from_handle(handle);
    if (!map || !key) return NULL;
    uint64_t hash = map->key.hash_value ? map->key.hash_value(key) : 0;
    size_t index = sengoo_raw_hashmap_find(map, key, hash);
    return index == SIZE_MAX ? NULL : sengoo_raw_hashmap_value(map, index);
}

long long sengoo_raw_hashmap_contains(long long handle, const void* key) {
    return sengoo_raw_hashmap_get(handle, key) ? 1 : 0;
}

long long sengoo_raw_hashmap_remove(long long handle, const void* key, void* out_value) {
    SengooRawHashMap* map = sengoo_raw_hashmap_from_handle(handle);
    if (!map || !key || !out_value) return SENGOO_STATUS_INVALID_ARGUMENT;
    uint64_t hash = map->key.hash_value ? map->key.hash_value(key) : 0;
    size_t index = sengoo_raw_hashmap_find(map, key, hash);
    if (index == SIZE_MAX) return SENGOO_STATUS_NOT_FOUND;
    map->key.drop_value(sengoo_raw_hashmap_key(map, index));
    map->value.move_value(out_value, sengoo_raw_hashmap_value(map, index));
    for (size_t cursor = index + 1; cursor < map->len; ++cursor) {
        map->key.move_value(sengoo_raw_hashmap_key(map, cursor - 1), sengoo_raw_hashmap_key(map, cursor));
        map->value.move_value(sengoo_raw_hashmap_value(map, cursor - 1), sengoo_raw_hashmap_value(map, cursor));
        map->hashes[cursor - 1] = map->hashes[cursor];
    }
    map->len -= 1;
    return SENGOO_STATUS_OK;
}

long long sengoo_raw_hashmap_remove_string(long long handle, const void* key) {
    long long value = 0;
    long long status = sengoo_raw_hashmap_remove(handle, key, &value);
    return status == SENGOO_STATUS_OK ? value : -status;
}

long long sengoo_raw_hashmap_clear(long long handle) {
    SengooRawHashMap* map = sengoo_raw_hashmap_from_handle(handle);
    if (!map) return SENGOO_STATUS_INVALID_HANDLE;
    while (map->len > 0) {
        map->len -= 1;
        map->key.drop_value(sengoo_raw_hashmap_key(map, map->len));
        map->value.drop_value(sengoo_raw_hashmap_value(map, map->len));
    }
    return SENGOO_STATUS_OK;
}

long long sengoo_raw_hashmap_free(long long handle) {
    SengooRawHashMap* map = sengoo_raw_hashmap_from_handle(handle);
    if (!map) return SENGOO_STATUS_INVALID_HANDLE;
    sengoo_raw_hashmap_clear(handle);
    map = (SengooRawHashMap*)sengoo_opaque_handle_take(handle);
    if (!map) return SENGOO_STATUS_INVALID_HANDLE;
    sengoo_aligned_free_bytes(map->keys);
    sengoo_aligned_free_bytes(map->values);
    free(map->hashes);
    free(map);
    return SENGOO_STATUS_OK;
}

typedef struct {
    long long map_handle;
    size_t index;
} SengooRawMapKeyIter;

long long sengoo_raw_map_key_iter_new(long long handle) {
    if (!sengoo_raw_hashmap_from_handle(handle)) return 0;
    SengooRawMapKeyIter* iter = (SengooRawMapKeyIter*)calloc(1, sizeof(SengooRawMapKeyIter));
    if (!iter) return 0;
    iter->map_handle = handle;
    return sengoo_opaque_handle_new(iter);
}

void* sengoo_raw_map_key_iter_next(long long handle) {
    SengooRawMapKeyIter* iter = (SengooRawMapKeyIter*)sengoo_opaque_handle_get(handle);
    if (!iter) return NULL;
    SengooRawHashMap* map = sengoo_raw_hashmap_from_handle(iter->map_handle);
    if (!map || iter->index >= map->len) return NULL;
    return sengoo_raw_hashmap_key(map, iter->index++);
}

long long sengoo_raw_map_key_iter_done(long long handle) {
    SengooRawMapKeyIter* iter = (SengooRawMapKeyIter*)sengoo_opaque_handle_get(handle);
    if (!iter) return 1;
    SengooRawHashMap* map = sengoo_raw_hashmap_from_handle(iter->map_handle);
    return !map || iter->index >= map->len;
}

long long sengoo_raw_map_key_iter_reset(long long handle) {
    SengooRawMapKeyIter* iter = (SengooRawMapKeyIter*)sengoo_opaque_handle_get(handle);
    if (!iter) return SENGOO_STATUS_INVALID_HANDLE;
    iter->index = 0;
    return SENGOO_STATUS_OK;
}

long long sengoo_raw_map_key_iter_index(long long handle) {
    SengooRawMapKeyIter* iter = (SengooRawMapKeyIter*)sengoo_opaque_handle_get(handle);
    if (!iter) return 0;
    return (long long)iter->index;
}

long long sengoo_raw_map_key_iter_free(long long handle) {
    SengooRawMapKeyIter* iter = (SengooRawMapKeyIter*)sengoo_opaque_handle_take(handle);
    if (!iter) return SENGOO_STATUS_INVALID_HANDLE;
    free(iter);
    return SENGOO_STATUS_OK;
}
extern long long sengoo_string_clone_status(long long handle);
extern long long sengoo_string_from_bytes_copy(long long bytes_ptr, long long len);

long long sengoo_string_map_key_iter_next_string(long long iter_handle) {
    SengooStringMapKeyIter* iter = sengoo_string_map_key_iter_from_handle(iter_handle);
    SengooStringMap* map = iter ? sengoo_string_map_from_handle(iter->map_handle) : NULL;
    if (!iter || !map) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (iter->index >= map->len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    const char* key = map->entries[iter->index].key;
    long long copied = sengoo_string_from_bytes_copy((long long)(uintptr_t)key, (long long)strlen(key));
    if (copied >= 0) {
        iter->index += 1;
        iter->yielded += 1;
    }
    return copied;
}

typedef struct {
    long long key;
    long long value;
} SengooI64BTreeEntry;

typedef struct {
    SengooI64BTreeEntry* entries;
    size_t len;
    size_t cap;
} SengooI64BTreeMap;

typedef struct {
    long long map_handle;
    size_t index;
    size_t yielded;
} SengooI64BTreeKeyIter;

static SengooI64BTreeMap* sengoo_i64_btree_from_handle(long long handle) {
    return (SengooI64BTreeMap*)sengoo_opaque_handle_get(handle);
}

static SengooI64BTreeKeyIter* sengoo_i64_btree_key_iter_from_handle(long long handle) {
    return (SengooI64BTreeKeyIter*)sengoo_opaque_handle_get(handle);
}

static int sengoo_i64_btree_reserve(SengooI64BTreeMap* map, size_t min_cap) {
    if (!map) {
        return 0;
    }
    if (map->cap >= min_cap) {
        return 1;
    }
    size_t next = map->cap == 0 ? 8 : map->cap;
    while (next < min_cap) {
        if (next > SIZE_MAX / 2) {
            return 0;
        }
        next *= 2;
    }
    SengooI64BTreeEntry* entries = (SengooI64BTreeEntry*)realloc(
        map->entries,
        next * sizeof(SengooI64BTreeEntry)
    );
    if (!entries) {
        return 0;
    }
    map->entries = entries;
    map->cap = next;
    return 1;
}

static size_t sengoo_i64_btree_find_index(
    const SengooI64BTreeMap* map,
    long long key,
    int* found
) {
    size_t low = 0;
    size_t high = map ? map->len : 0;
    if (found) {
        *found = 0;
    }

    while (low < high) {
        size_t mid = low + ((high - low) / 2);
        long long candidate = map->entries[mid].key;
        if (candidate == key) {
            if (found) {
                *found = 1;
            }
            return mid;
        }
        if (candidate < key) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    return low;
}

long long sengoo_btreemap_new_i64(void) {
    SengooI64BTreeMap* map = (SengooI64BTreeMap*)calloc(1, sizeof(SengooI64BTreeMap));
    if (!map) {
        return 0;
    }
    long long handle = sengoo_opaque_handle_new(map);
    if (handle == 0) {
        free(map);
    }
    return handle;
}

long long sengoo_btreemap_len_i64(long long handle) {
    SengooI64BTreeMap* map = sengoo_i64_btree_from_handle(handle);
    return map ? (long long)map->len : 0;
}

long long sengoo_btreemap_clear_i64_status(long long handle) {
    SengooI64BTreeMap* map = sengoo_i64_btree_from_handle(handle);
    if (!map) {
        return 0;
    }
    map->len = 0;
    return 1;
}

long long sengoo_btreemap_free_i64_status(long long handle) {
    SengooI64BTreeMap* map = (SengooI64BTreeMap*)sengoo_opaque_handle_take(handle);
    if (!map) {
        return 1;
    }
    free(map->entries);
    free(map);
    return 1;
}

long long sengoo_btreemap_insert_i64(long long handle, long long key, long long value) {
    SengooI64BTreeMap* map = sengoo_i64_btree_from_handle(handle);
    if (!map) {
        return 0;
    }
    int found = 0;
    size_t index = sengoo_i64_btree_find_index(map, key, &found);
    if (found) {
        map->entries[index].value = value;
        return 1;
    }
    if (!sengoo_i64_btree_reserve(map, map->len + 1)) {
        return 0;
    }
    if (index < map->len) {
        memmove(
            &map->entries[index + 1],
            &map->entries[index],
            (map->len - index) * sizeof(SengooI64BTreeEntry)
        );
    }
    map->entries[index].key = key;
    map->entries[index].value = value;
    map->len += 1;
    return 1;
}

long long sengoo_btreemap_contains_i64(long long handle, long long key) {
    SengooI64BTreeMap* map = sengoo_i64_btree_from_handle(handle);
    if (!map) {
        return 0;
    }
    int found = 0;
    sengoo_i64_btree_find_index(map, key, &found);
    return found ? 1 : 0;
}

long long sengoo_btreemap_get_or_default_i64(
    long long handle,
    long long key,
    long long fallback
) {
    SengooI64BTreeMap* map = sengoo_i64_btree_from_handle(handle);
    if (!map) {
        return fallback;
    }
    int found = 0;
    size_t index = sengoo_i64_btree_find_index(map, key, &found);
    return found ? map->entries[index].value : fallback;
}

long long sengoo_btreemap_remove_i64(long long handle, long long key) {
    SengooI64BTreeMap* map = sengoo_i64_btree_from_handle(handle);
    if (!map) {
        return 0;
    }
    int found = 0;
    size_t index = sengoo_i64_btree_find_index(map, key, &found);
    if (!found) {
        return 0;
    }
    if (index + 1 < map->len) {
        memmove(
            &map->entries[index],
            &map->entries[index + 1],
            (map->len - index - 1) * sizeof(SengooI64BTreeEntry)
        );
    }
    map->len -= 1;
    if (map->len < map->cap) {
        memset(&map->entries[map->len], 0, sizeof(SengooI64BTreeEntry));
    }
    return 1;
}

long long sengoo_btreemap_key_iter_new_i64(long long map_handle) {
    if (!sengoo_i64_btree_from_handle(map_handle)) {
        return 0;
    }
    SengooI64BTreeKeyIter* iter =
        (SengooI64BTreeKeyIter*)calloc(1, sizeof(SengooI64BTreeKeyIter));
    if (!iter) {
        return 0;
    }
    iter->map_handle = map_handle;
    long long handle = sengoo_opaque_handle_new(iter);
    if (handle == 0) {
        free(iter);
    }
    return handle;
}

long long sengoo_btreemap_key_iter_done_i64(long long iter_handle) {
    SengooI64BTreeKeyIter* iter = sengoo_i64_btree_key_iter_from_handle(iter_handle);
    SengooI64BTreeMap* map = iter ? sengoo_i64_btree_from_handle(iter->map_handle) : NULL;
    return (!iter || !map || iter->index >= map->len) ? 1 : 0;
}

long long sengoo_btreemap_key_iter_index_i64(long long iter_handle) {
    SengooI64BTreeKeyIter* iter = sengoo_i64_btree_key_iter_from_handle(iter_handle);
    return iter ? (long long)iter->yielded : 0;
}

long long sengoo_btreemap_key_iter_next_or_default_i64(
    long long iter_handle,
    long long fallback
) {
    SengooI64BTreeKeyIter* iter = sengoo_i64_btree_key_iter_from_handle(iter_handle);
    SengooI64BTreeMap* map = iter ? sengoo_i64_btree_from_handle(iter->map_handle) : NULL;
    if (!iter || !map || iter->index >= map->len) {
        return fallback;
    }
    long long key = map->entries[iter->index].key;
    iter->index += 1;
    iter->yielded += 1;
    return key;
}

long long sengoo_btreemap_key_iter_reset_i64_status(long long iter_handle) {
    SengooI64BTreeKeyIter* iter = sengoo_i64_btree_key_iter_from_handle(iter_handle);
    if (!iter) {
        return 0;
    }
    iter->index = 0;
    iter->yielded = 0;
    return 1;
}

long long sengoo_btreemap_key_iter_free_i64_status(long long iter_handle) {
    free(sengoo_opaque_handle_take(iter_handle));
    return 1;
}

typedef struct {
    long long* items;
    size_t len;
    size_t cap;
} SengooStringVec;

typedef struct {
    long long vec_handle;
    size_t index;
} SengooStringVecIter;

static SengooStringVec* sengoo_string_vec_from_handle(long long handle) {
    return (SengooStringVec*)sengoo_opaque_handle_get(handle);
}

static SengooStringVecIter* sengoo_string_vec_iter_from_handle(long long handle) {
    return (SengooStringVecIter*)sengoo_opaque_handle_get(handle);
}

static int sengoo_string_vec_reserve(SengooStringVec* vec, size_t min_cap) {
    if (!vec) {
        return 0;
    }
    if (vec->cap >= min_cap) {
        return 1;
    }
    size_t next = vec->cap == 0 ? 8 : vec->cap;
    while (next < min_cap) {
        if (next > SIZE_MAX / 2) {
            return 0;
        }
        next *= 2;
    }
    long long* items = (long long*)realloc(vec->items, next * sizeof(long long));
    if (!items) {
        return 0;
    }
    vec->items = items;
    vec->cap = next;
    return 1;
}

static void sengoo_string_vec_clear_items(SengooStringVec* vec) {
    if (!vec) {
        return;
    }
    for (size_t i = 0; i < vec->len; ++i) {
        sengoo_string_free_status(vec->items[i]);
        vec->items[i] = 0;
    }
    vec->len = 0;
}

long long sengoo_vec_new_string(void) {
    SengooStringVec* vec = (SengooStringVec*)calloc(1, sizeof(SengooStringVec));
    if (!vec) {
        return 0;
    }
    long long handle = sengoo_opaque_handle_new(vec);
    if (handle == 0) {
        free(vec);
    }
    return handle;
}

long long sengoo_vec_string_len(long long handle) {
    SengooStringVec* vec = sengoo_string_vec_from_handle(handle);
    return vec ? (long long)vec->len : 0;
}

long long sengoo_vec_string_clear_status(long long handle) {
    SengooStringVec* vec = sengoo_string_vec_from_handle(handle);
    if (!vec) {
        return 0;
    }
    sengoo_string_vec_clear_items(vec);
    return 1;
}

long long sengoo_vec_string_free_status(long long handle) {
    SengooStringVec* vec = (SengooStringVec*)sengoo_opaque_handle_take(handle);
    if (!vec) {
        return 1;
    }
    sengoo_string_vec_clear_items(vec);
    free(vec->items);
    free(vec);
    return 1;
}

long long sengoo_vec_string_push(long long handle, long long value_handle) {
    SengooStringVec* vec = sengoo_string_vec_from_handle(handle);
    if (!vec || value_handle <= 0) {
        return 0;
    }
    if (!sengoo_string_vec_reserve(vec, vec->len + 1)) {
        return 0;
    }
    vec->items[vec->len++] = value_handle;
    return 1;
}

long long sengoo_vec_string_get_clone(long long handle, long long index) {
    SengooStringVec* vec = sengoo_string_vec_from_handle(handle);
    if (!vec) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (index < 0 || (size_t)index >= vec->len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    return sengoo_string_clone_status(vec->items[(size_t)index]);
}

long long sengoo_vec_string_set(long long handle, long long index, long long value_handle) {
    SengooStringVec* vec = sengoo_string_vec_from_handle(handle);
    if (!vec) {
        return 0;
    }
    if (index < 0 || (size_t)index >= vec->len || value_handle <= 0) {
        return 0;
    }
    size_t idx = (size_t)index;
    sengoo_string_free_status(vec->items[idx]);
    vec->items[idx] = value_handle;
    return 1;
}

long long sengoo_vec_string_insert(long long handle, long long index, long long value_handle) {
    SengooStringVec* vec = sengoo_string_vec_from_handle(handle);
    if (!vec) {
        return 0;
    }
    if (index < 0 || (size_t)index > vec->len || value_handle <= 0) {
        return 0;
    }
    if (!sengoo_string_vec_reserve(vec, vec->len + 1)) {
        return 0;
    }
    size_t idx = (size_t)index;
    for (size_t i = vec->len; i > idx; --i) {
        vec->items[i] = vec->items[i - 1];
    }
    vec->items[idx] = value_handle;
    vec->len += 1;
    return 1;
}

long long sengoo_vec_string_remove_transfer(long long handle, long long index) {
    SengooStringVec* vec = sengoo_string_vec_from_handle(handle);
    if (!vec) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (index < 0 || (size_t)index >= vec->len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    size_t idx = (size_t)index;
    long long value = vec->items[idx];
    for (size_t i = idx + 1; i < vec->len; ++i) {
        vec->items[i - 1] = vec->items[i];
    }
    vec->len -= 1;
    return value;
}

long long sengoo_vec_string_iter_new(long long vec_handle) {
    SengooStringVec* vec = sengoo_string_vec_from_handle(vec_handle);
    if (!vec) {
        return 0;
    }
    SengooStringVecIter* iter = (SengooStringVecIter*)calloc(1, sizeof(SengooStringVecIter));
    if (!iter) {
        return 0;
    }
    iter->vec_handle = vec_handle;
    iter->index = 0;
    long long handle = sengoo_opaque_handle_new(iter);
    if (handle == 0) {
        free(iter);
    }
    return handle;
}

long long sengoo_vec_string_iter_done(long long iter_handle) {
    SengooStringVecIter* iter = sengoo_string_vec_iter_from_handle(iter_handle);
    SengooStringVec* vec = iter ? sengoo_string_vec_from_handle(iter->vec_handle) : NULL;
    return (!iter || !vec || iter->index >= vec->len) ? 1 : 0;
}

long long sengoo_vec_string_iter_next_clone(long long iter_handle) {
    SengooStringVecIter* iter = sengoo_string_vec_iter_from_handle(iter_handle);
    SengooStringVec* vec = iter ? sengoo_string_vec_from_handle(iter->vec_handle) : NULL;
    if (!iter || !vec) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (iter->index >= vec->len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    long long cloned = sengoo_string_clone_status(vec->items[iter->index]);
    if (cloned >= 0) {
        iter->index += 1;
    }
    return cloned;
}

long long sengoo_vec_string_iter_collect(long long iter_handle) {
    SengooStringVecIter* iter = sengoo_string_vec_iter_from_handle(iter_handle);
    SengooStringVec* source = iter ? sengoo_string_vec_from_handle(iter->vec_handle) : NULL;
    if (!iter || !source) {
        return 0;
    }
    long long collected_handle = sengoo_vec_new_string();
    SengooStringVec* collected = sengoo_string_vec_from_handle(collected_handle);
    if (!collected) {
        return 0;
    }
    while (iter->index < source->len) {
        long long cloned = sengoo_string_clone_status(source->items[iter->index]);
        if (cloned <= 0) {
            sengoo_vec_string_free_status(collected_handle);
            return 0;
        }
        if (!sengoo_string_vec_reserve(collected, collected->len + 1)) {
            sengoo_string_free_status(cloned);
            sengoo_vec_string_free_status(collected_handle);
            return 0;
        }
        collected->items[collected->len++] = cloned;
        iter->index += 1;
    }
    return collected_handle;
}

long long sengoo_vec_string_iter_reset_status(long long iter_handle) {
    SengooStringVecIter* iter = sengoo_string_vec_iter_from_handle(iter_handle);
    if (!iter) {
        return 0;
    }
    iter->index = 0;
    return 1;
}

long long sengoo_vec_string_iter_free_status(long long iter_handle) {
    free(sengoo_opaque_handle_take(iter_handle));
    return 1;
}

typedef struct {
    char* key;
    long long value_handle;
} SengooStringMapStringEntry;

typedef struct {
    SengooStringMapStringEntry* entries;
    size_t len;
    size_t cap;
} SengooStringMapString;

typedef struct {
    long long map_handle;
    size_t index;
} SengooStringMapStringKeyIter;

static SengooStringMapString* sengoo_string_map_string_from_handle(long long handle) {
    return (SengooStringMapString*)sengoo_opaque_handle_get(handle);
}

static SengooStringMapStringKeyIter* sengoo_string_map_string_key_iter_from_handle(long long handle) {
    return (SengooStringMapStringKeyIter*)sengoo_opaque_handle_get(handle);
}

static int sengoo_string_map_string_reserve(SengooStringMapString* map, size_t min_cap) {
    if (!map) {
        return 0;
    }
    if (map->cap >= min_cap) {
        return 1;
    }
    size_t next = map->cap == 0 ? 8 : map->cap;
    while (next < min_cap) {
        if (next > SIZE_MAX / 2) {
            return 0;
        }
        next *= 2;
    }
    SengooStringMapStringEntry* entries = (SengooStringMapStringEntry*)realloc(
        map->entries,
        next * sizeof(SengooStringMapStringEntry));
    if (!entries) {
        return 0;
    }
    map->entries = entries;
    map->cap = next;
    return 1;
}

static size_t sengoo_string_map_string_find_index(SengooStringMapString* map, const char* key, int* found) {
    size_t low = 0;
    size_t high = map ? map->len : 0;
    if (found) {
        *found = 0;
    }
    while (low < high) {
        size_t mid = low + ((high - low) / 2);
        int cmp = strcmp(map->entries[mid].key, key);
        if (cmp == 0) {
            if (found) {
                *found = 1;
            }
            return mid;
        }
        if (cmp < 0) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    return low;
}

static void sengoo_string_map_string_clear_entries(SengooStringMapString* map) {
    if (!map) {
        return;
    }
    for (size_t i = 0; i < map->len; ++i) {
        free(map->entries[i].key);
        sengoo_string_free_status(map->entries[i].value_handle);
        map->entries[i].key = NULL;
        map->entries[i].value_handle = 0;
    }
    map->len = 0;
}

long long sengoo_string_map_string_new(void) {
    SengooStringMapString* map = (SengooStringMapString*)calloc(1, sizeof(SengooStringMapString));
    if (!map) {
        return 0;
    }
    long long handle = sengoo_opaque_handle_new(map);
    if (handle == 0) {
        free(map);
    }
    return handle;
}

long long sengoo_string_map_string_len(long long handle) {
    SengooStringMapString* map = sengoo_string_map_string_from_handle(handle);
    return map ? (long long)map->len : 0;
}

long long sengoo_string_map_string_clear_status(long long handle) {
    SengooStringMapString* map = sengoo_string_map_string_from_handle(handle);
    if (!map) {
        return 0;
    }
    sengoo_string_map_string_clear_entries(map);
    return 1;
}

long long sengoo_string_map_string_free_status(long long handle) {
    SengooStringMapString* map = (SengooStringMapString*)sengoo_opaque_handle_take(handle);
    if (!map) {
        return 1;
    }
    sengoo_string_map_string_clear_entries(map);
    free(map->entries);
    free(map);
    return 1;
}

long long sengoo_string_map_string_insert(long long handle, long long key_ptr, long long value_handle) {
    SengooStringMapString* map = sengoo_string_map_string_from_handle(handle);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!map || !key || value_handle <= 0) {
        return 0;
    }
    int found = 0;
    size_t index = sengoo_string_map_string_find_index(map, key, &found);
    if (found) {
        sengoo_string_free_status(map->entries[index].value_handle);
        map->entries[index].value_handle = value_handle;
        return 1;
    }
    char* key_copy = sengoo_copy_cstr_from_handle(key_ptr);
    if (!key_copy || !sengoo_string_map_string_reserve(map, map->len + 1)) {
        free(key_copy);
        return 0;
    }
    for (size_t i = map->len; i > index; --i) {
        map->entries[i] = map->entries[i - 1];
    }
    map->entries[index].key = key_copy;
    map->entries[index].value_handle = value_handle;
    map->len += 1;
    return 1;
}

long long sengoo_string_map_string_contains(long long handle, long long key_ptr) {
    SengooStringMapString* map = sengoo_string_map_string_from_handle(handle);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!map || !key) {
        return 0;
    }
    int found = 0;
    sengoo_string_map_string_find_index(map, key, &found);
    return found ? 1 : 0;
}

long long sengoo_string_map_string_get_clone(long long handle, long long key_ptr) {
    SengooStringMapString* map = sengoo_string_map_string_from_handle(handle);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!map) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (!key) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    int found = 0;
    size_t index = sengoo_string_map_string_find_index(map, key, &found);
    if (!found) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    return sengoo_string_clone_status(map->entries[index].value_handle);
}

long long sengoo_string_map_string_remove_transfer(long long handle, long long key_ptr) {
    SengooStringMapString* map = sengoo_string_map_string_from_handle(handle);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!map) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (!key) {
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
    int found = 0;
    size_t index = sengoo_string_map_string_find_index(map, key, &found);
    if (!found) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    long long value = map->entries[index].value_handle;
    free(map->entries[index].key);
    for (size_t i = index + 1; i < map->len; ++i) {
        map->entries[i - 1] = map->entries[i];
    }
    map->len -= 1;
    return value;
}

long long sengoo_string_map_string_key_iter_new(long long map_handle) {
    SengooStringMapString* map = sengoo_string_map_string_from_handle(map_handle);
    if (!map) {
        return 0;
    }
    SengooStringMapStringKeyIter* iter = (SengooStringMapStringKeyIter*)calloc(1, sizeof(SengooStringMapStringKeyIter));
    if (!iter) {
        return 0;
    }
    iter->map_handle = map_handle;
    iter->index = 0;
    long long handle = sengoo_opaque_handle_new(iter);
    if (handle == 0) {
        free(iter);
    }
    return handle;
}

long long sengoo_string_map_string_key_iter_done(long long iter_handle) {
    SengooStringMapStringKeyIter* iter = sengoo_string_map_string_key_iter_from_handle(iter_handle);
    SengooStringMapString* map = iter ? sengoo_string_map_string_from_handle(iter->map_handle) : NULL;
    return (!iter || !map || iter->index >= map->len) ? 1 : 0;
}

long long sengoo_string_map_string_key_iter_next_string(long long iter_handle) {
    SengooStringMapStringKeyIter* iter = sengoo_string_map_string_key_iter_from_handle(iter_handle);
    SengooStringMapString* map = iter ? sengoo_string_map_string_from_handle(iter->map_handle) : NULL;
    if (!iter || !map) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (iter->index >= map->len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    const char* key = map->entries[iter->index].key;
    long long cloned = sengoo_string_from_bytes_copy((long long)(intptr_t)key, (long long)strlen(key));
    if (cloned >= 0) {
        iter->index += 1;
    }
    return cloned;
}

long long sengoo_string_map_string_key_iter_reset_status(long long iter_handle) {
    SengooStringMapStringKeyIter* iter = sengoo_string_map_string_key_iter_from_handle(iter_handle);
    if (!iter) {
        return 0;
    }
    iter->index = 0;
    return 1;
}

long long sengoo_string_map_string_key_iter_free_status(long long iter_handle) {
    free(sengoo_opaque_handle_take(iter_handle));
    return 1;
}
