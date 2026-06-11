## MODIFIED Requirements
### Requirement: Async default follow-ups SHALL preserve current supported subset

Sengoo SHALL keep the current async supported subset stable while separately
owning remaining mainstream-default async gaps. This includes preserving
timer/TCP reactor wakeups, platform-specific owned-fd readiness,
`select(2..8)`, `timeout`, `timeout_cancel`, opt-in thread pool, channels,
mutexes, `async-channel-smoke`, user-defined Future support, and any
documented async HTTP serving subset once `async-http-serving` lands.

Async HTTP serving SHALL reduce the "no async HTTP server loop" gap only for
the host-scoped subset proven by its own native and realworld tests. It SHALL
NOT imply general task cancellation, select loser cancellation, TLS server
support, streaming bodies, keep-alive, or all-host owned-fd readiness.

#### Scenario: Existing async subset remains available

- **WHEN** implementation works on this change
- **THEN** timer/TCP reactor wakeups, platform-specific owned-fd readiness,
  `select(2..8)`, `timeout`, `timeout_cancel`, opt-in thread pool, channels,
  mutexes, async HTTP serving where documented, and `async-channel-smoke`
  behavior remain source-compatible
- **AND** unsupported paths fail with stable diagnostics or statuses rather than
  invalid LLVM or unresolved native symbols

#### Scenario: Async HTTP serving does not expand cancellation claims

- **WHEN** an async HTTP request future is dropped or times out
- **THEN** its own reactor interest and accepted-but-unpublished connection are
  cleaned up according to `stdlib-http-server`
- **AND** support matrices still mark broad task cancellation and select loser
  cancellation as Deferred unless separately proven
