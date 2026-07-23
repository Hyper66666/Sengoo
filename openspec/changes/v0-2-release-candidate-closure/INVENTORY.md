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
| HTTP keep-alive / streaming | `codex/v0-2-http-keepalive` and `codex/v0-2-http-streaming` are both fully contained in `origin/main` | Superseded by merged owner history; do not merge again |
| Senline dogfood rebase | `codex/senline-service-dogfood-rebase` at `609db4dcd`, 53 commits ahead / 0 behind this baseline | Do not merge wholesale: the product owner is incomplete and its post-merge `sgpm` distribution test does not parse. Consume only reviewed general distribution/compiler/runtime slices; leave Senline packages, evidence, and external pin work with `senline-service-dogfood` |
| Historical feature worktrees | All clean `codex/*` worktrees except Senline are 0 commits ahead, or are old pre-main branches hundreds of commits behind | Preserve but treat as contained/superseded; no release evidence comes from them |
| Dirty async worktree | `codex/async-native-execution-sync` is 543 commits behind and has uncommitted March-era async sources | Preserve untouched for owner review; current main's archived async owners and tests are authoritative for v0.2 |
| Old split/toolchain branches | `large-file-splits-*` and `toolchain-roadmap-stdlib` retain old unique commits on a 543-commit-behind base | Intentionally deferred from release convergence; preserve branches, never merge wholesale |

The local `main` matches `origin/main`. Unrelated untracked root directories and
generated worktree transcripts are not release evidence. No required release
claim may depend on them.

PR #51 merged as `af18e8fadaa20ce99ccea1b087ca57c4c3266859` after
compatibility, core, bounded fuzz, native safety, compile-scale, installed
realworld, and Windows/Linux/macOS x64/macOS arm64 package-smoke jobs passed.
The HTTP worktree's only residual (`tls_fix.txt`) is a generated local test
transcript and is neither unique source nor release evidence.

### Senline slice disposition

The following general fixes were reviewed independently of the unfinished
Senline product owner and are integrated by file-bounded patches in this
release branch:

- installed native-runtime packaging and manifest verification from
  `dbff39d2d`, plus `ed280b9a3`, `ccb366cf5`, `5f40cabb5`, and `ba0d03ae3`;
- sgpm runtime-mode/transitive module propagation from `bd6056d29` without
  Senline-only package tests;
- abandoned HTTP-request cleanup and exact Buffer used-length handling from
  `1777562d3`;
- lambda/aggregate drop and branch-local ownership fixes from `9773569ce`,
  `257e8ce0d`, `d7732afa9`, `d7d53dc03`, and `609db4dcd`;
- deterministic link ordering/flags from `592eaa931`, `0676dd408`,
  `3e747e63b`, and the relevant `67401c600` corrections.

Senline domain packages, product workflows, policy/evidence files, and tasks
requiring a writable external Senline revision remain intentionally deferred
to their own change. The malformed final-branch test merge is not copied.

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
