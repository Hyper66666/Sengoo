#define WIN32_LEAN_AND_MEAN
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <windows.h>
typedef struct {
  bool is_ok;
  bool value;
  int64_t error;
} Outcome;
_Static_assert(sizeof(Outcome) == 16, "outcome size");
_Static_assert(_Alignof(Outcome) == 8, "outcome alignment");
_Static_assert(offsetof(Outcome, is_ok) == 0 && offsetof(Outcome, value) == 1 &&
                   offsetof(Outcome, error) == 8,
               "outcome offsets");
typedef int64_t (*Open)(int64_t);
typedef bool (*Close)(int64_t);
typedef int64_t (*Read)(int64_t, uint8_t *, int64_t);
typedef int64_t (*Start)(int64_t, int64_t);
typedef int64_t (*Poll)(int64_t);
typedef void (*Result)(Outcome *, int64_t);
typedef bool (*Cancel)(int64_t);
typedef void (*Drop)(int64_t);
typedef int64_t (*Observe)(int64_t);
#define REQUIRE(x)                                                             \
  do {                                                                         \
    if (!(x)) {                                                                \
      fprintf(stderr, "readiness probe failed at line %d: %s (Win32=%lu)\n",   \
              __LINE__, #x, GetLastError());                                   \
      return __LINE__;                                                         \
    }                                                                          \
  } while (0)
static DWORD handles(void) {
  DWORD n = 0;
  return GetProcessHandleCount(GetCurrentProcess(), &n) ? n : UINT32_MAX;
}
static int outcome(Result result, int64_t wait, bool ok, bool value,
                   int64_t error) {
  struct {
    uint64_t before;
    Outcome result;
    uint64_t after;
  } guarded = {UINT64_C(0x1234567890abcdef),
               {false, false, -777},
               UINT64_C(0xfedcba0987654321)};
  result(&guarded.result, wait);
  REQUIRE(guarded.before == UINT64_C(0x1234567890abcdef) &&
          guarded.after == UINT64_C(0xfedcba0987654321));
  REQUIRE(guarded.result.is_ok == ok && guarded.result.value == value &&
          guarded.result.error == error);
  return 0;
}
int readiness_probe(Open open, Close close, Read read, Start start, Poll poll,
                    Result result, Cancel cancel, Drop drop, Observe observe,
                    const char *disk_path, int rounds) {
  char pipe_path[128];
  snprintf(pipe_path, sizeof(pipe_path), "\\\\.\\pipe\\sengoo-readiness-%lu",
           GetCurrentProcessId());
  REQUIRE(open(0) == -2);
  REQUIRE(!close(0));
  REQUIRE(poll(0) == 1);
  REQUIRE(!cancel(0));
  drop(0);
  REQUIRE(outcome(result, 0, false, false, 3) == 0);
  for (int round = -1; round < rounds; round++)
    for (int scenario = 0; scenario < 16; scenario++) {
      DWORD base_handles = handles();
      int64_t base_allocations = observe(0), base_bytes = observe(1);
      HANDLE server = NULL;
      int64_t file = 0, wait = 0;
      if (scenario < 4 || scenario == 12 || scenario == 13) {
        file = open((int64_t)(intptr_t)disk_path);
        REQUIRE(file > 0);
      } else if (scenario != 9) {
        server =
            CreateNamedPipeA(pipe_path, PIPE_ACCESS_OUTBOUND,
                             PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT, 1,
                             4096, 4096, 0, NULL);
        REQUIRE(server != INVALID_HANDLE_VALUE);
        file = open((int64_t)(intptr_t)pipe_path);
        REQUIRE(file > 0);
        BOOL connected = ConnectNamedPipe(server, NULL);
        REQUIRE(connected || GetLastError() == ERROR_PIPE_CONNECTED);
      }
      DWORD source_handles = handles();
      int64_t source_allocations = observe(0), source_bytes = observe(1);
      wait = start(scenario == 9 ? 0 : file, scenario == 8   ? -1
                                             : scenario == 7 ? 0
                                                             : 1000);
      REQUIRE(wait > 0);
      if (round >= 0) {
        REQUIRE(observe(0) == source_allocations + 1);
        REQUIRE(observe(1) > source_bytes);
        REQUIRE(handles() ==
                source_handles + (scenario != 8 && scenario != 9 ? 1 : 0));
      }
      if (scenario == 0) {
        REQUIRE(poll(wait) == 1);
        REQUIRE(poll(wait) == -3);
        REQUIRE(outcome(result, wait, true, true, 0) == 0);
        wait = 0;
        uint8_t bytes[8] = {0};
        REQUIRE(read(file, NULL, 1) == -2);
        REQUIRE(read(file, bytes, -1) == -2);
        REQUIRE(read(file, NULL, 0) == 0);
        REQUIRE(read(file, bytes, 8) == 5 && !memcmp(bytes, "ready", 5));
      }
      if (scenario == 1) {
        REQUIRE(close(file));
        file = 0;
        REQUIRE(poll(wait) == 1);
        REQUIRE(outcome(result, wait, true, true, 0) == 0);
        wait = 0;
      }
      if (scenario == 2) {
        REQUIRE(cancel(wait));
        wait = 0;
      }
      if (scenario == 3) {
        drop(wait);
        wait = 0;
      }
      if (scenario == 4) {
        REQUIRE(poll(wait) == 0);
        DWORD written = 0;
        REQUIRE(WriteFile(server, "ready", 5, &written, NULL) && written == 5);
        ULONGLONG until = GetTickCount64() + 1000;
        int64_t state = 0;
        do {
          state = poll(wait);
          if (state == 0)
            Sleep(1);
        } while (state == 0 && GetTickCount64() < until);
        REQUIRE(state == 1);
        REQUIRE(outcome(result, wait, true, true, 0) == 0);
        wait = 0;
        uint8_t bytes[8] = {0};
        REQUIRE(read(file, bytes, 8) == 5 && !memcmp(bytes, "ready", 5));
      }
      if (scenario == 5) {
        REQUIRE(poll(wait) == 0);
        REQUIRE(close(file));
        file = 0;
        REQUIRE(poll(wait) == 0);
        REQUIRE(cancel(wait));
        wait = 0;
      }
      if (scenario == 6) {
        REQUIRE(poll(wait) == 0);
        drop(wait);
        wait = 0;
      }
      if (scenario == 7) {
        REQUIRE(poll(wait) == 1);
        REQUIRE(outcome(result, wait, false, false, 11) == 0);
        wait = 0;
      }
      if (scenario == 8) {
        REQUIRE(poll(wait) == 1);
        REQUIRE(outcome(result, wait, false, false, 2) == 0);
        wait = 0;
      }
      if (scenario == 9) {
        REQUIRE(poll(wait) == 1);
        REQUIRE(outcome(result, wait, false, false, 3) == 0);
        wait = 0;
      }
      if (scenario == 10) {
        REQUIRE(poll(wait) == 0);
        REQUIRE(outcome(result, wait, false, false, 1) == 0);
        wait = 0;
      }
      if (scenario == 11) {
        REQUIRE(poll(wait) == 0);
        result(NULL, wait);
        wait = 0;
      }
      if (scenario == 12) {
        REQUIRE(poll(wait) == 1);
        REQUIRE(cancel(wait));
        wait = 0;
      }
      if (scenario == 13) {
        REQUIRE(poll(wait) == 1);
        drop(wait);
        wait = 0;
      }
      if (scenario == 14) {
        REQUIRE(poll(wait) == 0);
        REQUIRE(close(file));
        file = 0;
        DWORD written = 0;
        REQUIRE(WriteFile(server, "ready", 5, &written, NULL) && written == 5);
        REQUIRE(poll(wait) == 1);
        REQUIRE(outcome(result, wait, true, true, 0) == 0);
        wait = 0;
      }
      if (scenario == 15) {
        REQUIRE(poll(wait) == 0);
        REQUIRE(CloseHandle(server));
        server = NULL;
        REQUIRE(poll(wait) == 1);
        REQUIRE(outcome(result, wait, true, true, 0) == 0);
        wait = 0;
      }
      REQUIRE(wait == 0);
      if (file) {
        REQUIRE(close(file));
        REQUIRE(!close(file));
      }
      if (server)
        REQUIRE(CloseHandle(server));
      REQUIRE(handles() == base_handles);
      if (round >= 0) {
        REQUIRE(observe(0) == base_allocations);
        REQUIRE(observe(1) == base_bytes);
      }
      if (round == rounds - 1)
        printf("completed readiness case %d, measured rounds %d\n", scenario,
               rounds);
    }
  REQUIRE(fflush(stdout) == 0);
  return 0;
}
