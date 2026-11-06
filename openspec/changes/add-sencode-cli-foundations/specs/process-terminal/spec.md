## ADDED Requirements

### Requirement: Piped processes SHALL have bounded independent I/O and explicit status

The native process API SHALL spawn literal argv with independent stdin/stdout/stderr
pipes. Each read/write copies at most 64 KiB without accumulating output in the
runtime. `try_wait` SHALL distinguish a running process from an exited/reaped
process. A wait timeout SHALL leave the process reachable and running.

#### Scenario: Both output pipes exceed kernel capacity
- **WHEN** a child emits at least 512 KiB on both stdout and stderr
- **THEN** alternating bounded reads receive both streams without deadlock
- **AND** EOF and temporary lack of bytes remain distinguishable

#### Scenario: A nonblocking wait times out
- **WHEN** `wait(0)` or `reap(0)` times out on a sleeping child
- **THEN** `try_wait` still reports running
- **AND** exit-code lookup does not fabricate a completed result

### Requirement: Owned process trees SHALL be terminated and reaped

Windows spawning SHALL use Unicode APIs, an explicit inherited handle list, and
an assigned kill-on-close Job Object before the child starts. POSIX children SHALL
use an owned process group. Explicit termination and Drop SHALL stop the owned
tree, reap the direct child and close all pipes. Deliberate POSIX process-group
escape and OS-uninterruptible processes are outside this boundary.

#### Scenario: A child starts a grandchild
- **WHEN** an owning handle is dropped while its grandchild is sleeping
- **THEN** the direct child is reaped and the owned grandchild stops
- **AND** closed generation handles reject further status queries

#### Scenario: Unicode argv and executable paths
- **WHEN** Windows receives a UTF-8 executable path and a Chinese argument with spaces and quotes
- **THEN** CreateProcessW passes the exact argument to the child without a shell

### Requirement: Terminal input SHALL expose bounded native events and restore state

The native bridge SHALL expose UTF-8 key, bracketed paste, resize, cancellation,
timeout and EOF events; line editing and rendering SHALL remain Sengoo code.
Paste is limited to 64 KiB and overflow is drained through its end marker before
subsequent keys. Raw mode is explicit and unsupported on non-TTY handles. Drop
and normal exit SHALL restore modes and codepages. Fatal process termination
without cleanup is not claimed to restore the terminal.

#### Scenario: Redirected input
- **WHEN** stdin/stdout are redirected
- **THEN** opening and reading events does not enable raw mode
- **AND** attempting raw mode reports unsupported and EOF remains observable

#### Scenario: Paste contains newlines or malformed bytes
- **WHEN** a bounded bracketed paste contains Unicode and newlines
- **THEN** one paste event owns the complete UTF-8 text
- **AND** invalid UTF-8 or oversized paste produces an explicit status

#### Scenario: Raw console ownership ends
- **WHEN** the owning terminal restores or drops after key/cancel/resize events
- **THEN** the saved input/output modes and Windows codepages are restored

### Requirement: Terminal input can be explicitly discarded at an interaction boundary
The terminal SHALL provide an explicit TTY-only discard operation that clears
pending OS input and partial decoder/paste state, preserves modes, owned handles
and cancellation signals, and reports unsupported for redirected streams.

#### Scenario: Ignored console events precede pretyped confirmation text
- **WHEN** queued KEYUP or mouse records precede old text and discard is called
- **THEN** old OS records, partial UTF-16 and decoder/paste bytes are removed
- **AND** subsequently entered text remains readable without changing raw mode.

### Requirement: Terminal output can clear its trailing buffer without reading text
The terminal SHALL expose a TTY-only clear operation from the current cursor to
the end of the output buffer. On Windows it SHALL include rows outside the
viewport and preserve cursor coordinates, modes and codepages. This generic OS
operation SHALL NOT inspect or return transcript text.

#### Scenario: Windows reflows an application-owned output region below the viewport
- **WHEN** the application positions its cursor at the region's known origin and
  clears its trailing output
- **THEN** stale characters and their attributes below the viewport are cleared
- **AND** preceding output and cursor position remain unchanged.
