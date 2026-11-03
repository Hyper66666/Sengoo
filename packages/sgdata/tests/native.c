#include "native_runtime.h"
#include "../native/sgdata.c"
int main(void) {
    const char *input="name,note\r\n\"Alice, A\",\"line one\nline \"\"two\"\"\"\r\nBob,last\n";
    long long t=sgdata_csv_parse(test_pointer(input),(long long)strlen(input));assert(t>0);
    assert(sgdata_csv_rows(t)==3 && sgdata_csv_columns(t)==2);
    test_string(sgdata_csv_field(t,1,0),"Alice, A");test_string(sgdata_csv_field(t,1,1),"line one\nline \"two\"");
    long long serialized=sgdata_csv_serialize(t);assert(serialized>0);
    long long other=sgdata_csv_parse(serialized,(long long)strlen((const char*)(intptr_t)serialized));assert(other>0);
    free((void*)(intptr_t)serialized);assert(sgdata_csv_rows(other)==3);
    assert(sgdata_csv_close(t)==0 && sgdata_csv_close(other)==0);
    assert(sgdata_csv_parse(test_pointer("a,b\nc"),5)==-10);
    assert(strstr(data_message,"record 2: expected 2 columns, found 1"));
    assert(sgdata_csv_parse(test_pointer("\"unclosed"),9)==-10);
    assert(strstr(data_message,"record 1, field 1, byte offset 9"));
    const char *broken="name,note\nAlice,first\nBob,\"unclosed";
    assert(sgdata_csv_parse(test_pointer(broken),(long long)strlen(broken))==-10);
    assert(strstr(data_message,"record 3, field 2, byte offset"));
    assert(sgdata_csv_parse(test_pointer("a\"b"),3)==-10);
    assert(sgdata_csv_parse(test_pointer("a\rb"),3)==-10);
    assert(sgdata_csv_parse(test_pointer("\xff"),1)==-20);
    t=sgdata_csv_parse(test_pointer("a,"),2);assert(t>0);assert(sgdata_csv_columns(t)==2);test_string(sgdata_csv_field(t,0,1),"");sgdata_csv_close(t);
    t=sgdata_csv_parse(test_pointer("\n"),1);assert(t>0);assert(sgdata_csv_rows(t)==1&&sgdata_csv_columns(t)==1);sgdata_csv_close(t);
    t=sgdata_csv_parse(test_pointer(""),0);assert(t>0);assert(sgdata_csv_rows(t)==0);sgdata_csv_close(t);
    t=sgdata_csv_new(2);assert(t>0);
    assert(sgdata_csv_push_field(t,test_pointer("quote\""),6)==0);assert(sgdata_csv_serialize(t)==-2);
    assert(sgdata_csv_push_field(t,test_pointer(""),0)==0);test_string(sgdata_csv_serialize(t),"\"quote\"\"\",\"\"\r\n");sgdata_csv_close(t);
    const char *path="sgdata-native-text.tmp";const char *text="hello \xe4\xb8\x96\xe7\x95\x8c\n";
    assert(sgdata_write_text(test_pointer(path),test_pointer(text),(long long)strlen(text))==0);
    test_string(sgdata_read_text(test_pointer(path),(long long)strlen(text)),text);
    assert(sgdata_read_text(test_pointer(path),2)==-4);assert(sgdata_read_text(test_pointer(path),-1)==-2);
    FILE *f=fopen(path,"wb");assert(f);fputc(0xff,f);fclose(f);assert(sgdata_read_text(test_pointer(path),10)==-20);remove(path);
    f=fopen(path,"wb");assert(f);assert(fwrite("a\0b",1,3,f)==3);fclose(f);
    assert(sgdata_read_text(test_pointer(path),10)==-10);assert(strstr(data_message,"input contains NUL"));remove(path);
    assert(sgdata_live_handles()==0);puts("sgdata native: passed");return 0;
}
