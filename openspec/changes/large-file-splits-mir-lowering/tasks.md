## 1. Inventory and baseline

- [x] 1.1 Confirm current branch state and active OpenSpec status with `openspec.cmd list` before implementation.
- [x] 1.2 Confirm `compiler/src/mir/lowering.rs` line count and record existing `compiler/src/mir/lowering/*.rs` child helper line counts.
- [x] 1.3 Record public API inventory before code moves: `lower_hir`, `lower_hir_with_options`, `MirLowerOptions`, `MirLowerOptions::new`, `MirLowerOptions::with_async_functions`, and `impl Default for MirLowerOptions`.
- [x] 1.4 Record public re-export inventory before code moves: `compiler/src/mir/mod.rs::pub use lowering::{lower_hir, lower_hir_with_options, MirLowerOptions}` and `compiler/src/lib.rs::pub use mir::{lower_hir, lower_hir_with_options, MirLowerOptions}`.
- [x] 1.5 Record internal API inventory used by child modules/tests: `LoweringContext`, `FunctionSig`, `LoopContext`, `LambdaEnv`, `mir_local_name`, `lower_function`, and `LoweringContext` methods called from `compiler/src/mir/lowering/*.rs`.
- [x] 1.6 Record existing child-module inventory before code moves: all 24 files under `compiler/src/mir/lowering/` and any tests inside those files.
- [x] 1.7 Run and record full verification baseline before Slice 0: `cargo test -p sengoo-compiler --lib`, `cargo test -p sgc`, `cargo test -p sengoo-runtime --lib`, and `cargo test -p sgpm`.
- [x] 1.8 Run and record targeted MIR-lowering smoke before Slice 0: `cargo test -p sengoo-compiler lowering --lib` and `cargo test -p sengoo-compiler generic_typeck --lib`.

## 2. Slice 0: Mechanical directory-module rename

- [x] 2.1 Rename `compiler/src/mir/lowering.rs` to `compiler/src/mir/lowering/mod.rs` with byte-identical content.
- [x] 2.2 Do not edit content in the same slice; isolate Rust module-resolution risk from helper-move risk.
- [x] 2.3 Verify the existing child module declarations still resolve to `compiler/src/mir/lowering/*.rs`.
- [x] 2.4 Verify `compiler/src/mir/mod.rs` and `compiler/src/lib.rs` public re-exports still compile unchanged.
- [x] 2.5 Run the full verification baseline.
- [x] 2.6 Commit `refactor(mir): convert lowering to directory module (slice 0/N)` with baseline pass evidence.

## 3. Slice 1: Extract public options and entry API

- [x] 3.1 Create `compiler/src/mir/lowering/options.rs`.
- [x] 3.2 Move `MirLowerOptions`, `impl Default for MirLowerOptions`, `MirLowerOptions::new`, and `MirLowerOptions::with_async_functions` into `options.rs` without changing signatures or public fields.
- [x] 3.3 Re-export `MirLowerOptions` from `mod.rs` so existing parent-module re-exports remain unchanged.
- [x] 3.4 Move the `mir_lower_options_clone_shares_async_function_set` test with the options implementation or keep an equivalent root test without changing assertions.
- [x] 3.5 Create `compiler/src/mir/lowering/entry.rs`.
- [x] 3.6 Move `lower_hir` and `lower_hir_with_options` into `entry.rs` without changing signatures, error aggregation, item iteration order, lazy generic mono behavior, async function sharing, or eager trait function ordering.
- [x] 3.7 Re-export `lower_hir` and `lower_hir_with_options` from `mod.rs` so `compiler/src/mir/mod.rs` can remain unchanged.
- [x] 3.8 Promote only the helpers required by `entry.rs` to `pub(super)`; do not expose them beyond the `lowering` module.
- [x] 3.9 Run targeted MIR-lowering smoke and the full verification baseline.
- [x] 3.10 Commit `refactor(mir): extract lowering entry API (slice 1/N)`.

## 4. Slice 2: Extract function lowering orchestration

- [x] 4.1 Create `compiler/src/mir/lowering/function_lowering.rs`.
- [x] 4.2 Move `lower_function` into `function_lowering.rs` without changing parameter binding, contract pre/postcondition injection, error formatting, lambda function extraction, or return ordering.
- [x] 4.3 Keep `FunctionSig` accessible to existing helpers and tests; preserve its `pub(crate)` fields and derive attributes.
- [x] 4.4 Add minimal imports to `function_lowering.rs`; avoid wildcard imports unless matching existing module style is the least-risk path.
- [x] 4.5 Run targeted MIR-lowering smoke and the full verification baseline.
- [x] 4.6 Commit `refactor(mir): extract function lowering orchestration (slice 2/N)`.

## 5. Slice 3: Extract context construction and materialization helpers

- [ ] 5.1 Create `compiler/src/mir/lowering/context_methods.rs`.
- [ ] 5.2 Move `LoweringContext::new`, `async_dispatch_kind_id`, `lower_materialized_method`, `try_materialize_inherent_method`, `try_materialize_trait_method`, `infer_struct_literal_type`, `lambda_name`, and `async_block_name` into `context_methods.rs` as an `impl<'a> LoweringContext<'a>` block.
- [ ] 5.3 Keep `LoweringContext` struct fields in `mod.rs` for this change; do not promote fields unless a compiler error proves it is required.
- [ ] 5.4 Promote moved methods to `pub(super)` only if sibling modules call them.
- [ ] 5.5 Run targeted MIR-lowering smoke and the full verification baseline.
- [ ] 5.6 Commit `refactor(mir): extract lowering context helpers (slice 3/N)`.

## 6. Slice 4: Extract block, local, loop, and cast state helpers

- [ ] 6.1 Create `compiler/src/mir/lowering/block_state_methods.rs`.
- [ ] 6.2 Move loop-stack helpers `push_loop`, `pop_loop`, `get_break_target`, and `get_continue_target` into `block_state_methods.rs`.
- [ ] 6.3 Move local/block helpers `add_local`, `bind_local_symbol`, `get_local_type`, `resolve_local`, `new_block`, `set_current_block`, `current_block_or_error`, `current_block`, `push_inst`, and `set_terminator` into `block_state_methods.rs`.
- [ ] 6.4 Move future-origin and cast helpers `propagate_future_origin_through_phi`, `reconcile_binary_operand_types`, and `insert_cast` into `block_state_methods.rs`.
- [ ] 6.5 Move or keep `set_terminator_without_current_block_records_error_instead_of_panicking` next to `set_terminator` without changing assertions.
- [ ] 6.6 Promote moved methods to `pub(super)` only where child or sibling modules call them.
- [ ] 6.7 Run targeted MIR-lowering smoke and the full verification baseline.
- [ ] 6.8 Commit `refactor(mir): extract lowering block state helpers (slice 4/N)`.

## 7. Slice 5: Extract async, contract, and body dispatch helpers

- [ ] 7.1 Create `compiler/src/mir/lowering/async_methods.rs` and move `collect_async_block_free_vars`, `lower_async_block`, `infer_poll_func_from_last_call`, and `resolve_async_base_name` into it.
- [ ] 7.2 Create `compiler/src/mir/lowering/contract_methods.rs` and move `inject_precondition_check`, `inject_postcondition_checks`, and `lower_contract_condition` into it.
- [ ] 7.3 Create `compiler/src/mir/lowering/body_dispatch_methods.rs` and move `lower_body_to_block`, `lower_body_to_block_val`, `lower_body_to_block_with_return`, `lower_stmt`, and `lower_expr` into it.
- [ ] 7.4 Keep body/expression dispatch arms byte-for-byte equivalent except for import/path adjustments.
- [ ] 7.5 Promote moved methods to `pub(super)` only where child or sibling modules call them.
- [ ] 7.6 Run targeted MIR-lowering smoke and the full verification baseline.
- [ ] 7.7 Commit `refactor(mir): extract lowering async contract and dispatch helpers (slice 5/N)`.

## 8. Slice 6: Extract print, pattern, literal, and operator helpers

- [ ] 8.1 Create `compiler/src/mir/lowering/print_methods.rs` and move `emit_runtime_print_call`, `emit_print_str_literal`, and `emit_print_value` into it.
- [ ] 8.2 Create `compiler/src/mir/lowering/pattern_methods.rs` and move `matches_pattern` and `lower_pattern_bindings` into it.
- [ ] 8.3 Create `compiler/src/mir/lowering/literal_op_methods.rs` and move `lower_literal`, `lower_un_op`, and `lower_bin_op` into it.
- [ ] 8.4 Move `mir_local_name` into the smallest module that still satisfies its callers, or keep it in `mod.rs` if it remains root-local.
- [ ] 8.5 Promote moved methods to `pub(super)` only where child or sibling modules call them.
- [ ] 8.6 Run targeted MIR-lowering smoke and the full verification baseline.
- [ ] 8.7 Commit `refactor(mir): extract lowering leaf helpers (slice 6/N)`.

## 9. Slice 7: Import pruning, file-size evidence, and documentation

- [ ] 9.1 Prune unused imports from `compiler/src/mir/lowering/mod.rs` and all new sibling files.
- [ ] 9.2 Run rustfmt check on touched lowering files; if `cargo fmt --all -- --check` is blocked by unrelated formatting drift, record the blocker and the narrower touched-file rustfmt command that passed.
- [ ] 9.3 Compute final line counts for `compiler/src/mir/lowering/mod.rs` and every file under `compiler/src/mir/lowering/`.
- [ ] 9.4 Verify size targets: `mod.rs` below ~500 LoC, every resulting file below the original root size, and every resulting non-test file below the roadmap ~1000 LoC target.
- [ ] 9.5 Update `docs/plans/2026-05-18-next-priorities.md` with the implementation status and next Large File Splits candidate.
- [ ] 9.6 Update this `tasks.md` with actual final file sizes, visibility promotions, formatting evidence, and verification evidence.
- [ ] 9.7 Run final targeted MIR-lowering smoke and full verification baseline.
- [ ] 9.8 Commit `docs(mir): update lowering split status and roadmap (slice 7/N)`.

## 10. Archival prerequisites

- [ ] 10.1 All implementation tasks above are checked and completion notes are recorded.
- [ ] 10.2 `openspec.cmd validate large-file-splits-mir-lowering --strict` reports no errors.
- [ ] 10.3 `openspec.cmd list` shows the change ready to archive.
- [ ] 10.4 Archive to `openspec/changes/archive/YYYY-MM-DD-large-file-splits-mir-lowering/`.
- [ ] 10.5 Promote the updated `large-file-splits` capability spec into `openspec/specs/large-file-splits/spec.md`.
- [ ] 10.6 Update persistent roadmap notes so the next split can reuse the existing-child-directory root split SOP.

## 11. Inventory notes

- 1.1 Branch before implementation: `codex/large-file-splits-jit-codegen`. Active OpenSpec change: `large-file-splits-mir-lowering` with `0/72` tasks before inventory checkoff. Working tree also contains intentional untracked `.tmp/`, `bench/`, `website/`, and ChatGPT PNG plus the new change directory.
- 1.2 Root line count before Slice 0: `compiler/src/mir/lowering.rs` 1501 LoC by PowerShell `Get-Content` count. Existing child helper counts: `aggregate_expr_helpers.rs` 333, `assignment_helpers.rs` 258, `block_async_expr_helpers.rs` 157, `body_lowering_helpers.rs` 68, `builtin_helpers.rs` 580, `call_emission_helpers.rs` 119, `call_expr_helpers.rs` 70, `call_invocation_helpers.rs` 93, `call_target_helpers.rs` 164, `for_expr_helpers.rs` 400, `if_expr_helpers.rs` 126, `lambda_expr_helpers.rs` 196, `let_stmt_helpers.rs` 351, `loop_control_helpers.rs` 158, `loop_expr_helpers.rs` 93, `match_expr_helpers.rs` 320, `method_builtin_helpers.rs` 70, `method_call_helpers.rs` 395, `method_expr_helpers.rs` 63, `named_call_helpers.rs` 112, `non_named_call_helpers.rs` 50, `op_expr_helpers.rs` 477, `pointer_expr_helpers.rs` 146, `while_expr_helpers.rs` 121.
- 1.3 Public API before code moves: `pub struct MirLowerOptions` with public fields `runtime_contract_checks`, `lazy_generic_mono`, and `async_functions`; `impl Default for MirLowerOptions`; `MirLowerOptions::new`; `MirLowerOptions::with_async_functions`; `pub fn lower_hir`; `pub fn lower_hir_with_options`.
- 1.4 Public re-exports before code moves: `compiler/src/mir/mod.rs` contains `pub use lowering::{lower_hir, lower_hir_with_options, MirLowerOptions};`; `compiler/src/lib.rs` contains `pub use mir::{lower_hir, lower_hir_with_options, MirLowerOptions};`.
- 1.5 Internal API before code moves: `mir_local_name`, `lower_function`, `LoopContext`, `FunctionSig`, `LambdaEnv`, `LoweringContext<'a>`, and the current `impl LoweringContext<'a>` methods are root-owned and used by child helpers through `use super::*`. Grep found direct `ctx.*` helper usage across 23 child helper files, so moved methods must be promoted only as needed.
- 1.6 Existing child-module inventory before code moves: 24 child files under `compiler/src/mir/lowering/`; grep found unit-test modules and `LoweringContext::new` test setup patterns across many helpers including `while_expr_helpers.rs`, `pointer_expr_helpers.rs`, `op_expr_helpers.rs`, `named_call_helpers.rs`, `method_call_helpers.rs`, `let_stmt_helpers.rs`, `match_expr_helpers.rs`, and others.
- 1.7 Baseline before Slice 0 passed: `cargo test -p sengoo-compiler --lib` 559 passed; `cargo test -p sgc` 217 passed; `cargo test -p sengoo-runtime --lib` 42 passed; `cargo test -p sgpm` 18 unit + 8 integration passed.
- 1.8 Targeted MIR-lowering smoke before Slice 0 passed: `cargo test -p sengoo-compiler lowering --lib` 87 passed; `cargo test -p sengoo-compiler generic_typeck --lib` 12 passed.
- 2.1-2.4 Slice 0 was a byte-identical `R100` rename from `compiler/src/mir/lowering.rs` to `compiler/src/mir/lowering/mod.rs`; `compiler/src/mir/lowering/` contains `mod.rs` plus the same 24 existing helper files; `compiler/src/mir/mod.rs` and `compiler/src/lib.rs` public re-exports remained unchanged.
- 2.5 Slice 0 full baseline passed: `cargo test -p sengoo-compiler --lib` 559 passed; `cargo test -p sgc` 217 passed; `cargo test -p sengoo-runtime --lib` 42 passed; `cargo test -p sgpm` 18 unit + 8 integration passed.
- 3.1-3.8 Slice 1 created `options.rs` and `entry.rs`. `MirLowerOptions` and its clone-sharing test moved to `options.rs`; `lower_hir` and `lower_hir_with_options` moved to `entry.rs`; `mod.rs` re-exports `MirLowerOptions`, `lower_hir`, and `lower_hir_with_options`. No helper visibility promotion was required in this slice because `entry.rs` uses `use super::*` and root-private helpers remain accessible within the child module.
- 3.9 Slice 1 targeted smoke passed: `cargo test -p sengoo-compiler lowering --lib` 87 passed; `cargo test -p sengoo-compiler generic_typeck --lib` 12 passed. Full baseline passed: `cargo test -p sengoo-compiler --lib` 559 passed; `cargo test -p sgc` 217 passed; `cargo test -p sengoo-runtime --lib` 42 passed; `cargo test -p sgpm` 18 unit + 8 integration passed.
- 4.1-4.4 Slice 2 created `function_lowering.rs` and moved `lower_function` intact except visibility changed to `pub(super)` because `entry.rs` calls it. `FunctionSig` stayed in `mod.rs` with its existing `pub(crate)` fields and derives.
- 4.5 Slice 2 targeted smoke passed: `cargo test -p sengoo-compiler lowering --lib` 87 passed; `cargo test -p sengoo-compiler generic_typeck --lib` 12 passed. Full baseline passed: `cargo test -p sengoo-compiler --lib` 559 passed; `cargo test -p sgc` 217 passed; `cargo test -p sengoo-runtime --lib` 42 passed; `cargo test -p sgpm` 18 unit + 8 integration passed.
