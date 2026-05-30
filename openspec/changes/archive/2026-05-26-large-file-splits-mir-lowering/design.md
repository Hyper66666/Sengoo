# Design: large-file-splits-mir-lowering

## Context

`compiler/src/mir/lowering.rs` is the next roadmap P0 Large File Splits target after the archived JIT split. It is roughly 1.5k LoC and already owns a child helper directory, `compiler/src/mir/lowering/`, with 24 focused helper files. The existing module shape is therefore different from both prior splits: there is already a helper directory, but the root file still contains the public entry points, lowering options, function orchestration, the `LoweringContext` state type, a large `impl LoweringContext`, and root-local tests.

Current public surface preserved by `compiler/src/mir/mod.rs` and `compiler/src/lib.rs`:

- `pub fn lower_hir(items: &[HIRItem]) -> Result<Vec<MirFunction>, String>`.
- `pub fn lower_hir_with_options(items: &[HIRItem], options: MirLowerOptions) -> Result<Vec<MirFunction>, String>`.
- `pub struct MirLowerOptions` with public fields `runtime_contract_checks`, `lazy_generic_mono`, and `async_functions`.
- `impl Default for MirLowerOptions`.
- Public inherent methods `MirLowerOptions::new` and `MirLowerOptions::with_async_functions`.

Current internal surface used by child helper modules and tests:

- `LoweringContext<'a>` and its fields/methods, accessed by many child files through `use super::*`.
- `FunctionSig`, currently `pub(crate)` because other MIR helper modules and tests instantiate it.
- `LoopContext`, `LambdaEnv`, `mir_local_name`, `lower_function`, and the root tests.

Current root inventory:

| Lines | Concern | Planned home |
|---:|---|---|
| 1-47 | module docs/imports | `mod.rs` |
| 48-93 | child module declarations/import wiring | `mod.rs` |
| 95-130 | `MirLowerOptions` | `options.rs` re-exported from `mod.rs` |
| 132-139 | `mir_local_name` | `context_utils.rs` or `mod.rs` if still only root-local |
| 141-348 | `lower_hir`, `lower_hir_with_options` orchestration | `entry.rs` re-exported from `mod.rs` |
| 350-425 | `lower_function` | `function_lowering.rs` |
| 427-455 | `LoopContext`, `FunctionSig`, `LambdaEnv` | `mod.rs` or small state-support file depending on privacy inventory |
| 457-493 | `LoweringContext` fields | `mod.rs` |
| 496-1505 | `impl LoweringContext` methods | focused sibling impl files |
| 1507-1570 | root tests | colocate with moved concern or `tests` module in `mod.rs` |

Existing child helper directory inventory before this change:

```text
compiler/src/mir/lowering/
  aggregate_expr_helpers.rs      333 LoC
  assignment_helpers.rs          258 LoC
  block_async_expr_helpers.rs    157 LoC
  body_lowering_helpers.rs        68 LoC
  builtin_helpers.rs             580 LoC
  call_emission_helpers.rs       119 LoC
  call_expr_helpers.rs            70 LoC
  call_invocation_helpers.rs      93 LoC
  call_target_helpers.rs         164 LoC
  for_expr_helpers.rs            400 LoC
  if_expr_helpers.rs             126 LoC
  lambda_expr_helpers.rs         196 LoC
  let_stmt_helpers.rs            351 LoC
  loop_control_helpers.rs        158 LoC
  loop_expr_helpers.rs            93 LoC
  match_expr_helpers.rs          320 LoC
  method_builtin_helpers.rs       70 LoC
  method_call_helpers.rs         395 LoC
  method_expr_helpers.rs          63 LoC
  named_call_helpers.rs          112 LoC
  non_named_call_helpers.rs       50 LoC
  op_expr_helpers.rs             477 LoC
  pointer_expr_helpers.rs        146 LoC
  while_expr_helpers.rs          121 LoC
```

## Goals / Non-Goals

**Goals:**

1. Convert `compiler/src/mir/lowering.rs` to `compiler/src/mir/lowering/mod.rs` without changing the logical module path.
2. Preserve public MIR-lowering API paths and signatures through `compiler/src/mir/mod.rs` and `compiler/src/lib.rs`.
3. Preserve every existing child helper module path and behavior under `compiler/src/mir/lowering/`.
4. Split the large `impl LoweringContext` into focused sibling impl files while keeping the `LoweringContext` type itself in the module root unless later inventory proves a low-risk move.
5. Keep every resulting file below the original root size and below the roadmap ~1000 LoC target; target `mod.rs` below ~500 LoC.
6. Preserve all MIR instructions, block ordering, error strings, async origin tracking, contract checks, and generic method materialization behavior.

**Non-Goals:**

- No MIR semantic changes, optimization changes, or lowering cleanup.
- No changes to HIR, type checking, MIR data model, codegen, or runtime behavior.
- No splitting of existing helper files such as `builtin_helpers.rs` or `op_expr_helpers.rs` in this change, unless required to keep the root split compiling.
- No conversion of `LoweringContext` into a trait, adapter object, or separate public API.
- No test assertion rewrites or fixture changes to accommodate output drift.
- No dependency changes.

## Decisions

### Decision 1: Use a mechanical directory-module rename as Slice 0

Slice 0 will move `compiler/src/mir/lowering.rs` to `compiler/src/mir/lowering/mod.rs` byte-for-byte. The existing `compiler/src/mir/lowering/*.rs` helper files remain in place.

Rationale: this isolates Rust module-resolution risk from method-move risk. Existing child declarations such as `mod aggregate_expr_helpers;` should keep resolving to the same physical files after the root becomes `mod.rs`.

Alternative considered: keep `lowering.rs` as a flat module and only move methods into a new `lowering_root/` directory. Rejected because it would create a new logical module name and would not advance the standard directory-module SOP.

### Decision 2: Keep `LoweringContext` struct fields in `mod.rs` initially

The `LoweringContext` type and its fields should remain in the module root during the first split. Focused sibling files can define additional `impl<'a> LoweringContext<'a>` blocks and still access parent-private fields because they are descendants of the parent module.

Rationale: existing child helpers rely heavily on direct `ctx` method calls and state access through `use super::*`. Moving the struct itself into `context.rs` would turn existing helper files into siblings of the struct definition and would force broad `pub(super)` field promotion. Keeping the struct in `mod.rs` avoids unnecessary visibility widening.

Alternative considered: move `LoweringContext` and all fields into `context.rs`. Rejected for this change because it increases privacy churn and makes field visibility the dominant risk instead of the structural split.

### Decision 3: Move methods by dependency clusters, not by expression syntax alone

The large `impl LoweringContext` will be split into several sibling impl files. Planned clusters:

```text
compiler/src/mir/lowering/
  mod.rs                    (module wiring, imports, state structs, re-exports)
  options.rs                (MirLowerOptions + Default/new/with_async_functions)
  entry.rs                  (lower_hir, lower_hir_with_options)
  function_lowering.rs      (lower_function and parameter binding orchestration)
  context_methods.rs        (new, materialization, type inference, naming helpers)
  block_state_methods.rs    (blocks, locals, loop stack, push_inst, set_terminator, casts)
  contract_methods.rs       (precondition/postcondition/contract-condition helpers)
  body_dispatch_methods.rs  (body lowering, stmt dispatch, expr dispatch)
  print_methods.rs          (print lowering helpers)
  async_methods.rs          (async block/future origin helper methods)
  pattern_methods.rs        (pattern matching/binding helpers)
```

Rationale: this keeps each moved method close to methods that share state and call each other. It also avoids a premature semantic split of existing expression helper files.

Alternative considered: move each existing `HIRExpr` match arm into a separate helper file. Rejected because most match arms already delegate to existing helper modules; this change should split the remaining root, not redesign expression lowering.

### Decision 4: Re-export public items from `mod.rs`

If `MirLowerOptions`, `lower_hir`, or `lower_hir_with_options` move into sibling files, `mod.rs` will re-export them so existing `pub use lowering::{...}` in `compiler/src/mir/mod.rs` remains unchanged.

Rationale: public consumers should not observe the physical file move. The module root remains the contract boundary.

Alternative considered: update `compiler/src/mir/mod.rs` to re-export from deeper paths such as `lowering::entry::lower_hir`. Rejected because it leaks implementation layout into the parent module and creates avoidable future churn.

### Decision 5: Minimal method visibility widening only

Methods moved into sibling impl files become `pub(super)` only when they are called from another sibling module. Methods used only within their new file remain private. `FunctionSig` stays `pub(crate)` because existing helper tests and other MIR helpers instantiate it.

Rationale: this follows the JIT split's inherent-impl visibility rule while preserving the private nature of lowering internals.

Alternative considered: make all moved `LoweringContext` methods `pub(super)` up front. Rejected because it hides real dependencies and widens more than needed.

## Risks / Trade-offs

- **Private method calls break across sibling impl files** -> Mitigate by inventorying each moved method's callers before the slice and promoting only required methods to `pub(super)`.
- **Field visibility churn if `LoweringContext` moves too early** -> Mitigate by keeping the struct and fields in `mod.rs` for this change.
- **Existing child helper modules depend on `use super::*`** -> Mitigate by preserving root re-exports/import wiring until each helper compiles unchanged.
- **MIR behavior drift from moving body/expression dispatch** -> Mitigate by moving methods intact and running compiler-focused tests after every slice.
- **`cargo fmt --all -- --check` may be blocked by unrelated formatting drift** -> Mitigate by running rustfmt check on touched files and recording any unrelated blockers separately, as done in the JIT split.
- **Root file may still be moderately large** -> Accept if `mod.rs` remains under ~500 LoC and every file is below the original and roadmap thresholds; further existing-helper splits can be follow-up changes.

## Migration Plan

1. Create and strictly validate this OpenSpec change before implementation.
2. Run the baseline verification before code moves:
   - `cargo test -p sengoo-compiler --lib`
   - `cargo test -p sgc`
   - `cargo test -p sengoo-runtime --lib`
   - `cargo test -p sgpm`
3. Implement one slice at a time, committing each independently after tests pass.
4. Run targeted MIR-lowering smoke after high-risk slices:
   - `cargo test -p sengoo-compiler lowering --lib`
   - `cargo test -p sengoo-compiler generic_typeck --lib`
5. Finalize with file-size evidence, roadmap update, full baseline, strict OpenSpec validation, and archive.

Rollback strategy: each slice is a pure structural commit, so rollback is a normal `git revert` of the most recent slice. Slice 0 can be reverted independently if directory-module resolution fails.

## Open Questions

- Should `LoweringContext` be moved to a dedicated `context.rs` in a later follow-up after method clusters stabilize? Tentative answer: only if the field visibility inventory shows limited `pub(super)` churn.
- Should large existing helper files such as `builtin_helpers.rs` or `op_expr_helpers.rs` become separate Large File Splits follow-ups? Tentative answer: yes, but not in this change because they are already below the roadmap threshold.
