## Why

`compiler/src/mir/lowering.rs` is now one of the largest remaining compiler source files at roughly 1.5k LoC. It mixes the public MIR-lowering entry points, lowering options, function orchestration, `LoweringContext` state, contract lowering, body/statement dispatch, literal/operator helpers, and test-only coverage in a single module root while already coordinating two dozen child helper modules under `compiler/src/mir/lowering/`.

After the archived `large-file-splits-runtime-db` and `large-file-splits-jit-codegen` changes, this is the next roadmap P0 split target and a good place to reuse the established SOP without changing lowering semantics.

## What Changes

- Convert `compiler/src/mir/lowering.rs` into the directory module root `compiler/src/mir/lowering/mod.rs`, preserving the existing logical module path `crate::mir::lowering`.
- Extract focused concerns from the current module root into sibling files under `compiler/src/mir/lowering/`, while preserving existing child helper modules and their call sites.
- Preserve the public compiler API exposed through `compiler/src/mir/mod.rs` and `compiler/src/lib.rs`, especially `lower_hir`, `lower_hir_with_options`, and `MirLowerOptions`.
- Preserve internal helper compatibility for existing lowering child modules that currently rely on `use super::*`, including `LoweringContext`, `FunctionSig`, and helper methods, with no wider visibility than needed.
- Keep the change as a structural refactor only: no MIR semantic changes, no diagnostics rewrites, no lowering behavior changes, and no test assertion rewrites.
- Update roadmap/OpenSpec completion notes after implementation and archive the change when the verification baseline is green.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

- `large-file-splits`: Add an explicit requirement for splitting a large module root that already owns a child helper directory, preserving child module paths and parent-private helper access with minimal visibility widening.

## Impact

- Affected code:
  - `compiler/src/mir/lowering.rs`
  - `compiler/src/mir/lowering/*.rs`
  - `compiler/src/mir/mod.rs` only if imports/re-exports need mechanical path preservation adjustments
  - `docs/plans/2026-05-18-next-priorities.md`
- Public APIs to preserve:
  - `sengoo_compiler::lower_hir`
  - `sengoo_compiler::lower_hir_with_options`
  - `sengoo_compiler::MirLowerOptions`
  - `sengoo_compiler::mir::{lower_hir, lower_hir_with_options, MirLowerOptions}`
- Internal APIs to preserve for child modules/tests:
  - `LoweringContext`
  - `FunctionSig`
  - `LoopContext`
  - `LambdaEnv`
  - helper methods currently used by `compiler/src/mir/lowering/*.rs`
- Dependencies: none.
- Runtime/compiler behavior: no intended observable behavior change.
