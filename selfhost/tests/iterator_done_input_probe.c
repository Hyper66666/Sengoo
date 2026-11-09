#include "runtime_shared.h"
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
extern long long sengoo_raw_vec_iter_done(long long);
extern long long sengoo_raw_vec_iter_index(long long);
static int drops;
static long long dropped_sum;
static void move_i64(void *dst,void *src){memcpy(dst,src,8);memset(src,0,8);}
static void drop_i64(void *value){++drops;dropped_sum+=*(int64_t*)value;memset(value,0,8);}
static uint64_t hash_i64(const void *value){return *(const uint64_t*)value;}
static long long eq_i64(const void *a,const void *b){return *(const int64_t*)a==*(const int64_t*)b;}
static void check(int good,int code){if(!good)exit(code);}
static void empty(void){check(!sengoo_string_live_handle_count()&&!sengoo_buffer_live_handle_count()&&!sengoo_opaque_live_handle_count(),70);}
int main(void){
 for(int round=0;round<100;++round){
  empty();check(drops==round*3&&dropped_sum==round*84,71);
  long long vec=sengoo_raw_vec_new_parts(8,8,(void*)move_i64,(void*)drop_i64);
  int64_t value=42;check(vec>0&&sengoo_raw_vec_push(vec,&value)==0&&value==0,72);
  long long iter=sengoo_raw_vec_iter_new(vec);
  check(iter>0&&sengoo_opaque_live_handle_count()==2,73);
  for(int repeated=0;repeated<4;++repeated){
   check(sengoo_raw_vec_iter_done(iter)==0&&sengoo_raw_vec_iter_index(iter)==0,74);
   check(sengoo_opaque_live_handle_count()==2&&drops==round*3,75);
  }
  int64_t *item=(int64_t*)sengoo_raw_vec_iter_next(iter);
  check(item&&*item==42,76);
  for(int repeated=0;repeated<4;++repeated)check(sengoo_raw_vec_iter_done(iter)==1&&sengoo_raw_vec_iter_index(iter)==1,77);
  check(sengoo_raw_vec_free(vec)==0&&drops==round*3+1,78);
  check(sengoo_opaque_live_handle_count()==1&&sengoo_raw_vec_iter_done(iter)==1,79);
  check(sengoo_raw_vec_iter_free(iter)==0&&sengoo_raw_vec_iter_done(iter)==1,80);
  empty();
  long long map=sengoo_raw_hashmap_new_parts(8,8,(void*)move_i64,(void*)drop_i64,(void*)hash_i64,(void*)eq_i64,8,8,(void*)move_i64,(void*)drop_i64);
  int64_t key=7,mapped=35;
  check(map>0&&sengoo_raw_hashmap_insert(map,&key,&mapped)==0&&key==0&&mapped==0,81);
  iter=sengoo_raw_map_key_iter_new(map);
  check(iter>0&&sengoo_opaque_live_handle_count()==2,82);
  for(int repeated=0;repeated<4;++repeated){
   check(sengoo_raw_map_key_iter_done(iter)==0&&sengoo_raw_map_key_iter_index(iter)==0,83);
   check(sengoo_opaque_live_handle_count()==2&&drops==round*3+1,84);
  }
  item=(int64_t*)sengoo_raw_map_key_iter_next(iter);
  check(item&&*item==7,85);
  for(int repeated=0;repeated<4;++repeated)check(sengoo_raw_map_key_iter_done(iter)==1&&sengoo_raw_map_key_iter_index(iter)==1,86);
  check(sengoo_raw_hashmap_free(map)==0&&drops==round*3+3&&dropped_sum==(round+1)*84,87);
  check(sengoo_opaque_live_handle_count()==1&&sengoo_raw_map_key_iter_done(iter)==1,88);
  check(sengoo_raw_map_key_iter_free(iter)==0&&sengoo_raw_map_key_iter_done(iter)==1,89);
  check(sengoo_raw_vec_iter_done(0)==1&&sengoo_raw_map_key_iter_done(0)==1,90);
  empty();
 }
 return 0;
}
