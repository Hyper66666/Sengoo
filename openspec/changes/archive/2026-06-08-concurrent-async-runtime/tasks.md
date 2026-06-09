## 1. Preparation

- [x] 1.1 Run `openspec validate concurrent-async-runtime --strict`.

## 2. Implementation

- [x] 2.1 Thread pool and cross-thread wakeup bridge.
- [x] 2.2 `std::async::runtime_enable_thread_pool` and `spawn_blocking_i64` lowering/runtime queue, including disabled/invalid-pool errors.
- [x] 2.3 `channel_bounded` and `Mutex` stdlib wrappers, including close/drop wakeup semantics.
- [x] 2.4 Compiler `Send` diagnostics for cross-thread captures, including non-Send runtime handles and `AsyncContext`.

## 3. Verification

- [x] 3.1 `cargo test -p sengoo-runtime --features native-bridge concurrent`
- [x] 3.2 `cargo test -p sengoo-compiler concurrent`
- [x] 3.3 `cargo test -p sgc async_native_runtime -- --test-threads=1`
- [x] 3.4 Update `docs/runtime-async-semantics.md` and SUPPORT_MATRIX
- [x] 3.5 Native e2e covers enabled thread pool, `spawn_blocking_i64`, non-Send capture rejection, channel full pending, channel close wakeup, mutex contention, and canceled blocking future cleanup.

## Archive Gate

- [x] `openspec validate concurrent-async-runtime --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] Single-thread default verified; concurrent path covered by tests.
