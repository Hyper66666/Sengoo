## Why

`compiler/src/codegen/jit.rs` is 1363 LoC and concentrates the entire `JITCodegen` implementation in one giant `impl` block, including module orchestration, function/block lowering, an ~880-line instruction matcher, terminator lowering, casts, and naming/type utilities.

This is the next roadmap P0 Large File Splits target after `runtime_db`: it is smaller and lower-risk than the 2 KLoC runtime/tooling giants, but it exercises a new split shape — multiple sibling files contributing inherent `impl JITCodegen` blocks — so it extends the proven split SOP beyond flat free-function modules.

## What Changes

- Convert `compiler/src/codegen/jit.rs` into the existing `compiler/src/codegen/jit/` directory module by moving it to `compiler/src/codegen/jit/mod.rs`.
- Keep the public `JITCodegen` type and its public methods (`new`, `generate`, `add_string`, `to_string`) available through the unchanged paths:
  - `sengoo_compiler::JITCodegen`
  - `sengoo_compiler::codegen::JITCodegen`
  - `sengoo_compiler::codegen::jit::JITCodegen`
- Preserve the existing `compiler/src/codegen/jit/declaration_helpers.rs` submodule and its `declare_runtime_functions` helper.
- Extract focused impl-block submodules for function/block lowering, instruction lowering, terminator lowering, casts/opcode lowering, and utility helpers.
- Preserve generated LLVM IR text behavior, error strings, test assertions, and public Rust API. This is a pure structural refactor.
- Extend the `large-file-splits` capability with an explicit impl-block splitting requirement so future large-file changes can safely split large inherent impls across sibling modules.
- No **BREAKING** Rust API change, no dependency change, no MIR lowering behavior change, and no LLVM IR semantic improvement in this change.

## Capabilities

### New Capabilities

- None.

### Modified Capabilities

- `large-file-splits`: Add requirements for splitting large inherent `impl` blocks across sibling module files while preserving the public type surface and limiting helper visibility to `pub(super)` where cross-file calls require it.

## Impact

- Affected code:
  - `compiler/src/codegen/jit.rs` -> `compiler/src/codegen/jit/mod.rs` plus focused sibling submodules under `compiler/src/codegen/jit/`.
  - `compiler/src/codegen/mod.rs` should continue to use `pub mod jit; pub use jit::JITCodegen;` unchanged.
- Existing callsites must remain unchanged:
  - `compiler/src/lib.rs: pub use codegen::{jit::JITCodegen, Codegen};`
  - `compiler/src/codegen/mod.rs: pub use jit::JITCodegen;`
  - `compiler/src/tests/cast_semantics_tests.rs` JIT tests using `JITCodegen::new().generate(...)`.
- Verification must keep the standard baseline green after every implementation slice:
  - `cargo test -p sengoo-compiler --lib`
  - `cargo test -p sgc`
  - `cargo test -p sengoo-runtime --lib`
  - `cargo test -p sgpm`
- Extra compiler-focused smoke during final verification:
  - `cargo test -p sengoo-compiler cast_semantics --lib`
