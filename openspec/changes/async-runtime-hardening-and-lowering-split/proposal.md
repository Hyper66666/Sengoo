# Async Runtime Hardening and Lowering Split

## Why

The async pipeline is broadly functional and heavily tested, but a few seams are
still too fragile:

- `lowering.rs` still contains at least one user-reachable ICE path through
  `current_block.expect(...)`.
- `runtime.c` hardens async frame `load/store`, but `frame_free` still does not
  follow the same debug/release contract.
- `async_lowering.rs` has already been split substantially; the remaining work
  is to finish error-propagation cleanup and move structural pressure toward
  `lowering.rs`.

## What Changes

This change tracks:

1. Recursive future-escape detection through wrapper/container type shapes.
2. Stable ordinal-based async dispatch IDs instead of hashed IDs.
3. Converting async-lowering ICE paths into structured diagnostics.
4. Hardening runtime frame contracts, including `frame_free`.
5. Splitting `async_lowering.rs` and then continuing first-cut decomposition of
   `lowering.rs`.

## Current Status

Already completed on `main`:

- Recursive future-escape checks for `Ref/Ptr/Fn`
- Stable ordinal-based async dispatch registry
- Async-lowering hot-path `panic!/expect` cleanup for CFG/remap/result-dispatch
- Debug/release contract for async frame `load/store`
- `async_lowering.rs` submodules for:
  - cfg planning
  - frame layout
  - poll synthesis
  - entry synthesis
  - dispatch synthesis

Still open:

- `lowering.rs current_block.expect(...)` -> non-ICE diagnostic path
- `runtime.c sengoo_async_frame_free(...)` debug/release contract alignment
- `lowering.rs` first-cut modular split for builtin/planning-heavy logic

## Scope Guard

This change does **not**:

- redesign the public async syntax,
- merge the Rust and C runtimes,
- type-systemize task handles/status in this batch,
- or refactor every `expect()` in general MIR lowering.
