/* Compile this actual bridge independently; every input is synthetic. */
#include <assert.h>
#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include "../runtime_json.c"

static void* handles[256];
static SengooFfiBuffer test_buffer;
long long sengoo_opaque_handle_new(void* value) { for(int i=1;i<256;i++) if(!handles[i]) { handles[i]=value; return i; } return 0; }
void* sengoo_opaque_handle_get(long long value) { return value>0 && value<256 ? handles[value] : NULL; }
void* sengoo_opaque_handle_take(long long value) { void* result=sengoo_opaque_handle_get(value); if(result) handles[value]=NULL; return result; }
long long sengoo_ptr_to_handle(void* value) { return (long long)(intptr_t)value; }
SengooFfiBuffer* sengoo_ffi_buffer_from_handle(long long value) { return value==700 ? &test_buffer : NULL; }
long long sengoo_copy_bytes_to_managed_buffer(long long handle,const char* bytes,size_t length) {
    if(handle!=700) return -SENGOO_STATUS_INVALID_HANDLE;
    if(length>test_buffer.capacity) return -SENGOO_STATUS_BUFFER_TOO_SMALL;
    if(length) memcpy(test_buffer.bytes,bytes,length); test_buffer.used_len=length; return (long long)length;
}
long long sengoo_string_from_bytes_copy(long long pointer,long long length) {
    char* value=malloc((size_t)length+1); assert(value); memcpy(value,(const void*)(intptr_t)pointer,(size_t)length); value[length]=0; return (long long)(intptr_t)value;
}
char* sengoo_copy_cstr_from_handle(long long value) {
    if(!value) return NULL; const char* text=(const char*)(intptr_t)value; size_t length=strlen(text); char* copy=malloc(length+1); assert(copy); memcpy(copy,text,length+1); return copy;
}
static long long raw(const void* value) { return (long long)(intptr_t)value; }
static void close_doc(long long doc) { assert(sengoo_json_doc_close(doc)==0); }
int main(void) {
    const size_t size=1024U*1024U+2;
    char* text=malloc(SENGOO_JSON_BOUNDED_MAX_BYTES+1); assert(text);
    memset(text,'a',SENGOO_JSON_BOUNDED_MAX_BYTES); text[0]='"'; text[size-1]='"'; text[size]=0;
    assert(!sengoo_json_parse_text(raw(text),size) && sengoo_json_last_error_code()==10);
    assert(!sengoo_json_parse_text_bounded(1,16777217,16777216) && sengoo_json_last_error_code()==13);
    assert(!sengoo_json_parse_text_bounded(1,10,9) && sengoo_json_last_error_code()==13);
    assert(!sengoo_json_parse_text_bounded(1,1,0) && sengoo_json_last_error_code()==2);
    assert(!sengoo_json_parse_text_bounded(1,1,16777217) && sengoo_json_last_error_code()==2);
    assert(!sengoo_json_parse_buffer_bounded(1,10,9) && sengoo_json_last_error_code()==13);
    test_buffer.bytes=(unsigned char*)(intptr_t)1; test_buffer.capacity=10; test_buffer.used_len=0;
    assert(!sengoo_json_parse_buffer_bounded(700,1,10) && sengoo_json_last_error_code()==2);
    long long doc=sengoo_json_parse_text_bounded(raw(text),size,size); assert(doc>0);
    long long root=sengoo_json_doc_root(doc); assert(root>0);
    long long encoded=sengoo_json_doc_serialize_string(doc); assert(encoded>0 && strlen((char*)(intptr_t)encoded)==size); free((void*)(intptr_t)encoded);
    long long small=sengoo_json_doc_object_new_bounded(32); assert(small>0);
    size_t original=sengoo_json_doc_from_handle(small)->len;
    for(int i=0;i<32;i++) {
        assert(!sengoo_json_doc_copy(small,doc,root) && sengoo_json_last_error_code()==13);
        SengooJsonDoc* checked=sengoo_json_doc_from_handle(small); assert(checked->len==original && !checked->owned_payload && !checked->owned_edges);
    }
    assert(!sengoo_json_doc_new_string_bytes(small,1,33) && sengoo_json_last_error_code()==13);
    long long nul=sengoo_json_doc_new_string_bytes(small,raw("x\0y"),3); assert(nul>0);
    assert(sengoo_json_object_set(small,sengoo_json_doc_root(small),raw("n"),small,nul)==1);
    encoded=sengoo_json_doc_serialize_string(small); assert(encoded>0 && !strcmp((char*)(intptr_t)encoded,"{\"n\":\"x\\u0000y\"}")); free((void*)(intptr_t)encoded); close_doc(small);
    long long destination=sengoo_json_doc_object_new_bounded(size+32); assert(destination>0);
    long long copy=sengoo_json_doc_copy(destination,doc,root); assert(copy>0); close_doc(doc);
    assert(sengoo_json_object_set(destination,sengoo_json_doc_root(destination),raw("v"),destination,copy)==1);
    encoded=sengoo_json_doc_serialize_string(destination); assert(encoded>0 && strlen((char*)(intptr_t)encoded)==size+6); free((void*)(intptr_t)encoded); close_doc(destination);
    long long over=sengoo_json_doc_object_new_bounded(4); assert(over>0);
    long long value=sengoo_json_doc_new_string_bytes(over,raw("xxxx"),4); assert(value>0);
    assert(sengoo_json_object_set(over,sengoo_json_doc_root(over),raw(""),over,value)==1);
    assert(sengoo_json_doc_serialize_string(over)==-13);
    test_buffer.bytes=(unsigned char*)text; test_buffer.capacity=100; test_buffer.used_len=17;
    assert(sengoo_json_doc_serialize(over,700)==-13 && test_buffer.used_len==17); close_doc(over);
    long long cyclic=sengoo_json_doc_object_new_bounded(128); assert(cyclic>0);
    assert(sengoo_json_object_set(cyclic,sengoo_json_doc_root(cyclic),raw("self"),cyclic,sengoo_json_doc_root(cyclic))==1);
    assert(sengoo_json_doc_serialize_string(cyclic)==-2); close_doc(cyclic);
    for(int i=0;i<32;i++) {
        assert(!sengoo_json_parse_text_bounded(raw("{\""),2,128));
        assert(!sengoo_json_bounded_live_documents() && !sengoo_json_bounded_reserved_bytes());
    }
    long long edges=sengoo_json_doc_object_new_bounded(65536); assert(edges>0);
    long long array=sengoo_json_doc_new_array(edges); value=sengoo_json_doc_new_null(edges); assert(array>0 && value>0);
    for(int i=0;i<4096;i++) assert(sengoo_json_array_push(edges,array,edges,value)==1);
    assert(sengoo_json_array_push(edges,array,edges,value)==-13); close_doc(edges);
    memset(text,'0',size); text[1]='.';
    doc=sengoo_json_parse_text_bounded(raw(text),size,size); assert(doc>0);
    destination=sengoo_json_doc_object_new_bounded(size+2); assert(destination>0);
    assert(sengoo_json_doc_copy(destination,doc,sengoo_json_doc_root(doc))>0);
    original=sengoo_json_doc_from_handle(destination)->len;
    assert(!sengoo_json_doc_copy(destination,doc,sengoo_json_doc_root(doc)) && sengoo_json_last_error_code()==13);
    assert(sengoo_json_doc_from_handle(destination)->len==original && sengoo_json_doc_from_handle(destination)->owned_payload==size);
    close_doc(destination); close_doc(doc);
    memset(text,'a',SENGOO_JSON_BOUNDED_MAX_BYTES); text[0]='"'; text[SENGOO_JSON_BOUNDED_MAX_BYTES-1]='"';
    doc=sengoo_json_parse_text_bounded(raw(text),SENGOO_JSON_BOUNDED_MAX_BYTES,SENGOO_JSON_BOUNDED_MAX_BYTES); assert(doc>0); close_doc(doc);
    long long owners[64]; int count=0;
    while(count<64) { long long owner=sengoo_json_doc_object_new_bounded(16777216); if(!owner) break; owners[count++]=owner; }
    assert(count==5 && sengoo_json_last_error_code()==13);
    for(int i=0;i<count;i++) close_doc(owners[i]);
    for(int i=0;i<64;i++) { owners[i]=sengoo_json_doc_object_new_bounded(1); assert(owners[i]>0); }
    assert(!sengoo_json_doc_object_new_bounded(1) && sengoo_json_last_error_code()==13);
    for(int i=0;i<64;i++) close_doc(owners[i]);
    assert(!sengoo_json_bounded_live_documents() && !sengoo_json_bounded_reserved_bytes());
    /* Many small nodes hit the old node cap far below the byte cap. */
    assert(!sengoo_json_parse_text_limits(1,1,128,0) && sengoo_json_last_error_code()==2);
    assert(!sengoo_json_parse_text_limits(1,1,128,65537) && sengoo_json_last_error_code()==2);
    assert(!sengoo_json_parse_text_limits(1,129,128,65536) && sengoo_json_last_error_code()==13);
    assert(!sengoo_json_parse_buffer_limits(1,129,128,65536) && sengoo_json_last_error_code()==13);
    assert(!sengoo_json_parse_text_limits(raw("[}"),2,128,65536) && sengoo_json_last_error_code()==10);
    size_t offset=0; text[offset++]='[';
    for(int i=0;i<5000;i++) { if(i) text[offset++]=','; memcpy(text+offset,"{\"n\":1}",7); offset+=7; }
    text[offset++]=']'; text[offset]=0;
    assert(!sengoo_json_parse_text(raw(text),offset) && sengoo_json_last_error_code()==10);
    assert(!sengoo_json_parse_text_bounded(raw(text),offset,offset) && sengoo_json_last_error_code()==10);
    assert(!sengoo_json_parse_text_limits(raw(text),offset,offset,4096) && sengoo_json_last_error_code()==13);
    doc=sengoo_json_parse_text_limits(raw(text),offset,offset,32768); assert(doc>0);
    assert(sengoo_json_array_len(doc,sengoo_json_doc_root(doc))==5000);
    destination=sengoo_json_doc_object_new_limits(offset+64,32768); assert(destination>0);
    copy=sengoo_json_doc_copy(destination,doc,sengoo_json_doc_root(doc)); assert(copy>0);
    assert(sengoo_json_object_set(destination,sengoo_json_doc_root(destination),raw("rows"),destination,copy)==1);
    encoded=sengoo_json_doc_serialize_string(destination); assert(encoded>0 && strlen((char*)(intptr_t)encoded)==offset+9); free((void*)(intptr_t)encoded);
    small=sengoo_json_doc_object_new_limits(offset+64,4096); assert(small>0);
    original=sengoo_json_doc_from_handle(small)->len;
    for(int i=0;i<3;i++) {
        assert(!sengoo_json_doc_copy(small,doc,sengoo_json_doc_root(doc)) && sengoo_json_last_error_code()==13);
        SengooJsonDoc* checked=sengoo_json_doc_from_handle(small);
        assert(checked->len==original && !checked->owned_payload && !checked->owned_edges);
    }
    close_doc(small); close_doc(destination); close_doc(doc);
    test_buffer.bytes=(unsigned char*)text; test_buffer.capacity=offset; test_buffer.used_len=offset;
    doc=sengoo_json_parse_buffer_limits(700,offset,offset,32768); assert(doc>0); close_doc(doc);
    edges=sengoo_json_doc_object_new_limits(128,5002); assert(edges>0);
    array=sengoo_json_doc_new_array(edges); value=sengoo_json_doc_new_null(edges);
    for(int i=0;i<5002;i++) assert(sengoo_json_array_push(edges,array,edges,value)==1);
    assert(sengoo_json_array_push(edges,array,edges,value)==-13); close_doc(edges);
    assert(!sengoo_json_bounded_live_documents() && !sengoo_json_bounded_reserved_bytes());
    for(int i=1;i<256;i++) assert(!handles[i]);
    free(text); puts("bounded JSON native: exact16MiB, pre-read rejection, copy rollback, NUL, cycles, links and aggregate release passed"); return 0;
}
