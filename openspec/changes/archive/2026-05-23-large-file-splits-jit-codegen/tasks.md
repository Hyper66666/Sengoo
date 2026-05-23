## 1. Inventory and baseline

- [x] 1.1 Confirm no active OpenSpec changes with `openspec list` before implementation.
- [x] 1.2 Record the public API inventory before code moves: `pub struct JITCodegen`, public methods `new`, `generate`, `add_string`, `to_string`, and `impl Default`.
- [x] 1.3 Record the public re-export inventory before code moves: `compiler/src/codegen/mod.rs::pub mod jit`, `compiler/src/codegen/mod.rs::pub use jit::JITCodegen`, and `compiler/src/lib.rs::pub use codegen::{jit::JITCodegen, Codegen}`.
- [x] 1.4 Record the external test/callsite inventory before code moves: `compiler/src/tests/cast_semantics_tests.rs` JIT tests that construct `JITCodegen::new()` and call `generate`.
- [x] 1.5 Record the existing child-module inventory before code moves: `compiler/src/codegen/jit/declaration_helpers.rs` already contains `impl JITCodegen` helper method `declare_runtime_functions`.
- [x] 1.6 Run and record the full verification baseline before Slice 0:
  `cargo test -p sengoo-compiler --lib`, `cargo test -p sgc`,
  `cargo test -p sengoo-runtime --lib`, `cargo test -p sgpm`.
- [x] 1.7 Run and record targeted compiler smoke before Slice 0:
  `cargo test -p sengoo-compiler cast_semantics --lib`.

## 2. Slice 0: Mechanical directory-module rename

- [x] 2.1 Rename `compiler/src/codegen/jit.rs` to `compiler/src/codegen/jit/mod.rs` with byte-identical content.
- [x] 2.2 Do not edit content in the same slice; this isolates Rust module-resolution risk from helper-move risk.
- [x] 2.3 Verify `compiler/src/codegen/jit/declaration_helpers.rs` still resolves from `mod declaration_helpers;` under the new `jit/mod.rs` parent.
- [x] 2.4 Run the full verification baseline.
- [x] 2.5 Commit `refactor(jit): convert to directory module (slice 0/6)` with baseline pass counts.

## 3. Slice 1: Extract `utils.rs`

- [x] 3.1 Create `compiler/src/codegen/jit/utils.rs`.
- [x] 3.2 Move `get_local_type`, `get_type_size`, `local_name`, `local_reg`, `mir_type_to_llvm_str`, `mir_constant_to_llvm_str`, and `emit_indent` from `mod.rs` into `utils.rs`.
- [x] 3.3 Add `use super::JITCodegen;`, `use super::common;`, and the minimal `crate::mir::{Local, MIRType, MirConstant, MirFunction}` imports needed by `utils.rs`.
- [x] 3.4 Promote moved helpers to `pub(super)` only where they are called from sibling modules; keep any helper private if it is only used inside `utils.rs`.
- [x] 3.5 Add `mod utils;` to `mod.rs` and avoid wildcard imports unless needed.
- [x] 3.6 Prune now-unused imports from `mod.rs`; keep verification warning-free.
- [x] 3.7 Run the full verification baseline.
- [x] 3.8 Commit `refactor(jit): extract utils.rs (slice 1/6)`.

## 4. Slice 2: Extract opcode and cast helpers

- [x] 4.1 Create `compiler/src/codegen/jit/ops.rs`.
- [x] 4.2 Move `binary_op_to_llvm` from `mod.rs` into `ops.rs`.
- [x] 4.3 Add `use super::JITCodegen;`, `use super::common;`, and the minimal `crate::mir::{MIRType, MirBinOp}` imports needed by `ops.rs`.
- [x] 4.4 Make `binary_op_to_llvm` `pub(super)` because `instructions.rs` will call it after later slices.
- [x] 4.5 Add `mod ops;` to `mod.rs`, prune unused imports, and run the full verification baseline.
- [x] 4.6 Commit `refactor(jit): extract ops.rs (slice 2a/6)`.
- [x] 4.7 Create `compiler/src/codegen/jit/casts.rs`.
- [x] 4.8 Move `emit_cast_value` from `mod.rs` into `casts.rs`.
- [x] 4.9 Add `use super::JITCodegen;` and the minimal `crate::mir::MIRType` import needed by `casts.rs`.
- [x] 4.10 Make `emit_cast_value` `pub(super)` because both instruction lowering and call argument lowering use it.
- [x] 4.11 Add `mod casts;` to `mod.rs`, prune unused imports, and run the full verification baseline.
- [x] 4.12 Commit `refactor(jit): extract casts.rs (slice 2b/6)`.

## 5. Slice 3: Extract `terminators.rs`

- [x] 5.1 Create `compiler/src/codegen/jit/terminators.rs`.
- [x] 5.2 Move `codegen_terminator` from `mod.rs` into `terminators.rs`.
- [x] 5.3 Add `use super::JITCodegen;` and the minimal `crate::mir::{self, MirFunction}` imports needed by `terminators.rs`.
- [x] 5.4 Make `codegen_terminator` `pub(super)` because `functions.rs::codegen_basic_block` calls it.
- [x] 5.5 Add `mod terminators;` to `mod.rs`, prune unused imports, and keep generated IR strings/error strings unchanged.
- [x] 5.6 Run the full verification baseline.
- [x] 5.7 Commit `refactor(jit): extract terminators.rs (slice 3/6)`.

## 6. Slice 4: Extract `functions.rs`

- [x] 6.1 Create `compiler/src/codegen/jit/functions.rs`.
- [x] 6.2 Move `codegen_function`, `codegen_basic_block`, and `emit_main_wrapper` from `mod.rs` into `functions.rs`.
- [x] 6.3 Add `use super::JITCodegen;` and the minimal `crate::mir::{self, MirFunction}` imports needed by `functions.rs`.
- [x] 6.4 Make `codegen_function` and `emit_main_wrapper` `pub(super)` because `mod.rs::generate` calls them.
- [x] 6.5 Keep `codegen_basic_block` private if it is only called by `codegen_function` within `functions.rs`; otherwise document any `pub(super)` promotion in this tasks file.
- [x] 6.6 Add `mod functions;` to `mod.rs`, prune unused imports, and verify that `generate` still has the same signature and output behavior.
- [x] 6.7 Run the full verification baseline.
- [x] 6.8 Commit `refactor(jit): extract functions.rs (slice 4/6)`.

## 7. Slice 5: Extract `instructions.rs`

- [x] 7.1 Create `compiler/src/codegen/jit/instructions.rs`.
- [x] 7.2 Move `codegen_instruction` from `mod.rs` into `instructions.rs` as a single intact function; do not split match arms in this change.
- [x] 7.3 Add `use super::JITCodegen;`, `use super::common;`, and the minimal `crate::mir::{self, MIRType, MirConstant, MirFunction, MirUnOp}` imports needed by `instructions.rs`.
- [x] 7.4 Make `codegen_instruction` `pub(super)` because `functions.rs::codegen_basic_block` calls it.
- [x] 7.5 Keep all generated LLVM IR fragments, fallback comments such as `unhandled instruction`, and error strings byte-for-byte unchanged.
- [x] 7.6 Add `mod instructions;` to `mod.rs`, prune unused imports, and verify no warning remains.
- [x] 7.7 Run the full verification baseline.
- [x] 7.8 Run targeted compiler smoke: `cargo test -p sengoo-compiler cast_semantics --lib`.
- [x] 7.9 Commit `refactor(jit): extract instructions.rs (slice 5/6)`.

## 8. Slice 6: Documentation, roadmap, and final validation

- [x] 8.1 Compute final line counts for every file under `compiler/src/codegen/jit/`.
- [x] 8.2 Verify size targets: `mod.rs` below 500 LoC, every submodule below the original 1363 LoC, and the largest resulting file below the roadmap ~1000 LoC target.
- [x] 8.3 Update `docs/plans/2026-05-18-next-priorities.md`: mark `large-file-splits-jit-codegen` as the active or completed P0 slice depending on implementation state, and keep the next recommended target list current.
- [x] 8.4 Update this `tasks.md` with actual final file sizes and any visibility promotions that differed from the design.
- [x] 8.5 Run final full verification baseline.
- [x] 8.6 Run final targeted compiler smoke: `cargo test -p sengoo-compiler cast_semantics --lib`.
- [x] 8.7 Commit `docs(jit): update split status and roadmap (slice 6/6)`.

## 9. Archival prerequisites

- [x] 9.1 All implementation tasks above checked.
- [x] 9.2 `openspec validate large-file-splits-jit-codegen --strict` reports no errors.
- [x] 9.3 `openspec list` shows the change ready to archive.
- [x] 9.4 Archive to `openspec/changes/archive/YYYY-MM-DD-large-file-splits-jit-codegen/`.
- [x] 9.5 Promote the updated `large-file-splits` capability spec into `openspec/specs/large-file-splits/spec.md`.
- [x] 9.6 Update persistent roadmap notes so the next split target can reuse both the original SOP and the new impl-block extension.


## 10. Completion notes

- Final JIT directory sizes: `mod.rs` 127 LoC, `instructions.rs` 857 LoC, `declaration_helpers.rs` 153 LoC, `terminators.rs` 121 LoC, `functions.rs` 105 LoC, `casts.rs` 84 LoC, `utils.rs` 50 LoC, `ops.rs` 18 LoC.
- Size targets met: `mod.rs` is below 500 LoC, every submodule is below the original 1363 LoC, and the largest resulting file is below the roadmap ~1000 LoC target.
- Visibility outcome: `codegen_function`, `emit_main_wrapper`, `codegen_instruction`, `codegen_terminator`, `binary_op_to_llvm`, `emit_cast_value`, and cross-module utility helpers use `pub(super)`; `codegen_basic_block` remains private in `functions.rs`.
- Formatting evidence: touched JIT files pass `rustfmt --edition 2021 --check compiler/src/codegen/jit/*.rs`. `cargo fmt --all -- --check` was attempted and is blocked by unrelated formatting drift in `compiler/src/typeck/*` and `runtime/src/reflect/runtime_db/*`.
- Verification evidence: `cargo test -p sengoo-compiler --lib`, `cargo test -p sengoo-compiler cast_semantics --lib`, `cargo test -p sgc`, `cargo test -p sengoo-runtime --lib`, `cargo test -p sgpm`, and `openspec validate large-file-splits-jit-codegen --strict` passed for final validation.
- Archive target: `openspec/changes/archive/2026-05-23-large-file-splits-jit-codegen/`, with the spec delta promoted into `openspec/specs/large-file-splits/spec.md`.
