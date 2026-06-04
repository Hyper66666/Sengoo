#include "runtime_shared.h"

#include <stdint.h>
#include <stdlib.h>
#include <string.h>

typedef struct {
    char** items;
    size_t len;
    size_t cap;
} SengooTextList;

typedef struct {
    SengooTextList* list;
    size_t index;
} SengooTextListIter;

static SengooTextList* sengoo_text_list_from_handle(long long handle) {
    return (SengooTextList*)sengoo_handle_to_ptr(handle);
}

static SengooTextListIter* sengoo_text_list_iter_from_handle(long long handle) {
    return (SengooTextListIter*)sengoo_handle_to_ptr(handle);
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
    return sengoo_ptr_to_handle(list);
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
    SengooTextList* list = sengoo_text_list_from_handle(handle);
    if (!list) {
        return 1;
    }
    sengoo_text_list_clear_status(handle);
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
    iter->list = list;
    iter->index = 0;
    return sengoo_ptr_to_handle(iter);
}

long long sengoo_text_list_iter_done(long long iter_handle) {
    SengooTextListIter* iter = sengoo_text_list_iter_from_handle(iter_handle);
    return (!iter || !iter->list || iter->index >= iter->list->len) ? 1 : 0;
}

long long sengoo_text_list_iter_next_copy(long long iter_handle, long long out_buffer) {
    SengooTextListIter* iter = sengoo_text_list_iter_from_handle(iter_handle);
    if (!iter || !iter->list) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (iter->index >= iter->list->len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    const char* value = iter->list->items[iter->index];
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
    SengooTextListIter* iter = sengoo_text_list_iter_from_handle(iter_handle);
    if (iter) {
        free(iter);
    }
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
    SengooStringMap* map;
    size_t index;
} SengooStringMapKeyIter;

static SengooStringMap* sengoo_string_map_from_handle(long long handle) {
    return (SengooStringMap*)sengoo_handle_to_ptr(handle);
}

static SengooStringMapKeyIter* sengoo_string_map_key_iter_from_handle(long long handle) {
    return (SengooStringMapKeyIter*)sengoo_handle_to_ptr(handle);
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
    return sengoo_ptr_to_handle(map);
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
    SengooStringMap* map = sengoo_string_map_from_handle(handle);
    if (!map) {
        return 1;
    }
    sengoo_string_map_clear_status(handle);
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
    iter->map = map;
    iter->index = 0;
    return sengoo_ptr_to_handle(iter);
}

long long sengoo_string_map_key_iter_done(long long iter_handle) {
    SengooStringMapKeyIter* iter = sengoo_string_map_key_iter_from_handle(iter_handle);
    return (!iter || !iter->map || iter->index >= iter->map->len) ? 1 : 0;
}

long long sengoo_string_map_key_iter_next_copy(long long iter_handle, long long out_buffer) {
    SengooStringMapKeyIter* iter = sengoo_string_map_key_iter_from_handle(iter_handle);
    if (!iter || !iter->map) {
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (iter->index >= iter->map->len) {
        return -SENGOO_STATUS_NOT_FOUND;
    }
    const char* key = iter->map->entries[iter->index].key;
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
    SengooStringMapKeyIter* iter = sengoo_string_map_key_iter_from_handle(iter_handle);
    if (iter) {
        free(iter);
    }
    return 1;
}
