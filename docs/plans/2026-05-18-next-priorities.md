# Sengoo Next Priorities (updated 2026-05-27)

## Current State

All earlier roadmap OpenSpec changes are shipped and archived, including the
most recent `ty-interning-baseline` (Phase 1 compiler type interning) which
landed 9 slices A–I across commits `7c39031c` → `1f7dedf3` and was archived to
`openspec/changes/archive/2026-05-19-ty-interning-baseline/`. A new
capability spec at `openspec/specs/interned-types/spec.md` is the first entry
in the previously empty `openspec/specs/` directory.

The P0 `Large File Splits` track has now shipped four starter slices.
`large-file-splits-runtime-db` is archived at
`openspec/changes/archive/2026-05-19-large-file-splits-runtime-db/`; it
split `runtime/src/reflect/runtime_db.rs` (978 LoC) into a 6-file directory
module with the largest resulting file at 431 LoC. `large-file-splits-jit-codegen`
completed on 2026-05-23 and is archived at
`openspec/changes/archive/2026-05-23-large-file-splits-jit-codegen/`; it
split `compiler/src/codegen/jit.rs` (1363 LoC) into a directory module where
`mod.rs` is 127 LoC and the largest resulting file, `instructions.rs`, is
857 LoC. Standard verification and the cast-semantics smoke stayed green.
`large-file-splits-mir-lowering` completed on 2026-05-23 and is archived at
`openspec/changes/archive/2026-05-26-large-file-splits-mir-lowering/`; it
split `compiler/src/mir/lowering.rs` (1501 LoC) into the existing helper
directory, leaving `mod.rs` at 162 LoC and the largest resulting file,
`builtin_helpers.rs`, at 580 LoC. `large-file-splits-sgc-interface-commands`
completed on 2026-05-27 and is archived at
`openspec/changes/archive/2026-05-27-large-file-splits-sgc-interface-commands/`;
it split `tools/sgc/src/interface.rs` (2274 LoC) into a directory module where
`mod.rs` is 24 LoC and the largest resulting file, `generic_instances.rs`, is
959 LoC, and split `tools/sgc/src/commands.rs` (1390 LoC) into a directory
module where `mod.rs` is 9 LoC and the largest resulting file, `build.rs`, is
582 LoC. The next recommended Large File Splits target is
`tools/sglsp/src/main.rs` plus `tools/sgfmt/src/main.rs`.

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
| large-file-splits-runtime-db | First Large File Splits starter; runtime_db directory module, largest file 431 LoC |
| large-file-splits-jit-codegen | JIT impl-block split; mod.rs 127 LoC, largest file instructions.rs 857 LoC |
| large-file-splits-mir-lowering | MIR lowering root split; mod.rs 162 LoC, largest file builtin_helpers.rs 580 LoC |
| mir-async-functions-shared-state | Rc<RefCell> for async_functions plus ConcreteTypeRegistry |
| large-file-splits-sgc-interface-commands | sgc interface and command directory modules; interface mod.rs 24 LoC, commands mod.rs 9 LoC, largest file generic_instances.rs 959 LoC |
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
reviewable moves, not broad rewrites.

Full non-test LoC leaderboard (measured 2026-05-20):

| LoC  | File                                          |
|-----:|-----------------------------------------------|
| 2729 | `runtime/src/net.rs`                          |
| 2274 | `tools/sgc/src/interface/` (split from interface.rs, COMPLETE 2026-05-27; mod.rs 24 LoC, largest file 959 LoC) |
| 2110 | `tools/sglsp/src/main.rs`                     |
| 1519 | `runtime/src/reflect/runtime_ffi.rs`          |
| 1501 | `compiler/src/mir/lowering/` (split from lowering.rs, COMPLETE 2026-05-23; mod.rs 162 LoC, largest file 580 LoC) |
| 1440 | `tools/sgc/src/pipeline.rs`                   |
| 1363 | `compiler/src/codegen/jit/` (split from jit.rs, COMPLETE 2026-05-23; largest file 857 LoC) |
| 1349 | `runtime/src/async_runtime.rs`                |
| 1348 | `tools/sgfmt/src/main.rs`                     |
| 1332 | `compiler/src/parser/decl.rs`                 |
| 1390 | `tools/sgc/src/commands/` (split from commands.rs, COMPLETE 2026-05-27; mod.rs 9 LoC, largest file 582 LoC) |
|  978 | `runtime/src/reflect/runtime_db/` (← starter, COMPLETE 2026-05-20: largest file now 431 LoC) |

Completed Large File Splits slices:

- `large-file-splits-runtime-db` (archived 2026-05-19): split
  `runtime/src/reflect/runtime_db.rs` into a 6-file directory module; largest
  resulting file is 431 LoC.
- `large-file-splits-jit-codegen` (archived 2026-05-23): split
  `compiler/src/codegen/jit.rs` into an 8-file directory module; `mod.rs` is
  127 LoC and largest resulting file `instructions.rs` is 857 LoC. This also
  extended the SOP to sibling inherent `impl JITCodegen` blocks.
- `large-file-splits-mir-lowering` (archived 2026-05-26): split
  `compiler/src/mir/lowering.rs` into the existing `lowering/` helper
  directory; `mod.rs` is 162 LoC and largest resulting file
  `builtin_helpers.rs` is 580 LoC. This extended the SOP to roots that already
  own child helper directories while keeping `LoweringContext` fields private
  in `mod.rs`.
- `large-file-splits-sgc-interface-commands` (archived 2026-05-27): split
  `tools/sgc/src/interface.rs` and `tools/sgc/src/commands.rs` into directory
  modules; `interface/mod.rs` is 24 LoC, `commands/mod.rs` is 9 LoC, and the
  largest resulting files are `interface/generic_instances.rs` at 959 LoC and
  `commands/build.rs` at 582 LoC. This extended the SOP to tooling command
  modules with stable CLI behavior and test-only re-export paths.

Recommended next order, smallest-clear-seam first to keep growing SOP coverage:

1. `tools/sglsp/src/main.rs` (2110 LoC) and `tools/sgfmt/src/main.rs`
   (1348 LoC) - both currently monolithic `main.rs` files.
2. `runtime/src/net.rs` (2729 LoC) - largest, defer until SOP is well-proven
   because of extern C ABI surface.
3. `runtime/src/reflect/runtime_ffi.rs` (1519 LoC) - next reflect runtime
   boundary after the earlier `runtime_db` split.

Goal: no single non-test source file over 25 KB (~1000 LoC) unless there is
a documented reason to keep it whole.

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
