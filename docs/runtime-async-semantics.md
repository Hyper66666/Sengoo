# Sengoo Async Runtime Semantics

> Status: stable subset documented June 2026. Unsupported operations return
> compile-time or runtime errors; they do not leave unresolved native symbols.

## Scheduler

- **Model:** cooperative, single-threaded tick loop (`CoroutineScheduler`).
- **Progress:** each `tick` polls every queued task at most once per cycle.
- **Fairness:** round-robin via `VecDeque`; tasks that return `Pending` with a
  wakeup deadline are re-queued and skipped until `Instant::now()` reaches the
  deadline.
- **Idle:** `run_until_idle` loops `tick` until the queue is empty, sleeping
  until the nearest deadline when all tasks are waiting.

## Task lifecycle (`task_status`)

| Code | Name | Meaning |
|------|------|---------|
| 0 | unknown | Task id not tracked (never spawned or already collected) |
| 1 | pending | Spawned and not yet completed or canceled |
| 2 | completed | Polled to completion |
| 3 | canceled | Removed via `cancel_task` while still pending |

`spawn_task` registers lifecycle status. `spawn(future)` returns an awaitable
future without exposing lifecycle ids.

## Structured task scopes

- `task_scope()` creates an opaque, compiler-known `TaskScope` guard.
  User-written `TaskScope { ... }` literals, local aggregate storage, aggregate
  fields, and return types are rejected so the scope capability cannot escape
  or be forged. The lexical owner is introduced directly with
  `let scope = task_scope()`.
- `scope_spawn(&scope, direct_send_future)` returns `1` when the child is
  accepted and `0` when the scope/executor rejects it. It never exposes the
  child task id. Rejected future frames are cancelled or dropped exactly once.
- Normal lexical fallthrough joins every child before the guard's idempotent
  `Drop`. `return`, `?`, `break`, `continue`, and abort cleanup skip that normal
  marker, so `Drop` cancels pending children and then joins their terminal
  states.
- A worker joining a child pinned to its own affinity queue helps poll that
  queue. This preserves progress for nested scopes on a one-worker executor
  without changing fixed-affinity scheduling.
- Scope registry entries are removed at teardown. Runtime stress coverage
  asserts no live scope or executor task remains after repeated joins.

## Cancellation

- `cancel_task(id)` removes a **pending** queued task and marks status
  `canceled`.
- Already-completed tasks cannot be canceled (`cancel` returns `false`).
- Canceled tasks run `on_scheduler_drop` / future `drop` hooks to release
  owned handles.
- Cancellation is cooperative and observable at await boundaries. A canceled
  queued task is removed from the scheduler, so it does not resume user code
  after the pending await that made it cancellable.
- Plain `await Future<T>` still returns `T`; stable status values for
  cancellation-aware operations are carried by dedicated `Result<T, i64>`
  futures such as `timeout_cancel`, not by changing ordinary await types.
- `select` does **not** cancel the losing future. `select_cancel` is the
  consuming loser-canceling variant.

## Timeout (`timeout(future, ms)`)

- Returns `Future<bool>`: `true` if the inner future completed before the
  deadline, `false` if the deadline elapsed first.
- When `false`, the inner future may still be running; callers should not
  assume it was stopped.

## Timeout cancel (`timeout_cancel(future, ms)`)

- Returns `Future<Result<T, i64>>` and **consumes** the inner future.
- When the deadline elapses first, the runtime cancels/drops the inner future
  and the awaited result carries `STATUS_TIMEOUT` (`11`) in the error field.
- When the inner future completes first, the result is `Ok(value)`.

## Select (`select(f0, .., fn)`)

- Accepts **2..8** homogeneous `Future<T>` operands.
- Each blocking `select` call rotates its first-polled operand between internal
  poll rounds; the first ready operand in the current order wins.
- Losing operands are **not** canceled; they are dropped through normal future
  cleanup when their handles go out of scope.

## Select Cancel (`select_cancel(f0, .., fn)`)

- Accepts **2..8** homogeneous `Future<T>` operands and uses the same rotating
  poll order as `select`.
- The first ready operand wins and its value is returned.
- Every losing operand is canceled and dropped before `select_cancel` returns.
  If a loser was also scheduled through `spawn(future)`, the matching
  scheduler task is removed and marked canceled so it cannot resume user code
  after the select returns.
- The existing `select` builtin keeps its non-canceling semantics.

## Reactor

- `CoroutineScheduler` remains cooperative; a reactor layer registers timer, TCP
  readable, and owned-fd interests that feed scheduler wakeup deadlines.
- `sengoo_async_reactor_*` helpers bridge readiness into poll wakeup hints.
- A reactor wait owns its child future after `start`. Result, cancellation, and
  Drop unregister the interest first, then cancel-or-drop that child exactly
  once; callers must not retain or release the transferred child handle.
- Owned-fd readiness is platform-specific in the current supported subset:
  POSIX hosts use `poll(2)` for poll-backed fds; Windows maps CRT fds to OS
  handles and supports disk files plus named/anonymous pipes. Registration owns
  a duplicated descriptor/handle until unregister, preventing numeric handle
  reuse from retargeting a stale interest after the caller closes the original.
  Unsupported hosts or file kinds do not claim readiness support. Actions run
  `29292788788` proves the shared timer/TCP/owned-handle suite on Ubuntu,
  Windows, macOS x64, and macOS arm64 without extending that claim to other
  hosts or file kinds.
- `std::file` exposes owned `AsyncFile` handles. `wait_readable(timeout_ms)`
  registers a clone of the runtime file resource, and result, cancellation, or
  future Drop unregisters it exactly once. `read_into(&mut Buffer)` performs a
  single capacity-bounded read after readiness; it does not promise background
  disk throughput or async writes. Actions run `29298052840` passes the
  AsyncFile runtime ownership/readiness/read/close/cancel/drop suite on Ubuntu,
  Windows, macOS x64, and macOS arm64. Run `29298052830` additionally passes
  the generated-code `sgc` AsyncFile E2E on Ubuntu.

## User `Future` surface

- `tools/stdlib/async_futures.sg` defines `Poll<T>`, opaque `AsyncContext`, and
  the `Future<T>` trait contract for user-defined awaitables.
- Awaiting a user future calls `poll(&mut self, ctx)`. `Poll { is_ready:
  false, .. }` keeps the same future slot alive for the next poll; `Poll {
  is_ready: true, value }` completes the await with `value` and is not polled
  again by that await operation.
- A Pending path must call `ctx.wake()` or `ctx.wake_after(delay_ms)` before it
  yields. `wake()` requests an immediate retry; repeated `wake_after` calls keep
  the earliest non-negative deadline. An evident Pending path without either
  call is rejected as `async::user_future_missing_wakeup`. If a dynamic path
  still reaches Pending without a registration, the runtime uses one bounded
  fallback retry instead of spinning.
- `AsyncContext` is poll-scoped and opaque: user code cannot construct, store,
  return, compare, or capture it into `spawn_blocking_i64` /
  `spawn_blocking_future_i64`.
- The accepted v1 subset covers same-thread cooperative user futures, including
  immediate Ready and Pending-then-Ready native execution. The owning task
  serializes polling, and generated await control flow does not poll after
  Ready. Malformed `Poll<T>` layout, non-`Poll<T>` return, and non-`&mut self`
  receiver errors use the stable `async::user_future_contract` diagnostic
  family; missing wakeup registration has its own stable code in compiler,
  `sgc` JSON, and `sglsp` coverage. Inline user futures remain pinned to their
  owning task and are rejected at runtime-handle `select` and cross-thread
  `spawn_task` boundaries until they have cancellation/drop dispatch.

## Sleep

- `sleep(ms)` is an awaitable timer backed by `SleepFutureState`.
- Poll returns pending until the deadline; result is `()`.

## Cleanup guarantees

| Outcome | Resource cleanup |
|---------|------------------|
| Completed | Task removed from queue; future `drop` runs |
| Canceled | Task removed; `cancel` + `drop` hooks run |
| Timed out (inner still pending) | Timeout future dropped; inner future dropped when last handle released |
| Scheduler dropped | All pending tasks receive `on_scheduler_drop` |
| Normal `TaskScope` exit | All scoped children joined; guard Drop is idempotent |
| Early `TaskScope` exit | Pending children canceled, then every child joined |

## Concurrent runtime (`std::async`, opt-in)

- **Default:** unchanged cooperative single-thread scheduler; programs that do
  not call `runtime_enable_thread_pool` behave as before.
- **Thread pool:** `runtime_enable_thread_pool(n)` with `n >= 1` enables bounded
  worker threads. Invalid counts return `STATUS_INVALID_ARGUMENT`.
- **Blocking offload:** `spawn_blocking_i64(work: fn() -> i64)` returns
  `Err(STATUS_UNSUPPORTED)` until the pool is enabled. Worker threads complete
  host work; the scheduler resumes awaiters. Dropping/canceling the blocking
  future does not kill the host thread; results are discarded.
- **Send:** `spawn_blocking_i64` cross-thread captures must be primitive/unit or
  sendable aggregates. Runtime handles, references, pointers, `AsyncContext`, and
  `Future` values are rejected at compile time.
- **Send/Sync marker surface:** `Send` and `Sync` are compiler-known auto marker
  traits surfaced by `std::async`. Bool, fixed-width integers, pointer-sized
  integers, and f32/f64 satisfy them structurally, so user APIs can spell
  `T: Send` / `T: Sync` bounds
  today. Generic `T: Send` / `T: Sync` bounds structurally inspect tuples,
  arrays, user structs, generic enum payloads, and nested generic arguments.
  Runtime handle fields such as `Buffer` and single-thread shared ownership
  such as `Rc<T>` remain non-send. Source-level `impl !Send` / `impl !Sync`
  declarations are enforced for the single-thread stdlib handle inventory.
  Current cross-thread entry points enforce their boundary: blocking closures
  require `Send`, channel values require `Send`, and shared state requires both
  `Send` and `Sync`.
- **Arc:** descriptor-backed `Arc<T>` provides `arc_new`, `clone_arc`,
  `strong_count`, typed borrowing, and automatic scope-exit `Drop`. Payloads
  move into shared ownership and drop exactly once after the final Arc.
- **Shared counter proof:** `arc_mutex_new_i64` exposes a pinned
  `ArcMutex<i64>` transition type backed by a real runtime
  `Arc<Mutex<i64>>`. `spawn_shared_counter_i64(shared, delta, repetitions)`
  clones the backing Arc into an enabled worker pool; each returned
  `SharedCounterJobI64` has a deterministic `join()` and scope-exit cleanup.
  The shared argument is checked for both `Send` and `Sync`, and a `!Send`
  argument is rejected before lowering. This API is evidence for cross-thread
  ownership and locking, not a substitute for the still-open generic public
  `Arc<Mutex<T>>` surface.
- **Channels:** descriptor-backed `channel<T>(capacity)` provides owned sender
  and receiver endpoints, async `channel_send`, and `channel_recv_into` move-out
  without requiring `T: Default`. Full sends wait with backpressure; close,
  cancellation, and abandoned-value paths preserve exact payload ownership.
- **Mutex:** descriptor-backed `Mutex<T>` and `MutexGuard<T>` provide async FIFO
  acquisition, Copy-only reads, owned replacement, and scope-exit unlock.
  Lock-move and guard-escape checks keep the lock alive for every guard.
- **RwLock:** descriptor-backed `RwLock<T>` provides multiple read guards and
  an exclusive write guard. Async FIFO waiter registration is writer-fair and
  cancellation-safe; guard Drop releases exactly once. Scalar i64 helpers
  remain compatibility wrappers over the same ownership rules.
- **Cleanup wrappers:** public `channel_pair_drop`, `channel_sender_drop`, and
  `mutex_drop` / `rwlock_drop` lower as void cleanup calls in package-shaped
  async programs.
- **Realworld smoke:** `examples/realworld/async-channel-smoke` exercises the
  public `std::async` channel/mutex create, send/receive, lock/unlock helpers
  and cleanup wrappers in a package loop. It does not claim unsupported file
  kinds, background file throughput, async writes, or inline user futures at
  runtime-handle select/cross-thread spawn boundaries.

`select`, `timeout`, and `timeout_cancel` semantics are unchanged when the pool
is enabled. `select_cancel` keeps the same loser-cancellation contract when the
pool is enabled.

## Unsupported (stable `STATUS_UNSUPPORTED` or compile error)

- Full owned-fd readiness polling on all hosts (TCP timer paths are supported)
- Runtime-handle `select` and cross-thread `spawn_task` for inline user futures
- Generic `spawn_blocking<T>` beyond the pinned `i64` worker ABI

Native binaries link `sengoo-runtime` with `native-bridge`; missing optional
async symbols must not cause link failures.
