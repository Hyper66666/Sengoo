# Current Inventory (six-pillar-gap-closure)

## Status Snapshot (June 2026)

This umbrella has mostly been consumed by archived child changes. It remains
active because the 1000k frontend performance child is still open, POSIX
reference-host evidence still needs CI confirmation for the newest staticlib/net
linking path, and the umbrella-wide final verification has not been completed in
one pass.

| Pillar | Priority | Current state | Primary evidence |
| --- | --- | --- | --- |
| 1 Stdlib MVP gap | High | Strong internal-tooling subset; recursive tree and fd helpers remain accepted-risk until fixture-backed | `openspec/specs/stdlib-mainstream-usability/spec.md`, `examples/realworld/SUPPORT_MATRIX.md` |
| 2 Async runtime | High | Supported subset: reactor hints, user Future lowering, 2..8 select, `select_cancel`, task cancellation boundaries, timeout_cancel, channels/mutex; all-host owned-fd readiness remains deferred | `openspec/specs/async-reactor-futures/spec.md`, `openspec/specs/async-default-followups/spec.md`, `docs/runtime-async-semantics.md` |
| 3 Package graph | High | Alias dependencies, lockfile v2, multi-version registry resolution, metadata JSON, and release fixture are implemented | `openspec/specs/sgpm-package-graph/spec.md`, `examples/realworld/package-release-loop` |
| 4 Language surface | High | Attributes/cfg/deprecated diagnostics, class trait headers, and dynamic native i64 arity 0..8 are implemented; broader FFI and payload enum frame widening are deferred | `openspec/specs/language-surface-expansion/spec.md`, `openspec/specs/language-default-polish/spec.md` |
| 5 Compile perf | Medium-high | Regression gates and low-memory mitigation exist; 1000k absolute RSS/share target remains open | `openspec/changes/frontend-1000k-perf-gate/tasks.md`, `bench/FRONTEND_BASELINE.md` |
| 6 Toolchain UX | Medium-high | Structured assertion output, realworld e2e, debugger/editor/release docs, and LSP realworld coverage exist | `openspec/specs/tooling-mainstream-ecosystem/spec.md`, `.github/workflows/realworld-e2e.yml` |

## Pillar 1 - Stdlib

| Capability | Status | Proof | Remaining gap |
| --- | --- | --- | --- |
| Owned `String` type and return boundaries | Supported subset | `tools/stdlib/string.sg`; `path_join_string`, `json_value_as_string`; SUPPORT_MATRIX owned string row | Broader owned wrapper migration remains future work |
| `Vec<i64>` / text collections | Supported subset | `tools/stdlib/collections.sg`, `runtime_collections.c`, LSP stdlib tests | Richer generic collections remain future work |
| JSON handle API | Supported subset | `tools/stdlib/json.sg`; 1 MiB cap and owned string reads in canonical stdlib spec | Streaming/schema/JSON5 remain deferred |
| Recursive dir ops | Accepted risk | `dir_walk`, `dir_copy_tree`, `dir_remove_tree`; stdlib/runtime tests; SUPPORT_MATRIX row | No committed realworld recursive-tree fixture yet |
| Process timeout/capture/pipes/background | Supported subset | `workspace-doc-loop`, `ProcessCommand.pipe_stdout_to`, `ProcessCommand.spawn`, `ProcessHandle.wait_cancellable`, stdlib process tests | Richer async process orchestration remains future work |
| Sync fd IO | Accepted risk | `io_fd_read`, `io_fd_write`; runtime tests; platform behavior docs | No committed realworld fd fixture yet |
| Async fd IO | Deferred | SUPPORT_MATRIX all-host owned-fd row | Needs future all-host reactor evidence |

## Pillar 2 - Async

| Capability | Status | Proof | Remaining gap |
| --- | --- | --- | --- |
| spawn/join/sleep/timeout | Supported subset | `compiler/src/tests/async_tests.rs`, `tools/sgc/src/tests.rs::async_native_runtime_*` | POSIX reference-host rerun remains useful |
| Reactor timer/TCP wakeups | Supported subset | SUPPORT_MATRIX reactor row; runtime `reactor.rs` tests | All-host owned-fd readiness still deferred |
| User `Future` trait / await lowering | Supported subset | `user_future_impl_can_be_awaited_and_lowers_poll_loop`, native user-future tests | Exhaustive negative JSON/LSP snapshots and wakeup-registration diagnostics deferred |
| Future value flow | Supported subset | local/parameter/return user-future tests | Cross-thread/unsafe escapes remain rejected |
| 2..8 select and `select_cancel` | Supported subset | compiler/runtime select tests; SUPPORT_MATRIX select rows; PR #17 checks | Existing non-canceling `select` remains unchanged |
| Task cancellation boundaries | Supported subset | SUPPORT_MATRIX task cancellation row; `async_native_runtime_cancel_task_prevents_post_await_code`; PR #17 checks | Richer cancellation propagation APIs remain future work |
| Windows native async link | Supported regression path | `cargo test -p sgc async_native_runtime`; fallback dispatch symbols in `runtime.c` | Linux CI should re-prove staticlib extraction/weak-symbol path |

## Pillar 3 - Package Graph

| Capability | Status | Proof |
| --- | --- | --- |
| path/git/registry deps | Supported | `tools/sgpm/tests/integration.rs` |
| workspace + lockfile | Supported | realworld locked loop tests |
| `[[test]]` manifest | Supported | `sgpm` + `sgc test` |
| Renamed deps | Supported | `package = "actual_name"` manifest parsing and resolver tests |
| Multi-version same name | Supported subset | `registry_dependency_multiversion_keeps_both_selected_versions`; `package-release-loop` shared_core 1.x/2.x |
| Metadata alias mapping | Supported | `metadata_json_alias_lists_dependency_edges_separately` |
| Real e2e realworld | Supported subset | `tools/sgpm/tests/realworld_e2e.rs`; `.github/workflows/realworld-e2e.yml` |

## Pillar 4 - Language Surface

| Capability | Status | Proof | Remaining gap |
| --- | --- | --- | --- |
| Generics / traits via impl | Supported | compiler tests | N/A for this umbrella |
| derive on struct/enum/class | Supported subset | language-surface/default-polish specs and tests | Derive breadth remains additive future work |
| `#[cfg]` / `#[deprecated]` diagnostics | Supported subset | `language-default-polish` archived tasks; `sglsp` diagnostics | Feature-selection CLI remains deferred |
| `class A: Base, Trait` | Supported subset | `object_declarations.rs`, trait dispatch tests | Multiple base classes remain rejected |
| Dynamic native i64 FFI arity | Supported subset | arity 0..8 in `language-surface-expansion`; runtime hardening tests | Aggregate/callback/owned String ABI broadening remains deferred |
| Async frame phase limits | Partially relaxed | user Future and async diagnostics tests | Payload enum across await remains deferred |

## Pillar 5 - Performance

| Workload | Current evidence | Status |
| --- | --- | --- |
| 1000k peak RSS target | Baseline 3.14x C++; target <= 1.8x | Open |
| 1000k frontend share target | Baseline 86.93%; target <= 65% | Open |
| 1000k e2e time vs C++ | Faster in baseline | Met but not sufficient |
| `--low-memory` RSS reduction | ~52% / 672 MB in README baseline | Mitigation only; not archive evidence |
| CI regression gates | `.github/workflows/perf-smoke.yml`, `bench/scripts/advanced-kpi-gate.py` | Regression gate exists; absolute target informational until met |

## Pillar 6 - Toolchain

| Capability | Status | Proof |
| --- | --- | --- |
| sgc/sgpm/sgfmt/sglsp | Supported | crate tests and realworld-e2e |
| JSON compile errors | Supported | `tools/sgc/tests/realworld.rs` |
| sglsp realworld fixtures | Supported | `tools/sglsp/src/stdlib.rs`, diagnostics/formatting realworld tests |
| `sgc test` capture | Supported | `tools/sgc/src/commands/test.rs` |
| Structured assert failure output | Supported subset | `tools/sgc/tests/assertion_transport.rs`, `SENGOO_ASSERT_REPORT` runner path |
| Debugger/editor/release docs | Supported docs | `docs/debugging-native.md`, `docs/editor-setup.md`, `docs/internal-release.md` |
| Real e2e CI | Supported subset | `.github/workflows/realworld-e2e.yml`; `tools/sgpm/tests/realworld_e2e.rs` |

## Child Change Status

| Pillar | Child change | Delta ownership | Status |
| --- | --- | --- | --- |
| 1 | `stdlib-production-surface` | `stdlib-mainstream-usability`, `owned-string-text` | Archived 2026-06-08 |
| 2 | `async-reactor-futures` | `async-reactor-futures` | Archived 2026-06-08 |
| 3 | `sgpm-alias-multiversion` | `sgpm-package-graph` | Archived 2026-06-08; later expanded by `package-release-defaults` |
| 4 | `language-surface-expansion` | `language-surface-expansion` | Archived 2026-06-08; later polished by `language-default-polish` |
| 5 | `frontend-1000k-perf-gate` | `frontend-build-performance`, `frontend-compile-perf` | Active; absolute 1000k gate open |
| 6 | `toolchain-internal-ux` | `tooling-mainstream-ecosystem` | Archived 2026-06-08 |

## Additional Archived Follow-Ups Consumed

- `async-default-followups` - archived 2026-06-10; support-matrix rows split supported subsets from deferred all-host owned-fd work.
- `async-cancellation-semantics` - archived 2026-06-12; task cancellation, `select_cancel`, and process `wait_cancellable` are supported subsets with PR #17 Windows/Ubuntu evidence.
- `language-default-polish` - archived 2026-06-10; diagnostic parity and still-rejected adjacent language forms recorded.
- `mainstream-default-readiness` - archived 2026-06-10; default-readiness inventory promoted to canonical spec.
- `mainstream-production-readiness` - archived 2026-06-10; front-five status promoted to canonical spec.
- `package-release-defaults` - archived 2026-06-10; release fixture, deterministic publish, metadata, and realworld release loop landed.
- `stdlib-breadth-mainstream` - archived 2026-06-10; canonical stdlib breadth updates consumed.
- `stdlib-default-followups` - archived 2026-06-10; compression support subset landed.
- `stdlib-http-server-handlers` - archived 2026-06-10; pull-based dynamic HTTP server subset landed.
- `stdlib-https-tls` - archived 2026-06-10 with POSIX/reference-host evidence still called out as required for confidence.

## Current Remaining Blockers Before Umbrella Archive

- `frontend-1000k-perf-gate` tasks 3.3 and archive gate remain open until the
  reference host proves 100k/1000k results and the 1000k RSS/frontend-share
  absolute targets pass or the spec is explicitly superseded.
- POSIX CI must run the new staticlib/native-net path through `realworld-e2e`
  and TLS tests. Windows local evidence exists, but Linux weak-symbol extraction
  is the risk the CI matrix must close.
- The umbrella verification section still needs one final green pass or explicit
  evidenced skips; do not archive the umbrella from child archive evidence alone.

## Support-Matrix Rows Moved

- `Background process management` - Supported subset.
- `Async IO wakeups` - Split into supported reactor timer/TCP subset and deferred all-host owned-fd readiness.
- `User-defined Future support` - Supported subset with deferred exhaustive diagnostics.
- `Multi-operand select` - Supported subset for 2..8.
- `Select loser cancellation` - Supported subset via `select_cancel`.
- `Task cancellation boundaries` - Supported subset.
- `Process cancellation` - Supported subset.
- `Terminal/fd APIs` - Accepted risk.
- `Recursive file transfer` - Accepted risk.
- `Shell pipelines` - Supported subset.
- `Owned string return boundaries` - Supported subset.
- `Package/test/doc diagnostics` - Supported subset.

Each future status change must continue to cite the child change and concrete
proof path.
