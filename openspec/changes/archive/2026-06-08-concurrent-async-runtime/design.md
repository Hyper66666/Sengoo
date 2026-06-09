## Architecture

```text
CoroutineScheduler (default single-thread)
        -> opt-in
ThreadPool + cross-thread wakeups
        -> Reactor interests (unchanged contract)
```

## API surface (stdlib)

- Module: `std::async`.
- `runtime_enable_thread_pool(worker_count: i64) -> Result<i64, i64>` returns
  `Ok(1)` after enabling or confirming an already-enabled compatible pool.
- `spawn_blocking_i64(work: fn() -> i64) -> Result<Future<i64>, i64>` runs `work`
  on a pool thread and resumes the awaiting task on the scheduler thread.
- `channel_bounded<T>(cap) -> (Sender<T>, Receiver<T>)`
- `Mutex<T>`: runtime mutex handle with async-compatible `lock_async()` wrapper.
- Generic `spawn_blocking<T>` and blocking closures returning owned handles are
  non-goals for v1 unless a follow-up spec pins their ABI.

## Safety

- Compiler rejects `spawn_blocking_i64` captures that violate `Send` when
  cross-thread.
- V1 `Send` whitelist: primitive scalars, unit, and aggregates composed only of
  sendable values. Runtime handles (`Buffer`, JSON/process/file/dir handles,
  async `Future`/task handles, `AsyncContext`, raw pointers, references, and FFI
  library/symbol handles) are non-`Send` unless a later spec explicitly opts them
  in.
- Default programs without `runtime_enable_thread_pool` behave identically to
  today.
- `runtime_enable_thread_pool(n)` rejects `n < 1` and implementation-defined
  excessive counts with `STATUS_INVALID_ARGUMENT`.
- Calling `spawn_blocking_i64` before the pool is enabled returns
  `Err(STATUS_UNSUPPORTED)`.
- Canceling or dropping a blocking future does not forcibly stop an
  already-running host thread; the result is discarded when the work completes.

## Verification

- `cargo test -p sengoo-runtime --features native-bridge concurrent`
- `cargo test -p sengoo-compiler concurrent_async`
- `cargo test -p sgc async_native_runtime -- --test-threads=1` single-thread
  regression suite still passes
