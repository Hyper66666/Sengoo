#include "runtime_shared.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

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
    }
    return copied;
}

long long sengoo_string_map_key_iter_reset_status(long long iter_handle) {
    SengooStringMapKeyIter* iter = sengoo_string_map_key_iter_from_handle(iter_handle);
    if (!iter) {
        return 0;
    }
    iter->index = 0;
    return 1;
}

long long sengoo_string_map_key_iter_free_status(long long iter_handle) {
    free(sengoo_opaque_handle_take(iter_handle));
    return 1;
}

extern long long sengoo_string_free_status(long long handle);
extern long long sengoo_string_clone_status(long long handle);
extern long long sengoo_string_from_bytes_copy(long long bytes_ptr, long long len);

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
