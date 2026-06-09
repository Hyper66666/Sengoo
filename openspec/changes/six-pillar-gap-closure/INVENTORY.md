# Baseline Inventory (six-pillar-gap-closure)

## Pillar status snapshot (June 2026)

| Pillar | Priority | Current state | Primary evidence |
| --- | --- | --- | --- |
| 1 Stdlib MVP gap | High | Partial — sync subset works; Buffer/handle heavy | `examples/realworld/SUPPORT_MATRIX.md`, `tools/stdlib/*.sg` |
| 2 Async runtime | High | Cooperative subset; native sleep/spawn/select path is regression-tested | `docs/runtime-async-semantics.md`, `compiler/src/tests/async_tests.rs`, `tools/sgc/src/tests.rs::async_native_runtime_*` |
| 3 Package graph | High | path/git/registry/workspace; no alias/multi-version | `tools/sgpm/src/resolver.rs` error strings |
| 4 Language surface | High | Explicit parser/typeck limits | `compiler/src/parser/decl/object_declarations.rs` |
| 5 Compile perf | Medium-high | `--low-memory` helps; 1000k RSS 3.14x C++ | `README.md` benchmarks |
| 6 Toolchain UX | Medium-high | Workflow and typed asserts exist; real e2e, structured assert output, debugger/release docs remain | `tools/stdlib/assert.sg`, `mainstream-usable-loop` tests |

## Pillar 1 — Stdlib

| Capability | Status | Proof | Gap-closure target |
| --- | --- | --- | --- |
| Owned `String` type | Supported | `openspec/specs/owned-string-text/spec.md` | Return ABI from stdlib helpers |
| `Vec<i64>` / string maps | Supported subset | `tools/stdlib/collections.sg` | `Vec<String>`, string values |
| JSON handle API | Supported subset | `tools/stdlib/json.sg` | 1 MiB cap, `String` returns |
| Recursive dir ops | Deferred | SUPPORT_MATRIX | `dir_walk`, tree copy/remove |
| Process timeout/capture | Supported | `tools/stdlib/process.sg` | pipes + background policy |
| Sync stdin/stdout | Supported | `tools/stdlib/io.sg` | fd subset |
| Async fd IO | Deferred | SUPPORT_MATRIX | Pillar 2 reactor |

## Pillar 2 — Async

| Capability | Status | Proof |
| --- | --- | --- |
| spawn/join/sleep/timeout | Supported subset | `async_tests.rs`, native tests |
| 2-way select | Supported | compiler + runtime tests |
| N-way select | Deferred | SUPPORT_MATRIX |
| IO wakeups / reactor | Deferred | no `reactor.rs` |
| User `Future` trait | Deferred | compiler rejects custom awaitables |
| Future value flow | Restricted | `future values cannot escape` diagnostics |
| Windows native async link | Supported regression path | `cargo test -p sgc async_native_runtime`; dispatch/ref-local regressions in compiler and sgc tests |

## Pillar 3 — Package graph

| Capability | Status | Proof |
| --- | --- | --- |
| path/git/registry deps | Supported | `sgpm` integration tests |
| workspace + lockfile | Supported | `realworld_locked_project_loop_*` |
| `[[test]]` manifest | Supported | `sgpm` + `sgc test` |
| Renamed deps | **Unsupported** | resolver error text |
| Multi-version same name | **Unsupported** | resolver error text |
| Real e2e realworld | **Partial** | fake `sgc` in locked-loop test |

## Pillar 4 — Language surface

| Capability | Status | Proof |
| --- | --- | --- |
| Generics / traits via impl | Supported | compiler tests |
| derive on struct/enum/class | Supported subset | `derive_expander.rs` |
| Attributes on most decls | **Rejected** | `decl.rs` errors |
| `class A: Base, Trait` | **Rejected** | `class_header_trait_list_not_supported` |
| FFI arity 0..4 | Supported subset | `runtime_ffi.rs` |
| Async frame phase limits | Restricted | async diagnostic tests |

## Pillar 5 — Performance (README Feb 2026)

| Workload | Sengoo | C++ | Ratio |
| --- | --- | --- | --- |
| 1000k peak RSS (MB) | 1367.99 | 435.22 | **3.14x** |
| 1000k frontend share | 86.93% | — | target ≤65% |
| 1000k e2e time (ms) | 1827.84 | 4883.70 | faster (OK) |
| `--low-memory` RSS reduction | ~52% | — | mitigation only |

## Pillar 6 — Toolchain

| Capability | Status | Proof |
| --- | --- | --- |
| sgc/sgpm/sgfmt/sglsp | Supported | crate tests |
| JSON compile errors | Supported | `tools/sgc/tests/realworld.rs` |
| sglsp realworld fixtures | Supported | `tools/sglsp/src/stdlib.rs` |
| `sgc test` capture | Supported | `commands/test.rs` |
| Typed assert helpers | Supported subset | `tools/stdlib/assert.sg`, `examples/stdlib/21_assert.sg` |
| Structured assert failure output | **Missing** | current helpers terminate through generic panic; no assertion object in `sgc test` JSON |
| Debugger docs | **Missing** | — |
| Internal release docs | **Missing** | — |
| Real e2e CI | **Missing** | fake sgc in sgpm test |

## Required child changes

| Pillar | Child change | Delta ownership | Status |
| --- | --- | --- | --- |
| 1 | `stdlib-production-surface` | `stdlib-mainstream-usability`, `owned-string-text` | Review-fixed — dual canonical deltas |
| 2 | `async-reactor-futures` | `async-reactor-futures` (new canonical) | Review-fixed — self-contained spec + async restriction cleanup |
| 3 | `sgpm-alias-multiversion` | `sgpm-package-graph` (new canonical) | Review-fixed — lockfile v2 schema frozen |
| 4 | `language-surface-expansion` | `language-surface-expansion` (new canonical) | Review-fixed — independent of async change |
| 5 | `frontend-1000k-perf-gate` | `frontend-build-performance`, `frontend-compile-perf` | Review-fixed — dual canonical deltas |
| 6 | `toolchain-internal-ux` | `tooling-mainstream-ecosystem` (MODIFIED) | Review-fixed — archive `sgc-test-manifest-tooling` first |

The umbrella cannot be archived while any row remains `Not created`, `Active`,
or otherwise unarchived.

## Support-matrix baseline rows

The implementation wave intends to move these current rows:

- `Background process management`
- `Async IO wakeups`
- `User-defined Future support`
- `Multi-operand select`
- `Select loser cancellation`
- `Terminal/fd APIs`
- `Recursive file transfer`
- `Shell pipelines`
- `Owned string return boundaries`
- `Package/test/doc diagnostics`

Each status change must cite the child change and concrete proof path.

## Cross-pillar dependencies

```text
P6 (assert, e2e) → enables verification for P1–P5
P1 (String ABI) → P4 (FFI/string returns)
P2 (reactor) → P1 async fd (if in scope)
P3 (real e2e) → P6 CI
P5 (perf) → independent; gate in Phase 6
```

## Upstream changes consumed

- `mainstream-usable-loop` — realworld fixtures, matrix, LSP reduced fixtures
- `sgc-test-manifest-tooling` — test command, lockfile, JSON reports
- `runtime-hardening-ffi-async` — status taxonomy, handle safety
- `stdlib-next-usability-wave` / `stdlib-breadth-mainstream` — stdlib modules
- `owned-string-text` — String semantics baseline
- `frontend-build-performance` / `frontend-compile-perf` — cache + perf baselines
