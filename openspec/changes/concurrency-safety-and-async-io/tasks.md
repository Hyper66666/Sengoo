## 1. Data-race safety model

- [ ] 1.1 Add `Send`/`Sync` marker traits with structural auto-derivation.
- [ ] 1.2 Mark non-thread-safe types (`Rc<T>`, single-thread handles) `!Send`/`!Sync`.
- [ ] 1.3 Require `Send`/`Sync` bounds at thread-spawn and shared-state APIs;
  stable diagnostic on violation.
- [ ] 1.4 Negative tests: a `!Send` value cannot cross a thread boundary.

## 2. Shared ownership and mutation

- [ ] 2.1 `Arc<T>` atomic-refcounted shared ownership.
- [ ] 2.2 `Mutex<T>` / `RwLock<T>` with RAII guards that release on `Drop`.
- [ ] 2.3 Tests: shared counter across threads via `Arc<Mutex<...>>`.

## 3. Multi-threaded executor

- [ ] 3.1 Add a work-stealing executor selectable at startup; keep cooperative
  default.
- [ ] 3.2 `spawn` requires `Send` futures on the multi-threaded executor.
- [ ] 3.3 Tests: parallel tasks complete and results join deterministically.

## 4. Cross-platform reactor

- [ ] 4.1 Reactor abstraction with Linux (epoll/poll) and Windows (IOCP/handle)
  backends for timer/socket/owned-fd readiness.
- [ ] 4.2 Wire stdlib async net/file helpers to the reactor.
- [ ] 4.3 Reference-host tests closing the "owned-fd all-host readiness" deferral
  for Linux + Windows; document macOS as the remaining channel.

## 5. Future trait, channels, structured concurrency

- [ ] 5.1 Generalize user futures to a `Future` trait with a documented `poll`
  contract; relax `select`/user-future restrictions where sound, keeping
  negative tests.
- [ ] 5.2 `channel<T>()` mpsc with async-aware send/recv.
- [ ] 5.3 `task_scope` structured-concurrency helper (join/cancel children on
  scope exit).
- [ ] 5.4 Tests for channels, scoped tasks, and cancellation boundaries.

## 6. Docs and matrix

- [ ] 6.1 Update `docs/runtime-async-semantics.md` and
  `examples/realworld/SUPPORT_MATRIX.md` (reactor + Send/Sync rows).
- [ ] 6.2 Run `openspec validate concurrency-safety-and-async-io --strict`.

## Verification

- `cargo test -p sengoo-runtime --lib --features native-bridge`
- `cargo test -p sgc async_native_runtime -- --test-threads=1`
- New multi-thread + reactor tests on the reference host
