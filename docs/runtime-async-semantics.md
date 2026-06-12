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
- Owned-fd readiness is platform-specific in the current supported subset:
  POSIX hosts use `poll(2)` for poll-backed fds; Windows maps CRT fds to OS
  handles and supports disk files plus named/anonymous pipes. Unsupported hosts
  or file kinds do not claim readiness support; all-host owned-fd readiness
  remains Deferred.

## User `Future` surface

- `tools/stdlib/async_futures.sg` defines `Poll<T>`, opaque `AsyncContext`, and
  the `Future<T>` trait contract for user-defined awaitables.
- Awaiting a user future calls `poll(&mut self, ctx)`. `Poll { is_ready:
  false, .. }` keeps the same future slot alive for the next poll; `Poll {
  is_ready: true, value }` completes the await with `value` and is not polled
  again by that await operation.
- `AsyncContext` is poll-scoped and opaque: user code cannot construct, store,
  return, compare, or capture it into `spawn_blocking_i64` /
  `spawn_blocking_future_i64`.
- The accepted v1 subset covers same-thread cooperative user futures, including
  immediate Ready and Pending-then-Ready native execution. Reentrant/concurrent
  poll and poll-after-Ready user-source diagnostics remain follow-up work.
  Malformed `Poll<T>` layout, non-`Poll<T>` return, and non-`&mut self`
  receiver errors use the stable `async::user_future_contract` diagnostic family
  in compiler, `sgc` JSON, and `sglsp` representative coverage; exhaustive
  snapshots for every rejected user-future shape remain follow-up work.

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
- **Channels:** `channel_bounded(cap)` provides async `channel_send_i64` /
  `channel_recv_i64`. A full channel leaves send pending; runtime-level close
  and drop paths wake receivers with `STATUS_INVALID_HANDLE`.
- **Mutex:** `mutex_new_i64` + `mutex_lock_async` polls pending under contention;
  runtime-level close/drop paths wake waiters with `STATUS_INVALID_HANDLE`.
- **Cleanup wrappers:** public `channel_pair_drop`, `channel_sender_drop`, and
  `mutex_drop` lower as void cleanup calls in package-shaped async programs.
- **Realworld smoke:** `examples/realworld/async-channel-smoke` exercises the
  public `std::async` channel/mutex create, send/receive, lock/unlock helpers
  and cleanup wrappers in a package loop. It does not claim all-host owned-fd
  readiness, complete user-future rejected-shape diagnostics, or cancellation
  semantics beyond the documented task/status APIs.

`select`, `timeout`, and `timeout_cancel` semantics are unchanged when the pool
is enabled. `select_cancel` keeps the same loser-cancellation contract when the
pool is enabled.

## Unsupported (stable `STATUS_UNSUPPORTED` or compile error)

- Full owned-fd readiness polling on all hosts (TCP timer paths are supported)
- Concurrent/reentrant poll of the same user future, source-level poll after
  `Ready`, and rejected-shape `sgc` JSON / `sglsp` snapshots for every user
  future diagnostic
- Generic `spawn_blocking<T>` beyond the pinned `i64` worker ABI

Native binaries link `sengoo-runtime` with `native-bridge`; missing optional
async symbols must not cause link failures.
