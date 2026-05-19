# Sengoo Next Priorities (updated 2026-05-20)

## Current State

All earlier roadmap OpenSpec changes are shipped and archived, including the
most recent `ty-interning-baseline` (Phase 1 compiler type interning) which
landed 9 slices A–I across commits `7c39031c` → `1f7dedf3` and was archived to
`openspec/changes/archive/2026-05-19-ty-interning-baseline/`. A new
capability spec at `openspec/specs/interned-types/spec.md` is the first entry
in the previously empty `openspec/specs/` directory.

The active P0 is now `P1-A: Large File Splits` (promoted below). No
OpenSpec change is currently open; the next change should be drafted for the
largest target file (`compiler/src/codegen/jit.rs`).

The `ty-interning-baseline` deliverables:

- `compiler/src/typeck/interner.rs`: `InternedTyId(u32)` newtype, mirror
  `InternedTyKind` enum, `TyInterner` session arena with structural dedup, plus
  `intern` / `intern_ty` / `lookup` / `try_lookup` / `materialize` /
  `ty_eq` / `id_eq_ty` API.
- `compiler/src/typeck/env.rs`: `TypeEnv` carries a shared
  `Rc<RefCell<TyInterner>>` field; all builtin and composite ctors funnel
  through `new_ty` which interns; `intern_ty(&Ty)` and `symbol_ty_id(&str)`
  passthroughs.
- `compiler/src/typeck/ty.rs`: `Subst.map` migrated to
  `HashMap<TyVarId, InternedTyId>`; checkpoint clones now duplicate `Copy`
  handles instead of deep-cloning Ty subtrees; `TypeckError` documented as
  intentionally retaining owned `TyKind` snapshots for diagnostic stability.
- 10 new typeck unit tests covering canonical reuse, structural distinction,
  invalid IDs, ty_eq, id_eq_ty, builtin pre-interning, shared-arena across
  env clones, symbol_ty_id hits, and the `subst_clone_is_cheap_via_shared_interner_and_id_handles`
  structural evidence test.

Phase 2 follow-ups carried forward from the archive’s tasks.md catalog:

- Migrate `SymbolKind::{Var, Function, Type, Const, Static}` storage to
  `InternedTyId` (touches 6 call sites listed in 6.1).
- Migrate `FunctionTy` / `MethodSig` / `ImplInfo` storage in `trait.rs` to
  `InternedTyId`.
- Restore `tools/stdlib/ffi.sg::ffi_buffer_from_bytes_raw` from
  `-> Buffer` to `-> Result<Buffer, i64>` (Slice I gate now met).

The even-earlier P0 item, `examples-coverage-expansion`, remains complete:

- `examples/async/` contains sleep/spawn, select, and task lifecycle demos.
- `examples/generics/` contains Vec-like, Option, and Result demos.
- `examples/traits/` contains trait dispatch and generic trait method demos.
- `examples/ffi/` has a Makefile plus refreshed build/run documentation.
- `tools/sgc/src/tests.rs` includes `examples_catalog_*` and
  `examples_smoke_*` coverage.
- `README.md`, `README.zh-CN.md`, and `examples/README.md` link the new
  examples index.

## Shipped And Archived

| Change | Summary |
|---|---|
| async-native-execution-and-ci-smoke | Async lowering, native runtime bridge, CI smoke |
| async-phase-2-features | async block, spawn, join, select core surface |
| async-runtime-hardening-and-lowering-split | Future-escape checks, dispatch IDs, lowering split |
| enforce-struct-field-completeness | Struct literal missing/duplicate/unknown field validation |
| examples-coverage-expansion | Async/generics/traits/ffi examples plus smoke coverage |
| mir-async-functions-shared-state | Rc<RefCell> for async_functions plus ConcreteTypeRegistry |
| mir-bitcast-instruction | MIR Bitcast for float async frame support |
| reflection-runtime-sengoo-wrappers | Sengoo-side wrappers for db/ffi/lua54/proto/net |
| sgpm-mvp-implementation | sgpm package manager MVP with path deps |
| stdlib-generic-support | Option/Result/Vec/HashMap generic surface consolidation |
| stdlib-module-decomposition | Split collections.sg into per-topic modules |
| toolchain-language-runtime-roadmap | sglsp, sgfmt, sgpm, generics, macros, docs |
| ty-interning-baseline | TyInterner + InternedTyId + Subst cheap-clone migration; FunctionTy/MethodSig/Symbol storage migration deferred to Phase 2 |
| verify-mixed-width-type-correctness | Mixed-width integer type pipeline verification |

## Active Backlog

### P0: Large File Splits (promoted from P1-A on 2026-05-20)

Split the largest non-test files while preserving behavior. Do this in small
reviewable moves, not broad rewrites. Suggested first OpenSpec change:
`compiler/src/codegen/jit.rs` (highest measured size, most cross-concern).

Target files:

- `compiler/src/codegen/jit.rs`: split instruction emission, block emission,
  frame helpers, and runtime bridge helpers.
- `compiler/src/mir/lowering.rs`: split coercion, async-frame glue, and generic
  instantiation helpers.
- `tools/sgc/src/interface.rs` and `tools/sgc/src/commands.rs`: split by
  subcommand or interface concern.

Goal: no single non-test source file over 25 KB unless there is a documented
reason to keep it whole.

### P1-A: Phase 2 Ty Interning Storage Sweep

New P1-A (split out of the archived `ty-interning-baseline` Phase 2
follow-ups). Coordinated rewrite of the owned-`Ty` storage boundaries
that Phase 1 intentionally left, per
`openspec/changes/archive/2026-05-19-ty-interning-baseline/tasks.md` §6.1:

- Migrate `SymbolKind::{Var, Function, Type, Const, Static}` in
  `compiler/src/typeck/env.rs` to store `InternedTyId` instead of owned `Ty`.
  Touches 6 known cloners (catalogued at archive 6.1).
- Migrate `FunctionTy` / `MethodSig` / `ImplInfo` in
  `compiler/src/typeck/trait.rs` to `InternedTyId`-based fields.
- Consider folding `Ty.id` (per-instance origin tag) if no consumer reads it
  for non-debug purposes.

### P1-B: Runtime Module Splits

Reduce runtime module coupling without changing the extern C ABI.

Target files:

- `runtime/src/net.rs`: split TCP, UDP, HTTP client, HTTP server, and WebSocket
  surfaces.
- `runtime/src/reflect/runtime_ffi.rs`: split C libraries, objects, buffers,
  and callbacks.
- Restore `tools/stdlib/ffi.sg::ffi_buffer_from_bytes_raw` (a stdlib `.sg`
  wrapper, not an extern C symbol) from `-> Buffer` to `-> Result<Buffer, i64>`.
  **Gate now met** (2026-05-20): `ty-interning-baseline` shipped with all
  verification green, satisfying the archive 6.2 prerequisite.

### P2: Cyclic Async CFG

Seal the remaining async state-machine boundary for loop-heavy `await` bodies
with back-edges in the generated CFG.

Focus areas:

- while/for loops with multiple await points.
- liveness propagation across loop back-edges.
- clear diagnostics when a CFG shape is unsupported.

### P3: Toolchain DX

Polish the user-facing toolchain after the compiler/runtime debt is reduced:

- `sglsp` incremental responsiveness.
- `sgfmt` idempotent fixture CI.
- `sgpm` registry/git dependency sources, `cache`, `update`, and `publish`.

## Verification Baseline

Every priority must preserve:

```powershell
cargo test -p sengoo-compiler --lib
cargo test -p sgc
cargo test -p sengoo-runtime --lib
cargo test -p sgpm
```

Async/native changes must also preserve:

```powershell
cargo test -p sgc async_native_runtime_ -- --nocapture
cargo test -p sgc examples_smoke_reflection_ -- --nocapture
```

Examples coverage checkpoint:

```powershell
cargo test -p sgc examples_ -- --nocapture
```
