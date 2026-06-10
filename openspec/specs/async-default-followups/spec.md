# async-default-followups Specification

## Purpose
Capture the async runtime's current default-readiness contract after the reactor,
user-future, select, timeout, thread-pool, channel, and mutex waves, while
pinning the remaining deferred gaps such as all-host owned-fd readiness,
exhaustive user-future diagnostics, wakeup-registration enforcement, and public
task cancellation APIs.

## Requirements
### Requirement: Async default follow-ups SHALL preserve current supported subset

Sengoo SHALL keep the current async supported subset stable while separately
owning remaining mainstream-default async gaps.

#### Scenario: Existing async subset remains available

- **WHEN** implementation works on this change
- **THEN** timer/TCP reactor wakeups, platform-specific owned-fd readiness,
  `select(2..8)`, `timeout`, `timeout_cancel`, opt-in thread pool, channels,
  mutexes, and `async-channel-smoke` behavior remain source-compatible
- **AND** unsupported paths fail with stable diagnostics or statuses rather than
  invalid LLVM or unresolved native symbols

### Requirement: User-defined Future lowering SHALL declare a bounded supported subset

User-defined `Future::poll` SHALL be supported only for the same-thread
cooperative v1 subset proven by compiler and native tests. Broader lifecycle
semantics, exhaustive diagnostics, and wakeup-registration rules SHALL remain
Deferred until separately proven. The public v1 surface SHALL use the existing
`Poll<T>`, `AsyncContext`, and `Future<T>::poll` contract from
`tools/stdlib/async_futures.sg`.

#### Scenario: Same-thread user Future support is claimed

- **WHEN** a user-defined type implements the pinned `Future<T>::poll` contract
- **THEN** `await value` compiles, polls through the same exclusive receiver, and
  resumes after wakeup without moving or duplicating the future
- **AND** compiler and native tests cover immediate Ready, Pending-then-Ready,
  multiple await points, local/parameter/return flow, and no repoll after Ready
  for the same await operation
- **AND** cleanup hooks, source-level reentrant/concurrent poll, source-level
  poll-after-Ready, and explicit wakeup-registration diagnostics remain
  Deferred rather than hidden behind the supported-subset claim
- **AND** `sgc` JSON and `sglsp` report stable representative diagnostics for
  malformed v1 contract shapes

#### Scenario: Poll contract uses the canonical struct surface

- **WHEN** code defines `Poll<T> { is_ready: bool, value: T }`,
  `AsyncContext { handle: i64 }`, and
  `trait Future<T> { def poll(&mut self, ctx: AsyncContext) -> Poll<T>; }`
- **THEN** the compiler treats `is_ready: false` as Pending and `is_ready: true`
  as Ready
- **AND** Ready is terminal for that future
- **AND** implementation does not introduce alternate public enum syntax for
  this lane

#### Scenario: Poll-scoped context cannot escape

- **WHEN** source attempts to construct, store, return, compare, capture across
  `spawn_blocking`, or place `AsyncContext` in a struct/global
- **THEN** compiler and `sglsp` diagnostics reject the program with a stable
  async-context diagnostic
- **AND** no invalid LLVM or unresolved runtime symbol is produced

#### Scenario: Broader user Future lifecycle remains deferred

- **WHEN** a candidate user Future behavior requires source-level reentrant poll,
  concurrent poll, manual poll-after-Ready, explicit wakeup registration, or
  user-defined cleanup hooks
- **THEN** the behavior is documented as Deferred
- **AND** support matrices do not claim it as part of the v1 supported subset

### Requirement: IO readiness defaults SHALL distinguish platform-specific support

All-host owned-fd readiness SHALL remain Deferred unless this change documents
and tests each host policy. Platform-specific readiness SHALL name host/file
handle shapes and unsupported cases.

#### Scenario: All-host owned-fd readiness is promoted

- **WHEN** all-host owned-fd readiness is claimed
- **THEN** Windows, POSIX, and unsupported-host behavior are documented
- **AND** tests prove supported file/socket/pipe shapes or record evidenced skips
- **AND** unsupported shapes return stable status or diagnostics

### Requirement: Cancellation defaults SHALL be user-visible and bounded

Task cancellation, timeout cancellation, and select loser behavior SHALL have
documented cleanup semantics before any new user-facing cancellation claim is
made.

#### Scenario: Select loser cancellation remains deferred

- **WHEN** `select` chooses a winning operand
- **THEN** losing operands are dropped through normal cleanup
- **AND** docs and support matrices do not claim cancellation unless tests prove
  the cancellation behavior

### Requirement: Public async cleanup wrappers SHALL lower correctly

Public `std::async` cleanup helpers SHALL not generate invalid LLVM in
package-shaped async programs.

#### Scenario: Cleanup helpers are used in a package smoke

- **WHEN** a package-shaped Sengoo program calls `channel_pair_drop`,
  `channel_sender_drop`, or `mutex_drop`
- **THEN** `sgpm check`, `sgpm test`, `sgpm build`, and `sgc run` succeed or fail
  with a stable diagnostic
- **AND** runtime cleanup/drop wakeup tests still pass
