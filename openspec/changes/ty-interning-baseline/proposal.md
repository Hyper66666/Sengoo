## Why

`compiler/src/typeck/ty.rs` currently represents every compiler type as an owned recursive `Ty` tree. Composite types such as tuples, functions, ADTs, references, pointers, slices, arrays, and futures deep-clone nested `Ty` values through type inference, type checking, substitutions, diagnostics, and MIR lowering.

This is now the next highest-leverage optimization because the roadmap cleanup is complete and the preceding shared-state work proved the `Rc<RefCell<...>>` pattern that can carry a compiler-wide type arena without a big-bang rewrite.

## What Changes

- Introduce a type interning baseline centered on a compact `TyId` handle and a `TyInterner` / type arena.
- Preserve the existing `Ty` / `TyKind` surface during phase 1 so the migration can be incremental rather than replacing every call site at once.
- Add canonical interning APIs for primitive, composite, function, ADT, variable, inferred, future, and error types.
- Add lookup APIs that let later phases resolve a `TyId` to its interned `TyKind` without cloning the full recursive tree.
- Migrate the high-impact ownership boundaries first: type environment storage, substitution maps, inference checkpoints, and type-checker helpers.
- Add equivalence, display, and diagnostic paths for interned types so behavior remains stable while storage changes.
- Add regression tests and measurement hooks for clone-count / allocation reduction.
- No **BREAKING** source-language change is intended.

## Capabilities

### New Capabilities

- `interned-types`: Defines the compiler capability to allocate, reuse, compare, and display type-checker types through interned `TyId` handles.

### Modified Capabilities

- None. `openspec/specs/` is currently empty, and this change is an internal compiler representation optimization rather than a source-language requirement change.

## Impact

- Affected code:
  - `compiler/src/typeck/ty.rs`
  - `compiler/src/typeck/env.rs`
  - `compiler/src/typeck/infer.rs`
  - `compiler/src/typeck/check.rs`
  - `compiler/src/mir/lowering.rs`
  - downstream helpers that currently store or clone owned `Ty`
- Public Sengoo language syntax and semantics are unchanged.
- Rust API surface inside the compiler changes by adding interner-backed APIs and gradually preferring `TyId` at storage/checkpoint boundaries.
- No new external dependencies are required.
- Verification must keep the compiler, sgc, runtime, and examples smoke tests green before any follow-up migration proceeds.
