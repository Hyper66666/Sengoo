## 1. Data-race safety model

- [x] 1.1 Add `Send`/`Sync` marker traits with structural auto-derivation.
  - `std::async` exposes `Send` and `Sync` marker trait names so
    user APIs can spell bounds, and compiler auto-marker rules cover bool,
    fixed-width integers, pointer-sized integers, and f32/f64. Generic `T: Send` /
    `T: Sync` bounds structurally inspect tuples, arrays, user structs, generic
    enum payloads, and nested generic arguments; known non-send runtime handles
    are rejected at any of those positions. Source-level negative marker impls
    now participate in the same structural checks; an explicit owned-handle
    marker policy remains open.
- [x] 1.2 Mark non-thread-safe types (`Rc<T>`, single-thread handles) `!Send`/`!Sync`.
  - `impl !Send for Type {}` / `impl !Sync for Type {}` parse and
    register as compile-time-only marker opt-outs. They are declaration-order
    independent, support generic targets, reject methods/associated items and
    non-marker traits with stable diagnostics, and propagate through nested
    struct/enum auto-marker checks. Rc, FFI/buffer, JSON, process, directory
    walk, DB, Lua, mutex guard, and poll-scoped async context handles carry explicit
    source-level negative marker declarations; an AST inventory regression
    prevents those declarations from silently returning to a Rust-only list.
- [x] 1.3 Require `Send`/`Sync` bounds at thread-spawn and shared-state APIs;
  stable diagnostic on violation.
  - `spawn_blocking_i64` / `spawn_blocking_future_i64` reject non-send captures,
    channel send checks its transferred value, and the shared-counter spawn
    boundary requires both `Send` and `Sync`. Compiler regressions cover
    `AsyncContext`, runtime-handle aggregates, and a direct `Buffer` argument
    with stable `not Send` diagnostics. Future generic thread APIs must preserve
    these bounds when added.
- [x] 1.4 Negative tests: a `!Send` value cannot cross a thread boundary.
  - Compiler tests cover non-send handle aggregate and `AsyncContext`
    captures crossing `spawn_blocking`, plus structural generic bounds rejecting
    user structs and enum variants with non-send runtime-handle fields, plus
    stdlib `Rc<T>` values. Source-level `!Send` tests now also cover direct,
    generic, nested struct/enum, and captured `spawn_blocking` values.

## 2. Shared ownership and mutation

- [x] 2.1 `Arc<T>` atomic-refcounted shared ownership.
  - `std::async` exposes descriptor-backed `Arc<T>` with `arc_new`,
    `clone_arc`, `strong_count`, typed borrowing, and automatic scope-exit
    `Drop`. Compiler tests cover arbitrary Copy payloads plus Send/Sync bounds,
    runtime tests prove exact generic payload move/drop, and native `sgc`
    tests exercise public `Arc<Mutex<i64>>` composition across worker threads.
- [ ] 2.2 `Mutex<T>` / `RwLock<T>` with RAII guards that release on `Drop`.
  - Partial: `std::async` now exposes descriptor-backed `Mutex<T>` and
    `MutexGuard<T>` with async acquisition, Copy-only reads, owned replacement,
    and automatic scope-exit unlock. Runtime and native tests cover arbitrary
    payload Drop, fresh lock acquisition, failed-lock cleanup, duplicate unlock
    rejection, and public `Arc<Mutex<i64>>` worker composition. The scalar
    transition surface still provides `RwLockI64` read/write guards and handoff
    coverage. Generic `RwLock<T>`, async rwlock waiting, and compiler-enforced
    lock-outlives-guard lifetimes remain open.
- [x] 2.3 Tests: shared counter across threads via `Arc<Mutex<...>>`.
  - `runtime/src/async_runtime.rs::concurrent_shared_counter_joins_workers_deterministically`
    submits eight jobs to four workers against a real `Arc<Mutex<i64>>` payload
    and joins every job before asserting `42`. The native Sengoo test
    `tools/sgc/src/tests.rs::async_stdlib_generic_arc_mutex_joins_cross_thread_workers`
    proves public `Arc<Mutex<i64>>` composition follows the same path, and the
    compiler suite covers `Arc<Mutex<Payload>>` with a user-defined payload.

## 3. Multi-threaded executor

- [ ] 3.1 Add a multi-threaded executor selectable at startup; keep cooperative
  default. Correctness, bounded queues/backpressure, shutdown, cancellation,
  and error isolation are required; work stealing is optional.
  - Partial: the existing native thread pool is opt-in via
    `runtime_enable_thread_pool`; the cooperative scheduler remains default.
    Generic future scheduling, bounded backpressure, shutdown/cancellation, and
    error isolation remain open. Work stealing does not block archive.
- [ ] 3.2 `spawn` requires `Send` futures on the multi-threaded executor.
- [ ] 3.3 Tests: parallel tasks complete and results join deterministically.
  - Partial: native tests cover `spawn_blocking` completion and deterministic
    joined shared-state jobs through the opt-in pool. Deterministic joined
    `spawn` futures on the bounded multi-threaded executor remain open.

## 4. Cross-platform reactor

- [ ] 4.1 Reactor abstraction with Linux, Windows, and supported macOS backends
  for timer/socket/owned-handle readiness.
- [ ] 4.2 Wire stdlib async net/file helpers to the reactor.
- [ ] 4.3 Reference-host tests closing the owned-handle readiness deferral for
  Windows, Linux, and the supported macOS release channel, including no-busy-
  poll, cancellation, timeout, and close behavior.

## 5. Future trait, channels, structured concurrency

- [ ] 5.1 Generalize user futures to a `Future` trait with a documented `poll`
  contract; relax `select`/user-future restrictions where sound, keeping
  negative tests.
  - Partial: `tools/stdlib/async_futures.sg` defines `Poll<T>`,
    `AsyncContext`, and `Future<T>::poll`, with compiler and native tests for
    ready/pending user futures plus rejected Poll/receiver shapes.
- [ ] 5.2 `channel<T>()` mpsc with async-aware send/recv.
  - Partial: `std::async` exposes bounded i64 channels with async send/recv
    outcomes and realworld smoke coverage. Generic `channel<T>` remains open.
- [ ] 5.3 `task_scope` structured-concurrency helper (join/cancel children on
  scope exit).
- [ ] 5.4 Tests for channels, scoped tasks, and cancellation boundaries.

## 6. Docs and matrix

- [ ] 6.1 Update `docs/runtime-async-semantics.md` and
  `examples/realworld/SUPPORT_MATRIX.md` (reactor + Send/Sync rows).
  - Partial: docs and the realworld support matrix now record the current
    `Send`/`Sync` marker surface, scalar impls, known `spawn_blocking_i64`
    non-send diagnostics, structural generic bounds for user structs, and
    source-level negative impl semantics and explicit stdlib single-thread
    handle declarations. Current blocking, channel-transfer, and shared-state
    boundaries are enforced; future thread APIs must preserve those bounds.
    Reactor rows pre-existed for the current supported subset; all-host
    owned-fd readiness remains open.
- [x] 6.2 Run `openspec validate concurrency-safety-and-async-io --strict`.

## Verification

- `cargo test -p sengoo-runtime --lib --features native-bridge`
- `cargo test -p sgc async_native_runtime -- --test-threads=1`
- New multi-thread + reactor tests on the reference host
