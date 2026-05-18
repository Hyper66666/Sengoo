# Proposal: Async Phase-2 Shipped Surface Alignment

## Status

Completed on `main`, documented here on 2026-04-23.

## Summary

This change no longer proposes new async Phase-2 functionality from scratch.
Its purpose is to document the Phase-2 surface that is already shipped and
covered in the repository:

- `async { ... }` blocks
- `sleep(...)` and `timeout(...)`
- `spawn(...)`, `spawn_task(...)`, `cancel_task(...)`, and `task_status(...)`
- `join(...)`
- the current `select(...)` surface

## Evidence

Compiler-side evidence already exists in `compiler/src/tests/async_tests.rs`,
including:

- async block parsing and lowering
- sleep / timeout builtins
- spawn / spawn_task / cancel_task / task_status builtins
- join and select lowering
- current select type-boundary diagnostics

Native/runtime evidence already exists in `tools/sgc/src/tests.rs`, including:

- async block native execution
- timer-related async runtime tests for `sleep` and `timeout`
- spawn / spawn_task / cancel_task / task_status execution tests
- join/select execution tests for the currently supported shapes

Runtime-side scheduler/task support already exists in `runtime/src/async_runtime.rs`.

## Current Boundary

This change records shipped behavior, not a perfect or final async model.
The remaining boundaries still matter:

- `select(...)` is still not the final generalized surface for every future result shape
- cyclic async CFG support for loop-heavy `await` bodies is still incomplete
- richer async frame support such as payload-carrying enums across `await` remains unfinished
- no full reactor / non-blocking IO layer is claimed here

## Why This Change Exists

The previous OpenSpec state for `async-phase-2-features` was misleading because
it declared completion without proposal/tasks/spec artifacts. This change makes
that completed status auditable.
