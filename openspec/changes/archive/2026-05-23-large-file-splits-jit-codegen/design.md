# Design: large-file-splits-jit-codegen

## Context

`compiler/src/codegen/jit.rs` is the next roadmap P0 Large File Splits target after the archived `large-file-splits-runtime-db` starter. It is 1363 LoC and already participates in the `compiler/src/codegen/jit/` subdirectory because `jit.rs` declares `mod declaration_helpers;`, which currently resolves to `compiler/src/codegen/jit/declaration_helpers.rs` (154 LoC).

The shape differs from `runtime_db`: almost all logic lives in one large inherent `impl JITCodegen` block rather than in free functions. This change therefore applies the proven SOP while extending it to multiple sibling files that each contribute an `impl JITCodegen { ... }` block.

Current public surface:

- `pub struct JITCodegen` with private fields `ir`, `indent`, `extern_decls`, `strings`, `string_counter`, `current_block_id`, `function_signatures`.
- Public inherent methods: `new`, `generate`, `add_string`, `to_string`.
- `impl Default for JITCodegen`.
- Re-export paths preserved by existing callers:
  - `compiler/src/codegen/mod.rs`: `pub mod jit; pub use jit::JITCodegen;`
  - `compiler/src/lib.rs`: `pub use codegen::{jit::JITCodegen, Codegen};`

Current method inventory in `jit.rs`:

| Lines | Method | Planned home |
|---:|---|---|
| 32-46 | `pub fn new` | `mod.rs` |
| 47-55 | `emit_header` | `mod.rs` |
| 56-85 | `pub fn generate` | `mod.rs` |
| 86-103 | `emit_string_constants` | `mod.rs` |
| 104-111 | `pub fn add_string` | `mod.rs` |
| 112-149 | `codegen_function` | `functions.rs` |
| 150-203 | `codegen_basic_block` | `functions.rs` |
| 204-1085 | `codegen_instruction` | `instructions.rs` |
| 1086-1205 | `codegen_terminator` | `terminators.rs` |
| 1206-1212 | `binary_op_to_llvm` | `ops.rs` |
| 1213-1293 | `emit_cast_value` | `casts.rs` |
| 1294-1306 | `emit_main_wrapper` | `functions.rs` |
| 1307-1353 | local/type/indent helpers | `utils.rs` |
| 1354-1359 | `pub fn to_string` | `mod.rs` |
| 1359-1363 | `impl Default` | `mod.rs` |

## Goals / Non-Goals

**Goals:**

1. Convert `compiler/src/codegen/jit.rs` to a directory module at `compiler/src/codegen/jit/mod.rs` without changing public paths.
2. Split the large `impl JITCodegen` into focused sibling impl blocks.
3. Preserve generated LLVM IR text behavior and existing test assertions exactly.
4. Keep every resulting non-test file below the original 1363 LoC and below the roadmap ~1000 LoC target.
5. Update the Large File Splits SOP with an impl-block-specific rule: cross-file inherent methods become `pub(super)`, while methods used only inside one submodule remain private.

**Non-Goals:**

- No LLVM IR cleanup, optimization, formatting normalization, or semantic fixes.
- No changes to MIR lowering, MIR types, `common.rs`, or non-JIT codegen helpers.
- No changes to `JITCodegen` public fields (none exist) or public method signatures.
- No new dependency and no inkwell integration change.
- No test assertion rewrites to accommodate new output.

## Decisions

### Decision 1: Keep `JITCodegen` and public methods in `mod.rs`

`mod.rs` remains the reader-facing entry point for the type and public API:

```text
compiler/src/codegen/jit/
  mod.rs                 (~180 LoC: struct, public methods, module wiring, Default)
  declaration_helpers.rs (existing ~154 LoC)
  utils.rs               (~60 LoC)
  ops.rs                 (~20 LoC)
  casts.rs               (~90 LoC)
  terminators.rs         (~130 LoC)
  functions.rs           (~110 LoC)
  instructions.rs        (~900 LoC)
```

Rationale: preserving the public type definition and public methods in the root makes `pub mod jit; pub use jit::JITCodegen;` behavior obvious and keeps external consumers insulated from the split.

Alternative considered: move `JITCodegen` to `state.rs` or `core.rs` and re-export it from `mod.rs`. Rejected because the public type itself is the module's primary API and moving it would force every sibling impl file to import through an extra layer without reducing risk.

### Decision 2: Split by inherent impl blocks, not traits or free functions

Each submodule will contain `use super::JITCodegen;` and an `impl JITCodegen { ... }` block. Cross-submodule helper methods become `pub(super)` only where required.

Rationale: this is the least invasive refactor. It avoids introducing traits, wrappers, borrowed context structs, or free-function adapters that could change method resolution or make generated IR state mutation harder to audit.

Alternative considered: extract free functions that receive `&mut JITCodegen`. Rejected because it would be more disruptive than moving methods and would obscure the current stateful emitter style.

### Decision 3: Extract `instructions.rs` as one large but bounded slice

`codegen_instruction` is ~880 LoC and contains one single `match mir::Instruction`. The first split will move it as a unit to `instructions.rs` rather than splitting match arms into many helper files.

Rationale: splitting the match arms would be a semantic refactor, not a simple module split. Moving the function intact keeps the first jit split behavior-preserving and still brings the largest file below 1000 LoC.

Alternative considered: split instruction arms into `memory_ops.rs`, `aggregate_ops.rs`, `calls.rs`, and `arith.rs` immediately. Rejected for this change; it can be a follow-up after the module boundary is stable.

### Decision 4: Slice order follows dependency direction

Slice order is smallest-to-largest, but adjusted for impl-block dependencies:

1. Slice 0: byte-identical `git mv jit.rs -> jit/mod.rs`.
2. Slice 1: extract `utils.rs` (local/type/indent helpers).
3. Slice 2: extract `ops.rs` and `casts.rs` in separate commits/slices.
4. Slice 3: extract `terminators.rs`.
5. Slice 4: extract `functions.rs`.
6. Slice 5: extract `instructions.rs` last.
7. Slice 6: docs/roadmap/tasks completion and final validation.

Rationale: early slices establish impl-block visibility and import patterns while moving small helpers first. `instructions.rs` lands last because it has the broadest call surface and highest merge-conflict risk.

### Decision 5: No doc sidecar required for jit unless one already exists

Unlike `runtime_db.md`, no dedicated `jit.md` sidecar exists today. The final documentation update will therefore be limited to the OpenSpec tasks/design and the roadmap file.

Rationale: creating a new sidecar just to document a structural split would add maintenance surface. If a future codegen architecture doc exists, it can reference the layout then.

## Risks / Trade-offs

- **Rust privacy mistakes across sibling impl files** -> Mitigate by using `pub(super)` only for methods called from a different `jit/` submodule, and by running `cargo test -p sengoo-compiler --lib` after every slice.
- **Import path mistakes after moving from `jit.rs` to `jit/mod.rs`** -> Mitigate in Slice 0 with a byte-identical rename first; `use super::common;` should still resolve because `super` remains `compiler::codegen` from `jit/mod.rs`.
- **`instructions.rs` remains large (~900 LoC)** -> Accept for this change because it is still below the roadmap ~1000 LoC target and well below the original 1363 LoC. Further match-arm decomposition is explicitly deferred.
- **Generated IR text drift** -> Mitigate by moving methods without internal edits and preserving existing compiler tests, especially `cast_semantics_tests` JIT assertions.
- **Warnings from temporarily unused imports** -> Mitigate with per-slice warning-free verification and import pruning in the same slice.

## Migration Plan

1. Create this OpenSpec change and validate it strictly.
2. Implement slices one at a time, each committed independently.
3. Run the full verification baseline after every slice:
   - `cargo test -p sengoo-compiler --lib`
   - `cargo test -p sgc`
   - `cargo test -p sengoo-runtime --lib`
   - `cargo test -p sgpm`
4. In the final slice, run the targeted compiler smoke:
   - `cargo test -p sengoo-compiler cast_semantics --lib`
5. Update `docs/plans/2026-05-18-next-priorities.md` with completion status and the next Large File Splits target.
6. Archive the change and promote the updated `large-file-splits` spec delta.

Rollback strategy: because each slice is a pure structural commit, rollback is a normal `git revert` of the most recent slice commit. Slice 0 can be reverted independently if the directory-module rename fails.

## Open Questions

- Should a follow-up change split `instructions.rs` by MIR instruction categories after this structural split lands? Tentative answer: yes, but only after the directory module is stable and tests remain green.
- Should this change add a dedicated `compiler/src/codegen/jit.md` sidecar? Tentative answer: no; roadmap and OpenSpec archive are sufficient for this structural refactor.
