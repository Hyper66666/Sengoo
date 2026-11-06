/* A stopped reader must not prevent timeout or tree cancellation. */
#define wmain foundation_wmain
#define main foundation_main
#define probe_main foundation_probe_main
#include "process_terminal_probe.c"
#undef wmain
#undef main
#undef probe_main
#ifdef _WIN32
static void pressure(long long child) {
    unsigned char bytes[SENGOO_PROCESS_IO_CHUNK]; memset(bytes, 0xab, sizeof(bytes));
    SengooFfiBuffer input = { bytes, sizeof(bytes), sizeof(bytes) };
    size_t accepted = 0; int blocked = 0; long long started = sengoo_process_now_ms();
    for (int i = 0; i < 32; i++) {
        long long before = sengoo_process_now_ms();
        long long n = sengoo_process_handle_try_write_stdin(child, raw(&input), sizeof(bytes));
        assert(n >= 0 && n <= (long long)sizeof(bytes) && sengoo_process_now_ms() - before < 500);
        if (!n) { blocked = 1; break; }
        accepted += (size_t)n;
    }
    assert(blocked && accepted >= SENGOO_PROCESS_IO_CHUNK && sengoo_process_now_ms() - started < 1000);
    assert(sengoo_process_handle_try_wait(child) == 0);
    for (int i = 0; i < 1000; i++) assert(sengoo_process_handle_try_write_stdin(child, raw(&input), sizeof(bytes)) == 0);
    assert(sengoo_process_now_ms() - started < 1500);
}
static void read_marker(long long child, void* bytes, size_t length) {
    size_t offset = 0; long long deadline = sengoo_process_now_ms() + 5000;
    while (offset < length) {
        SengooFfiBuffer out = {(unsigned char*)bytes + offset, length - offset, 0};
        long long n = sengoo_process_handle_read(child, 1, raw(&out)); assert(n >= 0); offset += (size_t)n;
        assert(sengoo_process_now_ms() < deadline); if (!n) Sleep(1);
    }
}
int wmain(int argc, wchar_t** argv) {
    if (argc == 2 && !wcscmp(argv[1], L"--ignore-stdin")) { Sleep(30000); return 0; }
    if (argc == 2 && !wcscmp(argv[1], L"--closed-stdin")) {
        CloseHandle(GetStdHandle(STD_INPUT_HANDLE)); child_write(1, "R", 1); Sleep(30000); return 0;
    }
    if (argc > 1) return foundation_wmain(argc, argv);
    wchar_t wide_self[32768]; char self[65536]; assert(GetModuleFileNameW(NULL, wide_self, 32768));
    assert(WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide_self, -1, self, sizeof(self), NULL, NULL));
    long long warmup = spawn_child(self, "--exit");
    assert(sengoo_process_handle_reap(warmup, 5000) >= 0 && sengoo_process_handle_close(warmup) == 0);
    DWORD baseline; assert(GetProcessHandleCount(GetCurrentProcess(), &baseline));
    for (int i = 0; i < 30; i++) {
        long long child = spawn_child(self, "--ignore-stdin"); pressure(child);
        long long before = sengoo_process_now_ms();
        assert(sengoo_process_handle_reap(child, 2) == -SENGOO_STATUS_TIMEOUT && sengoo_process_now_ms() - before < 500);
        assert(sengoo_process_handle_terminate_tree(child) == 1 && sengoo_process_handle_reap(child, 5000) >= 0);
        assert(sengoo_process_handle_close(child) == 0 && sengoo_process_live_handle_count() == 0);
    }
    long long child = spawn_child(self, "--grandchild"); long long pid; read_marker(child, &pid, sizeof(pid));
    HANDLE descendant = OpenProcess(SYNCHRONIZE, FALSE, (DWORD)pid); assert(descendant); pressure(child);
    assert(sengoo_process_handle_terminate_tree(child) == 1 && sengoo_process_handle_reap(child, 5000) >= 0);
    assert(WaitForSingleObject(descendant, 5000) == WAIT_OBJECT_0); CloseHandle(descendant);
    assert(sengoo_process_handle_close(child) == 0);
    child = spawn_child(self, "--closed-stdin"); unsigned char marker; read_marker(child, &marker, 1); assert(marker == 'R');
    unsigned char bytes[32]; memset(bytes, 'q', sizeof(bytes)); SengooFfiBuffer input = {bytes, sizeof(bytes), sizeof(bytes)};
    long long before = sengoo_process_now_ms();
    assert(sengoo_process_handle_try_write_stdin(child, raw(&input), sizeof(bytes)) == -SENGOO_STATUS_IO);
    assert(sengoo_process_now_ms() - before < 500 && sengoo_process_handle_close(child) == 0);
    child = spawn_child(self, "--exit"); assert(sengoo_process_handle_reap(child, 5000) >= 0);
    assert(sengoo_process_handle_try_write_stdin(child, raw(&input), sizeof(bytes)) == -SENGOO_STATUS_IO);
    assert(sengoo_process_handle_close(child) == 0);
    child = spawn_child(self, "--echo");
    assert(sengoo_process_handle_try_write_stdin(child, raw(&input), sizeof(bytes)) == sizeof(bytes));
    memset(bytes, 'z', sizeof(bytes)); assert(sengoo_process_handle_close_stdin(child) == 1);
    unsigned char echoed[32]; read_marker(child, echoed, sizeof(echoed));
    for (size_t i = 0; i < sizeof(echoed); i++) assert(echoed[i] == 'q');
    assert(sengoo_process_handle_reap(child, 5000) == 0 && sengoo_process_handle_close(child) == 0);
    child = spawn_child(self, "--ignore-stdin"); input.capacity = 1;
    assert(sengoo_process_handle_try_write_stdin(child, raw(&input), 32) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_process_handle_close(child) == 0);
    DWORD after; assert(GetProcessHandleCount(GetCurrentProcess(), &after));
    if (after > baseline + 2) fprintf(stderr, "stdin handles: before=%lu after=%lu\n", baseline, after);
    assert(after <= baseline + 2);
    assert(sengoo_process_live_resource_count() == 0);
    puts("stdin: nonwaiting backpressure, timeout, reader failure, tree cancel and stable handles passed");
    return 0;
}
#else
int main(void) { puts("stdin: Windows stress probe; POSIX covered by process_terminal"); return 0; }
#endif
