# Terminal frame admission and retained-handle allocation

Additive native development API: `terminal.write_frame(text, width, height)`
returns `Result<bool, i64>`. True means dimensions matched before and after
writing; false means a geometry race or stale expected dimensions. Invalidate
the UI's previous-frame cache and retry on a later event-loop frame. The call
does not compute layout, consume keyboard input, enter fullscreen or own a UI.
TTY, handle and argument failures use existing status codes. Ordinary `write`
retains its behavior. A geometry change can still happen after the final
check; applications should consume resize events and perform a settled repaint.
Windows buffer notifications now deliver resize invalidation even if an
away-and-back transition leaves the same final dimensions.

The native String, Buffer and opaque handle allocators reuse freed slot indices
through private freelists rather than scanning retained live values. Encoding,
generation checks, idempotent same-generation free/take and thread ownership
remain unchanged. Invalid or repeated release never enqueues the same slot
twice. No ABI-version bump or unbounded per-allocation log is introduced.

Independent native `tools/stdlib/tests/handle_churn.sg` retains 20000 values and
repeats temporary creation/release 100000 times in each domain, tests stale
generations and verifies live counters return to baseline. Process/terminal C
tests cover live geometry mismatch, same-size invalidation, queued keys and
mode restoration. These are focused Windows checks; other platforms retain
the generic interface and need their own execution evidence.

Run the isolated allocation regression with the native development compiler:

```text
sgc --runtime-mode source-development build tools/stdlib/tests/handle_churn.sg -o target/handle-churn
```

Run the resulting executable to check retained values, stale generations and
live-count cleanup. Its three printed durations measure String, Buffer and
opaque allocation loops; they are coarse monotonic milliseconds, not terminal
input latency. The CLI's real-console wheel fixture measures delivery separately.
