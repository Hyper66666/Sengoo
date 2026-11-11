typedef long long i64;
static volatile unsigned char state[8193];
static i64 initial = 2;
static volatile unsigned char * volatile address = state;
int main(void) {
  for (i64 i=0;i<8193;i++) if(state[i]!=0) return 61;
  if(address!=state) return 62;
  state[0]=20;state[8192]=20;
  return state[0]+state[8192]+(int)initial;
}
