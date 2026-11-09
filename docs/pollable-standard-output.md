# Pollable standard output (development)

`std::io` provides `io_stdout_writer(capacity_bytes)` and
`io_stderr_writer(capacity_bytes)`, returning `Result<StdWriter, i64>`.
This initial backend supports inherited **local Windows byte pipes**, verified
on Windows 11 x64. Console, regular file, message pipe, remote pipe and POSIX
endpoints return `STATUS_UNSUPPORTED`; other platforms remain unverified.
Existing synchronous writes keep their signatures and behavior.

The owner is `!Send`/`!Sync`. Independent owners may be created on different
threads; registry admission and resource accounting are synchronized. There is
one owner per stdout/stderr endpoint and one dedicated native byte worker per
owner. Workers never access Sengoo values or callbacks. Select one output API
per endpoint, flush buffered synchronous output before switching, and do not
replace standard handles or mix writes while the writer owns that endpoint.

`try_write(&Buffer, length)` copies initialized bytes and returns the accepted
count; zero means backpressure. A call accepts at most 65,536 bytes. Applications
retain a bounded FIFO outbox and submit only the unaccepted suffix. Acceptance
is not native write completion or peer acknowledgement. Bytes are unmodified,
including NUL and split UTF-8. Queue plus in-flight storage shares a fixed ring,
so a blocked worker does not make capacity available prematurely.

Capacity is 1,024 through 1,048,576 bytes. Admission has eight generation-checked
slots and an 8 MiB aggregate byte limit; endpoint exclusivity currently limits
simultaneous public writers to two. Generations retire instead of wrapping.
Invalid input, construction failures and duplicate constructors release every
reservation. Only a duplicated non-inheritable endpoint is closed by cleanup;
the original remains open with unchanged pipe mode.

`poll(timeout_ms)` reports bits: writable 1, drained 2, failed 4,
cancel requested 8, joined/quiescent 16. Zero never waits. Positive timeouts
(at most 60,000 ms) wait only for progress events when no status is ready.
An atomic `snapshot()` returns `state` (1 open, 2 cancel requested, 3 draining,
4 failed, 5 joined), `accepted_bytes`, `written_bytes`, `queued_bytes`,
`in_flight_bytes`, `os_error`, `unknown_delivery` and `capacity_bytes`.
After join the failure/cancellation poll flags remain inspectable. Drained
means native write completion only, with no durability or protocol guarantee.

`finish()` rejects new submissions and drains accepted bytes before stopping.
`cancel()` rejects new submissions and requests cancellation, but never claims
completion. `wait_quiescent(timeout_ms)` returns false when its deadline expires,
retaining the owner and all live resources. It repeats cancellation across the
intent/WriteFile race and reports true only after joining the stopped worker and
closing its duplicate/thread/events and freeing the ring. Failed native writes
record their OS error and unknown partial delivery; output is never replayed.
Drop cancels and joins using the same cleanup. It may wait for a driver to
complete cancellation; no unconditional latency guarantee applies to arbitrary
Windows devices.

`io_writer_resource_count(field)` reports retained owner slots (0), unjoined
workers (1), native handles (2), queued bytes (3), in-flight bytes (4) and
reserved capacity (5). An owner remains charged after join until Drop; all
other counters become zero after join. These diagnostics are independent of
process/opaque-handle counters.

Run `tools/stdlib/tests/verify_std_writer.py --compiler <sgc> --clang <clang>
--formatter <sgfmt> --evidence <report.json>` for real isolated native and
generated-Sengoo checks. The parent intentionally stops reading stdout while
still sending stdin cancellation. The native gate also covers cancellation
before a selected write is issued, full queues, timeout ownership, broken/slow
readers, exact binary bytes, stderr, constructor rollback, stale tokens, OS
handle/thread baselines and concurrent independent constructors. See the
change's `EVIDENCE.md` for the named current-host result.
