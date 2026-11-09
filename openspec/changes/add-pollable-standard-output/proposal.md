# Pollable standard-output writers

## Why

The extracted `sgmcp` client already uses bounded, pollable child-process pipes,
but its optional server stdio adapter still writes through synchronous
`io_stdout_write`. A peer that stops reading can therefore block the server's
state owner and delay input, cancellation and timeout handling. The same gap
affects protocol servers, command runners and redirected streaming programs.

`ProcessHandle.try_write_stdin` does not solve this for inherited stdout: its
pipe was created by the runtime with nonwaiting semantics. An inherited handle
cannot safely be assumed to support that mode, or have its flags changed.

## What changes

- Add an owning, single-state-owner writer to generic `std::io`, with bounded
  byte submission, progress polling, cancellation and explicit quiescence.
- Implement the first backend for inherited local Windows byte-pipe stdout and
  stderr using a dedicated byte-only worker and a duplicated endpoint.
- Account for writer slots, worker threads, native handles and queued/in-flight
  bytes; release ownership only after actual worker completion and join.
- Keep the existing synchronous I/O API, runtime ABI version and edition
  behavior compatible. Unsupported endpoint kinds/platforms fail explicitly.
- Subsequently consume this generic API from the MCP stdio adapter without
  adding protocol-aware native code.

## Scope and ownership

The generic writer is implemented in `std::io` and the C runtime. Its isolated
Windows host gate and exact source identities are recorded in `EVIDENCE.md`.
The generic I/O implementer owns its runtime and native regressions; the
ecosystem integration owner owns the MCP adapter and distribution gates.

The first implementation targets validated local Windows byte pipes. Console,
regular-file, message-pipe, socket/device and POSIX backends require separate
host evidence before returning a supported writer. Public interfaces remain
portable and return `STATUS_UNSUPPORTED` when no verified backend applies.

## Impact

Expected implementation locations are `tools/stdlib/io.sg`, generic runtime I/O
sources and native/runtime regression tests. No MCP parsing, JSON, application
state or policy enters a worker. Existing synchronous callers retain their
current behavior. Applications select either this writer or synchronous writes
for an endpoint and drain existing buffered output before switching.
