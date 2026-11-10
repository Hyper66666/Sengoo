#include "runtime_shared.h"
#include <stdint.h>
#include <stdlib.h>
typedef struct { int64_t handle,marker; } NativeVec;
typedef struct { int64_t token,value; } NativeOwner;
/* These SG function signatures use the same verified Windows aggregate ABI.
 * No C-ABI aggregate signature is admitted by the source type checker. */
extern NativeVec source_i64(void),source_bool(void),source_owner(void),source_i64_second(void);
static void *tokens[50];static int live,dropped,bad,observed;
void source_drop_observe(void){observed++;}
void source_release(int64_t value){for(int i=0;i<50;i++){if(tokens[i]&&(int64_t)(intptr_t)tokens[i]==value){free(tokens[i]);tokens[i]=NULL;live--;dropped++;return;}}bad=1;}
int main(void){for(int round=0;round<1000;round++){
    NativeVec a=source_i64(),b=source_bool(),c=source_owner(),d=source_i64_second();
    if(a.handle<=0||b.handle<=0||c.handle<=0||d.handle<=0||a.marker||b.marker||c.marker||d.marker)return 70;
    for(int i=0;i<50;i++){int64_t number=i+1;uint8_t flag=(uint8_t)(i&1);tokens[i]=malloc(8);if(!tokens[i])return 71;live++;NativeOwner owner={(int64_t)(intptr_t)tokens[i],i+19};if(sengoo_raw_vec_push(a.handle,&number)||number||sengoo_raw_vec_push(b.handle,&flag)||flag||sengoo_raw_vec_push(c.handle,&owner)||owner.token||owner.value)return 72;}
    for(int i=0;i<50;i++){const int64_t *number=sengoo_raw_vec_get(a.handle,i);const uint8_t *flag=sengoo_raw_vec_get(b.handle,i);const NativeOwner *owner=sengoo_raw_vec_get(c.handle,i);if(!number||!flag||!owner||*number!=i+1||*flag!=(i&1)||owner->token!=(int64_t)(intptr_t)tokens[i]||owner->value!=i+19)return 73;}
    if(live!=50||bad)return 74;
    if(sengoo_raw_vec_free(a.handle)||sengoo_raw_vec_free(b.handle)||sengoo_raw_vec_free(c.handle)||sengoo_raw_vec_free(d.handle)||sengoo_opaque_live_handle_count()||live||bad||dropped!=(round+1)*50||observed!=(round+1)*50)return 75;
}return 42;}
