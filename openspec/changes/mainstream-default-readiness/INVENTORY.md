# Inventory (mainstream-default-readiness)

## Priority Mapping

| Priority | Mainstream gap | Current owner | Current state | Next OpenSpec action |
| --- | --- | --- | --- | --- |
| P0 | 1000k compile RSS/frontend share | `compile-scale-production-gate`; superseded evidence from `frontend-1000k-perf-gate` | Closed by focused reference-host evidence: 1000k RSS 0.11x C++ and frontend share 31.83% | Archive `compile-scale-production-gate` with `bench/results/1780946346830-advanced-pipeline.json` and hard P0 gate artifact |
| P1 | Async IO/default runtime maturity | archived `async-reactor-futures`, `concurrent-async-runtime`, `runtime-hardening-ffi-async`; active `async-default-followups` | Supported subsets exist and `async-channel-smoke` proves package-shaped public async use. Windows native evidence passes `cargo test -p sengoo-runtime --lib --features native-bridge async -- --test-threads=1` (35 tests), `cargo test -p sengoo-runtime --features native-bridge concurrent` (7 tests), and `cargo test -p sgc async_native_runtime -- --nocapture --test-threads=1` (36 tests). User-future accepted flows and representative rejected-shape diagnostics use stable `async::user_future_contract` coverage. POSIX reference-host run is skipped in this workspace because `wsl uname -a` reports WSL is not installed. | Use `async-default-followups` for all-host owned-fd readiness, missing wakeup-registration checks, exhaustive rejected-shape snapshots, and future POSIX/reference-host evidence |
| P2 | Package ecosystem and release defaults | archived `ecosystem-toolchain-maturity`, `toolchain-internal-ux`, superseded `sgpm-alias-multiversion`; active `package-release-defaults` | Package graph ownership transferred; registry `yanked`/`features` metadata is implemented and tested; `package-release-defaults` now owns deterministic publish artifacts, registry diagnostics/cache evidence, `package-release-loop`, and release smoke/rollback docs | Use `package-release-defaults` as the archive gate for publish metadata, local/remote registry evidence, and release-channel CI/docs |
| P3 | Stdlib thickness | archived `stdlib-production-surface`, `stdlib-breadth-mainstream`, `stdlib-next-usability-wave`; active `stdlib-https-tls`, `stdlib-default-followups` | Strong internal-tooling subset; process capture/pipelines/background handles are fixture-backed by `workspace-doc-loop`; compression is now a `Supported subset` through deterministic one-shot gzip Buffer helpers and the `compressed-json-artifact` fixture; recursive tree and sync fd helper rows remain accepted-risk/runtime-test-backed until a realworld fixture exists | Keep `stdlib-https-tls` open for POSIX/reference-host TLS success evidence; use future fixture-backed children for streaming compression/JSON/schema, terminal control, file locks/watch streams, Unicode/grapheme/locale behavior, or broader network helpers |
| P4 | Language surface polish | archived `language-surface-expansion`; active `language-default-polish`; archived evidence from `try-and-match-ergonomics` and `owned-string-text` | Pinned cfg predicates, deprecated warnings, stable FFI rejection diagnostics, payload-enum async-frame deferral, and match/try JSON/LSP parity are implemented; payload enum crossing `await` remains intentionally Deferred with a stable diagnostic before LLVM | Keep breaking cleanup behind migration docs; future feature-selection CLI or payload-enum async frame support needs its own accepted OpenSpec evidence |

## Current Evidence Anchors

- `examples/realworld/SUPPORT_MATRIX.md`: user-facing supported/deferred rows.
- `tools/stdlib/README.md`: stdlib source-module summaries, JSON input cap, and current compression placeholders.
- `packages/GRAPHICS_SUPPORT_MATRIX.md`: graphics package supported/deferred rows.
- `docs/runtime-async-semantics.md`: async/runtime supported subset and cleanup
  semantics.
- `openspec/changes/compile-scale-production-gate/tasks.md`: closed P0 gate.
- `openspec/changes/package-release-defaults/tasks.md`: deterministic package
  publish artifacts, registry publish evidence, realworld release fixture, and
  toolchain release smoke owner.
- `openspec/changes/stdlib-https-tls/tasks.md`: reference-host TLS evidence
  blocker.
- `openspec/changes/async-default-followups/tasks.md`: remaining P1 async
  default follow-up owner.
- `openspec/changes/language-default-polish/`: P4 additive ownership for
  remaining language restrictions and diagnostic parity.
- `openspec/changes/stdlib-default-followups/tasks.md`: compression and
  streaming-data follow-up ownership for P3 stdlib thickness.

## Archive Reconciliation

| Lane | Archive/defer state | User-facing evidence |
| --- | --- | --- |
| P0 compile scale | `compile-scale-production-gate` complete; `frontend-1000k-perf-gate` superseded | `bench/results/1780946346830-advanced-pipeline.json`; hard P0 gate artifact |
| P1 async/runtime | Completed async/runtime children are archived; remaining defaults are deferred to `async-default-followups` with matrix rows | `docs/runtime-async-semantics.md`; async rows in `examples/realworld/SUPPORT_MATRIX.md`; Windows local test evidence and POSIX skip recorded above |
| P2 ecosystem/release | Package graph and toolchain UX children are archived; release-channel UX is child-owned by `package-release-defaults` | `examples/realworld/package-release-loop`; `tools/sgpm/tests/integration.rs::realworld_package_release_loop_covers_publish_defaults`; `docs/sgpm-quickstart.md`; package rows in `SUPPORT_MATRIX.md` |
| P3 stdlib thickness | Accepted user-facing stdlib subsets are fixture-backed where claimed as supported; compression is implemented as a bounded `Supported subset`; streaming-data follow-ups remain deferred behind fixture-backed OpenSpec gates; runtime-test-only helpers are marked accepted risk | Stdlib rows in `examples/realworld/SUPPORT_MATRIX.md`; `workspace-doc-loop`; `compressed-json-artifact`; `stdlib-default-followups` design/spec resource-limit gates |
| P4 language polish | Completed `language-surface-expansion` is archived; additive relaxations and diagnostics are implemented in `language-default-polish`; no source-incompatible cleanup is implemented in this umbrella | `language-default-polish` tasks; `tools/sglsp/src/diagnostics.rs` parity tests; `tools/sgc/src/tests.rs::render_compile_error_json_extracts_attribute_code` |

## Ownership Supersession Notes

- `frontend-1000k-perf-gate` remains active but is superseded for this umbrella
  by archived `compile-scale-production-gate`; it should not be used as a
  competing P0 archive blocker.
- `six-pillar-gap-closure` and `mainstream-production-readiness` are predecessor
  umbrellas. This change owns the archive-readiness reconciliation layer; actual
  implementation remains in narrower child changes or in explicitly deferred
  support-matrix rows.
- Active child changes that remain in this contract are intentionally deferred
  owners: `async-default-followups`, `stdlib-default-followups`,
  `stdlib-https-tls`, `language-default-polish`, and
  `package-release-defaults`.

## Open Risks

- Support matrices can drift as active children land; stale `Deferred` rows must
  block archive until reconciled.
- `mainstream-default-readiness` should stay an umbrella. Large implementation
  work belongs in child changes with canonical deltas.
- P4 follow-up ownership remains additive only; source-incompatible cleanup
  still needs migration documentation before implementation.
- P2 registry `yanked`/`features` metadata is no longer a concrete blocker.
  Remaining package-manager and release-channel verification lives in
  `package-release-defaults` until its archive gate passes on reference hosts.
