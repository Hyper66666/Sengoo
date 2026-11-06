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
#define SENGOO_JSON_BOUNDED_MAX_BYTES (16U * 1024U * 1024U)
#define SENGOO_JSON_BOUNDED_TOTAL_RESERVATION (512U * 1024U * 1024U)
#define SENGOO_JSON_BOUNDED_MAX_DOCS 64

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
    size_t key_len;
    long long value_node;
} SengooJsonMember;

typedef struct {
    int kind;
    char* string_value;
    size_t string_len;
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
    size_t byte_limit, owned_payload, owned_edges, reservation;
    int bounded;
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
    size_t limit;
} SengooJsonBuilder;

static int sengoo_json_last_error = SENGOO_STATUS_OK;
static long long sengoo_json_last_offset = -1;
static char sengoo_json_last_message[256] = {0};
static void sengoo_json_doc_destroy(SengooJsonDoc* doc);
static size_t sengoo_json_bounded_reservation, sengoo_json_bounded_documents;

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

static SengooJsonDoc* sengoo_json_doc_allocate(size_t explicit_limit) {
    size_t reservation = explicit_limit ? 6 * explicit_limit + 4U * 1024U * 1024U : 0;
    if (explicit_limit && (sengoo_json_bounded_documents >= SENGOO_JSON_BOUNDED_MAX_DOCS
        || reservation > SENGOO_JSON_BOUNDED_TOTAL_RESERVATION - sengoo_json_bounded_reservation)) {
        sengoo_json_set_error(SENGOO_STATUS_OVERFLOW, -1, "bounded JSON aggregate admission exhausted");
        return NULL;
    }
    SengooJsonDoc* doc = calloc(1,sizeof(*doc));
    if (!doc) { sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY,-1,"json document allocation failed"); return NULL; }
    doc->bounded = explicit_limit != 0;
    doc->byte_limit = explicit_limit ? explicit_limit : SENGOO_JSON_MAX_BYTES;
    doc->reservation = reservation;
    if (doc->bounded) { sengoo_json_bounded_documents++; sengoo_json_bounded_reservation += reservation; }
    return doc;
}

static int sengoo_json_payload_charge(SengooJsonDoc* doc, size_t bytes) {
    if (!doc->bounded) return 1;
    if (bytes > doc->byte_limit - doc->owned_payload) {
        sengoo_json_set_error(SENGOO_STATUS_OVERFLOW,-1,"bounded JSON owned payload limit exceeded"); return 0;
    }
    doc->owned_payload += bytes; return 1;
}
static int sengoo_json_edge_charge(SengooJsonDoc* doc) {
    if (!doc->bounded) return 1;
    if (doc->owned_edges >= SENGOO_JSON_MAX_NODES) {
        sengoo_json_set_error(SENGOO_STATUS_OVERFLOW,-1,"bounded JSON container link limit exceeded"); return 0;
    }
    doc->owned_edges++; return 1;
}
static int sengoo_json_explicit_limit(long long limit) {
    if (limit < 1 || limit > SENGOO_JSON_BOUNDED_MAX_BYTES) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT,0,"bounded JSON limit must be 1..16MiB"); return 0;
    }
    return 1;
}
long long sengoo_json_bounded_live_documents(void) { return (long long)sengoo_json_bounded_documents; }
long long sengoo_json_bounded_reserved_bytes(void) { return (long long)sengoo_json_bounded_reservation; }

static SengooJsonDoc* sengoo_json_doc_from_handle(long long handle) {
    SengooJsonDoc* doc = (SengooJsonDoc*)sengoo_opaque_handle_get(handle);
    return doc && !doc->closed ? doc : NULL;
}

static long long sengoo_json_doc_into_handle(SengooJsonDoc* doc) {
    if (!doc) {
        return 0;
    }
    long long handle = sengoo_opaque_handle_new(doc);
    if (handle == 0) {
        sengoo_json_doc_destroy(doc);
        sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, -1, "json handle allocation failed");
    }
    return handle;
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
    if (doc->bounded) { sengoo_json_bounded_documents--; sengoo_json_bounded_reservation -= doc->reservation; }
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

static SengooJsonDoc* sengoo_json_doc_new_with_root_limit(int root_kind, size_t explicit_limit) {
    SengooJsonDoc* doc = sengoo_json_doc_allocate(explicit_limit);
    if (!doc) {
        if (!sengoo_json_last_error) sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, -1, "json document allocation failed");
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
    if (builder->limit && (builder->len > builder->limit || extra > builder->limit - builder->len)) {
        sengoo_json_set_error(SENGOO_STATUS_OVERFLOW,-1,"bounded JSON output byte limit exceeded"); return 0;
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

static int sengoo_json_builder_append_escaped_string(SengooJsonBuilder* builder, const char* value, size_t len) {
    if (!sengoo_json_builder_append_char(builder, '"')) {
        return 0;
    }
    const unsigned char* cursor = (const unsigned char*)value;
    while (cursor && len-- > 0) {
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

static int sengoo_json_object_set_member(SengooJsonDoc* doc, SengooJsonNode* object, const char* key, size_t key_len, long long value_node) {
    if (!object || object->kind != SENGOO_JSON_KIND_OBJECT || !key || value_node <= 0) {
        return 0;
    }
    for (size_t i = 0; i < object->member_len; ++i) {
        if (object->members[i].key_len == key_len && memcmp(object->members[i].key, key, key_len) == 0) {
            object->members[i].value_node = value_node;
            return 1;
        }
    }
    size_t prior_payload = doc->owned_payload, prior_edges = doc->owned_edges;
    if (!sengoo_json_payload_charge(doc,key_len) || !sengoo_json_edge_charge(doc)) {
        doc->owned_payload = prior_payload; doc->owned_edges = prior_edges; return 0;
    }
    char* key_copy = (char*)malloc(key_len + 1);
    if (key_copy) { memcpy(key_copy, key, key_len); key_copy[key_len] = 0; }
    if (!key_copy || !sengoo_json_object_reserve(object, object->member_len + 1)) {
        free(key_copy);
        doc->owned_payload = prior_payload; doc->owned_edges = prior_edges;
        return 0;
    }
    object->members[object->member_len].key = key_copy;
    object->members[object->member_len].key_len = key_len;
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

static int sengoo_json_hex4(SengooJsonParser* parser, unsigned* code) {
    if (parser->pos + 4 > parser->len) return 0;
    *code = 0;
    for (int i = 0; i < 4; ++i) {
        int nibble = sengoo_json_hex_value(parser->data[parser->pos + (size_t)i]);
        if (nibble < 0) return 0;
        *code = (*code << 4) | (unsigned)nibble;
    }
    parser->pos += 4;
    return 1;
}

static char* sengoo_json_parse_string_raw(SengooJsonParser* parser, size_t* output_len) {
    if (parser->pos >= parser->len || parser->data[parser->pos] != '"') {
        sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "expected json string");
        return NULL;
    }
    parser->pos += 1;

    SengooJsonBuilder builder = {0};
    if (parser->doc->bounded) builder.limit = parser->doc->byte_limit;
    while (parser->pos < parser->len) {
        unsigned char c = (unsigned char)parser->data[parser->pos++];
        if (c == '"') {
            if (!sengoo_json_builder_reserve(&builder, 0)) {
                free(builder.data);
                return NULL;
            }
            *output_len = builder.len;
            return builder.data ? builder.data : sengoo_copy_cstr_from_handle((long long)(intptr_t)"");
        }
        if (c < 0x20) {
            free(builder.data);
            sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)(parser->pos - 1), "control character in json string");
            return NULL;
        }
        if (c != '\\') {
            /* Validate raw UTF-8 here, including overlongs, surrogate codepoints
               and values beyond U+10ffff. Escapes use the same scalar contract. */
            if (c >= 0x80) {
                unsigned code = 0;
                int tail = 0;
                unsigned minimum = 0;
                if (c >= 0xc2 && c <= 0xdf) { tail = 1; code = c & 31; minimum = 0x80; }
                else if (c >= 0xe0 && c <= 0xef) { tail = 2; code = c & 15; minimum = 0x800; }
                else if (c >= 0xf0 && c <= 0xf4) { tail = 3; code = c & 7; minimum = 0x10000; }
                else goto invalid_unicode;
                if (parser->pos + (size_t)tail > parser->len) goto invalid_unicode;
                for (int i = 0; i < tail; ++i) {
                    unsigned char next = (unsigned char)parser->data[parser->pos + (size_t)i];
                    if ((next & 0xc0) != 0x80) goto invalid_unicode;
                    code = (code << 6) | (next & 63);
                }
                if (code < minimum || code > 0x10ffff || (code >= 0xd800 && code <= 0xdfff)) goto invalid_unicode;
                if (!sengoo_json_builder_append_char(&builder, (char)c)) goto allocation_failed;
                for (int i = 0; i < tail; ++i)
                    if (!sengoo_json_builder_append_char(&builder, parser->data[parser->pos++])) goto allocation_failed;
                continue;
            }
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
                unsigned code = 0, low = 0;
                if (!sengoo_json_hex4(parser, &code)) goto invalid_unicode;
                if (code >= 0xd800 && code <= 0xdbff) {
                    if (parser->pos + 2 > parser->len || parser->data[parser->pos] != '\\'
                        || parser->data[parser->pos + 1] != 'u') goto invalid_unicode;
                    parser->pos += 2;
                    if (!sengoo_json_hex4(parser, &low) || low < 0xdc00 || low > 0xdfff) goto invalid_unicode;
                    code = 0x10000 + ((code - 0xd800) << 10) + low - 0xdc00;
                } else if (code >= 0xdc00 && code <= 0xdfff) goto invalid_unicode;
                if (code < 0x80) { decoded = (char)code; break; }
                if (code < 0x800) {
                    if (!sengoo_json_builder_append_char(&builder, (char)(0xc0 | (code >> 6)))) goto allocation_failed;
                } else {
                    if (code >= 0x10000) {
                        if (!sengoo_json_builder_append_char(&builder, (char)(0xf0 | (code >> 18)))) goto allocation_failed;
                        if (!sengoo_json_builder_append_char(&builder, (char)(0x80 | ((code >> 12) & 63)))) goto allocation_failed;
                    } else if (!sengoo_json_builder_append_char(&builder, (char)(0xe0 | (code >> 12)))) goto allocation_failed;
                    if (!sengoo_json_builder_append_char(&builder, (char)(0x80 | ((code >> 6) & 63)))) goto allocation_failed;
                }
                if (!sengoo_json_builder_append_char(&builder, (char)(0x80 | (code & 63)))) goto allocation_failed;
                continue;
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
invalid_unicode:
    free(builder.data);
    sengoo_json_set_error(SENGOO_STATUS_PARSE, (long long)parser->pos, "invalid JSON Unicode scalar or escape");
    return NULL;
allocation_failed:
    free(builder.data);
    sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, (long long)parser->pos, "json string allocation failed");
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
    if (!sengoo_json_payload_charge(parser->doc,len)) return 0;
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
        array = sengoo_json_node_from_id(parser->doc, node_id);
        if (!sengoo_json_edge_charge(parser->doc) || !sengoo_json_array_reserve(array, array->array_len + 1)) {
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
        size_t key_len = 0;
        char* key = sengoo_json_parse_string_raw(parser, &key_len);
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
        /* Recursive parsing can reallocate the node array. */
        object = sengoo_json_node_from_id(parser->doc, node_id);
        if (!sengoo_json_object_set_member(parser->doc, object, key, key_len, child)) {
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
        size_t value_len = 0;
        char* value = sengoo_json_parse_string_raw(parser, &value_len);
        if (value) {
            if (!sengoo_json_payload_charge(parser->doc,value_len)) { free(value); parser->depth--; return 0; }
            node_id = sengoo_json_doc_add_node(parser->doc, SENGOO_JSON_KIND_STRING);
            SengooJsonNode* node = sengoo_json_node_from_id(parser->doc, node_id);
            if (node) {
                node->string_value = value;
                node->string_len = value_len;
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

static long long sengoo_json_parse_bytes_limit(const char* data, size_t len, size_t explicit_limit) {
    sengoo_json_clear_error();
    if (len > (explicit_limit ? explicit_limit : SENGOO_JSON_MAX_BYTES)) {
        sengoo_json_set_error(explicit_limit ? SENGOO_STATUS_OVERFLOW : SENGOO_STATUS_PARSE, 0, "json input byte limit exceeded");
        return 0;
    }
    if (len > 0 && !data) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, 0, "json input is null");
        return 0;
    }

    SengooJsonDoc* doc = sengoo_json_doc_allocate(explicit_limit);
    if (!doc) {
        if (!sengoo_json_last_error) sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, 0, "json document allocation failed");
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
    return sengoo_json_doc_into_handle(doc);
}

long long sengoo_json_parse_text(long long data, long long len) {
    if (len < 0) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, 0, "negative json input length");
        return 0;
    }
    return sengoo_json_parse_bytes_limit((const char*)(intptr_t)data, (size_t)len, 0);
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
    return sengoo_json_parse_bytes_limit((const char*)buffer->bytes, (size_t)input_len, 0);
}

long long sengoo_json_parse_text_bounded(long long data, long long length, long long max_bytes) {
    sengoo_json_clear_error();
    if (!sengoo_json_explicit_limit(max_bytes)) return 0;
    if (length < 0) { sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT,0,"negative json input length"); return 0; }
    return sengoo_json_parse_bytes_limit((const char*)(intptr_t)data,(size_t)length,(size_t)max_bytes);
}
long long sengoo_json_parse_buffer_bounded(long long buffer_handle, long long length, long long max_bytes) {
    sengoo_json_clear_error();
    if (!sengoo_json_explicit_limit(max_bytes)) return 0;
    if (length < 0) { sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT,0,"negative json input length"); return 0; }
    if (length > max_bytes) { sengoo_json_set_error(SENGOO_STATUS_OVERFLOW,0,"json input byte limit exceeded"); return 0; }
    SengooFfiBuffer* buffer = sengoo_ffi_buffer_from_handle(buffer_handle);
    if (!buffer) { sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE,0,"buffer handle not found"); return 0; }
    if ((unsigned long long)length > buffer->used_len || buffer->used_len > buffer->capacity) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT,0,"json input exceeds initialized buffer bytes"); return 0;
    }
    return sengoo_json_parse_bytes_limit((const char*)buffer->bytes,(size_t)length,(size_t)max_bytes);
}

long long sengoo_json_doc_object_new(void) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_new_with_root_limit(SENGOO_JSON_KIND_OBJECT,0);
    return sengoo_json_doc_into_handle(doc);
}
long long sengoo_json_doc_object_new_bounded(long long max_bytes) {
    sengoo_json_clear_error();
    if (!sengoo_json_explicit_limit(max_bytes)) return 0;
    return sengoo_json_doc_into_handle(sengoo_json_doc_new_with_root_limit(SENGOO_JSON_KIND_OBJECT,(size_t)max_bytes));
}

long long sengoo_json_doc_close(long long handle) {
    sengoo_json_clear_error();
    if (handle == 0) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
        return -SENGOO_STATUS_INVALID_HANDLE;
    }
    SengooJsonDoc* doc = (SengooJsonDoc*)sengoo_opaque_handle_take(handle);
    if (!doc) {
        return 0;
    }
    sengoo_json_doc_destroy(doc);
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

long long sengoo_json_doc_new_string_bytes(long long handle, long long value_ptr, long long length) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc) { sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE,-1,"json document handle not found"); return 0; }
    if (length < 0 || (unsigned long long)length > SENGOO_RUNTIME_MAX_BUFFER_BYTES) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT,-1,"invalid json string length"); return 0;
    }
    size_t prior_payload = doc->owned_payload;
    if (!sengoo_json_payload_charge(doc,(size_t)length)) return 0;
    if (!value_ptr && length) { doc->owned_payload = prior_payload; sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT,-1,"json string input is null"); return 0; }
    char* bytes = malloc((size_t)length + 1);
    if (!bytes) { doc->owned_payload = prior_payload; sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY,-1,"json string allocation failed"); return 0; }
    if (length) memcpy(bytes,(const void*)(intptr_t)value_ptr,(size_t)length);
    bytes[length] = 0;
    long long id = sengoo_json_doc_add_node(doc,SENGOO_JSON_KIND_STRING);
    SengooJsonNode* node = sengoo_json_node_from_id(doc,id);
    if (!node) { free(bytes); doc->owned_payload = prior_payload; return 0; }
    node->string_value = bytes; node->string_len = (size_t)length; return id;
}

static size_t sengoo_json_bounded_cstr_length(const char* value, size_t limit) {
    size_t length = 0;
    if (value) while (length <= limit && value[length]) length++;
    return length;
}

long long sengoo_json_doc_new_string(long long handle, long long value_ptr) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
        return 0;
    }
    if (doc->bounded && value_ptr) {
        size_t length = sengoo_json_bounded_cstr_length((const char*)(intptr_t)value_ptr,doc->byte_limit);
        return sengoo_json_doc_new_string_bytes(handle,value_ptr,(long long)length);
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
    node->string_len = strlen(copy);
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
    if (!sengoo_json_payload_charge(doc,strlen(text))) return 0;
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

/* Preserve the decimal spelling instead of rounding through binary64. */
long long sengoo_json_doc_new_i64(long long handle, long long value) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
        return 0;
    }
    long long node_id = sengoo_json_doc_add_node(doc, SENGOO_JSON_KIND_NUMBER);
    SengooJsonNode* node = sengoo_json_node_from_id(doc, node_id);
    if (!node) return 0;
    char text[32];
    snprintf(text, sizeof(text), "%lld", value);
    if (!sengoo_json_payload_charge(doc,strlen(text))) return 0;
    node->number_text = sengoo_copy_cstr_from_handle((long long)(intptr_t)text);
    if (!node->number_text) {
        sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, -1, "json integer allocation failed");
        return 0;
    }
    node->number_i64 = value;
    node->number_has_i64 = 1;
    node->number_f64 = (double)value;
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

long long sengoo_json_object_len(long long doc_handle, long long node_id) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    if (!node) { sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json value handle not found"); return -SENGOO_STATUS_INVALID_HANDLE; }
    if (node->kind != SENGOO_JSON_KIND_OBJECT) { sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json value is not an object"); return -SENGOO_STATUS_INVALID_ARGUMENT; }
    return (long long)node->member_len;
}

long long sengoo_json_object_key(long long doc_handle, long long node_id, long long index) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    if (!node) { sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json value handle not found"); return -SENGOO_STATUS_INVALID_HANDLE; }
    if (node->kind != SENGOO_JSON_KIND_OBJECT) { sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json value is not an object"); return -SENGOO_STATUS_INVALID_ARGUMENT; }
    if (index < 0 || (unsigned long long)index >= node->member_len) { sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json object key index out of range"); return -SENGOO_STATUS_INVALID_ARGUMENT; }
    SengooJsonMember* member = &node->members[index];
    long long result = sengoo_string_from_bytes_copy((long long)(intptr_t)member->key, (long long)member->key_len);
    if (result <= 0) { sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, -1, "json key allocation failed"); return -SENGOO_STATUS_OUT_OF_MEMORY; }
    return result;
}

long long sengoo_json_object_has(long long doc_handle, long long node_id, long long key_ptr) {
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    const char* key = (const char*)(intptr_t)key_ptr;
    if (!node || node->kind != SENGOO_JSON_KIND_OBJECT || !key) {
        return 0;
    }
    for (size_t i = 0; i < node->member_len; ++i) {
        if (node->members[i].key_len == strlen(key) && memcmp(node->members[i].key, key, strlen(key)) == 0) {
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
        if (node->members[i].key_len == strlen(key) && memcmp(node->members[i].key, key, strlen(key)) == 0) {
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
    size_t key_length = doc->bounded ? sengoo_json_bounded_cstr_length(key,doc->byte_limit) : strlen(key);
    if (!sengoo_json_object_set_member(doc, node, key, key_length, value_node_id)) {
        return sengoo_json_set_error(sengoo_json_last_error ? sengoo_json_last_error : SENGOO_STATUS_OUT_OF_MEMORY, -1, "json object allocation failed");
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
    size_t prior_edges = doc->owned_edges;
    if (!sengoo_json_edge_charge(doc) || !sengoo_json_array_reserve(node, node->array_len + 1)) {
        doc->owned_edges = prior_edges;
        return sengoo_json_set_error(sengoo_json_last_error ? sengoo_json_last_error : SENGOO_STATUS_OUT_OF_MEMORY, -1, "json array allocation failed");
    }
    node->array_items[node->array_len++] = value_node_id;
    return 1;
}

typedef struct {
    SengooJsonDoc* destination;
    SengooJsonDoc* source;
    long long path[SENGOO_JSON_MAX_DEPTH];
    size_t bytes;
} SengooJsonCopy;

static int sengoo_json_copy_charge(SengooJsonCopy* copy, size_t bytes) {
    if (bytes > copy->destination->byte_limit - copy->bytes) {
        sengoo_json_set_error(copy->destination->bounded ? SENGOO_STATUS_OVERFLOW : SENGOO_STATUS_PARSE, -1, "json copy byte limit exceeded");
        return 0;
    }
    copy->bytes += bytes;
    return 1;
}

static int sengoo_json_copy_string_charge(SengooJsonCopy* copy, const char* value, size_t length) {
    if (!sengoo_json_copy_charge(copy, 2)) return 0;
    for (size_t i = 0; i < length; ++i) {
        unsigned char byte = (unsigned char)value[i];
        size_t encoded = byte == '\b' || byte == '\f' || byte == '\n' || byte == '\r' || byte == '\t'
            || byte == '"' || byte == '\\' ? 2 : (byte < 0x20 ? 6 : 1);
        if (!sengoo_json_copy_charge(copy, encoded)) return 0;
    }
    return 1;
}

static long long sengoo_json_copy_node(SengooJsonCopy* copy, long long source_id, size_t depth) {
    if (depth >= SENGOO_JSON_MAX_DEPTH) {
        sengoo_json_set_error(SENGOO_STATUS_PARSE, -1, "json copy depth limit exceeded");
        return 0;
    }
    for (size_t i = 0; i < depth; ++i) {
        if (copy->path[i] == source_id) {
            sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "cyclic JSON value cannot be copied");
            return 0;
        }
    }
    SengooJsonNode* original = sengoo_json_node_from_id(copy->source, source_id);
    if (!original) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json source value not found");
        return 0;
    }
    // A same-document copy may relocate the node table. Heap-owned payloads
    // remain stable, and this scalar snapshot never points into that table.
    SengooJsonNode source = *original;
    copy->path[depth] = source_id;
    long long id = sengoo_json_doc_add_node(copy->destination, source.kind);
    SengooJsonNode* target = sengoo_json_node_from_id(copy->destination, id);
    if (!target) return 0;
    target->bool_value = source.bool_value;
    target->number_f64 = source.number_f64;
    target->number_i64 = source.number_i64;
    target->number_has_i64 = source.number_has_i64;
    if (source.kind == SENGOO_JSON_KIND_STRING) {
        if (!sengoo_json_copy_string_charge(copy, source.string_value, source.string_len)) return 0;
        if (!sengoo_json_payload_charge(copy->destination,source.string_len)) return 0;
        target->string_value = (char*)malloc(source.string_len + 1);
        if (!target->string_value) goto allocation_failed;
        memcpy(target->string_value, source.string_value, source.string_len);
        target->string_value[source.string_len] = 0;
        target->string_len = source.string_len;
    } else if (source.kind == SENGOO_JSON_KIND_NUMBER) {
        size_t length = source.number_text ? strlen(source.number_text) : 1;
        if (!sengoo_json_copy_charge(copy, length)) return 0;
        if (!sengoo_json_payload_charge(copy->destination,length)) return 0;
        target->number_text = (char*)malloc(length + 1);
        if (!target->number_text) goto allocation_failed;
        memcpy(target->number_text, source.number_text ? source.number_text : "0", length + 1);
    } else if (source.kind == SENGOO_JSON_KIND_ARRAY || source.kind == SENGOO_JSON_KIND_OBJECT) {
        if (!sengoo_json_copy_charge(copy, 2)) return 0;
        size_t length = source.kind == SENGOO_JSON_KIND_ARRAY ? source.array_len : source.member_len;
        for (size_t i = 0; i < length; ++i) {
            if (i && !sengoo_json_copy_charge(copy, 1)) return 0;
            if (source.kind == SENGOO_JSON_KIND_OBJECT &&
                (!sengoo_json_copy_string_charge(copy, source.members[i].key, source.members[i].key_len)
                 || !sengoo_json_copy_charge(copy, 1))) return 0;
            long long child = sengoo_json_copy_node(copy,
                source.kind == SENGOO_JSON_KIND_ARRAY ? source.array_items[i] : source.members[i].value_node, depth + 1);
            if (!child) return 0;
            target = sengoo_json_node_from_id(copy->destination, id);
            if (source.kind == SENGOO_JSON_KIND_ARRAY) {
                if (!sengoo_json_edge_charge(copy->destination)) return 0;
                if (!sengoo_json_array_reserve(target, target->array_len + 1)) goto allocation_failed;
                target->array_items[target->array_len++] = child;
            } else if (!sengoo_json_object_set_member(copy->destination, target, source.members[i].key, source.members[i].key_len, child)) {
                if (sengoo_json_last_error) return 0;
                goto allocation_failed;
            }
        }
    } else if (source.kind == SENGOO_JSON_KIND_BOOL || source.kind == SENGOO_JSON_KIND_NULL) {
        if (!sengoo_json_copy_charge(copy, source.kind == SENGOO_JSON_KIND_BOOL ? (source.bool_value ? 4 : 5) : 4)) return 0;
    } else {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "invalid JSON source kind");
        return 0;
    }
    return id;
allocation_failed:
    sengoo_json_set_error(SENGOO_STATUS_OUT_OF_MEMORY, -1, "json copy allocation failed");
    return 0;
}

long long sengoo_json_doc_copy(long long destination_handle, long long source_handle, long long source_id) {
    sengoo_json_clear_error();
    SengooJsonDoc* destination = sengoo_json_doc_from_handle(destination_handle);
    SengooJsonDoc* source = sengoo_json_doc_from_handle(source_handle);
    if (!destination || !source || !sengoo_json_node_from_id(source, source_id)) {
        sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json copy document or value not found");
        return 0;
    }
    size_t original_length = destination->len;
    size_t original_payload = destination->owned_payload, original_edges = destination->owned_edges;
    SengooJsonCopy copy = {0};
    copy.destination = destination;
    copy.source = source;
    long long result = sengoo_json_copy_node(&copy, source_id, 0);
    if (!result) {
        for (size_t i = original_length; i < destination->len; ++i) sengoo_json_node_free(&destination->nodes[i]);
        destination->len = original_length;
        destination->owned_payload = original_payload; destination->owned_edges = original_edges;
    }
    return result;
}

long long sengoo_json_string_copy(long long doc_handle, long long node_id, long long buffer_handle) {
    sengoo_json_clear_error();
    SengooJsonNode* node = sengoo_json_node_from_id(sengoo_json_doc_from_handle(doc_handle), node_id);
    if (!node || node->kind != SENGOO_JSON_KIND_STRING || !node->string_value) {
        return sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT, -1, "json value is not a string");
    }
    long long copied = sengoo_copy_bytes_to_managed_buffer(buffer_handle, node->string_value, node->string_len);
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
    size_t len = node->string_len;
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

static int sengoo_json_serialize_node(SengooJsonDoc* doc, long long node_id, SengooJsonBuilder* builder, size_t depth, long long* path) {
    if (doc->bounded) {
        if (depth >= SENGOO_JSON_MAX_DEPTH) { sengoo_json_set_error(SENGOO_STATUS_OVERFLOW,-1,"bounded JSON serialization depth exceeded"); return 0; }
        for (size_t i = 0; i < depth; i++) if (path[i] == node_id) {
            sengoo_json_set_error(SENGOO_STATUS_INVALID_ARGUMENT,-1,"cyclic JSON cannot be serialized"); return 0;
        }
        path[depth] = node_id;
    }
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
            return sengoo_json_builder_append_escaped_string(builder, node->string_value ? node->string_value : "", node->string_len);
        case SENGOO_JSON_KIND_ARRAY:
            if (!sengoo_json_builder_append_char(builder, '[')) return 0;
            for (size_t i = 0; i < node->array_len; ++i) {
                if (i > 0 && !sengoo_json_builder_append_char(builder, ',')) return 0;
                if (!sengoo_json_serialize_node(doc, node->array_items[i], builder, depth+1, path)) return 0;
            }
            return sengoo_json_builder_append_char(builder, ']');
        case SENGOO_JSON_KIND_OBJECT:
            if (!sengoo_json_builder_append_char(builder, '{')) return 0;
            for (size_t i = 0; i < node->member_len; ++i) {
                if (i > 0 && !sengoo_json_builder_append_char(builder, ',')) return 0;
                if (!sengoo_json_builder_append_escaped_string(builder, node->members[i].key, node->members[i].key_len)) return 0;
                if (!sengoo_json_builder_append_char(builder, ':')) return 0;
                if (!sengoo_json_serialize_node(doc, node->members[i].value_node, builder, depth+1, path)) return 0;
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
    if (doc->bounded) builder.limit = doc->byte_limit;
    long long path[SENGOO_JSON_MAX_DEPTH];
    if (!sengoo_json_serialize_node(doc, doc->root, &builder, 0, path)) {
        free(builder.data);
        return sengoo_json_set_error(sengoo_json_last_error ? sengoo_json_last_error : SENGOO_STATUS_OUT_OF_MEMORY, -1, "json serialization failed");
    }
    long long copied = sengoo_copy_bytes_to_managed_buffer(buffer_handle, builder.data, builder.len);
    free(builder.data);
    if (copied < 0) {
        sengoo_json_set_error(-copied, -1, "json serialization buffer too small");
    }
    return copied;
}

long long sengoo_string_from_str_copy(long long value_ptr);

long long sengoo_json_doc_serialize_string(long long handle) {
    sengoo_json_clear_error();
    SengooJsonDoc* doc = sengoo_json_doc_from_handle(handle);
    if (!doc || doc->root == 0) {
        return sengoo_json_set_error(SENGOO_STATUS_INVALID_HANDLE, -1, "json document handle not found");
    }
    SengooJsonBuilder builder = {0};
    if (doc->bounded) builder.limit = doc->byte_limit;
    long long path[SENGOO_JSON_MAX_DEPTH];
    if (!sengoo_json_serialize_node(doc, doc->root, &builder, 0, path)) {
        free(builder.data);
        return sengoo_json_set_error(sengoo_json_last_error ? sengoo_json_last_error : SENGOO_STATUS_OUT_OF_MEMORY, -1, "json serialization failed");
    }
    long long value = sengoo_string_from_bytes_copy((long long)(intptr_t)builder.data,(long long)builder.len);
    free(builder.data);
    if (value < 0) {
        sengoo_json_set_error(-value, -1, "json serialization failed");
    }
    return value;
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
