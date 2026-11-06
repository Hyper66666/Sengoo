# Design

`PtyCommand` owns copied UTF-8 argv/cwd/environment edits. Borrowed String inputs
pass their real byte lengths, rejecting NUL and invalid UTF-8 before launching.
The builder supports literal argv, explicit cwd, inherited environment on/off
and case-insensitive environment overrides/removal. It borrows on spawn and is
reusable. There is no implicit shell or application command policy.

`PtyProcess` owns HPCON, direct child, kill-on-close job, pipes, events and two
64-KiB byte queues. The ConPTY ends use synchronous byte handles; the owner ends
use overlapped I/O so read/write never wait for another application to consume
bytes. Poll may wait at most 60 seconds. The child starts suspended, joins its
job before resume, and does not inherit unrelated handles. Output is raw merged
terminal/VT bytes; no native code writes it to the current terminal.

The process remains reachable after reap timeout. Read zero is distinct from
EOF; try_wait only reports direct-child completion. Tree termination uses the
job. Close/Drop terminates the job and direct child, cancels pending pipe I/O,
and completes outstanding operations before releasing storage. ConPTY shutdown
must not deadlock on a full output pipe: cancellation/closure of owner pipe
ends and job termination precede pseudoconsole disposal. Normal output draining
is the caller's responsibility; close can discard pending bytes.

Handles are opaque, generation-checked, domain-checked and non-Send/non-Sync.
No JSON, agent state, protocol parsing or renderer is present in the C bridge.
