/* Standalone native contract tests. Uses real OS children, not mocked waits. */
#define _CRT_SECURE_NO_WARNINGS
#if !defined(_WIN32)
#define _XOPEN_SOURCE 700
#define _DEFAULT_SOURCE 1
#endif
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "../runtime_process.c"
#include "../runtime_terminal.c"

/* Tiny deterministic allocation boundary for the isolated C probe. Native SG
 * integration separately uses the real String/Buffer/opaque runtime. */
typedef struct { void* pointer; uint32_t generation; } ProbeSlot;
static ProbeSlot probe_slots[128];
long long sengoo_opaque_handle_new(void* pointer) {
    for (size_t i = 1; i < 128; i++) if (!probe_slots[i].pointer) {
        probe_slots[i].pointer = pointer; probe_slots[i].generation++;
        return ((long long)probe_slots[i].generation << 32) | (long long)i;
    }
    assert(!"probe allocator exhausted"); return 0;
}
void* sengoo_opaque_handle_get(long long handle) {
    size_t index = (size_t)((uint64_t)handle & UINT32_MAX);
    return handle > 0 && index < 128 && probe_slots[index].generation == ((uint64_t)handle >> 32)
        ? probe_slots[index].pointer : NULL;
}
void* sengoo_opaque_handle_take(long long handle) {
    void* p = sengoo_opaque_handle_get(handle);
    if (p) probe_slots[(uint64_t)handle & UINT32_MAX].pointer = NULL;
    return p;
}
long long sengoo_ptr_to_handle(void* pointer) { return (long long)(intptr_t)pointer; }
void* sengoo_handle_to_ptr(long long handle) { return (void*)(intptr_t)handle; }
char* sengoo_strdup_bytes(const char* text) { size_t n = strlen(text) + 1; char* p = malloc(n); if (p) memcpy(p, text, n); return p; }
SengooFfiBuffer* sengoo_ffi_buffer_from_handle(long long handle) { return (SengooFfiBuffer*)(intptr_t)handle; }
long long sengoo_string_from_bytes_copy(long long bytes, long long length) {
    unsigned char* value = malloc((size_t)length + 1);
    if (!value) return 0;
    memcpy(value, (void*)(intptr_t)bytes, (size_t)length); value[length] = 0;
    return (long long)(intptr_t)value;
}
long long sengoo_time_unix_ms(void) { return 0; }
#ifdef _WIN32
int sengoo_size_add(size_t* total, size_t value) { if (*total > SIZE_MAX - value) return 1; *total += value; return 0; }
char* sengoo_windows_append_arg(char* out, const char* text) {
    *out++ = '"'; size_t backslashes = 0;
    for (;;) {
        if (*text == '\\') { backslashes++; text++; continue; }
        if (!*text || *text == '"') {
            for (size_t i = 0; i < backslashes * 2; i++) *out++ = '\\';
            if (*text == '"') *out++ = '\\';
        } else for (size_t i = 0; i < backslashes; i++) *out++ = '\\';
        backslashes = 0;
        if (!*text) break;
        *out++ = *text++;
    }
    *out++ = '"'; return out;
}
#endif
static long long raw(const void* p) { return (long long)(intptr_t)p; }
static void pause_ms(int ms) {
#ifdef _WIN32
    Sleep((DWORD)ms);
#else
    struct timespec delay = { ms / 1000, (ms % 1000) * 1000000L }; nanosleep(&delay, NULL);
#endif
}
static void child_write(int stream, const void* bytes, size_t count) {
#ifdef _WIN32
    DWORD written; assert(WriteFile(GetStdHandle(stream == 1 ? STD_OUTPUT_HANDLE : STD_ERROR_HANDLE), bytes, (DWORD)count, &written, NULL));
    assert(written == count);
#else
    assert(write(stream, bytes, count) == (ssize_t)count);
#endif
}
static long long spawn_child(const char* self, const char* mode) {
    long long command = sengoo_process_command_new(raw(self)); assert(command);
    assert(sengoo_process_command_arg(command, raw(mode)) == 1);
    long long child = sengoo_process_command_spawn_piped(command); assert(child > 0);
    sengoo_process_command_close(command);
    return child;
}
static void drain(long long child, size_t expected_out, size_t expected_err) {
    unsigned char data[4096]; SengooFfiBuffer buffer = {data, sizeof(data), 0};
    size_t out = 0, err = 0; long long deadline = sengoo_process_now_ms() + 10000;
    for (;;) {
        long long n = sengoo_process_handle_read(child, 1, raw(&buffer)); assert(n >= 0); out += (size_t)n;
        n = sengoo_process_handle_read(child, 2, raw(&buffer)); assert(n >= 0); err += (size_t)n;
        assert(sengoo_process_now_ms() < deadline);
        if (sengoo_process_handle_try_wait(child) == 1
            && sengoo_process_handle_stream_eof(child, 1) == 1 && sengoo_process_handle_stream_eof(child, 2) == 1) break;
        pause_ms(1);
    }
    if (out != expected_out || err != expected_err) fprintf(stderr, "drain: out=%zu/%zu err=%zu/%zu exit=%lld\n", out, expected_out, err, expected_err, sengoo_process_handle_exit_code(child));
    assert(out == expected_out && err == expected_err);
    assert(sengoo_process_handle_reap(child, 0) == 0);
    assert(sengoo_process_handle_close(child) == 0);
    assert(sengoo_process_handle_try_wait(child) == -SENGOO_STATUS_INVALID_HANDLE);
}
static void unicode_environment_test(const char* self) {
    char path[32768], executable[32768];
#ifdef _WIN32
    wchar_t temporary[32768], directory[32768], module[32768];
    DWORD count=GetTempPathW(32000,temporary);assert(count&&count<32000);
    swprintf(directory,32768,L"%lssengoo-cwd-\x76ee\x5f55-%lu",temporary,GetCurrentProcessId());
    assert(CreateDirectoryW(directory,NULL));
    assert(GetModuleFileNameW(NULL,module,32768));
    assert(WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,directory,-1,path,sizeof(path),NULL,NULL));
    assert(WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,module,-1,executable,sizeof(executable),NULL,NULL));
#else
    char temporary[]="/tmp/sengoo-cwd-目录-XXXXXX";
    assert(mkdtemp(temporary));snprintf(path,sizeof(path),"%s",temporary);
    char* absolute=realpath(self,NULL);assert(absolute);snprintf(executable,sizeof(executable),"%s",absolute);free(absolute);
#endif
    long long command=sengoo_process_command_new(raw(executable));assert(command>0);
    assert(sengoo_process_command_arg(command,raw("--cwd-env"))==1);
    assert(sengoo_process_command_arg(command,raw(path))==1);
    assert(sengoo_process_command_cwd(command,raw(path))==1);
    assert(sengoo_process_command_env_clear(command)==1);
    assert(sengoo_process_command_env_set(command,raw("SENGOO_NATIVE_PROBE"),raw("环境-value"))==1);
    /* The instrumented child needs the parent's diagnostic destination even
     * when this fixture deliberately replaces its application environment. */
    const char* sanitizer_options=getenv("ASAN_OPTIONS");
    if(sanitizer_options) assert(sengoo_process_command_env_set(command,raw("ASAN_OPTIONS"),raw(sanitizer_options))==1);
    long long child=sengoo_process_command_spawn_piped(command);assert(child>0);
    assert(sengoo_process_command_close(command)==0);drain(child,0,0);
#ifdef _WIN32
    assert(RemoveDirectoryW(directory));
#else
    assert(rmdir(path)==0);
#endif
}
static void parser_tests(void) {
    SengooTerminal* terminal = calloc(1, sizeof(*terminal)); assert(terminal);
    const unsigned char input[] = "\xe4\xb8\xad\033[A\003\033[200~paste\n\xe6\x96\x87\033[201~";
    memcpy(terminal->pending, input, sizeof(input) - 1); terminal->pending_len = sizeof(input) - 1;
    assert(sengoo_terminal_parse(terminal) == SG_TERM_KEY && terminal->code == 0x4e2d);
    assert(sengoo_terminal_parse(terminal) == SG_TERM_KEY && terminal->code == 1001);
    assert(sengoo_terminal_parse(terminal) == SG_TERM_CANCEL);
    assert(sengoo_terminal_parse(terminal) == SG_TERM_PASTE && terminal->text_len == 9);
    terminal->paste = 1; terminal->pending_len = SENGOO_TERMINAL_LIMIT + 1;
    memset(terminal->pending, 'x', terminal->pending_len);
    assert(sengoo_terminal_parse(terminal) == -SENGOO_STATUS_OVERFLOW && terminal->pending_len == 5);
    memcpy(terminal->pending + 5, "\033[201~z", 7); terminal->pending_len += 7;
    assert(sengoo_terminal_parse(terminal) == 0);
    assert(sengoo_terminal_parse(terminal) == SG_TERM_KEY && terminal->code == 'z');
    terminal->paste = 1; terminal->pending_len = SENGOO_TERMINAL_LIMIT + 8;
    memset(terminal->pending, 'x', SENGOO_TERMINAL_LIMIT + 1);
    memcpy(terminal->pending + SENGOO_TERMINAL_LIMIT + 1, "\033[201~z", 7);
    assert(sengoo_terminal_parse(terminal) == -SENGOO_STATUS_OVERFLOW);
    assert(sengoo_terminal_parse(terminal) == SG_TERM_KEY && terminal->code == 'z');
    terminal->pending[0] = 0xff; terminal->pending_len = 1;
    assert(sengoo_terminal_parse(terminal) == -SENGOO_STATUS_INVALID_UTF8);
    free(terminal);
}

static void seed_discard_state(SengooTerminal* terminal) {
    memcpy(terminal->pending, "yes", 3); terminal->pending_len = 3;
    memcpy(terminal->text, "old", 3); terminal->text_len = 3;
    terminal->kind = SG_TERM_KEY; terminal->code = 'y'; terminal->modifiers = 1;
    terminal->escape_since = 1; terminal->paste = terminal->discard_paste = 1;
#ifdef _WIN32
    terminal->high_surrogate = 0xd83d;
#endif
}
static int discard_state_reset(SengooTerminal* terminal) {
    return !terminal->pending_len && !terminal->text_len && !terminal->kind && !terminal->code
        && !terminal->modifiers && !terminal->escape_since && !terminal->paste && !terminal->discard_paste
#ifdef _WIN32
        && !terminal->high_surrogate
#endif
        ;
}

#ifdef _WIN32
static int clear_after_cursor_probe(long long handle, HANDLE input, HANDLE output) {
    CONSOLE_SCREEN_BUFFER_INFO before, after;
    DWORD input_mode, output_mode, current_input, current_output, written;
    UINT input_cp = GetConsoleCP(), output_cp = GetConsoleOutputCP();
    if (!GetConsoleScreenBufferInfo(output, &before) || !GetConsoleMode(input, &input_mode)
        || !GetConsoleMode(output, &output_mode) || before.dwSize.X < 12
        || before.srWindow.Bottom + 2 >= before.dwSize.Y) return 100;
    COORD origin = { 5, (SHORT)(before.srWindow.Top + 1) };
    DWORD all = (DWORD)before.dwSize.X * (DWORD)before.dwSize.Y;
    COORD zero = { 0, 0 };
    if (!FillConsoleOutputCharacterW(output, L'X', all, zero, &written) || written != all
        || !FillConsoleOutputAttribute(output, 0x4e, all, zero, &written) || written != all
        || !SetConsoleTextAttribute(output, 0x1b) || !SetConsoleCursorPosition(output, origin)) return 101;
    CONSOLE_SCREEN_BUFFER_INFO marked;
    if (!GetConsoleScreenBufferInfo(output, &marked)
        || sengoo_terminal_clear_after_cursor(handle) != 1
        || !GetConsoleScreenBufferInfo(output, &after)) return 102;
    DWORD start = (DWORD)origin.Y * (DWORD)before.dwSize.X + (DWORD)origin.X;
    WCHAR chars[257]; WORD attributes[257];
    for (DWORD offset = 0; offset < all;) {
        DWORD amount = all - offset; if (amount > 257) amount = 257;
        COORD position = { (SHORT)(offset % before.dwSize.X), (SHORT)(offset / before.dwSize.X) };
        if (!ReadConsoleOutputCharacterW(output, chars, amount, position, &written) || written != amount
            || !ReadConsoleOutputAttribute(output, attributes, amount, position, &written) || written != amount) return 103;
        for (DWORD i = 0; i < amount; ++i) {
            if (chars[i] != (offset + i < start ? L'X' : L' ')
                || attributes[i] != (offset + i < start ? 0x4e : 0x1b)) return 104;
        }
        offset += amount;
    }
    if (after.dwCursorPosition.X != origin.X || after.dwCursorPosition.Y != origin.Y
        || memcmp(&after.srWindow, &marked.srWindow, sizeof(after.srWindow)) != 0 || after.wAttributes != marked.wAttributes
        || !GetConsoleMode(input, &current_input) || !GetConsoleMode(output, &current_output)
        || current_input != input_mode || current_output != output_mode
        || GetConsoleCP() != input_cp || GetConsoleOutputCP() != output_cp) return 105;
    if (!SetConsoleTextAttribute(output, before.wAttributes)
        || !SetConsoleCursorPosition(output, before.dwCursorPosition)
        || !SetConsoleWindowInfo(output, TRUE, &before.srWindow)) return 106;
    return 0;
}
static int discard_input_probe(long long handle, HANDLE input, HANDLE output) {
    DWORD before_input, before_output, after_input, after_output, count;
    UINT before_input_cp = GetConsoleCP(), before_output_cp = GetConsoleOutputCP();
    if (!GetConsoleMode(input, &before_input) || !GetConsoleMode(output, &before_output)) return 90;
    if (sengoo_terminal_discard_input(handle) != 1) return 91;
    INPUT_RECORD events[6]; memset(events, 0, sizeof(events));
    events[0].EventType = KEY_EVENT; events[0].Event.KeyEvent.bKeyDown = FALSE;
    events[0].Event.KeyEvent.uChar.UnicodeChar = L'y';
    events[1].EventType = MOUSE_EVENT;
    const wchar_t stale[] = L"yes\r";
    for (int i = 0; i < 4; ++i) {
        events[i + 2].EventType = KEY_EVENT;
        events[i + 2].Event.KeyEvent.bKeyDown = TRUE;
        events[i + 2].Event.KeyEvent.wRepeatCount = 1;
        events[i + 2].Event.KeyEvent.uChar.UnicodeChar = stale[i];
    }
    if (!WriteConsoleInputW(input, events, 6, &count) || count != 6) return 92;
    /* Polling skips unrelated ready records, but the stale suffix still needs
     * an explicit boundary that flushes OS records and every parser buffer. */
    if (sengoo_terminal_read_event(handle, 0) != SG_TERM_KEY
        || sengoo_terminal_event_field(handle, 1) != 'y') return 93;
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return 94;
    seed_discard_state(terminal);
    HANDLE owned_input = terminal->input, owned_output = terminal->output;
    if (sengoo_terminal_discard_input(handle) != 1 || !discard_state_reset(terminal)
        || !GetNumberOfConsoleInputEvents(input, &count) || count != 0
        || sengoo_terminal_read_event(handle, 0) != SG_TERM_NONE) return 95;
    if (terminal->input != owned_input || terminal->output != owned_output || !terminal->raw
        || !GetConsoleMode(input, &after_input) || !GetConsoleMode(output, &after_output)
        || before_input != after_input || before_output != after_output
        || before_input_cp != GetConsoleCP() || before_output_cp != GetConsoleOutputCP()) return 96;
    InterlockedExchange(&sengoo_terminal_cancelled, 1);
    if (sengoo_terminal_discard_input(handle) != 1
        || sengoo_terminal_read_event(handle, 0) != SG_TERM_CANCEL) return 97;
    events[0] = events[2]; events[0].Event.KeyEvent.uChar.UnicodeChar = L'x';
    if (!WriteConsoleInputW(input, events, 1, &count) || count != 1
        || sengoo_terminal_read_event(handle, 1000) != SG_TERM_KEY
        || sengoo_terminal_event_field(handle, 1) != 'x') return 98;
    return 0;
}
static int tty_probe(void) {
    DWORD input_mode, output_mode;
    HANDLE input = GetStdHandle(STD_INPUT_HANDLE), output = GetStdHandle(STD_OUTPUT_HANDLE);
    if (!GetConsoleMode(input, &input_mode) || !GetConsoleMode(output, &output_mode)) return 71;
    UINT input_cp = GetConsoleCP(), output_cp = GetConsoleOutputCP();
    long long terminal = sengoo_terminal_open();
    if (terminal <= 0 || sengoo_terminal_is_tty(terminal) != 1) return 72;
    if (sengoo_terminal_enter_raw(terminal) != 1) return 73;
    INPUT_RECORD events[2]; memset(events, 0, sizeof(events));
    for (int i = 0; i < 2; i++) { events[i].EventType = KEY_EVENT; events[i].Event.KeyEvent.bKeyDown = TRUE; events[i].Event.KeyEvent.wRepeatCount = 1; }
    events[0].Event.KeyEvent.uChar.UnicodeChar = L'\x6587';
    events[1].Event.KeyEvent.uChar.UnicodeChar = 3;
    DWORD written;
    if (!WriteConsoleInputW(input, events, 2, &written) || written != 2) return 74;
    if (sengoo_terminal_read_event(terminal, 1000) != SG_TERM_KEY
        || sengoo_terminal_event_field(terminal, 1) != 0x6587) return 75;
    if (sengoo_terminal_read_event(terminal, 1000) != SG_TERM_CANCEL) return 76;
    int discarded = discard_input_probe(terminal, input, output);
    if (discarded) return discarded;
    int cleared = clear_after_cursor_probe(terminal, input, output);
    if (cleared) return cleared;
    CONSOLE_SCREEN_BUFFER_INFO info;
    if (!GetConsoleScreenBufferInfo(output, &info)) return 77;
    SMALL_RECT window = info.srWindow;
    if (window.Right - window.Left < 10) return 78;
    window.Right--;
    if (!SetConsoleWindowInfo(output, TRUE, &window)) return 79;
    if (sengoo_terminal_read_event(terminal, 1000) != SG_TERM_RESIZE) return 80;
    if (sengoo_terminal_restore(terminal) != 1) return 81;
    DWORD current_in, current_out;
    if (!GetConsoleMode(input, &current_in) || !GetConsoleMode(output, &current_out)
        || current_in != input_mode || current_out != output_mode
        || GetConsoleCP() != input_cp || GetConsoleOutputCP() != output_cp) return 82;
    if (sengoo_terminal_enter_raw(terminal) != 1 || sengoo_terminal_close(terminal) != 0) return 83;
    if (!GetConsoleMode(input, &current_in) || current_in != input_mode
        || !GetConsoleMode(output, &current_out) || current_out != output_mode
        || sengoo_terminal_live_handle_count() != 0) return 84;
    return 0;
}
static void real_tty_test(const char* self) {
    long long command = sengoo_process_command_new(raw(self)); assert(command);
    assert(sengoo_process_command_arg(command, raw("--tty-probe")) == 1);
    char* line = sengoo_windows_process_command_line_dyn(sengoo_process_command_from_handle(command));
    wchar_t* wide = sengoo_process_utf16(line); assert(wide); free(line);
    STARTUPINFOW startup; PROCESS_INFORMATION info; memset(&startup, 0, sizeof(startup)); memset(&info, 0, sizeof(info));
    startup.cb = sizeof(startup); startup.dwFlags = STARTF_USESHOWWINDOW; startup.wShowWindow = SW_HIDE;
    assert(CreateProcessW(NULL, wide, NULL, NULL, FALSE, CREATE_NEW_CONSOLE, NULL, NULL, &startup, &info));
    free(wide); sengoo_process_command_close(command);
    DWORD waited = WaitForSingleObject(info.hProcess, 10000), code = 1;
    if (waited != WAIT_OBJECT_0) { TerminateProcess(info.hProcess, 1); WaitForSingleObject(info.hProcess, INFINITE); }
    assert(GetExitCodeProcess(info.hProcess, &code));
    CloseHandle(info.hThread); CloseHandle(info.hProcess);
    if (code) fprintf(stderr, "hidden console TTY probe failed: %lu\n", code);
    assert(waited == WAIT_OBJECT_0 && code == 0);
}
#else
static void real_tty_test(const char* self) {
    (void)self;
    int master = posix_openpt(O_RDWR | O_NOCTTY); assert(master >= 0);
    assert(grantpt(master) == 0 && unlockpt(master) == 0);
    char* slave_name = ptsname(master); assert(slave_name);
    int slave = open(slave_name, O_RDWR | O_NOCTTY); assert(slave >= 0);
    struct winsize size = {24,80,0,0}; assert(ioctl(slave, TIOCSWINSZ, &size) == 0);
    int control[2]; assert(pipe(control) == 0);
    pid_t child = fork(); assert(child >= 0);
    if (!child) {
        close(master); close(control[0]);
        assert(dup2(slave, 0) >= 0 && dup2(slave, 1) >= 0); close(slave);
        struct termios saved, after; assert(tcgetattr(0, &saved) == 0);
        long long terminal = sengoo_terminal_open(); assert(terminal > 0);
        assert(sengoo_terminal_is_tty(terminal) == 1 && sengoo_terminal_enter_raw(terminal) == 1);
        assert(write(control[1], "r", 1) == 1);
        assert(sengoo_terminal_read_event(terminal, 5000) == SG_TERM_KEY);
        assert(sengoo_terminal_event_field(terminal, 1) == 0x4e2d);
        assert(sengoo_terminal_read_event(terminal, 5000) == SG_TERM_PASTE);
        assert(sengoo_terminal_read_event(terminal, 5000) == SG_TERM_CANCEL);
        assert(write(control[1], "f", 1) == 1);
        struct pollfd queued = {0, POLLIN, 0};
        assert(poll(&queued, 1, 5000) == 1);
        SengooTerminal* owned = sengoo_terminal_resolve(terminal); assert(owned);
        seed_discard_state(owned);
        struct termios before_flush;
        assert(tcgetattr(0, &before_flush) == 0);
        sengoo_terminal_cancelled = 1;
        assert(sengoo_terminal_discard_input(terminal) == 1 && discard_state_reset(owned));
        assert(sengoo_terminal_read_event(terminal, 0) == SG_TERM_CANCEL);
        assert(sengoo_terminal_read_event(terminal, 0) == SG_TERM_NONE);
        assert(tcgetattr(0, &after) == 0 && before_flush.c_iflag == after.c_iflag
            && before_flush.c_lflag == after.c_lflag && before_flush.c_cflag == after.c_cflag
            && before_flush.c_oflag == after.c_oflag && memcmp(before_flush.c_cc, after.c_cc, sizeof(after.c_cc)) == 0);
        assert(write(control[1], "n", 1) == 1);
        assert(sengoo_terminal_read_event(terminal, 5000) == SG_TERM_KEY);
        assert(sengoo_terminal_event_field(terminal, 1) == 'x');
        assert(write(control[1], "s", 1) == 1);
        assert(sengoo_terminal_read_event(terminal, 5000) == SG_TERM_RESIZE);
        assert(sengoo_terminal_event_field(terminal, 3) == 79);
        assert(sengoo_terminal_restore(terminal) == 1);
        assert(tcgetattr(0, &after) == 0);
        assert(saved.c_iflag == after.c_iflag && saved.c_lflag == after.c_lflag
            && saved.c_cflag == after.c_cflag && saved.c_oflag == after.c_oflag
            && memcmp(saved.c_cc, after.c_cc, sizeof(saved.c_cc)) == 0);
        assert(sengoo_terminal_enter_raw(terminal) == 1 && sengoo_terminal_close(terminal) == 0);
        assert(tcgetattr(0, &after) == 0 && saved.c_lflag == after.c_lflag);
        close(control[1]); _exit(0);
    }
    close(slave); close(control[1]);
    struct pollfd ready = {control[0], POLLIN, 0}; char marker;
    assert(poll(&ready, 1, 5000) == 1 && read(control[0], &marker, 1) == 1 && marker == 'r');
    const char input[] = "\xe4\xb8\xad\033[200~paste\n\033[201~\003";
    assert(write(master, input, sizeof(input)-1) == sizeof(input)-1);
    assert(poll(&ready, 1, 5000) == 1 && read(control[0], &marker, 1) == 1 && marker == 'f');
    assert(write(master, "yes\n", 4) == 4);
    assert(poll(&ready, 1, 5000) == 1 && read(control[0], &marker, 1) == 1 && marker == 'n');
    assert(write(master, "x", 1) == 1);
    assert(poll(&ready, 1, 5000) == 1 && read(control[0], &marker, 1) == 1 && marker == 's');
    size.ws_col = 79; assert(ioctl(master, TIOCSWINSZ, &size) == 0);
    int status; assert(waitpid(child, &status, 0) == child && WIFEXITED(status) && WEXITSTATUS(status) == 0);
    close(master); close(control[0]);
}
#endif

static int probe_main(int argc, char** argv) {
#ifdef _WIN32
    if (argc > 1 && strcmp(argv[1], "--tty-probe") == 0) return tty_probe();
#endif
    if (argc > 1 && strcmp(argv[1], "--flood") == 0) {
        char chunk[4096]; memset(chunk, 'f', sizeof(chunk));
        for (int i = 0; i < 128; i++) { child_write(1, chunk, sizeof(chunk)); child_write(2, chunk, sizeof(chunk)); }
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "--sleep") == 0) { pause_ms(30000); return 0; }
    if (argc > 1 && strcmp(argv[1], "--exit") == 0) return 0;
    if (argc > 1 && strcmp(argv[1], "--echo") == 0) {
        unsigned char input[256];
#ifdef _WIN32
        DWORD n; assert(ReadFile(GetStdHandle(STD_INPUT_HANDLE), input, sizeof(input), &n, NULL));
#else
        ssize_t n = read(0, input, sizeof(input)); assert(n >= 0);
#endif
        child_write(1, input, n); return 0;
    }
    if (argc > 1 && strcmp(argv[1], "--unicode") == 0) {
        assert(argc == 3 && strcmp(argv[2], "中文 \"参数\"") == 0);
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "--cwd-env") == 0) {
        assert(argc==3);
#ifdef _WIN32
        wchar_t current[32768], environment[128];char current_utf8[32768],environment_utf8[256];
        assert(GetCurrentDirectoryW(32768,current));
        assert(GetEnvironmentVariableW(L"SENGOO_NATIVE_PROBE",environment,128));
        assert(WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,current,-1,current_utf8,sizeof(current_utf8),NULL,NULL));
        assert(WideCharToMultiByte(CP_UTF8,WC_ERR_INVALID_CHARS,environment,-1,environment_utf8,sizeof(environment_utf8),NULL,NULL));
        assert(!strcmp(current_utf8,argv[2])&&!strcmp(environment_utf8,"环境-value"));
#else
        char current[32768];assert(getcwd(current,sizeof(current))&&!strcmp(current,argv[2]));
        assert(getenv("SENGOO_NATIVE_PROBE")&&!strcmp(getenv("SENGOO_NATIVE_PROBE"),"环境-value"));
#endif
        return 0;
    }
    if (argc > 1 && strcmp(argv[1], "--grandchild") == 0) {
#ifdef _WIN32
        long long child = spawn_child(argv[0], "--sleep");
        SengooProcessHandleState* state = sengoo_process_handle_resolve(child); assert(state);
        long long pid = GetProcessId(state->process);
#else
        pid_t pid = fork(); assert(pid >= 0);
        if (!pid) { for (;;) pause_ms(100); }
#endif
        long long child_pid = pid;
        child_write(1, &child_pid, sizeof(child_pid));
        for (;;) pause_ms(100);
    }
    parser_tests();
    unicode_environment_test(argv[0]);
    real_tty_test(argv[0]);
    drain(spawn_child(argv[0], "--flood"), 128 * 4096, 128 * 4096);
    long long child = spawn_child(argv[0], "--sleep");
    assert(sengoo_process_handle_wait(child, 0) == -SENGOO_STATUS_TIMEOUT);
    assert(sengoo_process_handle_try_wait(child) == 0);
    assert(sengoo_process_handle_exit_code(child) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_process_handle_wait_cancellable(child, 1) == -SENGOO_STATUS_TIMEOUT);
    assert(sengoo_process_handle_try_wait(child) == 0);
    assert(sengoo_process_handle_terminate_tree(child) == 1);
    assert(sengoo_process_handle_reap(child, 5000) >= 0);
    assert(sengoo_process_handle_close(child) == 0);
    child = spawn_child(argv[0], "--echo");
    unsigned char payload[] = "stdin 世界"; SengooFfiBuffer input = { payload, sizeof(payload), sizeof(payload) - 1 };
    assert(sengoo_process_handle_write_stdin(child, raw(&input), sizeof(payload) - 1) == sizeof(payload) - 1);
    assert(sengoo_process_handle_close_stdin(child) == 1);
    drain(child, sizeof(payload) - 1, 0);
    child = spawn_child(argv[0], "--exit");
    assert(sengoo_process_handle_reap(child, 5000) == 0);
#ifndef _WIN32
    struct sigaction saved_pipe, ignored_pipe; memset(&ignored_pipe, 0, sizeof(ignored_pipe));
    ignored_pipe.sa_handler = SIG_IGN; sigemptyset(&ignored_pipe.sa_mask);
    assert(sigaction(SIGPIPE, &ignored_pipe, &saved_pipe) == 0);
#endif
    assert(sengoo_process_handle_write_stdin(child, raw(&input), sizeof(payload) - 1) == -SENGOO_STATUS_IO);
#ifndef _WIN32
    assert(sigaction(SIGPIPE, &saved_pipe, NULL) == 0);
#endif
    assert(sengoo_process_handle_close(child) == 0);
    child = spawn_child(argv[0], "--sleep");
    unsigned char* large = malloc(SENGOO_PROCESS_IO_CHUNK); assert(large);
    memset(large, 'b', SENGOO_PROCESS_IO_CHUNK);
    SengooFfiBuffer large_input = {large, SENGOO_PROCESS_IO_CHUNK, SENGOO_PROCESS_IO_CHUNK};
    long long write_started = sengoo_process_now_ms(); int blocked = 0;
    for (int i = 0; i < 100; i++) {
        long long n = sengoo_process_handle_write_stdin(child, raw(&large_input), SENGOO_PROCESS_IO_CHUNK);
        assert(n >= 0); if (n == 0) { blocked = 1; break; }
    }
    assert(blocked && sengoo_process_now_ms() - write_started < 1000);
    free(large); assert(sengoo_process_handle_close(child) == 0);
    long long command = sengoo_process_command_new(raw(argv[0]));
    assert(sengoo_process_command_arg(command, raw("--unicode")) == 1);
    assert(sengoo_process_command_arg(command, raw("中文 \"参数\"")) == 1);
    child = sengoo_process_command_spawn_piped(command); assert(child > 0);
    sengoo_process_command_close(command);
    drain(child, 0, 0);
    child = spawn_child(argv[0], "--grandchild");
    unsigned char pid_data[8]; SengooFfiBuffer pid_buffer = {pid_data, 8, 0};
    long long deadline = sengoo_process_now_ms() + 5000;
    while (sengoo_process_handle_read(child, 1, raw(&pid_buffer)) == 0) { assert(sengoo_process_now_ms() < deadline); pause_ms(1); }
    long long grandchild; memcpy(&grandchild, pid_data, sizeof(grandchild));
#ifdef _WIN32
    HANDLE process = OpenProcess(SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, FALSE, (DWORD)grandchild); assert(process);
#endif
    assert(sengoo_process_handle_close(child) == 0);
#ifdef _WIN32
    assert(WaitForSingleObject(process, 5000) == WAIT_OBJECT_0); CloseHandle(process);
#else
    /* A dead orphan may remain a zombie until init reaps it. */
    pause_ms(50);
#if defined(__linux__)
    char stat_path[128]; snprintf(stat_path, sizeof(stat_path), "/proc/%lld/stat", grandchild);
    FILE* stat = fopen(stat_path, "r");
    if (stat) {
        char line[1024]; assert(fgets(line, sizeof(line), stat)); fclose(stat);
        char* command_end = strrchr(line, ')'); assert(command_end && command_end[2] == 'Z');
    }
#else
    assert(kill((pid_t)grandchild, 0) == -1 && errno == ESRCH);
#endif
#endif
    long long terminal = sengoo_terminal_open(); assert(terminal > 0);
    assert(sengoo_terminal_is_tty(terminal) == 0);
    assert(sengoo_terminal_enter_raw(terminal) == -SENGOO_STATUS_UNSUPPORTED);
    assert(sengoo_terminal_discard_input(terminal) == -SENGOO_STATUS_UNSUPPORTED);
    assert(sengoo_terminal_clear_after_cursor(terminal) == -SENGOO_STATUS_UNSUPPORTED);
    assert(sengoo_terminal_read_event(terminal, -1) == -SENGOO_STATUS_INVALID_ARGUMENT);
    assert(sengoo_terminal_close(terminal) == 0 && sengoo_terminal_live_handle_count() == 0);
    assert(sengoo_terminal_restore(terminal) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_terminal_discard_input(terminal) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_terminal_discard_input(0) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_terminal_clear_after_cursor(terminal) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_terminal_clear_after_cursor(0) == -SENGOO_STATUS_INVALID_HANDLE);
    void* foreign = malloc(1); assert(foreign);
    long long foreign_handle = sengoo_opaque_handle_new(foreign);
    assert(sengoo_terminal_restore(foreign_handle) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_terminal_discard_input(foreign_handle) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_terminal_clear_after_cursor(foreign_handle) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_process_command_arg(foreign_handle, raw("x")) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_process_output_exit_code(foreign_handle) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_process_handle_try_wait(foreign_handle) == -SENGOO_STATUS_INVALID_HANDLE);
    assert(sengoo_opaque_handle_take(foreign_handle) == foreign); free(foreign);
    size_t baseline_slots = sengoo_process_owned_count;
    for (int i = 0; i < 10000; i++) {
        long long owner = sengoo_process_command_new(raw("unexecuted-probe")); assert(owner > 0);
        long long output = sengoo_process_output_register(sengoo_process_output_new()); assert(output > 0);
        assert(sengoo_process_output_exit_code(owner) == -SENGOO_STATUS_INVALID_HANDLE);
        assert(sengoo_process_command_arg(output, raw("x")) == -SENGOO_STATUS_INVALID_HANDLE);
        assert(sengoo_process_command_close(owner) == 0 && sengoo_process_command_close(owner) == 0);
        assert(sengoo_process_output_close(output) == 0 && sengoo_process_output_close(output) == 0);
        assert(sengoo_process_command_arg(owner, raw("x")) == -SENGOO_STATUS_INVALID_HANDLE);
        assert(sengoo_process_output_exit_code(output) == -SENGOO_STATUS_INVALID_HANDLE);
    }
    assert(sengoo_process_owned_count <= baseline_slots + 2 && sengoo_process_live_resource_count() == 0);
    puts("process/terminal: flood, timeout, Unicode, stdin, descendant cleanup, parser, non-TTY passed");
    return 0;
}
#ifdef _WIN32
int wmain(int argc, wchar_t** wide) {
    char** args = calloc((size_t)argc + 1, sizeof(char*)); assert(args);
    for (int i = 0; i < argc; i++) {
        int length = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide[i], -1, NULL, 0, NULL, NULL);
        args[i] = malloc((size_t)length); assert(args[i]);
        assert(WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide[i], -1, args[i], length, NULL, NULL));
    }
    int code = probe_main(argc, args);
    for (int i = 0; i < argc; i++) free(args[i]); free(args); return code;
}
#else
int main(int argc, char** argv) { return probe_main(argc, argv); }
#endif
