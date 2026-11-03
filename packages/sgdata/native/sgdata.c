#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <limits.h>
#ifdef _WIN32
#include <windows.h>
#endif

extern long long sengoo_string_from_bytes_copy(long long, long long);
enum { DATA_INVALID=2, DATA_HANDLE=3, DATA_LIMIT=4, DATA_NOT_FOUND=5,
       DATA_IO=9, DATA_PARSE=10, DATA_OVERFLOW=13, DATA_OOM=14, DATA_UTF8=20 };
static long long data_error;
static char data_message[160];
static long long data_fail(long long code, const char *message) {
    data_error=code; snprintf(data_message,sizeof(data_message),"%s",message); return -code;
}
long long sgdata_error_code(void) { return data_error; }
long long sgdata_error_message(void) {
    return sengoo_string_from_bytes_copy((long long)(intptr_t)data_message,(long long)strlen(data_message));
}
static int data_utf8(const unsigned char *s, size_t n) {
    size_t i=0;
    while (i<n) {
        unsigned c=s[i++];
        if (c<128) continue;
        unsigned count, low=128, high=191;
        if (c>=194 && c<=223) count=1;
        else if (c>=224 && c<=239) { count=2; if(c==224)low=160; if(c==237)high=159; }
        else if (c>=240 && c<=244) { count=3; if(c==240)low=144; if(c==244)high=143; }
        else return 0;
        if (n-i<count || s[i]<low || s[i]>high) return 0;
        ++i;
        while (--count) { if(s[i]<128 || s[i]>191)return 0; ++i; }
    }
    return 1;
}
static FILE *data_open(const char *path, int writing) {
    if (!path || !*path) return NULL;
#ifdef _WIN32
    int n=MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,path,-1,NULL,0);
    if (!n) return NULL;
    wchar_t *wide=(wchar_t*)malloc((size_t)n*sizeof(wchar_t));
    if (!wide) return NULL;
    MultiByteToWideChar(CP_UTF8,MB_ERR_INVALID_CHARS,path,-1,wide,n);
    FILE *f=_wfopen(wide,writing?L"wb":L"rb"); free(wide); return f;
#else
    return fopen(path,writing?"wb":"rb");
#endif
}
long long sgdata_read_text(long long path, long long maximum) {
    data_error=0;
    if(maximum<0 || (uint64_t)maximum>SIZE_MAX-1) return data_fail(DATA_INVALID,"invalid maximum input byte count");
    FILE *f=data_open((const char*)(intptr_t)path,0);
    if(!f)return data_fail(DATA_IO,"cannot open UTF-8 input file");
    size_t cap=4096, used=0, limit=(size_t)maximum;
    if(cap>limit+1)cap=limit+1;
    char *bytes=(char*)malloc(cap?cap:1);
    if(!bytes){fclose(f);return data_fail(DATA_OOM,"input allocation failed");}
    for(;;) {
        size_t got=fread(bytes+used,1,cap-used,f); used+=got;
        if(used>limit){free(bytes);fclose(f);return data_fail(DATA_LIMIT,"file exceeds maximum input bytes");}
        if(ferror(f)){free(bytes);fclose(f);return data_fail(DATA_IO,"file read failed");}
        if(feof(f))break;
        if(used==cap) {
            size_t next=cap>SIZE_MAX/2?limit+1:cap*2;
            if(next>limit+1)next=limit+1;
            char *grown=(char*)realloc(bytes,next);
            if(!grown){free(bytes);fclose(f);return data_fail(DATA_OOM,"input allocation failed");}
            bytes=grown;cap=next;
        }
    }
    fclose(f);
    if(!data_utf8((const unsigned char*)bytes,used)){free(bytes);return data_fail(DATA_UTF8,"input is not valid UTF-8");}
    /* Borrowed Sengoo &str is NUL-terminated. Refuse bytes that would be
       silently truncated by csv_read/json_read or subsequent as_str calls. */
    if(used && memchr(bytes,0,used)){free(bytes);return data_fail(DATA_PARSE,"input contains NUL; UTF-8 text files must not contain NUL bytes");}
    long long result=sengoo_string_from_bytes_copy((long long)(intptr_t)bytes,(long long)used);
    free(bytes); return result;
}
long long sgdata_write_text(long long path, long long bytes, long long length) {
    data_error=0;
    if(length<0 || (length && !bytes))return data_fail(DATA_INVALID,"invalid text input");
    if(!data_utf8((const unsigned char*)(intptr_t)bytes,(size_t)length))return data_fail(DATA_UTF8,"output is not valid UTF-8");
    FILE *f=data_open((const char*)(intptr_t)path,1);
    if(!f)return data_fail(DATA_IO,"cannot open output file");
    int failed=fwrite((const void*)(intptr_t)bytes,1,(size_t)length,f)!=(size_t)length;
    if(fclose(f)!=0)failed=1;
    return failed?data_fail(DATA_IO,"file write failed"):0;
}

typedef struct DataCsv {
    long long id;
    size_t columns, rows, count, capacity;
    char **fields;
    struct DataCsv *next;
} DataCsv;
static DataCsv *data_csvs;
static long long data_next_id=1;
static DataCsv *data_csv(long long id) {
    for(DataCsv *t=data_csvs;t;t=t->next)if(t->id==id)return t;
    data_fail(DATA_HANDLE,"CSV handle is closed or invalid");return NULL;
}
static void data_destroy(DataCsv *t) {
    for(size_t i=0;i<t->count;++i)free(t->fields[i]);free(t->fields);free(t);
}
static int data_field(DataCsv *t,const char *s,size_t n) {
    if(t->count==t->capacity) {
        size_t cap=t->capacity?t->capacity*2:16;
        if(cap>SIZE_MAX/sizeof(char*))return 0;
        char **p=(char**)realloc(t->fields,cap*sizeof(char*));if(!p)return 0;
        t->fields=p;t->capacity=cap;
    }
    char *p=(char*)malloc(n+1);if(!p)return 0;
    memcpy(p,s,n);p[n]=0;t->fields[t->count++]=p;return 1;
}
static long long data_register(DataCsv *t) {
    if(data_next_id==LLONG_MAX){data_destroy(t);return data_fail(DATA_OVERFLOW,"CSV handle identities exhausted");}
    t->id=data_next_id++;t->next=data_csvs;data_csvs=t;return t->id;
}
long long sgdata_csv_new(long long columns) {
    data_error=0;
    if(columns<=0)return data_fail(DATA_INVALID,"CSV must have at least one column");
    DataCsv *t=(DataCsv*)calloc(1,sizeof(*t));if(!t)return data_fail(DATA_OOM,"CSV allocation failed");
    t->columns=(size_t)columns;return data_register(t);
}
long long sgdata_csv_close(long long id) {
    DataCsv **p=&data_csvs;
    while(*p && (*p)->id!=id)p=&(*p)->next;
    if(!*p)return -DATA_HANDLE;
    DataCsv *t=*p;*p=t->next;data_destroy(t);return 0;
}
long long sgdata_csv_parse(long long bytes,long long length) {
    data_error=0;
    if(length<0 || (length && !bytes))return data_fail(DATA_INVALID,"invalid CSV input");
    const char *s=(const char*)(intptr_t)bytes;size_t n=(size_t)length;
    if(!data_utf8((const unsigned char*)s,n))return data_fail(DATA_UTF8,"CSV is not valid UTF-8");
    if(n && memchr(s,0,n))return data_fail(DATA_PARSE,"CSV contains NUL");
    DataCsv *t=(DataCsv*)calloc(1,sizeof(*t));char *field=(char*)malloc(n+1);
    if(!t || !field){free(t);free(field);return data_fail(DATA_OOM,"CSV allocation failed");}
    size_t i=0,row_start=0,column=1;
    while(i<n) {
        column=t->count-row_start+1;
        size_t used=0;
        if(s[i]=='"') {
            ++i;int closed=0;
            while(i<n) {
                char c=s[i++];
                if(c=='"') {
                    if(i<n && s[i]=='"'){field[used++]='"';++i;}
                    else {closed=1;break;}
                } else field[used++]=c;
            }
            if(!closed || (i<n && s[i]!=',' && s[i]!='\r' && s[i]!='\n'))goto syntax;
        } else {
            while(i<n && s[i]!=',' && s[i]!='\r' && s[i]!='\n') {
                if(s[i]=='"')goto syntax;
                field[used++]=s[i++];
            }
        }
        if(!data_field(t,field,used))goto oom;
        if(i<n && s[i]==',') {
            ++i;
            if(i==n && !data_field(t,"",0))goto oom;
            else if(i<n)continue;
        } else if(i<n) {
            if(s[i]=='\r') { ++i;if(i>=n || s[i]!='\n')goto syntax; }
            ++i;
        }
        size_t cols=t->count-row_start;
        if(t->rows==0)t->columns=cols;
        else if(t->columns!=cols)goto columns;
        ++t->rows;row_start=t->count;
    }
    free(field);return data_register(t);
syntax:
    {
        char detail[160];
        snprintf(detail,sizeof(detail),"CSV record %zu, field %zu, byte offset %zu: malformed quoting or line ending",t->rows+1,column,i);
        free(field);data_destroy(t);return data_fail(DATA_PARSE,detail);
    }
columns:
    {
        char detail[160];
        snprintf(detail,sizeof(detail),"CSV record %zu: expected %zu columns, found %zu",t->rows+1,t->columns,t->count-row_start);
        free(field);data_destroy(t);return data_fail(DATA_PARSE,detail);
    }
oom:
    free(field);data_destroy(t);return data_fail(DATA_OOM,"CSV allocation failed");
}
long long sgdata_csv_rows(long long id) { DataCsv *t=data_csv(id);return t?(long long)t->rows:-DATA_HANDLE; }
long long sgdata_csv_columns(long long id) { DataCsv *t=data_csv(id);return t?(long long)t->columns:-DATA_HANDLE; }
long long sgdata_csv_field(long long id,long long row,long long col) {
    DataCsv *t=data_csv(id);if(!t)return -DATA_HANDLE;
    if(row<0 || col<0 || (size_t)row>=t->rows || (size_t)col>=t->columns)return data_fail(DATA_NOT_FOUND,"CSV field is out of bounds");
    const char *s=t->fields[(size_t)row*t->columns+(size_t)col];
    return sengoo_string_from_bytes_copy((long long)(intptr_t)s,(long long)strlen(s));
}
/* Rows are accumulated privately and become visible only when complete. */
long long sgdata_csv_push_field(long long id,long long bytes,long long length) {
    DataCsv *t=data_csv(id);if(!t)return -DATA_HANDLE;
    if(length<0 || (length && !bytes))return data_fail(DATA_INVALID,"invalid CSV field");
    const char *s=(const char*)(intptr_t)bytes;
    if(!data_utf8((const unsigned char*)s,(size_t)length))return data_fail(DATA_UTF8,"CSV field is not valid UTF-8");
    if(length && memchr(s,0,(size_t)length))return data_fail(DATA_PARSE,"CSV field contains NUL");
    if(t->count-t->rows*t->columns>=t->columns)return data_fail(DATA_INVALID,"CSV pending row is full");
    if(!data_field(t,s,(size_t)length))return data_fail(DATA_OOM,"CSV field allocation failed");
    if(t->count%t->columns==0)++t->rows;
    return 0;
}
long long sgdata_csv_serialize(long long id) {
    DataCsv *t=data_csv(id);if(!t)return -DATA_HANDLE;
    if(t->count!=t->rows*t->columns)return data_fail(DATA_INVALID,"CSV has an incomplete pending row");
    size_t cap=1;
    for(size_t i=0;i<t->count;++i) {
        size_t n=strlen(t->fields[i]);if(n>(SIZE_MAX-cap-4)/2)return data_fail(DATA_OVERFLOW,"CSV output is too large");cap+=n*2+4;
    }
    char *out=(char*)malloc(cap);if(!out)return data_fail(DATA_OOM,"CSV output allocation failed");
    size_t used=0;
    for(size_t r=0;r<t->rows;++r) {
        for(size_t c=0;c<t->columns;++c) {
            if(c)out[used++]=',';
            const char *s=t->fields[r*t->columns+c];int quote=strpbrk(s,",\r\n\"")!=NULL || !*s;
            if(quote)out[used++]='"';
            for(;*s;++s){out[used++]=*s;if(*s=='"')out[used++]='"';}
            if(quote)out[used++]='"';
        }
        out[used++]='\r';out[used++]='\n';
    }
    long long result=sengoo_string_from_bytes_copy((long long)(intptr_t)out,(long long)used);free(out);return result;
}
long long sgdata_live_handles(void) { long long n=0;for(DataCsv *t=data_csvs;t;t=t->next)++n;return n; }
