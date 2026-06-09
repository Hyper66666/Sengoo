## Why

`async-reactor-futures`, `concurrent-async-runtime`, and
`runtime-hardening-ffi-async` delivered a useful async subset: timers, select,
timeout helpers, platform-specific owned-fd readiness, opt-in thread pool,
channels, mutexes, and cleanup semantics. The realworld
`async-channel-smoke` fixture proves package-shaped public `std::async` usage,
but the support matrix still has intentional default-readiness gaps.

This change owns those remaining async/default follow-ups so
`mainstream-default-readiness` can point at a concrete child instead of a vague
future lane. The implemented result is a documented supported subset plus
explicitly deferred edges, not a claim that every mainstream async semantic is
complete.

## What Changes

- Promote same-thread cooperative user-defined `Future::poll` lowering to a
  supported subset, with broader lifecycle and wakeup diagnostics explicitly
  deferred.
- Own all-host owned-fd readiness policy beyond the current Unix/Windows subset.
- Own user-facing cancellation boundaries and select loser cancellation policy.
- Own public `std::async` void cleanup wrapper lowering in package-shaped async
  programs.
- Require realworld smoke coverage and `sglsp` diagnostic parity before any
  supported-subset claim changes.

## Impact

- Affected areas: `compiler/`, `runtime/`, `tools/stdlib/async.sg`,
  `tools/sgc`, `tools/sglsp`, `examples/realworld`, and
  `docs/runtime-async-semantics.md`.
- This is a follow-up ownership change. It updates supported-subset claims only
  where compiler/native tests, docs, and support-matrix wording already agree.

## Non-Goals

- No public async executor rewrite.
- No source-incompatible async syntax cleanup.
- No promise of cross-thread generic `spawn_blocking<T>` in this change.
- No promotion of deferred matrix rows without tests and docs.
