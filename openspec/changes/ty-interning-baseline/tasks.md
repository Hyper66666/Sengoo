## 1. Interner Foundation

- [ ] 1.1 Add a `TyInterner` / arena structure in `compiler/src/typeck/ty.rs` with stable session-local `TyId` allocation and structural lookup.
- [ ] 1.2 Add interned type records or keys for primitives, tuples, arrays, slices, refs, pointers, function types, ADTs, dyn/impl trait markers, futures, `Self`, inferred, error, and type variables.
- [ ] 1.3 Add constructor APIs that intern every supported `TyKind` shape and return canonical `TyId` values.
- [ ] 1.4 Add lookup APIs that resolve `TyId` to interned kind data without cloning full recursive `Ty` trees.
- [ ] 1.5 Add unit tests for canonical reuse, structural distinction, nested composite lookup, and invalid-ID handling.

## 2. Compatibility Layer

- [ ] 2.1 Preserve existing `Ty` / `TyKind` behavior through compatibility constructors or views so unmigrated call sites continue to compile.
- [ ] 2.2 Add interner-aware formatting helpers that produce the same display strings currently emitted by `Ty` / `TyKind`.
- [ ] 2.3 Add equality helpers for comparing interned type handles and compatibility `Ty` values consistently.
- [ ] 2.4 Audit `TypeckError` construction and keep diagnostics user-facing equivalent during phase 1.

## 3. Type Checker Integration

- [ ] 3.1 Wire the interner into `TypeEnv`, `TypeInfer`, and `TypeChecker` construction without changing public Sengoo language behavior.
- [ ] 3.2 Migrate primitive and common composite type constructors in `TypeEnv` to allocate through the interner.
- [ ] 3.3 Migrate `Subst` and inference checkpoint storage toward `TyId` or an interner-backed cheap handle.
- [ ] 3.4 Migrate symbol/type storage in `compiler/src/typeck/env.rs` where repeated owned `Ty` clones are currently stored.
- [ ] 3.5 Update generic substitution and instantiation helpers in `compiler/src/typeck/check.rs` and `compiler/src/typeck/infer.rs` to use interner-backed handles at storage boundaries.

## 4. MIR and Downstream Boundaries

- [ ] 4.1 Update MIR lowering adapters that consume type-checker types so they can read interned type information where needed.
- [ ] 4.2 Keep owned compatibility conversion available for lowering paths that are not migrated in this baseline.
- [ ] 4.3 Verify FFI validation paths still report and compare type information correctly.

## 5. Verification and Measurements

- [ ] 5.1 Run `cargo test -p sengoo-compiler --lib` and confirm the full compiler library suite passes.
- [ ] 5.2 Run `cargo test -p sgc` and confirm sgc integration tests pass.
- [ ] 5.3 Run `cargo test -p sengoo-runtime --lib` and confirm runtime library tests pass.
- [ ] 5.4 Run the examples smoke coverage that exists in sgc and confirm examples continue to compile/run or gracefully skip environment-dependent cases.
- [ ] 5.5 Record clone-count or allocation-reduction evidence for `compiler/src/typeck/ty.rs`, `infer.rs`, `check.rs`, `env.rs`, and relevant MIR lowering boundaries.
- [ ] 5.6 Confirm no broad source-language behavior changes and no unrelated refactors are included in the diff.

## 6. Follow-up Gate

- [ ] 6.1 Document remaining unmigrated owned-`Ty` call sites for the next interning rollout phase.
- [ ] 6.2 Do not restore `ffi_buffer_from_bytes_raw` to `Result<Buffer, i64>` until this baseline is implemented and all verification commands pass.
