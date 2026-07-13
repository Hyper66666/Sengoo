## ADDED Requirements

### Requirement: The language SHALL enforce a data-race safety model

The language SHALL provide `Send`/`Sync` marker traits and SHALL reject sharing
or moving non-thread-safe values across thread boundaries.

#### Scenario: Non-Send value cannot cross threads

- **WHEN** a program tries to move a `!Send` value into a thread or shared-state
  API
- **THEN** type-check fails with a stable thread-safety diagnostic

#### Scenario: Safe shared mutation across threads

- **WHEN** multiple threads share state through `Arc<Mutex<T>>`
- **THEN** the program type-checks, the lock guard releases on `Drop`, and the
  shared state is updated without a data race

#### Scenario: Lock outlives its guard

- **WHEN** a program acquires a `Mutex<T>` or `RwLock<T>` guard
- **THEN** moving the owning lock while the guard borrow is active is rejected
- **AND** returning a guard from a non-acquisition wrapper is rejected so a
  guard cannot escape a borrowed lock

### Requirement: The runtime SHALL provide a multi-threaded executor

The runtime SHALL offer a multi-threaded executor in addition to the cooperative
scheduler, with `spawn` requiring `Send` futures. The public contract SHALL
define bounded submission/backpressure, progress, cancellation, shutdown,
error isolation, and join behavior independently of the scheduling algorithm.

#### Scenario: Parallel tasks run and join

- **WHEN** a program spawns multiple `Send` tasks on the multi-threaded executor
- **THEN** the tasks make progress in parallel and their results join
  deterministically

#### Scenario: Executor is saturated or shut down

- **WHEN** submission exceeds configured bounds or shutdown begins
- **THEN** submission returns the documented status without unbounded growth
- **AND** accepted tasks are joined or cancelled per the shutdown contract
- **AND** a task failure does not terminate unrelated worker tasks

### Requirement: The async runtime SHALL provide a cross-platform IO reactor

The runtime SHALL drive timer, socket, and owned-handle readiness on supported
Windows, Linux, and macOS release hosts, closing the prior all-host readiness
deferral for those hosts.

#### Scenario: Owned-fd readiness on a reference host

- **WHEN** an async helper awaits readiness on an owned descriptor, handle, or
  socket on a supported release host
- **THEN** the reactor wakes the task when the fd becomes ready
- **AND** a reference-host test demonstrates the wakeup without busy polling
- **AND** timeout, cancellation, and close unregister the wait without stale
  wakeups or leaked registrations

### Requirement: The runtime SHALL provide a Future trait, channels, and structured concurrency

The runtime SHALL expose a general `Future` trait with a documented `poll`
contract, an mpsc `channel<T>`, and a structured task scope.

#### Scenario: User future via the Future trait

- **WHEN** a user type implements `Future` and is awaited
- **THEN** it is polled per the documented contract and completes with its
  `Output`

#### Scenario: Channel message passing

- **WHEN** one task sends values on a `channel<T>` and another receives them
- **THEN** the receiver observes the sent values in order and is woken
  asynchronously when data arrives
- **AND** the v1 `channel_recv_into` operation replaces an initialized output
  by moving the received `T` exactly once, without requiring `T: Default`
- **AND** public and compiler-known send entry points reject a `!Send` payload

#### Scenario: Structured task scope joins children

- **WHEN** tasks are spawned inside a `task_scope` and the scope exits
- **THEN** all child tasks are joined on normal exit and cancelled on early exit,
  leaving no leaked tasks
