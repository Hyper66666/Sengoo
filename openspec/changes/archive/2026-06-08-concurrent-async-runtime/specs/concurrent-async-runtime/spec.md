## ADDED Requirements

### Requirement: Async runtime exposes an opt-in thread pool

Sengoo SHALL keep the existing cooperative single-thread scheduler as the default
and SHALL provide an explicit API to enable a bounded worker thread pool for
blocking and CPU-bound work.

The v1 public API SHALL live in `std::async` and SHALL include
`runtime_enable_thread_pool(worker_count: i64) -> Result<i64, i64>` and
`spawn_blocking_i64(work: fn() -> i64) -> Result<Future<i64>, i64>`. Generic
blocking returns or runtime-handle returns require a follow-up OpenSpec change.

#### Scenario: Default programs remain single-threaded

- **WHEN** a program does not call the thread-pool enable API
- **THEN** async scheduling behavior matches the pre-change cooperative runtime
- **AND** existing `async_native_runtime` tests pass without modification

#### Scenario: Thread pool enables parallel blocking i64 work

- **WHEN** a program calls `runtime_enable_thread_pool(n)` with `n >= 1` and awaits
  a future returned by `spawn_blocking_i64(work)` where `work` satisfies `Send`
  bounds
- **THEN** `work` executes on a pool thread while the scheduler can poll other tasks
- **AND** the awaiting task resumes with the returned value on the scheduler thread

#### Scenario: Disabled or invalid pool setup fails predictably

- **WHEN** a program calls `runtime_enable_thread_pool(n)` with `n < 1`
- **THEN** it returns `Err(STATUS_INVALID_ARGUMENT)`
- **WHEN** a program calls `spawn_blocking_i64` before enabling the pool
- **THEN** it returns `Err(STATUS_UNSUPPORTED)` without starting host work

### Requirement: Cross-thread spawn_blocking captures SHALL satisfy Send bounds

The compiler SHALL reject `spawn_blocking_i64` closures that capture non-`Send`
values when cross-thread execution is possible. For this change, values are
`Send` only if they are primitive scalars, unit, or aggregates composed entirely
of sendable values. Runtime handles (`Buffer`, JSON/process/file/dir handles,
async `Future`/task handles, `AsyncContext`, raw pointers, references, and FFI
library/symbol handles) SHALL be non-`Send` unless a later spec explicitly opts
them in.

#### Scenario: Non-Send capture is rejected at compile time

- **WHEN** a `spawn_blocking_i64` closure captures a value the compiler cannot
  prove `Send`
- **THEN** type checking fails with a stable diagnostic naming the captured binding
- **AND** no runtime undefined behavior occurs from the rejected program

#### Scenario: Poll-scoped AsyncContext cannot cross worker threads

- **WHEN** a `spawn_blocking_i64` closure captures `AsyncContext`, a future handle,
  or a borrowed reference derived from the async frame
- **THEN** the compiler rejects the program before lowering
- **AND** no runtime worker thread can poll or resume through that captured context

### Requirement: Bounded channels and mutex primitives integrate with async

Sengoo SHALL expose bounded async channels and a mutex type with an async lock
helper that does not block the scheduler thread while waiting.

#### Scenario: Channel send and receive round-trip

- **WHEN** a program creates `channel_bounded(8)`, sends a value, and receives it
  from another async task on the same runtime
- **THEN** the received value matches the sent value
- **AND** a full channel causes send-side pending until capacity is available

#### Scenario: Mutex serializes conflicting access

- **WHEN** two async tasks contend for the same `Mutex` protected state
- **THEN** `lock_async` grants exclusive access to one task at a time
- **AND** the loser polls pending without blocking the scheduler thread

#### Scenario: Channel close and mutex drop are deterministic

- **WHEN** all senders for a bounded channel are closed or dropped
- **THEN** pending receivers wake and return a stable closed status
- **WHEN** a `Mutex` handle is closed while tasks are waiting
- **THEN** waiters wake and return `STATUS_INVALID_HANDLE`

### Requirement: Reactor and select semantics remain stable under concurrency

Enabling the thread pool SHALL NOT change `select` loser non-cancellation,
`timeout` non-consuming behavior, or `timeout_cancel` `STATUS_TIMEOUT` semantics
documented in `docs/runtime-async-semantics.md`.

#### Scenario: Select losers are still not canceled with thread pool enabled

- **WHEN** `select(f0, f1)` runs with the thread pool enabled and multiple operands
  become ready
- **THEN** losing operands are not canceled and follow normal drop cleanup
- **AND** native tests cover at least a two-branch case under concurrent mode

#### Scenario: Blocking future cancellation does not kill host work

- **WHEN** a blocking future is dropped or canceled after its worker closure starts
- **THEN** the runtime does not forcibly terminate the host thread
- **AND** completion discards the result without resuming the canceled waiter
