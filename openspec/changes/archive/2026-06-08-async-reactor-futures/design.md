## Scope

Child change for `six-pillar-gap-closure` Pillar 2. Semantic contracts are frozen
in `specs/async-reactor-futures/spec.md`.

## Architecture

```text
CoroutineScheduler -> Reactor (timers/sockets/fds) -> host backend
```

## Includes

- `Poll<T>`, `Future<T>` with an exclusive borrowed `&mut self` poll receiver,
  and opaque `AsyncContext`
- Timer, TCP socket, and supported owned-fd readiness
- Variadic `select` 2..8 with rotating poll order; losers not canceled
- `timeout` non-canceling; `timeout_cancel` returns `STATUS_TIMEOUT` after cleanup
- Removal of obsolete phase-only async frame restrictions with regression tests
- Windows native dispatch link regression on reference CI host

## Verification

- `cargo test -p sengoo-compiler async`
- `cargo test -p sengoo-runtime --features native-bridge async`
- `cargo test -p sgc async_native_runtime` on Windows reference CI host
