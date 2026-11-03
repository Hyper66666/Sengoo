#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <errno.h>
#include <limits.h>
#ifdef _WIN32
#include <windows.h>
#endif

extern long long sengoo_string_from_bytes_copy(long long,long long);
extern long long sengoo_args_len(void);
extern long long sengoo_arg_len(long long);
extern long long sengoo_arg_copy(long long,long long,long long);

typedef struct CliOption { char *name,*short_name,*scope,*help; int kind,repeat,required; } CliOption;
typedef struct CliCommand { char *name,*help; } CliCommand;
typedef struct CliApp {
    long long id;char *name,*version,*description;char error[256];
    CliOption *options;size_t noptions;
    CliCommand *commands;size_t ncommands;long long minpos,maxpos;
    struct CliApp *next;
} CliApp;
typedef struct CliValue { char *name,*text;int kind;long long integer; } CliValue;
typedef struct CliMatches {
    long long id;char **args;size_t nargs;
    CliValue *values;size_t nvalues;
    char **positions;size_t npositions;char *command;int parsed;
    struct CliMatches *next;
} CliMatches;
static CliApp *cli_apps;
static CliMatches *cli_matches;
static long long cli_next=1;
static char *cli_copy(const char *s) {
    if(!s)s="";size_t n=strlen(s)+1;char *p=(char*)malloc(n);if(p)memcpy(p,s,n);return p;
}
static CliApp *cli_app(long long id) { for(CliApp *a=cli_apps;a;a=a->next)if(a->id==id)return a;return NULL; }
static CliMatches *cli_match(long long id) { for(CliMatches *m=cli_matches;m;m=m->next)if(m->id==id)return m;return NULL; }
static long long cli_fail(CliApp *a,const char *message,const char *argument) {
    if(a)snprintf(a->error,sizeof(a->error),"%s%s%s",message,argument?": ":"",argument?argument:"");return -2;
}
static int cli_name(const char *s) {
    if(!s || !*s || *s=='-')return 0;
    for(;*s;++s)if(!((*s>='a'&&*s<='z')||(*s>='A'&&*s<='Z')||(*s>='0'&&*s<='9')||*s=='-'))return 0;
    return 1;
}
static long long cli_owned_string(const char *s) { return sengoo_string_from_bytes_copy((long long)(intptr_t)s,(long long)strlen(s)); }
long long sgcli_new(long long name,long long version,long long description) {
    CliApp *a=(CliApp*)calloc(1,sizeof(*a));if(!a)return -14;
    a->name=cli_copy((const char*)(intptr_t)name);a->version=cli_copy((const char*)(intptr_t)version);
    a->description=cli_copy((const char*)(intptr_t)description);a->maxpos=LLONG_MAX;
    if(!a->name||!a->version||!a->description){free(a->name);free(a->version);free(a->description);free(a);return -14;}
    if(cli_next==LLONG_MAX){free(a->name);free(a->version);free(a->description);free(a);return -13;}
    a->id=cli_next++;a->next=cli_apps;cli_apps=a;return a->id;
}
long long sgcli_close(long long id) {
    CliApp **p=&cli_apps;while(*p&&(*p)->id!=id)p=&(*p)->next;if(!*p)return -3;
    CliApp *a=*p;*p=a->next;
    for(size_t i=0;i<a->noptions;++i){CliOption *o=&a->options[i];free(o->name);free(o->short_name);free(o->scope);free(o->help);}
    for(size_t i=0;i<a->ncommands;++i){free(a->commands[i].name);free(a->commands[i].help);}
    free(a->name);free(a->version);free(a->description);free(a->options);free(a->commands);free(a);return 0;
}
long long sgcli_command(long long id,long long name,long long help) {
    CliApp *a=cli_app(id);if(!a)return -3;const char *s=(const char*)(intptr_t)name;
    if(!cli_name(s))return cli_fail(a,"invalid subcommand name",s);
    for(size_t i=0;i<a->ncommands;++i)if(!strcmp(a->commands[i].name,s))return cli_fail(a,"duplicate subcommand",s);
    char *n=cli_copy(s),*h=cli_copy((const char*)(intptr_t)help);
    if(!n||!h){free(n);free(h);return -14;}
    CliCommand *p=(CliCommand*)realloc(a->commands,(a->ncommands+1)*sizeof(*p));
    if(!p){free(n);free(h);return -14;}a->commands=p;p[a->ncommands++]=(CliCommand){n,h};return 0;
}
long long sgcli_option(long long id,long long scope,long long name,long long short_name,long long kind,long long repeat,long long required,long long help) {
    CliApp *a=cli_app(id);if(!a)return -3;
    const char *sc=(const char*)(intptr_t)scope,*n=(const char*)(intptr_t)name,*s=(const char*)(intptr_t)short_name;
    if(!sc||!cli_name(n)||!s||strlen(s)>1||(*s&&!cli_name(s))||kind<0||kind>2)return cli_fail(a,"invalid option registration",n);
    if(!strcmp(n,"help")||!strcmp(n,"version")||!strcmp(s,"h"))return cli_fail(a,"reserved built-in option",n);
    if(*sc){int found=0;for(size_t i=0;i<a->ncommands;++i)if(!strcmp(a->commands[i].name,sc))found=1;if(!found)return cli_fail(a,"unknown subcommand",sc);}
    for(size_t i=0;i<a->noptions;++i){CliOption *o=&a->options[i];
        if((!*sc||!*o->scope||!strcmp(sc,o->scope)) && (!strcmp(n,o->name)||(*s&&!strcmp(s,o->short_name))))return cli_fail(a,"duplicate option",n);
    }
    CliOption o={cli_copy(n),cli_copy(s),cli_copy(sc),cli_copy((const char*)(intptr_t)help),(int)kind,repeat!=0,required!=0};
    if(!o.name||!o.short_name||!o.scope||!o.help){free(o.name);free(o.short_name);free(o.scope);free(o.help);return -14;}
    CliOption *p=(CliOption*)realloc(a->options,(a->noptions+1)*sizeof(*p));
    if(!p){free(o.name);free(o.short_name);free(o.scope);free(o.help);return -14;}
    a->options=p;p[a->noptions++]=o;return 0;
}
long long sgcli_positionals(long long id,long long minimum,long long maximum) {
    CliApp *a=cli_app(id);if(!a)return -3;
    if(minimum<0||maximum<minimum)return cli_fail(a,"invalid positional bounds",NULL);
    a->minpos=minimum;a->maxpos=maximum;return 0;
}
long long sgcli_error(long long id) { CliApp *a=cli_app(id);return a?cli_owned_string(a->error):-3; }
long long sgcli_matches_new(void) {
    CliMatches *m=(CliMatches*)calloc(1,sizeof(*m));if(!m)return -14;
    if(cli_next==LLONG_MAX){free(m);return -13;}m->id=cli_next++;m->next=cli_matches;cli_matches=m;return m->id;
}
long long sgcli_matches_close(long long id) {
    CliMatches **p=&cli_matches;while(*p&&(*p)->id!=id)p=&(*p)->next;if(!*p)return -3;
    CliMatches *m=*p;*p=m->next;
    for(size_t i=0;i<m->nargs;++i)free(m->args[i]);
    for(size_t i=0;i<m->nvalues;++i){free(m->values[i].name);free(m->values[i].text);}
    for(size_t i=0;i<m->npositions;++i)free(m->positions[i]);
    free(m->args);free(m->values);free(m->positions);free(m->command);free(m);return 0;
}
long long sgcli_argument(long long id,long long value) {
    CliMatches *m=cli_match(id);if(!m)return -3;if(m->parsed)return -2;
    char *v=cli_copy((const char*)(intptr_t)value);if(!v)return -14;
    char **p=(char**)realloc(m->args,(m->nargs+1)*sizeof(*p));if(!p){free(v);return -14;}
    m->args=p;p[m->nargs++]=v;return 0;
}
long long sgcli_process_arguments(long long id) {
#ifdef _WIN32
    /* The C main argv uses the active ANSI code page. Read the OS wide command
     * line so CLI values preserve Unicode regardless of the user's locale. */
    HMODULE shell=LoadLibraryExW(L"shell32.dll",NULL,LOAD_LIBRARY_SEARCH_SYSTEM32);
    if(!shell)return -9;
    typedef LPWSTR *(WINAPI *CliWideArguments)(LPCWSTR,int *);
    CliWideArguments parse=(CliWideArguments)(void*)GetProcAddress(shell,"CommandLineToArgvW");
    if(!parse){FreeLibrary(shell);return -8;}
    int count=0;LPWSTR *arguments=parse(GetCommandLineW(),&count);
    if(!arguments){FreeLibrary(shell);return -14;}
    long long result=0;
    for(int i=1;i<count;++i) {
        int bytes=WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,arguments[i],-1,NULL,0,NULL,NULL);
        if(!bytes){result=-20;break;}
        char *text=(char*)malloc((size_t)bytes);
        if(!text){result=-14;break;}
        if(!WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,arguments[i],-1,text,bytes,NULL,NULL)){free(text);result=-20;break;}
        result=sgcli_argument(id,(long long)(intptr_t)text);free(text);
        if(result<0)break;
    }
    LocalFree(arguments);FreeLibrary(shell);return result;
#else
    for(long long i=0;i<sengoo_args_len();++i) {
        long long n=sengoo_arg_len(i);if(n<0)return n;
        char *s=(char*)malloc((size_t)n+1);if(!s)return -14;
        long long got=sengoo_arg_copy(i,(long long)(intptr_t)s,n);if(got<0){free(s);return got;}
        s[n]=0;long long r=sgcli_argument(id,(long long)(intptr_t)s);free(s);if(r<0)return r;
    }return 0;
#endif
}
static long long cli_count(CliMatches *m,const char *name) {long long n=0;for(size_t i=0;i<m->nvalues;++i)if(!strcmp(m->values[i].name,name))++n;return n;}
static int cli_value(CliMatches *m,const char *name,const char *text,int kind,long long integer) {
    CliValue v={cli_copy(name),cli_copy(text),kind,integer};if(!v.name||!v.text){free(v.name);free(v.text);return 0;}
    CliValue *p=(CliValue*)realloc(m->values,(m->nvalues+1)*sizeof(*p));if(!p){free(v.name);free(v.text);return 0;}
    m->values=p;p[m->nvalues++]=v;return 1;
}
static int cli_integer(const char *s,long long *out) {
    if(!s||!*s)return 0;const char *p=s;if(*p=='-'||*p=='+')++p;
    if(!*p)return 0;for(;*p;++p)if(*p<'0'||*p>'9')return 0;
    errno=0;char *end;long long v=strtoll(s,&end,10);if(errno==ERANGE||*end)return 0;*out=v;return 1;
}
long long sgcli_parse(long long id,long long mid) {
    CliApp *a=cli_app(id);CliMatches *m=cli_match(mid);if(!a||!m)return -3;
    if(m->parsed)return cli_fail(a,"arguments already parsed",NULL);m->parsed=1;a->error[0]=0;
    int positional=0;
    for(size_t i=0;i<m->nargs;++i) {
        const char *arg=m->args[i];
        if(!positional&&!strcmp(arg,"--")){positional=1;continue;}
        if(!positional&&(!strcmp(arg,"--help")||!strcmp(arg,"-h")||!strcmp(arg,"--version"))) {
            const char *name=!strcmp(arg,"--version")?"version":"help";
            if(cli_count(m,name))return cli_fail(a,"duplicate single-value option",arg);
            if(!cli_value(m,name,"",0,1))return -14;continue;
        }
        long long ignored;
        if(!positional&&arg[0]=='-'&&arg[1]&&!cli_integer(arg,&ignored)) {
            const char *start=arg+(arg[1]=='-'?2:1),*equal=strchr(start,'=');size_t len=equal?(size_t)(equal-start):strlen(start);
            CliOption *option=NULL;
            for(size_t j=0;j<a->noptions;++j){CliOption *o=&a->options[j];
                if(*o->scope&&(!m->command||strcmp(o->scope,m->command)))continue;
                const char *candidate=arg[1]=='-'?o->name:o->short_name;
                if(*candidate&&strlen(candidate)==len&&!strncmp(candidate,start,len)){option=o;break;}
            }
            if(!option)return cli_fail(a,"unknown option",arg);
            if(!option->repeat&&cli_count(m,option->name))return cli_fail(a,"duplicate single-value option",arg);
            const char *text="";long long value=1;
            if(option->kind==0){if(equal)return cli_fail(a,"boolean flag does not take a value",arg);}
            else {
                if(equal)text=equal+1;
                else {
                    if(i+1>=m->nargs)return cli_fail(a,"missing option value",arg);
                    text=m->args[++i];
                    if(!strcmp(text,"--")||(option->kind==1&&text[0]=='-'&&text[1]))return cli_fail(a,"missing option value",arg);
                }
                if(option->kind==2&&!cli_integer(text,&value))return cli_fail(a,"invalid or out-of-range i64 option",arg);
            }
            if(!cli_value(m,option->name,text,option->kind,value))return -14;
        } else {
            int command=0;
            if(!positional&&!m->command&&m->npositions==0) {
                for(size_t j=0;j<a->ncommands;++j)if(!strcmp(a->commands[j].name,arg)){m->command=cli_copy(arg);if(!m->command)return -14;command=1;break;}
                if(a->ncommands&&!command)return cli_fail(a,"unknown subcommand",arg);
            }
            if(!command){
                char *v=cli_copy(arg);if(!v)return -14;
                char **p=(char**)realloc(m->positions,(m->npositions+1)*sizeof(*p));if(!p){free(v);return -14;}
                m->positions=p;p[m->npositions++]=v;
            }
        }
    }
    if(cli_count(m,"help")||cli_count(m,"version"))return 0;
    for(size_t j=0;j<a->noptions;++j){CliOption *o=&a->options[j];
        if(o->required&&(!*o->scope||(m->command&&!strcmp(m->command,o->scope)))&&!cli_count(m,o->name))return cli_fail(a,"missing required option",o->name);
    }
    if(m->npositions<(uint64_t)a->minpos||m->npositions>(uint64_t)a->maxpos)return cli_fail(a,"incorrect positional argument count",NULL);
    return 0;
}
/* Publish an owning result only after construction and parsing have succeeded. */
long long sgcli_parse_owned(long long id,long long mid) {
    long long status=sgcli_parse(id,mid);
    if(status<0){sgcli_matches_close(mid);return status;}
    return mid;
}
long long sgcli_parse_process(long long id,long long argc) {
    (void)argc; /* The typed caller requests compiler argv initialization. */
    long long mid=sgcli_matches_new();if(mid<0)return mid;
    long long status=sgcli_process_arguments(mid);
    if(status<0){CliApp *a=cli_app(id);if(a)snprintf(a->error,sizeof(a->error),"cannot read process arguments as UTF-8");sgcli_matches_close(mid);return status;}
    return sgcli_parse_owned(id,mid);
}
long long sgcli_count(long long id,long long name) {CliMatches *m=cli_match(id);return m?cli_count(m,(const char*)(intptr_t)name):-3;}
static CliValue *cli_find(CliMatches *m,const char *name,long long index) {
    if(index<0)return NULL;for(size_t i=0;i<m->nvalues;++i)if(!strcmp(m->values[i].name,name)&&index--==0)return &m->values[i];return NULL;
}
long long sgcli_string(long long id,long long name,long long index) {
    CliMatches *m=cli_match(id);if(!m)return -3;CliValue *v=cli_find(m,(const char*)(intptr_t)name,index);
    return v?cli_owned_string(v->text):-5;
}
long long sgcli_integer(long long id,long long name,long long index) {
    CliMatches *m=cli_match(id);if(!m)return 0;CliValue *v=cli_find(m,(const char*)(intptr_t)name,index);return v?v->integer:0;
}
long long sgcli_kind(long long id,long long name,long long index) {
    CliMatches *m=cli_match(id);if(!m)return -3;CliValue *v=cli_find(m,(const char*)(intptr_t)name,index);return v?v->kind:-5;
}
long long sgcli_positional_count(long long id) {CliMatches *m=cli_match(id);return m?(long long)m->npositions:-3;}
long long sgcli_positional(long long id,long long index) {
    CliMatches *m=cli_match(id);if(!m)return -3;
    return index<0||(size_t)index>=m->npositions?-5:cli_owned_string(m->positions[index]);
}
long long sgcli_selected_command(long long id) {CliMatches *m=cli_match(id);return m?cli_owned_string(m->command?m->command:""):-3;}
long long sgcli_version(long long id) {CliApp *a=cli_app(id);return a?cli_owned_string(a->version):-3;}
long long sgcli_help(long long id) {
    CliApp *a=cli_app(id);if(!a)return -3;
    size_t cap=strlen(a->name)+strlen(a->description)+256;
    for(size_t i=0;i<a->noptions;++i){CliOption *o=&a->options[i];cap+=strlen(o->name)+strlen(o->scope)+strlen(o->help)+80;}
    for(size_t i=0;i<a->ncommands;++i)cap+=strlen(a->commands[i].name)+strlen(a->commands[i].help)+8;
    char *s=(char*)malloc(cap);if(!s)return -14;
    size_t n=(size_t)snprintf(s,cap,"Usage: %s [OPTIONS]%s [ARGS]\n\n%s\n\nOptions:\n  -h, --help  Show help\n      --version  Show version\n",a->name,a->ncommands?" <COMMAND>":"",a->description);
    for(size_t i=0;i<a->noptions;++i){CliOption *o=&a->options[i];
        n+=(size_t)snprintf(s+n,cap-n,"  %s%s%s--%s%s%s%s  %s%s%s%s\n",*o->short_name?"-":"",o->short_name,*o->short_name?", ":"",o->name,o->kind==1?" <STRING>":o->kind==2?" <I64>":"",o->repeat?"...":"",o->required?" (required)":"",o->help,*o->scope?" [":"",o->scope,*o->scope?"]":"");
    }
    if(a->ncommands){n+=(size_t)snprintf(s+n,cap-n,"\nCommands:\n");for(size_t i=0;i<a->ncommands;++i)n+=(size_t)snprintf(s+n,cap-n,"  %s  %s\n",a->commands[i].name,a->commands[i].help);}
    long long result=cli_owned_string(s);free(s);return result;
}
long long sgcli_live_handles(void) {long long n=0;for(CliApp *a=cli_apps;a;a=a->next)++n;for(CliMatches *m=cli_matches;m;m=m->next)++n;return n;}
