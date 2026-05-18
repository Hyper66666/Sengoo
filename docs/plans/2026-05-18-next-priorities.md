# Sengoo Next Priorities (2026-05-18)

## Current State

All prior roadmap items are shipped and their OpenSpec changes archived.
Only `openspec/changes/examples-coverage-expansion` remains in-flight.

### Shipped (archived)

| Change | Summary |
|---|---|
| async-native-execution-and-ci-smoke | Async lowering, native runtime bridge, CI smoke |
| async-phase-2-features | async block, spawn, join, select core surface |
| async-runtime-hardening-and-lowering-split | Future-escape checks, dispatch IDs, lowering split |
| enforce-struct-field-completeness | Struct literal missing/duplicate/unknown field validation |
| mir-async-functions-shared-state | Rc<RefCell> for async_functions + ConcreteTypeRegistry |
| mir-bitcast-instruction | MIR Bitcast for float async frame support |
| reflection-runtime-sengoo-wrappers | Sengoo-side wrappers for db/ffi/lua54/proto/net |
| sgpm-mvp-implementation | sgpm package manager MVP with path deps |
| stdlib-generic-support | Option/Result/Vec/HashMap generic surface consolidation |
| stdlib-module-decomposition | Split collections.sg into per-topic modules |
| toolchain-language-runtime-roadmap | sglsp, sgfmt, sgpm, generics, macros, docs |
| verify-mixed-width-type-correctness | Mixed-width integer type pipeline verification |

### Still In-Flight

| Change | Status |
|---|---|
| examples-coverage-expansion | Proposed — async/generics/traits/ffi example demos + CI smoke |

## Next Priorities

### P0: Examples Coverage Expansion (1–2 weeks)

Implement the proposed `examples-coverage-expansion` change:
- Add `examples/async/`, `examples/generics/`, `examples/traits/` demos
- Harden `examples/ffi/` with Makefile and README
- Add `examples_smoke_*` CI gate in `tools/sgc/src/tests.rs`
- Cross-link from both READMEs

### P1-A: Ty Interning — Compiler Performance Baseline (2–4 weeks)

Introduce `TyInterner` + `TyId(NonZeroU32)` to eliminate clone hotspots in:
- `compiler/src/typeck/infer.rs` and `check.rs` (unify, subst, fresh_var)
- `compiler/src/mir/lowering.rs` (type instantiation)

Builds on the `Rc<RefCell>` pattern proven in `mir-async-functions-shared-state`.

### P1-B: Large File Splits (parallel with P1-A)

Target files and expected splits:
- `compiler/src/codegen/jit.rs` (60 KB) → instruction / block / frame / bridge
- `compiler/src/mir/lowering.rs` (60 KB) → coercion / async_frame / generic_instantiation
- `tools/sgc/src/interface.rs` (77 KB) + `commands.rs` (51 KB) → per-subcommand modules

Goal: no single non-test source file > 25 KB.

### P1.5: Runtime Module Splits

- `runtime/src/net.rs` (90 KB) → tcp / udp / http_client / http_server / ws
- `runtime/src/reflect/runtime_ffi.rs` (45 KB) → c_libraries / objects / buffers / callbacks
- Restore `ffi_buffer_from_bytes_raw` to `Result<Buffer, i64>` after Ty interning lands

### P2: Cyclic Async CFG

Seal the largest remaining async boundary: loop-heavy `await` bodies with
back-edges in the state-machine CFG.

### P3: Toolchain DX

- `sglsp` incremental responsiveness
- `sgfmt` idempotent fixture CI
- `sgpm` registry/git dependency sources, `cache`, `update`, `publish`

## Verification Baseline

Every priority must preserve:

```powershell
cargo test -p sengoo-compiler --lib
cargo test -p sgc
cargo test -p sengoo-runtime --lib
cargo test -p sgpm
```

Async/native changes also preserve:

```powershell
cargo test -p sgc async_native_runtime_ -- --nocapture
cargo test -p sgc examples_smoke_reflection_ -- --nocapture
```
