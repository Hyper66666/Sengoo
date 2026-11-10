#include "collection_os_probe.h"
#include "runtime_shared.h"
#include <stdint.h>
#include <string.h>
extern long long sengoo_ffi_buffer_new(long long);
extern long long sengoo_ffi_buffer_free(long long);
extern long long sengoo_buffer_live_handle_count(void);
extern int64_t sg_optional_i64(int64_t);
extern int64_t sg_optional_bool(int64_t);
extern int64_t sg_optional_scalar_i64(int64_t);
extern int64_t sg_optional_scalar_bool(int64_t);
extern int64_t sg_optional_owner(int64_t);
typedef struct {int64_t handle;int64_t value;} Owner;
static int live_owned,drops,moves,scalar_drops,bool_drops;
static void move_i64(void *to,void *from){memcpy(to,from,8);memset(from,0,8);}
static void drop_i64(void *from){++scalar_drops;memset(from,0,8);}
static void move_bool(void *to,void *from){memcpy(to,from,1);memset(from,0,1);}
static void drop_bool(void *from){++bool_drops;memset(from,0,1);}
static void move_owner(void *to,void *from){if(((uintptr_t)to&7)||((uintptr_t)from&7))__builtin_trap();memcpy(to,from,sizeof(Owner));memset(from,0,sizeof(Owner));++moves;}
void optional_owner_release(Owner *owner){if(!owner->handle||sengoo_ffi_buffer_free(owner->handle))__builtin_trap();owner->handle=0;owner->value=0;--live_owned;++drops;}
static void drop_owner(void *from){optional_owner_release((Owner*)from);}
static int run_i64(int round){
 long long vector=sengoo_raw_vec_new_parts(8,8,(void*)move_i64,(void*)drop_i64);int64_t a=40,b=2;
 if(!vector||sengoo_raw_vec_push(vector,&a)||sengoo_raw_vec_push(vector,&b))return 70;
 long long iterator=sengoo_raw_vec_iter_new(vector);
 if(!iterator||sg_optional_i64(iterator)!=40||sg_optional_i64(iterator)!=2||sg_optional_i64(iterator)!=-1||sg_optional_i64(iterator)!=-1||scalar_drops!=round*2)return 71;
 if(sengoo_raw_vec_iter_free(iterator))return 82;iterator=sengoo_raw_vec_iter_new(vector);if(!iterator||sg_optional_scalar_i64(iterator)!=40||sg_optional_scalar_i64(iterator)!=2||sg_optional_scalar_i64(iterator)!=-1||sg_optional_scalar_i64(iterator)!=-1)return 83;
 if(sengoo_raw_vec_iter_free(iterator)||sengoo_raw_vec_free(vector)||scalar_drops!=(round+1)*2)return 72;
 return 0;
}
static int run_bool(int round){
 long long vector=sengoo_raw_vec_new_parts(1,1,(void*)move_bool,(void*)drop_bool);unsigned char a=0,b=1;
 if(!vector||sengoo_raw_vec_push(vector,&a)||sengoo_raw_vec_push(vector,&b))return 73;
 long long iterator=sengoo_raw_vec_iter_new(vector);
 if(!iterator||sg_optional_bool(iterator)!=0||sg_optional_bool(iterator)!=1||sg_optional_bool(iterator)!=-1||sg_optional_bool(iterator)!=-1||bool_drops!=round*2)return 74;
 if(sengoo_raw_vec_iter_free(iterator))return 84;iterator=sengoo_raw_vec_iter_new(vector);if(!iterator||sg_optional_scalar_bool(iterator)!=0||sg_optional_scalar_bool(iterator)!=1||sg_optional_scalar_bool(iterator)!=-1||sg_optional_scalar_bool(iterator)!=-1)return 85;
 if(sengoo_raw_vec_iter_free(iterator)||sengoo_raw_vec_free(vector)||bool_drops!=(round+1)*2)return 75;
 return 0;
}
static int run_owner(int round){
 long long vector=sengoo_raw_vec_new_parts(16,8,(void*)move_owner,(void*)drop_owner);
 Owner a={sengoo_ffi_buffer_new(64),40},b={sengoo_ffi_buffer_new(96),2};if(!a.handle||!b.handle)return 76;live_owned+=2;
 if(!vector||sengoo_raw_vec_push(vector,&a)||sengoo_raw_vec_push(vector,&b)||a.handle||b.handle)return 77;
 long long iterator=sengoo_raw_vec_iter_new(vector);
 if(!iterator||sg_optional_owner(iterator)!=40||sg_optional_owner(iterator)!=2||sg_optional_owner(iterator)!=-1||sg_optional_owner(iterator)!=-1||drops!=round*2||live_owned!=2)return 78;
 if(sengoo_raw_vec_iter_free(iterator)||sengoo_raw_vec_free(vector)||drops!=(round+1)*2||live_owned)return 79;
 return 0;
}
int main(int argc,char **argv){if(argc>1&&strcmp(argv[1],"--os-providers")==0)return sh_collection_os_modules();
 long long buffers=sengoo_buffer_live_handle_count(),opaque=sengoo_opaque_live_handle_count();
 for(int round=0;round<1000;++round){int r=run_i64(round);if(r)return r;r=run_bool(round);if(r)return r;r=run_owner(round);if(r)return r;if(sengoo_buffer_live_handle_count()!=buffers||sengoo_opaque_live_handle_count()!=opaque)return 80;}
 return drops==2000&&moves==2000&&scalar_drops==2000&&bool_drops==2000?42:81;
}
