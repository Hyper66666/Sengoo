#include "runtime_shared.h"
#include <stdint.h>
extern int64_t source_some_i64(void);
extern int64_t source_some_bool(void);
extern int64_t source_none_i64(void);
extern int64_t source_none_bool(void);
extern int64_t source_helper_i64(void);
extern int64_t source_helper_bool(void);
#ifndef OBSERVATION_OLD_MIR
extern int64_t source_reuse_i64(void);
#endif
int64_t opaque(void){return 0;}
int main(void){
 int64_t before=sengoo_opaque_live_handle_count();
 for(int round=0;round<1000;round++){
  if(source_some_i64()!=42)return 71;if(sengoo_opaque_live_handle_count()!=before)return 81;
  if(source_some_bool()!=42)return 72;if(sengoo_opaque_live_handle_count()!=before)return 82;
  if(source_none_i64()!=42)return 73;if(sengoo_opaque_live_handle_count()!=before)return 83;
  if(source_none_bool()!=42)return 74;if(sengoo_opaque_live_handle_count()!=before)return 84;
  if(source_helper_i64()!=42)return 75;if(sengoo_opaque_live_handle_count()!=before)return 85;
#ifndef OBSERVATION_OLD_MIR
  if(source_reuse_i64()!=42)return 77;if(sengoo_opaque_live_handle_count()!=before)return 87;
#endif
  if(source_helper_bool()!=42)return 76;if(sengoo_opaque_live_handle_count()!=before)return 86;
 }
 return 42;
}
