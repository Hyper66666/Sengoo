#define _CRT_SECURE_NO_WARNINGS
#if !defined(_WIN32)
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#endif
#endif
#include "runtime_shared.h"
#include <errno.h>
#include <limits.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <wchar.h>
#ifdef _WIN32
#include <windows.h>
#include <objidl.h>
#else
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <sys/ioctl.h>
#include <termios.h>
#include <time.h>
#include <unistd.h>
#endif

/* This is a byte/key/console bridge, not a line editor or renderer. */
#define SENGOO_TERMINAL_MAGIC UINT64_C(0x73677465726d7631)
#define SENGOO_TERMINAL_LIMIT 65536
enum { SG_TERM_NONE, SG_TERM_KEY, SG_TERM_PASTE, SG_TERM_RESIZE, SG_TERM_CANCEL, SG_TERM_EOF, SG_TERM_MOUSE };
typedef struct SengooTerminal {
    uint64_t magic;
    long long handle;
    struct SengooTerminal* next;
#ifdef _WIN32
    HANDLE input, output;
    DWORD input_mode, output_mode;
    UINT input_cp, output_cp;
    wchar_t high_surrogate;
    WORD repeat_left;
    long long repeat_code, repeat_modifiers;
    unsigned char repeat_bytes[4];
    size_t repeat_len;
    DWORD mouse_buttons;
    CONSOLE_CURSOR_INFO cursor_before_screen;
    int cursor_before_valid;
#else
    int input, output;
    struct termios saved;
    struct sigaction interrupt_action, terminate_action;
#endif
    int input_tty, output_tty, raw, eof, paste, discard_paste;
    int fullscreen, screen_owned_raw, screen_mouse_before, mouse;
    long long kind, code, modifiers, width, height, escape_since;
    long long mouse_x, mouse_y, mouse_button, mouse_action, scroll_x, scroll_y, mouse_clicks;
    size_t pending_len, text_len;
    unsigned char pending[SENGOO_TERMINAL_LIMIT + 8];
    unsigned char text[SENGOO_TERMINAL_LIMIT];
} SengooTerminal;
static SengooTerminal* sengoo_terminal_raw_owner;
static SengooTerminal* sengoo_terminal_handles;
static long long sengoo_terminal_live_count;
#ifdef _WIN32
static volatile LONG sengoo_terminal_cancelled;
static BOOL WINAPI sengoo_terminal_control(DWORD event) {
    if (event == CTRL_C_EVENT || event == CTRL_BREAK_EVENT || event == CTRL_CLOSE_EVENT) {
        InterlockedExchange(&sengoo_terminal_cancelled, 1); return TRUE;
    }
    return FALSE;
}
#else
static volatile sig_atomic_t sengoo_terminal_cancelled;
static void sengoo_terminal_control(int signal_number) { (void)signal_number; sengoo_terminal_cancelled = 1; }
#endif
long long sengoo_string_from_bytes_copy(long long bytes, long long length);
long long sengoo_terminal_restore(long long handle);
long long sengoo_terminal_set_mouse(long long handle, long long enabled);
static long long sengoo_terminal_clock(void) {
#ifdef _WIN32
    return (long long)GetTickCount64();
#else
    struct timespec now; clock_gettime(CLOCK_MONOTONIC, &now);
    return (long long)now.tv_sec * 1000 + now.tv_nsec / 1000000;
#endif
}
static SengooTerminal* sengoo_terminal_resolve(long long handle) {
    for (SengooTerminal* terminal = sengoo_terminal_handles; terminal; terminal = terminal->next)
        if (terminal->handle == handle) return terminal;
    return NULL;
}
static void sengoo_terminal_dimensions(SengooTerminal* terminal, long long* width, long long* height) {
    *width = *height = 0;
#ifdef _WIN32
    CONSOLE_SCREEN_BUFFER_INFO info;
    if (terminal->output_tty && GetConsoleScreenBufferInfo(terminal->output, &info)) {
        *width = info.srWindow.Right - info.srWindow.Left + 1;
        *height = info.srWindow.Bottom - info.srWindow.Top + 1;
    }
#else
    struct winsize size;
    if (terminal->output_tty && ioctl(terminal->output, TIOCGWINSZ, &size) == 0) {
        *width = size.ws_col; *height = size.ws_row;
    }
#endif
}
static int sengoo_terminal_write_bytes(SengooTerminal* terminal, const unsigned char* data, size_t length) {
#ifdef _WIN32
    if (terminal->output_tty) {
        if (!length) return 1;
        int size = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, (const char*)data, (int)length, NULL, 0);
        if (!size) return 0;
        wchar_t* wide = (wchar_t*)malloc((size_t)size * sizeof(wchar_t));
        if (!wide) return 0;
        DWORD written = 0;
        int ok = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, (const char*)data, (int)length, wide, size)
            && WriteConsoleW(terminal->output, wide, (DWORD)size, &written, NULL) && written == (DWORD)size;
        free(wide); return ok;
    }
    DWORD written = 0;
    return WriteFile(terminal->output, data, (DWORD)length, &written, NULL) && written == length;
#else
    size_t offset = 0;
    while (offset < length) {
        ssize_t n = write(terminal->output, data + offset, length - offset);
        if (n < 0 && errno == EINTR) continue;
        if (n <= 0) return 0;
        offset += (size_t)n;
    }
    return 1;
#endif
}
static int sengoo_terminal_leave_screen(SengooTerminal* terminal) {
    if (!terminal->fullscreen) return 1;
    int ok = 1;
    const unsigned char leave[] = "\033[?1049l\033[0m\033[?25h";
    if (!sengoo_terminal_write_bytes(terminal, leave, sizeof(leave) - 1)) ok = 0;
#ifdef _WIN32
    if (terminal->cursor_before_valid
        && !SetConsoleCursorInfo(terminal->output, &terminal->cursor_before_screen)) ok = 0;
#endif
    terminal->fullscreen = 0;
    return ok;
}
static int sengoo_terminal_restore_state(SengooTerminal* terminal) {
    int ok = sengoo_terminal_leave_screen(terminal);
    terminal->screen_owned_raw = terminal->screen_mouse_before = 0;
    if (!terminal->raw) return ok;
    const unsigned char reset[] = "\033[?1000l\033[?1002l\033[?1003l\033[?1006l\033[?2004l\033[0m\033[?25h";
    if (terminal->output_tty && !sengoo_terminal_write_bytes(terminal, reset, sizeof(reset) - 1)) ok = 0;
#ifdef _WIN32
    if (!SetConsoleMode(terminal->input, terminal->input_mode)) ok = 0;
    if (terminal->output_tty && !SetConsoleMode(terminal->output, terminal->output_mode)) ok = 0;
    if (!SetConsoleCP(terminal->input_cp) || !SetConsoleOutputCP(terminal->output_cp)) ok = 0;
    SetConsoleCtrlHandler(sengoo_terminal_control, FALSE);
    if (terminal->cursor_before_valid
        && !SetConsoleCursorInfo(terminal->output, &terminal->cursor_before_screen)) ok = 0;
    terminal->cursor_before_valid = 0;
#else
    if (tcsetattr(terminal->input, TCSANOW, &terminal->saved)) ok = 0;
    sigaction(SIGINT, &terminal->interrupt_action, NULL);
    sigaction(SIGTERM, &terminal->terminate_action, NULL);
#endif
    terminal->raw = terminal->mouse = 0;
    if (sengoo_terminal_raw_owner == terminal) sengoo_terminal_raw_owner = NULL;
    return ok;
}
static void sengoo_terminal_at_exit(void) {
    if (sengoo_terminal_raw_owner) sengoo_terminal_restore_state(sengoo_terminal_raw_owner);
}
long long sengoo_terminal_open(void) {
    SengooTerminal* terminal = (SengooTerminal*)calloc(1, sizeof(*terminal));
    if (!terminal) return -SENGOO_STATUS_OUT_OF_MEMORY;
#ifdef _WIN32
    HANDLE self = GetCurrentProcess(); DWORD mode;
    if (!DuplicateHandle(self, GetStdHandle(STD_INPUT_HANDLE), self, &terminal->input, 0, FALSE, DUPLICATE_SAME_ACCESS)
        || !DuplicateHandle(self, GetStdHandle(STD_OUTPUT_HANDLE), self, &terminal->output, 0, FALSE, DUPLICATE_SAME_ACCESS)) {
        if (terminal->input) CloseHandle(terminal->input);
        free(terminal); return -SENGOO_STATUS_IO;
    }
    terminal->input_tty = GetConsoleMode(terminal->input, &mode) != 0;
    terminal->output_tty = GetConsoleMode(terminal->output, &mode) != 0;
#else
    terminal->input = dup(STDIN_FILENO); terminal->output = dup(STDOUT_FILENO);
    if (terminal->input < 0 || terminal->output < 0) {
        if (terminal->input >= 0) close(terminal->input);
        if (terminal->output >= 0) close(terminal->output);
        free(terminal); return -SENGOO_STATUS_IO;
    }
    fcntl(terminal->input, F_SETFD, FD_CLOEXEC); fcntl(terminal->output, F_SETFD, FD_CLOEXEC);
    terminal->input_tty = isatty(terminal->input); terminal->output_tty = isatty(terminal->output);
#endif
    terminal->magic = SENGOO_TERMINAL_MAGIC;
    sengoo_terminal_dimensions(terminal, &terminal->width, &terminal->height);
    long long handle = sengoo_opaque_handle_new(terminal);
    if (handle <= 0) {
#ifdef _WIN32
        CloseHandle(terminal->input); CloseHandle(terminal->output);
#else
        close(terminal->input); close(terminal->output);
#endif
        free(terminal); return -SENGOO_STATUS_OUT_OF_MEMORY;
    }
    sengoo_terminal_live_count++;
    terminal->handle = handle; terminal->next = sengoo_terminal_handles;
    sengoo_terminal_handles = terminal;
    return handle;
}
long long sengoo_terminal_is_tty(long long handle) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    return terminal ? terminal->input_tty && terminal->output_tty : -SENGOO_STATUS_INVALID_HANDLE;
}
long long sengoo_terminal_enter_raw(long long handle) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    if (terminal->raw) return 1;
    if (!terminal->input_tty || !terminal->output_tty) return -SENGOO_STATUS_UNSUPPORTED;
    if (sengoo_terminal_raw_owner) return -SENGOO_STATUS_ALREADY_EXISTS;
#ifdef _WIN32
    if (!GetConsoleMode(terminal->input, &terminal->input_mode)
        || !GetConsoleMode(terminal->output, &terminal->output_mode)) return -SENGOO_STATUS_IO;
    terminal->input_cp = GetConsoleCP(); terminal->output_cp = GetConsoleOutputCP();
    DWORD mode = (terminal->input_mode & ~(ENABLE_LINE_INPUT | ENABLE_ECHO_INPUT | ENABLE_PROCESSED_INPUT | ENABLE_QUICK_EDIT_MODE | ENABLE_VIRTUAL_TERMINAL_INPUT | ENABLE_MOUSE_INPUT))
        | ENABLE_EXTENDED_FLAGS | ENABLE_WINDOW_INPUT;
    if (!SetConsoleMode(terminal->input, mode)) return -SENGOO_STATUS_IO;
    terminal->raw = 1;
    if (!SetConsoleMode(terminal->output, terminal->output_mode | ENABLE_VIRTUAL_TERMINAL_PROCESSING)
        || !SetConsoleCP(CP_UTF8) || !SetConsoleOutputCP(CP_UTF8)
        || !SetConsoleCtrlHandler(sengoo_terminal_control, TRUE)) {
        sengoo_terminal_restore_state(terminal); return -SENGOO_STATUS_IO;
    }
#else
    if (tcgetattr(terminal->input, &terminal->saved)) return -SENGOO_STATUS_IO;
    struct termios raw = terminal->saved;
    raw.c_iflag &= ~(BRKINT | ICRNL | INPCK | ISTRIP | IXON);
    raw.c_cflag |= CS8; raw.c_lflag &= ~(ECHO | ICANON | IEXTEN | ISIG);
    raw.c_cc[VMIN] = 0; raw.c_cc[VTIME] = 0;
    if (tcsetattr(terminal->input, TCSANOW, &raw)) return -SENGOO_STATUS_IO;
    struct sigaction action; memset(&action, 0, sizeof(action));
    action.sa_handler = sengoo_terminal_control; sigemptyset(&action.sa_mask);
    if (sigaction(SIGINT, &action, &terminal->interrupt_action)) {
        tcsetattr(terminal->input, TCSANOW, &terminal->saved); return -SENGOO_STATUS_IO;
    }
    if (sigaction(SIGTERM, &action, &terminal->terminate_action)) {
        sigaction(SIGINT, &terminal->interrupt_action, NULL);
        tcsetattr(terminal->input, TCSANOW, &terminal->saved); return -SENGOO_STATUS_IO;
    }
    terminal->raw = 1;
#endif
    sengoo_terminal_cancelled = 0; sengoo_terminal_raw_owner = terminal;
    static int registered;
    if (!registered) { atexit(sengoo_terminal_at_exit); registered = 1; }
    const unsigned char paste[] = "\033[?2004h";
    if (!sengoo_terminal_write_bytes(terminal, paste, sizeof(paste) - 1)) {
        sengoo_terminal_restore_state(terminal); return -SENGOO_STATUS_IO;
    }
    return 1;
}
long long sengoo_terminal_set_mouse(long long handle, long long enabled) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    if (enabled != 0 && enabled != 1) return -SENGOO_STATUS_INVALID_ARGUMENT;
    if (!terminal->input_tty || !terminal->output_tty) return -SENGOO_STATUS_UNSUPPORTED;
    if (!terminal->raw) return -SENGOO_STATUS_INVALID_ARGUMENT;
    if (terminal->mouse == enabled) return 1;
#ifdef _WIN32
    DWORD mode;
    if (!GetConsoleMode(terminal->input, &mode)) return -SENGOO_STATUS_IO;
    mode = enabled ? mode | ENABLE_MOUSE_INPUT : mode & ~ENABLE_MOUSE_INPUT;
    if (!SetConsoleMode(terminal->input, mode)) return -SENGOO_STATUS_IO;
    terminal->mouse_buttons = 0;
#else
    const char* sequence = enabled ? "\033[?1003h\033[?1006h" : "\033[?1003l\033[?1002l\033[?1000l\033[?1006l";
    if (!sengoo_terminal_write_bytes(terminal, (const unsigned char*)sequence, strlen(sequence))) return -SENGOO_STATUS_IO;
#endif
    terminal->mouse = (int)enabled;
    return 1;
}
long long sengoo_terminal_enter_fullscreen(long long handle) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    if (terminal->fullscreen) return 1;
    int owned_raw = !terminal->raw;
    long long status = sengoo_terminal_enter_raw(handle);
    if (status < 0) return status;
    terminal->screen_owned_raw = owned_raw;
    terminal->screen_mouse_before = terminal->mouse;
#ifdef _WIN32
    terminal->cursor_before_valid = GetConsoleCursorInfo(terminal->output, &terminal->cursor_before_screen) != 0;
#endif
    const unsigned char enter[] = "\033[?1049h\033[2J\033[H\033[?25l";
    terminal->fullscreen = 1;
    if (!sengoo_terminal_write_bytes(terminal, enter, sizeof(enter) - 1)
        || sengoo_terminal_set_mouse(handle, 1) < 0) {
        sengoo_terminal_leave_screen(terminal);
        if (owned_raw) sengoo_terminal_restore_state(terminal);
        else {
            sengoo_terminal_set_mouse(handle, terminal->screen_mouse_before);
            terminal->screen_owned_raw = terminal->screen_mouse_before = 0;
#ifdef _WIN32
            terminal->cursor_before_valid = 0;
#endif
        }
        return -SENGOO_STATUS_IO;
    }
    return 1;
}
long long sengoo_terminal_leave_fullscreen(long long handle) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    if (!terminal->fullscreen) return 1;
    int owned_raw = terminal->screen_owned_raw, previous_mouse = terminal->screen_mouse_before;
    int ok = sengoo_terminal_leave_screen(terminal);
    terminal->screen_owned_raw = terminal->screen_mouse_before = 0;
    if (owned_raw) {
        if (!sengoo_terminal_restore_state(terminal)) ok = 0;
    } else {
        if (sengoo_terminal_set_mouse(handle, previous_mouse) < 0) ok = 0;
#ifdef _WIN32
        terminal->cursor_before_valid = 0;
#endif
    }
    return ok ? 1 : -SENGOO_STATUS_IO;
}
long long sengoo_terminal_restore(long long handle) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    return sengoo_terminal_restore_state(terminal) ? 1 : -SENGOO_STATUS_IO;
}
long long sengoo_terminal_discard_input(long long handle) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    if (!terminal->input_tty || !terminal->output_tty) return -SENGOO_STATUS_UNSUPPORTED;
#ifdef _WIN32
    if (!FlushConsoleInputBuffer(terminal->input)) return -SENGOO_STATUS_IO;
#else
    if (tcflush(terminal->input, TCIFLUSH)) return -SENGOO_STATUS_IO;
#endif
    /* Discard parsed/partial input as well as OS events. Cancellation raised
     * through a console/signal handler remains pending for read_event(). */
    terminal->pending_len = terminal->text_len = 0;
    terminal->kind = terminal->code = terminal->modifiers = terminal->escape_since = 0;
    terminal->mouse_x = terminal->mouse_y = terminal->mouse_button = terminal->mouse_action = 0;
    terminal->scroll_x = terminal->scroll_y = terminal->mouse_clicks = 0;
    terminal->paste = terminal->discard_paste = 0;
#ifdef _WIN32
    terminal->high_surrogate = 0;
    terminal->mouse_buttons = 0;
    terminal->repeat_left = 0;
    terminal->repeat_len = 0;
#endif
    return 1;
}
long long sengoo_terminal_clear_after_cursor(long long handle) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    if (!terminal->input_tty || !terminal->output_tty) return -SENGOO_STATUS_UNSUPPORTED;
#ifdef _WIN32
    CONSOLE_SCREEN_BUFFER_INFO info;
    if (!GetConsoleScreenBufferInfo(terminal->output, &info)) return -SENGOO_STATUS_IO;
    if (info.dwSize.X <= 0 || info.dwSize.Y <= 0 || info.dwCursorPosition.X < 0
        || info.dwCursorPosition.Y < 0 || info.dwCursorPosition.X >= info.dwSize.X
        || info.dwCursorPosition.Y >= info.dwSize.Y) return -SENGOO_STATUS_IO;
    DWORD cells = (DWORD)(info.dwSize.Y - info.dwCursorPosition.Y) * (DWORD)info.dwSize.X
        - (DWORD)info.dwCursorPosition.X;
    DWORD written;
    if (!FillConsoleOutputCharacterW(terminal->output, L' ', cells, info.dwCursorPosition, &written)
        || written != cells
        || !FillConsoleOutputAttribute(terminal->output, info.wAttributes, cells, info.dwCursorPosition, &written)
        || written != cells) return -SENGOO_STATUS_IO;
#else
    const unsigned char clear[] = "\033[J";
    if (!sengoo_terminal_write_bytes(terminal, clear, sizeof(clear) - 1)) return -SENGOO_STATUS_IO;
#endif
    return 1;
}
long long sengoo_terminal_write(long long handle, long long bytes, long long length) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    if (length < 0 || length > SENGOO_TERMINAL_LIMIT || (length && !bytes)) return -SENGOO_STATUS_INVALID_ARGUMENT;
    return sengoo_terminal_write_bytes(terminal, (const unsigned char*)(intptr_t)bytes, (size_t)length) ? length : -SENGOO_STATUS_IO;
}
/* Frame content/layout belongs to Sengoo. This boundary only admits a write
 * against live terminal geometry; a rejected/raced write is not a valid cache. */
long long sengoo_terminal_write_frame(long long handle, long long bytes, long long length, long long width, long long height) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    if (length < 0 || length > SENGOO_TERMINAL_LIMIT || (length && !bytes) || width < 1 || height < 1)
        return -SENGOO_STATUS_INVALID_ARGUMENT;
    if (!terminal->output_tty) return -SENGOO_STATUS_UNSUPPORTED;
    long long actual_width, actual_height;
    sengoo_terminal_dimensions(terminal, &actual_width, &actual_height);
    if (actual_width != width || actual_height != height) return 0;
    if (!sengoo_terminal_write_bytes(terminal, (const unsigned char*)(intptr_t)bytes, (size_t)length)) return -SENGOO_STATUS_IO;
    sengoo_terminal_dimensions(terminal, &actual_width, &actual_height);
    return actual_width == width && actual_height == height;
}
static void sengoo_terminal_consume(SengooTerminal* terminal, size_t length) {
    memmove(terminal->pending, terminal->pending + length, terminal->pending_len - length);
    terminal->pending_len -= length; terminal->escape_since = 0;
}
/* Strict scalar UTF-8 validation, independent of locale and console codepage. */
static int sengoo_terminal_utf8(const unsigned char* text, size_t length, unsigned* scalar) {
    if (!length) return 0;
    unsigned c = text[0]; int n;
    if (c < 128) n = 1;
    else if (c >= 0xc2 && c <= 0xdf) { n = 2; c &= 31; }
    else if (c >= 0xe0 && c <= 0xef) { n = 3; c &= 15; }
    else if (c >= 0xf0 && c <= 0xf4) { n = 4; c &= 7; }
    else return -1;
    if (length < (size_t)n) return 0;
    for (int i = 1; i < n; i++) { if ((text[i] & 0xc0) != 0x80) return -1; c = (c << 6) | (text[i] & 63); }
    if ((n == 2 && c < 128) || (n == 3 && c < 2048) || (n == 4 && c < 65536)
        || c > 0x10ffff || (c >= 0xd800 && c <= 0xdfff)) return -1;
    *scalar = c; return n;
}
/* SGR mouse transport only; hit testing, selection and scrolling belong to
 * the Sengoo renderer. A complete malformed packet is consumed once. */
static long long sengoo_terminal_parse_mouse(SengooTerminal* terminal) {
    unsigned char* p = terminal->pending;
    size_t n = terminal->pending_len, at = 3;
    unsigned values[3] = {0, 0, 0};
    for (int field = 0; field < 3; field++) {
        size_t start = at;
        while (at < n && p[at] >= '0' && p[at] <= '9') {
            if (values[field] > 100000 || at >= 63) {
                sengoo_terminal_consume(terminal, at + 1); return -SENGOO_STATUS_PARSE;
            }
            values[field] = values[field] * 10 + p[at++] - '0';
        }
        if (at == n) return 0;
        if (at == start || (field < 2 ? p[at] != ';' : p[at] != 'M' && p[at] != 'm')) {
            sengoo_terminal_consume(terminal, at + 1); return -SENGOO_STATUS_PARSE;
        }
        if (field < 2) at++;
    }
    if (!values[1] || !values[2] || values[0] > 255) {
        sengoo_terminal_consume(terminal, at + 1); return -SENGOO_STATUS_PARSE;
    }
    unsigned button = values[0];
    terminal->mouse_x = values[1] - 1; terminal->mouse_y = values[2] - 1;
    terminal->modifiers = ((button & 4) ? 1 : 0) | ((button & 8) ? 2 : 0) | ((button & 16) ? 4 : 0);
    terminal->mouse_button = (button & 128) ? 4 + (button & 1) : (button & 3) == 3 ? 0 : 1 + (button & 3);
    terminal->mouse_action = p[at] == 'm' ? 2 : (button & 32) ? 3 : (button & 3) == 3 ? 2 : 1;
    terminal->scroll_x = terminal->scroll_y = terminal->mouse_clicks = 0;
    if (button & 64) {
        terminal->mouse_button = 0; terminal->mouse_action = 4;
        if ((button & 3) < 2) terminal->scroll_y = (button & 1) ? -120 : 120;
        else terminal->scroll_x = (button & 1) ? 120 : -120;
    } else if (terminal->mouse_action == 1) terminal->mouse_clicks = 1;
    sengoo_terminal_consume(terminal, at + 1);
    return SG_TERM_MOUSE;
}
static long long sengoo_terminal_parse(SengooTerminal* terminal) {
    if (!terminal->pending_len) return 0;
    unsigned char* p = terminal->pending; size_t n = terminal->pending_len;
    if (terminal->paste || terminal->discard_paste) {
        for (size_t i = 0; i + 6 <= n; i++) if (memcmp(p + i, "\033[201~", 6) == 0) {
            if (terminal->discard_paste) {
                terminal->discard_paste = terminal->paste = 0; sengoo_terminal_consume(terminal, i + 6); return 0;
            }
            if (i > SENGOO_TERMINAL_LIMIT) {
                terminal->paste = terminal->discard_paste = 0;
                sengoo_terminal_consume(terminal, i + 6);
                return -SENGOO_STATUS_OVERFLOW;
            }
            memcpy(terminal->text, p, i); terminal->text_len = i;
            size_t offset = 0; unsigned scalar;
            while (offset < i) { int size = sengoo_terminal_utf8(p + offset, i - offset, &scalar);
                if (size <= 0) { terminal->paste = 0; sengoo_terminal_consume(terminal, i + 6); return -SENGOO_STATUS_INVALID_UTF8; }
                offset += (size_t)size;
            }
            terminal->paste = 0; sengoo_terminal_consume(terminal, i + 6); return SG_TERM_PASTE;
        }
        if (n > SENGOO_TERMINAL_LIMIT) {
            memmove(p, p + n - 5, 5); terminal->pending_len = 5;
            if (!terminal->discard_paste) { terminal->paste = 0; terminal->discard_paste = 1; return -SENGOO_STATUS_OVERFLOW; }
        }
        return 0;
    }
    if (p[0] == 27) {
        if (n >= 6 && memcmp(p, "\033[200~", 6) == 0) { terminal->paste = 1; sengoo_terminal_consume(terminal, 6); return sengoo_terminal_parse(terminal); }
        /* Standard BackTab must match the native Windows Shift+Tab event.
         * Incomplete prefixes keep the existing bounded escape wait below. */
        if (n >= 3 && memcmp(p, "\033[Z", 3) == 0) {
            terminal->code = 9; terminal->modifiers = 1;
            sengoo_terminal_consume(terminal, 3); return SG_TERM_KEY;
        }
        if (terminal->mouse && n >= 3 && memcmp(p, "\033[<", 3) == 0) {
            long long result = sengoo_terminal_parse_mouse(terminal);
            if (result) return result;
            if (!terminal->escape_since) terminal->escape_since = sengoo_terminal_clock();
            if (!terminal->eof && sengoo_terminal_clock() - terminal->escape_since < 25) return 0;
        }
        static const char* sequences[] = {"\033[A","\033[B","\033[C","\033[D","\033[H","\033[F","\033[3~","\033[5~","\033[6~","\033[2~"};
        for (size_t i = 0; i < sizeof(sequences)/sizeof(sequences[0]); i++) {
            size_t length = strlen(sequences[i]);
            if (n >= length && memcmp(p, sequences[i], length) == 0) {
                terminal->code = 1001 + (long long)i; sengoo_terminal_consume(terminal, length); return SG_TERM_KEY;
            }
        }
        if (!terminal->escape_since) terminal->escape_since = sengoo_terminal_clock();
        if (sengoo_terminal_clock() - terminal->escape_since < 25 && !terminal->eof) return 0;
    }
    unsigned scalar; int length = sengoo_terminal_utf8(p, n, &scalar);
    if (length == 0 && !terminal->eof) return 0;
    if (length <= 0) { sengoo_terminal_consume(terminal, 1); return -SENGOO_STATUS_INVALID_UTF8; }
    terminal->code = scalar;
    memcpy(terminal->text, p, (size_t)length); terminal->text_len = (size_t)length;
    sengoo_terminal_consume(terminal, (size_t)length);
    return scalar == 3 ? SG_TERM_CANCEL : SG_TERM_KEY;
}
#ifdef _WIN32
static long long sengoo_terminal_windows_mouse(SengooTerminal* terminal, const MOUSE_EVENT_RECORD* event) {
    if (!terminal->mouse) return 0;
    DWORD buttons = LOWORD(event->dwButtonState), changed = terminal->mouse_buttons ^ buttons;
    DWORD pressed = changed & buttons, released = changed & terminal->mouse_buttons;
    terminal->mouse_buttons = buttons;
    terminal->mouse_x = event->dwMousePosition.X; terminal->mouse_y = event->dwMousePosition.Y;
    CONSOLE_SCREEN_BUFFER_INFO info;
    if (GetConsoleScreenBufferInfo(terminal->output, &info)) {
        terminal->mouse_x -= info.srWindow.Left; terminal->mouse_y -= info.srWindow.Top;
    }
    terminal->modifiers = ((event->dwControlKeyState & SHIFT_PRESSED) ? 1 : 0)
        | ((event->dwControlKeyState & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED)) ? 2 : 0)
        | ((event->dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED)) ? 4 : 0);
    if (event->dwEventFlags & (MOUSE_WHEELED | MOUSE_HWHEELED)) {
        terminal->mouse_action = 4;
        long long delta = (SHORT)HIWORD(event->dwButtonState);
        if (event->dwEventFlags & MOUSE_WHEELED) terminal->scroll_y = delta;
        else terminal->scroll_x = delta;
        return SG_TERM_MOUSE;
    }
    DWORD selected = pressed ? pressed : released ? released : buttons;
    if (selected & FROM_LEFT_1ST_BUTTON_PRESSED) terminal->mouse_button = 1;
    else if (selected & FROM_LEFT_2ND_BUTTON_PRESSED) terminal->mouse_button = 2;
    else if (selected & RIGHTMOST_BUTTON_PRESSED) terminal->mouse_button = 3;
    else if (selected & FROM_LEFT_3RD_BUTTON_PRESSED) terminal->mouse_button = 4;
    else if (selected & FROM_LEFT_4TH_BUTTON_PRESSED) terminal->mouse_button = 5;
    if (event->dwEventFlags & MOUSE_MOVED) terminal->mouse_action = 3;
    else if (pressed || event->dwEventFlags & DOUBLE_CLICK) {
        terminal->mouse_action = 1;
        terminal->mouse_clicks = (event->dwEventFlags & DOUBLE_CLICK) ? 2 : 1;
    } else if (released) terminal->mouse_action = 2;
    else return 0;
    return SG_TERM_MOUSE;
}
#endif
static long long sengoo_terminal_read_input(SengooTerminal* terminal) {
    size_t capacity = sizeof(terminal->pending) - terminal->pending_len;
    if (capacity > 4096) capacity = 4096;
    if (!capacity) return -SENGOO_STATUS_OVERFLOW;
#ifdef _WIN32
    if (terminal->input_tty) {
        if (terminal->repeat_left) {
            if (capacity < terminal->repeat_len) return -SENGOO_STATUS_OVERFLOW;
            terminal->repeat_left--;
            terminal->modifiers = terminal->repeat_modifiers;
            if (!terminal->repeat_len) {
                terminal->code = terminal->repeat_code;
                return SG_TERM_KEY;
            }
            memcpy(terminal->pending + terminal->pending_len, terminal->repeat_bytes, terminal->repeat_len);
            terminal->pending_len += terminal->repeat_len;
            return 0;
        }
        /* KEYUP/focus/disabled mouse records are not public events. Drain a
         * bounded ready batch rather than treating each as an idle poll (and
         * delaying the next already-queued key by the caller's frame timer).
         * The cap gives the outer deadline/cancellation loop control even if
         * another process continuously supplies irrelevant input. */
        for (unsigned records = 0; records < 64; records++) {
            DWORD ready = WaitForSingleObject(terminal->input, 0);
            if (ready == WAIT_FAILED) return -SENGOO_STATUS_IO;
            if (ready != WAIT_OBJECT_0) return 0;
            INPUT_RECORD event; DWORD count;
            if (!ReadConsoleInputW(terminal->input, &event, 1, &count)) return -SENGOO_STATUS_IO;
            if (!count) continue;
            if (event.EventType == WINDOW_BUFFER_SIZE_EVENT) {
                /* The window may have changed away and back since the last
                 * poll. Deliver an invalidation even if its final size matches. */
                sengoo_terminal_dimensions(terminal, &terminal->width, &terminal->height);
                return SG_TERM_RESIZE;
            }
            if (event.EventType == MOUSE_EVENT) {
                long long kind = sengoo_terminal_windows_mouse(terminal, &event.Event.MouseEvent);
                if (kind) return kind;
                continue;
            }
            if (event.EventType != KEY_EVENT || !event.Event.KeyEvent.bKeyDown) continue;
            KEY_EVENT_RECORD key = event.Event.KeyEvent;
            terminal->modifiers = ((key.dwControlKeyState & SHIFT_PRESSED) ? 1 : 0)
                | ((key.dwControlKeyState & (LEFT_ALT_PRESSED | RIGHT_ALT_PRESSED)) ? 2 : 0)
                | ((key.dwControlKeyState & (LEFT_CTRL_PRESSED | RIGHT_CTRL_PRESSED)) ? 4 : 0);
            wchar_t character = key.uChar.UnicodeChar;
            if (!character && !terminal->paste && !terminal->pending_len) {
                WORD codes[] = { VK_UP,VK_DOWN,VK_RIGHT,VK_LEFT,VK_HOME,VK_END,VK_DELETE,VK_PRIOR,VK_NEXT,VK_INSERT };
                for (size_t i = 0; i < sizeof(codes)/sizeof(codes[0]); i++) if (key.wVirtualKeyCode == codes[i]) {
                    terminal->repeat_left = key.wRepeatCount ? key.wRepeatCount - 1 : 0;
                    terminal->repeat_code = 1001 + (long long)i;
                    terminal->repeat_modifiers = terminal->modifiers;
                    terminal->repeat_len = 0;
                    terminal->code = 1001 + (long long)i; return SG_TERM_KEY;
                }
                continue;
            }
            wchar_t wide[2]; int length = 1; wide[0] = character;
            if (character >= 0xd800 && character <= 0xdbff) { terminal->high_surrogate = character; continue; }
            if (terminal->high_surrogate) { wide[0] = terminal->high_surrogate; wide[1] = character; length = 2; terminal->high_surrogate = 0; }
            if (!character) continue;
            int n = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide, length,
                (char*)terminal->pending + terminal->pending_len, (int)capacity, NULL, NULL);
            if (!n) return -SENGOO_STATUS_INVALID_UTF8;
            terminal->repeat_left = key.wRepeatCount ? key.wRepeatCount - 1 : 0;
            terminal->repeat_modifiers = terminal->modifiers;
            terminal->repeat_len = (size_t)n;
            memcpy(terminal->repeat_bytes, terminal->pending + terminal->pending_len, (size_t)n);
            terminal->pending_len += (size_t)n; return 0;
        }
        return 0;
    }
    DWORD type = GetFileType(terminal->input), available = 0, count = 0;
    if (type == FILE_TYPE_PIPE) {
        if (!PeekNamedPipe(terminal->input, NULL, 0, NULL, &available, NULL)) {
            if (GetLastError() == ERROR_BROKEN_PIPE) { terminal->eof = 1; return 0; }
            return -SENGOO_STATUS_IO;
        }
        if (!available) return 0;
        if (capacity > available) capacity = available;
    }
    if (!ReadFile(terminal->input, terminal->pending + terminal->pending_len, (DWORD)capacity, &count, NULL)) {
        if (GetLastError() == ERROR_BROKEN_PIPE) { terminal->eof = 1; return 0; }
        return -SENGOO_STATUS_IO;
    }
    terminal->pending_len += count; if (!count) terminal->eof = 1;
#else
    struct pollfd input = { terminal->input, POLLIN, 0 };
    int ready = poll(&input, 1, 0);
    if (ready < 0) return errno == EINTR ? 0 : -SENGOO_STATUS_IO;
    if (!ready) return 0;
    ssize_t n = read(terminal->input, terminal->pending + terminal->pending_len, capacity);
    if (n < 0) return errno == EINTR || errno == EAGAIN ? 0 : -SENGOO_STATUS_IO;
    if (!n) terminal->eof = 1; else terminal->pending_len += (size_t)n;
#endif
    return 0;
}
long long sengoo_terminal_read_event(long long handle, long long timeout_ms) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    if (timeout_ms < 0 || timeout_ms > 60000) return -SENGOO_STATUS_INVALID_ARGUMENT;
    long long deadline = sengoo_terminal_clock() + timeout_ms;
    int read_once = 0;
    terminal->kind = terminal->code = terminal->modifiers = 0; terminal->text_len = 0;
    terminal->mouse_x = terminal->mouse_y = terminal->mouse_button = terminal->mouse_action = 0;
    terminal->scroll_x = terminal->scroll_y = terminal->mouse_clicks = 0;
    for (;;) {
        if (sengoo_terminal_cancelled) { sengoo_terminal_cancelled = 0; terminal->kind = SG_TERM_CANCEL; return terminal->kind; }
        long long width, height; sengoo_terminal_dimensions(terminal, &width, &height);
        if (width != terminal->width || height != terminal->height) {
            terminal->width = width; terminal->height = height; terminal->kind = SG_TERM_RESIZE; return terminal->kind;
        }
        long long kind = sengoo_terminal_parse(terminal);
        if (kind < 0) return kind;
        if (kind > 0) { terminal->kind = kind; return kind; }
        if (terminal->eof) {
            if (terminal->paste || terminal->discard_paste) { terminal->paste = terminal->discard_paste = 0; terminal->pending_len = 0; return -SENGOO_STATUS_PARSE; }
            terminal->kind = SG_TERM_EOF; return terminal->kind;
        }
        if (read_once && sengoo_terminal_clock() >= deadline) return 0;
        size_t before = terminal->pending_len;
        kind = sengoo_terminal_read_input(terminal);
        read_once = 1;
        if (kind < 0) return kind;
        if (kind > 0) { terminal->kind = kind; return kind; }
        if (terminal->pending_len != before || terminal->eof) continue;
        if (sengoo_terminal_clock() >= deadline) return 0;
#ifdef _WIN32
        Sleep(1);
#else
        struct timespec delay = {0, 1000000}; nanosleep(&delay, NULL);
#endif
    }
}
long long sengoo_terminal_event_field(long long handle, long long field) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    switch (field) {
        case 0: return terminal->kind;
        case 1: return terminal->code;
        case 2: return terminal->modifiers;
        case 3: return terminal->width;
        case 4: return terminal->height;
        case 5: return terminal->mouse_x;
        case 6: return terminal->mouse_y;
        case 7: return terminal->mouse_button;
        case 8: return terminal->mouse_action;
        case 9: return terminal->scroll_x;
        case 10: return terminal->scroll_y;
        case 11: return terminal->mouse_clicks;
        default: return -SENGOO_STATUS_INVALID_ARGUMENT;
    }
}
long long sengoo_terminal_event_text(long long handle) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return -SENGOO_STATUS_INVALID_HANDLE;
    long long value = sengoo_string_from_bytes_copy((long long)(intptr_t)terminal->text, (long long)terminal->text_len);
    return value > 0 ? value : -SENGOO_STATUS_OUT_OF_MEMORY;
}
long long sengoo_terminal_close(long long handle) {
    SengooTerminal* terminal = sengoo_terminal_resolve(handle);
    if (!terminal) return handle == 0 ? 0 : -SENGOO_STATUS_INVALID_HANDLE;
    int ok = sengoo_terminal_restore_state(terminal);
    SengooTerminal** entry = &sengoo_terminal_handles;
    while (*entry && *entry != terminal) entry = &(*entry)->next;
    if (*entry) *entry = terminal->next;
    sengoo_opaque_handle_take(handle);
#ifdef _WIN32
    CloseHandle(terminal->input); CloseHandle(terminal->output);
#else
    close(terminal->input); close(terminal->output);
#endif
    terminal->magic = 0; free(terminal); sengoo_terminal_live_count--;
    return ok ? 0 : -SENGOO_STATUS_IO;
}
long long sengoo_terminal_live_handle_count(void) { return sengoo_terminal_live_count; }

/* Clipboard transport is independent of terminal/render state. Resolve the
 * Windows UI system library at runtime so ordinary console builds retain their
 * existing link dependency contract. No window is made visible. */
long long sengoo_terminal_copy_text(long long bytes, long long length) {
    if (length < 0 || length > SENGOO_TERMINAL_LIMIT || (length && !bytes)) return -SENGOO_STATUS_INVALID_ARGUMENT;
#ifdef _WIN32
    const char* text = (const char*)(intptr_t)bytes;
    if (length && memchr(text, 0, (size_t)length)) return -SENGOO_STATUS_INVALID_ARGUMENT;
    int count = length ? MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, (int)length, NULL, 0) : 0;
    if (length && !count) return -SENGOO_STATUS_INVALID_UTF8;
    HGLOBAL memory = GlobalAlloc(GMEM_MOVEABLE, ((size_t)count + 1) * sizeof(wchar_t));
    if (!memory) return -SENGOO_STATUS_OUT_OF_MEMORY;
    wchar_t* wide = (wchar_t*)GlobalLock(memory);
    if (!wide) { GlobalFree(memory); return -SENGOO_STATUS_OUT_OF_MEMORY; }
    if (count && !MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, text, (int)length, wide, count)) {
        GlobalUnlock(memory); GlobalFree(memory); return -SENGOO_STATUS_INVALID_UTF8;
    }
    wide[count] = 0; GlobalUnlock(memory);
    HMODULE library = LoadLibraryExW(L"user32.dll", NULL, LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!library) { GlobalFree(memory); return -SENGOO_STATUS_UNSUPPORTED; }
    typedef BOOL (WINAPI *OpenFn)(HWND);
    typedef BOOL (WINAPI *CloseFn)(void);
    typedef BOOL (WINAPI *EmptyFn)(void);
    typedef HANDLE (WINAPI *SetFn)(UINT, HANDLE);
    typedef HWND (WINAPI *WindowFn)(DWORD, LPCWSTR, LPCWSTR, DWORD, int, int, int, int, HWND, HMENU, HINSTANCE, LPVOID);
    typedef BOOL (WINAPI *DestroyFn)(HWND);
    OpenFn open = (OpenFn)GetProcAddress(library, "OpenClipboard");
    CloseFn close = (CloseFn)GetProcAddress(library, "CloseClipboard");
    EmptyFn empty = (EmptyFn)GetProcAddress(library, "EmptyClipboard");
    SetFn set = (SetFn)GetProcAddress(library, "SetClipboardData");
    WindowFn window = (WindowFn)GetProcAddress(library, "CreateWindowExW");
    DestroyFn destroy = (DestroyFn)GetProcAddress(library, "DestroyWindow");
    long long status = -SENGOO_STATUS_UNSUPPORTED;
    if (open && close && empty && set && window && destroy) {
        HWND owner = window(0, L"STATIC", L"Sengoo clipboard owner", 0, 0, 0, 0, 0, HWND_MESSAGE, NULL, GetModuleHandleW(NULL), NULL);
        status = -SENGOO_STATUS_IO;
        if (owner) {
            if (open(owner)) {
                if (empty() && set(CF_UNICODETEXT, memory)) { memory = NULL; status = 1; }
                if (!close()) status = -SENGOO_STATUS_IO;
            }
            destroy(owner);
        }
    }
    if (memory) GlobalFree(memory);
    FreeLibrary(library); return status;
#else
    (void)bytes; return -SENGOO_STATUS_UNSUPPORTED;
#endif
}

/* Read-only clipboard snapshots. All DLLs come from System32 and are released
 * per call; no extra link dependency or application clipboard state is added. */
#ifdef _WIN32
#define SG_CLIP_MAX_BYTES (64u * 1024u * 1024u)
#define SG_CLIP_MAX_PIXELS (16u * 1024u * 1024u)
typedef struct SgClipboard {
    HMODULE library;
    BOOL (WINAPI *open)(HWND);
    BOOL (WINAPI *close)(void);
    BOOL (WINAPI *available)(UINT);
    HANDLE (WINAPI *get)(UINT);
    UINT (WINAPI *format)(LPCWSTR);
    int opened;
} SgClipboard;
static long long sg_clip_load(SgClipboard* clip, int open) {
    memset(clip, 0, sizeof(*clip));
    clip->library = LoadLibraryExW(L"user32.dll", NULL, LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!clip->library) return -SENGOO_STATUS_UNSUPPORTED;
    clip->open = (void*)GetProcAddress(clip->library, "OpenClipboard");
    clip->close = (void*)GetProcAddress(clip->library, "CloseClipboard");
    clip->available = (void*)GetProcAddress(clip->library, "IsClipboardFormatAvailable");
    clip->get = (void*)GetProcAddress(clip->library, "GetClipboardData");
    clip->format = (void*)GetProcAddress(clip->library, "RegisterClipboardFormatW");
    if (!clip->open || !clip->close || !clip->available || !clip->get || !clip->format) return -SENGOO_STATUS_UNSUPPORTED;
    if (open) { if (!clip->open(NULL)) return -SENGOO_STATUS_IO; clip->opened = 1; }
    return 1;
}
static void sg_clip_close(SgClipboard* clip) {
    if (clip->opened) clip->close();
    if (clip->library) FreeLibrary(clip->library);
    memset(clip, 0, sizeof(*clip));
}
static UINT sg_clip_image_format(SgClipboard* clip) {
    UINT png = clip->format(L"PNG");
    if (png && clip->available(png)) return png;
    if (clip->available(CF_DIBV5)) return CF_DIBV5;
    if (clip->available(CF_DIB)) return CF_DIB;
    if (clip->available(CF_BITMAP)) return CF_BITMAP;
    return 0;
}
static wchar_t* sg_clip_path(long long pointer, long long length, long long* error) {
    const char* path = (const char*)(intptr_t)pointer;
    if (!pointer || length < 1 || length > SENGOO_RUNTIME_MAX_PATH_BYTES || memchr(path, 0, (size_t)length)) { *error = -SENGOO_STATUS_INVALID_ARGUMENT; return NULL; }
    int count = MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, (int)length, NULL, 0);
    if (!count) { *error = -SENGOO_STATUS_INVALID_UTF8; return NULL; }
    wchar_t* wide = malloc(((size_t)count + 1) * sizeof(wchar_t));
    if (!wide) { *error = -SENGOO_STATUS_OUT_OF_MEMORY; return NULL; }
    if (!MultiByteToWideChar(CP_UTF8, MB_ERR_INVALID_CHARS, path, (int)length, wide, count)) { free(wide); *error = -SENGOO_STATUS_INVALID_UTF8; return NULL; }
    wide[count] = 0; return wide;
}
static long long sg_clip_utf8(const wchar_t* wide, size_t count, size_t maximum) {
    if (count > INT_MAX) return -SENGOO_STATUS_OVERFLOW;
    int bytes = count ? WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide, (int)count, NULL, 0, NULL, NULL) : 0;
    if (count && !bytes) return -SENGOO_STATUS_INVALID_UTF8;
    if ((size_t)bytes > maximum) return -SENGOO_STATUS_OVERFLOW;
    char* output = malloc((size_t)bytes + 1); if (!output) return -SENGOO_STATUS_OUT_OF_MEMORY;
    if (bytes && !WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide, (int)count, output, bytes, NULL, NULL)) { free(output); return -SENGOO_STATUS_INVALID_UTF8; }
    long long result = sengoo_string_from_bytes_copy((long long)(intptr_t)output, bytes); free(output); return result ? result : -SENGOO_STATUS_OUT_OF_MEMORY;
}
#endif

long long sengoo_terminal_clipboard_has_image(void) {
#ifdef _WIN32
    SgClipboard clip; long long result = sg_clip_load(&clip, 0);
    if (result > 0) result = sg_clip_image_format(&clip) ? 1 : 0;
    sg_clip_close(&clip); return result;
#else
    return -SENGOO_STATUS_UNSUPPORTED;
#endif
}

long long sengoo_terminal_clipboard_text(void) {
#ifdef _WIN32
    SgClipboard clip; long long result = sg_clip_load(&clip, 1);
    if (result < 0) { sg_clip_close(&clip); return result; }
    if (!clip.available(CF_UNICODETEXT)) { sg_clip_close(&clip); return sengoo_string_from_bytes_copy(0, 0); }
    HGLOBAL memory = clip.get(CF_UNICODETEXT); SIZE_T size = memory ? GlobalSize(memory) : 0;
    if (!size || size > 2 * (SENGOO_TERMINAL_LIMIT + 1)) { sg_clip_close(&clip); return -SENGOO_STATUS_OVERFLOW; }
    const wchar_t* text = GlobalLock(memory);
    if (!text) { sg_clip_close(&clip); return -SENGOO_STATUS_IO; }
    size_t count = 0; while (count < size / sizeof(wchar_t) && text[count]) count++;
    result = count == size / sizeof(wchar_t) ? -SENGOO_STATUS_PARSE : sg_clip_utf8(text, count, SENGOO_TERMINAL_LIMIT);
    GlobalUnlock(memory); sg_clip_close(&clip); return result;
#else
    return -SENGOO_STATUS_UNSUPPORTED;
#endif
}

long long sengoo_terminal_clipboard_files(void) {
#ifdef _WIN32
    SgClipboard clip; long long result = sg_clip_load(&clip, 1); HMODULE shell = NULL; char* json = NULL;
    if (result < 0) goto done;
    if (!clip.available(CF_HDROP)) { result = sengoo_string_from_bytes_copy((long long)(intptr_t)"[]", 2); goto done; }
    shell = LoadLibraryExW(L"shell32.dll", NULL, LOAD_LIBRARY_SEARCH_SYSTEM32);
    typedef UINT (WINAPI *QueryFn)(HANDLE, UINT, LPWSTR, UINT);
    QueryFn query = shell ? (void*)GetProcAddress(shell, "DragQueryFileW") : NULL;
    if (!query) { result = -SENGOO_STATUS_UNSUPPORTED; goto done; }
    HANDLE drop = clip.get(CF_HDROP); UINT count = drop ? query(drop, UINT_MAX, NULL, 0) : 0;
    if (!drop || count > 32) { result = -SENGOO_STATUS_OVERFLOW; goto done; }
    json = malloc(1024 * 1024 + 1); if (!json) { result = -SENGOO_STATUS_OUT_OF_MEMORY; goto done; }
    size_t used = 0; json[used++] = '[';
    for (UINT index = 0; index < count; index++) {
        UINT length = query(drop, index, NULL, 0);
        if (!length || length > SENGOO_RUNTIME_MAX_PATH_BYTES) { result = -SENGOO_STATUS_OVERFLOW; goto done; }
        wchar_t* wide = malloc(((size_t)length + 1) * sizeof(wchar_t)); if (!wide) { result = -SENGOO_STATUS_OUT_OF_MEMORY; goto done; }
        UINT copied = query(drop, index, wide, length + 1);
        if (copied != length || wcslen(wide) != length) { free(wide); result = -SENGOO_STATUS_PARSE; goto done; }
        int bytes = WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide, length, NULL, 0, NULL, NULL);
        if (!bytes || bytes > SENGOO_RUNTIME_MAX_PATH_BYTES) { free(wide); result = -SENGOO_STATUS_INVALID_UTF8; goto done; }
        char* text = malloc((size_t)bytes); if (!text) { free(wide); result = -SENGOO_STATUS_OUT_OF_MEMORY; goto done; }
        if (!WideCharToMultiByte(CP_UTF8, WC_ERR_INVALID_CHARS, wide, length, text, bytes, NULL, NULL)) { free(text); free(wide); result = -SENGOO_STATUS_INVALID_UTF8; goto done; }
        free(wide);
        if (used + (size_t)bytes * 6 + 4 > 1024 * 1024) { free(text); result = -SENGOO_STATUS_OVERFLOW; goto done; }
        if (index) json[used++] = ','; json[used++] = '"';
        for (int at = 0; at < bytes; at++) {
            unsigned char ch = (unsigned char)text[at];
            if (ch == '"' || ch == '\\') { json[used++] = '\\'; json[used++] = ch; }
            else if (ch < 32) { static const char hex[] = "0123456789abcdef"; json[used++] = '\\'; json[used++] = 'u'; json[used++] = '0'; json[used++] = '0'; json[used++] = hex[ch >> 4]; json[used++] = hex[ch & 15]; }
            else json[used++] = ch;
        }
        free(text); json[used++] = '"';
    }
    json[used++] = ']'; result = sengoo_string_from_bytes_copy((long long)(intptr_t)json, (long long)used);
done:
    free(json); if (shell) FreeLibrary(shell); sg_clip_close(&clip); return result;
#else
    return -SENGOO_STATUS_UNSUPPORTED;
#endif
}

#ifdef _WIN32
typedef struct SgGdiStartup { UINT32 version; void* debug; BOOL suppress_thread, suppress_codecs; } SgGdiStartup;
typedef struct SgClipGdi {
    HMODULE gdi, ole;
    ULONG_PTR token;
    int (WINAPI *start)(ULONG_PTR*, const SgGdiStartup*, void*);
    void (WINAPI *stop)(ULONG_PTR);
    int (WINAPI *from_stream)(IStream*, void**);
    int (WINAPI *from_dib)(const BITMAPINFO*, void*, void**);
    int (WINAPI *from_bitmap)(HBITMAP, HPALETTE, void**);
    int (WINAPI *width)(void*, UINT*);
    int (WINAPI *height)(void*, UINT*);
    int (WINAPI *save)(void*, IStream*, const CLSID*, const void*);
    int (WINAPI *dispose)(void*);
    HRESULT (WINAPI *stream)(HGLOBAL, BOOL, IStream**);
} SgClipGdi;
static int sg_clip_dimensions(UINT width, UINT height) { return width && height && width <= 8192 && height <= 8192 && (uint64_t)width * height <= SG_CLIP_MAX_PIXELS; }
static long long sg_clip_gdi_load(SgClipGdi* gdi) {
    memset(gdi, 0, sizeof(*gdi));
    gdi->gdi = LoadLibraryExW(L"gdiplus.dll", NULL, LOAD_LIBRARY_SEARCH_SYSTEM32);
    gdi->ole = LoadLibraryExW(L"ole32.dll", NULL, LOAD_LIBRARY_SEARCH_SYSTEM32);
    if (!gdi->gdi || !gdi->ole) return -SENGOO_STATUS_UNSUPPORTED;
#define SG_CLIP_PROC(field, name) gdi->field = (void*)GetProcAddress(gdi->gdi, name); if (!gdi->field) return -SENGOO_STATUS_UNSUPPORTED
    SG_CLIP_PROC(start, "GdiplusStartup"); SG_CLIP_PROC(stop, "GdiplusShutdown");
    SG_CLIP_PROC(from_stream, "GdipCreateBitmapFromStream"); SG_CLIP_PROC(from_dib, "GdipCreateBitmapFromGdiDib"); SG_CLIP_PROC(from_bitmap, "GdipCreateBitmapFromHBITMAP");
    SG_CLIP_PROC(width, "GdipGetImageWidth"); SG_CLIP_PROC(height, "GdipGetImageHeight"); SG_CLIP_PROC(save, "GdipSaveImageToStream"); SG_CLIP_PROC(dispose, "GdipDisposeImage");
#undef SG_CLIP_PROC
    gdi->stream = (void*)GetProcAddress(gdi->ole, "CreateStreamOnHGlobal"); if (!gdi->stream) return -SENGOO_STATUS_UNSUPPORTED;
    SgGdiStartup input = {1, NULL, FALSE, TRUE};
    return gdi->start(&gdi->token, &input, NULL) ? -SENGOO_STATUS_IO : 1;
}
static void sg_clip_gdi_close(SgClipGdi* gdi) {
    if (gdi->token && gdi->stop) gdi->stop(gdi->token);
    if (gdi->ole) FreeLibrary(gdi->ole); if (gdi->gdi) FreeLibrary(gdi->gdi);
}
#endif

long long sengoo_terminal_clipboard_image_save(long long path, long long length, long long maximum) {
#ifdef _WIN32
    if (maximum < 1 || maximum > SG_CLIP_MAX_BYTES) return -SENGOO_STATUS_INVALID_ARGUMENT;
    long long result = 0; wchar_t* destination = sg_clip_path(path, length, &result); if (!destination) return result;
    SgClipboard clip; SgClipGdi gdi; memset(&gdi, 0, sizeof(gdi));
    void* image = NULL; IStream* input = NULL; IStream* output = NULL; HGLOBAL copied = NULL; int copied_locked = 0; HANDLE file = INVALID_HANDLE_VALUE;
    result = sg_clip_load(&clip, 1); if (result < 0) goto done;
    UINT format = sg_clip_image_format(&clip); if (!format) { result = 0; goto done; }
    HANDLE source = clip.get(format); if (!source) { result = -SENGOO_STATUS_IO; goto done; }
    result = sg_clip_gdi_load(&gdi); if (result < 0) goto done;
    if (format == CF_BITMAP) {
        HMODULE library = LoadLibraryExW(L"gdi32.dll", NULL, LOAD_LIBRARY_SEARCH_SYSTEM32);
        typedef int (WINAPI *ObjectFn)(HANDLE, int, LPVOID); ObjectFn object = library ? (void*)GetProcAddress(library, "GetObjectW") : NULL;
        BITMAP bitmap; memset(&bitmap, 0, sizeof(bitmap));
        int valid = object && object(source, sizeof(bitmap), &bitmap) == sizeof(bitmap) && sg_clip_dimensions(bitmap.bmWidth, bitmap.bmHeight);
        if (library) FreeLibrary(library);
        if (!valid) { result = -SENGOO_STATUS_OVERFLOW; goto done; }
        if (gdi.from_bitmap(source, NULL, &image)) { result = -SENGOO_STATUS_IO; goto done; }
    } else {
        SIZE_T size = GlobalSize(source);
        if (!size || size > SG_CLIP_MAX_BYTES) { result = -SENGOO_STATUS_OVERFLOW; goto done; }
        void* bytes = GlobalLock(source); if (!bytes) { result = -SENGOO_STATUS_IO; goto done; }
        copied = GlobalAlloc(GMEM_MOVEABLE, size); void* owned = copied ? GlobalLock(copied) : NULL;
        if (owned) memcpy(owned, bytes, size); GlobalUnlock(source);
        if (!owned) { result = -SENGOO_STATUS_OUT_OF_MEMORY; goto done; }
        if (format == CF_DIB || format == CF_DIBV5) {
            BITMAPINFOHEADER* header = owned; size_t offset = 0;
            if (size < sizeof(*header) || (header->biSize != 40 && header->biSize != 108 && header->biSize != 124) || header->biSize > size || header->biPlanes != 1 || header->biWidth <= 0 || !header->biHeight || header->biHeight == LONG_MIN) { GlobalUnlock(copied); result = -SENGOO_STATUS_PARSE; goto done; }
            UINT height = header->biHeight < 0 ? (UINT)-header->biHeight : (UINT)header->biHeight;
            if (!sg_clip_dimensions(header->biWidth, height)) { GlobalUnlock(copied); result = -SENGOO_STATUS_OVERFLOW; goto done; }
            UINT bits = header->biBitCount;
            if ((bits != 1 && bits != 4 && bits != 8 && bits != 16 && bits != 24 && bits != 32) || (header->biCompression != BI_RGB && header->biCompression != BI_BITFIELDS)) { GlobalUnlock(copied); result = -SENGOO_STATUS_UNSUPPORTED; goto done; }
            uint64_t colors = header->biClrUsed ? header->biClrUsed : (bits <= 8 ? 1u << bits : 0);
            offset = header->biSize + (header->biSize == 40 && header->biCompression == BI_BITFIELDS ? 12 : 0);
            uint64_t row = (((uint64_t)header->biWidth * bits + 31) / 32) * 4;
            uint64_t end = offset + colors * sizeof(RGBQUAD) + row * height;
            if (colors > 256 || end > size) { GlobalUnlock(copied); result = -SENGOO_STATUS_PARSE; goto done; }
            offset += (size_t)colors * sizeof(RGBQUAD);
            if (gdi.from_dib(owned, (unsigned char*)owned + offset, &image)) { GlobalUnlock(copied); result = -SENGOO_STATUS_IO; goto done; }
            copied_locked = 1;
        } else {
            const unsigned char* png = owned;
            if (size < 24 || memcmp(png, "\x89PNG\r\n\x1a\n", 8) || memcmp(png + 12, "IHDR", 4)) { GlobalUnlock(copied); result = -SENGOO_STATUS_PARSE; goto done; }
            UINT png_width = (UINT)png[16] << 24 | (UINT)png[17] << 16 | (UINT)png[18] << 8 | png[19];
            UINT png_height = (UINT)png[20] << 24 | (UINT)png[21] << 16 | (UINT)png[22] << 8 | png[23];
            if (!sg_clip_dimensions(png_width, png_height)) { GlobalUnlock(copied); result = -SENGOO_STATUS_OVERFLOW; goto done; }
            GlobalUnlock(copied);
            if (FAILED(gdi.stream(copied, TRUE, &input))) { result = -SENGOO_STATUS_IO; goto done; }
            copied = NULL;
            if (gdi.from_stream(input, &image)) { result = -SENGOO_STATUS_PARSE; goto done; }
        }
    }
    /* HBITMAP remains clipboard-owned for the lifetime of the GDI+ image.
     * DIB/PNG use owned copies; those formats can release the clipboard now. */
    if (format != CF_BITMAP) sg_clip_close(&clip);
    UINT width = 0, height = 0;
    if (gdi.width(image, &width) || gdi.height(image, &height) || !sg_clip_dimensions(width, height)) { result = -SENGOO_STATUS_OVERFLOW; goto done; }
    if (FAILED(gdi.stream(NULL, TRUE, &output))) { result = -SENGOO_STATUS_IO; goto done; }
    const CLSID png = {0x557cf406, 0x1a04, 0x11d3, {0x9a, 0x73, 0x00, 0x00, 0xf8, 0x1e, 0xf3, 0x2e}};
    if (gdi.save(image, output, &png, NULL)) { result = -SENGOO_STATUS_IO; goto done; }
    STATSTG stat; memset(&stat, 0, sizeof(stat));
    if (FAILED(output->lpVtbl->Stat(output, &stat, STATFLAG_NONAME))) { result = -SENGOO_STATUS_IO; goto done; }
    if (!stat.cbSize.QuadPart || stat.cbSize.QuadPart > (ULONGLONG)maximum) { result = -SENGOO_STATUS_OVERFLOW; goto done; }
    LARGE_INTEGER zero; zero.QuadPart = 0;
    if (FAILED(output->lpVtbl->Seek(output, zero, STREAM_SEEK_SET, NULL))) { result = -SENGOO_STATUS_IO; goto done; }
    file = CreateFileW(destination, GENERIC_WRITE, 0, NULL, CREATE_NEW, FILE_ATTRIBUTE_NORMAL, NULL);
    if (file == INVALID_HANDLE_VALUE) { DWORD error = GetLastError(); result = error == ERROR_FILE_EXISTS || error == ERROR_ALREADY_EXISTS ? -SENGOO_STATUS_ALREADY_EXISTS : -SENGOO_STATUS_IO; goto done; }
    ULONGLONG left = stat.cbSize.QuadPart; unsigned char buffer[16384]; result = 1;
    while (left) {
        ULONG wanted = left > sizeof(buffer) ? sizeof(buffer) : (ULONG)left, read = 0; DWORD written = 0;
        if (FAILED(output->lpVtbl->Read(output, buffer, wanted, &read)) || read != wanted || !WriteFile(file, buffer, read, &written, NULL) || written != read) { result = -SENGOO_STATUS_IO; break; }
        left -= read;
    }
    if (result > 0 && !FlushFileBuffers(file)) result = -SENGOO_STATUS_IO;
done:
    if (file != INVALID_HANDLE_VALUE) { if (!CloseHandle(file)) result = -SENGOO_STATUS_IO; if (result < 0) DeleteFileW(destination); }
    if (output) output->lpVtbl->Release(output);
    if (image && gdi.dispose) gdi.dispose(image);
    if (input) input->lpVtbl->Release(input);
    if (copied_locked) GlobalUnlock(copied);
    if (copied) GlobalFree(copied);
    sg_clip_gdi_close(&gdi); sg_clip_close(&clip); free(destination); return result;
#else
    (void)path; (void)length; (void)maximum; return -SENGOO_STATUS_UNSUPPORTED;
#endif
}
