## Why

Sengoo async today is cooperative and single-threaded. Mainstream server runtimes
(Go, Tokio, Node worker threads) expose multi-core execution without abandoning
async/await ergonomics.

## What Changes

- Opt-in thread pool for `std::async::spawn_blocking_i64` and CPU-bound work,
  with later generic expansion gated by follow-up spec.
- `Send` bounds on cross-thread captures with a conservative v1 whitelist.
- Bounded channels and mutex primitives integrated with the existing scheduler.
- Document single-thread default; concurrent mode explicit at runtime init.

## Capabilities

### New Capabilities

- `concurrent-async-runtime`: thread pool, blocking offload, sync primitives.

### Modified Capabilities

- None in canonical `openspec/specs/` today.

## Impact

- `runtime/src/async_runtime/`, `tools/stdlib/async*.sg`, compiler send-check hooks
- Parent umbrella: `mainstream-production-readiness` Block 2
