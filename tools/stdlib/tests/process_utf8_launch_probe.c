/* Real Windows synchronous launches. The shared native fixture supplies only
 * the isolated opaque-token allocator; SG integration uses the actual runtime. */
#define wmain foundation_wmain
#define main foundation_main
#define probe_main foundation_probe_main
#include "process_terminal_probe.c"
#undef wmain
#undef main
#undef probe_main

#ifndef _WIN32
#error This regression requires native Windows wide process APIs.
#endif

static void to_utf8(const wchar_t* wide, char* text, int capacity) {
    assert(WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide, -1,
                              text, capacity, NULL, NULL));
}
static long long utf8_command(const char* executable, const char* cwd, int clear) {
    long long command = sengoo_process_command_new(raw(executable)); assert(command > 0);
    assert(sengoo_process_command_cwd(command, raw(cwd)) == 1);
    assert(sengoo_process_command_capture_stdout(command, 1) == 1);
    assert(sengoo_process_command_capture_stderr(command, 1) == 1);
    if (clear) assert(sengoo_process_command_env_clear(command) == 1);
    assert(sengoo_process_command_env_set(command, raw("SENGOO_UTF8_LAUNCH"), raw("env 世界😀")) == 1);
    assert(sengoo_process_command_env_set(command, raw("SENGOO_环境"), raw("value😀")) == 1);
    assert(sengoo_process_command_arg(command, raw("--verify")) == 1);
    assert(sengoo_process_command_arg(command, raw(cwd)) == 1);
    assert(sengoo_process_command_arg(command, raw("世界 😀 \"quoted\" &")) == 1);
    assert(sengoo_process_command_arg(command, raw("")) == 1);
    assert(sengoo_process_command_arg(command, raw("tail\\")) == 1);
    assert(sengoo_process_command_arg(command, raw("&literal")) == 1);
    assert(sengoo_process_command_timeout_ms(command, 5000) == 1);
    return command;
}
static void check_output(long long output) {
    assert(output > 0 && sengoo_process_output_exit_code(output) == 0);
    assert(sengoo_process_output_timed_out(output) == 0);
    char out[64] = {0}, err[64] = {0};
    long long out_len = sengoo_process_output_stdout_copy(output, raw(out), 63);
    long long err_len = sengoo_process_output_stderr_copy(output, raw(err), 63);
    assert(out_len == 11 && !memcmp(out, "UTF8 CHILD\n", 11));
    assert(err_len == 10 && !memcmp(err, "UTF8 ERROR", 10));
    assert(sengoo_process_output_close(output) == 0);
}
int wmain(int argc, wchar_t** argv) {
    if (argc > 1 && !wcscmp(argv[1], L"--verify")) {
        assert(argc == 7);
        assert(!wcscmp(argv[3], L"世界 😀 \"quoted\" &"));
        assert(!wcscmp(argv[4], L""));
        assert(!wcscmp(argv[5], L"tail\\"));
        assert(!wcscmp(argv[6], L"&literal"));
        wchar_t cwd[32768], value[128];
        assert(GetCurrentDirectoryW(32768, cwd) && !wcscmp(cwd, argv[2]));
        assert(GetEnvironmentVariableW(L"SENGOO_UTF8_LAUNCH", value, 128));
        assert(!wcscmp(value, L"env 世界😀"));
        assert(GetEnvironmentVariableW(L"SENGOO_环境", value, 128));
        assert(!wcscmp(value, L"value😀"));
        child_write(1, "UTF8 CHILD\n", 11); child_write(2, "UTF8 ERROR", 10);
        return 0;
    }
    if (argc == 2 && !wcscmp(argv[1], L"--sleep")) { Sleep(30000); return 0; }
    if (argc == 2 && !wcscmp(argv[1], L"--emit")) {
        child_write(1, "pipeline 世界😀", strlen("pipeline 世界😀")); return 0;
    }
    if (argc == 2 && !wcscmp(argv[1], L"--consume")) {
        char input[128] = {0}; DWORD size = 0;
        assert(ReadFile(GetStdHandle(STD_INPUT_HANDLE), input, 127, &size, NULL));
        assert(size == strlen("pipeline 世界😀") && !memcmp(input, "pipeline 世界😀", size));
        child_write(1, "PIPELINE OK", 11); return 0;
    }
    assert(argc == 2 && !wcscmp(argv[1], L"--native-tests"));
    wchar_t image[32768], directory[32768]; char executable[131072], cwd[131072];
    assert(GetModuleFileNameW(NULL, image, 32768));
    wcscpy(directory, image); wchar_t* slash = wcsrchr(directory, L'\\'); assert(slash); *slash = 0;
    to_utf8(image, executable, sizeof(executable)); to_utf8(directory, cwd, sizeof(cwd));

    long long warmup = utf8_command(executable, cwd, 0);
    check_output(sengoo_process_command_run(warmup));
    assert(sengoo_process_command_close(warmup) == 0);
    DWORD baseline; assert(GetProcessHandleCount(GetCurrentProcess(), &baseline));
    for (int iteration = 0; iteration < 30; iteration++) {
        long long command = utf8_command(executable, cwd, iteration % 2);
        check_output(sengoo_process_command_run(command));
        assert(sengoo_process_command_close(command) == 0);
        const char bad[] = { (char)0xff, 0 };
        for (int field = 0; field < 4; field++) {
            command = sengoo_process_command_new(raw(field == 0 ? bad : executable)); assert(command > 0);
            if (field == 1) assert(sengoo_process_command_arg(command, raw(bad)) == 1);
            if (field == 2) assert(sengoo_process_command_cwd(command, raw(bad)) == 1);
            if (field == 3) assert(sengoo_process_command_env_set(command, raw("INVALID"), raw(bad)) == 1);
            assert(sengoo_process_command_run(command) == 0);
            assert(sengoo_process_command_close(command) == 0);
        }
        assert(sengoo_process_live_handle_count() == 0);
        assert(sengoo_process_live_resource_count() == 0);
        DWORD count; assert(GetProcessHandleCount(GetCurrentProcess(), &count) && count == baseline);
    }

    long long timeout = sengoo_process_command_new(raw(executable)); assert(timeout > 0);
    assert(sengoo_process_command_arg(timeout, raw("--sleep")) == 1);
    assert(sengoo_process_command_timeout_ms(timeout, 30) == 1);
    long long expired = sengoo_process_command_run(timeout); assert(expired > 0);
    assert(sengoo_process_output_timed_out(expired) == 1);
    assert(sengoo_process_output_close(expired) == 0 && sengoo_process_command_close(timeout) == 0);

    long long upstream = sengoo_process_command_new(raw(executable));
    long long downstream = sengoo_process_command_new(raw(executable));
    assert(upstream > 0 && downstream > 0);
    assert(sengoo_process_command_arg(upstream, raw("--emit")) == 1);
    assert(sengoo_process_command_arg(downstream, raw("--consume")) == 1);
    assert(sengoo_process_command_capture_stdout(downstream, 1) == 1);
    long long pipeline = sengoo_process_command_pipe_stdout_to(upstream, downstream); assert(pipeline > 0);
    long long piped = sengoo_process_command_run(pipeline); assert(piped > 0);
    assert(sengoo_process_output_exit_code(piped) == 0);
    char received[16] = {0};
    assert(sengoo_process_output_stdout_copy(piped, raw(received), 16) == 11);
    assert(!memcmp(received, "PIPELINE OK", 11));
    assert(sengoo_process_output_close(piped) == 0);
    assert(sengoo_process_command_close(pipeline) == 0);
    /* Pipeline run consumes/closes upstream; pipeline is the downstream token. */
    assert(sengoo_process_live_handle_count() == 0 && sengoo_process_live_resource_count() == 0);
    DWORD final; assert(GetProcessHandleCount(GetCurrentProcess(), &final) && final == baseline);
    puts("process UTF-8 native: 30 launches, 120 invalid inputs, argv/cwd/env, pipeline, timeout and exact handles passed");
    return 0;
}
