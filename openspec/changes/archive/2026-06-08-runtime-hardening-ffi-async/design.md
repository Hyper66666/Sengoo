## Scope

This change is the P2 runtime hardening lane. It should be implemented after or
alongside stdlib/tooling work, but it is independently archivable when runtime
failure modes become explicit and tested.

## Async Semantics

Accepted semantics to specify and test:

- Scheduler progress and fairness expectations for runnable tasks.
- Cancellation points and cleanup responsibilities.
- Timeout behavior and whether timed-out tasks are guaranteed stopped.
- Task status values and status-category error reporting.
- Resource cleanup for completed, cancelled, timed-out, and failed tasks.

If existing async behavior is narrower than these requirements, implementation
must document the stable subset and return `STATUS_UNSUPPORTED` for unsupported
operations.

## FFI Policy

Dynamic FFI must be one of:

- supported on the host with dynamic library loading, symbol lookup, call-shape
  validation, callback rules where accepted, and tests; or
- explicitly unsupported with stable status errors, diagnostics, and docs.

Unresolved link symbols or host crashes are not acceptable unsupported behavior.

## Platform Policy

Runtime behavior that differs across Windows/POSIX must be described by API:
paths, permissions, symlinks, process termination, signals, stdio capture,
network bind/connect, and path encoding. When portable behavior cannot be
guaranteed, the API returns a stable unsupported or host-specific status.

## Handle And Resource Model

Runtime-owned handles must validate type, generation, closed state, and resource
limits where practical. Invalid handle, double close, use-after-close, and
resource exhaustion return status errors and should not read freed storage.

## Done Definition

This lane is done when runtime failure paths are observable as stable statuses
or diagnostics, and the documented unsupported paths are tested instead of
failing through crashes or unresolved native symbols.
