## Context

The compiler type checker currently models types with `Ty { id: TyId, kind: TyKind }`, where `TyKind` recursively owns child types through `Vec<Ty>` and `Box<Ty>`. This keeps call sites simple, but it makes every cloned composite type a deep clone. The cost appears in `compiler/src/typeck/infer.rs` substitution checkpoints, `compiler/src/typeck/check.rs` substitution helpers and diagnostics, `compiler/src/typeck/env.rs` symbol storage, and MIR lowering paths that cross from HIR/typeck types into MIR types.

The next optimization must be incremental. `Ty` is already used across type checking, generic substitution, FFI validation, error reporting, and lowering. A direct replacement of every `Ty` with `TyId` would be too invasive and would risk changing language behavior while optimizing representation.

This design therefore introduces interning as a baseline capability first: create an arena-backed `TyInterner`, establish canonical `TyId` handles, add lookup/display/equality APIs, and migrate only the highest-impact storage/checkpoint boundaries before deeper call-site rewrites.

## Goals / Non-Goals

**Goals:**

- Introduce a single authoritative interner for type-checker `TyKind` values during one type-checking session.
- Make `TyId` a compact, copyable handle that can be used in maps, checkpoints, and stored symbols without deep cloning nested types.
- Preserve behavior of type equality, diagnostics, display strings, FFI validation, generic instantiation, and MIR lowering.
- Support a two-phase migration: phase 1 adds interning and compatibility adapters; phase 2 moves more APIs from owned `Ty` to `TyId`.
- Keep all current compiler and sgc tests green before any follow-up work such as restoring `ffi_buffer_from_bytes_raw` to `Result<Buffer, i64>`.

**Non-Goals:**

- No source-language syntax or semantic change.
- No big-bang replacement of every `Ty` use site.
- No thread-safe global interner; compilation remains single-threaded for this path.
- No new external dependency unless measurement proves the standard library containers are insufficient.
- No MIR type-system redesign beyond consuming interned type information where needed.

## Decisions

### Decision 1: Add `TyInterner` as an owned compiler/session state object

`TyInterner` will own an arena of interned type records and a structural lookup map. It should be stored in the type-checking context and passed or shared into helpers that need to allocate or inspect type IDs.

- **Chosen:** session-local interner owned by type checking.
- **Alternative considered:** global/static interner.
- **Rationale:** a session-local arena avoids cross-compilation contamination, avoids locking, and matches current single-run compiler state.

### Decision 2: Keep the existing `Ty` surface during baseline migration

The baseline should not force all call sites to rewrite from `&Ty` to `TyId` immediately. Instead, `Ty` remains available as a compatibility view while interned IDs are introduced at high-impact boundaries.

- **Chosen:** `Ty` compatibility layer with interner-backed constructors and lookup helpers.
- **Alternative considered:** change `TyKind` child fields from `Ty` to `TyId` immediately.
- **Rationale:** immediate recursive-field replacement would touch nearly every type checker match arm, every diagnostic construction, and MIR lowering conversion in one risky patch.

### Decision 3: Intern structural type shapes, not diagnostic strings

The interner key should be based on semantic structure: primitive variants, child `TyId`s, ADT names and argument IDs, function params/return IDs, mutability, array length, and type variable IDs. Display strings remain derived output.

- **Chosen:** structural keys.
- **Alternative considered:** string-based canonicalization.
- **Rationale:** display strings are for humans and can lose detail; structural keys preserve equality and future optimization opportunities.

### Decision 4: Migrate substitution and environment storage before broad expression checking

The first performance win should target repeated deep clones in substitution maps and symbol/type environment storage. `Subst` and `TypeEnv` are natural boundaries because they store types long-term and are cloned/checkpointed during unification.

- **Chosen:** migrate `Subst` values, symbols, and selected type checker helpers toward `TyId` first.
- **Alternative considered:** rewrite expression checking return types first.
- **Rationale:** return-type rewrites fan out widely, while maps/checkpoints give immediate clone reduction with clearer invariants.

### Decision 5: Use scoped shared mutable state if helpers need ownership across modules

If the interner must be shared between `TypeChecker`, `TypeInfer`, and lowering helpers, use the already-proven `Rc<RefCell<...>>` shape from prior lowering shared-state work. Borrow scopes must be short and never cross recursive calls unnecessarily.

- **Chosen:** session-local owner first; `Rc<RefCell<TyInterner>>` only where ownership boundaries require it.
- **Alternative considered:** pass `&mut TyInterner` through every helper immediately.
- **Rationale:** explicit mutable references are preferable when local, but shared handles reduce churn at existing context boundaries.

## Risks / Trade-offs

- **Risk:** `TyId` values from different interners could be compared accidentally. → **Mitigation:** keep interner ownership scoped to one type-checking session and avoid exposing raw IDs without lookup context.
- **Risk:** compatibility `Ty` values can hide whether a path is still deep-cloning. → **Mitigation:** define explicit migration tasks and add clone-count/allocation checks around targeted files.
- **Risk:** `RefCell` borrow panics if shared interner borrows are held too long. → **Mitigation:** prefer `&mut TyInterner`; where `Rc<RefCell>` is necessary, keep borrow scopes inside small blocks.
- **Risk:** structural key hashing for large types still needs traversal on first insert. → **Mitigation:** canonicalization pays once per new shape; repeated uses become cheap `TyId` copies.
- **Risk:** diagnostics may regress if display paths lose access to full type structure. → **Mitigation:** add formatting APIs that accept `TyId` plus interner and test representative nested types.

## Migration Plan

1. Add `TyInterner`, interned records, structural keys, and constructor APIs in `compiler/src/typeck/ty.rs` without changing current behavior.
2. Add unit tests for canonical IDs, distinct structurally different IDs, recursive composite lookups, and display formatting.
3. Wire the interner into `TypeEnv`, `TypeInfer`, and `TypeChecker` constructors while preserving existing helper APIs.
4. Migrate `Subst` and inference checkpoint storage from owned recursive `Ty` values toward `TyId` or a compatibility handle that clones cheaply.
5. Migrate selected environment/symbol type storage and generic instantiation hot paths.
6. Update MIR lowering adapters only where type information crosses the typeck/MIR boundary.
7. Run the full verification suite and record clone-count or allocation deltas.
8. Defer broad `TyKind` recursive field replacement to a follow-up change once the baseline is stable.

Rollback is straightforward while phase 1 keeps compatibility APIs: revert the interner wiring and keep current owned `Ty` storage. No source-language migration is needed.

## Open Questions

- Should `TypeckError` store `TyId` plus interner-aware formatting, or keep owned `TyKind` snapshots for diagnostics during phase 1?
- Should `TyId` remain `usize` or become a newtype to prevent mixing arena IDs with type variable IDs?
- How much of MIR lowering should depend on interned type IDs in this baseline versus only after the type-checker migration settles?
