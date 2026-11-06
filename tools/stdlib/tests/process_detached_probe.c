/* Independent Windows launch evidence; all data is synthetic and temporary. */
#define wmain foundation_wmain
#define main foundation_main
#define probe_main foundation_probe_main
#include "process_terminal_probe.c"
#undef wmain
#undef main
#undef probe_main
#ifdef _WIN32
static long long detached_birth(unsigned char bytes[8]) {
    unsigned long long value = 0;
    for (size_t i = 0; i < 8; i++) value |= (unsigned long long)bytes[i] << (8 * i);
    return (long long)value;
}
static void detached_marker(const wchar_t* directory, const wchar_t* name) {
    wchar_t path[32768]; assert(swprintf(path, 32768, L"%ls\\%ls", directory, name) > 0);
    HANDLE file = CreateFileW(path, GENERIC_WRITE, FILE_SHARE_READ, NULL, CREATE_ALWAYS, FILE_ATTRIBUTE_NORMAL, NULL);
    assert(file != INVALID_HANDLE_VALUE); DWORD written;
    assert(WriteFile(file, "ok", 2, &written, NULL) && written == 2); CloseHandle(file);
}
static int detached_child(int argc, wchar_t** argv) {
    if (argc != 5 || wcscmp(argv[3], L"\u542b \u7a7a\u683c \U0001f600\"quoted")) return 101;
    DWORD flags;
    if (SetEvent((HANDLE)(uintptr_t)_wcstoui64(argv[4], NULL, 10))) return 102;
    HANDLE standard[] = {GetStdHandle(STD_INPUT_HANDLE),GetStdHandle(STD_OUTPUT_HANDLE),GetStdHandle(STD_ERROR_HANDLE)};
    for (size_t i=0;i<3;i++) {
        DWORD mode;
        DWORD kind = GetFileType(standard[i]);
        if (kind == FILE_TYPE_DISK || kind == FILE_TYPE_PIPE || GetConsoleMode(standard[i], &mode)) return 103;
    }
    wchar_t cwd[32768]; if (!GetCurrentDirectoryW(32768, cwd) || _wcsicmp(cwd, argv[2])) return 104;
    wchar_t value[128]; if (!GetEnvironmentVariableW(L"SENGOO_DETACHED_USER", value, 128) || wcscmp(value, L"\u4e2d\u6587-env")) return 105;
    if (GetEnvironmentVariableW(L"SENGOO_DETACHED_SECRET", value, 128)) return 106;
    detached_marker(argv[2], L"ready");
    Sleep(1200);
    detached_marker(argv[2], L"done");
    return 0;
}
static long long detached_launch(const char* self, const char* directory, HANDLE event, unsigned char birth[8]) {
    long long command = sengoo_process_command_new(raw(self)); assert(command > 0);
    assert(sengoo_process_command_env_clear(command) >= 0);
    const char* system_root = getenv("SystemRoot");
    if (system_root) assert(sengoo_process_command_env_set(command, raw("SystemRoot"), raw(system_root)) >= 0);
    assert(sengoo_process_command_env_set(command, raw("SENGOO_DETACHED_USER"), raw("\xe4\xb8\xad\xe6\x96\x87-env")) >= 0);
    assert(sengoo_process_command_cwd(command, raw(directory)) >= 0);
    assert(sengoo_process_command_arg(command, raw("--detached")) >= 0);
    assert(sengoo_process_command_arg(command, raw(directory)) >= 0);
    assert(sengoo_process_command_arg(command, raw("\xe5\x90\xab \xe7\xa9\xba\xe6\xa0\xbc \xf0\x9f\x98\x80\"quoted")) >= 0);
    char handle[40]; snprintf(handle, sizeof(handle), "%llu", (unsigned long long)(uintptr_t)event);
    assert(sengoo_process_command_arg(command, raw(handle)) >= 0);
    SengooFfiBuffer identity = {birth, 8, 0};
    long long pid = sengoo_process_command_spawn_detached(command, raw(&identity));
    assert(sengoo_process_command_close(command) == 0);
    return pid;
}
static void detached_read(long long child, void* bytes, size_t length) {
    size_t used = 0; long long started = sengoo_process_now_ms();
    while (used < length) {
        SengooFfiBuffer buffer = {(unsigned char*)bytes + used, length - used, 0};
        long long count = sengoo_process_handle_read(child, 1, raw(&buffer));
        assert(count >= 0); used += (size_t)count;
        unsigned char diagnostic[4096]; SengooFfiBuffer errors={diagnostic,sizeof(diagnostic),0};
        long long n=sengoo_process_handle_read(child,2,raw(&errors));
        assert(n >= 0); if(n) fwrite(diagnostic,1,(size_t)n,stderr);
        if(sengoo_process_handle_try_wait(child)==1 && used<length) {
            fprintf(stderr,"receipt pipe ended early: exit=%lld received=%zu expected=%zu\n",sengoo_process_handle_exit_code(child),used,length);assert(0);
        }
        assert(sengoo_process_now_ms() - started < 15000); if (!count) Sleep(1);
    }
}
int wmain(int argc, wchar_t** argv) {
    if (argc > 1 && !wcscmp(argv[1], L"--detached")) return detached_child(argc, argv);
    wchar_t wide_self[32768]; char self[65536]; assert(GetModuleFileNameW(NULL, wide_self, 32768));
    assert(WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide_self, -1, self, sizeof(self), NULL, NULL));
    if (argc == 2 && !wcscmp(argv[1],L"--exit")) return 0;
    if (argc == 2 && !wcscmp(argv[1],L"--sleep")) { Sleep(30000); return 0; }
    if (argc == 2 && !wcscmp(argv[1],L"--grandchild")) {
        long long descendant=spawn_child(self,"--sleep");
        SengooProcessHandleState* child_state=sengoo_process_handle_resolve(descendant);assert(child_state);
        long long child_pid=GetProcessId(child_state->process);child_write(1,&child_pid,sizeof(child_pid));
        Sleep(30000);sengoo_process_handle_close(descendant);return 0;
    }
    if (argc == 3 && !wcscmp(argv[1], L"--launcher")) {
        Sleep(200); /* Let the probe configure its own enclosing Job. */
        char directory[65536]; assert(WideCharToMultiByte(CP_UTF8, 0, argv[2], -1, directory, sizeof(directory), NULL, NULL));
        SECURITY_ATTRIBUTES security = {sizeof(security), NULL, TRUE};
        HANDLE event = CreateEventW(&security, TRUE, FALSE, NULL); assert(event);
        unsigned char identity[8]; long long pid = detached_launch(self, directory, event, identity);
        assert(pid > 0); long long receipt[2] = {pid, detached_birth(identity)};
        child_write(1, receipt, sizeof(receipt)); CloseHandle(event); return 0;
    }
    if (argc == 3 && !wcscmp(argv[1], L"--forbidden")) {
        char directory[65536]; assert(WideCharToMultiByte(CP_UTF8, 0, argv[2], -1, directory, sizeof(directory), NULL, NULL));
        unsigned char identity[8];
        assert(detached_launch(self, directory, NULL, identity) == -SENGOO_STATUS_PERMISSION_DENIED);
        return 0;
    }
    if (argc > 1) return 2;
    assert(argc == 1);
    wchar_t temporary[32768], directory[32768]; assert(GetTempPathW(32768, temporary));
    swprintf(directory, 32768, L"%lssengoo-detached-\u4e2d\u6587 \U0001f600-%lu", temporary, GetCurrentProcessId());
    assert(CreateDirectoryW(directory, NULL)); char utf8[65536];
    assert(WideCharToMultiByte(CP_UTF8, 0, directory, -1, utf8, sizeof(utf8), NULL, NULL));
    assert(SetEnvironmentVariableW(L"SENGOO_DETACHED_SECRET", L"synthetic-only"));
    long long warm = spawn_child(self, "--exit");
    /* Warm ordinary owned-process creation as well as detached failure. */
    if (sengoo_process_handle_reap(warm, 5000) < 0) assert(0);
    assert(sengoo_process_handle_close(warm) == 0);
    /* Warm the OS's failed-launch path before counting stable owners. */
    char missing[65536]; assert(snprintf(missing,sizeof(missing),"%s\\not-present.exe",utf8) > 0);
    unsigned char warm_identity[8]; SengooFfiBuffer warm_buffer={warm_identity,8,0};
    long long failed_command=sengoo_process_command_new(raw(missing));
    assert(sengoo_process_command_spawn_detached(failed_command,raw(&warm_buffer)) < 0);
    assert(sengoo_process_command_close(failed_command) == 0);
    DWORD baseline; assert(GetProcessHandleCount(GetCurrentProcess(), &baseline));
    SECURITY_ATTRIBUTES security = {sizeof(security), NULL, TRUE};
    HANDLE event = CreateEventW(&security, TRUE, FALSE, NULL); assert(event);
    unsigned char identity[8]; long long pid = detached_launch(self, utf8, event, identity);
    if (pid <= 0) {
        JOBOBJECT_EXTENDED_LIMIT_INFORMATION current; memset(&current,0,sizeof(current));
        QueryInformationJobObject(NULL,JobObjectExtendedLimitInformation,&current,sizeof(current),NULL);
        fprintf(stderr,"detached root failed: status=%lld error=%lu current job flags=0x%lx\n",pid,GetLastError(),current.BasicLimitInformation.LimitFlags);
    };
    assert(pid > 0);
    long long birth = detached_birth(identity);
    assert(sengoo_process_identity_matches(pid, birth) == 1);
    assert(sengoo_process_identity_matches(pid, birth + 1) == 0);
    assert(sengoo_process_identity_matches(-1, birth) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(WaitForSingleObject(event, 0) == WAIT_TIMEOUT);
    HANDLE child = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, FALSE, (DWORD)pid); assert(child);
    assert(WaitForSingleObject(child, 5000) == WAIT_OBJECT_0);
    DWORD child_code; assert(GetExitCodeProcess(child,&child_code));
    if(child_code) fprintf(stderr,"detached child exit=%lu\n",child_code);
    assert(child_code == 0); CloseHandle(child);
    assert(WaitForSingleObject(event, 0) == WAIT_TIMEOUT); CloseHandle(event);
    assert(sengoo_process_identity_matches(pid, birth) == 0);
    wchar_t ready[32768], done[32768]; swprintf(ready,32768,L"%ls\\ready",directory); swprintf(done,32768,L"%ls\\done",directory);
    assert(GetFileAttributesW(ready) != INVALID_FILE_ATTRIBUTES && GetFileAttributesW(done) != INVALID_FILE_ATTRIBUTES);
    assert(DeleteFileW(ready) && DeleteFileW(done));

    /* The owned piped launcher lives in a kill job. Enable actual breakaway on
     * only this test-owned job, then prove its exit does not kill the child. */
    long long command = sengoo_process_command_new(raw(self));
    assert(sengoo_process_command_arg(command, raw("--launcher")) >= 0);
    assert(sengoo_process_command_arg(command, raw(utf8)) >= 0);
    long long launcher = sengoo_process_command_spawn_piped(command); assert(launcher > 0);
    assert(sengoo_process_command_close(command) == 0);
    SengooProcessHandleState* state = sengoo_process_handle_resolve(launcher); assert(state && state->job);
    JOBOBJECT_EXTENDED_LIMIT_INFORMATION limits; memset(&limits, 0, sizeof(limits));
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE | JOB_OBJECT_LIMIT_BREAKAWAY_OK;
    assert(SetInformationJobObject(state->job, JobObjectExtendedLimitInformation, &limits, sizeof(limits)));
    long long receipt[2]; detached_read(launcher, receipt, sizeof(receipt));
    assert(sengoo_process_handle_reap(launcher, 5000) == 0);
    assert(sengoo_process_handle_close(launcher) == 0);
    assert(sengoo_process_identity_matches(receipt[0],receipt[1]) == 1);
    child = OpenProcess(SYNCHRONIZE, FALSE, (DWORD)receipt[0]); assert(child);
    assert(WaitForSingleObject(child,5000) == WAIT_OBJECT_0); CloseHandle(child);
    assert(GetFileAttributesW(done) != INVALID_FILE_ATTRIBUTES);
    assert(DeleteFileW(ready) && DeleteFileW(done));

    command = sengoo_process_command_new(raw(self));
    assert(sengoo_process_command_arg(command,raw("--forbidden")) >= 0);
    assert(sengoo_process_command_arg(command,raw(utf8)) >= 0);
    long long forbidden = sengoo_process_command_spawn_piped(command); assert(forbidden > 0);
    sengoo_process_command_close(command);
    assert(sengoo_process_handle_reap(forbidden,5000) == 0 && sengoo_process_handle_close(forbidden) == 0);
    assert(GetFileAttributesW(ready) == INVALID_FILE_ATTRIBUTES);

    long long owned=spawn_child(self,"--grandchild");
    long long descendant_pid; detached_read(owned,&descendant_pid,sizeof(descendant_pid));
    assert(sengoo_process_handle_tree_quiescent(owned) == 0);
    assert(sengoo_process_handle_terminate_tree(owned) == 1 && sengoo_process_handle_reap(owned,5000) >= 0);
    long long cleanup_started=sengoo_process_now_ms();
    while(!sengoo_process_handle_tree_quiescent(owned)){assert(sengoo_process_now_ms()-cleanup_started<5000);Sleep(1);}
    assert(sengoo_process_handle_close(owned) == 0);

    DWORD after;
    for (int i=0;i<24;i++) {
        command = sengoo_process_command_new(raw(missing));
        SengooFfiBuffer buffer = {identity,8,0};
        assert(sengoo_process_command_spawn_detached(command,raw(&buffer)) < 0 && buffer.used_len == 0);
        assert(sengoo_process_command_close(command) == 0);
    }
    assert(GetProcessHandleCount(GetCurrentProcess(),&after));
    assert(after <= baseline + 1 && sengoo_process_live_handle_count() == 0 && sengoo_process_live_resource_count() == 0);
    assert(RemoveDirectoryW(directory));
    puts("detached launch: birth identity, UTF-8/argv/cwd/env, no inherited handles/stdio, parent exit, forbidden breakaway and failed-spawn owner stability passed");
    return 0;
}
#else
int main(void) { unsigned char identity[8]; SengooFfiBuffer b={identity,8,0}; long long c=sengoo_process_command_new(raw("unused")); assert(sengoo_process_command_spawn_detached(c,raw(&b)) == -SENGOO_STATUS_UNSUPPORTED); assert(sengoo_process_identity_matches(1,1) == -SENGOO_STATUS_UNSUPPORTED); sengoo_process_command_close(c); return 0; }
#endif
