## Why

Sengoo async today is a cooperative sleep/spawn/2-select subset without IO
wakeups, user-defined futures, or mainstream future flow. This child change owns
the new canonical `async-reactor-futures` capability for Pillar 2.

## What Changes

- Reactor-backed timer/socket/fd wakeups.
- `Poll<T>`, `Future<T>`, opaque `AsyncContext`.
- Variadic `select` 2..8; losers not canceled.
- `timeout_cancel` returning `STATUS_TIMEOUT`; preserve existing `timeout`.
- Remove obsolete phase-only async restrictions with regression tests.
- Windows native dispatch link regression on reference CI host.

## Capabilities

### New Capabilities

- `async-reactor-futures`: reactor, trait futures, N-select, timeout_cancel, async
  restriction cleanup.

### Modified Capabilities

- None in canonical `openspec/specs/` today.

## Impact

- `runtime/src/async_runtime/`, `compiler/src/mir/async_*`, native async tests
- Parent umbrella: `six-pillar-gap-closure` Pillar 2
