#define _CRT_SECURE_NO_WARNINGS

#include "runtime_shared.h"

extern long long sengoo_string_from_bytes_copy(long long bytes_ptr, long long len);

#include <ctype.h>
#include <errno.h>
#include <limits.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SENGOO_JSON_MAX_BYTES SENGOO_RUNTIME_MAX_JSON_BYTES
#define SENGOO_JSON_MAX_DEPTH 64
#define SENGOO_JSON_MAX_NODES 4096

enum {
    SENGOO_JSON_KIND_NULL = 0,
    SENGOO_JSON_KIND_BOOL = 1,
    SENGOO_JSON_KIND_NUMBER = 2,
    SENGOO_JSON_KIND_STRING = 3,
    SENGOO_JSON_KIND_ARRAY = 4,
    SENGOO_JSON_KIND_OBJECT = 5
};

typedef struct {
    char* key;
    long long value_node;
} SengooJsonMember;

typedef struct {
    int kind;
    char* string_value;
    char* number_text;
    double number_f64;
    long long number_i64;
    int number_has_i64;
    int bool_value;
    long long* array_items;
    size_t array_len;
    size_t array_cap;
    SengooJsonMember* members;
    size_t member_len;
    size_t member_cap;
} SengooJsonNode;

typedef struct {
    SengooJsonNode* nodes;
    size_t len;
    size_t cap;
    long long root;
    int closed;
} SengooJsonDoc;

typedef struct {
    const char* data;
    size_t len;
    size_t pos;
    int depth;
    SengooJsonDoc* doc;
} SengooJsonParser;

typedef struct {
    char* data;
    size_t len;
    size_t cap;
} SengooJsonBuilder;

static int sengoo_json_last_error = SENGOO_STATUS_OK;
static long long sengoo_json_last_offset = -1;
static char sengoo_json_last_message[256] = {0};

static void sengoo_json_clear_error(void) {
    sengoo_json_last_error = SENGOO_STATUS_OK;
    sengoo_json_last_offset = -1;
    sengoo_json_last_message[0] = '\0';
}

static long long sengoo_json_set_error(long long code, long long offset, const char* message) {
    sengoo_json_last_error = (int)code;
    sengoo_json_last_offset = offset;
    snprintf(
        sengoo_json_last_message,
        sizeof(sengoo_json_last_message),
        "%s",
        message ? message : "json error"
    );
    return -code;
}

static SengooJsonDoc* sengoo_json_doc_from_handle(long long handle) {
    SengooJsonDoc* doc = (SengooJsonDoc*)sengoo_handle_to_ptr(handle);
    return doc && !doc->closed ? doc : NULL;
}

static SengooJsonNode* sengoo_json_node_from_id(SengooJsonDoc* doc, long long node_id) {
    if (!doc || node_id <= 0 || (size_t)node_id > doc->len) {
        return NULL;
    }
    return &doc->nodes[(size_t)node_id - 1];
}

static void sengoo_json_node_free(SengooJsonNode* node) {
    if (!node) {
        return;
    }
    free(node->string_value);
    free(node->number_text);
    free(node->array_items);
    for (size_t i = 0; i < node->member_len; ++i) {
        free(node->members[i].key);
    }
    free(node->members);
    memset(node, 0, sizeof(*node));
}

static void sengoo_json_doc_free(SengooJsonDoc* doc) {
    if (!doc) {
        return;
    }
    for (size_t i = 0; i < doc->len; ++i) {
        sengoo_json_node_free(&doc->nodes[i]);
    }
    free(doc->nodes);
    doc->nodes = NULL;
    doc->len = 0;
    doc->cap = 0;
    doc->root = 0;
    doc->closed = 1;
}

static void sengoo_json_doc_destroy(SengooJsonDoc* doc) {
    if (!doc) {
        return;
    }
    if (!doc->closed) {
        sengoo_json_doc_free(doc);
    }
    free(doc);
}

static int sengoo_json_doc_reserve(SengooJsonDoc* doc, size_t min_cap) {
    if (!doc) {
        return 0;
    }
    if (doc->cap >= min_cap) {
        return 1;
    }
    size_t next = doc->cap == 0 ? 16 : doc->cap;
    while (next < min_cap) {
        if (next > SIZE_MAX / 2) {
            return 0;
        }
        next *= 2;
    }
    SengooJsonNode* nodes = (SengooJsonNode*)realloc(doc->nodes, next * sizeof(SengooJsonNode));
    if (!nodes) {
        return 0;
    }
    memset(nodes + doc->cap, 0, (next - doc->cap) * sizeof(SengooJsonNode));
    doc->nodes = nodes;
    doc->cap = next;
    return 1;
}

static long long sengoo_json_doc_add_node(SengooJsonDoc* doc, int kind) {
    if (!doc) {
        return 0;
    }
    if (doc->len >= SENGOO_JSON_MAX_NODES) {
        sengoo_json_set_error(SENGOO_STATUS_PARSE, -1, "json node limit exceeded");
        return 0;
    }
    if (!sengoo_json_doc_reserve(doc, doc->len + 1)) {
        sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, -1, "json node allocation failed");
        return 0;
    }
    SengooJsonNode* node = &doc->nodes[doc->len];
    memset(node, 0, sizeof(*node));
    node->kind = kind;
    doc->len += 1;
    return (long long)doc->len;
}

static SengooJsonDoc* sengoo_json_doc_new_with_root(int root_kind) {
    SengooJsonDoc* doc = (SengooJsonDoc*)calloc(1, sizeof(SengooJsonDoc));
    if (!doc) {
        sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, -1, "json document allocation failed");
        return NULL;
    }
    long long root = sengoo_json_doc_add_node(doc, root_kind);
    if (root == 0) {
        sengoo_json_doc_destroy(doc);
        return NULL;
    }
    doc->root = root;
    return doc;
}

static int sengoo_json_builder_reserve(SengooJsonBuilder* builder, size_t extra) {
    if (!builder) {
        return 0;
    }
    if (extra > SIZE_MAX - builder->len - 1) {
        return 0;
    }
    size_t needed = builder->len + extra + 1;
    if (builder->cap >= needed) {
        return 1;
    }
    size_t next = builder->cap == 0 ? 64 : builder->cap;
    while (next < needed) {
        if (next > SIZE_MAX / 2) {
            return 0;
        }
        next *= 2;
    }
    char* data = (char*)realloc(builder->data, next);
    if (!data) {
        return 0;
    }
    builder->data = data;
    builder->cap = next;
    return 1;
}

static int sengoo_json_builder_append_bytes(SengooJsonBuilder* builder, const char* bytes, size_t len) {
    if (len > 0 && !bytes) {
        return 0;
    }
    if (!sengoo_json_builder_reserve(builder, len)) {
        return 0;
    }
    if (len > 0) {
        memcpy(builder->data + builder->len, bytes, len);
    }
    builder->len += len;
    builder->data[builder->len] = '\0';
    return 1;
}

static int sengoo_json_builder_append_cstr(SengooJsonBuilder* builder, const char* value) {
    return sengoo_json_builder_append_bytes(builder, value, strlen(value));
}

static int sengoo_json_builder_append_char(SengooJsonBuilder* builder, char value) {
    return sengoo_json_builder_append_bytes(builder, &value, 1);
}

static int sengoo_json_builder_append_escaped_string(SengooJsonBuilder* builder, const char* value) {
    if (!sengoo_json_builder_append_char(builder, '"')) {
        return 0;
    }
    const unsigned char* cursor = (const unsigned char*)value;
    while (cursor && *cursor) {
        unsigned char c = *cursor++;
        switch (c) {
            case '"':
                if (!sengoo_json_builder_append_cstr(builder, "\\\"")) return 0;
                break;
            case '\\':
                if (!sengoo_json_builder_append_cstr(builder, "\\\\")) return 0;
                break;
            case '\b':
                if (!sengoo_json_builder_append_cstr(builder, "\\b")) return 0;
                break;
            case '\f':
                if (!sengoo_json_builder_append_cstr(builder, "\\f")) return 0;
                break;
            case '\n':
                if (!sengoo_json_builder_append_cstr(builder, "\\n")) return 0;
                break;
            case '\r':
                if (!sengoo_json_builder_append_cstr(builder, "\\r")) return 0;
                break;
            case '\t':
                if (!sengoo_json_builder_append_cstr(builder, "\\t")) return 0;
                break;
            default:
                if (c < 0x20) {
                    char escaped[7];
                    snprintf(escaped, sizeof(escaped), "\\u%04x", c);
                    if (!sengoo_json_builder_append_cstr(builder, escaped)) return 0;
                } else if (!sengoo_json_builder_append_char(builder, (char)c)) {
                    return 0;
                }
                break;
        }
    }
    return sengoo_json_builder_append_char(builder, '"');
}

static int sengoo_json_array_reserve(SengooJsonNode* node, size_t min_cap) {
    if (!node || node->kind != SENGOO_JSON_KIND_ARRAY) {
        return 0;
    }
    if (node->array_cap >= min_cap) {
        return 1;
    }
    size_t next = node->array_cap == 0 ? 8 : node->array_cap;
    while (next < min_cap) {
        if (next > SIZE_MAX / 2) {
            return 0;
        }
        next *= 2;
    }
    long long* items = (long long*)realloc(node->array_items, next * sizeof(long long));
    if (!items) {
        return 0;
    }
    node->array_items = items;
    node->array_cap = next;
    return 1;
}

static int sengoo_json_object_reserve(SengooJsonNode* node, size_t min_cap) {
    if (!node || node->kind != SENGOO_JSON_KIND_OBJECT) {
        return 0;
    }
    if (node->member_cap >= min_cap) {
        return 1;
    }
    size_t next = node->member_cap == 0 ? 8 : node->member_cap;
    while (next < min_cap) {
        if (next > SIZE_MAX / 2) {
            return 0;
        }
        next *= 2;
    }
    SengooJsonMember* members = (SengooJsonMember*)realloc(node->members, next * sizeof(SengooJsonMember));
    if (!members) {
        return 0;
    }
    node->members = members;
    node->member_cap = next;
    return 1;
}

static int sengoo_json_object_set_member(SengooJsonNode* object, const char* key, long long value_node) {
    if (!object || object->kind != SENGOO_JSON_KIND_OBJECT || !key || value_node <= 0) {
        return 0;
    }
    for (size_t i = 0; i < object->member_len; ++i) {
        if (strcmp(object->members[i].key, key) == 0) {
            object->members[i].value_node = value_node;
            return 1;
        }
    }
    char* key_copy = sengoo_copy_cstr_from_handle((long long)(intptr_t)key);
    if (!key_copy || !sengoo_json_object_reserve(object, object->member_len + 1)) {
        free(key_copy);
        return 0;
    }
    object->members[object->member_len].key = key_copy;
    object->members[object->member_len].value_node = value_node;
    object->member_len += 1;
    return 1;
}

static void sengoo_json_skip_ws(SengooJsonParser* parser) {
    while (parser->pos < parser->len) {
        char c = parser->data[parser->pos];
        if (c != ' ' && c != '\n' && c != '\r' && c != '\t') {
            break;
        }
        parser->pos += 1;
    }
}

static int sengoo_json_hex_value(char c) {
    if (c >= '0' && c <= '9') return c - '0';
    if (c >= 'a' && c <= 'f') return c - 'a' + 10;
    if (c >= 'A' && c <= 'F') return c - 'A' + 10;
    return -1;
}

static char* sengoo_json_parse_string_raw(SengooJsonParser* parser) {
    if (parser->pos >= parser->len || parser->data[parser->pos] != '"') {
        sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "expected json string");
        return NULL;
    }
    parser->pos += 1;

    SengooJsonBuilder builder = {0};
    while (parser->pos < parser->len) {
        unsigned char c = (unsigned char)parser->data[parser->pos++];
        if (c == '"') {
            if (!sengoo_json_builder_reserve(&builder, 0)) {
                free(builder.data);
                return NULL;
            }
            return builder.data ? builder.data : sengoo_copy_cstr_from_handle((long long)(intptr_t)"");
        }
        if (c < 0x20) {
            free(builder.data);
            sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)(parser->pos - 1), "control character in json string");
            return NULL;
        }
        if (c != '\\') {
            if (!sengoo_json_builder_append_char(&builder, (char)c)) {
                free(builder.data);
                sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, (long long)parser->pos, "json string allocation failed");
                return NULL;
            }
            continue;
        }
        if (parser->pos >= parser->len) {
            free(builder.data);
            sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "unterminated json escape");
            return NULL;
        }
        char escaped = parser->data[parser->pos++];
        char decoded = 0;
        switch (escaped) {
            case '"': decoded = '"'; break;
            case '\\': decoded = '\\'; break;
            case '/': decoded = '/'; break;
            case 'b': decoded = '\b'; break;
            case 'f': decoded = '\f'; break;
            case 'n': decoded = '\n'; break;
            case 'r': decoded = '\r'; break;
            case 't': decoded = '\t'; break;
            case 'u': {
                if (parser->pos + 4 > parser->len) {
                    free(builder.data);
                    sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "short unicode escape");
                    return NULL;
                }
                int code = 0;
                for (int i = 0; i < 4; ++i) {
                    int nibble = sengoo_json_hex_value(parser->data[parser->pos + (size_t)i]);
                    if (nibble < 0) {
                        free(builder.data);
                        sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "invalid unicode escape");
                        return NULL;
                    }
                    code = (code << 4) | nibble;
                }
                parser->pos += 4;
                decoded = (code >= 0x20 && code <= 0x7f) ? (char)code : '?';
                break;
            }
            default:
                free(builder.data);
                sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)(parser->pos - 1), "invalid json escape");
                return NULL;
        }
        if (!sengoo_json_builder_append_char(&builder, decoded)) {
            free(builder.data);
            sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, (long long)parser->pos, "json string allocation failed");
            return NULL;
        }
    }
    free(builder.data);
    sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "unterminated json string");
    return NULL;
}

static int sengoo_json_match_literal(SengooJsonParser* parser, const char* literal) {
    size_t len = strlen(literal);
    if (parser->pos + len > parser->len || memcmp(parser->data + parser->pos, literal, len) != 0) {
        return 0;
    }
    parser->pos += len;
    return 1;
}

static long long sengoo_json_parse_value(SengooJsonParser* parser);

static long long sengoo_json_parse_number(SengooJsonParser* parser) {
    size_t start = parser->pos;
    if (parser->pos < parser->len && parser->data[parser->pos] == '-') {
        parser->pos += 1;
    }
    if (parser->pos >= parser->len || !isdigit((unsigned char)parser->data[parser->pos])) {
        sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "expected json number");
        return 0;
    }
    if (parser->data[parser->pos] == '0') {
        parser->pos += 1;
    } else {
        while (parser->pos < parser->len && isdigit((unsigned char)parser->data[parser->pos])) {
            parser->pos += 1;
        }
    }
    int integral = 1;
    if (parser->pos < parser->len && parser->data[parser->pos] == '.') {
        integral = 0;
        parser->pos += 1;
        if (parser->pos >= parser->len || !isdigit((unsigned char)parser->data[parser->pos])) {
            sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "expected json fraction digit");
            return 0;
        }
        while (parser->pos < parser->len && isdigit((unsigned char)parser->data[parser->pos])) {
            parser->pos += 1;
        }
    }
    if (parser->pos < parser->len && (parser->data[parser->pos] == 'e' || parser->data[parser->pos] == 'E')) {
        integral = 0;
        parser->pos += 1;
        if (parser->pos < parser->len && (parser->data[parser->pos] == '+' || parser->data[parser->pos] == '-')) {
            parser->pos += 1;
        }
        if (parser->pos >= parser->len || !isdigit((unsigned char)parser->data[parser->pos])) {
            sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "expected json exponent digit");
            return 0;
        }
        while (parser->pos < parser->len && isdigit((unsigned char)parser->data[parser->pos])) {
            parser->pos += 1;
        }
    }

    size_t len = parser->pos - start;
    char* text = (char*)malloc(len + 1);
    if (!text) {
        sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, (long long)start, "json number allocation failed");
        return 0;
    }
    memcpy(text, parser->data + start, len);
    text[len] = '\0';

    long long node_id = sengoo_json_doc_add_node(parser->doc, SENGOO_JSON_KIND_NUMBER);
    if (node_id == 0) {
        free(text);
        return 0;
    }
    SengooJsonNode* node = sengoo_json_node_from_id(parser->doc, node_id);
    node->number_text = text;
    node->number_f64 = strtod(text, NULL);
    if (integral) {
        errno = 0;
        char* end = NULL;
        long long value = strtoll(text, &end, 10);
        if (errno == 0 && end && *end == '\0') {
            node->number_i64 = value;
            node->number_has_i64 = 1;
        }
    }
    return node_id;
}

static long long sengoo_json_parse_array(SengooJsonParser* parser) {
    parser->pos += 1;
    long long node_id = sengoo_json_doc_add_node(parser->doc, SENGOO_JSON_KIND_ARRAY);
    if (node_id == 0) {
        return 0;
    }
    SengooJsonNode* array = sengoo_json_node_from_id(parser->doc, node_id);
    sengoo_json_skip_ws(parser);
    if (parser->pos < parser->len && parser->data[parser->pos] == ']') {
        parser->pos += 1;
        return node_id;
    }
    while (parser->pos < parser->len) {
        long long child = sengoo_json_parse_value(parser);
        if (child == 0) {
            return 0;
        }
        if (!sengoo_json_array_reserve(array, array->array_len + 1)) {
            sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, (long long)parser->pos, "json array allocation failed");
            return 0;
        }
        array->array_items[array->array_len++] = child;
        sengoo_json_skip_ws(parser);
        if (parser->pos < parser->len && parser->data[parser->pos] == ',') {
            parser->pos += 1;
            sengoo_json_skip_ws(parser);
            continue;
        }
        if (parser->pos < parser->len && parser->data[parser->pos] == ']') {
            parser->pos += 1;
            return node_id;
        }
        sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "expected comma or array end");
        return 0;
    }
    sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "unterminated json array");
    return 0;
}

static long long sengoo_json_parse_object(SengooJsonParser* parser) {
    parser->pos += 1;
    long long node_id = sengoo_json_doc_add_node(parser->doc, SENGOO_JSON_KIND_OBJECT);
    if (node_id == 0) {
        return 0;
    }
    SengooJsonNode* object = sengoo_json_node_from_id(parser->doc, node_id);
    sengoo_json_skip_ws(parser);
    if (parser->pos < parser->len && parser->data[parser->pos] == '}') {
        parser->pos += 1;
        return node_id;
    }
    while (parser->pos < parser->len) {
        char* key = sengoo_json_parse_string_raw(parser);
        if (!key) {
            return 0;
        }
        sengoo_json_skip_ws(parser);
        if (parser->pos >= parser->len || parser->data[parser->pos] != ':') {
            free(key);
            sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "expected object colon");
            return 0;
        }
        parser->pos += 1;
        long long child = sengoo_json_parse_value(parser);
        if (child == 0) {
            free(key);
            return 0;
        }
        if (!sengoo_json_object_set_member(object, key, child)) {
            free(key);
            sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, (long long)parser->pos, "json object allocation failed");
            return 0;
        }
        free(key);
        sengoo_json_skip_ws(parser);
        if (parser->pos < parser->len && parser->data[parser->pos] == ',') {
            parser->pos += 1;
            sengoo_json_skip_ws(parser);
            continue;
        }
        if (parser->pos < parser->len && parser->data[parser->pos] == '}') {
            parser->pos += 1;
            return node_id;
        }
        sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "expected comma or object end");
        return 0;
    }
    sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "unterminated json object");
    return 0;
}

static long long sengoo_json_parse_value(SengooJsonParser* parser) {
    sengoo_json_skip_ws(parser);
    if (parser->depth >= SENGOO_JSON_MAX_DEPTH) {
        sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "json depth limit exceeded");
        return 0;
    }
    if (parser->pos >= parser->len) {
        sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "expected json value");
        return 0;
    }

    parser->depth += 1;
    char c = parser->data[parser->pos];
    long long node_id = 0;
    if (c == '"') {
        char* value = sengoo_json_parse_string_raw(parser);
        if (value) {
            node_id = sengoo_json_doc_add_node(parser->doc, SENGOO_JSON_KIND_STRING);
            SengooJsonNode* node = sengoo_json_node_from_id(parser->doc, node_id);
            if (node) {
                node->string_value = value;
            } else {
                free(value);
            }
        }
    } else if (c == '{') {
        node_id = sengoo_json_parse_object(parser);
    } else if (c == '[') {
        node_id = sengoo_json_parse_array(parser);
    } else if (c == 't') {
        if (sengoo_json_match_literal(parser, "true")) {
            node_id = sengoo_json_doc_add_node(parser->doc, SENGOO_JSON_KIND_BOOL);
            SengooJsonNode* node = sengoo_json_node_from_id(parser->doc, node_id);
            if (node) node->bool_value = 1;
        } else {
            sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "invalid literal");
        }
    } else if (c == 'f') {
        if (sengoo_json_match_literal(parser, "false")) {
            node_id = sengoo_json_doc_add_node(parser->doc, SENGOO_JSON_KIND_BOOL);
            SengooJsonNode* node = sengoo_json_node_from_id(parser->doc, node_id);
            if (node) node->bool_value = 0;
        } else {
            sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "invalid literal");
        }
    } else if (c == 'n') {
        if (sengoo_json_match_literal(parser, "null")) {
            node_id = sengoo_json_doc_add_node(parser->doc, SENGOO_JSON_KIND_NULL);
        } else {
            sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "invalid literal");
        }
    } else if (c == '-' || isdigit((unsigned char)c)) {
        node_id = sengoo_json_parse_number(parser);
    } else {
        sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "unexpected json token");
    }
    parser->depth -= 1;
    return node_id;
}

static long long sengoo_json_parse_bytes(const char* data, size_t len) {
    sengoo_json_clear_error();
    if (len > SENGOO_JSON_MAX_BYTES) {
        sengoo_json_set_error(SENGOO_STATUS_PARSE, 0, "json input byte limit exceeded");
        return 0;
    }
    if (len > 0 && !data) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, 0, "json input is null");
        return 0;
    }

    SengooJsonDoc* doc = (SengooJsonDoc*)calloc(1, sizeof(SengooJsonDoc));
    if (!doc) {
        sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, 0, "json document allocation failed");
        return 0;
    }
    SengooJsonParser parser = { data, len, 0, 0, doc };
    long long root = sengoo_json_parse_value(&parser);
    if (root == 0) {
                sengoo_json_doc_destroy(doc);
        return 0;
    }
    sengoo_json_skip_ws(&parser);
    if (parser.pos != parser.len) {
        sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser.pos, "trailing json input");
                sengoo_json_doc_destroy(doc);
        return 0;
    }
    doc->root = root;
    return sengoo_ptr_to_handle(doc);
}

long long sengoo_json_parse_text(long long data, long long len) {
    if (len < 0) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, 0, "negative json input length");
        return 0;
    }
    return sengoo_json_parse_bytes((const char*)(intptr_t)data, (size_t)len);
}

long long sengoo_json_parse_buffer(long long buffer_handle, long long input_len) {
    sengoo_json_clear_error();
    if (input_len < 0) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, 0, "negative json input length");
        return 0;
    }
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, 0, "buffer handle not found");
        return 0;
    }
    if ((unsigned long long)input_len > (unsigned long long)buffer->capacity) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, 0, "json input length exceeds buffer capacity");
        return 0;
    }
    return sengoo_json_parse_bytes((const char*)buffer->bytes, (size_t)input_len);
}

long long sengoo_json_doc_object_new(void) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_new_with_root(SENGOO_JSON_KIND_OBJECT);
    return doc ? sengoo_ptr_to_handle(doc) : 0;
}

long long sengoo_json_doc_close(long long handle) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = (SengooJsonDoc*)sengoo_handle_to_ptr(handle);
    if (!doc) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    if (doc->closed) {
        return 0;
    }
    sengoo_json_doc_free(doc);
    return 0;
}

long long sengoo_json_doc_root(long long handle) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc || doc->root == 0) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document root not found");
        return 0;
    }
    return doc->root;
}

long long sengoo_json_doc_new_object(long long handle) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
        return 0;
    }
    return sengoo_json_doc_add_node(doc, SENGOO_JSON_KIND_OBJECT);
}

long long sengoo_json_doc_new_array(long long handle) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
        return 0;
    }
    return sengoo_json_doc_add_node(doc, SENGOO_JSON_KIND_ARRAY);
}

long long sengoo_json_doc_new_string(long long handle, long long value_ptr) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
        return 0;
    }
    char* copy = sengoo_copy_cstr_from_handle(value_ptr);
    if (!copy) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json string input is null");
        return 0;
    }
    long long node_id = sengoo_json_doc_add_node(doc, SENGOO_JSON_KIND_STRING);
    SengooJsonNode* node = sengoo_json_node_from_id(doc, node_id);
    if (!node) {
        free(copy);
        return 0;
    }
    node->string_value = copy;
    return node_id;
}

long long sengoo_json_doc_new_bool(long long handle, long long value) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
        return 0;
    }
    long long node_id = sengoo_json_doc_add_node(doc, SENGOO_JSON_KIND_BOOL);
    SengooJsonNode* node = sengoo_json_node_from_id(doc, node_id);
    if (node) {
        node->bool_value = value != 0 ? 1 : 0;
    }
    return node_id;
}

long long sengoo_json_doc_new_number(long long handle, double value) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
        return 0;
    }
    long long node_id = sengoo_json_doc_add_node(doc, SENGOO_JSON_KIND_NUMBER);
    SengooJsonNode* node = sengoo_json_node_from_id(doc, node_id);
    if (!node) {
        return 0;
    }
    char text[64];
    snprintf(text, sizeof(text), "%.17g", value);
    node->number_text = sengoo_copy_cstr_from_handle((long long)(intptr_t)text);
    if (!node->number_text) {
        sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, -1, "json number allocation failed");
        return 0;
    }
    node->number_f64 = value;
    if (value >= (double)LLONG_MIN && value <= (double)LLONG_MAX) {
        long long as_i64 = (long long)value;
        if ((double)as_i64 == value) {
            node->number_i64 = as_i64;
            node->number_has_i64 = 1;
        }
    }
    return node_id;
}

long long sengoo_json_doc_new_null(long long handle) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
        return 0;
    }
    return sengoo_json_doc_add_node(doc, SENGOO_JSON_KIND_NULL);
}

long long sengoo_json_value_kind(long long doc_handle, long long node_id) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    if (!node) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json value handle not found");
        return 0;
    }
    return node->kind;
}

long long sengoo_json_object_has(long long doc_handle, long long node_id, long long key_ptr) {
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!node || node->kind != SENGOO_JSON_KIND_OBJECT || !key) {
        return 0;
    }
    for (size_t i = 0; i < node->member_len; ++i) {
        if (strcmp(node->members[i].key, key) == 0) {
            return 1;
        }
    }
    return 0;
}

long long sengoo_json_object_get(long long doc_handle, long long node_id, long long key_ptr) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!node || node->kind != SENGOO_JSON_KIND_OBJECT || !key) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json value is not an object");
        return 0;
    }
    for (size_t i = 0; i < node->member_len; ++i) {
        if (strcmp(node->members[i].key, key) == 0) {
            return node->members[i].value_node;
        }
    }
    sengoo_json_set_error(SENGOO_STATUS_NOT_FOUND, -1, "json object key not found");
    return 0;
}

long long sengoo_json_object_set(long long doc_handle, long long node_id, long long key_ptr, long long value_doc_handle, long long value_node_id) {
    sengoo_json_clear_error();
    if (doc_handle != value_doc_handle) {
        return sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json value belongs to a different document");
    }
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(doc_handle);
    SengooJsonNode* node = sengoo_json_node_from_id(doc, node_id);
    SengooJsonNode* value = sengoo_json_node_from_id(doc, value_node_id);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!node || node->kind != SENGOO_JSON_KIND_OBJECT || !value || !key) {
        return sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "invalid json object set");
    }
    if (!sengoo_json_object_set_member(node, key, value_node_id)) {
        return sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, -1, "json object allocation failed");
    }
    return 1;
}

long long sengoo_json_array_len(long long doc_handle, long long node_id) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    if (!node || node->kind != SENGOO_JSON_KIND_ARRAY) {
        return sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json value is not an array");
    }
    return (long long)node->array_len;
}

long long sengoo_json_array_get(long long doc_handle, long long node_id, long long index) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    if (!node || node->kind != SENGOO_JSON_KIND_ARRAY || index < 0 || (size_t)index >= node->array_len) {
        sengoo_json_set_error(SENGOO_STATUS_NOT_FOUND, -1, "json array index not found");
        return 0;
    }
    return node->array_items[(size_t)index];
}

long long sengoo_json_array_push(long long doc_handle, long long node_id, long long value_doc_handle, long long value_node_id) {
    sengoo_json_clear_error();
    if (doc_handle != value_doc_handle) {
        return sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json value belongs to a different document");
    }
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(doc_handle);
    SengooJsonNode* node = sengoo_json_node_from_id(doc, node_id);
    SengooJsonNode* value = sengoo_json_node_from_id(doc, value_node_id);
    if (!node || node->kind != SENGOO_JSON_KIND_ARRAY || !value) {
        return sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "invalid json array push");
    }
    if (!sengoo_json_array_reserve(node, node->array_len + 1)) {
        return sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, -1, "json array allocation failed");
    }
    node->array_items[node->array_len++] = value_node_id;
    return 1;
}

long long sengoo_json_string_copy(long long doc_handle, long long node_id, long long buffer_handle) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    if (!node || node->kind != SENGOO_JSON_KIND_STRING || !node->string_value) {
        return sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json value is not a string");
    }
    long long copied = sengoo_copy_bytes_to_managed_buffer(buffer_handle, node->string_value, strlen(node->string_value));
    if (copied < 0) {
        sengoo_json_set_error(-copied, -1, "json string output buffer too small");
    }
    return copied;
}

long long sengoo_json_string_value(long long doc_handle, long long node_id) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    if (!node || node->kind != SENGOO_JSON_KIND_STRING || !node->string_value) {
        return sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json value is not a string");
    }
    size_t len = strlen(node->string_value);
    long long handle = sengoo_string_from_bytes_copy((long long)(intptr_t)node->string_value, (long long)len);
    if (handle < 0) {
        sengoo_json_set_error(-handle, -1, "json string allocation failed");
    }
    return handle;
}

long long sengoo_json_bool_value(long long doc_handle, long long node_id) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    if (!node || node->kind != SENGOO_JSON_KIND_BOOL) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json value is not a bool");
        return 0;
    }
    return node->bool_value ? 1 : 0;
}

long long sengoo_json_number_i64(long long doc_handle, long long node_id) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    if (!node || node->kind != SENGOO_JSON_KIND_NUMBER || !node->number_has_i64) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json number is not an exact i64");
        return 0;
    }
    return node->number_i64;
}

double sengoo_json_number_f64(long long doc_handle, long long node_id) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    if (!node || node->kind != SENGOO_JSON_KIND_NUMBER) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json value is not a number");
        return 0.0;
    }
    return node->number_f64;
}

static int sengoo_json_serialize_node(SengooJsonDoc* doc, long long node_id, SengooJsonBuilder* builder) {
    SengooJsonNode* node = sengoo_json_node_from_id(doc, node_id);
    if (!node) {
        return 0;
    }
    switch (node->kind) {
        case SENGOO_JSON_KIND_NULL:
            return sengoo_json_builder_append_cstr(builder, "null");
        case SENGOO_JSON_KIND_BOOL:
            return sengoo_json_builder_append_cstr(builder, node->bool_value ? "true" : "false");
        case SENGOO_JSON_KIND_NUMBER:
            return sengoo_json_builder_append_cstr(builder, node->number_text ? node->number_text : "0");
        case SENGOO_JSON_KIND_STRING:
            return sengoo_json_builder_append_escaped_string(builder, node->string_value ? node->string_value : "");
        case SENGOO_JSON_KIND_ARRAY:
            if (!sengoo_json_builder_append_char(builder, '[')) return 0;
            for (size_t i = 0; i < node->array_len; ++i) {
                if (i > 0 && !sengoo_json_builder_append_char(builder, ',')) return 0;
                if (!sengoo_json_serialize_node(doc, node->array_items[i], builder)) return 0;
            }
            return sengoo_json_builder_append_char(builder, ']');
        case SENGOO_JSON_KIND_OBJECT:
            if (!sengoo_json_builder_append_char(builder, '{')) return 0;
            for (size_t i = 0; i < node->member_len; ++i) {
                if (i > 0 && !sengoo_json_builder_append_char(builder, ',')) return 0;
                if (!sengoo_json_builder_append_escaped_string(builder, node->members[i].key)) return 0;
                if (!sengoo_json_builder_append_char(builder, ':')) return 0;
                if (!sengoo_json_serialize_node(doc, node->members[i].value_node, builder)) return 0;
            }
            return sengoo_json_builder_append_char(builder, '}');
        default:
            return 0;
    }
}

long long sengoo_json_doc_serialize(long long handle, long long buffer_handle) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc || doc->root == 0) {
        return sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
    }
    SengooJsonBuilder builder = {0};
    if (!sengoo_json_serialize_node(doc, doc->root, &builder)) {
        free(builder.data);
        return sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, -1, "json serialization failed");
    }
    long long copied = sengoo_copy_bytes_to_managed_buffer(buffer_handle, builder.data, builder.len);
    free(builder.data);
    if (copied < 0) {
        sengoo_json_set_error(-copied, -1, "json serialization buffer too small");
    }
    return copied;
}

long long sengoo_json_last_error_code(void) {
    return sengoo_json_last_error;
}

long long sengoo_json_last_error_offset(void) {
    return sengoo_json_last_offset;
}

long long sengoo_json_last_error_copy(long long buffer_handle) {
    size_t len = strlen(sengoo_json_last_message);
    long long copied = sengoo_copy_bytes_to_managed_buffer(buffer_handle, sengoo_json_last_message, len);
    return copied;
}
