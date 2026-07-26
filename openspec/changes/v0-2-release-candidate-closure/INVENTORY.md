# v0.2 Release Candidate Closure Inventory

Snapshot date: 2026-07-26

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

## Candidate Branch Pre-Merge Evidence

PR #54 candidate SHA
`39a6b036aff5892eedf56e4f32ec249c07124c00` passed the complete branch gate
set below. Every listed run reports that exact full SHA; no older run is used
as candidate evidence.

| Gate | Retained run | Result |
| --- | --- | --- |
| Core conformance | [30180980354](https://github.com/Hyper66666/Sengoo/actions/runs/30180980354) | Passed |
| Native safety | [30180980339](https://github.com/Hyper66666/Sengoo/actions/runs/30180980339) | Passed |
| Hardening fuzz | [30180980340](https://github.com/Hyper66666/Sengoo/actions/runs/30180980340) | Passed |
| Compatibility prerelease | [30180980389](https://github.com/Hyper66666/Sengoo/actions/runs/30180980389) | Passed |
| Installed realworld | [30180980373](https://github.com/Hyper66666/Sengoo/actions/runs/30180980373) | Passed on Windows x64, Linux x64, macOS x64, and macOS arm64 |
| Toolchain distribution | [30180980360](https://github.com/Hyper66666/Sengoo/actions/runs/30180980360) | Passed package/install/upgrade smoke on all four hosts |
| Production performance | [30180980357](https://github.com/Hyper66666/Sengoo/actions/runs/30180980357) | Passed the committed 100k/1000k and resource budgets |

The distribution run executed the verified-CA and `localhost` hostname path
for Schannel on Windows and rustls on Linux plus both macOS architectures. It
composed managed TLS Buffers, the Sengoo `Vec` Router, keep-alive, and chunked
response streaming in both the runtime test and a real `sgc` program. The
installed realworld run then repeated that composition using the packaged
toolchain outside the checkout and passed the locked `http-echo-service`
fixture on every host. These runs close tasks 2.2 and 2.3.

Retained workflow artifacts include `debugger-native-cdb-transcripts`
(artifact `8625599149`), all four `sengoo-toolchain-*` archives, Linux and
Windows reproducibility bundles, and `production-performance-evidence`
(artifact `8625840339`). These are pre-merge branch artifacts, not published
release assets. Section 3 remains open until the converged `main` SHA reruns
the matrix and the matching tag produces `release-evidence.json`, provenance,
and published transition transcripts.

## Converged Main and Candidate 1

PR #54 merged to `main` as
`f5a09c4baa83f539c5d7e889c9fce3d23e2b4289`. The complete main matrix used
only that SHA:

| Gate | Retained run | Result |
| --- | --- | --- |
| Core conformance / LLDB | [30183586698](https://github.com/Hyper66666/Sengoo/actions/runs/30183586698) | Passed |
| Native safety | [30183586670](https://github.com/Hyper66666/Sengoo/actions/runs/30183586670) | Passed |
| Hardening fuzz | [30183586677](https://github.com/Hyper66666/Sengoo/actions/runs/30183586677) | Passed |
| Compatibility prerelease | [30183586694](https://github.com/Hyper66666/Sengoo/actions/runs/30183586694) | Passed |
| Installed realworld | [30183586721](https://github.com/Hyper66666/Sengoo/actions/runs/30183586721) | Passed on all four hosts |
| Production performance | [30183586679](https://github.com/Hyper66666/Sengoo/actions/runs/30183586679) | Passed 100k/1000k and resource budgets |
| Manual main distribution | [30183605458](https://github.com/Hyper66666/Sengoo/actions/runs/30183605458) | Passed on all four hosts |

Annotated tag `v0.2.0-rc.1` points to the same full SHA. Tag run
[30184545506](https://github.com/Hyper66666/Sengoo/actions/runs/30184545506)
passed four package jobs, four published upgrade/rollback jobs, the one-SHA
collector, provenance attestation, evidence generation, and publication. The
published prerelease is
[v0.2.0-rc.1](https://github.com/Hyper66666/Sengoo/releases/tag/v0.2.0-rc.1),
with provenance attestation `37127632`.

| Target | Published archive SHA-256 |
| --- | --- |
| `aarch64-apple-darwin` | `0689b17c3383d59fd5fa0834be37c6bf22e38cf0e498e6c1457576d5dd4fef0e` |
| `x86_64-apple-darwin` | `e24e10dc460b0c2bb7b359a5942cb4c5265fd8df4f1ce38a918a89e1ef6f0e67` |
| `x86_64-pc-windows-msvc` | `9915908f7941fe01ceadb65d065ff4a8124a9350a230a1c1e7eac212541b8562` |
| `x86_64-unknown-linux-gnu` | `ff55c4608836bce3a1eceef9e806a90d7124f879e28b40e427bf3253b1f175e3` |

Post-publication audit downloaded all nine release assets, matched every
sidecar, independently verified GitHub provenance for all four archives, and
validated the evidence manifest's six main runs, eight distribution jobs,
four target manifests/tool-version sets, and `v0.1.0-rc.1` fixture hashes.
Transition artifacts `8627024903` (Linux), `8627026644` (Windows),
`8627022847` (macOS arm64), and `8627031690` (macOS x64) each retain
`previous`, `upgraded`, and `rolled-back` phases without protected-file edits.

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
