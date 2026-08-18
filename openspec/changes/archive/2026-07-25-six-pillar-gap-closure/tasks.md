
> **Historical (v0.2 M0):** This umbrella no longer owns new capability work.
> New native v0.2 implementation is coordinated by `sengoo-v0-2-mainstream-core`
> and its M0-M4 children. Remaining open tasks here are evidence debt only.
## 0. Program setup

- [x] 0.1 Add `INVENTORY.md` baseline for all six pillars with current evidence links.
- [x] 0.2 Run `openspec validate six-pillar-gap-closure --strict`.
- [x] 0.3 Map cross-pillar dependencies in `INVENTORY.md` (see `design.md`).
- [x] 0.4 Snapshot current `SUPPORT_MATRIX.md` rows that this change intends to move.
- [x] 0.5 Create the six child changes named in `proposal.md`, each with its owned capability deltas and archive gate.
- [x] 0.6 Link every child proposal back to this umbrella and record its owner/status in `INVENTORY.md`.
- [x] 0.7 Freeze public API and semantic tables in `design.md`; any later public-name or behavior change must update the table before code edits.
- [x] 0.8 Archive completed upstream changes in dependency order, or list them as explicit blockers before a child claims the same capability.
  - Archived/consumed: `stdlib-production-surface`,
    `async-reactor-futures`, `sgpm-alias-multiversion`,
    `language-surface-expansion`, `toolchain-internal-ux`,
    `async-default-followups`, `language-default-polish`,
    `mainstream-default-readiness`, `mainstream-production-readiness`,
    `package-release-defaults`, `stdlib-default-followups`,
    `stdlib-http-server-handlers`, `stdlib-https-tls`.
  - Original children are archived/consumed. `frontend-1000k-perf-gate` was
    superseded by archived `compile-scale-production-gate` and archived with
    `--skip-specs` to avoid re-applying overlapping canonical deltas.

## 1. Pillar 6 - Toolchain quick wins (unblock other lanes)

### Assertions and test output

- [x] 1.1 Extend the existing `std::assert` typed helpers with readable expected/actual failure messages; keep `std::error` compatibility.
- [x] 1.2 Preserve non-zero assert exits and expose a schema-version-1 envelope through the runner-owned `SENGOO_ASSERT_REPORT` result path.
- [x] 1.3 Extend `sgc test` JSON schema with optional `assertion { schema_version, helper, message, file, line, expected, actual }` data without removing existing fields.
- [x] 1.4 Migrate at least one realworld smoke test to use assert helpers.

### Real e2e

- [x] 1.5 Add `tools/sgpm/tests/realworld_e2e.rs` (or feature-gated integration tests) using real `sgc`/`sgpm`.
- [x] 1.6 Add CI job `realworld-e2e` documented in `tasks.md` verification section.
- [x] 1.7 Remove or narrow fake-`sgc` coverage where real e2e supersedes it.
  - Fake-delegation coverage remains for focused sgpm unit/integration behavior;
    real package-loop coverage lives in `tools/sgpm/tests/realworld_e2e.rs` and
    `.github/workflows/realworld-e2e.yml`.

### Debugger and editor

- [x] 1.8 Add `docs/debugging-native.md` with lldb/Windows steps validated manually.
- [x] 1.9 Add `docs/editor-setup.md` linking `sglsp` launch, fmt-on-save, JSON diagnostics.
- [x] 1.10 Add `docs/internal-release.md` with versioned binary smoke matrix.

## 2. Pillar 1 - Stdlib production surface

### Owned string return ABI

- [x] 2.1 In `stdlib-production-surface`, add deltas for `owned-string-text` and `stdlib-mainstream-usability`.
- [x] 2.2 Add the pinned path/directory `_string` helpers returning `Result<String, i64>`; keep existing Buffer APIs unchanged.
- [x] 2.3 Add `JsonValue.string_value()` and `json_value_as_string(value)` owned `String` returns.
- [x] 2.4 Update `sglsp` signatures and realworld examples to stop using raw `ffi_buffer_*`.
  - Public support is recorded as a subset; Buffer APIs intentionally remain for
    lower-level examples and compatibility.

### String collections

- [x] 2.5 Add runtime backing for `Vec<String>` in `runtime_collections.c`.
- [x] 2.6 Expose `std::collections` `Vec<String>` and `StringMapString` wrappers.
- [x] 2.7 Add compiler + native tests for move-on-insert, clone-on-read, transfer-on-remove, drop, and invalid-handle behavior.

### JSON expansion

- [x] 2.8 Raise JSON input cap to at least 1 MiB with constant in `runtime_shared.h`.
- [x] 2.9 Add oversize JSON negative tests in `sgc` hardening or stdlib tests.
- [x] 2.10 Document remaining JSON gaps (streaming, JSON5) as Deferred in matrix.

### Recursive filesystem

- [x] 2.11 Add `dir_walk`, `dir_copy_tree`, `dir_remove_tree` with depth/entry limits.
- [x] 2.12 Add platform behavior tests (symlink, permission) or accepted skips.
- [x] 2.13 Add `examples/realworld` or stdlib example exercising tree copy.
  - Recursive transfer is recorded as `Accepted risk` in SUPPORT_MATRIX because
    coverage is stdlib/runtime-test-backed rather than a dedicated realworld
    tree-copy fixture.

### Process pipes and background

- [x] 2.14 Implement `ProcessCommand.pipe_stdout_to(child) -> Result<ProcessCommand, i64>` with the ownership, final-stage output, and shell-free semantics pinned in `design.md`.
- [x] 2.15 Implement `ProcessCommand.spawn()` and generation-checked `ProcessHandle.wait/kill/exit_code/close` with pinned result/status shapes.
- [x] 2.16 Add argv-safety tests for piped commands (no shell).

### Sync fd IO

- [x] 2.17 Extend `std::io` with fd read/write subset for internal CLI use.
- [x] 2.18 Document fd API host scope in `docs/runtime-platform-behavior.md`.
  - Sync fd helpers are recorded as `Accepted risk` until a realworld fixture
    needs the surface.

## 3. Pillar 4 - Language surface expansion

### Attributes

- [x] 3.1 Publish allowed attribute table in `design.md` implementation section.
- [x] 3.2 Implement `#[derive]`, `#[cfg(target_os = "...")]`, and `#[deprecated]` only on the declaration kinds allowed by that table.
- [x] 3.3 Implement cfg filtering and deprecated-use warnings, then replace blanket "attributes not supported" errors with per-attribute/site diagnostics.

### Class header traits

- [x] 3.4 Parse `class Name: Base, TraitA, TraitB` in `object_declarations.rs`.
- [x] 3.5 Typecheck first-path class/trait disambiguation, at most one base class, and trait-only headers.
- [x] 3.6 Add codegen/tests for trait method calls on class types.

### FFI widening

- [x] 3.7 Extend the dynamic native i64 ABI arity bucket from `0..=4` to `0..=8`; keep aggregate and owned-value signatures unsupported.
- [x] 3.8 Add positive/negative tests; keep `runtime-hardening-ffi-async` suite green.

### Async restriction cleanup

- [x] 3.9 Audit "phase-1" async diagnostics and remove those superseded by Pillar 2.
- [x] 3.10 Add one regression test per removed restriction.

## 4. Pillar 3 - Package graph maturity

### Renamed dependencies

- [x] 4.1 Add `package = "actual_name"` to dependency tables in the manifest schema.
- [x] 4.2 Update `sgpm` resolver to map dependency keys to package names.
- [x] 4.3 Update `sgpm metadata --format json` with alias mapping.
- [x] 4.4 Add integration tests for alias resolution and mismatch diagnostics.

### Multi-version resolution

- [x] 4.5 Define package identity as `(name, version, source)` and record aliases on dependency edges.
- [x] 4.6 Bump the lockfile schema to `version = 2`; retain compatible v1 reads, deterministic `sgpm update` migration, and no writes from locked/check/build/test commands.
- [x] 4.7 Add resolver tests for diamond graphs with two versions.
- [x] 4.8 Document limitations (same source path, conflicting features) in `docs/sgpm-quickstart.md`.

### Internal registry workflow

- [x] 4.9 Document internal monorepo + registry workflow in `docs/sgpm-quickstart.md`.
- [x] 4.10 Add workspace example using alias + multi-version if feasible.

## 5. Pillar 2 - Mainstream async runtime

### Reactor

- [x] 5.1 Add `runtime/src/async_runtime/reactor.rs` with timer, socket, and supported owned-fd readiness registration.
- [x] 5.2 Implement Windows and POSIX readiness backends behind a trait.
- [x] 5.3 Bridge reactor wakeups to `CoroutineScheduler` deadline/poll hints.
  - Current claim is a supported subset. Full all-host owned-fd readiness remains
    deferred in SUPPORT_MATRIX.

### Future trait and flow

- [x] 5.4 Define the pinned `Poll<T>`, `Future<T>::poll(&mut self, ctx)`, and poll-scoped opaque async context surface in the `async-reactor-futures` child change before compiler edits.
- [x] 5.5 Lower user-defined futures to poll vtables.
- [x] 5.6 Relax future escape rules per `spec.md` with negative tests for concurrent/reentrant polling, polling after `Ready`, and context escape.

### Select and cancellation

- [x] 5.7 Implement homogeneous variadic `select` for 2..8 operands with rotating poll order.
- [x] 5.8 Document and test loser policy: losers are not canceled by default and are dropped through normal future cleanup.
- [x] 5.9 Preserve existing non-canceling `timeout(future, ms)` and add consuming `timeout_cancel(future, ms) -> Result<T, i64>` with cancel/drop tests.

### Native and stdlib integration

- [x] 5.10 Add async TCP client example (http:// or raw TCP) using reactor.
- [x] 5.11 Fix/prevent `LNK2019` async dispatch failures on Windows CI.
- [x] 5.12 Update `docs/runtime-async-semantics.md` for reactor and N-select.

## 6. Pillar 5 - Large-scale compile performance

- [x] 6.1 Record the reference CI host profile, compiler revisions, generator seed, C++ command, and three-run median baselines in `INVENTORY.md`.
  - Superseded by archived `compile-scale-production-gate` evidence copied from
    `frontend-1000k-perf-gate`.
- [x] 6.2 Implement frontend memory reductions (interning, pruning, default mode tuning).
  - Covered by `compile-scale-production-gate` phase-timing and pruning evidence.
- [x] 6.3 Add permanent CI perf gates for the absolute targets and 10% RSS/time plus 5 percentage-point frontend-share regression thresholds from `design.md`.
  - `.github/workflows/perf-smoke.yml`, `bench/scripts/advanced-kpi-gate.py`,
    and PR #22 `compile-scale-production-gate` check remain green.
- [x] 6.4 Verify runtime cache fingerprint correctness after perf changes.
  - Covered by `compile-scale-production-gate` sgc/runtime fingerprint tests.
- [x] 6.5 Document `--low-memory` recommendation for 1000k until default mode meets target.
  - Historical mitigation remains in README/bench docs; default-mode target is
    now closed by the passing compile-scale evidence.
- [x] 6.6 If an interim run misses the target, record the measured ratio, host profile, and mitigation without marking the Pillar 5 child complete.
  - Failing and passing reference-host runs are recorded in archived
    `compile-scale-production-gate/INVENTORY.md`.

## 7. Integration and documentation

- [x] 7.1 Refresh `examples/realworld/SUPPORT_MATRIX.md` for all moved capabilities.
- [x] 7.2 Update `README.md` / `README.zh-CN.md` with six-pillar closure summary.
- [x] 7.3 Add `docs/plans/2026-06-six-pillar-gap-closure.md` implementation plan mirror.
- [x] 7.4 Run subagent or peer review on spec acceptance gaps before implementation wave.
- [x] 7.5 Ensure each child change points back to this umbrella and each completed pillar updates the canonical capability before umbrella archive.
  - Re-synced `INVENTORY.md` after child archives. The umbrella still stays
    active for POSIX/reference-host evidence and final verification.

## 8. Verification

- [x] 8.1 `cargo fmt --check`
- [x] 8.2 `cargo test -p sengoo-compiler --lib`
- [x] 8.3 `cargo test -p sengoo-runtime --lib --features native-bridge`
- [x] 8.4 `cargo test -p sgc`
- [x] 8.5 `cargo test -p sgpm`
- [x] 8.6 `cargo test -p sglsp`
- [x] 8.7 `cargo clippy -p sgc -p sgpm -p sengoo-compiler -p sengoo-runtime -p sgfmt -p sglsp --all-targets -- -D warnings`
- [x] 8.8 `realworld-e2e` job (locked loop, real binaries)
  Retained PR #51 jobs cover core, compile-scale, safety, four-host package,
  and four-host installed realworld execution; the release-closure branch
  re-ran fmt, Clippy, HTTP integration, and strict OpenSpec after the last
  child archive.
- [x] 8.9 `advanced_pipeline_bench.py` perf gate (100k + 1000k)
  - Archived `compile-scale-production-gate` produced the passing P0-focused
    gate artifact; PR #22 also passed `compile-scale-production-gate` in CI.
- [x] 8.10 `openspec validate six-pillar-gap-closure --strict`
- [x] 8.11 `openspec validate --all --strict`

## Done Definition

- [x] All six child changes are strictly validated, implemented, and archived into their owned canonical capabilities.
- [x] Realworld fixtures run end-to-end with real `sgc`/`sgpm` in CI.
- [x] Internal docs (`debugging-native`, `editor-setup`, `internal-release`) exist.
- [x] `SUPPORT_MATRIX.md` reflects closure status with proof links.
- [x] The 1000k reference perf gate meets the absolute RSS and frontend-share targets and is recorded in `INVENTORY.md`.

## Archive Gate

- [x] `openspec validate six-pillar-gap-closure --strict` passes.
- [x] `openspec validate --all --strict` passes.
- [x] All six required child changes are archived before the umbrella.
  - `frontend-1000k-perf-gate` is archived as superseded by
    `compile-scale-production-gate`; no original child remains active.
- [x] All verification commands in section 8 pass; platform-specific skips document evidence and do not omit a pillar implementation.
