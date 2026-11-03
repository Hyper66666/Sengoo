#include "native_runtime.h"
#include "../native/sgcli.c"
static long long arguments(const char **args,size_t count) {
    long long m=sgcli_matches_new();assert(m>0);
    for(size_t i=0;i<count;++i)assert(sgcli_argument(m,test_pointer(args[i]))==0);return m;
}
static void rejected(long long a,const char **args,size_t count,const char *message) {
    long long m=arguments(args,count);assert(sgcli_parse(a,m)==-2);
    long long error=sgcli_error(a);assert(strstr((const char*)(intptr_t)error,message));free((void*)(intptr_t)error);
    assert(sgcli_matches_close(m)==0);
}
int main(int argc,char **argv) {
    test_argc=argc;test_argv=argv;
    if(argc==3) {
        long long process=sgcli_matches_new();assert(process>0);
        assert(sgcli_process_arguments(process)==0);
        CliMatches *captured=cli_match(process);
        assert(captured&&captured->nargs==2);
        assert(!strcmp(captured->args[0],"--native-unicode-test"));
        assert(!strcmp(captured->args[1],"\xe4\xb8\x96\xe7\x95\x8c"));
        sgcli_matches_close(process);
    }
    long long a=sgcli_new(test_pointer("tool"),test_pointer("0.1"),test_pointer("Example tool"));assert(a>0);
    assert(sgcli_option(a,test_pointer(""),test_pointer("verbose"),test_pointer("v"),0,0,0,test_pointer("Verbose"))==0);
    assert(sgcli_option(a,test_pointer(""),test_pointer("name"),test_pointer("n"),1,0,1,test_pointer("Name"))==0);
    assert(sgcli_option(a,test_pointer(""),test_pointer("number"),test_pointer("i"),2,1,0,test_pointer("Integer"))==0);
    const char *ok[]={"-v","--name=hello","--number","-42","-i=9223372036854775807","--","--literal","-9"};
    long long m=arguments(ok,8);assert(sgcli_parse(a,m)==0);
    assert(sgcli_count(m,test_pointer("verbose"))==1);
    test_string(sgcli_string(m,test_pointer("name"),0),"hello");
    assert(sgcli_integer(m,test_pointer("number"),0)==-42);
    assert(sgcli_integer(m,test_pointer("number"),1)==LLONG_MAX);
    test_string(sgcli_positional(m,0),"--literal");test_string(sgcli_positional(m,1),"-9");
    assert(sgcli_matches_close(m)==0);assert(sgcli_matches_close(m)==-3);
    const char *unknown[]={"--bad"};rejected(a,unknown,1,"unknown option");
    const char *missing[]={"--name"};rejected(a,missing,1,"missing option value");
    const char *missing2[]={"--name","--verbose"};rejected(a,missing2,2,"missing option value");
    const char *duplicate[]={"--name=a","-n","b"};rejected(a,duplicate,3,"duplicate");
    const char *overflow[]={"--name=a","--number=9223372036854775808"};rejected(a,overflow,2,"out-of-range");
    const char *flagvalue[]={"--verbose=yes"};rejected(a,flagvalue,1,"does not take a value");
    const char *empty[]={"--help"};m=arguments(empty,1);assert(sgcli_parse(a,m)==0);assert(sgcli_matches_close(m)==0);
    long long h1=sgcli_help(a),h2=sgcli_help(a);assert(!strcmp((const char*)(intptr_t)h1,(const char*)(intptr_t)h2));
    assert(strstr((const char*)(intptr_t)h1,"Usage: tool [OPTIONS] [ARGS]\n"));free((void*)(intptr_t)h1);free((void*)(intptr_t)h2);
    assert(sgcli_close(a)==0);
    a=sgcli_new(test_pointer("db"),test_pointer("1"),test_pointer("Database"));
    assert(sgcli_command(a,test_pointer("query"),test_pointer("Query rows"))==0);
    assert(sgcli_option(a,test_pointer("query"),test_pointer("limit"),test_pointer("l"),2,0,0,test_pointer("Maximum rows"))==0);
    const char *command[]={"query","--limit","-3","input.csv"};m=arguments(command,4);assert(sgcli_parse(a,m)==0);
    test_string(sgcli_selected_command(m),"query");assert(sgcli_integer(m,test_pointer("limit"),0)==-3);
    assert(sgcli_matches_close(m)==0);
    const char *scope[]={"--limit=3","query"};rejected(a,scope,2,"unknown option");
    const char *badcommand[]={"unknown"};rejected(a,badcommand,1,"unknown subcommand");
    assert(sgcli_close(a)==0);assert(sgcli_live_handles()==0);
    puts("sgcli native: passed");return 0;
}
