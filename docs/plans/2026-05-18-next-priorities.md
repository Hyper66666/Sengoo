# Sengoo Next Priorities (updated 2026-05-19)

## Current State

All earlier roadmap OpenSpec changes are shipped and archived. The active
change is now `openspec/changes/ty-interning-baseline/`, covering the P0
compiler type interning baseline.

The previous P0 item, `examples-coverage-expansion`, is complete:

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
| verify-mixed-width-type-correctness | Mixed-width integer type pipeline verification |

## Active Backlog

### P0: Ty Interning / Compiler Performance Baseline

Introduce `TyInterner` plus a compact `TyId` representation to reduce clone
pressure in the type checker and MIR lowering.

Initial target surface:

- `compiler/src/typeck/ty.rs`: define the interner shape and stable ID API.
- `compiler/src/typeck/infer.rs`: reduce cloning in unify, substitution, and
  fresh-variable paths.
- `compiler/src/typeck/check.rs`: route high-volume type construction through
  the interner without changing diagnostics.
- `compiler/src/mir/lowering.rs`: avoid deep type clones during generic and
  async frame instantiation where possible.

Constraints:

- Preserve existing public compiler APIs unless a narrower migration plan is
  written first.
- Adopt a two-phase migration: introduce `TyId` alongside the existing `Ty`
  type without removing `Ty`, then sweep call sites in subsequent passes.
- Keep diagnostics stable.
- Add regression tests for type equality, fresh variables, and substituted
  generic method calls before replacing clone-heavy paths.

### P1-A: Large File Splits

Split the largest non-test files while preserving behavior. Do this in small
reviewable moves, not broad rewrites.

Target files:

- `compiler/src/codegen/jit.rs`: split instruction emission, block emission,
  frame helpers, and runtime bridge helpers.
- `compiler/src/mir/lowering.rs`: split coercion, async-frame glue, and generic
  instantiation helpers.
- `tools/sgc/src/interface.rs` and `tools/sgc/src/commands.rs`: split by
  subcommand or interface concern.

Goal: no single non-test source file over 25 KB unless there is a documented
reason to keep it whole.

### P1-B: Runtime Module Splits

Reduce runtime module coupling without changing the extern C ABI.

Target files:

- `runtime/src/net.rs`: split TCP, UDP, HTTP client, HTTP server, and WebSocket
  surfaces.
- `runtime/src/reflect/runtime_ffi.rs`: split C libraries, objects, buffers,
  and callbacks.
- Restore `ffi_buffer_from_bytes_raw` (a stdlib `.sg` wrapper, not an
  extern C symbol) to `Result<Buffer, i64>` after Ty interning makes the
  type path cheaper and easier to reason about.

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
