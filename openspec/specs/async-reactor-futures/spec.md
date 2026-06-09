# async-reactor-futures Specification

## Purpose
TBD - created by archiving change async-reactor-futures. Update Purpose after archive.
## Requirements
### Requirement: Async runtime SHALL provide a reactor with IO wakeups

Sengoo SHALL add a reactor layer behind the existing cooperative scheduler for
timer, TCP socket, and supported file-descriptor readiness.

#### Scenario: Reactor unblocks timer, socket, and supported fd futures

- **WHEN** an async program awaits a reactor-backed timer, TCP readiness, or
  supported owned-file-descriptor readiness future
- **THEN** the scheduler registers interest with the reactor
- **AND** the task resumes only after readiness or deadline
- **AND** compiler and native runtime tests cover the behavior

### Requirement: User-defined awaitables SHALL implement a frozen Future contract

Sengoo SHALL accept user types that implement this contract:

```sengoo
enum Poll<T> {
    Ready(T),
    Pending,
}

trait Future<T> {
    def poll(&mut self, ctx: AsyncContext) -> Poll<T>;
}
```

Rules:

- `await value` accepts compiler-generated futures and user types implementing
  `Future<T>`.
- `&mut self` is an exclusive borrow for one poll call. It does not consume the
  future, and `Poll.Pending` preserves the same future value for a later poll.
- The runtime and compiler SHALL reject or prevent concurrent and reentrant polls
  of the same future.
- `Poll.Ready(value)` is terminal. The future SHALL NOT be polled again, and its
  remaining state is dropped exactly once after the ready value is transferred.
- `AsyncContext` is opaque; user code cannot construct, store, return, or compare it.
- `AsyncContext` is valid only for the dynamic extent of the current poll call.
- `poll` must not block the scheduler thread.
- Returning `Poll.Pending` requires registering a wakeup or deadline through
  `ctx` during that poll; otherwise the runtime reports a stable error.
- Futures may flow through locals, parameters, returns, and struct fields only when
  the compiler can prove sound ownership within one async frame or a static handle.
- Cross-thread escape, captured stack references inside returned futures, and storing
  non-static futures in global state remain rejected.

#### Scenario: User Future types compile and poll correctly

- **WHEN** a type implements `Future<T>` and obeys the poll contract
- **THEN** `await` on that type is accepted by the compiler
- **AND** a pending value can be polled again through the same exclusive mutable
  receiver without moving or duplicating the future
- **AND** a pending poll without wakeup registration fails with a stable diagnostic
  or runtime error

#### Scenario: A future is not polled concurrently or after completion

- **WHEN** an implementation attempts a reentrant/concurrent poll of one future,
  or polls it after `Poll.Ready`
- **THEN** the compiler rejects the shape where statically visible
- **AND** runtime-managed dynamic cases fail with a stable error rather than
  aliasing mutable state or polling freed state

#### Scenario: Obsolete phase-only async restrictions are removed with tests

- **WHEN** this change enables an async shape previously rejected only for
  implementation-phase limits
- **THEN** the compiler accepts the shape
- **AND** a regression test in `async_tests.rs` or native async tests guards the
  acceptance

### Requirement: Variadic select SHALL support two to eight homogeneous operands

`select` SHALL accept between two and eight operands of the same result type.

#### Scenario: Three-branch select rotates poll order and does not cancel losers

- **WHEN** a program calls `select(f0, f1, f2)` and multiple operands become ready
- **THEN** each select instance rotates its first-polled operand between polls
- **AND** the first ready operand in the current poll order wins
- **AND** losing branches are not canceled and are dropped through normal future
  cleanup
- **AND** native tests cover at least a three-branch case

### Requirement: Timeout helpers split non-canceling and canceling semantics

Sengoo SHALL preserve the existing non-canceling `timeout` helper and add
`timeout_cancel`, which consumes the inner future and returns `STATUS_TIMEOUT`
after cancel/drop cleanup when the deadline elapses first.

#### Scenario: Existing timeout does not consume the inner future

- **WHEN** a program uses `timeout(future, ms)`
- **THEN** timeout readiness does not consume or cancel the inner future

#### Scenario: timeout_cancel consumes and returns STATUS_TIMEOUT

- **WHEN** a program uses `timeout_cancel(future, ms)` and the deadline elapses before
  the future completes
- **THEN** the operation consumes the future, performs cancel/drop cleanup, and returns
  `STATUS_TIMEOUT`
- **AND** when the future completes before the deadline, the operation returns the
  completed value

#### Scenario: cancel_task remains stable for pending tasks

- **WHEN** a program cancels a pending spawned task by id
- **THEN** `cancel_task` behavior remains stable and covered by tests

### Requirement: Native async dispatch links on Windows CI

The native async runtime SHALL resolve `sengoo_async_poll_dispatch` and related
dispatch symbols on the reference Windows CI host without linker regressions.

#### Scenario: Windows async native tests pass on the reference CI host

- **WHEN** `cargo test -p sgc async_native_runtime` runs on the reference Windows CI
  host with the native toolchain installed
- **THEN** `sengoo_async_poll_dispatch` and related dispatch symbols resolve
- **AND** the async native regression suite passes

