# Local byte IPC and terminal screen additions

These APIs preserve edition 2026, runtime ABI 1 and existing event kinds.
`std::ipc` currently implements Windows x64 only; other platforms return
`STATUS_UNSUPPORTED`. This is a development toolchain addition, not a claim that
older v0.2 compilers expose these names. Terminal screen/mouse semantics are in
[the terminal foundation](process-terminal-runtime.md).

## Owning local transport

`import std::ipc;` supplies `ipc_listen(name)` and `ipc_connect(name)`, returning
`Result<IpcListener,i64>` and `Result<IpcStream,i64>`. Both values own their OS
resources, implement Drop and cannot be sent/shared between threads. Endpoint
names contain 1 through 120 ASCII letters, digits, dot, underscore or hyphen.
Do not pass a filesystem path, a raw named-pipe path or a remote endpoint.

The bridge derives a local Windows named-pipe name containing the current logon
SID and session ID. A protected access list admits the current logon SID only;
remote clients are rejected. Connecting and accepting separately inspect the
peer process token and require the same Windows user, logon SID and session.
The API fails closed if token inspection fails. The connection identifies an OS
peer; application authorization and protocol validation remain application work.

| Operation | Behavior |
| --- | --- |
| `listener.try_accept()` | Nonblocking; success with `stream.is_open()==false` means no client waiting |
| `stream.read(out: &mut Buffer)` | Copies at most 64 KiB, updates used length; zero means no ready bytes or EOF |
| `stream.write(data: &Buffer, length)` | Accepts at most 64 KiB into an owned queue; zero means queue backpressure |
| `stream.poll(events, timeout_ms)` | Interest 1 readable, 2 writable, 3 either; result bits 1 data, 2 queue empty, 4 EOF |
| `stream.eof()` | True only after peer closure and all buffered input was consumed |
| `stream.peer_pid()` | PID observed during the successful peer identity check |
| `stream.close()` / `listener.close()` | Cancel pending operations, release native handles, invalidate the owner |

`ipc_connect` never waits for missing/busy endpoints. Missing names report
not-found, busy endpoints report timeout, and a duplicate listener reports
already-exists. An endpoint supports multiple accepted peers; each accepted
stream remains independent while the listener accepts its next peer. A process
has at most 64 live listeners and streams combined. Exceeding that bound reports
overflow. Every stream has one 64-KiB read buffer and one 64-KiB write buffer.

The caller may release or mutate its input Buffer immediately after `write`
returns accepted bytes. Acceptance is not an application acknowledgment. Use
`poll(2, timeout)` to wait until the native write queue finishes before closing
when delivery matters. Close cancels queued writes; it does not flush or wait for
the application peer to acknowledge anything. `poll` accepts 0..60,000 ms;
zero is a nonblocking check. Use separate read/write interests to avoid spinning
on a stream that is always writable. The timeout returns zero without closing
the stream. Cancellation completes before releasing each OVERLAPPED/buffer.

The bridge handles only bytes. Framing, bounded application messages, protocol
versions, request IDs, handshake, retry/duplicate policy, host ownership and
agent state belong in Sengoo application code. No JSON object or database handle
crosses into this transport. There is no implicit reconnection or replay.

```sg
import std::ipc;
import std::ffi;

def exchange() -> i64 {
    let listener = ipc_listen("example.v1");
    if !listener.is_ok { return 1; }
    let client = ipc_connect("example.v1");
    if !client.is_ok { return 2; }
    let server = listener.value.try_accept();
    if !server.is_ok || !server.value.is_open() { return 3; }
    let data = ffi_buffer_from_bytes("hello");
    if !data.is_ok { return 4; }
    let written = client.value.write(&data.value, 5);
    if !written.is_ok || written.value != 5 { return 5; }
    if client.value.poll(2, 1000).value != 2 { return 6; }
    let mut out = ffi_buffer_new(64);
    if !out.is_ok || server.value.poll(1, 1000).value != 1 { return 7; }
    let read = server.value.read(&mut out.value);
    if !read.is_ok || read.value != 5 { return 8; }
    return 0;
}

def main() -> i64 { exchange(); }
```

Production peers should loop over `try_accept`, read/write and poll, checking
every Result and applying their own cancellation/deadline policy. The compact
example demonstrates local ownership and automatic cleanup only.

## Windows x64 evidence

On 2026-11-04, the working tree based on
`d46b3f6cd8d6e770734f98edc57aa67cb931e26a` used clang 19.1.7 with the local MSVC
toolchain. `cargo test -p sgc --test local_ipc` passed 2/2: an independent native
probe plus compiled public Sengoo APIs and Drop. The native probe uses two real
processes and binary payloads above pipe capacity, mutation of borrowed input
after accepted writes, backpressure, EOF, sequential clients, duplicate listener
rejection, malformed names and pending I/O cleanup. Eighty pressured connection
cycles leave the process handle count stable. Security checks inspect the real
pipe DACL, actual peer PIDs/tokens and rejection of altered identity fields.
This host did not run a second Windows login or other-user process; the identity
comparison and restricted DACL have executable coverage, but no such cross-login
execution is claimed. Non-Windows branches are explicitly unsupported.

Runtime source, installed bridge payload and LSP registration include ipc.sg and
runtime_ipc.c. `cargo test -p sgc --test runtime_distribution` passed 15/15 after
these additions: its fresh relocated installed toolchain builds and runs a
consumer of IPC, terminal, Unicode and file hashing without invoking Cargo or
retaining the source-checkout runtime identity. The matching LSP symbol
regression passed 1/1. Candidate/release verification must rerun the named checks at a
committed SHA; these working-tree results are not a published release.
