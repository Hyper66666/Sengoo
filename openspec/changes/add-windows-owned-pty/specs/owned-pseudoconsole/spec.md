## ADDED Requirements

### Requirement: Pseudoconsole commands SHALL preserve explicit launch inputs
`std::pty` SHALL offer an owning reusable builder with literal argv, cwd and
controlled environment inheritance. Borrowed String inputs SHALL be copied
using their byte lengths and reject NUL/invalid UTF-8. Launch SHALL be suspended
until assigned to a kill-on-close job and SHALL not inherit unrelated handles.

#### Scenario: A Unicode command uses an isolated working directory
- **WHEN** a caller supplies Unicode argv/cwd/environment and launches a child
- **THEN** the child observes the exact values and selected environment policy
- **AND** the caller may change or release builder input Strings after each setter

### Requirement: Pseudoconsole I/O SHALL be bounded and independent of the caller terminal
Pseudoconsole input/output SHALL use separate 64-KiB owning queues. Read/write
SHALL return immediately with partial bytes or zero backpressure. Poll SHALL
accept 0..60000ms. Output SHALL remain raw merged VT bytes and SHALL never be
written into the user's terminal by the native bridge. Resize SHALL validate
positive dimensions representable by ConPTY.

#### Scenario: A child emits output while it stops consuming input
- **WHEN** the input pipe is full and output exceeds a queue
- **THEN** write returns bounded backpressure and read remains usable
- **AND** the caller can poll, cancel or resize without waiting for input drain

### Requirement: Pseudoconsole lifetime SHALL retain explicit status and cleanup
An owning non-Send/non-Sync process SHALL expose try_wait, tree quiescence, exit status, EOF,
tree termination, timed reap and close. Reap timeout SHALL retain the live
process. Drop/close SHALL terminate descendants, reap the child and release
pending operations without double-free. Foreign and stale handles SHALL fail.

#### Scenario: Cancellation occurs with pending terminal output
- **WHEN** a process and grandson are alive with queued I/O and Drop runs
- **THEN** both processes exit and OS handle ownership returns to baseline
- **AND** queued output does not deadlock pseudoconsole shutdown

#### Scenario: The host has no ConPTY support
- **WHEN** a non-Windows host or Windows without required ConPTY functions calls spawn
- **THEN** the API reports unsupported and does not start a fallback shell
