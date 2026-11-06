# Native process and terminal foundation

These additive development APIs preserve existing `std::process` functions,
edition 2026 and runtime ABI 1. Windows x64 is the first verified host. POSIX
implementations are present; a Windows run does not qualify POSIX behavior.

## Piped processes

`ProcessCommand.spawn_piped()` borrows a command and returns an owning,
non-Send/non-Sync `ProcessHandle`. Unlike legacy `spawn`, it leaves the builder
usable. It passes argv directly, with no shell interpretation. An application
that wants a shell must select one explicitly.

| Method | Result |
| --- | --- |
| `try_wait()` | `Result<bool,i64>`: false running, true exited and reaped |
| `read_stdout(out: &mut Buffer)` / `read_stderr(out: &mut Buffer)` | Nonblocking byte count; updates `out.used_len()` |
| `stdout_eof()` / `stderr_eof()` | Distinguishes EOF from a read with no bytes ready |
| `write_stdin(data: &Buffer, used_len)` | Nonblocking partial byte count; zero is backpressure |
| `try_write_stdin(data: &Buffer, used_len)` | Explicit alias for the same nonblocking partial-write contract |
| `close_stdin()` | Idempotently sends EOF to the child |
| `terminate_tree()` | Stops the owned process tree; call `reap` afterward |
| `reap(timeout_ms)` | Exit code; timeout leaves the child alive and reachable |

Each I/O call transfers at most 64 KiB, also bounded by the caller's buffer.
The runtime does not collect an unbounded output string. Drain stdout and stderr
alternately, even when sending stdin. After a partial write, submit only the
unwritten suffix. A zero-byte read is not EOF until the corresponding EOF method
returns true. Drain remaining pipe data after `try_wait` reports completion.
`reap` and legacy `wait` do not drain pipes on the caller's behalf.
Windows `spawn_piped` uses a byte-mode named pipe with `PIPE_NOWAIT`; its
synchronous `WriteFile` does not wait for a full pipe to drain. POSIX uses
`O_NONBLOCK`. This contract belongs to the owning piped API; legacy captured
`run`/pipeline helpers retain their separate synchronous behavior. A caller's
loop must still enforce deadlines, process cancellation and drain both output
streams rather than busy-waiting on zero-byte writes.

The application owns total-output, elapsed-time and cancellation policy. The
configured builder timeout remains a legacy `run()` option; it does not silently
install a timer on `spawn_piped()`. `wait(0)` and `wait_cancellable(0)` now return
timeout without falsely marking a running child completed. Use `try_wait` for
polling rather than interpreting a timeout as an exit code.

Drop terminates the owned tree, waits for the direct child and closes the pipes.
On Windows, `CreateProcessW` preserves UTF-8 argv/cwd/environment through UTF-16,
an explicit inherited-handle list prevents unrelated inheritable descriptors
leaking into the child, and the suspended child joins a kill-on-close Job Object
before it runs. A failure to assign the job fails spawning. POSIX uses a new
process group, close-on-exec pipes and `waitpid`; descendants that deliberately
escape the process group are outside that guarantee. The OS may retain dead
orphan zombies until init reaps them. No bounded completion time is promised for
POSIX processes stuck in an uninterruptible kernel operation.

`sengoo_process_live_handle_count()` observes open child handles for tests;
`sengoo_process_live_resource_count()` also includes commands and captured
outputs. These domains validate their generational tokens before accessing
storage, and close frees their complete allocations. Metadata slots are reused
as owners close; they do not grow with the number of completed tool calls.
Repeated close of a recently closed handle succeeds until its metadata slot
is reused, after which that stale identity is rejected.

## Terminal events

`import std::terminal;` supplies `terminal_open() -> Result<Terminal,i64>`.
The non-Send/non-Sync terminal owns duplicated input/output handles; it does not
close the application's original stdin/stdout. `is_tty` reports interactive
input and output together. `enter_raw` is explicit and returns unsupported when
either is redirected. There can be only one raw-mode owner in a process.

`read_event(timeout_ms)` accepts 0 through 60,000 milliseconds. It returns an
owning `TerminalEvent` with `kind`, `code`, `modifiers`, `width`, `height` and
`data: String`. Events own their text independently of later reads.

Windows console polls skip up to 64 ready records that produce no public event
(for example KEYUP, focus, or mouse records with tracking disabled) in each
native input attempt. A short queued Unicode key therefore does not wait for a
separate application frame per ignored record. A zero-time poll yields when
that batch is exhausted, so kind 0 can also mean bounded work yielded behind a
long ignored queue. Positive timeouts recheck their deadline and cancellation
between batches. An unmatched high UTF-16 surrogate survives a timeout until
its low surrogate arrives; callers still own the polling cadence.

| kind | Meaning |
| --- | --- |
| 0 | No event before timeout |
| 1 | Key: Unicode scalar in code, UTF-8 text in data |
| 2 | Bracketed paste: complete UTF-8 text in data |
| 3 | Resize: new width/height |
| 4 | Cancellation request: Ctrl+C, and handled process console signals |
| 5 | EOF |
| 6 | Mouse: snapshot with `terminal.mouse()` before the next read |

Key codes 1001–1010 are up, down, right, left, home, end, delete, page-up,
page-down and insert. Windows key modifiers use bits 1 Shift, 2 Alt, 4 Control.
The POSIX byte parser handles the documented basic ANSI sequences; it does not
claim every terminal's extended keyboard protocol. Paste recognition uses
bracketed-paste delimiters and a 64-KiB cap; oversize paste returns overflow,
then drains through the closing marker before delivering subsequent keys.
Malformed UTF-8 and truncated paste return explicit errors. Redirected input
uses the same bounded parser and EOF behavior without modifying terminal modes.

The standard BackTab byte sequence `ESC [ Z` is normalized to key code 9 with
Shift modifier bit 1, matching native Windows Shift+Tab. Ordinary Tab remains
code 9 without modifiers. Complete or fragmented BackTab bytes use the existing
25 ms escape-disambiguation window; bracketed-paste contents remain literal.
The dedicated `terminal_backtab` native/public regression verifies this common
decoder and owned Windows-console parity. It does not qualify a live POSIX
terminal host or other extended keyboard sequences.

`discard_input() -> Result<bool,i64>` explicitly drops queued interactive input
at an application boundary such as a newly displayed approval prompt. It flushes
the OS input queue and clears partial UTF-8/UTF-16, escape and paste state plus
the last key/text fields. It preserves terminal handles, modes, dimensions and
pending console/signal cancellation; a later `read_event` still reports that
cancellation. A successful call returns true. Invalid handles fail and redirected
input returns unsupported without consuming any bytes. Input typed after the
flush remains available; applications must still validate fresh approval input.

`clear_after_cursor() -> Result<bool,i64>` clears from the cursor inclusively
through the writable tail of the active terminal. On Windows it fills the
remaining console screen buffer with spaces and the current output attributes,
including rows below the viewport; it does not read console text, move the
cursor, change the viewport, or erase the prefix before the cursor. On POSIX it
emits the standard erase-to-end-of-screen sequence, following the host terminal's
screen semantics. Invalid handles fail; redirected input/output is unsupported
and is left untouched. Applications must first position the cursor at the start
of a region they own, such as an active composer after resize. This is not a
transcript history or arbitrary screen inspection API.

`write(text)` writes at most 64 KiB of UTF-8. It uses `WriteConsoleW` on a Windows
console and ordinary bytes on redirected output. The bridge contains no line
editor, command parser, transcript renderer or prompt business logic.

`enter_fullscreen()` enters the alternate screen, hides the cursor and enables
raw and mouse events. `leave_fullscreen()` restores the preceding screen and
mouse preference; it retains raw mode when the caller entered raw beforehand.
Both operations are idempotent. Explicit restore, Drop and normal exit also
leave the alternate screen. Redirected input/output is unsupported. Fullscreen
does not supply a renderer, input editor, transcript or scrolling policy.

`set_mouse(enabled)` explicitly changes tracking while raw mode is active.
`mouse()` returns a `TerminalMouseEvent` with zero-based viewport `column/row`,
`button` (0 none, 1 left, 2 middle, 3 right, 4/5 extra), `action` (1 press,
2 release, 3 move, 4 wheel), `scroll_x/scroll_y`, `clicks` and `modifiers`.
Wheel deltas use 120 units per detent, positive x right and positive y up.
Modifiers use bits 1 Shift, 2 Alt, 4 Control. Snapshot this value before the next
`read_event`; mouse events have empty text and do not change key codes.
Windows uses native console mouse records; the POSIX parser accepts bounded
SGR mouse sequences. A Windows run does not qualify POSIX terminal operation.

`terminal_copy_text(text: &String)` writes at most 64KiB of owned text to the
Windows CF_UNICODETEXT clipboard using strict UTF-8 conversion. Owned byte length
prevents embedded NUL from silently truncating the content; NUL is rejected
before the clipboard is opened. The generic function uses a temporary invisible
clipboard owner and contains no selection/render policy. It returns unsupported
on other platforms. Ordinary terminal link dependencies remain unchanged.

The following additive, experimental Windows paste APIs are independent of
terminal raw/fullscreen mode. Invoke them only for an explicit user paste;
they never clear or replace clipboard contents.

| Function | Result |
| --- | --- |
| `terminal_clipboard_has_image()` | `Result<bool,i64>`: PNG, DIBV5, DIB or bitmap availability |
| `terminal_clipboard_text()` | `Result<String,i64>`: owned UTF-8, empty when absent; maximum 64KiB |
| `terminal_clipboard_files()` | `Result<String,i64>`: one-snapshot JSON array of original CF_HDROP paths; `[]` when absent |
| `terminal_clipboard_image_save(path: &String, maximum: i64)` | `Result<bool,i64>`: true creates a new PNG, false means no image and no output file |

Dropped-file snapshots contain at most 32 paths, each at most 32KiB UTF-8,
and at most 1MiB JSON. The API does not copy those files into the project.
The application decides which original paths to reference. For a raw clipboard
bitmap with no original path, it may explicitly request a new file in an owned
temporary cache; cache naming and cleanup remain application responsibilities.

Image input and output are each bounded to 64MiB; `maximum` selects a smaller
PNG output limit. Neither image side may exceed 8192 pixels and total pixel
count is at most 16Mi. Dimensions are checked before PNG/DIB decoding. PNG
encoding succeeds within the output limit before exclusive file creation;
an existing destination returns status 6 and is preserved. Failed writes remove
only their newly created output. Strict UTF-8/NUL path checks precede clipboard
access. Overflow returns 13, malformed content 10, invalid UTF-8 20 and ordinary
I/O failure 9; non-Windows returns unsupported (8). DIB/PNG conversion owns its
source bytes. Clipboard-owned bitmap handles remain protected through GDI+
image disposal. DLL imports come only from System32 and require no extra
application linker options. This is a synchronous bounded OS boundary, not a
background image decoder.

`restore()` and Drop reset bracketed paste, cursor visibility, input/output
modes and Windows console codepages. Normal process exit also restores the raw
owner. The caller must react to cancellation and unwind; hard process kill,
abort or machine failure cannot be promised to run restoration code.

## Executable evidence

`cargo test --locked -p sgc --test process_terminal` runs a standalone C probe
with real children and a public Sengoo API/Drop integration. The probe covers
dual-stream flood, stdin backpressure, timeouts, Unicode argv/executable/cwd/environment paths,
descendant cleanup, stale handles, parser limits and redirected input. Windows
uses an isolated hidden console for real raw/key/cancel/resize/restoration
checks, including stale key-up/mouse/paste queues, discard and fresh input;
POSIX uses an isolated pseudo-terminal. The discard regression also verifies
unchanged modes, retained cancellation and invalid/non-TTY handle errors.

`std::args` also uses the Windows OS wide command line, converts each value to
strict UTF-8 and owns it until argument reinitialization or process exit. User
indexes exclude the executable. Quotes, empty values, supplementary Unicode and
trailing backslashes survive independently of the active ANSI codepage. A
negative `args_len()` reports initialization failure; `arg_len` and `arg_copy`
return the same existing error status. Lossy ANSI fallback is not used.
Current-directory length/copy functions likewise read the OS wide directory
and return strict UTF-8, preserving a Unicode caller directory. Directory
existence/creation converts UTF-8 to the OS wide path before querying/creating.
The managed `arg_copy` wrapper records `Buffer.used_len` after a successful
copy (including an empty argument); `string_from_buffer` can consume exactly
those initialized bytes without a raw-pointer workaround.
Managed `env_var_copy` follows the same initialized-length rule; Windows
environment names/values use the OS wide API rather than ANSI `getenv`, including
empty values and supplementary Unicode.
`cargo test --locked -p sgc --test args_unicode` verifies the compiled public
boundary, including extreme indexes and a Unicode directory, on Windows x64.

The isolated native probe can also be built with Clang AddressSanitizer. On
Windows, an absolute quoted `ASAN_OPTIONS` destination (for example
`log_path='D:\Sengoo\target\process-terminal-asan'`) retains interceptor messages
separately even when the child changes working directory. The isolated cwd/env
fixture preserves this sanitizer setting. Instrumented child stderr therefore
does not corrupt exact stream byte-count assertions. Do not interpret Windows
ASan as LeakSanitizer support.

Windows x64 verification on 2026-11-04 used clang 19.1.7 with MSVC, based on
`d46b3f6cd8d6e770734f98edc57aa67cb931e26a` plus the working-tree implementation:
2/2 public/native integration tests, 6/6 existing process regressions, the core
idempotent-release regression, and the standalone ASan probe passed. The native
probe includes 10,000 command/output allocation/close cycles, cross-domain and
stale-token rejection, full-pipe backpressure, Unicode executable/argument/cwd
paths and environment values, real hidden-console mode restoration and descendants. POSIX branches
have no execution evidence from this Windows host. Candidate/release gates must
rerun these checks at the eventual committed revision.

The additive screen regression is `cargo test -p sgc --test terminal_screen`:
3/3 passed on the same Windows x64 host on 2026-11-04. Its hidden console checks
native press/release/drag/wheel, raw ownership, previous hidden cursor state,
explicit leave, Drop and normal-exit restoration. The independent parser checks
fragmented SGR packets, Unicode interleaving, invalid coordinates and oversized
parameters. The Sengoo test compiles the public owning API and verifies
redirected-handle behavior. See [local byte IPC](local-terminal-ipc.md) for the
separate execution-host transport.
The clipboard test replaces Win32 clipboard/window calls with an isolated
fixture and uses real UTF conversion and GlobalAlloc ownership. It checks
Unicode, empty/NUL/invalid input and failed open/set/import cleanup without
opening the interactive clipboard. This is transport ownership evidence, not a
real OS clipboard integration run; private window-station creation was denied
on this host. No automated test should overwrite the user's clipboard unless
every original format can be restored.

`cargo test -p sgc --test clipboard_images` adds an isolated read-only clipboard
fixture with real GDI+, COM streams, CF_HDROP parsing, UTF conversion and file
writes. It checks DIB/DIBV5/PNG/CF_BITMAP export, source lifetime, Unicode text and
original paths, no overwrite, dimension/byte/path rejection and repeated cleanup.
The public Sengoo probe type-checks all new functions and executes NUL rejection
before clipboard access. The two-test Rust harness was compiled directly with
`rustc --test`, using the pinned prebuilt `sgc` to avoid changing its identity;
2/2 tests and the prior clipboard-copy fixture passed on Windows x64,
2026-11-05, against baseline `d46b3f6cd8d6e770734f98edc57aa67cb931e26a`
plus the working tree; hashes and commands are retained in
`target/clipboard-image/evidence.json`. A separate private-window-station
probe returned 77 (`CreateWindowStation` error 5) on this host. Real OS clipboard
integration remains unverified; no interactive clipboard was inspected or changed.

`cargo test -p sgc --test process_stdin` adds a dedicated Windows pressure probe
with an external 25-second watchdog. Thirty stopped-reader cycles fill stdin,
perform 1,000 immediate backpressure checks each, retain working reap timeouts
and cancel the owned tree. It verifies grandson exit, closed/exited stdin
errors, borrowed-input reuse and stable OS handle counts. This confirmed the
existing nonwaiting piped behavior; no blocking-write defect was reproduced.

`cargo test --locked -p sgc --test terminal_polling` injects real Windows
hidden-console records and runs both the native boundary and a separately
compiled public Sengoo `Terminal` consumer. It reproduces the earlier false
idle result, checks zero/positive timeouts, queued and split emoji pairs,
Chinese/ASCII order and owned event text, a 2048-record ignored queue, the
64-record nonblocking bound and cancellation. The related `process_terminal`
discard test retains stale-suffix flushing, unchanged modes and fresh input.
On 2026-11-05 Windows x64 (`DESKTOP-FUAQMFB`, build 26300), polling 1/1,
process/terminal 2/2, screen 3/3 and the complete Unicode regression 1/1 passed
against base `d46b3f6cd8d6e770734f98edc57aa67cb931e26a` plus working-tree
changes. The strengthened polling assertions passed again independently.
Runtime source SHA256 was
`56a7cb86331badccba38a920473fbb64f97d68b0ba2a4fbb8077bd54889fc953`;
commands, source hashes and before/after logs are in
`target/terminal-polling/evidence.json`. These tests qualify this Windows input
boundary, not POSIX operation or application-level rendering latency.
