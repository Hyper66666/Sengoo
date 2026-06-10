## 1. Inventory

- [x] 1.1 Run `openspec validate mainstream-default-readiness --strict`.
- [x] 1.2 Run `openspec validate --all --strict`.
- [x] 1.3 Create an inventory table mapping current active changes to P0-P4.
- [x] 1.4 Update support matrices to remove stale rows or mark them with accepted
  deferred status and proof paths.

## 2. P0 Compile Scale

- [x] 2.1 Record that `frontend-1000k-perf-gate` is superseded by
  `compile-scale-production-gate`; the absolute 1000k gate is now closed by
  P0-focused reference-host evidence.
- [x] 2.2 Complete `compile-scale-production-gate` reference-host benchmark:
  required 100k and 1000k evidence, with 2500k as optional/report-only stretch
  when runnable. P0-focused evidence is recorded in
  `bench/results/1780946346830-advanced-pipeline.json`; the gate artifact is
  `bench/results/1780946346830-advanced-pipeline-advanced-gate.json`.
- [x] 2.3 Meet or update the explicit 1000k gate: median RSS <= 1.8x C++ and
  frontend share <= 65%, or keep this umbrella open. Latest P0 evidence passes:
  1000k RSS is 0.11x C++ and frontend share is 31.83%.
- [x] 2.4 Verify runtime fingerprint/native cache behavior remains stable after
  scale optimizations.

## 3. P1 Async And Runtime Maturity

- [x] 3.1 Reconcile `docs/runtime-async-semantics.md` and
  `examples/realworld/SUPPORT_MATRIX.md` after `async-reactor-futures` and
  `concurrent-async-runtime`, and `runtime-hardening-ffi-async` archive. The
  supported subset remains documented; remaining default semantics are deferred
  to `async-default-followups`.
- [x] 3.2 Open child follow-up for remaining async IO/default semantics if owned-fd
  readiness, user `Future` poll lowering, or cancellation semantics remain
  partial: `async-default-followups`.
- [x] 3.3 Add realworld async package smoke that uses only public stdlib APIs:
  `examples/realworld/async-channel-smoke`.
- [x] 3.4 Verify native async paths on Windows and POSIX reference hosts or record
  evidenced platform skips. Windows local native-bridge runs passed; POSIX
  reference-host execution is recorded as skipped in `INVENTORY.md` because this
  workspace has no WSL/POSIX reference host available.

## 4. P2 Ecosystem And Release Defaults

- [x] 4.1 Archive `sgpm-alias-multiversion` or explicitly supersede its package
  graph deltas.
- [x] 4.2 Confirm `ecosystem-toolchain-maturity` owns package graph, metadata,
  cross-compile, LSP dependency, and registry workflow gates, or open narrower
  child changes for unowned deltas. Registry `yanked`/`features` metadata is
  implemented and tested in `ecosystem-toolchain-maturity`.
- [x] 4.3 Open or identify the child change that owns internal release channel
  evidence: versioned binaries, smoke matrix, rollback, and supported host
  policy.
- [x] 4.4 Gate archive on child-owned package workflow docs that a new internal
  project can follow without hidden repo knowledge.

## 5. P3 Stdlib Thickness

- [x] 5.1 Inventory demand-backed stdlib gaps and open child changes only for
  accepted expansions. `stdlib-default-followups` owns compression and
  streaming-data follow-up gates.
- [x] 5.2 Require each stdlib child change to own resource limits and stable
  statuses for its scope: compression, streaming JSON/schema, terminal control,
  file watch, file locks, richer Unicode/text formatting, or network helpers.
  Compression and future streaming data are deferred to
  `stdlib-default-followups`; TLS POSIX/reference-host success remains in
  `stdlib-https-tls`.
- [x] 5.3 Gate archive on child-owned realworld fixtures for accepted stdlib
  expansions. Supported process capture, shell-free pipeline, and background
  handle claims are fixture-backed by `workspace-doc-loop`; runtime-test-only
  tree/fd helper rows are marked `Accepted risk` instead of unconditional
  supported subset.
- [x] 5.4 Gate any Buffer API breakage on a separate migration OpenSpec; otherwise
  require child changes to preserve compatibility. Current accepted rows
  preserve Buffer compatibility; future compression keeps that requirement in
  `stdlib-default-followups`.

## 6. P4 Language Surface Polish

- [x] 6.1 Inventory remaining phase-only restrictions after archived
  `try-and-match-ergonomics` and `owned-string-text` evidence.
- [x] 6.2 Open child changes for additive relaxations in parser, typeck, lowering,
  async frames, FFI, attributes, match/try, and diagnostics.
- [x] 6.3 Gate any source-incompatible cleanup on child-owned migration docs.
- [x] 6.4 Require each language-surface child change to own `sglsp` diagnostic and
  quick-fix parity for newly relaxed/rejected forms. Completed
  `language-surface-expansion` is archived; remaining additive parity work is
  owned by `language-default-polish`, and stable `sgc` JSON diagnostic codes now
  reach `sglsp` quick fixes.

## 7. Integration

- [x] 7.1 `cargo test -p sgc`
- [x] 7.2 `cargo test -p sgpm`
- [x] 7.3 `cargo test -p sglsp`
- [x] 7.4 `cargo clippy -p sgc -p sgpm -p sglsp --all-targets -- -D warnings`
- [x] 7.5 Realworld package loop passes with real toolchain binaries.
- [x] 7.6 Support matrices and docs cite all supported subset claims.

## Archive Gate

- [x] P0 compile-scale gate has real reference-host evidence. The latest
  P0-focused gate passes; see 2.2 and 2.3 for report and target evidence.
- [x] P1-P4 child changes are archived or explicitly deferred with support matrix
  rows.
- [x] No active child change overlaps canonical ownership without a supersession
  note.
- [x] `openspec validate mainstream-default-readiness --strict` passes.
- [x] `openspec validate --all --strict` passes.
