#define SENGOO_RESOURCE_GZIP
#define SENGOO_GZIP_PROBE_MAIN sh_resource_original_main
#include "gzip_probe.c"
#include "deflate_resource_probe.h"
int main(int argc,char **argv){
    if(argc==2&&!strcmp(argv[1],"--oom-protocol"))return sh_resource_oom();
    int result=sh_resource_original_main(argc,argv);sh_resource_receipt();return result;
}
