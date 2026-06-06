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
- `select` does **not** cancel the losing future.

## Timeout (`timeout(future, ms)`)

- Returns `Future<bool>`: `true` if the inner future completed before the
  deadline, `false` if the deadline elapsed first.
- When `false`, the inner future may still be running; callers should not
  assume it was stopped.
- Stdlib/async builtins surface timeout waits through the native bridge; they do
  not currently map to `STATUS_TIMEOUT` at the language layer.

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

## Unsupported (stable `STATUS_UNSUPPORTED` or compile error)

- IO/async networking wakeups
- `select` with more than two operands
- User-defined `Future` trait implementations
- General timer wheel beyond `sleep` / `timeout`

Native binaries link `sengoo-runtime` with `native-bridge`; missing optional
async symbols must not cause link failures.
