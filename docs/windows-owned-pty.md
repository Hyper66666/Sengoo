# Owning Windows pseudoconsole processes

`import std::pty;` adds a generic Windows ConPTY boundary in the development
`0.2.1-dev.0` toolchain containing this change. Edition 2026, runtime ABI 1 and
existing `std::process` APIs remain unchanged. Windows 10 version 1809 or later
with the required ConPTY functions is necessary; availability is detected at
runtime. Other platforms return `STATUS_UNSUPPORTED` and have no execution
qualification from this Windows x64 host.

## Launch ownership

`pty_command(executable: &String) -> Result<PtyCommand,i64>` copies its input.
`PtyCommand` supplies `arg(&String)`, `cwd(&String)`, `inherit_env(bool)`,
`env(name: &String, value: &String)`, `env_remove(name: &String)` and
`spawn(columns, rows) -> Result<PtyProcess,i64>`. Setters borrow the builder;
spawn also borrows it, so a validated builder can be reused. There is no shell
interpretation. To run a shell, choose its executable and argv explicitly.

String inputs preserve their owned byte lengths. Empty argv/value strings are
valid; executable/cwd/environment names must be nonempty. NUL and malformed
UTF-8 are rejected before launch. Environment names are compared without case;
overrides/removals apply to inherited values. `inherit_env(false)` creates a
fresh environment containing only explicit nonremoved edits. If a selected
program needs variables such as SystemRoot, the caller must provide them.

The builder holds at most 128 args and 128 environment edits. Each text field
fits 32,766 UTF-16 units, the complete command line fits 32,767 units including
its terminator, and the resulting environment fits 1,048,576 UTF-16 units.
At most 64 builder/process owners and 16 live pseudoconsole processes exist
per runtime instance. Exceeding a cap returns overflow. Dimensions are 1..1000
cells in each direction. Neither text nor dimensions are an application trust
policy or an operating-system sandbox.

Every process owns its pseudoconsole, direct child, kill-on-close Job Object,
pipes and I/O events. The child starts suspended, joins the job before resume,
and does not inherit unrelated caller handles. Explicit empty startup standard
handles ensure Windows installs the pseudoconsole handles even when the parent
has redirected stdout/stderr. The native bridge never prints the child's
output to the parent's current terminal or stream.

## Byte I/O and process status

Both owners implement Drop and reject Send/Sync bounds. Handles are opaque and
checked for generation and domain before use. They must not be cast to native
pointers. `close(&mut self)` is available when early explicit cleanup is useful;
the wrapper invalidates itself after a successful close, so repeated wrapper
close is safe. Direct stale native tokens return invalid-handle.

| PtyProcess method | Contract |
| --- | --- |
| `read(out: &mut Buffer)` | Immediate byte count, sets used length; at most 64KiB |
| `write(input: &Buffer, length)` | Copies accepted bytes before return, at most 64KiB; zero is backpressure |
| `poll(events, timeout_ms)` | Interests 1 output, 2 input, 4 direct-child exit; combine bits |
| `resize(columns, rows)` | Updates the live pseudoconsole dimensions |
| `try_wait()` | False while direct child runs, true after observed exit |
| `tree_quiescent()` | True only when every child/descendant in the owned job has exited |
| `exit_code()` | Child DWORD exit code as i64, or timeout while running |
| `eof()` | True only when the output pipe is closed and buffered bytes are drained |
| `terminate_tree()` | Terminates the owned job, including descendants |
| `reap(timeout_ms)` | Waits for the direct child and returns its exit code; timeout retains ownership |
| `close_input()` | Cancels queued input and closes its pipe; no application EOF handshake is promised |
| `close()` | Terminates/reaps, cancels pending I/O and frees the complete owner |

Poll accepts 0..60000ms. Results are bit 1 output bytes ready, bit 2 input queue
empty, bit 4 drained output EOF, bit 8 direct child exited. Zero is timeout.
After write, poll interest 2 confirms the native queue has completed, which is
different from the child application processing that input. Do not include an
always-ready input interest in a loop that waits only for output.

ConPTY merges stdout and stderr and emits raw terminal/VT bytes, possibly split
inside UTF-8 or an escape sequence. Sengoo code owns incremental decoding,
terminal emulation, scrollback, sanitization and rendering. Read zero with EOF
false means no bytes currently available. Callers should drain output and send
partial input in a bounded event loop with their own cancellation/deadline.
No native input queue or output queue exceeds 64KiB; OS pipes are bounded too.
This API does not collect the whole transcript or launch a hidden terminal UI.

Direct-child exit does not imply output EOF or tree quiescence: descendants may
remain in the job. Applications holding write permits must retain them until
`tree_quiescent()` succeeds and the direct child has been reaped.
After the job becomes empty the runtime disposes the pseudoconsole on one
bounded-lifetime native worker, because `ClosePseudoConsole` may wait for output
drain. This worker only performs that OS disposal call. Reads and agent state
stay on the owner thread. Close/Drop first terminates the job and child, cancels
pending operations, closes both owner endpoints, then joins disposal before
freeing storage. Closing can discard queued bytes. A killed owner process closes
its job and causes Windows to terminate the owned processes; machine failure is
outside a normal cleanup guarantee.

## Windows x64 evidence

The independent `tools/stdlib/tests/pty_probe.c` uses an isolated pseudoconsole,
not the user's current terminal. It verifies Chinese/emoji interactive
ReadConsole/echo, quoted/trailing-backslash argv, Unicode cwd and controlled
environment, 96x28 to 73x19 resize, exit code/EOF, and borrowed-input reuse.
Twelve stopped-reader cycles each perform 1,000 immediate full-pipe checks;
unread output floods are cancelled without a disposal deadlock. A real grandson
is terminated by the job, eight failed launches release partial resources, and
OS handle counts return to baseline. Invalid UTF-8/NUL/arg caps and stale-domain
identities are exercised. Clang 19.1.7 AddressSanitizer passed this probe;
Windows LeakSanitizer is not claimed, and OS/live-owner checks cover cleanup.

`cargo test --locked -p sgc --test owned_pty` compiles that independent probe
and a public Sengoo consumer. The latter launches real Windows PowerShell,
streams Chinese/emoji text, sends a Read-Host answer, resizes, verifies exit 9,
and leaves a still-running process to automatic Drop. Redirected caller output
must remain empty. The suite also checks rejection of Send/Sync for both owner
types. Its external 45-second watchdog turns a cleanup stall into a failure.
The suite passed 3/3 on Windows x64. The corresponding LSP symbol regression
passed 1/1, and the fresh installed-runtime consumer built and ran a
`std::pty`/cmd.exe process outside the checkout without invoking Cargo or
retaining the source runtime identity (1/1, 99.85 seconds).

The host baseline is
`d46b3f6cd8d6e770734f98edc57aa67cb931e26a` plus working-tree additions, using
clang 19.1.7 and MSVC on 2026-11-04. Candidate/release verification must rerun at
a committed revision; these checks do not certify an earlier published binary
or a non-Windows platform.
