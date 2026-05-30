# Sengoo Next Priorities (updated 2026-05-31)

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
582 LoC. `tools/sglsp/src/main.rs` has since completed its follow-up split:
diagnostics, formatting, workspace, semantic token, symbol, and signature
helpers now live in sibling modules. UTF-16 position conversion, incremental
text patches, and folding ranges have also moved into `text_editing.rs`,
leaving the production protocol root at 489 LoC and `text_editing.rs` at 187
LoC.
`tools/sgfmt/src/main.rs` has also completed its follow-up split: CLI config
and expression formatting now live in sibling modules. A 2026-05-31 pass moved
the reusable formatter API into `tools/sgfmt/src/lib.rs`, leaving the CLI root
at 65 LoC and allowing `sglsp` document formatting to share `sgfmt` behavior
for parseable buffers. `runtime/src/net.rs` has now been split by protocol family, leaving
the root at 992 LoC and the largest sibling at 733 LoC.
`runtime/src/reflect/runtime_ffi.rs` has now been split by handle family and
compatibility bridge, leaving the root at 750 LoC and the largest sibling,
`lua.rs`, at 372 LoC. `tools/sgc/src/pipeline.rs` has now been split by
pruning domain, leaving the root at 591 LoC, `ast_pruning.rs` at 428 LoC, and
`hir_pruning.rs` at 331 LoC. `runtime/src/async_runtime.rs` has now been split
by native bridge concern, leaving a 363 LoC production root and the largest
sibling, `bridge.rs`, at 223 LoC; embedded native-bridge tests remain in the
root file. `compiler/src/parser/decl.rs` has now been split by
declaration family, leaving the root at 476 LoC and the largest sibling,
`object_declarations.rs`, at 398 LoC. `runtime/src/reflect.rs` has now been split by native
loader concern, leaving the root at 819 LoC and the native loader sibling at
223 LoC. `compiler/src/hir/lower.rs` has now been split by HIR type and
expression lowering concern, leaving the root at 667 LoC and the largest
sibling, `expressions.rs`, at 406 LoC. `tools/sgc/src/bench.rs` has now
extracted the reflection and incremental benchmark flows, leaving the shared
benchmark root at 579 LoC, `incremental.rs` at 250 LoC, and `reflection.rs` at
223 LoC. `compiler/src/parser/expr.rs` has now
been split by aggregate and control-flow expression family, leaving the Pratt
root at 692 LoC, `control_flow.rs` at 207 LoC, and `aggregates.rs` at 126 LoC.
`compiler/src/lexer/token.rs`
has now extracted keyword metadata, leaving the token root at 762 LoC and
`keyword.rs` at 190 LoC. `tools/sgc/src/interface/generic_instances.rs` has
now extracted type-signature utilities and AST traversal, leaving the instance
collector root at 376 LoC, `collector.rs` at 493 LoC, and
`type_signatures.rs` at 157 LoC. The next recommended target is
`compiler/src/codegen/jit/instructions.rs`. Its aggregate-emission and complex
memory-emission branches have now moved into sibling helpers, leaving the
instruction dispatch root at 498 LoC, `memory_instructions.rs` at 260 LoC, and
`aggregate_instructions.rs` at 162 LoC. `tools/sgc/src/main.rs` has now
extracted compile-error reporting and documentation rendering, leaving the CLI
root at 672 LoC, `error_reporting.rs` at 182 LoC, and `doc_rendering.rs` at 125
LoC. The next recommended target is text editing in `tools/sglsp/src/main.rs`.

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
- `tools/stdlib/ffi.sg::ffi_buffer_from_bytes_raw` restored from
  `-> Buffer` to `-> Result<Buffer, i64>` (2026-05-29). The change also fixed
  typechecker generic-parameter freshening so `Result<Buffer, i64>` and other
  `Result<struct, i64>` wrappers do not collide with expression-inference
  variables.
- `tools/stdlib/collections.sg` now exposes runtime-backed `Vec<bool>` and
  bool/i64 `HashMap` mutators/query helpers on top of the existing i64 runtime
  storage (2026-05-29). Compiler stdlib surface tests and sgc native runtime
  smoke tests cover push/get/set/pop/contains/remove plus bool iterator
  adapters.

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

Selected non-test LoC leaderboard (remeasured 2026-05-30):

| LoC  | File                                          |
|-----:|-----------------------------------------------|
| 2480 | `runtime/src/net/` (split from net.rs, COMPLETE 2026-05-30; root 992 LoC, largest file http_server.rs 733 LoC) |
| 2274 | `tools/sgc/src/interface/` (split from interface.rs, COMPLETE 2026-05-27; mod.rs 24 LoC, largest file 959 LoC) |
|  489 | `tools/sglsp/src/main.rs` (follow-up split complete: production root 489 LoC, embedded tests remain in main.rs; diagnostics 575, formatting 32, workspace 239, semantic 205, symbols 187, signatures 225, text_editing 187) |
|  750 | `runtime/src/reflect/runtime_ffi/` (split from runtime_ffi.rs, COMPLETE 2026-05-30; root 750 LoC, largest file lua.rs 372 LoC) |
| 1501 | `compiler/src/mir/lowering/` (split from lowering.rs, COMPLETE 2026-05-23; mod.rs 162 LoC, largest file 580 LoC) |
|  591 | `tools/sgc/src/pipeline/` (split from pipeline.rs, COMPLETE 2026-05-30; root 591 LoC, ast_pruning.rs 428 LoC, hir_pruning.rs 331 LoC) |
|  672 | `tools/sgc/src/main.rs` (follow-up split complete 2026-05-30; root 672 LoC, error_reporting.rs 182 LoC, doc_rendering.rs 125 LoC) |
| 1363 | `compiler/src/codegen/jit/` (split from jit.rs, COMPLETE 2026-05-23; follow-up aggregate and complex memory emission splits 2026-05-30; largest file instructions.rs 498 LoC, memory_instructions.rs 260 LoC) |
|  363 | `runtime/src/async_runtime/` (split from async_runtime.rs, COMPLETE 2026-05-30; production root 363 LoC with embedded native-bridge tests, largest sibling bridge.rs 223 LoC) |
|  740 | `tools/sgfmt/src/lib.rs` (follow-up split complete: CLI root 65 LoC, config 81, expressions 397; shared formatter API feeds sgfmt and sglsp) |
|  476 | `compiler/src/parser/decl/` (split from decl.rs, COMPLETE 2026-05-30; root 476 LoC, object_declarations.rs 398 LoC, ffi.rs 269 LoC, data_declarations.rs 178 LoC) |
|  819 | `runtime/src/reflect/` (split from reflect.rs, COMPLETE 2026-05-30; root 819 LoC, native.rs 223 LoC) |
|  667 | `compiler/src/hir/lower/` (split from lower.rs, COMPLETE 2026-05-30; root 667 LoC, largest file expressions.rs 406 LoC) |
|  579 | `tools/sgc/src/bench/` (split from bench.rs, COMPLETE 2026-05-30; root 579 LoC, incremental.rs 250 LoC, reflection.rs 223 LoC) |
|  692 | `compiler/src/parser/expr/` (split from expr.rs, COMPLETE 2026-05-30; root 692 LoC, control_flow.rs 207 LoC, aggregates.rs 126 LoC) |
|  762 | `compiler/src/lexer/token/` (split from token.rs, COMPLETE 2026-05-30; root 762 LoC, keyword.rs 190 LoC) |
|  493 | `tools/sgc/src/interface/generic_instances/` (split from generic_instances.rs, COMPLETE 2026-05-30; root 376 LoC, collector.rs 493 LoC, type_signatures.rs 157 LoC) |
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
- `tools/sglsp/src/main.rs` follow-up split (completed 2026-05-30): extracted
  diagnostics, formatting, workspace, semantic token, symbol, and signature
  helpers into sibling modules. A second pass extracted UTF-16 position
  conversion, incremental content changes, and folding ranges into
  `text_editing.rs`, leaving the production protocol orchestration root at 489
  LoC and the text-editing sibling at 187 LoC. Embedded tests remain in
  `main.rs`.
- `tools/sgfmt/src/main.rs` follow-up split (completed 2026-05-30, updated
  2026-05-31): extracted CLI config and expression formatting into sibling
  modules, then moved the reusable formatter implementation into
  `tools/sgfmt/src/lib.rs`. The CLI root is now 65 LoC; the shared formatter
  library is 740 LoC, `config.rs` is 81 LoC, and `expressions.rs` is 397 LoC.
- `runtime/src/net.rs` follow-up split (completed 2026-05-30): extracted TCP,
  UDP, HTTP client, HTTP server, and WebSocket protocol modules, preserving
  the extern C ABI while leaving the shared runtime root at 992 LoC and the
  largest sibling, `http_server.rs`, at 733 LoC.
- `runtime/src/reflect/runtime_ffi.rs` follow-up split (completed 2026-05-30):
  extracted buffer, callback, object, and Lua compatibility modules,
  preserving the extern C ABI and pre-split Rust paths while leaving the
  shared dynamic-library root at 750 LoC and the largest sibling, `lua.rs`,
  at 372 LoC.
- `tools/sgc/src/pipeline.rs` follow-up split (completed 2026-05-30): extracted
  AST, HIR, and MIR reachability pruning into sibling modules, preserving
  frontend phase ordering and phase timing keys while leaving the orchestration
  root at 591 LoC, `ast_pruning.rs` at 428 LoC, and `hir_pruning.rs` at 331
  LoC.
- `runtime/src/async_runtime.rs` follow-up split (completed 2026-05-30):
  extracted native scheduler bridge, sleep/timeout futures, and select
  helpers, preserving the extern C ABI while leaving the shared scheduler
  production root at 363 LoC and the largest sibling, `bridge.rs`, at 223 LoC.
  Embedded native-bridge tests remain in the root file.
- `compiler/src/parser/decl.rs` follow-up split (completed 2026-05-30):
  extracted extern/FFI declarations, simple leaf declarations, struct/enum
  data declarations, and class/trait/impl declarations. Top-level dispatch and
  diagnostics remain stable while the shared declaration root is now 476 LoC
  and the largest sibling, `object_declarations.rs`, is 398 LoC.
- `runtime/src/reflect.rs` follow-up split (completed 2026-05-30): extracted
  platform dynamic-library loading and i64 native binding adaptation,
  preserving the shared loader path used by FFI and Lua bridges while leaving
  the reflection root at 819 LoC and `native.rs` at 223 LoC.
- `compiler/src/hir/lower.rs` follow-up split (completed 2026-05-30): extracted
  HIR type inference and expression lowering helpers, preserving the existing
  lambda parameter inference behavior while leaving the declaration lowering
  root at 667 LoC and the largest sibling, `expressions.rs`, at 406 LoC.
- `tools/sgc/src/bench.rs` follow-up split (completed 2026-05-30): extracted
  the reflection and incremental benchmark flows, preserving CLI entrypoints
  while leaving shared benchmark utilities and the remaining flows in a 579
  LoC root. `incremental.rs` is 250 LoC and `reflection.rs` is 223 LoC.
- `compiler/src/parser/expr.rs` follow-up split (completed 2026-05-30):
  extracted array, path-call, and struct-literal parsing into an aggregate
  expression sibling, then extracted block, control-flow, lambda, async, and
  parallel parsing into a second sibling. Pratt parsing and diagnostics remain
  stable while the shared expression root is now 692 LoC,
  `control_flow.rs` is 207 LoC, and `aggregates.rs` is 126 LoC.
- `compiler/src/lexer/token.rs` follow-up split (completed 2026-05-30):
  extracted public keyword metadata and lookup formatting into a sibling
  module, preserving the `lexer::Keyword` API while leaving the token root at
  762 LoC and `keyword.rs` at 190 LoC.
- `tools/sgc/src/interface/generic_instances.rs` follow-up split (completed
  2026-05-30): extracted type-signature splitting, inference, substitution,
  unification utilities, and AST traversal, preserving instance insertion
  order and fingerprint sorting while leaving the root at 376 LoC,
  `collector.rs` at 493 LoC, and `type_signatures.rs` at 157 LoC.
- `compiler/src/codegen/jit/instructions.rs` follow-up split (completed
  2026-05-30): extracted aggregate instruction emission for arrays, tuples,
  and enums, preserving IR order and diagnostics while leaving the
  instruction dispatch root at 737 LoC and `aggregate_instructions.rs` at 160
  LoC.
- `compiler/src/codegen/jit/instructions.rs` memory follow-up split (completed
  2026-05-30): extracted the complex `Store` and `IndexAddr` instruction
  emitters, preserving pointer, aggregate-copy, and index-address IR templates
  while leaving the instruction dispatch root at 498 LoC and
  `memory_instructions.rs` at 260 LoC.
- `tools/sgc/src/main.rs` follow-up split (completed 2026-05-30): extracted
  compile-error JSON/location rendering and documentation HTML rendering,
  preserving CLI-facing error paths and `sgc doc` output while leaving the CLI
  root at 672 LoC, `error_reporting.rs` at 182 LoC, and `doc_rendering.rs` at
  125 LoC.

Recommended next order, smallest-clear-seam first to keep growing SOP coverage:

1. `compiler/src/typeck/check.rs` (973 production LoC, 37 KB) - split only
   after the active semantic edits settle; keep this separate from behavior
   changes.
2. `compiler/src/codegen/instruction_helpers.rs` (996 production LoC, 36 KB) -
   split only after the active codegen edits settle; preserve emitted IR
   templates byte-for-byte.

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
  surfaces (completed 2026-05-30).
- `runtime/src/reflect/runtime_ffi.rs`: split buffers, callbacks, objects, and
  the Lua compatibility bridge while leaving shared C-library invocation in
  the root (completed 2026-05-30).
- `runtime/src/async_runtime.rs`: split native scheduler bridge, future
  implementations, and select helpers while leaving the reusable scheduler
  core in the root (completed 2026-05-30).
- `tools/stdlib/ffi.sg::ffi_buffer_from_bytes_raw` (a stdlib `.sg` wrapper, not
  an extern C symbol) was restored from `-> Buffer` to
  `-> Result<Buffer, i64>` on 2026-05-29. The restoration required unifying
  generic type-parameter freshening through `TypeInfer` so stdlib
  `Result<struct, i64>` wrappers remain type-safe after earlier expression
  inference binds numeric locals.

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
- `sgc doc <file.sg> --output <dir>` now exposes the existing rustdoc-like API
  documentation generator through the CLI (2026-05-30).
- `std::math` and `std::error` now have curated runnable examples and `sgc`
  smoke coverage alongside the existing `std::string` example (2026-05-30).
- `sgpm init [name] [--path <dir>]` now initializes existing directories,
  derives the package name from the directory by default, preserves unrelated
  files, and refuses to overwrite scaffold files (2026-05-30).
- `sgpm new --lib` and `sgpm init --lib` now scaffold reusable library packages
  with `[lib] path = "src/lib.sg"` (2026-05-30).
- `sgc` now expands relative source imports dependency-first, and `sgpm`
  exposes resolved `[lib]` package entries as importable modules while
  type-checking pure library nodes before dependent binary builds; `sgpm run`
  now rejects pure library roots with an actionable `[bin]` hint (2026-05-30).
- `sgc` now rejects unresolved ordinary and `std::*` imports instead of
  silently ignoring them. Frontend graph probes expand each module's semantic
  imports while retaining raw module fingerprints, so cross-module generic
  stdlib types and mapped package reflection imports participate correctly in
  incremental analysis. `sgpm` also rejects dependency keys that differ from
  the target `[package].name` until renamed aliases are implemented
  (2026-05-30).
- `sgpm test` now exposes a library package's own `[lib]` entry to its
  `tests/*.sg` files, so package tests can import the public module directly
  (2026-05-30).
- `sgpm` now rejects dependency graphs where one package name resolves to
  multiple manifests, preventing silent `SENGOO_MODULE_MAP` overwrites until
  renamed or multi-version dependencies are implemented (2026-05-30).
- `sgpm` now maps dual-target package imports to `[lib].path` while retaining
  `[bin].path` as the build and run entry, so packages can ship a CLI and a
  reusable library together (2026-05-30).
- `sgpm` now rejects missing, absolute, and package-root-escaping `[bin]` and
  `[lib]` entries during dependency resolution, preventing unusable package
  archives and inconsistent command behavior (2026-05-30).
- `sgpm` now validates remote registry cache hits before reuse and downloads an
  incomplete cached package again automatically (2026-05-30).
- Remote registry downloads now unpack and validate in sibling staging
  directories before replacing a cache version, so failed archive extraction
  does not expose partial cache state (2026-05-30).
- Git dependency clones and refreshes now complete in sibling staging paths,
  incomplete checkout caches are rebuilt automatically before use, and broken
  refreshed packages cannot replace a previously valid checkout (2026-05-30).
- `sgpm cache list` now reports downloaded remote registry package versions,
  and `sgpm cache clean --registry` removes that cache explicitly without
  touching normal build artifacts (2026-05-30).
- `sgpm` now applies the scaffold package-name grammar to hand-written
  manifests, dependency keys, and optional binary names, preventing path-like
  names from escaping output or local registry directories (2026-05-30).
- `sgpm fmt` now formats both package sources and `tests/**/*.sg` files, so the
  package formatting workflow covers the same first-class test tree that
  `sgpm test` executes (2026-05-31).
- Registry configuration keys and dependency selectors now require
  alphanumeric boundaries with lowercase ASCII letters, digits, `_`, `-`, or
  `.` internally, preventing ambiguous lockfile identifiers and remote-cache
  path collisions (2026-05-30).
- `sgpm` now rejects duplicate workspace member package names during member
  loading, so all-member commands and workspace lockfiles retain unambiguous
  roots (2026-05-30).
- `std::net` now exposes the existing HTTP server runtime through `HttpServer`
  wrappers for bind, limits, static routes, required-header middleware, WS
  echo routes, serve-once timeout handling, and close (2026-05-30).
- Generic free functions now materialize concrete MIR/IR instances at call
  sites, so stdlib helpers can expose normal generic constructors instead of
  only hand-written scalar entry points. `std::option` now provides
  `option_some<T>` / `option_none_with<T>`, `std::result` provides
  `result_ok_with<T, E>` / `result_err_with<T, E>`, and a runnable
  `examples/stdlib/04_option_result.sg` smoke test covers bool constructors
  plus projection helpers (2026-05-31).
- `std::option` now also exposes `option_some_bool` / `option_none_bool`, and
  `std::result` exposes `result_ok_bool` / `result_err_bool` for
  `Result<bool, i64>`. Bool `Vec`/`HashMap` helpers reuse these constructors
  instead of hand-writing tagged `Option<bool>` literals, with compiler
  surface and `sgc` runtime smoke coverage (2026-05-31).
- `Option<bool>` and `Result<bool, i64>` now provide `unwrap` and `expect`
  specializations alongside the existing `unwrap_or`, matching the i64
  convenience surface for common success-path use (2026-05-31).
- `sglsp` completion and hover now use the same workspace document set as
  definition, references, rename, and workspace-symbol queries. Imported
  `std::*` modules are also indexed from embedded stdlib sources so standard
  library symbols complete and hover correctly even outside the Sengoo source
  checkout (2026-05-31).
- `sglsp` signature help now also searches workspace documents and imported
  stdlib modules, while signature labels render AST types directly instead of
  relying on token spans that can include trailing punctuation (2026-05-31).
- `sglsp` stdlib completion/signature dependencies now mirror `sgc` stdlib
  preloading for reflection modules: importing `std::ffi`, `std::db`,
  `std::lua54`, `std::net`, or `std::proto` also exposes the `Option`/`Result`
  family needed by those wrappers (2026-05-31).
- `sglsp` document/range formatting now reuses the shared `sgfmt` formatter API
  for parseable buffers, while retaining trailing-whitespace cleanup as the
  fallback for incomplete in-editor source (2026-05-31).
- `sglsp` range formatting now returns edits limited to the requested line
  span instead of replacing the entire document, using `sgfmt` output when line
  mapping is stable and a whitespace-only fallback otherwise (2026-05-31).
- `sglsp` compiler diagnostics now fall back to the embedded
  `sengoo-compiler` pipeline when `sgc --error-format json` cannot be started,
  so editors still receive parse/type errors even when `sgc` is missing from
  PATH (2026-05-31).
- `sglsp` also falls back to embedded compiler diagnostics when a discovered
  `sgc` exits with non-JSON error text, covering stale tools or mismatched PATH
  entries that do not support `--error-format json` (2026-05-31).
- `sgpm doc` now exposes package-level API documentation generation through
  `sgc doc`, including workspace/package selection, lockfile checking, and
  `[lib]`-first entry selection for reusable packages (2026-05-31).
- `sgpm test` now forwards profile optimization flags explicitly:
  debug uses `sgc run -O 0`, and `--release` uses `sgc run -O 2`
  (2026-05-29).
- `sgpm publish --dry-run` now creates a root-package `.tar.gz` artifact and
  `.sha256` checksum under `target/package/`, excluding build artifacts
  (2026-05-29).
- `sgpm update` now writes a generated `Sengoo.lock` snapshot for the resolved
  package graph, including package versions, `path+...` sources,
  `git+...#<commit>` sources, registry sources, manifest paths, and direct dependencies
  (2026-05-29).
- `sgpm update --check` now verifies `Sengoo.lock` freshness without rewriting
  it, giving CI a lockfile drift check for local path and git dependency graphs
  (2026-05-29).
- `sgpm update` now stages generated lockfiles beside the final path before
  replacement, preserving the previous snapshot when staging or replacement
  fails (2026-05-30).
- `sgpm <command> --locked` execution is now available on package graph commands
  (`build`, `check`, `run`, `test`, `fmt`, `tree`, and `publish`) so stale
  lockfiles fail before delegated `sgc`/`sgfmt` invocation or packaging
  (2026-05-29).
- `sgpm` now resolves git dependencies through a root-package
  `target/sgpm/git` cache and records resolved git commits in `Sengoo.lock`
  (2026-05-29).
- `sgpm update --refresh` now reclones git dependency caches before writing
  `Sengoo.lock`, giving branch/local-source dependency graphs an explicit
  refresh control (2026-05-29).
- `sgpm cache list` and `sgpm cache clean --git` now expose and prune the root
  package git dependency cache under `target/sgpm/git` (2026-05-29).
- `sgpm` now supports root-level local file registries via `[registries.<name>]`
  with semver dependency constraints, highest matching version selection,
  lockfile `registry+<registry>/<package>@<version>` sources, and actionable
  same-package version-conflict diagnostics (2026-05-29).
- `sgpm publish --registry <name>` now publishes the root package into a
  configured local file registry, excludes `.git/`, `target/`, and registry
  output directories, and refuses to overwrite an existing package version
  (2026-05-29).
- Local registry publish now stages source copies beside the final version
  directory, renames only completed packages into place, and removes failed
  staging directories so interrupted copies do not block retries (2026-05-30).
- Package file enumeration now propagates traversal errors instead of silently
  omitting unreadable paths from dry-run archives, local registry copies, or
  remote upload artifacts (2026-05-30).
- `sgpm test` and `sgpm fmt` source enumeration now propagates traversal errors
  instead of silently skipping unreadable `.sg` files (2026-05-30).
- `sgpm` now supports `[workspace].members`, direct-child `/*` member
  expansion, workspace-level local registry inheritance, and `--package <name>`
  member selection across package graph commands (2026-05-29).
- `sgpm` now supports `--workspace` all-member execution for `build`, `check`,
  `test`, `fmt`, `tree`, `clean`, and `update`; `update --workspace` writes one
  root `Sengoo.lock` snapshot for all members (2026-05-30).
- `sgpm` remote registries now support cached package fetches, optional bearer
  auth via `token_env`, checksum verification, and package archive upload
  (2026-05-30).
- Further `sgpm` work should follow concrete registry protocol compatibility
  and workflow feedback; the earlier remote-registry and aggregate-lockfile
  placeholders are complete.

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
