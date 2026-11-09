# Design

## Current evidence and platform constraints

`tools/stdlib/runtime.c` implements stdout/stderr using `fwrite` and `fflush`,
and `io_fd_write` performs a direct `WriteFile`/`write`. These calls can wait for
the reader. `runtime_process.c` avoids that for child stdin by creating its own
Windows `PIPE_NOWAIT` endpoint or setting its owned POSIX pipe nonblocking. It
also bounds each write and reports partial progress. The new API reuses those
semantics and validation patterns, not their ownership assumptions.

Windows `CancelSynchronousIo` requests cancellation for a particular thread;
it does not wait for completion and the write may still complete normally.
The worker therefore has a dedicated thread handle for its entire lifetime,
and cancellation never targets unrelated pooled work. The implementation must
close the cancellation-before-WriteFile race: set the terminal intent, wake an
idle worker, request cancellation, and repeat the cancellation request while a
write remains issued until the worker acknowledges completion. `ERROR_NOT_FOUND`
is a race observation, not proof of quiescence. No mutex is held across a write.

See [CancelSynchronousIo](https://learn.microsoft.com/en-us/windows/win32/api/ioapiset/nf-ioapiset-cancelsynchronousio)
and [cancellation considerations](https://learn.microsoft.com/en-us/windows/win32/fileio/canceling-pending-i-o-operations).
These references establish the need to observe completion; they do not prove
the Sengoo implementation or provide a universal cancellation latency bound.

Duplicating a POSIX fd shares its open file description and status flags.
Changing `O_NONBLOCK` on that duplicate can consequently change the original.
Similarly, readiness followed by a blocking write does not guarantee a
nonblocking operation in the presence of another writer. The initial POSIX
implementation returns unsupported. A later backend must either own a separate
nonblocking endpoint or prove a safely cancellable worker with deferred-only
cancellation, cleanup handlers, signal handling and a real join. Neither
`pthread_cancel` alone nor detached threads satisfy this contract.

## Additive interface

The implemented API is:

```text
io_stdout_writer(capacity_bytes: i64) -> Result<StdWriter, i64>
io_stderr_writer(capacity_bytes: i64) -> Result<StdWriter, i64>
StdWriter.try_write(bytes: &Buffer, length: i64) -> Result<i64, i64>
StdWriter.poll(timeout_ms: i64) -> Result<i64, i64>
StdWriter.cancel() -> Result<bool, i64>
StdWriter.finish() -> Result<bool, i64>
StdWriter.wait_quiescent(timeout_ms: i64) -> Result<bool, i64>
StdWriter.snapshot() -> Result<StdWriterStatus, i64>
```

`StdWriter` is an owning, non-Send/non-Sync value with generation-checked native
identity. `try_write` copies at most 64 KiB from a validated managed Buffer and
returns the accepted byte count; zero means backpressure. Acceptance is not
delivery. `poll(0)` performs no blocking I/O, and positive timeouts only wait for
bounded status/wakeup events. A snapshot reports state, accepted bytes, confirmed
written bytes, queued/in-flight bytes, the original OS error and whether the
last operation's partial delivery is unknown. Byte counts use checked arithmetic.

Poll bits distinguish writable capacity, drained, terminal failure, cancellation
requested and actual quiescence. Drained means all accepted bytes completed at
the OS write boundary, not that a peer parsed or acknowledged them. It implies
neither durable file synchronization nor protocol delivery. The API adds no
automatic replay and performs no UTF-8 decoding or newline conversion.

The worker owns only a native queue, duplicate endpoint, counters and wakeup
resources. Sengoo Strings, JsonDocs, agent state and user callbacks never cross
the worker boundary. Each write is FIFO with its own bounded completion update.

## Capacity and ownership

The initial defaults are a 64 KiB per-writer capacity, configurable from 1 KiB
through 1 MiB, at most eight live writers and at most 8 MiB of aggregate charged
byte storage. Queue plus in-flight storage count against the same capacity;
moving bytes into a worker does not create space before that storage is safe
to release. Admission reserves limits before allocation or copying, and rolls
back all reservations on any construction or spawn failure.

There is at most one live writer per standard endpoint, and at most one worker
per writer. A duplicate constructor for the same endpoint reports conflict;
mixing synchronous writes or replacing the endpoint while a writer owns it is
outside the supported ordering contract. Construction does not call a blocking
stdio flush implicitly. The caller must complete such flushing before selecting
the asynchronous writer. The runtime duplicates, never closes or changes, the
original standard handle. New handles are non-inheritable.

## Lifecycle and reclamation

The public states are open, cancel-requested, draining/closing, failed and
quiescent. Cancellation immediately rejects new submissions and discards only
not-yet-issued bytes once safe. In-flight data and all worker-visible storage
remain owned until actual completion. Completed writes can race with cancellation
and still increase confirmed-written counters. A failed/aborted write with an
unavailable partial count records unknown delivery; it is never replayed.

`cancel` requests termination and returns promptly. `wait_quiescent` reports a
timeout while preserving a usable owner if cleanup has not completed. It may
return true only after the worker has stopped, its thread has been joined, its
duplicate endpoint and wakeups have closed, and no queued storage is reachable.
Subsequent status inspection remains valid until the writer value is dropped.

Drop requests cancellation and performs the same real join before freeing the
state. No `TerminateThread`, detached cleanup, process-exit-only reclamation or
success-on-cancel-request shortcut is permitted. If an underlying driver does
not complete cancellation, Drop may wait; the runtime must not turn that into
a false quiescent status or use-after-free. The first supported pipe backend
requires blocked-reader native tests on the actual Windows release host.
Measured cancellation timings are evidence for those hosts/endpoints, not a
guarantee for every Windows handle or uninterruptible kernel operation.

## Accounting and compatibility

Add generic debug/test snapshots for live writers, worker threads, native writer
handles, queued bytes and in-flight bytes. Counts remain charged until resources
are actually released. Existing process live-handle/resource counters stay at
their baseline because this backend does not spawn child processes. Repeated
failure, cancellation and Drop tests also inspect OS handle/thread counts to
cross-check the runtime's own accounting. Stale tokens cannot address a new
writer occupying a reused slot.

Old synchronous I/O signatures and statuses are unchanged. Constructors use the
existing invalid argument, capacity, I/O and unsupported status families; a
timeout is distinct from a completed close. Successful Result payloads are
valid owned values, and inactive error payloads contain no owning handles.

## Verification and integration

Native tests use a separate parent process which intentionally never reads the
writer child's stdout, fill the pipe, continue sending input and cancellation,
and require the child to observe input, stop, join and recover all resources.
Race hooks cover cancel before worker wait, before native write, during blocked
write and after write completion. Further cases cover exact binary bytes,
partial writes, slow/broken readers, zero length, invalid capacity/length,
queue saturation, constructor failures and stale-handle reuse. Capacity checks
must precede dereferencing a source buffer. Existing buffered synchronous calls
remain independently tested.

Only after those gates pass will `SgMcpStdio` replace its synchronous output
adapter with the generic writer. The adapter keeps an ordered bounded outbox,
accepts partial submissions, polls stdin while output is blocked, retains one
deadline and cleans up on cancel. Independent client/server tests verify that a
blocked peer cannot prevent server cancellation or cause an unbounded queue.
SDK integration has separate independent-peer and external-consumer evidence;
the runtime gate alone does not establish those results.
