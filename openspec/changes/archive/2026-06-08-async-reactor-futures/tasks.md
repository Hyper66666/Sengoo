## 1. Preparation

- [x] 1.1 Run `openspec validate async-reactor-futures --strict`.
- [x] 1.2 Run `openspec validate --all --strict`.

## 2. Reactor and futures

- [x] 2.1 Implement reactor with timer, TCP, and supported owned-fd readiness.
- [x] 2.2 Implement `Poll<T>`, `Future<T>::poll(&mut self, ctx)`, and opaque poll-scoped `AsyncContext`.
- [x] 2.3 Relax sound future-flow rules with negative tests.
- [x] 2.4 Remove obsolete phase-only async restrictions with one regression test each.
- [x] 2.5 Add negative tests for concurrent/reentrant polling, polling after `Ready`, and storing `AsyncContext`.

## 3. Select and timeout

- [x] 3.1 Implement variadic `select` 2..8 with rotating poll order.
- [x] 3.2 Preserve non-canceling `timeout`; add `timeout_cancel` returning `STATUS_TIMEOUT`.

## 4. Verification

- [x] 4.1 `cargo test -p sengoo-compiler async`
- [x] 4.2 `cargo test -p sengoo-runtime --lib --features native-bridge async`
- [x] 4.3 `cargo test -p sgc async_native_runtime -- --nocapture --test-threads=1` passes on the reference Windows host (30/30).
- [x] 4.4 Update `docs/runtime-async-semantics.md` and SUPPORT_MATRIX async rows

## Archive Gate

- [x] `openspec validate async-reactor-futures --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] Verification commands in section 4 pass on the reference Windows host; runtime tests cover timer and owned-fd readiness, compiler tests cover user Future lowering and negative poll rules, and sgc native tests cover select and timeout behavior.
