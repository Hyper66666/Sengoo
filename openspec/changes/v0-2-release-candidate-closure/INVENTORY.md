# v0.2 Release Candidate Closure Inventory

Snapshot date: 2026-07-23

## Baseline

| Item | Evidence | Disposition |
| --- | --- | --- |
| Remote main | `origin/main` at `af18e8fadaa20ce99ccea1b087ca57c4c3266859` | Integration base for this change; PR #51 merged |
| Published toolchain | `v0.1.0-rc.1` | Retain for upgrade and rollback evidence |
| OpenSpec baseline | `openspec validate --all --strict`: 50 passed | Green before this change |
| LSP checkpoint | `codex/sglsp-smart-completion-checkpoint` is 0 ahead / 79 behind `origin/main` | Already contained; do not merge again |
| Production-hardening branch | `codex/production-hardening-v1` is contained in `origin/main` | Already contained; preserve history only |
| v0.2 program branch | `codex/sengoo-v0-2-openspec` is contained in `origin/main` | Already contained; residuals consumed here |
| HTTP TLS owner | `codex/v0-2-http-tls` at `3a2bf29cc` is contained in `origin/main`; PR #51 checks passed | No unique source delta remains; finish composition/fixture evidence under `http-production-serving` |

The local `main` matches `origin/main`. Unrelated untracked root directories and
generated worktree transcripts are not release evidence. No required release
claim may depend on them.

## Residuals Consumed

1. Complete and archive `http-production-serving`, including TLS + Router +
   keep-alive + streaming composition and POSIX host proof.
2. Reconcile historical active umbrellas whose code/evidence has moved ahead of
   their task state, especially `mainstream-adoption-gap-closure` and
   `six-pillar-gap-closure`.
3. Run one-SHA native safety, fuzz, performance, compatibility, installed
   realworld, and strict OpenSpec matrices.
4. Produce two consecutive v0.2 candidate runs with retained artifacts and
   executable upgrade/rollback evidence.
5. Publish release-shaped four-host artifacts and then `v0.2.0` without
   overstating portable targets or platform-specific stdlib rows.

## Capability Ownership

| Area | Owner | This change's role |
| --- | --- | --- |
| HTTP production serving | `http-production-serving` | Block on archive and consume evidence |
| Mainline convergence | `integration-baseline` | Tighten release convergence requirement |
| Safety/compat/perf/fuzz | `production-hardening` | Pin one-SHA host and RC gates |
| Archives/install/provenance | `toolchain-distribution` | Align canonical four-target matrix |
| WASM/bytecode | Existing experimental backend specs | Explicit non-goal |

## Evidence Manifest Fields

The retained release evidence manifest must record at least:

- candidate/stable version and full commit SHA;
- workflow run URLs and per-host job conclusions;
- archive filenames, SHA-256 values, and provenance identifiers;
- installed-tool version outputs for `sgc`, `sgpm`, `sgfmt`, and `sglsp`;
- compatibility fixture versions and lockfile hashes;
- safety/fuzz/performance report paths and threshold results;
- known platform-specific skips with matching support-matrix rows.
